use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig, SampleRate};
use hound::{WavSpec, WavWriter};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::thread;
use tracing::{info, error, warn};
use chrono::Local;

const SAMPLE_RATE: u32 = 16000;  // Optimal for Whisper
const CHANNELS: u16 = 1;         // Mono
const BITS_PER_SAMPLE: u16 = 16;

/// Audio recorder using CPAL for cross-platform audio capture
pub struct AudioRecorder {
    device: Device,
    config: StreamConfig,
    buffer: Arc<Mutex<Vec<f32>>>,
    stream: Option<Stream>,
    is_recording: Arc<Mutex<bool>>,
    is_initialized: bool,
}

impl AudioRecorder {
    /// Create a new audio recorder with default input device
    pub fn new() -> Result<Self> {
        let host = cpal::default_host();
        let device = host.default_input_device()
            .ok_or_else(|| anyhow!("No input device available"))?;
        
        info!("Using audio device: {}", device.name()?);
        
        let config = StreamConfig {
            channels: CHANNELS,
            sample_rate: SampleRate(SAMPLE_RATE),
            buffer_size: cpal::BufferSize::Default,
        };
        
        Ok(Self {
            device,
            config,
            buffer: Arc::new(Mutex::new(Vec::new())),
            stream: None,
            is_recording: Arc::new(Mutex::new(false)),
            is_initialized: false,
        })
    }
    
    /// Create recorder with specific device
    pub fn with_device(device_name: &str) -> Result<Self> {
        let host = cpal::default_host();
        
        // Find device by name
        let device = host.input_devices()?
            .find(|d| d.name().unwrap_or_default() == device_name)
            .ok_or_else(|| anyhow!("Device '{}' not found", device_name))?;
        
        info!("Using specified audio device: {}", device.name()?);
        
        let config = StreamConfig {
            channels: CHANNELS,
            sample_rate: SampleRate(SAMPLE_RATE),
            buffer_size: cpal::BufferSize::Default,
        };
        
        Ok(Self {
            device,
            config,
            buffer: Arc::new(Mutex::new(Vec::new())),
            stream: None,
            is_recording: Arc::new(Mutex::new(false)),
            is_initialized: false,
        })
    }
    
    /// Initialize the audio stream (pre-warm the microphone)
    pub fn initialize_stream(&mut self) -> Result<()> {
        if self.is_initialized {
            return Ok(());
        }
        
        info!("Initializing audio stream...");
        
        // Clone for move into closure
        let buffer = Arc::clone(&self.buffer);
        let is_recording = Arc::clone(&self.is_recording);
        
        // Build input stream that runs continuously
        let stream = self.device.build_input_stream(
            &self.config,
            move |data: &[f32], _: &_| {
                // Only buffer data when actually recording
                if *is_recording.lock().unwrap() {
                    buffer.lock().unwrap().extend_from_slice(data);
                }
                // Otherwise, data is discarded
            },
            |err| error!("Audio stream error: {}", err),
            None,
        )?;
        
        stream.play()?;
        self.stream = Some(stream);
        self.is_initialized = true;
        
        info!("Audio stream initialized and running (not recording yet)");
        Ok(())
    }
    
    /// Start recording audio (with pre-initialized stream)
    pub fn start_recording(&mut self) -> Result<()> {
        // Initialize stream if not already done
        if !self.is_initialized {
            self.initialize_stream()?;
        }
        
        // Clear buffer for new recording
        self.buffer.lock().unwrap().clear();
        
        // Set recording flag - this makes the stream callback start buffering
        *self.is_recording.lock().unwrap() = true;
        
        info!("Recording started (using pre-initialized stream)");
        Ok(())
    }
    
    /// Stop recording and save to WAV file (keeps stream running)
    pub fn stop_recording(&mut self) -> Result<PathBuf> {
        // Stop recording (but keep stream running)
        *self.is_recording.lock().unwrap() = false;
        
        info!("Recording stopped (stream still running for next recording)");
        
        // Generate output path
        let output_path = self.generate_output_path()?;
        
        // Save to WAV
        self.save_to_wav(&output_path)?;
        
        Ok(output_path)
    }
    
    /// Save recorded audio to WAV file
    fn save_to_wav(&self, path: &Path) -> Result<()> {
        let spec = WavSpec {
            channels: CHANNELS,
            sample_rate: SAMPLE_RATE,
            bits_per_sample: BITS_PER_SAMPLE,
            sample_format: hound::SampleFormat::Int,
        };
        
        let mut writer = WavWriter::create(path, spec)?;
        let buffer = self.buffer.lock().unwrap();
        
        info!("Saving {} samples to {}", buffer.len(), path.display());
        
        // Convert f32 samples to i16
        for &sample in buffer.iter() {
            let amplitude = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            writer.write_sample(amplitude)?;
        }
        
        writer.finalize()?;
        info!("Audio saved to: {}", path.display());
        
        Ok(())
    }
    
    /// Generate output path with timestamp
    fn generate_output_path(&self) -> Result<PathBuf> {
        let timestamp = Local::now();
        
        // Find the project root by looking for whisper directory
        let project_root = Self::find_project_root()?;
        
        let date_dir = project_root
            .join("notes")
            .join(timestamp.format("%Y").to_string())
            .join(timestamp.format("%Y-%m-%d").to_string());
        
        std::fs::create_dir_all(&date_dir)?;
        
        let filename = format!("{}-voice-note.wav", 
            timestamp.format("%H%M%S"));
        
        Ok(date_dir.join(filename))
    }
    
    /// Find the project root directory by looking for the whisper folder
    fn find_project_root() -> Result<PathBuf> {
        // Try current directory and parent directories
        let possible_roots = vec![
            PathBuf::from("."),                    // Current dir (when run from project root)
            PathBuf::from("../.."),                // When run from tauri/src-tauri
            PathBuf::from("../../.."),              // When run from deeper directories
        ];
        
        for root in possible_roots {
            let whisper_path = root.join("whisper");
            if whisper_path.exists() {
                return Ok(root.canonicalize()?);
            }
        }
        
        // Fallback to current directory if whisper not found
        warn!("Could not find project root with whisper directory, using current directory");
        Ok(PathBuf::from(".").canonicalize()?)
    }
    
    /// Get current recording duration
    pub fn get_duration(&self) -> Duration {
        let buffer = self.buffer.lock().unwrap();
        let samples = buffer.len() as u64;
        let seconds = samples / SAMPLE_RATE as u64;
        Duration::from_secs(seconds)
    }
    
    /// Check if currently recording
    pub fn is_recording(&self) -> bool {
        *self.is_recording.lock().unwrap()
    }
}

/// List all available audio input devices
pub fn list_audio_devices() -> Result<()> {
    let host = cpal::default_host();
    
    println!("\nAvailable audio input devices:");
    println!("==============================");
    
    let default_device = host.default_input_device();
    let default_name = default_device
        .as_ref()
        .and_then(|d| d.name().ok())
        .unwrap_or_else(|| "None".to_string());
    
    for (index, device) in host.input_devices()?.enumerate() {
        let name = device.name()?;
        let is_default = name == default_name;
        
        // Get supported configs
        let configs: Vec<_> = device.supported_input_configs()?.collect();
        let sample_rates: Vec<u32> = configs.iter()
            .map(|c| c.max_sample_rate().0)
            .collect();
        
        println!("{:2}. {} {}", 
            index + 1, 
            name,
            if is_default { "(DEFAULT)" } else { "" }
        );
        println!("    Sample rates: {:?}", sample_rates);
        println!("    Channels: {}", 
            configs.first().map(|c| c.channels()).unwrap_or(0));
    }
    
    Ok(())
}

/// Test recording for specified duration
pub fn test_recording(duration_secs: u64, device_name: Option<String>) -> Result<PathBuf> {
    info!("Starting {} second recording test", duration_secs);
    
    let mut recorder = match device_name {
        Some(name) => AudioRecorder::with_device(&name)?,
        None => AudioRecorder::new()?,
    };
    
    recorder.start_recording()?;
    
    // Show progress
    for i in 1..=duration_secs {
        thread::sleep(Duration::from_secs(1));
        print!("Recording... {}/{} seconds\r", i, duration_secs);
        std::io::Write::flush(&mut std::io::stdout())?;
    }
    println!();
    
    let output_path = recorder.stop_recording()?;
    
    // Print file info
    let metadata = std::fs::metadata(&output_path)?;
    println!("\nRecording complete!");
    println!("File: {}", output_path.display());
    println!("Size: {:.2} MB", metadata.len() as f64 / 1_048_576.0);
    println!("Duration: {} seconds", duration_secs);
    
    Ok(output_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_audio_device_listing() {
        // This should not panic
        let _ = list_audio_devices();
    }
    
    #[test]
    fn test_recorder_creation() {
        // May fail on CI without audio devices
        let _ = AudioRecorder::new();
    }
}