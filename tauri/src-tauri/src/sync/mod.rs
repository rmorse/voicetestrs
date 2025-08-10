use std::path::{Path, PathBuf};
use std::collections::HashSet;
use std::sync::Arc;
use chrono::{DateTime, Utc};
use walkdir::WalkDir;
use tauri::{AppHandle, Manager, Emitter};

use crate::database::{Database, models::{Transcription, SyncReport}};

pub struct FileSystemSync {
    db: Arc<Database>,
    notes_dir: PathBuf,
}

impl FileSystemSync {
    pub fn new(db: Arc<Database>, notes_dir: PathBuf) -> Self {
        Self {
            db,
            notes_dir,
        }
    }
    
    pub async fn sync_filesystem(&self) -> Result<SyncReport, Box<dyn std::error::Error>> {
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
        // Extract ID from filename (format: YYYYMMDD-HHMMSS-voice-note.wav)
        let file_name = audio_path.file_stem()
            .and_then(|s| s.to_str())
            .ok_or("Invalid file name")?;
        
        // Extract date-time part (first 15 characters)
        let id = if file_name.len() >= 15 {
            file_name[..15].replace("-", "")
        } else {
            // Fallback: use full filename
            file_name.to_string()
        };
        
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
            (None, None, "pending".to_string(), None)
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
            audio_path: audio_path.to_string_lossy().to_string(),
            text_path: text_path_opt,
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
        // Check if any audio file exists with this ID
        let patterns = vec![
            format!("{}-voice-note.wav", id),
            format!("{}.wav", id),
            format!("{}-voice-note.mp3", id),
            format!("{}.mp3", id),
        ];
        
        for pattern in patterns {
            let path = self.notes_dir.join(&pattern);
            if path.exists() {
                return true;
            }
            
            // Also check in date subdirectories
            if id.len() >= 8 {
                let year = &id[..4];
                let month = &id[4..6];
                let day = &id[6..8];
                
                let date_dir = self.notes_dir
                    .join(year)
                    .join(format!("{}-{}-{}", year, month, day));
                
                let date_path = date_dir.join(&pattern);
                if date_path.exists() {
                    return true;
                }
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
    app: AppHandle,
) -> Result<SyncReport, String> {
    // For now, use the project's notes directory
    // TODO: Later migrate to app data dir
    let notes_dir = std::path::PathBuf::from("D:\\projects\\claude\\voicetextrs\\notes");
    
    println!("Starting SQLx filesystem sync from: {:?}", notes_dir);
    
    // Create sync instance and run sync
    let sync = FileSystemSync::new(db.inner().clone(), notes_dir);
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