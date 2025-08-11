use std::path::{Path, PathBuf};
use std::sync::Arc;
use chrono::Local;
use walkdir::WalkDir;
use uuid::Uuid;

use crate::database::Database;

pub struct ImportProcessor {
    db: Arc<Database>,
    imports_dir: PathBuf,
    notes_dir: PathBuf,
}

impl ImportProcessor {
    pub fn new(db: Arc<Database>, imports_dir: PathBuf, notes_dir: PathBuf) -> Self {
        Self {
            db,
            imports_dir,
            notes_dir,
        }
    }
    
    /// Scan the imports/pending folder for new audio files to process
    pub async fn scan_imports(&self) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
        let pending_dir = self.imports_dir.join("pending");
        let mut imports = Vec::new();
        
        if !pending_dir.exists() {
            std::fs::create_dir_all(&pending_dir)?;
        }
        
        for entry in WalkDir::new(&pending_dir).max_depth(2) {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    let ext_lower = ext.to_string_lossy().to_lowercase();
                    if matches!(ext_lower.as_str(), "wav" | "mp3" | "m4a" | "ogg" | "flac" | "webm") {
                        imports.push(path.to_path_buf());
                    }
                }
            }
        }
        
        log::info!("Found {} audio files in imports/pending", imports.len());
        Ok(imports)
    }
    
    /// Queue an imported file for processing
    pub async fn queue_import(&self, import_path: &Path) -> Result<String, Box<dyn std::error::Error>> {
        let task_id = Uuid::new_v4().to_string();
        let transcription_id = Uuid::new_v4().to_string();
        
        // Determine target directory based on current date
        let now = Local::now();
        let year = now.format("%Y").to_string();
        let date = now.format("%Y-%m-%d").to_string();
        let target_dir = self.notes_dir.join(&year).join(&date);
        
        // Generate target filename
        let timestamp = now.format("%H%M%S").to_string();
        let original_name = import_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("imported");
        let target_filename = format!("{}-imported-{}.wav", timestamp, original_name);
        let target_path = target_dir.join(&target_filename);
        
        // Create task payload
        let payload = serde_json::json!({
            "type": "ProcessImport",
            "import_path": import_path.to_string_lossy(),
            "target_path": target_path.to_string_lossy(),
            "original_name": import_path.file_name().unwrap_or_default().to_string_lossy(),
        });
        
        // Insert into database
        let pool = self.db.pool();
        
        // First, add to transcriptions table as pending
        sqlx::query(
            "INSERT INTO transcriptions (id, audio_path, status, source, created_at)
             VALUES (?, ?, 'pending', 'import', datetime('now'))"
        )
        .bind(&transcription_id)
        .bind(target_path.to_string_lossy().as_ref())
        .execute(pool)
        .await?;
        
        // Then add to background tasks
        sqlx::query(
            "INSERT INTO background_tasks (id, transcription_id, task_type, priority, status, payload, created_at, retry_count, max_retries)
             VALUES (?, ?, 'ProcessImport', 1, 'pending', ?, datetime('now'), 0, 2)"
        )
        .bind(&task_id)
        .bind(&transcription_id)
        .bind(payload.to_string())
        .execute(pool)
        .await?;
        
        log::info!("Queued import: {} -> {}", import_path.display(), target_path.display());
        Ok(task_id)
    }
    
    /// Process an imported file (move to target location and prepare for transcription)
    pub async fn process_import(&self, import_path: &Path, target_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        // Create target directory if it doesn't exist
        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        // Move the file from imports/pending to the target location
        std::fs::rename(import_path, target_path)?;
        
        // Move to processed folder (create a record of what was imported)
        let processed_dir = self.imports_dir.join("processed");
        std::fs::create_dir_all(&processed_dir)?;
        
        // Create a metadata file in processed folder
        let import_name = import_path.file_name().unwrap_or_default();
        let metadata_path = processed_dir.join(format!("{}.json", import_name.to_string_lossy()));
        let metadata = serde_json::json!({
            "original_path": import_path.to_string_lossy(),
            "target_path": target_path.to_string_lossy(),
            "processed_at": Local::now().to_rfc3339(),
        });
        std::fs::write(metadata_path, metadata.to_string())?;
        
        log::info!("Processed import: {} -> {}", import_path.display(), target_path.display());
        Ok(())
    }
    
    /// Scan and queue all pending imports
    pub async fn scan_and_queue_all(&self) -> Result<usize, Box<dyn std::error::Error>> {
        let imports = self.scan_imports().await?;
        let mut queued = 0;
        
        for import_path in imports {
            match self.queue_import(&import_path).await {
                Ok(_) => queued += 1,
                Err(e) => log::error!("Failed to queue import {}: {}", import_path.display(), e),
            }
        }
        
        Ok(queued)
    }
}