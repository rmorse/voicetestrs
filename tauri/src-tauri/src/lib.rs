mod commands;

use std::sync::Arc;
use std::process::Command;
use tokio::sync::Mutex;
use voicetextrs::core::transcription::Transcriber;
use commands::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  // Find an unused port
  let port = portpicker::pick_unused_port().expect("failed to find unused port");
  println!("Using port: {}", port);
  
  // Start Vite dev server with the selected port
  if cfg!(debug_assertions) {
    std::env::set_var("VITE_PORT", port.to_string());
    
    // Start Vite in the background
    std::thread::spawn(move || {
      // On Windows, we need to use cmd to run npm
      let output = if cfg!(windows) {
        Command::new("cmd")
          .args(&["/C", "npm", "run", "dev"])
          .env("VITE_PORT", port.to_string())
          .current_dir("../")  // Go up to tauri directory
          .spawn()
      } else {
        Command::new("npm")
          .args(&["run", "dev"])
          .env("VITE_PORT", port.to_string())
          .current_dir("../")
          .spawn()
      };
      
      match output {
        Ok(mut child) => {
          println!("Vite dev server started on port {}", port);
          // Keep the process running
          let _ = child.wait();
        }
        Err(e) => {
          eprintln!("Failed to start Vite dev server: {}", e);
        }
      }
    });
    
    // Give Vite more time to start
    std::thread::sleep(std::time::Duration::from_secs(5));
  }
  
  // Initialize the app state
  let app_state = AppState {
    recorder: Arc::new(Mutex::new(None)),
    transcriber: Arc::new(Transcriber::new().expect("Failed to create transcriber")),
  };

  let mut context = tauri::generate_context!();
  
  // Update the dev URL to use our dynamic port
  if cfg!(debug_assertions) {
    let url = format!("http://localhost:{}", port).parse().unwrap();
    context.config_mut().build.dev_url = Some(url);
  }

  tauri::Builder::default()
    .plugin(tauri_plugin_localhost::Builder::new(port).build())
    .manage(app_state)
    .invoke_handler(tauri::generate_handler![
      commands::start_recording,
      commands::stop_recording,
      commands::quick_note,
      commands::transcribe_file,
      commands::get_recording_status,
    ])
    .setup(move |app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      
      Ok(())
    })
    .run(context)
    .expect("error while running tauri application");
}
