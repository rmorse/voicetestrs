use tauri::State;
use std::sync::Arc;
use crate::queue_manager::{QueueManager, QueueStatus, BackgroundTask, TaskType, TaskPriority, TaskStatus};
use crate::database::Database;
use serde_json::json;
use chrono::Local;
use uuid::Uuid;

#[tauri::command]
pub async fn get_queue_status(
    queue: State<'_, Arc<QueueManager>>,
    database: State<'_, Database>,
) -> Result<QueueStatus, String> {
    queue.get_queue_status(&database)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_queue_tasks(
    queue: State<'_, Arc<QueueManager>>,
    database: State<'_, Database>,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<Vec<BackgroundTask>, String> {
    queue.get_tasks(&database, limit.unwrap_or(50), offset.unwrap_or(0))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn enqueue_orphan_task(
    queue: State<'_, Arc<QueueManager>>,
    database: State<'_, Database>,
    transcription_id: String,
    audio_path: String,
) -> Result<(), String> {
    let output_path = audio_path.replace(".wav", ".txt")
        .replace(".mp3", ".txt")
        .replace(".m4a", ".txt");
    
    let task = BackgroundTask {
        id: Uuid::new_v4().to_string(),
        transcription_id,
        task_type: TaskType::TranscribeOrphan {
            audio_path: audio_path.clone(),
            output_path,
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
            "audio_path": audio_path,
        }),
    };
    
    queue.enqueue_task(&database, task)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pause_queue(
    queue: State<'_, Arc<QueueManager>>,
) -> Result<(), String> {
    queue.pause();
    Ok(())
}

#[tauri::command]
pub async fn resume_queue(
    queue: State<'_, Arc<QueueManager>>,
) -> Result<(), String> {
    queue.resume();
    Ok(())
}

#[tauri::command]
pub async fn retry_failed_task(
    queue: State<'_, Arc<QueueManager>>,
    database: State<'_, Database>,
    task_id: String,
) -> Result<(), String> {
    queue.retry_failed_task(&database, &task_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_completed_tasks(
    queue: State<'_, Arc<QueueManager>>,
    database: State<'_, Database>,
) -> Result<usize, String> {
    queue.clear_completed_tasks(&database)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn is_queue_paused(
    queue: State<'_, Arc<QueueManager>>,
) -> Result<bool, String> {
    Ok(queue.is_paused())
}