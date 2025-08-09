use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig, SampleRate};
use hound::{WavSpec, WavWriter};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::thread;
use tracing::{info, error};
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
        })
    }
    
    /// Start recording audio
    pub fn start_recording(&mut self) -> Result<()> {
        // Clear buffer
        self.buffer.lock().unwrap().clear();
        
        // Set recording flag
        *self.is_recording.lock().unwrap() = true;
        
        // Clone for move into closure
        let buffer = Arc::clone(&self.buffer);
        let is_recording = Arc::clone(&self.is_recording);
        
        // Build input stream
        let stream = self.device.build_input_stream(
            &self.config,
            move |data: &[f32], _: &_| {
                if *is_recording.lock().unwrap() {
                    buffer.lock().unwrap().extend_from_slice(data);
                }
            },
            |err| error!("Audio stream error: {}", err),
            None,
        )?;
        
        stream.play()?;
        self.stream = Some(stream);
        
        info!("Recording started");
        Ok(())
    }
    
    /// Stop recording and save to WAV file
    pub fn stop_recording(&mut self) -> Result<PathBuf> {
        // Stop recording
        *self.is_recording.lock().unwrap() = false;
        
        // Stop stream
        if let Some(stream) = self.stream.take() {
            stream.pause()?;
            drop(stream);
        }
        
        info!("Recording stopped");
        
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
        let date_dir = PathBuf::from("notes")
            .join(timestamp.format("%Y").to_string())
            .join(timestamp.format("%Y-%m-%d").to_string());
        
        std::fs::create_dir_all(&date_dir)?;
        
        let filename = format!("{}-voice-note.wav", 
            timestamp.format("%H%M%S"));
        
        Ok(date_dir.join(filename))
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