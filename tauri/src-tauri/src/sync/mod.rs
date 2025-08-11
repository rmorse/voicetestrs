pub mod imports;
pub mod file_watcher;

use std::path::{Path, PathBuf};
use std::collections::HashSet;
use std::sync::Arc;
use chrono::{DateTime, Utc};
use walkdir::WalkDir;
use tauri::{AppHandle, Emitter};

use crate::database::{Database, models::{Transcription, SyncReport}, utils};
use crate::queue_manager::{QueueManager, BackgroundTask, TaskType, TaskPriority, TaskStatus};
use uuid::Uuid;
use serde_json::json;
use chrono::Local;

pub struct FileSystemSync {
    db: Arc<Database>,
    notes_dir: PathBuf,
    queue_manager: Option<Arc<QueueManager>>,
}

impl FileSystemSync {
    pub fn new(db: Arc<Database>, notes_dir: PathBuf) -> Self {
        Self {
            db,
            notes_dir,
            queue_manager: None,
        }
    }
    
    pub fn with_queue_manager(mut self, queue_manager: Arc<QueueManager>) -> Self {
        self.queue_manager = Some(queue_manager);
        self
    }
    
    pub async fn sync_filesystem(&self) -> Result<SyncReport, Box<dyn std::error::Error + Send + Sync>> {
        let mut report = SyncReport::default();
        
        // Get existing IDs from database
        let existing_ids: HashSet<String> = self.db
            .get_all_transcription_ids()
            .await?
            .into_iter()
            .collect();
        
        // Scan filesystem for audio files
        let audio_files = self.scan_audio_files()?;
        report.total_files_found = audio_files.len();
        
        // Process each file
        for audio_path in audio_files {
            match self.process_audio_file(&audio_path, &existing_ids).await {
                Ok(ProcessResult::New) => report.new_transcriptions += 1,
                Ok(ProcessResult::Updated) => report.updated_transcriptions += 1,
                Ok(ProcessResult::Unchanged) => {},
                Err(e) => {
                    report.errors.push(format!("Error processing {:?}: {}", audio_path, e));
                }
            }
        }
        
        // Check for deleted files (mark as orphaned)
        for id in &existing_ids {
            if !self.file_exists_for_id(id) {
                if let Err(e) = self.db.update_transcription_status(id, "orphaned", None).await {
                    report.errors.push(format!("Error marking {} as orphaned: {}", id, e));
                } else {
                    report.missing_files += 1;
                }
            }
        }
        
        Ok(report)
    }
    
    fn scan_audio_files(&self) -> Result<Vec<PathBuf>, std::io::Error> {
        let mut audio_files = Vec::new();
        
        for entry in WalkDir::new(&self.notes_dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "wav" || ext == "mp3" || ext == "m4a" || ext == "ogg" {
                        audio_files.push(path.to_path_buf());
                    }
                }
            }
        }
        
        Ok(audio_files)
    }
    
    async fn process_audio_file(
        &self, 
        audio_path: &Path,
        existing_ids: &HashSet<String>
    ) -> Result<ProcessResult, Box<dyn std::error::Error>> {
        let transcription = self.create_transcription_from_file(audio_path)?;
        
        if !existing_ids.contains(&transcription.id) {
            // New file - insert
            self.db.insert_transcription(&transcription).await?;
            
            // If it's orphaned (no transcription), enqueue for background processing
            if transcription.status == "orphaned" {
                if let Some(ref queue_manager) = self.queue_manager {
                    let output_path = audio_path.with_extension("txt");
                    let task = BackgroundTask {
                        id: Uuid::new_v4().to_string(),
                        transcription_id: transcription.id.clone(),
                        task_type: TaskType::TranscribeOrphan {
                            audio_path: audio_path.to_string_lossy().to_string(),
                            output_path: output_path.to_string_lossy().to_string(),
                        },
                        priority: TaskPriority::Low,
                        status: TaskStatus::Pending,
                        created_at: Local::now(),
                        started_at: None,
                        completed_at: None,
                        retry_count: 0,
                        max_retries: 2,
                        error_message: None,
                        payload: json!({
                            "audio_path": audio_path.to_string_lossy().to_string(),
                        }),
                    };
                    
                    if let Err(e) = queue_manager.enqueue_task(&self.db, task).await {
                        log::error!("Failed to enqueue orphaned file {}: {}", transcription.id, e);
                    } else {
                        log::info!("Enqueued orphaned file {} for background transcription", transcription.id);
                    }
                }
            }
            
            Ok(ProcessResult::New)
        } else {
            // Check if needs update
            if let Some(existing) = self.db.get_transcription(&transcription.id).await? {
                if self.needs_update(&existing, &transcription) {
                    // For now, we'll just update the status if different
                    // In the future, we might update more fields
                    Ok(ProcessResult::Updated)
                } else {
                    Ok(ProcessResult::Unchanged)
                }
            } else {
                Ok(ProcessResult::Unchanged)
            }
        }
    }
    
    fn create_transcription_from_file(&self, audio_path: &Path) -> Result<Transcription, Box<dyn std::error::Error>> {
        // Extract ID from filename using utility function
        let file_name = audio_path.file_name()
            .and_then(|s| s.to_str())
            .ok_or("Invalid file name")?;
        
        let id = utils::generate_id_from_filename(file_name);
        
        // Get file metadata
        let metadata = std::fs::metadata(audio_path)?;
        let file_size_bytes = metadata.len() as i64;
        let created_at = metadata.created()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| DateTime::<Utc>::from_timestamp(d.as_secs() as i64, 0))
            .flatten()
            .unwrap_or_else(Utc::now);
        
        // Check for corresponding text file
        let text_path = audio_path.with_extension("txt");
        let (text_path_opt, transcription_text, status, transcribed_at) = if text_path.exists() {
            let text = std::fs::read_to_string(&text_path).ok();
            let transcribed_at = std::fs::metadata(&text_path)
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| DateTime::<Utc>::from_timestamp(d.as_secs() as i64, 0))
                .flatten();
            
            (
                Some(text_path.to_string_lossy().to_string()),
                text,
                "complete".to_string(),
                transcribed_at
            )
        } else {
            // File has no transcription - mark as orphaned and potentially queue for processing
            (None, None, "orphaned".to_string(), None)
        };
        
        // Check for JSON metadata
        let json_path = audio_path.with_extension("json");
        let metadata_json = if json_path.exists() {
            std::fs::read_to_string(&json_path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .map(|v| sqlx::types::Json(v))
        } else {
            None
        };
        
        Ok(Transcription {
            id,
            audio_path: utils::normalize_audio_path(audio_path),
            text_path: text_path_opt.map(|p| utils::normalize_audio_path(Path::new(&p))),
            transcription_text,
            created_at,
            transcribed_at,
            duration_seconds: 0.0, // Will be calculated later
            file_size_bytes,
            language: "en".to_string(),
            model: "base.en".to_string(),
            status,
            source: "import".to_string(),
            error_message: None,
            metadata: metadata_json,
            session_id: None,
        })
    }
    
    fn needs_update(&self, existing: &Transcription, new: &Transcription) -> bool {
        // Check if file has been modified since last sync
        existing.status != new.status ||
        existing.transcription_text != new.transcription_text ||
        existing.file_size_bytes != new.file_size_bytes
    }
    
    fn file_exists_for_id(&self, id: &str) -> bool {
        // ID format is YYYYMMDDHHMMSS, but filenames are just HHMMSS-voice-note.wav
        // Extract the time portion from the ID
        let time_part = if id.len() >= 14 {
            &id[8..14] // Skip YYYYMMDD, get HHMMSS
        } else if id.len() == 6 {
            id // Already just time
        } else {
            return false; // Invalid ID format
        };
        
        // Extract date parts for directory structure
        let (year, month, day) = if id.len() >= 8 {
            (&id[..4], &id[4..6], &id[6..8])
        } else {
            // Default to 2025-08-10 if no date in ID
            ("2025", "08", "10")
        };
        
        // Check in the date subdirectory
        let date_dir = self.notes_dir
            .join(year)
            .join(format!("{}-{}-{}", year, month, day));
        
        // Check for files with just the time portion
        let patterns = vec![
            format!("{}-voice-note.wav", time_part),
            format!("{}-voice-note.mp3", time_part),
            format!("{}-voice-note.m4a", time_part),
            format!("{}.wav", time_part),
        ];
        
        for pattern in patterns {
            let date_path = date_dir.join(&pattern);
            if date_path.exists() {
                return true;
            }
        }
        false
    }
}

enum ProcessResult {
    New,
    Updated,
    Unchanged,
}

// Tauri command for filesystem sync
#[tauri::command]
pub async fn sync_filesystem_sqlx(
    db: tauri::State<'_, Arc<Database>>,
    queue: tauri::State<'_, Arc<QueueManager>>,
    app: AppHandle,
) -> Result<SyncReport, String> {
    // For now, use the project's notes directory
    // TODO: Later migrate to app data dir
    let notes_dir = std::path::PathBuf::from("D:\\projects\\claude\\voicetextrs\\notes");
    
    println!("Starting SQLx filesystem sync from: {:?}", notes_dir);
    
    // Create sync instance with queue manager and run sync
    let sync = FileSystemSync::new(db.inner().clone(), notes_dir)
        .with_queue_manager(queue.inner().clone());
    let report = sync.sync_filesystem().await
        .map_err(|e| {
            eprintln!("Sync failed: {}", e);
            e.to_string()
        })?;
    
    println!("SQLx sync completed: {:?}", report);
    
    // Emit update event
    app.emit("sync-complete", &report)
        .map_err(|e| e.to_string())?;
    
    Ok(report)
}