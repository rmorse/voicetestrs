use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{AppHandle, State, Emitter};
use serde::{Deserialize, Serialize};

// Import our existing modules from the main project
use voicetextrs::core::audio::AudioRecorder;
use voicetextrs::core::transcription::Transcriber;

#[derive(Debug, Serialize, Deserialize)]
pub struct TranscriptionResult {
    pub text: String,
    pub audio_path: String,
}

pub struct AppState {
    pub recorder: Arc<Mutex<Option<AudioRecorder>>>,
    pub transcriber: Arc<Transcriber>,
    pub is_recording: Arc<Mutex<bool>>,
}

#[tauri::command]
pub async fn start_recording(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Check if already recording
    if *state.is_recording.lock().await {
        return Err("Already recording".to_string());
    }
    
    // Use the pre-initialized recorder
    let mut recorder_lock = state.recorder.lock().await;
    
    if let Some(recorder) = recorder_lock.as_mut() {
        // The stream is already initialized, just start recording
        recorder.start_recording()
            .map_err(|e| format!("Failed to start recording: {}", e))?;
        
        *state.is_recording.lock().await = true;
    } else {
        return Err("Recorder not initialized".to_string());
    }
    
    // Emit event to frontend
    app.emit("recording-status", serde_json::json!({
        "isRecording": true
    })).map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
pub async fn stop_recording(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<TranscriptionResult, String> {
    // Check if actually recording
    if !*state.is_recording.lock().await {
        return Err("Not recording".to_string());
    }
    
    // Set flag to false immediately to prevent double-stops
    *state.is_recording.lock().await = false;
    
    let mut recorder_lock = state.recorder.lock().await;
    
    // Keep the recorder alive (don't take it) - just stop recording
    let audio_path = if let Some(recorder) = recorder_lock.as_mut() {
        recorder.stop_recording()
            .map_err(|e| format!("Failed to stop recording: {}", e))?
    } else {
        return Err("Recorder not initialized".to_string());
    };
    
    // Emit status update
    app.emit("recording-status", serde_json::json!({
        "isRecording": false
    })).map_err(|e| e.to_string())?;
    
    // Transcribe the audio
    let transcription = state.transcriber.transcribe(&audio_path)
        .await
        .map_err(|e| format!("Transcription failed: {}", e))?;
    
    let result = TranscriptionResult {
        text: transcription.text.clone(),
        audio_path: audio_path.to_string_lossy().to_string(),
    };
    
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
    // Check if already recording
    if *state.is_recording.lock().await {
        return Err("Already recording".to_string());
    }
    
    // Start recording using the pre-initialized recorder
    let mut recorder_lock = state.recorder.lock().await;
    if let Some(recorder) = recorder_lock.as_mut() {
        recorder.start_recording()
            .map_err(|e| format!("Failed to start recording: {}", e))?;
        *state.is_recording.lock().await = true;
    } else {
        return Err("Recorder not initialized".to_string());
    }
    drop(recorder_lock); // Release the lock before sleeping
    
    // Emit event to frontend
    app.emit("recording-status", serde_json::json!({
        "isRecording": true
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
) -> Result<bool, String> {
    Ok(*state.is_recording.lock().await)
}