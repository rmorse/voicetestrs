use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use voicetextrs::core::database::{
    Transcription, TranscriptionStatus,
    BackgroundTask, QueueStatus
};
use voicetextrs::core::sync::{FileSystemSync, SyncReport};

#[derive(Debug, Serialize, Deserialize)]
pub struct TranscriptionFilter {
    pub status: Option<String>,
    pub source: Option<String>,
    pub search: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

#[tauri::command]
pub async fn db_get_transcriptions(
    _app: AppHandle,
    _filter: TranscriptionFilter,
) -> Result<Vec<Transcription>, String> {
    // SQL queries will be handled by the JavaScript side using the SQL plugin directly
    // This command is kept for compatibility
    Ok(vec![])
}

#[tauri::command]
pub async fn db_search_transcriptions(
    _app: AppHandle,
    _query: String,
) -> Result<Vec<Transcription>, String> {
    // Search will be handled by the JavaScript side using the SQL plugin directly
    // This command is kept for compatibility
    Ok(vec![])
}

#[tauri::command]
pub async fn db_insert_transcription(
    _app: AppHandle,
    _transcription: Transcription,
) -> Result<(), String> {
    // Insertion will be handled by the JavaScript side using the SQL plugin directly
    // This command is kept for compatibility
    Ok(())
}

#[tauri::command]
pub async fn db_update_transcription_status(
    _app: AppHandle,
    _id: String,
    _status: TranscriptionStatus,
    _error: Option<String>,
) -> Result<(), String> {
    // For now, just return OK - we'll implement SQL queries next
    Ok(())
}

#[tauri::command]
pub async fn db_get_queue_status(
    _app: AppHandle,
) -> Result<QueueStatus, String> {
    // For now, return default status - we'll implement SQL queries next
    Ok(QueueStatus {
        is_paused: false,
        pending_count: 0,
        processing_count: 0,
        completed_count: 0,
        failed_count: 0,
        active_task: None,
    })
}

#[tauri::command]
pub async fn db_enqueue_task(
    _app: AppHandle,
    _task: BackgroundTask,
) -> Result<(), String> {
    // For now, just return OK - we'll implement SQL queries next
    Ok(())
}

#[tauri::command]
pub async fn db_retry_task(
    _app: AppHandle,
    _task_id: String,
) -> Result<(), String> {
    // For now, just return OK - we'll implement SQL queries next
    Ok(())
}

#[tauri::command]
pub async fn db_clear_completed_tasks(
    _app: AppHandle,
) -> Result<(), String> {
    // Task management will be handled by the JavaScript side using the SQL plugin directly
    Ok(())
}

#[tauri::command]
pub async fn sync_filesystem(
    app: AppHandle,
) -> Result<SyncReport, String> {
    sync_filesystem_internal(app, false).await
}

#[tauri::command]
pub async fn sync_filesystem_force(
    app: AppHandle,
) -> Result<SyncReport, String> {
    sync_filesystem_internal(app, true).await
}

async fn sync_filesystem_internal(
    app: AppHandle,
    force_all: bool,
) -> Result<SyncReport, String> {
    // The notes directory is at the project root
    // In development, it's at D:\projects\claude\voicetextrs\notes
    // We need to go up from the tauri src directory
    let notes_dir = if cfg!(debug_assertions) {
        // In development, use the absolute path to the notes directory
        std::path::PathBuf::from(r"D:\projects\claude\voicetextrs\notes")
    } else {
        // In production, notes are relative to the app
        app.path().app_data_dir()
            .map_err(|e| format!("Failed to get app dir: {}", e))?
            .join("notes")
    };
    
    println!("Syncing filesystem from directory: {:?}", notes_dir);
    
    let sync = FileSystemSync::new(notes_dir.clone());
    let report = sync.sync_filesystem().await
        .map_err(|e| format!("Sync failed: {}", e))?;
    
    println!("Sync report: {:?}", report);
    
    // Process each audio file and insert into database via JavaScript
    let audio_files = sync.scan_audio_files()
        .map_err(|e| format!("Failed to scan files: {}", e))?;
    
    println!("Found {} audio files", audio_files.len());
    
    // Create a smart sync event that includes sync strategy
    for audio_path in audio_files {
        if let Ok(transcription) = sync.get_transcription_for_insert(&audio_path) {
            // Only emit sync events for files that need syncing:
            // 1. Orphaned files (no transcription text)
            // 2. Failed or pending files
            // 3. ALL files if force_all is true (for full resync)
            // Skip completed transcriptions unless forced
            use voicetextrs::core::database::TranscriptionStatus;
            
            let needs_sync = if force_all {
                true  // Force sync everything for full resync
            } else {
                match &transcription.status {
                    TranscriptionStatus::Orphaned => true,  // Always sync orphaned files
                    TranscriptionStatus::Failed => true,    // Always sync failed files
                    TranscriptionStatus::Pending => true,   // Always sync pending files
                    TranscriptionStatus::Processing => true, // Always sync processing files
                    TranscriptionStatus::Complete => false,  // SKIP completed - already in DB!
                }
            };
            
            if needs_sync {
                // Emit event with transcription data for JavaScript to insert
                app.emit("sync-transcription", &transcription)
                    .map_err(|e| format!("Failed to emit event: {}", e))?;
            }
        }
    }
    
    // Emit completion event
    app.emit("sync-complete", &report)
        .map_err(|e| format!("Failed to emit completion: {}", e))?;
    
    Ok(report)
}