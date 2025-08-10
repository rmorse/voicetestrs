# Background Task Queue System - Implementation Plan

## Overview
Add a background task queue for processing orphaned audio files (audio without transcriptions) while maintaining the priority of user-initiated recordings.

## Core Principles
1. **User recordings always have priority** - Never interfere with active recording/processing
2. **Single queue worker** - Process tasks sequentially to avoid resource contention
3. **Transparent to user** - Background tasks don't clutter main transcription list
4. **Resilient** - Survive app restarts, handle failures gracefully

## Architecture

### 1. Task Queue Data Model

```rust
// src/core/task_queue.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundTask {
    pub id: String,                    // Unique task ID
    pub task_type: TaskType,
    pub priority: TaskPriority,
    pub status: TaskStatus,
    pub created_at: DateTime<Local>,
    pub started_at: Option<DateTime<Local>>,
    pub completed_at: Option<DateTime<Local>>,
    pub retry_count: u8,
    pub max_retries: u8,               // Default: 1
    pub error: Option<String>,
}

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

### 2. Queue Manager

```rust
// src/core/queue_manager.rs

pub struct QueueManager {
    tasks: Arc<RwLock<VecDeque<BackgroundTask>>>,
    active_task: Arc<RwLock<Option<BackgroundTask>>>,
    is_paused: Arc<AtomicBool>,
    should_stop: Arc<AtomicBool>,
    worker_handle: Option<JoinHandle<()>>,
    app_state: Arc<Mutex<RecordingState>>, // Share with main app
    transcriber: Arc<Transcriber>,
}

impl QueueManager {
    pub fn new(app_state: Arc<Mutex<RecordingState>>) -> Self;
    
    // Core operations
    pub async fn add_task(&self, task: BackgroundTask);
    pub async fn get_queue_status(&self) -> QueueStatus;
    pub async fn pause(&self);
    pub async fn resume(&self);
    pub async fn retry_failed(&self, task_id: String);
    pub async fn clear_completed(&self);
    
    // Worker management
    fn start_worker(&mut self);
    async fn process_next_task(&self) -> Result<()>;
    async fn should_process(&self) -> bool;
    
    // Persistence
    pub async fn save_state(&self) -> Result<()>;
    pub async fn load_state(&mut self) -> Result<()>;
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

### 3. Processing Logic

```rust
async fn should_process(&self) -> bool {
    // Check if we should start processing a new task
    if self.is_paused.load(Ordering::Relaxed) {
        return false;
    }
    
    let app_state = self.app_state.lock().await;
    match *app_state {
        RecordingState::Idle => {
            // Safe to process background tasks
            true
        },
        RecordingState::Recording | RecordingState::Processing => {
            // User is actively recording/processing
            if self.active_task.read().await.is_some() {
                // Let current task finish but don't start new ones
                true
            } else {
                // Don't start new tasks
                false
            }
        }
    }
}

async fn process_next_task(&self) -> Result<()> {
    // Wait for right conditions
    while !self.should_process().await {
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        if self.should_stop.load(Ordering::Relaxed) {
            return Ok(());
        }
    }
    
    // Get next task
    let task = {
        let mut tasks = self.tasks.write().await;
        tasks.pop_front()
    };
    
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
        
        // Handle result
        match result {
            Ok(transcription) => {
                task.status = TaskStatus::Completed;
                task.completed_at = Some(Local::now());
                save_transcription(transcription);
            },
            Err(e) => {
                task.retry_count += 1;
                if task.retry_count < task.max_retries {
                    // Re-queue for retry
                    task.status = TaskStatus::Pending;
                    self.tasks.write().await.push_back(task.clone());
                } else {
                    // Mark as failed
                    task.status = TaskStatus::Failed { can_retry: true };
                    task.error = Some(e.to_string());
                }
            }
        }
        
        // Clear active task
        *self.active_task.write().await = None;
        
        // Save state to disk
        self.save_state().await?;
    }
    
    Ok(())
}
```

### 4. File Discovery

```rust
// src/core/orphan_scanner.rs

pub struct OrphanScanner {
    notes_dir: PathBuf,
}

impl OrphanScanner {
    pub async fn scan_for_orphans(&self) -> Vec<PathBuf> {
        let mut orphans = Vec::new();
        
        // Walk the notes directory
        for entry in WalkDir::new(&self.notes_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if let Some(ext) = entry.path().extension() {
                // Check if it's an audio file
                if ext == "wav" || ext == "mp3" || ext == "m4a" {
                    // Check if transcription exists
                    let txt_path = entry.path().with_extension("txt");
                    let json_path = entry.path().with_extension("json");
                    
                    if !txt_path.exists() || !json_path.exists() {
                        orphans.push(entry.path().to_path_buf());
                    }
                }
            }
        }
        
        orphans
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

// 1. Initialize queue manager
let queue_manager = QueueManager::new(app_state.state.clone());

// 2. Load persisted queue state
queue_manager.load_state().await.ok();

// 3. Scan for orphaned files
let scanner = OrphanScanner::new(notes_dir);
let orphans = scanner.scan_for_orphans().await;

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

### 9. Persistence

```json
// queue_state.json (in app data directory)
{
  "version": "1.0",
  "is_paused": false,
  "tasks": [
    {
      "id": "task_abc123",
      "task_type": {
        "TranscribeOrphan": {
          "audio_path": "notes/2024/2024-01-15/143022-voice-note.wav",
          "output_path": "notes/2024/2024-01-15/143022-voice-note.txt"
        }
      },
      "priority": "Low",
      "status": "Pending",
      "created_at": "2024-01-15T14:30:22Z",
      "started_at": null,
      "completed_at": null,
      "retry_count": 0,
      "max_retries": 1,
      "error": null
    }
  ]
}
```

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