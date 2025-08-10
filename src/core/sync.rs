use anyhow::Result;
use chrono::{DateTime, Local, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
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
        // Whisper creates .wav.json files, not .json files
        let json_path = PathBuf::from(format!("{}.json", audio_path.display()));
        
        let mut transcription = Transcription::new_orphan(relative_path.clone());
        transcription.id = id;
        
        // Get file size
        if let Ok(metadata) = fs::metadata(audio_path) {
            transcription.file_size_bytes = metadata.len() as i64;
        }
        
        // Use our robust timestamp extraction
        transcription.created_at = Self::extract_file_timestamp(audio_path);
        
        if json_path.exists() {
            if let Ok(json_content) = fs::read_to_string(&json_path) {
                if let Ok(metadata) = serde_json::from_str::<MetadataFile>(&json_content) {
                    transcription.duration_seconds = metadata.duration;
                    transcription.language = metadata.language;
                }
            }
        }
        
        // Check for transcription text file
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
        } else if json_path.exists() {
            // No text file, but check if JSON indicates it was processed (e.g., BLANK_AUDIO)
            if let Ok(json_content) = fs::read_to_string(&json_path) {
                if json_content.contains("[BLANK_AUDIO]") {
                    // This audio was transcribed but had no speech
                    transcription.transcription_text = Some("[BLANK_AUDIO]".to_string());
                    transcription.status = TranscriptionStatus::Complete;
                    transcription.source = TranscriptionSource::Recording;
                    transcription.transcribed_at = Some(Local::now());
                } else {
                    // Has JSON but not processed yet
                    transcription.status = TranscriptionStatus::Orphaned;
                    transcription.source = TranscriptionSource::Orphan;
                }
            } else {
                transcription.status = TranscriptionStatus::Orphaned;
                transcription.source = TranscriptionSource::Orphan;
            }
        } else {
            // No text file and no JSON file - truly orphaned
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
    
    /// Extract the best available timestamp from a file
    /// Priority: 1) Modified time, 2) Created time, 3) Current time
    /// This is reusable for import functionality later
    pub fn extract_file_timestamp(path: &Path) -> DateTime<Local> {
        if let Ok(metadata) = fs::metadata(path) {
            // First try modified time (most reliable when files are moved)
            if let Ok(modified) = metadata.modified() {
                if let Some(datetime) = Self::system_time_to_datetime(modified) {
                    return datetime;
                }
            }
            
            // Fall back to created time if modified is not available
            if let Ok(created) = metadata.created() {
                if let Some(datetime) = Self::system_time_to_datetime(created) {
                    return datetime;
                }
            }
        }
        
        // Last resort: try to parse from filename if it contains a timestamp
        if let Some(datetime) = Self::parse_timestamp_from_filename(path) {
            return datetime;
        }
        
        // Ultimate fallback: current time
        Local::now()
    }
    
    /// Convert SystemTime to DateTime<Local>
    fn system_time_to_datetime(sys_time: SystemTime) -> Option<DateTime<Local>> {
        if let Ok(duration) = sys_time.duration_since(std::time::UNIX_EPOCH) {
            let timestamp = duration.as_secs() as i64;
            if let Some(naive) = NaiveDateTime::from_timestamp_opt(timestamp, 0) {
                return Some(DateTime::from_naive_utc_and_offset(naive, *Local::now().offset()));
            }
        }
        None
    }
    
    /// Try to parse timestamp from filename (e.g., "2025-08-10/141201-voice-note.wav")
    fn parse_timestamp_from_filename(path: &Path) -> Option<DateTime<Local>> {
        // Get the parent directory name (should be date like 2025-08-10)
        let date_str = path.parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())?;
        
        // Get the filename time part (HHMMSS from "141201-voice-note.wav")
        let time_str = path.file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.split('-').next())?;
        
        // Parse date components
        let date_parts: Vec<&str> = date_str.split('-').collect();
        if date_parts.len() != 3 {
            return None;
        }
        
        let year: i32 = date_parts[0].parse().ok()?;
        let month: u32 = date_parts[1].parse().ok()?;
        let day: u32 = date_parts[2].parse().ok()?;
        
        // Parse time components (HHMMSS)
        if time_str.len() != 6 {
            return None;
        }
        
        let hour: u32 = time_str[0..2].parse().ok()?;
        let minute: u32 = time_str[2..4].parse().ok()?;
        let second: u32 = time_str[4..6].parse().ok()?;
        
        // Create NaiveDateTime and convert to Local
        let naive = NaiveDateTime::parse_from_str(
            &format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", 
                     year, month, day, hour, minute, second),
            "%Y-%m-%d %H:%M:%S"
        ).ok()?;
        
        Some(DateTime::from_naive_utc_and_offset(naive, *Local::now().offset()))
    }

    pub fn get_transcription_for_insert(&self, audio_path: &Path) -> Result<Transcription> {
        let relative_path = audio_path
            .strip_prefix(&self.notes_dir)
            .unwrap_or(audio_path)
            .to_string_lossy()
            .replace('\\', "/");
        
        let id = Self::extract_id_from_path(&relative_path);
        
        let text_path = audio_path.with_extension("txt");
        // Whisper creates .wav.json files, not .json files
        let json_path = PathBuf::from(format!("{}.json", audio_path.display()));
        
        let mut transcription = Transcription::new_orphan(relative_path.clone());
        transcription.id = id;
        
        // Get file size
        if let Ok(metadata) = fs::metadata(audio_path) {
            transcription.file_size_bytes = metadata.len() as i64;
        }
        
        // Use our robust timestamp extraction
        transcription.created_at = Self::extract_file_timestamp(audio_path);
        
        if json_path.exists() {
            if let Ok(json_content) = fs::read_to_string(&json_path) {
                if let Ok(metadata) = serde_json::from_str::<MetadataFile>(&json_content) {
                    transcription.duration_seconds = metadata.duration;
                    transcription.language = metadata.language;
                }
            }
        }
        
        // Check for transcription text file
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
        } else if json_path.exists() {
            // No text file, but check if JSON indicates it was processed (e.g., BLANK_AUDIO)
            if let Ok(json_content) = fs::read_to_string(&json_path) {
                if json_content.contains("[BLANK_AUDIO]") {
                    // This audio was transcribed but had no speech
                    transcription.transcription_text = Some("[BLANK_AUDIO]".to_string());
                    transcription.status = TranscriptionStatus::Complete;
                    transcription.source = TranscriptionSource::Recording;
                    transcription.transcribed_at = Some(Local::now());
                } else {
                    // Has JSON but not processed yet
                    transcription.status = TranscriptionStatus::Orphaned;
                    transcription.source = TranscriptionSource::Orphan;
                }
            } else {
                transcription.status = TranscriptionStatus::Orphaned;
                transcription.source = TranscriptionSource::Orphan;
            }
        } else {
            // No text file and no JSON file - truly orphaned
            transcription.status = TranscriptionStatus::Orphaned;
            transcription.source = TranscriptionSource::Orphan;
        }
        
        Ok(transcription)
    }
}