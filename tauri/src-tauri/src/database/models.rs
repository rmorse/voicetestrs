use sqlx::FromRow;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

#[derive(Debug, FromRow, Serialize, Deserialize, Clone)]
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
    pub status: String,
    pub source: String,
    pub error_message: Option<String>,
    pub metadata: Option<sqlx::types::Json<serde_json::Value>>,
    pub session_id: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TranscriptionUpdate {
    pub text_path: Option<String>,
    pub transcription_text: Option<String>,
    pub transcribed_at: Option<DateTime<Utc>>,
    pub status: Option<String>,
    pub error_message: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct SyncReport {
    pub total_files_found: usize,
    pub new_transcriptions: usize,
    pub updated_transcriptions: usize,
    pub missing_files: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DatabaseStats {
    pub total_transcriptions: i64,
    pub total_size_bytes: i64,
    pub total_duration_seconds: f64,
    pub pending_count: i64,
    pub completed_count: i64,
    pub failed_count: i64,
}