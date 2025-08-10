mod commands;
mod db_commands;

use std::sync::Arc;
use std::process::Command;
use tokio::sync::Mutex;
use voicetextrs::core::transcription::Transcriber;
use voicetextrs::core::audio::AudioRecorder;
use commands::{AppState, RecordingState};
use tauri::{
    Manager, Emitter,
    tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState},
    menu::{Menu, PredefinedMenuItem, MenuItemBuilder},
    AppHandle,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tauri_plugin_sql::{Migration, MigrationKind};

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
  
  // Initialize the app state with pre-initialized recorder
  println!("Creating audio recorder...");
  let mut recorder = AudioRecorder::new().expect("Failed to create audio recorder");
  
  // Pre-initialize the audio stream to avoid delay when recording starts
  println!("Pre-initializing audio stream to avoid recording delay...");
  recorder.initialize_stream().expect("Failed to initialize audio stream");
  println!("Audio stream pre-initialized successfully!");
  
  let app_state = AppState {
    recorder: Arc::new(Mutex::new(Some(recorder))),
    transcriber: Arc::new(Transcriber::new().expect("Failed to create transcriber")),
    state: Arc::new(Mutex::new(RecordingState::Idle)),
  };

  let mut context = tauri::generate_context!();
  
  // Update the dev URL to use our dynamic port
  if cfg!(debug_assertions) {
    let url = format!("http://localhost:{}", port).parse().unwrap();
    context.config_mut().build.dev_url = Some(url);
  }

  // Database migrations
  let migrations = vec![
    Migration {
      version: 1,
      description: "Initial schema",
      sql: include_str!("../migrations/001_initial.sql"),
      kind: MigrationKind::Up,
    },
    Migration {
      version: 2,
      description: "Add full-text search",
      sql: include_str!("../migrations/002_fts.sql"),
      kind: MigrationKind::Up,
    },
  ];

  tauri::Builder::default()
    .plugin(tauri_plugin_localhost::Builder::new(port).build())
    .plugin(tauri_plugin_global_shortcut::Builder::new().build())
    .plugin(
      tauri_plugin_sql::Builder::default()
        .add_migrations("sqlite:voicetextrs.db", migrations)
        .build(),
    )
    .manage(app_state)
    .invoke_handler(tauri::generate_handler![
      commands::start_recording,
      commands::stop_recording,
      commands::quick_note,
      commands::transcribe_file,
      commands::get_recording_status,
      db_commands::db_get_transcriptions,
      db_commands::db_search_transcriptions,
      db_commands::db_insert_transcription,
      db_commands::db_update_transcription_status,
      db_commands::db_get_queue_status,
      db_commands::db_enqueue_task,
      db_commands::db_retry_task,
      db_commands::db_clear_completed_tasks,
      db_commands::sync_filesystem,
    ])
    .setup(move |app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      
      // Set up system tray
      setup_system_tray(app)?;
      
      // Set up global hotkeys
      setup_global_hotkeys(app)?;
      
      // Trigger filesystem sync on startup
      let app_handle = app.handle().clone();
      tauri::async_runtime::spawn(async move {
        // Wait a bit for the frontend to be ready
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        
        // Emit event to trigger sync
        if let Err(e) = app_handle.emit("start-filesystem-sync", ()) {
          eprintln!("Failed to emit sync event: {}", e);
        }
      });
      
      // Check for --background flag
      let args: Vec<String> = std::env::args().collect();
      if args.contains(&"--background".to_string()) {
        // Start with window hidden
        if let Some(window) = app.get_webview_window("main") {
          window.hide().unwrap();
        }
      }
      
      Ok(())
    })
    .on_window_event(|window, event| {
      // Handle window close event - hide instead of quit
      if let tauri::WindowEvent::CloseRequested { api, .. } = event {
        // Hide the window instead of closing
        window.hide().unwrap();
        // Prevent the default close behavior
        api.prevent_close();
      }
    })
    .run(context)
    .expect("error while running tauri application");
}

fn setup_system_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // Create the tray menu with a single toggle item
    let show_hide = MenuItemBuilder::with_id("show_hide", "Show/Hide Window").build(app)?;
    let separator1 = PredefinedMenuItem::separator(app)?;
    let toggle_recording_item = MenuItemBuilder::with_id("toggle_recording", "Toggle Recording").build(app)?;
    let quick_note = MenuItemBuilder::with_id("quick_note", "Quick Note (10s)").build(app)?;
    let separator2 = PredefinedMenuItem::separator(app)?;
    let settings = MenuItemBuilder::with_id("settings", "Settings").build(app)?;
    let separator3 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
    
    let menu = Menu::with_items(
        app,
        &[
            &show_hide,
            &separator1,
            &toggle_recording_item,
            &quick_note,
            &separator2,
            &settings,
            &separator3,
            &quit,
        ],
    )?;
    
    // Create the system tray
    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| {
            match event.id.as_ref() {
                "show_hide" => {
                    toggle_window_visibility(app);
                }
                "toggle_recording" => {
                    // Toggle recording state
                    let app_handle = app.app_handle().clone();
                    tauri::async_runtime::spawn(async move {
                        toggle_recording(&app_handle).await;
                    });
                }
                "quick_note" => {
                    // Trigger quick note command
                    let app_handle = app.app_handle().clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = quick_note_from_tray(&app_handle).await {
                            eprintln!("Failed to start quick note: {}", e);
                        }
                    });
                }
                "settings" => {
                    // Show settings (for now just show the main window)
                    if let Some(window) = app.get_webview_window("main") {
                        window.show().unwrap();
                        window.set_focus().unwrap();
                    }
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            match event {
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => {
                    // Single left click - do nothing (menu is on right click)
                }
                TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                } => {
                    // Double click - toggle window visibility
                    toggle_window_visibility(&tray.app_handle());
                }
                _ => {}
            }
        })
        .tooltip("VoiceTextRS - Click to show menu")
        .build(app)?;
    
    Ok(())
}

fn setup_global_hotkeys(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let shortcuts = app.global_shortcut();
    
    // Register Ctrl+Shift+R for recording toggle
    let record_shortcut = Shortcut::new(Some(tauri_plugin_global_shortcut::Modifiers::CONTROL | tauri_plugin_global_shortcut::Modifiers::SHIFT), tauri_plugin_global_shortcut::Code::KeyR);
    match shortcuts.on_shortcut(record_shortcut.clone(), move |app_handle, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            println!("Recording hotkey pressed");
            let handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                toggle_recording(&handle).await;
            });
        }
    }) {
        Ok(_) => println!("Registered Ctrl+Shift+R"),
        Err(e) => eprintln!("Warning: Could not register Ctrl+Shift+R: {}", e),
    }
    
    // Register Ctrl+Shift+N for quick note
    let note_shortcut = Shortcut::new(Some(tauri_plugin_global_shortcut::Modifiers::CONTROL | tauri_plugin_global_shortcut::Modifiers::SHIFT), tauri_plugin_global_shortcut::Code::KeyN);
    match shortcuts.on_shortcut(note_shortcut.clone(), move |app_handle, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            println!("Quick note hotkey pressed");
            let handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = quick_note_from_tray(&handle).await {
                    eprintln!("Failed to start quick note: {}", e);
                }
            });
        }
    }) {
        Ok(_) => println!("Registered Ctrl+Shift+N"),
        Err(e) => eprintln!("Warning: Could not register Ctrl+Shift+N: {}", e),
    }
    
    // Register Ctrl+Shift+V for show/hide window
    let window_shortcut = Shortcut::new(Some(tauri_plugin_global_shortcut::Modifiers::CONTROL | tauri_plugin_global_shortcut::Modifiers::SHIFT), tauri_plugin_global_shortcut::Code::KeyV);
    match shortcuts.on_shortcut(window_shortcut.clone(), move |app_handle, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            println!("Show/hide window hotkey pressed");
            toggle_window_visibility(&app_handle);
        }
    }) {
        Ok(_) => println!("Registered Ctrl+Shift+V"),
        Err(e) => eprintln!("Warning: Could not register Ctrl+Shift+V: {}", e),
    }
    
    println!("Global hotkeys setup complete");
    Ok(())
}

fn toggle_window_visibility(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            window.hide().unwrap();
        } else {
            window.show().unwrap();
            window.set_focus().unwrap();
        }
    }
}

async fn toggle_recording(app: &AppHandle) {
    // Check current state
    let state = app.state::<AppState>();
    let current_state = *state.state.lock().await;
    
    if current_state == RecordingState::Recording {
        println!("Stopping recording via hotkey");
        if let Err(e) = stop_recording_from_tray(app).await {
            eprintln!("Failed to stop recording: {}", e);
        }
    } else {
        println!("Starting recording via hotkey");
        if let Err(e) = start_recording_from_tray(app).await {
            eprintln!("Failed to start recording: {}", e);
        }
    }
}

pub async fn start_recording_from_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let state = app.state::<AppState>();
    
    // Check current state - must be Idle
    let current_state = *state.state.lock().await;
    if current_state != RecordingState::Idle {
        println!("Cannot start recording in {:?} state", current_state);
        return Ok(());
    }
    
    // Use the pre-initialized recorder
    let mut recorder_lock = state.recorder.lock().await;
    if let Some(recorder) = recorder_lock.as_mut() {
        recorder.start_recording()?;
        *state.state.lock().await = RecordingState::Recording;
    } else {
        return Err("Recorder not initialized".into());
    }
    
    // TODO: Update tray menu text when Tauri supports it
    
    // Emit state change event
    app.emit("state-changed", serde_json::json!({
        "state": "recording"
    }))?;
    
    println!("Recording started from tray");
    Ok(())
}

pub async fn stop_recording_from_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let state = app.state::<AppState>();
    
    // Check current state - must be Recording
    let current_state = *state.state.lock().await;
    if current_state != RecordingState::Recording {
        println!("Cannot stop recording in {:?} state", current_state);
        return Ok(());
    }
    
    // Set state to Processing immediately
    *state.state.lock().await = RecordingState::Processing;
    
    // Emit state change to show processing UI
    app.emit("state-changed", serde_json::json!({
        "state": "processing"
    }))?;
    
    // Stop recording but keep the recorder alive (don't take it)
    let path = {
        let mut recorder_lock = state.recorder.lock().await;
        if let Some(recorder) = recorder_lock.as_mut() {
            recorder.stop_recording()?
        } else {
            return Err("No active recording".into());
        }
    };
    
    // TODO: Update tray menu text when Tauri supports it
    
    // Transcribe
    let result = state.transcriber.transcribe(&path).await?;
    
    // Save transcription
    let text_path = path.with_extension("txt");
    std::fs::write(&text_path, &result.text)?;
    
    // Save metadata
    let meta_path = path.with_extension("json");
    let metadata = serde_json::json!({
        "audio_file": path.to_string_lossy(),
        "text_file": text_path.to_string_lossy(),
        "language": result.language,
        "duration": result.duration,
        "timestamp": chrono::Local::now().to_rfc3339(),
    });
    std::fs::write(&meta_path, serde_json::to_string_pretty(&metadata)?)?;
    
    // Set state back to Idle
    *state.state.lock().await = RecordingState::Idle;
    
    // Emit state change back to idle
    app.emit("state-changed", serde_json::json!({
        "state": "idle"
    }))?;
    
    // Emit transcription complete event
    app.emit("transcription-complete", serde_json::json!({
        "text": result.text,
        "audio_path": path.to_string_lossy(),
        "text_path": text_path.to_string_lossy(),
    }))?;
    
    println!("Recording stopped and transcribed from tray");
    Ok(())
}

async fn quick_note_from_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // Start recording
    start_recording_from_tray(app).await?;
    
    // Schedule stop after 10 seconds
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        if let Err(e) = stop_recording_from_tray(&app_handle).await {
            eprintln!("Failed to stop quick note recording: {}", e);
        }
    });
    
    println!("Quick note started - will stop in 10 seconds");
    Ok(())
}