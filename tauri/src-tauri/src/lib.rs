mod commands;

use std::sync::Arc;
use tokio::sync::Mutex;
use voicetextrs::core::transcription::WhisperTranscriber;
use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  // Initialize the app state
  let app_state = AppState {
    recorder: Arc::new(Mutex::new(None)),
    transcriber: Arc::new(WhisperTranscriber::new()),
  };

  tauri::Builder::default()
    .manage(app_state)
    .invoke_handler(tauri::generate_handler![
      commands::start_recording,
      commands::stop_recording,
      commands::quick_note,
      commands::transcribe_file,
      commands::get_recording_status,
    ])
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
