# Database Architecture - SQLite Integration

## Overview
Use SQLite as the central metadata store and state manager, while keeping audio/text files in the filesystem. This provides the best of both worlds: database performance and reliability with file system simplicity.

## Database Schema

```sql
-- Main transcriptions table
CREATE TABLE IF NOT EXISTS transcriptions (
    id TEXT PRIMARY KEY,                    -- Format: YYYYMMDD-HHMMSS
    audio_path TEXT NOT NULL,               -- Relative path from notes/
    text_path TEXT,                         -- Relative path from notes/
    transcription_text TEXT,                -- Full text (cached for search)
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    transcribed_at DATETIME,                -- When transcription completed
    duration_seconds REAL DEFAULT 0.0,
    file_size_bytes INTEGER DEFAULT 0,
    language TEXT DEFAULT 'en',
    model TEXT DEFAULT 'base.en',
    status TEXT NOT NULL DEFAULT 'pending', -- pending, processing, complete, failed, orphaned
    source TEXT NOT NULL DEFAULT 'recording', -- recording, import, orphan
    error_message TEXT,
    metadata JSON,                          -- Additional flexible data
    
    CHECK (status IN ('pending', 'processing', 'complete', 'failed', 'orphaned')),
    CHECK (source IN ('recording', 'import', 'orphan'))
);

-- Indexes for performance
CREATE INDEX idx_transcriptions_created_at ON transcriptions(created_at DESC);
CREATE INDEX idx_transcriptions_status ON transcriptions(status);
CREATE INDEX idx_transcriptions_source ON transcriptions(source);

-- Full-text search
CREATE VIRTUAL TABLE IF NOT EXISTS transcriptions_fts USING fts5(
    transcription_text,
    content='transcriptions',
    content_rowid='rowid',
    tokenize='porter unicode61'
);

-- Triggers to keep FTS in sync
CREATE TRIGGER transcriptions_ai AFTER INSERT ON transcriptions BEGIN
    INSERT INTO transcriptions_fts(rowid, transcription_text) 
    VALUES (new.rowid, new.transcription_text);
END;

CREATE TRIGGER transcriptions_ad AFTER DELETE ON transcriptions BEGIN
    DELETE FROM transcriptions_fts WHERE rowid = old.rowid;
END;

CREATE TRIGGER transcriptions_au AFTER UPDATE ON transcriptions BEGIN
    UPDATE transcriptions_fts 
    SET transcription_text = new.transcription_text 
    WHERE rowid = new.rowid;
END;

-- Background task queue
CREATE TABLE IF NOT EXISTS background_tasks (
    id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    transcription_id TEXT,                  -- Link to transcription if applicable
    task_type TEXT NOT NULL,                -- transcribe_orphan, transcribe_import, reprocess
    priority INTEGER DEFAULT 0,             -- 0=low, 1=normal, 2=high
    status TEXT NOT NULL DEFAULT 'pending', -- pending, processing, completed, failed, cancelled
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    started_at DATETIME,
    completed_at DATETIME,
    retry_count INTEGER DEFAULT 0,
    max_retries INTEGER DEFAULT 1,
    error_message TEXT,
    payload JSON NOT NULL,                  -- Task-specific data
    
    CHECK (status IN ('pending', 'processing', 'completed', 'failed', 'cancelled')),
    CHECK (priority >= 0 AND priority <= 2),
    FOREIGN KEY (transcription_id) REFERENCES transcriptions(id) ON DELETE CASCADE
);

-- Index for efficient queue operations
CREATE INDEX idx_tasks_queue ON background_tasks(status, priority DESC, created_at)
WHERE status IN ('pending', 'processing');

-- Application state
CREATE TABLE IF NOT EXISTS app_state (
    key TEXT PRIMARY KEY,
    value JSON NOT NULL,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Settings
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value JSON NOT NULL,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Recording sessions (for grouping)
CREATE TABLE IF NOT EXISTS sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    ended_at DATETIME,
    transcription_count INTEGER DEFAULT 0,
    total_duration_seconds REAL DEFAULT 0.0
);

-- Link transcriptions to sessions
ALTER TABLE transcriptions ADD COLUMN session_id INTEGER 
    REFERENCES sessions(id) ON DELETE SET NULL;

-- Future: Tags
CREATE TABLE IF NOT EXISTS tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT UNIQUE NOT NULL,
    color TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS transcription_tags (
    transcription_id TEXT NOT NULL,
    tag_id INTEGER NOT NULL,
    PRIMARY KEY (transcription_id, tag_id),
    FOREIGN KEY (transcription_id) REFERENCES transcriptions(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
);

-- Migration tracking
CREATE TABLE IF NOT EXISTS migrations (
    version INTEGER PRIMARY KEY,
    applied_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    description TEXT
);
```

## Tauri SQL Plugin Setup

### 1. Installation

```toml
# tauri/src-tauri/Cargo.toml
[dependencies]
tauri-plugin-sql = { version = "2.0", features = ["sqlite"] }
```

```javascript
// tauri/package.json
"dependencies": {
  "@tauri-apps/plugin-sql": "^2.0.0"
}
```

### 2. Plugin Configuration

```rust
// tauri/src-tauri/src/lib.rs
use tauri_plugin_sql::{Migration, MigrationKind};

pub fn run() {
    // ... existing code ...
    
    let migrations = vec![
        Migration {
            version: 1,
            description: "Initial schema",
            sql: include_str!("../migrations/001_initial.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 2,
            description: "Add full-text search",
            sql: include_str!("../migrations/002_fts.sql"),
            kind: MigrationKind::Up,
        },
    ];
    
    tauri::Builder::default()
        .plugin(
            tauri_plugin_sql::Builder::new()
                .add_migrations("sqlite:voicetextrs.db", migrations)
                .build()
        )
        // ... rest of setup
}
```

## Database Manager Implementation

### 1. Rust Side (using Tauri SQL plugin)

```rust
// src/core/database.rs
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Local};
use tauri_plugin_sql::{Pool};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcription {
    pub id: String,
    pub audio_path: String,
    pub text_path: Option<String>,
    pub transcription_text: Option<String>,
    pub created_at: DateTime<Local>,
    pub transcribed_at: Option<DateTime<Local>>,
    pub duration_seconds: f64,
    pub file_size_bytes: i64,
    pub language: String,
    pub model: String,
    pub status: TranscriptionStatus,
    pub source: TranscriptionSource,
    pub error_message: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
pub enum TranscriptionStatus {
    #[sqlx(rename = "pending")]
    Pending,
    #[sqlx(rename = "processing")]
    Processing,
    #[sqlx(rename = "complete")]
    Complete,
    #[sqlx(rename = "failed")]
    Failed,
    #[sqlx(rename = "orphaned")]
    Orphaned,
}

pub struct DatabaseManager {
    pool: Pool,
}

impl DatabaseManager {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }
    
    // Transcription operations
    pub async fn insert_transcription(&self, transcription: &Transcription) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO transcriptions (
                id, audio_path, text_path, transcription_text,
                created_at, duration_seconds, file_size_bytes,
                language, model, status, source, metadata
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            transcription.id,
            transcription.audio_path,
            transcription.text_path,
            transcription.transcription_text,
            transcription.created_at,
            transcription.duration_seconds,
            transcription.file_size_bytes,
            transcription.language,
            transcription.model,
            transcription.status,
            transcription.source,
            transcription.metadata
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
    
    pub async fn get_transcriptions(
        &self,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<Transcription>> {
        let records = sqlx::query_as!(
            Transcription,
            r#"
            SELECT * FROM transcriptions
            WHERE status = 'complete'
            ORDER BY created_at DESC
            LIMIT ? OFFSET ?
            "#,
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await?;
        
        Ok(records)
    }
    
    pub async fn search_transcriptions(&self, query: &str) -> Result<Vec<Transcription>> {
        let records = sqlx::query_as!(
            Transcription,
            r#"
            SELECT t.* FROM transcriptions t
            JOIN transcriptions_fts fts ON t.rowid = fts.rowid
            WHERE fts.transcription_text MATCH ?
            ORDER BY rank
            LIMIT 100
            "#,
            query
        )
        .fetch_all(&self.pool)
        .await?;
        
        Ok(records)
    }
    
    pub async fn update_transcription_status(
        &self,
        id: &str,
        status: TranscriptionStatus,
        error: Option<String>,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE transcriptions 
            SET status = ?, error_message = ?, transcribed_at = ?
            WHERE id = ?
            "#,
            status,
            error,
            if status == TranscriptionStatus::Complete { Some(Local::now()) } else { None },
            id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
    
    pub async fn find_orphaned_audio(&self) -> Result<Vec<Transcription>> {
        let records = sqlx::query_as!(
            Transcription,
            r#"
            SELECT * FROM transcriptions
            WHERE status = 'orphaned'
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;
        
        Ok(records)
    }
}
```

### 2. JavaScript Side (Frontend)

```javascript
// tauri/src/lib/database.js
import Database from '@tauri-apps/plugin-sql';

let db = null;

export async function initDatabase() {
  if (!db) {
    db = await Database.load('sqlite:voicetextrs.db');
  }
  return db;
}

export async function getTranscriptions(limit = 50, offset = 0) {
  const db = await initDatabase();
  const result = await db.select(
    `SELECT * FROM transcriptions 
     WHERE status = 'complete' 
     ORDER BY created_at DESC 
     LIMIT ? OFFSET ?`,
    [limit, offset]
  );
  return result;
}

export async function searchTranscriptions(query) {
  const db = await initDatabase();
  const result = await db.select(
    `SELECT t.* FROM transcriptions t
     JOIN transcriptions_fts fts ON t.rowid = fts.rowid
     WHERE fts.transcription_text MATCH ?
     ORDER BY rank
     LIMIT 100`,
    [query]
  );
  return result;
}

export async function getQueueStatus() {
  const db = await initDatabase();
  const pending = await db.select(
    `SELECT COUNT(*) as count FROM background_tasks WHERE status = 'pending'`
  );
  const processing = await db.select(
    `SELECT * FROM background_tasks WHERE status = 'processing' LIMIT 1`
  );
  const failed = await db.select(
    `SELECT COUNT(*) as count FROM background_tasks WHERE status = 'failed'`
  );
  
  return {
    pendingCount: pending[0].count,
    activeTask: processing[0] || null,
    failedCount: failed[0].count
  };
}

// React hook for real-time updates
export function useTranscriptions() {
  const [transcriptions, setTranscriptions] = useState([]);
  const [loading, setLoading] = useState(true);
  
  useEffect(() => {
    // Initial load
    loadTranscriptions();
    
    // Listen for database changes
    const unlisten = listen('database-changed', async (event) => {
      if (event.payload.table === 'transcriptions') {
        // Reload transcriptions
        await loadTranscriptions();
      }
    });
    
    return () => unlisten();
  }, []);
  
  const loadTranscriptions = async () => {
    setLoading(true);
    const data = await getTranscriptions();
    setTranscriptions(data);
    setLoading(false);
  };
  
  return { transcriptions, loading, refresh: loadTranscriptions };
}
```

## File System Synchronization

### 1. Startup Sync

```rust
// src/core/sync.rs
use walkdir::WalkDir;
use std::path::{Path, PathBuf};

pub struct FileSystemSync {
    notes_dir: PathBuf,
    db: Arc<DatabaseManager>,
}

impl FileSystemSync {
    pub async fn sync_on_startup(&self) -> Result<SyncReport> {
        let mut report = SyncReport::default();
        
        // Step 1: Scan file system
        let files = self.scan_audio_files()?;
        
        // Step 2: Get existing records from DB
        let existing = self.db.get_all_transcription_ids().await?;
        
        // Step 3: Process each file
        for audio_path in files {
            let id = self.extract_id_from_path(&audio_path);
            
            if !existing.contains(&id) {
                // New orphaned file found
                let transcription = self.create_orphan_record(&audio_path)?;
                self.db.insert_transcription(&transcription).await?;
                
                // Add to background queue
                self.queue_for_processing(&transcription).await?;
                report.orphans_found += 1;
            } else {
                // Check if transcription exists
                let txt_path = audio_path.with_extension("txt");
                if txt_path.exists() {
                    // Update DB with text if needed
                    self.update_transcription_text(&id, &txt_path).await?;
                    report.synced += 1;
                }
            }
        }
        
        // Step 4: Check for DB records without files
        for id in existing {
            let audio_path = self.construct_audio_path(&id);
            if !audio_path.exists() {
                // Mark as missing
                self.db.update_transcription_status(
                    &id,
                    TranscriptionStatus::Failed,
                    Some("Audio file not found".to_string())
                ).await?;
                report.missing_files += 1;
            }
        }
        
        Ok(report)
    }
    
    fn scan_audio_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        
        for entry in WalkDir::new(&self.notes_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if let Some(ext) = entry.path().extension() {
                if ext == "wav" || ext == "mp3" || ext == "m4a" {
                    files.push(entry.path().to_path_buf());
                }
            }
        }
        
        Ok(files)
    }
    
    fn extract_id_from_path(&self, path: &Path) -> String {
        // Extract YYYYMMDD-HHMMSS from path
        // notes/2024/2024-01-15/143022-voice-note.wav -> 20240115-143022
        let date_part = path.parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .replace("-", "");
        
        let time_part = path.file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.split('-').next())
            .unwrap_or("");
        
        format!("{}-{}", date_part, time_part)
    }
}

#[derive(Default, Debug)]
pub struct SyncReport {
    pub orphans_found: usize,
    pub synced: usize,
    pub missing_files: usize,
    pub errors: Vec<String>,
}
```

### 2. File System Watcher

```rust
// src/core/watcher.rs
use notify::{Watcher, RecursiveMode, Event};
use std::sync::mpsc::channel;

pub struct FileSystemWatcher {
    watcher: notify::RecommendedWatcher,
    db: Arc<DatabaseManager>,
}

impl FileSystemWatcher {
    pub fn start(&mut self) -> Result<()> {
        let (tx, rx) = channel();
        let db = self.db.clone();
        
        // Create watcher
        let mut watcher = notify::recommended_watcher(move |res: Result<Event>| {
            if let Ok(event) = res {
                tx.send(event).ok();
            }
        })?;
        
        // Watch notes directory
        watcher.watch(&self.notes_dir, RecursiveMode::Recursive)?;
        
        // Process events
        std::thread::spawn(move || {
            for event in rx {
                match event.kind {
                    notify::EventKind::Create(_) => {
                        if is_audio_file(&event.paths[0]) {
                            // New audio file created
                            db.handle_new_audio_file(&event.paths[0]);
                        } else if is_text_file(&event.paths[0]) {
                            // Transcription completed
                            db.handle_new_transcription(&event.paths[0]);
                        }
                    },
                    notify::EventKind::Remove(_) => {
                        // File deleted
                        db.handle_file_deleted(&event.paths[0]);
                    },
                    _ => {}
                }
            }
        });
        
        Ok(())
    }
}
```

## Background Task Queue with Database

### 1. Queue Manager

```rust
// src/core/queue_manager.rs
use tokio::time::{sleep, Duration};

pub struct QueueManager {
    db: Arc<DatabaseManager>,
    is_paused: Arc<AtomicBool>,
    app_state: Arc<Mutex<RecordingState>>,
    transcriber: Arc<Transcriber>,
}

impl QueueManager {
    pub async fn start_worker(self: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                if !self.should_process().await {
                    sleep(Duration::from_secs(1)).await;
                    continue;
                }
                
                // Get next task from database
                if let Some(task) = self.get_next_task().await {
                    self.process_task(task).await;
                }
                
                sleep(Duration::from_millis(100)).await;
            }
        });
    }
    
    async fn get_next_task(&self) -> Option<BackgroundTask> {
        // Atomic database operation to claim a task
        let result = sqlx::query!(
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
        )
        .fetch_optional(&self.db.pool)
        .await
        .ok()?;
        
        result.map(|r| BackgroundTask::from_row(r))
    }
    
    async fn process_task(&self, mut task: BackgroundTask) {
        // Emit progress event
        self.emit_task_update(&task).await;
        
        // Process based on type
        let result = match task.task_type {
            TaskType::TranscribeOrphan { audio_path } => {
                self.transcriber.transcribe(&audio_path).await
            },
            _ => Ok(()),
        };
        
        // Update database with result
        match result {
            Ok(transcription) => {
                // Update task as completed
                sqlx::query!(
                    "UPDATE background_tasks SET status = 'completed', 
                     completed_at = CURRENT_TIMESTAMP WHERE id = ?",
                    task.id
                ).execute(&self.db.pool).await.ok();
                
                // Update transcription record
                sqlx::query!(
                    "UPDATE transcriptions SET status = 'complete',
                     transcription_text = ?, transcribed_at = CURRENT_TIMESTAMP
                     WHERE id = ?",
                    transcription.text,
                    task.transcription_id
                ).execute(&self.db.pool).await.ok();
            },
            Err(e) => {
                // Handle retry logic
                if task.retry_count < task.max_retries {
                    // Reset to pending for retry
                    sqlx::query!(
                        "UPDATE background_tasks SET status = 'pending',
                         retry_count = retry_count + 1 WHERE id = ?",
                        task.id
                    ).execute(&self.db.pool).await.ok();
                } else {
                    // Mark as failed
                    sqlx::query!(
                        "UPDATE background_tasks SET status = 'failed',
                         error_message = ? WHERE id = ?",
                        e.to_string(),
                        task.id
                    ).execute(&self.db.pool).await.ok();
                }
            }
        }
        
        // Emit completion event
        self.emit_task_complete(&task).await;
    }
}
```

## Benefits of Database Approach

1. **Atomic Operations**: No race conditions or partial writes
2. **Fast Search**: Full-text search with FTS5
3. **Efficient Queries**: Indexed lookups, sorting, filtering
4. **Crash Recovery**: Automatic rollback and recovery
5. **Concurrency**: Multiple readers, safe writes
6. **Extensibility**: Easy to add new features
7. **Cross-Platform**: SQLite works everywhere
8. **Zero Config**: No server needed

## Migration Path

### Phase 1: Database Setup (2 hours)
- Install Tauri SQL plugin
- Create initial schema
- Set up migrations

### Phase 2: Data Migration (2 hours)
- Scan existing files
- Import metadata to database
- Verify data integrity

### Phase 3: Core Integration (4 hours)
- Replace JSON operations with DB queries
- Update Tauri commands
- Implement file system sync

### Phase 4: Background Queue (3 hours)
- Implement database-backed queue
- Add worker thread
- Handle retries and failures

### Phase 5: UI Updates (3 hours)
- Use database for all data
- Implement search
- Add background tasks tab

## Performance Targets

- Load 10,000 transcriptions: < 100ms
- Search 10,000 transcriptions: < 50ms
- Queue operations: < 10ms
- Startup sync (1000 files): < 2s
- Memory usage: < 50MB for database