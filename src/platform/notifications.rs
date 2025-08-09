use anyhow::{Result, Context};
use notify_rust::{Notification, Timeout};
use tracing::info;

pub fn show_notification(title: &str, message: &str) -> Result<()> {
    Notification::new()
        .summary(title)
        .body(message)
        .appname("VoiceTextRS")
        .timeout(Timeout::Milliseconds(5000))
        .show()
        .context("Failed to show notification")?;
    
    info!("Notification shown: {} - {}", title, message);
    Ok(())
}

pub fn show_recording_started() -> Result<()> {
    show_notification(
        "Recording Started",
        "Voice recording is now active. Press hotkey again to stop."
    )
}

pub fn show_recording_stopped(duration_secs: u64) -> Result<()> {
    show_notification(
        "Recording Stopped",
        &format!("Recording saved ({} seconds). Transcribing...", duration_secs)
    )
}

pub fn show_transcription_complete(text: &str) -> Result<()> {
    let preview = if text.len() > 100 {
        format!("{}...", &text[..100])
    } else {
        text.to_string()
    };
    
    show_notification(
        "Transcription Complete",
        &preview
    )
}

pub fn show_error(error: &str) -> Result<()> {
    show_notification(
        "Error",
        error
    )
}