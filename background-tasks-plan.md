# Background Task Queue System - Implementation Plan

## Overview
Add a database-backed background task queue for processing orphaned audio files (audio without transcriptions) while maintaining the priority of user-initiated recordings. The queue state is stored in SQLite for persistence and atomic operations.

## Core Principles
1. **User recordings always have priority** - Never interfere with active recording/processing
2. **Single queue worker** - Process tasks sequentially to avoid resource contention
3. **Transparent to user** - Background tasks don't clutter main transcription list
4. **Resilient** - Survive app restarts, handle failures gracefully

## Architecture

### 1. Database-Backed Task Queue

Tasks are stored in the `background_tasks` table with atomic operations for claiming and updating:

```sql
-- Queue operations are atomic
UPDATE background_tasks
SET status = 'processing', started_at = CURRENT_TIMESTAMP
WHERE id = (
    SELECT id FROM background_tasks
    WHERE status = 'pending'
    ORDER BY priority DESC, created_at
    LIMIT 1
)
RETURNING *;
```

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskType {
    TranscribeOrphan {
        audio_path: PathBuf,
        output_path: PathBuf,
    },
    TranscribeImported {
        audio_path: PathBuf,
        original_name: String,          // For UI display
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    Low = 0,    // Orphaned files found on scan
    Normal = 1, // Future: User-initiated reprocess
    High = 2,   // Future: Priority imports
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Pending,
    Processing { 
        progress: f32  // 0.0 to 1.0
    },
    Completed,
    Failed { 
        can_retry: bool 
    },
}
```

### 2. Queue Manager with Database

```rust
// src/core/queue_manager.rs

pub struct QueueManager {
    db: Arc<DatabaseManager>,          // All state in database
    is_paused: Arc<AtomicBool>,        // Runtime flag only
    app_state: Arc<Mutex<RecordingState>>,
    transcriber: Arc<Transcriber>,
    worker_handle: Option<JoinHandle<()>>,
}

impl QueueManager {
    // All operations via database
    pub async fn add_task(&self, task: BackgroundTask) {
        sqlx::query!(
            "INSERT INTO background_tasks (transcription_id, task_type, priority, payload)
             VALUES (?, ?, ?, ?)",
            task.transcription_id, task.task_type, task.priority, task.payload
        ).execute(&self.db.pool).await
    }
    
    pub async fn get_queue_status(&self) -> QueueStatus {
        // Single SQL query for all counts
        sqlx::query!(
            "SELECT 
                COUNT(CASE WHEN status = 'pending' THEN 1 END) as pending,
                COUNT(CASE WHEN status = 'processing' THEN 1 END) as processing,
                COUNT(CASE WHEN status = 'failed' THEN 1 END) as failed
             FROM background_tasks"
        ).fetch_one(&self.db.pool).await
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct QueueStatus {
    pub is_paused: bool,
    pub active_task: Option<BackgroundTask>,
    pub pending_count: usize,
    pub completed_count: usize,
    pub failed_count: usize,
    pub tasks: Vec<BackgroundTask>,
}
```

### 3. Processing Logic with Database

```rust
async fn should_process(&self) -> bool {
    if self.is_paused.load(Ordering::Relaxed) {
        return false;
    }
    
    let app_state = self.app_state.lock().await;
    match *app_state {
        RecordingState::Idle => true,
        RecordingState::Recording | RecordingState::Processing => {
            // Check if task already processing via DB
            let active = sqlx::query!(
                "SELECT COUNT(*) as count FROM background_tasks 
                 WHERE status = 'processing'"
            ).fetch_one(&self.db.pool).await;
            
            active.count > 0  // Let current finish, don't start new
        }
    }
}

async fn process_next_task(&self) -> Result<()> {
    // Atomically claim next task from database
    let task = sqlx::query_as!(
        BackgroundTask,
        r#"
        UPDATE background_tasks
        SET status = 'processing', started_at = CURRENT_TIMESTAMP
        WHERE id = (
            SELECT id FROM background_tasks
            WHERE status = 'pending'
            ORDER BY priority DESC, created_at
            LIMIT 1
        )
        RETURNING *
        "#
    ).fetch_optional(&self.db.pool).await?;
    
    if let Some(mut task) = task {
        // Update status
        task.status = TaskStatus::Processing { progress: 0.0 };
        task.started_at = Some(Local::now());
        *self.active_task.write().await = Some(task.clone());
        
        // Emit event to UI
        emit_task_update(&task);
        
        // Process based on type
        let result = match &task.task_type {
            TaskType::TranscribeOrphan { audio_path, .. } => {
                self.transcriber.transcribe(audio_path).await
            },
            TaskType::TranscribeImported { audio_path, .. } => {
                self.transcriber.transcribe(audio_path).await
            },
        };
        
        // Handle result with database updates
        match result {
            Ok(transcription) => {
                // Transaction: update task and transcription
                let mut tx = self.db.pool.begin().await?;
                
                sqlx::query!(
                    "UPDATE background_tasks SET status = 'completed',
                     completed_at = CURRENT_TIMESTAMP WHERE id = ?",
                    task.id
                ).execute(&mut tx).await?;
                
                sqlx::query!(
                    "UPDATE transcriptions SET status = 'complete',
                     transcription_text = ?, transcribed_at = CURRENT_TIMESTAMP
                     WHERE id = ?",
                    transcription.text, task.transcription_id
                ).execute(&mut tx).await?;
                
                tx.commit().await?;
            },
            Err(e) => {
                if task.retry_count < task.max_retries {
                    // Reset for retry
                    sqlx::query!(
                        "UPDATE background_tasks SET status = 'pending',
                         retry_count = retry_count + 1 WHERE id = ?",
                        task.id
                    ).execute(&self.db.pool).await?;
                } else {
                    // Mark as failed
                    sqlx::query!(
                        "UPDATE background_tasks SET status = 'failed',
                         error_message = ? WHERE id = ?",
                        e.to_string(), task.id
                    ).execute(&self.db.pool).await?;
                }
            }
        }
    }
    
    Ok(())
}
```

### 4. File Discovery with Database

```rust
// src/core/orphan_scanner.rs

pub struct OrphanScanner {
    notes_dir: PathBuf,
    db: Arc<DatabaseManager>,
}

impl OrphanScanner {
    pub async fn scan_and_queue_orphans(&self) -> Result<usize> {
        let mut count = 0;
        
        // Get existing records from database
        let existing_ids = sqlx::query!(
            "SELECT id FROM transcriptions"
        ).fetch_all(&self.db.pool).await?;
        
        // Walk the notes directory
        for entry in WalkDir::new(&self.notes_dir) {
            if let Some(ext) = entry.path().extension() {
                if ext == "wav" || ext == "mp3" || ext == "m4a" {
                    let id = extract_id_from_path(entry.path());
                    
                    // Check if already in database
                    if !existing_ids.contains(&id) {
                        // Add to transcriptions table as orphaned
                        sqlx::query!(
                            "INSERT INTO transcriptions (id, audio_path, status, source)
                             VALUES (?, ?, 'orphaned', 'orphan')",
                            id, entry.path().to_str()
                        ).execute(&self.db.pool).await?;
                        
                        // Add to background queue
                        sqlx::query!(
                            "INSERT INTO background_tasks 
                             (transcription_id, task_type, priority, payload)
                             VALUES (?, 'transcribe_orphan', 0, ?)",
                            id, json!({"audio_path": entry.path()})
                        ).execute(&self.db.pool).await?;
                        
                        count += 1;
                    }
                }
            }
        }
        
        Ok(count)
    }
    
    pub fn is_imported_file(path: &Path) -> bool {
        // Imported files have different naming pattern
        // Our recordings: HHMMSS-voice-note.wav
        // Imported: any other pattern
        let filename = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        
        !filename.ends_with("-voice-note")
    }
}
```

### 5. UI Components

#### A. Background Tasks Tab

```jsx
// New component: BackgroundTasksTab.jsx

function BackgroundTasksTab() {
  const [queueStatus, setQueueStatus] = useState(null)
  const [isPaused, setIsPaused] = useState(false)
  
  useEffect(() => {
    // Load initial status
    invoke('get_queue_status').then(setQueueStatus)
    
    // Listen for updates
    const unlisten = listen('background-task-update', (event) => {
      setQueueStatus(event.payload)
    })
    
    return () => unlisten()
  }, [])
  
  const togglePause = async () => {
    if (isPaused) {
      await invoke('resume_background_tasks')
    } else {
      await invoke('pause_background_tasks')
    }
    setIsPaused(!isPaused)
  }
  
  return (
    <div className="background-tasks">
      <div className="tasks-header">
        <h2>Background Tasks</h2>
        <div className="task-counts">
          <span className="badge">{queueStatus?.pending_count || 0} pending</span>
          <span className="badge success">{queueStatus?.completed_count || 0} completed</span>
          <span className="badge error">{queueStatus?.failed_count || 0} failed</span>
        </div>
        <div className="controls">
          <button onClick={togglePause}>
            {isPaused ? '▶️ Resume' : '⏸️ Pause'}
          </button>
          <button onClick={() => invoke('clear_completed_tasks')}>
            Clear Completed
          </button>
        </div>
      </div>
      
      {/* Active Task */}
      {queueStatus?.active_task && (
        <div className="active-task">
          <h3>Currently Processing</h3>
          <TaskCard task={queueStatus.active_task} />
        </div>
      )}
      
      {/* Task Lists */}
      <TaskList 
        title="Pending" 
        tasks={queueStatus?.tasks.filter(t => t.status === 'Pending')} 
      />
      <TaskList 
        title="Failed" 
        tasks={queueStatus?.tasks.filter(t => t.status.Failed)} 
        onRetry={(id) => invoke('retry_task', { taskId: id })}
      />
      <TaskList 
        title="Completed" 
        tasks={queueStatus?.tasks.filter(t => t.status === 'Completed')} 
        collapsible={true}
      />
    </div>
  )
}
```

#### B. Task Card Component

```jsx
function TaskCard({ task, onRetry }) {
  const getStatusIcon = () => {
    if (task.status === 'Pending') return '⏳'
    if (task.status.Processing) return '⚙️'
    if (task.status === 'Completed') return '✅'
    if (task.status.Failed) return '❌'
  }
  
  const getFileName = () => {
    if (task.task_type.TranscribeOrphan) {
      return task.task_type.TranscribeOrphan.audio_path.split('/').pop()
    }
    if (task.task_type.TranscribeImported) {
      return task.task_type.TranscribeImported.original_name
    }
  }
  
  return (
    <div className="task-card">
      <span className="status-icon">{getStatusIcon()}</span>
      <div className="task-info">
        <div className="task-name">{getFileName()}</div>
        {task.status.Processing && (
          <div className="progress-bar">
            <div 
              className="progress-fill" 
              style={{ width: `${task.status.Processing.progress * 100}%` }}
            />
          </div>
        )}
        {task.error && (
          <div className="error-message">{task.error}</div>
        )}
      </div>
      {task.status.Failed && onRetry && (
        <button onClick={() => onRetry(task.id)}>Retry</button>
      )}
    </div>
  )
}
```

#### C. Main Tab Badge

```jsx
// Update main transcriptions tab to show background activity
<Tab>
  Transcriptions
  {backgroundTasksActive > 0 && (
    <span className="tab-badge">{backgroundTasksActive}</span>
  )}
</Tab>
```

### 6. Tauri Commands

```rust
// Add to commands.rs

#[tauri::command]
pub async fn get_queue_status(
    queue: State<'_, QueueManager>,
) -> Result<QueueStatus, String> {
    queue.get_queue_status().await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pause_background_tasks(
    queue: State<'_, QueueManager>,
) -> Result<(), String> {
    queue.pause().await;
    Ok(())
}

#[tauri::command]
pub async fn resume_background_tasks(
    queue: State<'_, QueueManager>,
) -> Result<(), String> {
    queue.resume().await;
    Ok(())
}

#[tauri::command]
pub async fn retry_task(
    queue: State<'_, QueueManager>,
    task_id: String,
) -> Result<(), String> {
    queue.retry_failed(task_id).await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_completed_tasks(
    queue: State<'_, QueueManager>,
) -> Result<(), String> {
    queue.clear_completed().await
        .map_err(|e| e.to_string())
}
```

### 7. Startup Sequence

```rust
// In lib.rs setup()

// 1. Initialize database
let db = DatabaseManager::new(pool).await?;

// 2. Initialize queue manager with database
let queue_manager = QueueManager::new(db.clone(), app_state.state.clone());

// 3. Scan for orphaned files and queue them
let scanner = OrphanScanner::new(notes_dir, db.clone());
let orphan_count = scanner.scan_and_queue_orphans().await?;
info!("Found {} orphaned audio files", orphan_count);

// 4. Add orphans to queue (if not already queued)
for orphan_path in orphans {
    let task = BackgroundTask {
        id: generate_id(),
        task_type: TaskType::TranscribeOrphan {
            audio_path: orphan_path.clone(),
            output_path: orphan_path.with_extension("txt"),
        },
        priority: TaskPriority::Low,
        status: TaskStatus::Pending,
        created_at: Local::now(),
        started_at: None,
        completed_at: None,
        retry_count: 0,
        max_retries: 1,
        error: None,
    };
    
    queue_manager.add_task(task).await;
}

// 5. Start queue worker
queue_manager.start_worker();
```

### 8. Integration with Main Recording Flow

```rust
// In stop_recording command

// Set state to Processing (existing)
*state.state.lock().await = RecordingState::Processing;

// This automatically prevents new background tasks from starting
// Current background task (if any) will complete

// ... existing transcription logic ...

// Set state back to Idle (existing)
*state.state.lock().await = RecordingState::Idle;

// Background queue will automatically resume
```

### 9. Database Persistence

All queue state is stored in the database - no separate JSON files needed:

```sql
-- Queue state is always consistent
SELECT 
    (SELECT COUNT(*) FROM background_tasks WHERE status = 'pending') as pending,
    (SELECT COUNT(*) FROM background_tasks WHERE status = 'processing') as processing,
    (SELECT COUNT(*) FROM background_tasks WHERE status = 'failed') as failed,
    (SELECT value FROM app_state WHERE key = 'queue_paused') as is_paused;
```

The database provides:
- **Atomic operations** - No race conditions
- **Crash recovery** - Automatic on restart
- **Query efficiency** - Indexed lookups
- **Transaction safety** - All-or-nothing updates

## Implementation Phases

### Phase 1: Core Queue System (4-5 hours)
- [ ] Create BackgroundTask and QueueManager structures
- [ ] Implement queue worker with state checking
- [ ] Add persistence to survive restarts
- [ ] Integrate with existing RecordingState

### Phase 2: File Discovery (2-3 hours)
- [ ] Implement OrphanScanner
- [ ] Scan on startup
- [ ] Add discovered files to queue
- [ ] Handle different audio formats

### Phase 3: UI Implementation (3-4 hours)
- [ ] Create Background Tasks tab
- [ ] Add task cards with status
- [ ] Implement pause/resume controls
- [ ] Add retry functionality for failed tasks
- [ ] Show badge on main tab

### Phase 4: Testing & Polish (2-3 hours)
- [ ] Test queue persistence
- [ ] Test interaction with manual recording
- [ ] Handle edge cases (corrupt files, etc.)
- [ ] Performance optimization

## Benefits

1. **Non-intrusive**: Background tasks don't interfere with user workflow
2. **Automatic**: Discovers and processes orphaned files automatically
3. **Resilient**: Retries failed tasks, survives restarts
4. **Transparent**: Clear UI showing what's happening
5. **Efficient**: Single worker prevents resource contention
6. **Flexible**: Extensible for future task types

## Future Enhancements

1. **Batch Processing**: Process multiple small files in parallel
2. **Model Selection**: Different models for background vs foreground
3. **Scheduling**: Only run during certain hours
4. **Cancellation**: Cancel individual tasks
5. **Reordering**: Drag-and-drop to reorder queue
6. **Import UI**: Drag-and-drop external audio files
7. **Progress Estimation**: Show time remaining
8. **Statistics**: Show processing stats and history