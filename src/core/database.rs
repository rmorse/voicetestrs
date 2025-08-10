use anyhow::Result;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    pub session_id: Option<i64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptionStatus {
    Pending,
    Processing,
    Complete,
    Failed,
    Orphaned,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptionSource {
    Recording,
    Import,
    Orphan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundTask {
    pub id: String,
    pub transcription_id: Option<String>,
    pub task_type: String,
    pub priority: i32,
    pub status: TaskStatus,
    pub created_at: DateTime<Local>,
    pub started_at: Option<DateTime<Local>>,
    pub completed_at: Option<DateTime<Local>>,
    pub retry_count: i32,
    pub max_retries: i32,
    pub error_message: Option<String>,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStatus {
    pub is_paused: bool,
    pub pending_count: usize,
    pub processing_count: usize,
    pub completed_count: usize,
    pub failed_count: usize,
    pub active_task: Option<BackgroundTask>,
}

impl Transcription {
    pub fn new_orphan(audio_path: String) -> Self {
        let id = Self::extract_id_from_path(&audio_path);
        Self {
            id,
            audio_path,
            text_path: None,
            transcription_text: None,
            created_at: Local::now(),
            transcribed_at: None,
            duration_seconds: 0.0,
            file_size_bytes: 0,
            language: "en".to_string(),
            model: "base.en".to_string(),
            status: TranscriptionStatus::Orphaned,
            source: TranscriptionSource::Orphan,
            error_message: None,
            metadata: None,
            session_id: None,
        }
    }

    pub fn new_recording(audio_path: String) -> Self {
        let id = Self::extract_id_from_path(&audio_path);
        Self {
            id,
            audio_path,
            text_path: None,
            transcription_text: None,
            created_at: Local::now(),
            transcribed_at: None,
            duration_seconds: 0.0,
            file_size_bytes: 0,
            language: "en".to_string(),
            model: "base.en".to_string(),
            status: TranscriptionStatus::Pending,
            source: TranscriptionSource::Recording,
            error_message: None,
            metadata: None,
            session_id: None,
        }
    }

    fn extract_id_from_path(path: &str) -> String {
        // Extract YYYYMMDD-HHMMSS from path
        // notes/2024/2024-01-15/143022-voice-note.wav -> 20240115-143022
        let path_buf = PathBuf::from(path);
        
        let date_part = path_buf
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .replace("-", "");
        
        let time_part = path_buf
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.split('-').next())
            .unwrap_or("");
        
        format!("{}-{}", date_part, time_part)
    }
}

impl Default for TranscriptionStatus {
    fn default() -> Self {
        TranscriptionStatus::Pending
    }
}

impl Default for TranscriptionSource {
    fn default() -> Self {
        TranscriptionSource::Recording
    }
}

impl Default for TaskStatus {
    fn default() -> Self {
        TaskStatus::Pending
    }
}