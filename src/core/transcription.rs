use anyhow::{Result, Context, bail};
use std::path::{Path, PathBuf};
use std::process::Command;
use serde::Deserialize;
use tracing::{info, warn};

pub struct Transcriber {
    whisper_path: PathBuf,
    model_path: PathBuf,
    model_type: String,
}

impl Transcriber {
    pub fn new() -> Result<Self> {
        // Try to find whisper in multiple locations
        let possible_paths = vec![
            PathBuf::from("whisper/Release/whisper-cli.exe"),
            PathBuf::from("../../whisper/Release/whisper-cli.exe"),
            PathBuf::from("../../../whisper/Release/whisper-cli.exe"),
        ];
        
        let whisper_path = possible_paths
            .iter()
            .find(|p| p.exists())
            .cloned()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Whisper binary not found. Tried paths: {:?}",
                    possible_paths
                )
            })?;
        
        info!("Found whisper binary at: {:?}", whisper_path);
        
        // Default to base.en model
        let model_type = "medium.en".to_string();
        
        // Try to find the model in the same relative location as whisper
        let whisper_dir = whisper_path.parent()
            .and_then(|p| p.parent())
            .ok_or_else(|| anyhow::anyhow!("Invalid whisper path"))?;
        let model_path = whisper_dir.join(format!("models/ggml-{}.bin", model_type));
        
        if !model_path.exists() {
            warn!("Model {:?} not found. Will download on first use.", model_path);
        }
        
        Ok(Self {
            whisper_path,
            model_path,
            model_type,
        })
    }
    
    pub fn with_model(model_type: &str) -> Result<Self> {
        // Try to find whisper in multiple locations
        let possible_paths = vec![
            PathBuf::from("whisper/Release/whisper-cli.exe"),
            PathBuf::from("../../whisper/Release/whisper-cli.exe"),
            PathBuf::from("../../../whisper/Release/whisper-cli.exe"),
        ];
        
        let whisper_path = possible_paths
            .iter()
            .find(|p| p.exists())
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Whisper binary not found"))?;
        
        let whisper_dir = whisper_path.parent()
            .and_then(|p| p.parent())
            .ok_or_else(|| anyhow::anyhow!("Invalid whisper path"))?;
        let model_path = whisper_dir.join(format!("models/ggml-{}.bin", model_type));
        
        Ok(Self {
            whisper_path,
            model_path,
            model_type: model_type.to_string(),
        })
    }
    
    pub async fn transcribe(&self, audio_path: &Path) -> Result<TranscriptionResult> {
        info!("Transcribing audio file: {:?}", audio_path);
        
        if !audio_path.exists() {
            bail!("Audio file not found: {:?}", audio_path);
        }
        
        // Build whisper command
        let output = Command::new(&self.whisper_path)
            .arg("--model").arg(&self.model_path)
            .arg("--file").arg(audio_path)
            .arg("--output-json")
            .arg("--no-timestamps")
            .arg("--language").arg("en")
            .arg("--threads").arg("4")
            .arg("--no-prints")  // Suppress progress output
            .output()
            .context("Failed to execute whisper")?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("Whisper failed: {}", stderr);
        }
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        // Parse the JSON output
        let json_path = audio_path.with_extension("json");
        if json_path.exists() {
            let json_content = std::fs::read_to_string(&json_path)?;
            let whisper_output: WhisperOutput = serde_json::from_str(&json_content)?;
            
            // Clean up JSON file
            std::fs::remove_file(json_path).ok();
            
            // Calculate duration before consuming segments
            let duration = whisper_output.segments.last().map(|s| s.end).unwrap_or(0.0);
            
            Ok(TranscriptionResult {
                text: whisper_output.text.trim().to_string(),
                segments: whisper_output.segments.into_iter().map(|s| TranscriptionSegment {
                    start: s.start,
                    end: s.end,
                    text: s.text.trim().to_string(),
                    confidence: 0.95, // Whisper doesn't provide confidence scores
                }).collect(),
                language: whisper_output.language.unwrap_or_else(|| "en".to_string()),
                duration,
            })
        } else {
            // Fallback to parsing text output
            Ok(TranscriptionResult {
                text: stdout.trim().to_string(),
                segments: vec![],
                language: "en".to_string(),
                duration: 0.0,
            })
        }
    }
    
    pub async fn download_model(&self) -> Result<()> {
        info!("Downloading model: {}", self.model_type);
        
        // Create models directory
        std::fs::create_dir_all("whisper/models")?;
        
        // Run whisper with --model-download flag
        let output = Command::new(&self.whisper_path)
            .arg("--model").arg(&self.model_type)
            .arg("--model-download")
            .output()
            .context("Failed to download model")?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("Model download failed: {}", stderr);
        }
        
        info!("Model downloaded successfully");
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct TranscriptionResult {
    pub text: String,
    pub segments: Vec<TranscriptionSegment>,
    pub language: String,
    pub duration: f32,
}

#[derive(Debug, Clone)]
pub struct TranscriptionSegment {
    pub start: f32,
    pub end: f32,
    pub text: String,
    pub confidence: f32,
}

// Whisper JSON output structures
#[derive(Debug, Deserialize)]
struct WhisperOutput {
    text: String,
    segments: Vec<WhisperSegment>,
    language: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WhisperSegment {
    start: f32,
    end: f32,
    text: String,
}