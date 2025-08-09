use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub audio: AudioConfig,
    pub recording: RecordingConfig,
    pub hotkeys: HotkeyConfig,
    pub whisper: WhisperConfig,
    pub storage: StorageConfig,
    pub ui: UiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub buffer_size: usize,
    pub device: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingConfig {
    pub mode: RecordingMode,
    pub max_duration_seconds: u64,
    pub auto_stop_silence_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecordingMode {
    PushToTalk,
    Toggle,
    VoiceActivityDetection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    pub record: String,
    pub stop: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhisperConfig {
    pub model: String,
    pub language: String,
    pub threads: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub notes_directory: PathBuf,
    pub keep_audio_files: bool,
    pub auto_archive_days: u32,
    pub compression: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub theme: String,
    pub minimize_to_tray: bool,
    pub show_notifications: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            audio: AudioConfig {
                sample_rate: 16000,
                channels: 1,
                buffer_size: 1024,
                device: None,
            },
            recording: RecordingConfig {
                mode: RecordingMode::PushToTalk,
                max_duration_seconds: 300,
                auto_stop_silence_ms: 2000,
            },
            hotkeys: HotkeyConfig {
                record: "Ctrl+Space".to_string(),
                stop: "Escape".to_string(),
            },
            whisper: WhisperConfig {
                model: "base".to_string(),
                language: "en".to_string(),
                threads: 4,
            },
            storage: StorageConfig {
                notes_directory: PathBuf::from("./notes"),
                keep_audio_files: true,
                auto_archive_days: 30,
                compression: false,
            },
            ui: UiConfig {
                theme: "dark".to_string(),
                minimize_to_tray: true,
                show_notifications: true,
            },
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        // TODO: Load from config.toml
        Ok(Self::default())
    }
    
    pub fn save(&self) -> Result<()> {
        // TODO: Save to config.toml
        Ok(())
    }
}