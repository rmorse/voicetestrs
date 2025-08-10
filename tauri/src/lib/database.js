import Database from '@tauri-apps/plugin-sql';
import { invoke } from '@tauri-apps/api/core';

let db = null;

// Initialize database connection
export async function initDatabase() {
  if (!db) {
    console.log('Initializing database connection...');
    try {
      db = await Database.load('sqlite:voicetextrs.db');
      console.log('Database connected successfully');
      
      // Check if tables exist
      const tables = await db.select("SELECT name FROM sqlite_master WHERE type='table'");
      console.log('Database tables:', tables);
    } catch (err) {
      console.error('Failed to initialize database:', err);
      throw err;
    }
  }
  return db;
}

// Get transcriptions with filtering
export async function getTranscriptions(filter = {}) {
  const { 
    status, // Don't set a default - let null mean "all statuses"
    limit = 50, 
    offset = 0,
    orderBy = 'created_at DESC'
  } = filter;
  
  const db = await initDatabase();
  
  let query = 'SELECT * FROM transcriptions WHERE 1=1';
  const params = [];
  
  // Only add status filter if explicitly provided and not null
  if (status !== null && status !== undefined) {
    query += ' AND status = ?';
    params.push(status);
  }
  
  query += ` ORDER BY ${orderBy} LIMIT ? OFFSET ?`;
  params.push(limit, offset);
  
  return await db.select(query, params);
}

// Search transcriptions using full-text search
export async function searchTranscriptions(searchQuery) {
  if (!searchQuery || searchQuery.trim() === '') {
    return await getTranscriptions();
  }
  
  const db = await initDatabase();
  
  const query = `
    SELECT t.* FROM transcriptions t
    JOIN transcriptions_fts fts ON t.rowid = fts.rowid
    WHERE fts.transcription_text MATCH ?
    ORDER BY rank
    LIMIT 100
  `;
  
  return await db.select(query, [searchQuery]);
}

// Get a single transcription by ID
export async function getTranscription(id) {
  const db = await initDatabase();
  const result = await db.select(
    'SELECT * FROM transcriptions WHERE id = ?',
    [id]
  );
  return result[0] || null;
}

// Insert or update a transcription
export async function insertTranscription(transcription) {
  const db = await initDatabase();
  
  const query = `
    INSERT INTO transcriptions (
      id, audio_path, text_path, transcription_text,
      created_at, duration_seconds, file_size_bytes,
      language, model, status, source, metadata
    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    ON CONFLICT(id) DO UPDATE SET
      audio_path = COALESCE(excluded.audio_path, transcriptions.audio_path),
      text_path = COALESCE(excluded.text_path, transcriptions.text_path),
      transcription_text = COALESCE(excluded.transcription_text, transcriptions.transcription_text),
      duration_seconds = excluded.duration_seconds,
      file_size_bytes = excluded.file_size_bytes,
      language = excluded.language,
      model = excluded.model,
      status = CASE 
        WHEN excluded.status = 'complete' THEN 'complete'
        WHEN transcriptions.status = 'complete' THEN 'complete'
        ELSE excluded.status
      END,
      source = excluded.source,
      metadata = COALESCE(excluded.metadata, transcriptions.metadata)
  `;
  
  const params = [
    transcription.id,
    transcription.audio_path,
    transcription.text_path || null,
    transcription.transcription_text || null,
    transcription.created_at || new Date().toISOString(),
    transcription.duration_seconds || 0,
    transcription.file_size_bytes || 0,
    transcription.language || 'en',
    transcription.model || 'base.en',
    transcription.status || 'pending',
    transcription.source || 'recording',
    transcription.metadata ? JSON.stringify(transcription.metadata) : null
  ];
  
  return await db.execute(query, params);
}

// Update transcription status
export async function updateTranscriptionStatus(id, status, errorMessage = null) {
  const db = await initDatabase();
  
  const query = `
    UPDATE transcriptions 
    SET status = ?, error_message = ?, 
        transcribed_at = CASE WHEN ? = 'complete' THEN datetime('now') ELSE transcribed_at END
    WHERE id = ?
  `;
  
  return await db.execute(query, [status, errorMessage, status, id]);
}

// Update transcription text
export async function updateTranscriptionText(id, text) {
  const db = await initDatabase();
  
  const query = `
    UPDATE transcriptions 
    SET transcription_text = ?, 
        status = 'complete',
        transcribed_at = datetime('now')
    WHERE id = ?
  `;
  
  return await db.execute(query, [text, id]);
}

// Delete a transcription
export async function deleteTranscription(id) {
  const db = await initDatabase();
  return await db.execute('DELETE FROM transcriptions WHERE id = ?', [id]);
}

// Get queue status
export async function getQueueStatus() {
  const db = await initDatabase();
  
  const counts = await db.select(`
    SELECT 
      SUM(CASE WHEN status = 'pending' THEN 1 ELSE 0 END) as pending_count,
      SUM(CASE WHEN status = 'processing' THEN 1 ELSE 0 END) as processing_count,
      SUM(CASE WHEN status = 'completed' THEN 1 ELSE 0 END) as completed_count,
      SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END) as failed_count
    FROM background_tasks
  `);
  
  const activeTask = await db.select(`
    SELECT * FROM background_tasks 
    WHERE status = 'processing' 
    LIMIT 1
  `);
  
  return {
    pending_count: counts[0]?.pending_count || 0,
    processing_count: counts[0]?.processing_count || 0,
    completed_count: counts[0]?.completed_count || 0,
    failed_count: counts[0]?.failed_count || 0,
    active_task: activeTask[0] || null,
    is_paused: false // We'll get this from app state later
  };
}

// Get all background tasks
export async function getBackgroundTasks(status = null) {
  const db = await initDatabase();
  
  let query = 'SELECT * FROM background_tasks';
  const params = [];
  
  if (status) {
    query += ' WHERE status = ?';
    params.push(status);
  }
  
  query += ' ORDER BY priority DESC, created_at DESC';
  
  return await db.select(query, params);
}

// Enqueue a new task
export async function enqueueTask(task) {
  const db = await initDatabase();
  
  const query = `
    INSERT INTO background_tasks (
      transcription_id, task_type, priority, status, payload
    ) VALUES (?, ?, ?, 'pending', ?)
  `;
  
  const params = [
    task.transcription_id || null,
    task.task_type,
    task.priority || 0,
    JSON.stringify(task.payload || {})
  ];
  
  return await db.execute(query, params);
}

// Retry a failed task
export async function retryTask(taskId) {
  const db = await initDatabase();
  
  const query = `
    UPDATE background_tasks 
    SET status = 'pending', 
        retry_count = retry_count + 1,
        error_message = NULL
    WHERE id = ? AND status = 'failed'
  `;
  
  return await db.execute(query, [taskId]);
}

// Clear completed tasks
export async function clearCompletedTasks() {
  const db = await initDatabase();
  return await db.execute("DELETE FROM background_tasks WHERE status = 'completed'");
}

// Get database statistics
export async function getDatabaseStats() {
  const db = await initDatabase();
  
  const stats = await db.select(`
    SELECT 
      COUNT(*) as total_transcriptions,
      SUM(CASE WHEN status = 'complete' THEN 1 ELSE 0 END) as completed,
      SUM(CASE WHEN status = 'orphaned' THEN 1 ELSE 0 END) as orphaned,
      SUM(duration_seconds) as total_duration,
      SUM(file_size_bytes) as total_size
    FROM transcriptions
  `);
  
  return stats[0] || {
    total_transcriptions: 0,
    completed: 0,
    orphaned: 0,
    total_duration: 0,
    total_size: 0
  };
}

// Clean up duplicate transcriptions (keep the one with proper paths)
export async function cleanupDuplicates() {
  const db = await initDatabase();
  
  // Delete entries with Windows absolute paths (they're duplicates)
  const query = `
    DELETE FROM transcriptions 
    WHERE audio_path LIKE '\\\\?\\%'
       OR audio_path LIKE 'C:%'
       OR audio_path LIKE 'D:%'
  `;
  
  return await db.execute(query);
}

// Export all functions for use in React components
export default {
  initDatabase,
  getTranscriptions,
  searchTranscriptions,
  getTranscription,
  insertTranscription,
  updateTranscriptionStatus,
  updateTranscriptionText,
  deleteTranscription,
  getQueueStatus,
  getBackgroundTasks,
  enqueueTask,
  retryTask,
  clearCompletedTasks,
  getDatabaseStats,
  cleanupDuplicates
};