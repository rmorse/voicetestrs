use anyhow::Result;
use chrono::{DateTime, Local, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::core::database::{Transcription, TranscriptionStatus, TranscriptionSource};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncReport {
    pub total_files_found: usize,
    pub new_transcriptions: usize,
    pub orphaned_audio: usize,
    pub completed_transcriptions: usize,
    pub errors: Vec<String>,
    pub synced_at: DateTime<Local>,
}

impl Default for SyncReport {
    fn default() -> Self {
        Self {
            total_files_found: 0,
            new_transcriptions: 0,
            orphaned_audio: 0,
            completed_transcriptions: 0,
            errors: Vec::new(),
            synced_at: Local::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MetadataFile {
    audio_file: String,
    text_file: Option<String>,
    timestamp: String,
    language: String,
    duration: f64,
}

pub struct FileSystemSync {
    notes_dir: PathBuf,
}

impl FileSystemSync {
    pub fn new(notes_dir: PathBuf) -> Self {
        Self { notes_dir }
    }

    pub async fn sync_filesystem(&self) -> Result<SyncReport> {
        let mut report = SyncReport::default();
        
        let audio_files = self.scan_audio_files()?;
        report.total_files_found = audio_files.len();
        
        for audio_path in audio_files {
            match self.process_audio_file(&audio_path).await {
                Ok(status) => {
                    match status {
                        TranscriptionStatus::Complete => report.completed_transcriptions += 1,
                        TranscriptionStatus::Orphaned => report.orphaned_audio += 1,
                        _ => report.new_transcriptions += 1,
                    }
                }
                Err(e) => {
                    report.errors.push(format!("Error processing {}: {}", audio_path.display(), e));
                }
            }
        }
        
        Ok(report)
    }

    pub fn scan_audio_files(&self) -> Result<Vec<PathBuf>> {
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

    async fn process_audio_file(&self, audio_path: &Path) -> Result<TranscriptionStatus> {
        let relative_path = audio_path
            .strip_prefix(&self.notes_dir)
            .unwrap_or(audio_path)
            .to_string_lossy()
            .replace('\\', "/");
        
        let id = Self::extract_id_from_path(&relative_path);
        
        let text_path = audio_path.with_extension("txt");
        let json_path = audio_path.with_extension("json");
        
        let mut transcription = Transcription::new_orphan(relative_path.clone());
        transcription.id = id;
        
        if let Ok(metadata) = fs::metadata(audio_path) {
            transcription.file_size_bytes = metadata.len() as i64;
            
            if let Ok(created) = metadata.created() {
                if let Ok(duration) = created.duration_since(std::time::UNIX_EPOCH) {
                    let naive = NaiveDateTime::from_timestamp_opt(duration.as_secs() as i64, 0)
                        .unwrap_or_else(|| NaiveDateTime::from_timestamp_opt(0, 0).unwrap());
                    transcription.created_at = DateTime::from_naive_utc_and_offset(naive, *Local::now().offset());
                }
            }
        }
        
        if json_path.exists() {
            if let Ok(json_content) = fs::read_to_string(&json_path) {
                if let Ok(metadata) = serde_json::from_str::<MetadataFile>(&json_content) {
                    transcription.duration_seconds = metadata.duration;
                    transcription.language = metadata.language;
                }
            }
        }
        
        if text_path.exists() {
            let text_relative = text_path
                .strip_prefix(&self.notes_dir)
                .unwrap_or(&text_path)
                .to_string_lossy()
                .replace('\\', "/");
            
            transcription.text_path = Some(text_relative);
            
            if let Ok(text_content) = fs::read_to_string(&text_path) {
                transcription.transcription_text = Some(text_content);
                transcription.status = TranscriptionStatus::Complete;
                transcription.source = TranscriptionSource::Recording;
                transcription.transcribed_at = Some(Local::now());
            }
        } else {
            transcription.status = TranscriptionStatus::Orphaned;
            transcription.source = TranscriptionSource::Orphan;
        }
        
        let status = transcription.status.clone();
        Ok(status)
    }

    fn extract_id_from_path(path: &str) -> String {
        let path_buf = PathBuf::from(path);
        
        let date_part = path_buf
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .replace('-', "");
        
        let time_part = path_buf
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.split('-').next())
            .unwrap_or("");
        
        format!("{}-{}", date_part, time_part)
    }

    pub fn get_transcription_for_insert(&self, audio_path: &Path) -> Result<Transcription> {
        let relative_path = audio_path
            .strip_prefix(&self.notes_dir)
            .unwrap_or(audio_path)
            .to_string_lossy()
            .replace('\\', "/");
        
        let id = Self::extract_id_from_path(&relative_path);
        
        let text_path = audio_path.with_extension("txt");
        let json_path = audio_path.with_extension("json");
        
        let mut transcription = Transcription::new_orphan(relative_path.clone());
        transcription.id = id;
        
        if let Ok(metadata) = fs::metadata(audio_path) {
            transcription.file_size_bytes = metadata.len() as i64;
            
            if let Ok(created) = metadata.created() {
                if let Ok(duration) = created.duration_since(std::time::UNIX_EPOCH) {
                    let naive = NaiveDateTime::from_timestamp_opt(duration.as_secs() as i64, 0)
                        .unwrap_or_else(|| NaiveDateTime::from_timestamp_opt(0, 0).unwrap());
                    transcription.created_at = DateTime::from_naive_utc_and_offset(naive, *Local::now().offset());
                }
            }
        }
        
        if json_path.exists() {
            if let Ok(json_content) = fs::read_to_string(&json_path) {
                if let Ok(metadata) = serde_json::from_str::<MetadataFile>(&json_content) {
                    transcription.duration_seconds = metadata.duration;
                    transcription.language = metadata.language;
                }
            }
        }
        
        if text_path.exists() {
            let text_relative = text_path
                .strip_prefix(&self.notes_dir)
                .unwrap_or(&text_path)
                .to_string_lossy()
                .replace('\\', "/");
            
            transcription.text_path = Some(text_relative);
            
            if let Ok(text_content) = fs::read_to_string(&text_path) {
                transcription.transcription_text = Some(text_content);
                transcription.status = TranscriptionStatus::Complete;
                transcription.source = TranscriptionSource::Recording;
                transcription.transcribed_at = Some(Local::now());
            }
        } else {
            transcription.status = TranscriptionStatus::Orphaned;
            transcription.source = TranscriptionSource::Orphan;
        }
        
        Ok(transcription)
    }
}