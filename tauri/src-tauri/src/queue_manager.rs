use std::sync::Arc;
use tokio::sync::{RwLock, Mutex};
use tokio::task::JoinHandle;
use std::sync::atomic::{AtomicBool, Ordering};
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Local};
use std::path::PathBuf;
use voicetextrs::core::transcription::Transcriber;
use sqlx::Row;
use tauri::{Manager, Emitter};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskType {
    TranscribeOrphan {
        audio_path: String,
        output_path: String,
    },
    TranscribeImported {
        audio_path: String,
        original_name: String,
    },
    FileSystemSync {
        full_scan: bool,
    },
    ProcessImport {
        import_path: String,
        target_dir: String,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    Low = 0,
    Normal = 1,
    High = 2,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Pending,
    Processing { progress: f32 },
    Completed,
    Failed { error: String, can_retry: bool },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundTask {
    pub id: String,
    pub transcription_id: String,
    pub task_type: TaskType,
    pub priority: TaskPriority,
    pub status: TaskStatus,
    pub created_at: DateTime<Local>,
    pub started_at: Option<DateTime<Local>>,
    pub completed_at: Option<DateTime<Local>>,
    pub retry_count: u32,
    pub max_retries: u32,
    pub error_message: Option<String>,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct QueueStatus {
    pub is_paused: bool,
    pub is_processing: bool,
    pub active_task: Option<BackgroundTask>,
    pub pending_count: usize,
    pub processing_count: usize,
    pub completed_count: usize,
    pub failed_count: usize,
    pub total_count: usize,
}

pub struct QueueManager {
    is_paused: Arc<AtomicBool>,
    is_running: Arc<AtomicBool>,
    active_task: Arc<RwLock<Option<BackgroundTask>>>,
    transcriber: Arc<Transcriber>,
    worker_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    sync_scheduler_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    app_handle: Option<tauri::AppHandle>,
    database: Arc<Mutex<Option<Arc<crate::database::Database>>>>,
}

impl QueueManager {
    pub fn new(transcriber: Arc<Transcriber>) -> Self {
        Self {
            is_paused: Arc::new(AtomicBool::new(false)),
            is_running: Arc::new(AtomicBool::new(false)),
            active_task: Arc::new(RwLock::new(None)),
            transcriber,
            worker_handle: Arc::new(Mutex::new(None)),
            sync_scheduler_handle: Arc::new(Mutex::new(None)),
            app_handle: None,
            database: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_app_handle(&mut self, handle: tauri::AppHandle) {
        self.app_handle = Some(handle);
    }

    pub async fn start_worker(&self, database: Arc<crate::database::Database>) {
        if self.is_running.load(Ordering::Relaxed) {
            log::warn!("Queue worker is already running");
            return;
        }

        // Store the database reference
        *self.database.lock().await = Some(database.clone());
        
        self.is_running.store(true, Ordering::Relaxed);
        
        let is_paused = self.is_paused.clone();
        let is_running = self.is_running.clone();
        let active_task = self.active_task.clone();
        let transcriber = self.transcriber.clone();
        let app_handle = self.app_handle.clone();

        let handle = tokio::spawn(async move {
            log::info!("Background queue worker started");
            
            while is_running.load(Ordering::Relaxed) {
                // Check if paused
                if is_paused.load(Ordering::Relaxed) {
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    continue;
                }

                // Check for recording state - don't process if recording is active
                if let Some(ref handle) = app_handle {
                    use crate::commands::RecordingState;
                    if let Some(state) = handle.try_state::<Arc<tokio::sync::Mutex<RecordingState>>>() {
                        let recording_state = state.lock().await;
                        if !matches!(*recording_state, RecordingState::Idle) {
                            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                            continue;
                        }
                    }
                }

                // Try to get next task from database
                match Self::claim_next_task(&database).await {
                    Ok(Some(mut task)) => {
                        log::info!("Processing task: {}", task.id);
                        
                        // Update active task
                        *active_task.write().await = Some(task.clone());
                        
                        // Emit event to UI
                        if let Some(ref handle) = app_handle {
                            let _ = handle.emit::<QueueTaskUpdate>("background-task-update", QueueTaskUpdate {
                                task_id: task.id.clone(),
                                status: task.status.clone(),
                            });
                        }

                        // Process the task
                        let result = Self::process_task(&task, &transcriber, &database).await;
                        
                        // Update task based on result
                        match result {
                            Ok(transcription_text) => {
                                task.status = TaskStatus::Completed;
                                task.completed_at = Some(Local::now());
                                
                                // Update database
                                if let Err(e) = Self::complete_task(&database, &task.id, &transcription_text).await {
                                    log::error!("Failed to mark task as completed: {}", e);
                                }
                            }
                            Err(e) => {
                                log::error!("Task {} failed: {}", task.id, e);
                                task.error_message = Some(e.to_string());
                                
                                if task.retry_count < task.max_retries {
                                    task.status = TaskStatus::Pending;
                                    task.retry_count += 1;
                                    
                                    if let Err(e) = Self::retry_task(&database, &task.id).await {
                                        log::error!("Failed to retry task: {}", e);
                                    }
                                } else {
                                    task.status = TaskStatus::Failed { 
                                        error: e.to_string(), 
                                        can_retry: false 
                                    };
                                    
                                    if let Err(e) = Self::fail_task(&database, &task.id, &e.to_string()).await {
                                        log::error!("Failed to mark task as failed: {}", e);
                                    }
                                }
                            }
                        }
                        
                        // Clear active task
                        *active_task.write().await = None;
                        
                        // Emit completion event
                        if let Some(ref handle) = app_handle {
                            let _ = handle.emit::<QueueTaskUpdate>("background-task-update", QueueTaskUpdate {
                                task_id: task.id.clone(),
                                status: task.status.clone(),
                            });
                        }
                    }
                    Ok(None) => {
                        // No tasks available, wait before checking again
                        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    }
                    Err(e) => {
                        log::error!("Error claiming task: {}", e);
                        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    }
                }
            }
            
            log::info!("Background queue worker stopped");
        });

        *self.worker_handle.lock().await = Some(handle);
    }

    pub async fn stop_worker(&self) {
        self.is_running.store(false, Ordering::Relaxed);
        
        if let Some(handle) = self.worker_handle.lock().await.take() {
            let _ = handle.await;
        }
        
        // Also stop the sync scheduler
        if let Some(handle) = self.sync_scheduler_handle.lock().await.take() {
            let _ = handle.await;
        }
    }

    pub async fn start_sync_scheduler(&self, database: Arc<crate::database::Database>) {
        let db = database.clone();
        let is_running = self.is_running.clone();
        
        let handle = tokio::spawn(async move {
            log::info!("Starting filesystem sync scheduler");
            
            // Run initial sync after 30 seconds
            tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
            
            while is_running.load(Ordering::Relaxed) {
                // Schedule a filesystem sync task
                if let Err(e) = Self::enqueue_sync_task(&db, false).await {
                    log::error!("Failed to enqueue sync task: {}", e);
                }
                
                // Wait 5 minutes before next sync
                tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;
            }
            
            log::info!("Sync scheduler stopped");
        });
        
        *self.sync_scheduler_handle.lock().await = Some(handle);
    }

    async fn enqueue_sync_task(database: &crate::database::Database, full_scan: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let pool = database.pool();
        
        // Check if a sync task is already pending or processing
        let existing = sqlx::query(
            "SELECT COUNT(*) as count FROM background_tasks 
             WHERE payload LIKE '%FileSystemSync%' 
             AND status IN ('pending', 'processing')"
        )
        .fetch_one(pool)
        .await?;
        
        let count: i64 = existing.get("count");
        if count > 0 {
            log::debug!("Sync task already in queue, skipping");
            return Ok(());
        }
        
        // Create a new sync task
        let task_id = uuid::Uuid::new_v4().to_string();
        let payload = serde_json::json!({
            "type": "FileSystemSync",
            "full_scan": full_scan
        });
        
        sqlx::query(
            "INSERT INTO background_tasks (id, transcription_id, task_type, priority, status, payload, created_at, retry_count, max_retries)
             VALUES (?, ?, 'FileSystemSync', 0, 'pending', ?, datetime('now'), 0, 1)"
        )
        .bind(&task_id)
        .bind(&task_id) // Use task_id as transcription_id for system tasks
        .bind(payload.to_string())
        .execute(pool)
        .await?;
        
        log::info!("Enqueued filesystem sync task (full_scan: {})", full_scan);
        Ok(())
    }

    pub fn pause(&self) {
        self.is_paused.store(true, Ordering::Relaxed);
        log::info!("Queue paused");
    }

    pub fn resume(&self) {
        self.is_paused.store(false, Ordering::Relaxed);
        log::info!("Queue resumed");
    }

    pub fn is_paused(&self) -> bool {
        self.is_paused.load(Ordering::Relaxed)
    }

    async fn claim_next_task(database: &crate::database::Database) -> Result<Option<BackgroundTask>, Box<dyn std::error::Error + Send + Sync>> {
        let pool = database.pool();
        
        // Simple query without macros
        let query = r#"
            UPDATE background_tasks
            SET status = 'processing', started_at = datetime('now')
            WHERE id = (
                SELECT id FROM background_tasks
                WHERE status = 'pending'
                ORDER BY priority DESC, created_at
                LIMIT 1
            )
            RETURNING *
        "#;
        
        let row = sqlx::query(query)
            .fetch_optional(pool)
            .await?;

        if let Some(row) = row {
            // Parse task type from string and payload
            let task_type_str: String = row.get("task_type");
            let payload: serde_json::Value = serde_json::from_str(row.get("payload")).unwrap_or(serde_json::Value::Null);
            
            let task_type = match task_type_str.as_str() {
                "TranscribeOrphan" => TaskType::TranscribeOrphan {
                    audio_path: payload["audio_path"].as_str().unwrap_or("").to_string(),
                    output_path: payload["output_path"].as_str().unwrap_or("").to_string(),
                },
                "TranscribeImported" => TaskType::TranscribeImported {
                    audio_path: payload["audio_path"].as_str().unwrap_or("").to_string(),
                    original_name: payload["original_name"].as_str().unwrap_or("").to_string(),
                },
                "FileSystemSync" => TaskType::FileSystemSync {
                    full_scan: payload["full_scan"].as_bool().unwrap_or(false),
                },
                "ProcessImport" => TaskType::ProcessImport {
                    import_path: payload["import_path"].as_str().unwrap_or("").to_string(),
                    target_dir: payload["target_path"].as_str().unwrap_or("").to_string(),
                },
                _ => TaskType::TranscribeOrphan {
                    audio_path: String::new(),
                    output_path: String::new(),
                },
            };
            
            let task = BackgroundTask {
                id: row.get("id"),
                transcription_id: row.get("transcription_id"),
                task_type,
                priority: match row.get::<i32, _>("priority") {
                    0 => TaskPriority::Low,
                    1 => TaskPriority::Normal,
                    2 => TaskPriority::High,
                    _ => TaskPriority::Normal,
                },
                status: TaskStatus::Processing { progress: 0.0 },
                created_at: Local::now(), // Simplified
                started_at: Some(Local::now()),
                completed_at: None,
                retry_count: row.get::<i32, _>("retry_count") as u32,
                max_retries: row.get::<i32, _>("max_retries") as u32,
                error_message: row.get("error_message"),
                payload,
            };
            
            Ok(Some(task))
        } else {
            Ok(None)
        }
    }

    async fn process_task(task: &BackgroundTask, transcriber: &Transcriber, database: &Arc<crate::database::Database>) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        match &task.task_type {
            TaskType::TranscribeOrphan { audio_path, output_path } |
            TaskType::TranscribeImported { audio_path, original_name: output_path } => {
                let audio_path = PathBuf::from(audio_path);
                let output_path = PathBuf::from(output_path);
                
                if !audio_path.exists() {
                    return Err(format!("Audio file not found: {:?}", audio_path).into());
                }

                // Transcribe the audio file
                let result = transcriber.transcribe(&audio_path).await?;
                
                // Write the transcription to file
                std::fs::write(&output_path, &result.text)?;
                
                Ok(result.text)
            }
            TaskType::FileSystemSync { full_scan: _ } => {
                // Perform filesystem sync using the sync module
                use crate::sync::FileSystemSync;
                
                let notes_dir = std::env::current_dir()
                    .map(|p| p.parent().unwrap_or(&p).join("notes"))
                    .unwrap_or_else(|_| PathBuf::from("notes"));
                
                let sync = FileSystemSync::new(database.clone(), notes_dir);
                let report = sync.sync_filesystem().await?;
                
                log::info!("FileSystemSync completed: {} new, {} updated, {} missing", 
                    report.new_transcriptions, report.updated_transcriptions, report.missing_files);
                
                Ok(format!("Sync complete: {} files processed", report.total_files_found))
            }
            TaskType::ProcessImport { import_path, target_dir } => {
                // Process import file
                let import_path = PathBuf::from(import_path);
                let target_path = PathBuf::from(target_dir);
                
                if !import_path.exists() {
                    return Err(format!("Import file not found: {:?}", import_path).into());
                }
                
                // Create target directory if needed
                if let Some(parent) = target_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                
                // Move the file to target location
                std::fs::rename(&import_path, &target_path)?;
                
                // Queue for transcription
                let transcription_id = uuid::Uuid::new_v4().to_string();
                let output_path = target_path.with_extension("txt");
                
                // Add transcription task
                let payload = serde_json::json!({
                    "type": "TranscribeImported",
                    "audio_path": target_path.to_string_lossy(),
                    "output_path": output_path.to_string_lossy(),
                });
                
                sqlx::query(
                    "INSERT INTO background_tasks (id, transcription_id, task_type, priority, status, payload, created_at, retry_count, max_retries)
                     VALUES (?, ?, 'TranscribeImported', 1, 'pending', ?, datetime('now'), 0, 2)"
                )
                .bind(uuid::Uuid::new_v4().to_string())
                .bind(&transcription_id)
                .bind(payload.to_string())
                .execute(database.pool())
                .await?;
                
                log::info!("Import processed and queued for transcription: {}", target_path.display());
                Ok(format!("Import processed: {}", target_path.display()))
            }
        }
    }

    async fn complete_task(database: &crate::database::Database, task_id: &str, transcription_text: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let pool = database.pool();
        
        let mut tx = pool.begin().await?;
        
        // Update the task
        sqlx::query("UPDATE background_tasks SET status = 'completed', completed_at = datetime('now') WHERE id = ?")
            .bind(task_id)
            .execute(&mut *tx)
            .await?;
        
        // Get the transcription_id
        let row = sqlx::query("SELECT transcription_id FROM background_tasks WHERE id = ?")
            .bind(task_id)
            .fetch_one(&mut *tx)
            .await?;
        
        let transcription_id: String = row.get("transcription_id");
        
        // Update the transcription
        sqlx::query("UPDATE transcriptions SET status = 'complete', transcription_text = ?, transcribed_at = datetime('now') WHERE id = ?")
            .bind(transcription_text)
            .bind(&transcription_id)
            .execute(&mut *tx)
            .await?;
        
        tx.commit().await?;
        
        Ok(())
    }

    async fn retry_task(database: &crate::database::Database, task_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let pool = database.pool();
        
        sqlx::query("UPDATE background_tasks SET status = 'pending', retry_count = retry_count + 1 WHERE id = ?")
            .bind(task_id)
            .execute(pool)
            .await?;
        
        Ok(())
    }

    async fn fail_task(database: &crate::database::Database, task_id: &str, error: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let pool = database.pool();
        
        sqlx::query("UPDATE background_tasks SET status = 'failed', error_message = ? WHERE id = ?")
            .bind(error)
            .bind(task_id)
            .execute(pool)
            .await?;
        
        Ok(())
    }

    pub async fn get_queue_status(&self, database: &crate::database::Database) -> Result<QueueStatus, Box<dyn std::error::Error + Send + Sync>> {
        let pool = database.pool();
        
        let query = r#"
            SELECT 
                COUNT(CASE WHEN status = 'pending' THEN 1 END) as pending,
                COUNT(CASE WHEN status = 'processing' THEN 1 END) as processing,
                COUNT(CASE WHEN status = 'completed' THEN 1 END) as completed,
                COUNT(CASE WHEN status = 'failed' THEN 1 END) as failed,
                COUNT(*) as total
            FROM background_tasks
        "#;
        
        let row = sqlx::query(query)
            .fetch_one(pool)
            .await?;
        
        let active_task = self.active_task.read().await.clone();
        
        Ok(QueueStatus {
            is_paused: self.is_paused.load(Ordering::Relaxed),
            is_processing: active_task.is_some(),
            active_task,
            pending_count: row.get::<i32, _>("pending") as usize,
            processing_count: row.get::<i32, _>("processing") as usize,
            completed_count: row.get::<i32, _>("completed") as usize,
            failed_count: row.get::<i32, _>("failed") as usize,
            total_count: row.get::<i32, _>("total") as usize,
        })
    }

    pub async fn enqueue_task(&self, database: &crate::database::Database, task: BackgroundTask) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let pool = database.pool();
        
        let task_type_json = serde_json::to_string(&task.task_type)?;
        let payload_json = serde_json::to_string(&task.payload)?;
        
        sqlx::query(r#"
            INSERT INTO background_tasks (
                id, transcription_id, task_type, priority, status,
                created_at, retry_count, max_retries, payload
            ) VALUES (?, ?, ?, ?, ?, datetime('now'), ?, ?, ?)
        "#)
        .bind(&task.id)
        .bind(&task.transcription_id)
        .bind(&task_type_json)
        .bind(task.priority as i32)
        .bind("pending")
        .bind(task.retry_count as i32)
        .bind(task.max_retries as i32)
        .bind(&payload_json)
        .execute(pool)
        .await?;
        
        Ok(())
    }

    pub async fn retry_failed_task(&self, database: &crate::database::Database, task_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let pool = database.pool();
        
        sqlx::query("UPDATE background_tasks SET status = 'pending', retry_count = 0, error_message = NULL WHERE id = ? AND status = 'failed'")
            .bind(task_id)
            .execute(pool)
            .await?;
        
        Ok(())
    }

    pub async fn clear_completed_tasks(&self, database: &crate::database::Database) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let pool = database.pool();
        
        let result = sqlx::query("DELETE FROM background_tasks WHERE status = 'completed'")
            .execute(pool)
            .await?;
        
        Ok(result.rows_affected() as usize)
    }

    pub async fn get_tasks(&self, database: &crate::database::Database, limit: i32, offset: i32) -> Result<Vec<BackgroundTask>, Box<dyn std::error::Error + Send + Sync>> {
        let pool = database.pool();
        
        let query = r#"
            SELECT * FROM background_tasks
            ORDER BY 
                CASE status 
                    WHEN 'processing' THEN 0
                    WHEN 'pending' THEN 1
                    WHEN 'failed' THEN 2
                    WHEN 'completed' THEN 3
                END,
                priority DESC,
                created_at DESC
            LIMIT ? OFFSET ?
        "#;
        
        let rows = sqlx::query(query)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;

        let mut tasks = Vec::new();
        for row in rows {
            // Parse task type from string and payload
            let task_type_str: String = row.get("task_type");
            let payload: serde_json::Value = serde_json::from_str(row.get("payload")).unwrap_or(serde_json::Value::Null);
            
            let task_type = match task_type_str.as_str() {
                "TranscribeOrphan" => TaskType::TranscribeOrphan {
                    audio_path: payload["audio_path"].as_str().unwrap_or("").to_string(),
                    output_path: payload["output_path"].as_str().unwrap_or("").to_string(),
                },
                "TranscribeImported" => TaskType::TranscribeImported {
                    audio_path: payload["audio_path"].as_str().unwrap_or("").to_string(),
                    original_name: payload["original_name"].as_str().unwrap_or("").to_string(),
                },
                "FileSystemSync" => TaskType::FileSystemSync {
                    full_scan: payload["full_scan"].as_bool().unwrap_or(false),
                },
                "ProcessImport" => TaskType::ProcessImport {
                    import_path: payload["import_path"].as_str().unwrap_or("").to_string(),
                    target_dir: payload["target_path"].as_str().unwrap_or("").to_string(),
                },
                _ => TaskType::TranscribeOrphan {
                    audio_path: String::new(),
                    output_path: String::new(),
                },
            };
            
            let task = BackgroundTask {
                id: row.get("id"),
                transcription_id: row.get("transcription_id"),
                task_type,
                priority: match row.get::<i32, _>("priority") {
                    0 => TaskPriority::Low,
                    1 => TaskPriority::Normal,
                    2 => TaskPriority::High,
                    _ => TaskPriority::Normal,
                },
                status: match row.get::<&str, _>("status") {
                    "pending" => TaskStatus::Pending,
                    "processing" => TaskStatus::Processing { progress: 0.0 },
                    "completed" => TaskStatus::Completed,
                    "failed" => TaskStatus::Failed { 
                        error: row.get::<Option<String>, _>("error_message").unwrap_or_default(), 
                        can_retry: row.get::<i32, _>("retry_count") < row.get::<i32, _>("max_retries")
                    },
                    _ => TaskStatus::Pending,
                },
                created_at: Local::now(), // Simplified
                started_at: None,
                completed_at: None,
                retry_count: row.get::<i32, _>("retry_count") as u32,
                max_retries: row.get::<i32, _>("max_retries") as u32,
                error_message: row.get("error_message"),
                payload,
            };
            
            tasks.push(task);
        }

        Ok(tasks)
    }
}

#[derive(Debug, Clone, Serialize)]
struct QueueTaskUpdate {
    task_id: String,
    status: TaskStatus,
}