use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{AppHandle, State, Emitter};
use serde::{Deserialize, Serialize};

// Import our existing modules from the main project
use voicetextrs::core::audio::AudioRecorder;
use voicetextrs::core::transcription::Transcriber;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecordingState {
    Idle,
    Recording,
    Processing,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TranscriptionResult {
    pub text: String,
    pub audio_path: String,
}

pub struct AppState {
    pub recorder: Arc<Mutex<Option<AudioRecorder>>>,
    pub transcriber: Arc<Transcriber>,
    pub state: Arc<Mutex<RecordingState>>,
}

#[tauri::command]
pub async fn start_recording(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Check current state - must be Idle to start recording
    let current_state = *state.state.lock().await;
    if current_state != RecordingState::Idle {
        return Err(format!("Cannot start recording in {:?} state", current_state));
    }
    
    // Use the pre-initialized recorder
    let mut recorder_lock = state.recorder.lock().await;
    
    if let Some(recorder) = recorder_lock.as_mut() {
        // The stream is already initialized, just start recording
        recorder.start_recording()
            .map_err(|e| format!("Failed to start recording: {}", e))?;
        
        // Update state to Recording
        *state.state.lock().await = RecordingState::Recording;
    } else {
        return Err("Recorder not initialized".to_string());
    }
    
    // Emit state change event to frontend
    app.emit("state-changed", serde_json::json!({
        "state": "recording"
    })).map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
pub async fn stop_recording(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<TranscriptionResult, String> {
    // Check current state - must be Recording to stop
    let current_state = *state.state.lock().await;
    if current_state != RecordingState::Recording {
        return Err(format!("Cannot stop recording in {:?} state", current_state));
    }
    
    // Set state to Processing immediately
    *state.state.lock().await = RecordingState::Processing;
    
    // Emit state change to show processing UI
    app.emit("state-changed", serde_json::json!({
        "state": "processing"
    })).map_err(|e| e.to_string())?;
    
    let mut recorder_lock = state.recorder.lock().await;
    
    // Keep the recorder alive (don't take it) - just stop recording
    let audio_path = if let Some(recorder) = recorder_lock.as_mut() {
        recorder.stop_recording()
            .map_err(|e| format!("Failed to stop recording: {}", e))?
    } else {
        // If error, set state back to Idle
        *state.state.lock().await = RecordingState::Idle;
        app.emit("state-changed", serde_json::json!({
            "state": "idle"
        })).ok();
        return Err("Recorder not initialized".to_string());
    };
    
    // Release the recorder lock before transcribing
    drop(recorder_lock);
    
    // Transcribe the audio
    let transcription = match state.transcriber.transcribe(&audio_path).await {
        Ok(t) => t,
        Err(e) => {
            // If transcription fails, set state back to Idle
            *state.state.lock().await = RecordingState::Idle;
            app.emit("state-changed", serde_json::json!({
                "state": "idle"
            })).ok();
            return Err(format!("Transcription failed: {}", e));
        }
    };
    
    let result = TranscriptionResult {
        text: transcription.text.clone(),
        audio_path: audio_path.to_string_lossy().to_string(),
    };
    
    // Set state back to Idle after successful transcription
    *state.state.lock().await = RecordingState::Idle;
    
    // Emit state change back to idle
    app.emit("state-changed", serde_json::json!({
        "state": "idle"
    })).map_err(|e| e.to_string())?;
    
    // Emit transcription complete event
    app.emit("transcription-complete", &result)
        .map_err(|e| e.to_string())?;
    
    Ok(result)
}

#[tauri::command]
pub async fn quick_note(
    app: AppHandle,
    state: State<'_, AppState>,
    duration: u64,
) -> Result<TranscriptionResult, String> {
    // Check current state - must be Idle to start
    let current_state = *state.state.lock().await;
    if current_state != RecordingState::Idle {
        return Err(format!("Cannot start quick note in {:?} state", current_state));
    }
    
    // Start recording using the pre-initialized recorder
    let mut recorder_lock = state.recorder.lock().await;
    if let Some(recorder) = recorder_lock.as_mut() {
        recorder.start_recording()
            .map_err(|e| format!("Failed to start recording: {}", e))?;
        *state.state.lock().await = RecordingState::Recording;
    } else {
        return Err("Recorder not initialized".to_string());
    }
    drop(recorder_lock); // Release the lock before sleeping
    
    // Emit state change event
    app.emit("state-changed", serde_json::json!({
        "state": "recording"
    })).map_err(|e| e.to_string())?;
    
    // Wait for the specified duration
    tokio::time::sleep(tokio::time::Duration::from_secs(duration)).await;
    
    // Stop and transcribe
    stop_recording(app, state).await
}

#[tauri::command]
pub async fn transcribe_file(
    state: State<'_, AppState>,
    file_path: String,
) -> Result<TranscriptionResult, String> {
    let path = std::path::PathBuf::from(&file_path);
    
    if !path.exists() {
        return Err("File not found".to_string());
    }
    
    let transcription = state.transcriber.transcribe(&path)
        .await
        .map_err(|e| format!("Transcription failed: {}", e))?;
    
    Ok(TranscriptionResult {
        text: transcription.text,
        audio_path: file_path,
    })
}

#[tauri::command]
pub async fn get_recording_status(
    state: State<'_, AppState>,
) -> Result<RecordingState, String> {
    Ok(*state.state.lock().await)
}