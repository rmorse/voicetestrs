use tauri::State;
use std::sync::Arc;
use crate::database::{Database, models::*};

#[tauri::command]
pub async fn get_transcriptions(
    db: State<'_, Arc<Database>>,
    limit: Option<i32>,
    offset: Option<i32>,
    status: Option<String>,
) -> Result<Vec<Transcription>, String> {
    db.list_transcriptions(
        limit.unwrap_or(50),
        offset.unwrap_or(0),
        status
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_transcription(
    db: State<'_, Arc<Database>>,
    id: String,
) -> Result<Option<Transcription>, String> {
    db.get_transcription(&id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_transcription(
    db: State<'_, Arc<Database>>,
    id: String,
    updates: TranscriptionUpdate,
) -> Result<(), String> {
    db.update_transcription(&id, updates)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_transcription(
    db: State<'_, Arc<Database>>,
    id: String,
) -> Result<(), String> {
    db.delete_transcription(&id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search_transcriptions(
    db: State<'_, Arc<Database>>,
    query: String,
) -> Result<Vec<Transcription>, String> {
    db.search_transcriptions(&query)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_database_stats(
    db: State<'_, Arc<Database>>,
) -> Result<DatabaseStats, String> {
    db.get_stats()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_database(
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    db.clear_all_transcriptions()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cleanup_duplicate_transcriptions(
    db: State<'_, Arc<Database>>,
) -> Result<usize, String> {
    db.cleanup_duplicates()
        .await
        .map_err(|e| e.to_string())
}