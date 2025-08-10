# Database Migration Plan: Frontend to Backend with SQLx

## Overview
This document outlines the migration strategy to move database ownership from the frontend (JavaScript/Tauri SQL plugin) to the backend (Rust/SQLx), establishing proper architecture where the backend owns data and provides APIs to the frontend.

## Current Architecture Problems
- Database managed entirely by frontend JavaScript
- Backend cannot check database state during sync
- SQL queries scattered throughout frontend code
- Backwards data flow: Backend → Events → Frontend → Database
- Sync logic split between frontend and backend
- Need for workarounds like `force_sync`

## Target Architecture
- Backend owns and manages the SQLite database via SQLx
- Backend provides clean CRUD APIs via Tauri commands
- Frontend calls APIs without any SQL knowledge
- Single source of truth for sync operations
- Atomic transactions for data consistency

## Migration Phases

### Phase 1: Setup SQLx in Backend (Week 1)

#### 1.1 Add Dependencies
```toml
# Cargo.toml
[dependencies]
sqlx = { version = "0.8", features = [
    "runtime-tokio",      # Tokio runtime (already used by Tauri)
    "sqlite",             # SQLite support
    "migrate",            # Migration support
    "macros",             # Compile-time checked queries
    "chrono",             # DateTime support
    "json",               # JSON column support
] }
tokio = { version = "1", features = ["full"] }
```

#### 1.2 Database Configuration
```rust
// src/core/database/mod.rs
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use std::time::Duration;

pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(3))
            .connect(database_url)
            .await?;
        
        // Run migrations
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await?;
        
        Ok(Self { pool })
    }
}
```

#### 1.3 Migration Files
Create `migrations/` directory with existing schema:
```sql
-- migrations/001_initial.sql
-- Copy existing schema from tauri/src-tauri/migrations/
```

### Phase 2: Database Manager Implementation (Week 1-2)

#### 2.1 Data Models
```rust
// src/core/database/models.rs
use sqlx::FromRow;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct Transcription {
    pub id: String,
    pub audio_path: String,
    pub text_path: Option<String>,
    pub transcription_text: Option<String>,
    pub created_at: DateTime<Utc>,
    pub transcribed_at: Option<DateTime<Utc>>,
    pub duration_seconds: f64,
    pub file_size_bytes: i64,
    pub language: String,
    pub model: String,
    pub status: String,  // Will convert to enum
    pub source: String,
    pub error_message: Option<String>,
    pub metadata: Option<sqlx::types::Json<serde_json::Value>>,
    pub session_id: Option<i32>,
}
```

#### 2.2 CRUD Operations
```rust
// src/core/database/repository.rs
impl Database {
    // Create
    pub async fn insert_transcription(&self, t: &Transcription) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO transcriptions (
                id, audio_path, text_path, transcription_text,
                created_at, duration_seconds, file_size_bytes,
                language, model, status, source, metadata
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            t.id, t.audio_path, t.text_path, t.transcription_text,
            t.created_at, t.duration_seconds, t.file_size_bytes,
            t.language, t.model, t.status, t.source, t.metadata
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
    
    // Read
    pub async fn get_transcription(&self, id: &str) -> Result<Option<Transcription>> {
        Ok(sqlx::query_as!(
            Transcription,
            "SELECT * FROM transcriptions WHERE id = ?",
            id
        )
        .fetch_optional(&self.pool)
        .await?)
    }
    
    // Update
    pub async fn update_transcription_status(
        &self, 
        id: &str, 
        status: &str,
        error: Option<String>
    ) -> Result<()> {
        sqlx::query!(
            "UPDATE transcriptions SET status = ?, error_message = ? WHERE id = ?",
            status, error, id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
    
    // Delete
    pub async fn delete_transcription(&self, id: &str) -> Result<()> {
        sqlx::query!("DELETE FROM transcriptions WHERE id = ?", id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    
    // List with pagination
    pub async fn list_transcriptions(
        &self,
        limit: i32,
        offset: i32,
        status_filter: Option<String>
    ) -> Result<Vec<Transcription>> {
        let query = if let Some(status) = status_filter {
            sqlx::query_as!(
                Transcription,
                r#"
                SELECT * FROM transcriptions 
                WHERE status = ? 
                ORDER BY created_at DESC 
                LIMIT ? OFFSET ?
                "#,
                status, limit, offset
            )
        } else {
            sqlx::query_as!(
                Transcription,
                r#"
                SELECT * FROM transcriptions 
                ORDER BY created_at DESC 
                LIMIT ? OFFSET ?
                "#,
                limit, offset
            )
        };
        
        Ok(query.fetch_all(&self.pool).await?)
    }
    
    // Search with FTS
    pub async fn search_transcriptions(&self, query: &str) -> Result<Vec<Transcription>> {
        Ok(sqlx::query_as!(
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
        .await?)
    }
    
    // Get all IDs (for sync optimization)
    pub async fn get_all_transcription_ids(&self) -> Result<Vec<String>> {
        let records = sqlx::query!("SELECT id FROM transcriptions")
            .fetch_all(&self.pool)
            .await?;
        
        Ok(records.into_iter().map(|r| r.id).collect())
    }
}
```

### Phase 3: Refactor Sync System (Week 2)

#### 3.1 Smart Sync Implementation
```rust
// src/core/sync.rs
use crate::core::database::Database;

impl FileSystemSync {
    db: Arc<Database>,
    
    pub async fn sync_filesystem(&self) -> Result<SyncReport> {
        let mut report = SyncReport::default();
        
        // Get existing IDs from database
        let existing_ids: HashSet<String> = self.db
            .get_all_transcription_ids()
            .await?
            .into_iter()
            .collect();
        
        // Scan filesystem
        let audio_files = self.scan_audio_files()?;
        report.total_files_found = audio_files.len();
        
        // Process each file
        for audio_path in audio_files {
            let transcription = self.create_transcription_from_file(&audio_path)?;
            
            if !existing_ids.contains(&transcription.id) {
                // New file - insert
                self.db.insert_transcription(&transcription).await?;
                report.new_transcriptions += 1;
            } else {
                // Check if needs update (timestamp, status change, etc.)
                if let Some(existing) = self.db.get_transcription(&transcription.id).await? {
                    if self.needs_update(&existing, &transcription) {
                        self.db.update_transcription(&transcription).await?;
                        report.updated_transcriptions += 1;
                    }
                }
            }
        }
        
        // Check for deleted files
        for id in existing_ids {
            if !self.file_exists_for_id(&id) {
                self.db.update_transcription_status(&id, "missing", None).await?;
                report.missing_files += 1;
            }
        }
        
        Ok(report)
    }
    
    fn needs_update(&self, existing: &Transcription, new: &Transcription) -> bool {
        // Check if file has been modified since last sync
        existing.status != new.status ||
        existing.transcription_text != new.transcription_text ||
        existing.file_size_bytes != new.file_size_bytes
    }
}
```

### Phase 4: Create API Commands (Week 2-3)

#### 4.1 Tauri Commands
```rust
// src-tauri/src/api/transcriptions.rs
use tauri::State;
use crate::database::Database;

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
pub async fn sync_filesystem(
    db: State<'_, Arc<Database>>,
    app: AppHandle,
) -> Result<SyncReport, String> {
    let sync = FileSystemSync::new(db.clone(), notes_dir);
    let report = sync.sync_filesystem().await
        .map_err(|e| e.to_string())?;
    
    // Emit update event
    app.emit("sync-complete", &report)?;
    
    Ok(report)
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
```

#### 4.2 Register Commands
```rust
// src-tauri/src/lib.rs
pub fn run() {
    // Initialize database
    let database = Arc::new(
        Database::new("sqlite:voicetextrs.db").await?
    );
    
    tauri::Builder::default()
        .manage(database)  // Make available to commands
        .invoke_handler(tauri::generate_handler![
            // Transcription APIs
            api::transcriptions::get_transcriptions,
            api::transcriptions::get_transcription,
            api::transcriptions::update_transcription,
            api::transcriptions::delete_transcription,
            api::transcriptions::search_transcriptions,
            api::transcriptions::sync_filesystem,
            api::transcriptions::get_database_stats,
            api::transcriptions::clear_database,
            // ... existing commands
        ])
        .run(tauri::generate_context!())
        .expect("error running tauri app");
}
```

### Phase 5: Frontend Migration (Week 3)

#### 5.1 New API Client
```javascript
// src/lib/api.js
import { invoke } from '@tauri-apps/api/core';

export const api = {
  // Transcriptions
  async getTranscriptions(params = {}) {
    return invoke('get_transcriptions', params);
  },
  
  async getTranscription(id) {
    return invoke('get_transcription', { id });
  },
  
  async updateTranscription(id, updates) {
    return invoke('update_transcription', { id, updates });
  },
  
  async deleteTranscription(id) {
    return invoke('delete_transcription', { id });
  },
  
  async searchTranscriptions(query) {
    return invoke('search_transcriptions', { query });
  },
  
  async syncFilesystem() {
    return invoke('sync_filesystem');
  },
  
  async getDatabaseStats() {
    return invoke('get_database_stats');
  },
  
  async clearDatabase() {
    return invoke('clear_database');
  }
};
```

#### 5.2 Update React Components
```javascript
// src/App.jsx
import { api } from './lib/api';

// Replace direct database calls
const loadTranscriptions = async () => {
  try {
    const data = await api.getTranscriptions({ 
      limit: 50, 
      offset: 0,
      status: null 
    });
    setTranscriptions(data);
  } catch (err) {
    console.error('Failed to load transcriptions:', err);
  }
};

const fullResync = async () => {
  try {
    setSyncStatus('Starting full resync...');
    await api.clearDatabase();
    const report = await api.syncFilesystem();
    await loadTranscriptions();
    setSyncStatus('Full resync completed!');
  } catch (err) {
    console.error('Full resync failed:', err);
  }
};
```

#### 5.3 Remove Old Database Code
- Delete `src/lib/database.js`
- Remove SQL plugin imports
- Clean up event listeners for sync events

## Migration Strategy

### Step-by-Step Approach
1. **Parallel Development**: Add backend DB alongside frontend DB
2. **Feature Parity**: Implement all current features in backend
3. **Gradual Migration**: Switch features one by one
4. **Testing**: Verify each migrated feature
5. **Cleanup**: Remove old frontend DB code

### Testing Plan
- Unit tests for database operations
- Integration tests for sync logic
- E2E tests for API commands
- Performance benchmarks
- Data integrity verification

### Rollback Plan
- Keep frontend DB code until fully migrated
- Feature flags to toggle between old/new
- Database backup before migration
- Version tags for each migration phase

## Performance Improvements

### Expected Benefits
- **Sync Performance**: Direct DB access, no event overhead
- **Bulk Operations**: Batch inserts with transactions
- **Query Speed**: Prepared statements, connection pooling
- **Memory Usage**: Streaming large result sets
- **Concurrency**: Proper connection management

### Benchmarks to Track
- Time to sync 1000 files
- Query response time for 10,000 records
- Memory usage during sync
- Concurrent request handling

## Timeline

### Week 1
- [ ] Setup SQLx with migrations
- [ ] Basic database connection
- [ ] Initial CRUD operations

### Week 2
- [ ] Complete database manager
- [ ] Refactor sync system
- [ ] Begin API commands

### Week 3
- [ ] Complete API commands
- [ ] Begin frontend migration
- [ ] Testing and verification

### Week 4
- [ ] Complete frontend migration
- [ ] Performance testing
- [ ] Documentation
- [ ] Cleanup old code

## Success Criteria
- All database operations handled by backend
- No SQL in frontend code
- Sync checks database before updating
- Better performance than current implementation
- Clean separation of concerns
- Maintainable and testable code

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Data loss during migration | High | Backup database, gradual migration |
| Performance regression | Medium | Benchmark before/after, optimize queries |
| Breaking changes | High | Keep backward compatibility, feature flags |
| Complex debugging | Medium | Comprehensive logging, error handling |

## Notes

### Why SQLx over rusqlite?
- Compile-time checked queries
- Better async support
- Already used by Tauri
- Migration system built-in
- Type-safe query macros

### Database Location
- Development: `./voicetextrs.db`
- Production: App data directory
- Configurable via environment variable

### Future Enhancements
- Database migrations UI
- Backup/restore functionality
- Multi-user support
- Cloud sync capability
- Real-time updates via WebSocket

## Conclusion
This migration will establish a proper architecture where the backend owns and manages data, providing clean APIs to the frontend. This is the correct pattern for desktop applications and will make the codebase more maintainable, testable, and performant.