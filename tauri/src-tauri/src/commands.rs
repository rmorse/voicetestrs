use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{AppHandle, Manager, State};
use serde::{Deserialize, Serialize};

// Import our existing modules from the main project
use voicetextrs::core::audio::AudioRecorder;
use voicetextrs::core::transcription::WhisperTranscriber;

#[derive(Debug, Serialize, Deserialize)]
pub struct TranscriptionResult {
    pub text: String,
    pub audio_path: String,
}

pub struct AppState {
    pub recorder: Arc<Mutex<Option<AudioRecorder>>>,
    pub transcriber: Arc<WhisperTranscriber>,
}

#[tauri::command]
pub async fn start_recording(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut recorder_lock = state.recorder.lock().await;
    
    if recorder_lock.is_some() {
        return Err("Already recording".to_string());
    }

    let recorder = AudioRecorder::new()
        .map_err(|e| format!("Failed to create recorder: {}", e))?;
    
    recorder.start_recording()
        .map_err(|e| format!("Failed to start recording: {}", e))?;
    
    *recorder_lock = Some(recorder);
    
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
    let mut recorder_lock = state.recorder.lock().await;
    
    let recorder = recorder_lock.take()
        .ok_or_else(|| "Not recording".to_string())?;
    
    let audio_path = recorder.stop_recording()
        .map_err(|e| format!("Failed to stop recording: {}", e))?;
    
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
    // Start recording
    start_recording(app.clone(), state.clone()).await?;
    
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
    let recorder_lock = state.recorder.lock().await;
    Ok(recorder_lock.is_some())
}