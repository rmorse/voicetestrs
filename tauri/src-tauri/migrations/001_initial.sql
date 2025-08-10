-- Initial database schema for VoiceTextRS

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
    status TEXT NOT NULL DEFAULT 'pending', 
    source TEXT NOT NULL DEFAULT 'recording',
    error_message TEXT,
    metadata TEXT,                          -- JSON string for additional data
    session_id INTEGER,
    
    CHECK (status IN ('pending', 'processing', 'complete', 'failed', 'orphaned')),
    CHECK (source IN ('recording', 'import', 'orphan'))
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_transcriptions_created_at ON transcriptions(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_transcriptions_status ON transcriptions(status);
CREATE INDEX IF NOT EXISTS idx_transcriptions_source ON transcriptions(source);

-- Background task queue
CREATE TABLE IF NOT EXISTS background_tasks (
    id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    transcription_id TEXT,                  -- Link to transcription if applicable
    task_type TEXT NOT NULL,                -- transcribe_orphan, transcribe_import, reprocess
    priority INTEGER DEFAULT 0,             -- 0=low, 1=normal, 2=high
    status TEXT NOT NULL DEFAULT 'pending',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    started_at DATETIME,
    completed_at DATETIME,
    retry_count INTEGER DEFAULT 0,
    max_retries INTEGER DEFAULT 1,
    error_message TEXT,
    payload TEXT NOT NULL,                  -- JSON string for task-specific data
    
    CHECK (status IN ('pending', 'processing', 'completed', 'failed', 'cancelled')),
    CHECK (priority >= 0 AND priority <= 2),
    FOREIGN KEY (transcription_id) REFERENCES transcriptions(id) ON DELETE CASCADE
);

-- Index for efficient queue operations
CREATE INDEX IF NOT EXISTS idx_tasks_queue ON background_tasks(status, priority DESC, created_at)
WHERE status IN ('pending', 'processing');

-- Application state
CREATE TABLE IF NOT EXISTS app_state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Settings
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
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
CREATE TABLE IF NOT EXISTS _migrations (
    version INTEGER PRIMARY KEY,
    applied_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    description TEXT
);