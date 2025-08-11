use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tauri::{Emitter, AppHandle};

use crate::database::Database;
use crate::sync::imports::ImportProcessor;

pub struct FileWatcher {
    db: Arc<Database>,
    notes_dir: PathBuf,
    imports_dir: PathBuf,
    app_handle: Option<AppHandle>,
}

impl FileWatcher {
    pub fn new(db: Arc<Database>, notes_dir: PathBuf, imports_dir: PathBuf) -> Self {
        Self {
            db,
            notes_dir,
            imports_dir,
            app_handle: None,
        }
    }
    
    pub fn set_app_handle(&mut self, handle: AppHandle) {
        self.app_handle = Some(handle);
    }
    
    pub async fn start_watching(self: Arc<Self>) -> Result<(), Box<dyn std::error::Error>> {
        let (tx, mut rx) = mpsc::channel(100);
        
        // Create the watcher with debouncing
        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    let _ = tx.blocking_send(event);
                }
            },
            Config::default()
                .with_poll_interval(Duration::from_secs(2))
                .with_compare_contents(false),
        )?;
        
        // Watch the notes directory recursively
        watcher.watch(&self.notes_dir, RecursiveMode::Recursive)?;
        
        // Watch the imports/pending directory
        let imports_pending = self.imports_dir.join("pending");
        if imports_pending.exists() {
            watcher.watch(&imports_pending, RecursiveMode::NonRecursive)?;
        }
        
        log::info!("File watcher started for {} and {}", 
            self.notes_dir.display(), imports_pending.display());
        
        // Process events
        while let Some(event) = rx.recv().await {
            self.handle_event(event).await;
        }
        
        Ok(())
    }
    
    async fn handle_event(&self, event: Event) {
        match event.kind {
            EventKind::Create(_) => {
                for path in event.paths {
                    self.handle_file_created(&path).await;
                }
            }
            EventKind::Modify(_) => {
                for path in event.paths {
                    self.handle_file_modified(&path).await;
                }
            }
            EventKind::Remove(_) => {
                for path in event.paths {
                    self.handle_file_removed(&path).await;
                }
            }
            _ => {}
        }
    }
    
    async fn handle_file_created(&self, path: &Path) {
        // Check if it's an import file
        if path.starts_with(&self.imports_dir.join("pending")) {
            if self.is_audio_file(path) {
                log::info!("New import detected: {}", path.display());
                
                // Queue the import for processing
                let processor = ImportProcessor::new(
                    self.db.clone(),
                    self.imports_dir.clone(),
                    self.notes_dir.clone(),
                );
                
                if let Err(e) = processor.queue_import(path).await {
                    log::error!("Failed to queue import {}: {}", path.display(), e);
                } else {
                    // Notify UI about new import
                    if let Some(ref handle) = self.app_handle {
                        let _ = handle.emit("import-queued", serde_json::json!({
                            "path": path.to_string_lossy(),
                            "timestamp": chrono::Local::now().to_rfc3339(),
                        }));
                    }
                }
            }
        }
        // Check if it's a new audio file in notes
        else if path.starts_with(&self.notes_dir) && self.is_audio_file(path) {
            log::info!("New audio file detected: {}", path.display());
            
            // Check if it already has a transcription
            let txt_path = path.with_extension("txt");
            if !txt_path.exists() {
                // This is an orphaned audio file, queue it for transcription
                self.queue_orphaned_file(path).await;
            }
        }
    }
    
    async fn handle_file_modified(&self, path: &Path) {
        // We primarily care about transcription text files being modified
        if path.starts_with(&self.notes_dir) && path.extension() == Some(std::ffi::OsStr::new("txt")) {
            log::debug!("Transcription modified: {}", path.display());
            
            // Update the database with the new content
            if let Ok(content) = std::fs::read_to_string(path) {
                let id = self.extract_id_from_path(path);
                
                if let Err(e) = self.update_transcription_text(&id, &content).await {
                    log::error!("Failed to update transcription {}: {}", id, e);
                }
                
                // Notify UI about the update
                if let Some(ref handle) = self.app_handle {
                    let _ = handle.emit("transcription-modified", serde_json::json!({
                        "id": id,
                        "path": path.to_string_lossy(),
                    }));
                }
            }
        }
    }
    
    async fn handle_file_removed(&self, path: &Path) {
        if path.starts_with(&self.notes_dir) {
            log::info!("File removed: {}", path.display());
            
            // If it's an audio file, mark the transcription as deleted
            if self.is_audio_file(path) {
                let id = self.extract_id_from_path(path);
                
                if let Err(e) = self.mark_transcription_deleted(&id).await {
                    log::error!("Failed to mark transcription {} as deleted: {}", id, e);
                }
                
                // Notify UI about the deletion
                if let Some(ref handle) = self.app_handle {
                    let _ = handle.emit("transcription-deleted", serde_json::json!({
                        "id": id,
                        "path": path.to_string_lossy(),
                    }));
                }
            }
        }
    }
    
    fn is_audio_file(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            let ext_lower = ext.to_string_lossy().to_lowercase();
            matches!(ext_lower.as_str(), "wav" | "mp3" | "m4a" | "ogg" | "flac" | "webm")
        } else {
            false
        }
    }
    
    fn extract_id_from_path(&self, path: &Path) -> String {
        // Extract ID from filename (e.g., "20250810143323" from "143323-voice-note.wav")
        if let Some(stem) = path.file_stem() {
            let filename = stem.to_string_lossy();
            
            // Handle our standard format: HHMMSS-voice-note
            if let Some(time_part) = filename.split('-').next() {
                if time_part.len() == 6 {
                    // Get the date from the parent directory
                    if let Some(parent) = path.parent() {
                        if let Some(date_dir) = parent.file_name() {
                            let date_str = date_dir.to_string_lossy();
                            // Convert YYYY-MM-DD to YYYYMMDD
                            let date_compact = date_str.replace("-", "");
                            if date_compact.len() == 8 {
                                return format!("{}{}", date_compact, time_part);
                            }
                        }
                    }
                }
            }
        }
        
        // Fallback to UUID if we can't extract a proper ID
        uuid::Uuid::new_v4().to_string()
    }
    
    async fn queue_orphaned_file(&self, path: &Path) {
        let task_id = uuid::Uuid::new_v4().to_string();
        let transcription_id = self.extract_id_from_path(path);
        let output_path = path.with_extension("txt");
        
        let payload = serde_json::json!({
            "type": "TranscribeOrphan",
            "audio_path": path.to_string_lossy(),
            "output_path": output_path.to_string_lossy(),
        });
        
        let pool = self.db.pool();
        
        // Add to transcriptions table if not exists
        let _ = sqlx::query(
            "INSERT OR IGNORE INTO transcriptions (id, audio_path, status, source, created_at)
             VALUES (?, ?, 'pending', 'orphan', datetime('now'))"
        )
        .bind(&transcription_id)
        .bind(path.to_string_lossy().as_ref())
        .execute(pool)
        .await;
        
        // Add to background tasks
        if let Err(e) = sqlx::query(
            "INSERT INTO background_tasks (id, transcription_id, task_type, priority, status, payload, created_at, retry_count, max_retries)
             VALUES (?, ?, 'TranscribeOrphan', 0, 'pending', ?, datetime('now'), 0, 2)"
        )
        .bind(&task_id)
        .bind(&transcription_id)
        .bind(payload.to_string())
        .execute(pool)
        .await {
            log::error!("Failed to queue orphaned file {}: {}", path.display(), e);
        } else {
            log::info!("Queued orphaned file for transcription: {}", path.display());
        }
    }
    
    async fn update_transcription_text(&self, id: &str, content: &str) -> Result<(), Box<dyn std::error::Error>> {
        let pool = self.db.pool();
        
        sqlx::query(
            "UPDATE transcriptions SET transcription_text = ?, updated_at = datetime('now') WHERE id = ?"
        )
        .bind(content)
        .bind(id)
        .execute(pool)
        .await?;
        
        Ok(())
    }
    
    async fn mark_transcription_deleted(&self, id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let pool = self.db.pool();
        
        // Soft delete - just mark as deleted
        sqlx::query(
            "UPDATE transcriptions SET status = 'deleted', updated_at = datetime('now') WHERE id = ?"
        )
        .bind(id)
        .execute(pool)
        .await?;
        
        Ok(())
    }
}