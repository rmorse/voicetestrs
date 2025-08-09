use anyhow::Result;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use std::path::PathBuf;
use tracing::{info, warn, error};

use crate::core::{audio::AudioRecorder, transcription::Transcriber};
use crate::platform::{
    tray::{TrayManager, TrayCommand},
    hotkeys::{HotkeyManager, HotkeyEvent},
    notifications,
};

pub struct App {
    tray_manager: TrayManager,
    hotkey_manager: HotkeyManager,
    audio_recorder: Arc<Mutex<Option<AudioRecorder>>>,
    transcriber: Arc<Transcriber>,
    is_recording: Arc<AtomicBool>,
    recording_start: Arc<Mutex<Option<Instant>>>,
    shutdown: Arc<AtomicBool>,
    enabled: bool,
}

impl App {
    pub fn new() -> Result<Self> {
        let tray_manager = TrayManager::new()?;
        let hotkey_manager = HotkeyManager::new()?;
        let transcriber = Arc::new(Transcriber::new()?);
        
        Ok(Self {
            tray_manager,
            hotkey_manager,
            audio_recorder: Arc::new(Mutex::new(None)),
            transcriber,
            is_recording: Arc::new(AtomicBool::new(false)),
            recording_start: Arc::new(Mutex::new(None)),
            shutdown: Arc::new(AtomicBool::new(false)),
            enabled: true,
        })
    }
    
    pub async fn run(&mut self) -> Result<()> {
        info!("Starting VoiceTextRS in background mode");
        
        // Initialize components
        self.tray_manager.init()?;
        self.hotkey_manager.register_defaults()?;
        
        // Show startup notification
        notifications::show_notification(
            "VoiceTextRS Started",
            "Press Ctrl+Shift+R to start recording"
        )?;
        
        // Main event loop
        while !self.shutdown.load(Ordering::Relaxed) {
            // Handle tray events
            if let Some(command) = self.tray_manager.handle_events()? {
                self.handle_tray_command(command).await?;
            }
            
            // Handle hotkey events
            if let Some(event) = self.hotkey_manager.handle_events()? {
                self.handle_hotkey_event(event).await?;
            }
            
            // Small delay to prevent busy-waiting
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        
        info!("Shutting down VoiceTextRS");
        Ok(())
    }
    
    async fn handle_tray_command(&mut self, command: TrayCommand) -> Result<()> {
        match command {
            TrayCommand::StartRecording => {
                self.start_recording().await?;
            }
            TrayCommand::StopRecording => {
                self.stop_recording().await?;
            }
            TrayCommand::ShowWindow => {
                info!("Show window requested (not implemented yet)");
            }
            TrayCommand::OpenSettings => {
                info!("Open settings requested (not implemented yet)");
            }
            TrayCommand::ToggleHotkeys => {
                self.enabled = !self.enabled;
                self.hotkey_manager.set_enabled(self.enabled);
                notifications::show_notification(
                    "Hotkeys",
                    if self.enabled { "Hotkeys enabled" } else { "Hotkeys disabled" }
                )?;
            }
            TrayCommand::Exit => {
                self.shutdown.store(true, Ordering::Relaxed);
            }
        }
        Ok(())
    }
    
    async fn handle_hotkey_event(&mut self, event: HotkeyEvent) -> Result<()> {
        match event {
            HotkeyEvent::RecordingToggle => {
                if self.is_recording.load(Ordering::Relaxed) {
                    self.stop_recording().await?;
                } else {
                    self.start_recording().await?;
                }
            }
            HotkeyEvent::QuickNote => {
                info!("Quick note requested");
                // Could implement a short recording with immediate transcription
                self.quick_note().await?;
            }
            HotkeyEvent::ShowWindow => {
                info!("Show window via hotkey");
            }
        }
        Ok(())
    }
    
    async fn start_recording(&mut self) -> Result<()> {
        if self.is_recording.load(Ordering::Relaxed) {
            warn!("Already recording");
            return Ok(());
        }
        
        info!("Starting recording");
        
        // Create new recorder
        let mut recorder = AudioRecorder::new()?;
        recorder.start_recording()?;
        
        // Store recorder and update state
        *self.audio_recorder.lock().unwrap() = Some(recorder);
        *self.recording_start.lock().unwrap() = Some(Instant::now());
        self.is_recording.store(true, Ordering::Relaxed);
        
        // Update UI
        self.tray_manager.set_recording(true)?;
        notifications::show_recording_started()?;
        
        Ok(())
    }
    
    async fn stop_recording(&mut self) -> Result<()> {
        if !self.is_recording.load(Ordering::Relaxed) {
            warn!("Not currently recording");
            return Ok(());
        }
        
        info!("Stopping recording");
        
        // Calculate duration
        let duration = self.recording_start.lock().unwrap()
            .take()
            .map(|start| start.elapsed().as_secs())
            .unwrap_or(0);
        
        // Stop recording and get path
        let audio_path = {
            let mut recorder_lock = self.audio_recorder.lock().unwrap();
            if let Some(mut recorder) = recorder_lock.take() {
                recorder.stop_recording()?
            } else {
                return Err(anyhow::anyhow!("No active recorder"));
            }
        };
        
        // Update state
        self.is_recording.store(false, Ordering::Relaxed);
        self.tray_manager.set_recording(false)?;
        
        // Show notification
        notifications::show_recording_stopped(duration)?;
        
        // Transcribe in background
        let transcriber = self.transcriber.clone();
        let audio_path_clone = audio_path.clone();
        
        tokio::spawn(async move {
            match transcriber.transcribe(&audio_path_clone).await {
                Ok(result) => {
                    info!("Transcription complete: {} chars", result.text.len());
                    
                    // Save transcription
                    let text_path = audio_path_clone.with_extension("txt");
                    if let Err(e) = std::fs::write(&text_path, &result.text) {
                        error!("Failed to save transcription: {}", e);
                    }
                    
                    // Show notification
                    if let Err(e) = notifications::show_transcription_complete(&result.text) {
                        error!("Failed to show notification: {}", e);
                    }
                }
                Err(e) => {
                    error!("Transcription failed: {}", e);
                    if let Err(e) = notifications::show_error(&format!("Transcription failed: {}", e)) {
                        error!("Failed to show error notification: {}", e);
                    }
                }
            }
        });
        
        Ok(())
    }
    
    async fn quick_note(&mut self) -> Result<()> {
        // Start recording for 10 seconds max
        self.start_recording().await?;
        
        info!("Quick note: Will auto-stop after 10 seconds");
        
        // Note: In a real implementation, we'd use a channel or shared state
        // to trigger the stop from the spawned task
        
        Ok(())
    }
}

// Function to run the app in CLI mode (for existing commands)
pub async fn run_cli_command(
    record: Option<u64>,
    transcribe: Option<String>,
    test: Option<u64>,
    list_devices: bool,
    device: Option<String>,
) -> Result<()> {
    use crate::core::audio;
    
    if list_devices {
        audio::list_audio_devices()?;
        return Ok(());
    }
    
    if let Some(duration) = test {
        info!("Testing audio recording for {} seconds", duration);
        audio::test_recording(duration, device)?;
        return Ok(());
    }
    
    if let Some(audio_file) = transcribe {
        info!("Transcribing audio file: {}", audio_file);
        let transcriber = Transcriber::new()?;
        let result = transcriber.transcribe(&PathBuf::from(audio_file)).await?;
        println!("\n=== Transcription ===");
        println!("{}", result.text);
        println!("====================\n");
        info!("Language: {}, Duration: {:.1}s", result.language, result.duration);
        return Ok(());
    }
    
    if let Some(duration) = record {
        info!("Recording and transcribing for {} seconds", duration);
        
        // Record audio
        let audio_path = audio::test_recording(duration, device)?;
        info!("Audio saved to: {:?}", audio_path);
        
        // Transcribe the recording
        let transcriber = Transcriber::new()?;
        let result = transcriber.transcribe(&audio_path).await?;
        
        println!("\n=== Transcription ===");
        println!("{}", result.text);
        println!("====================\n");
        
        // Save transcription to text file
        let text_path = audio_path.with_extension("txt");
        std::fs::write(&text_path, &result.text)?;
        info!("Transcription saved to: {:?}", text_path);
        
        return Ok(());
    }
    
    Ok(())
}