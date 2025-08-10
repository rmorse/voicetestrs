mod commands;
mod database;
mod api;
mod sync;
mod queue_manager;

use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use std::sync::Mutex as StdMutex;
use voicetextrs::core::transcription::Transcriber;
use voicetextrs::core::audio::AudioRecorder;
use queue_manager::QueueManager;
use commands::{AppState, RecordingState};
use tauri::{
    Manager, Emitter,
    tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState},
    menu::{Menu, PredefinedMenuItem, MenuItemBuilder},
    AppHandle,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  
  // Use fixed port for development
  let port = 5173;
  
  // Initialize the app state with pre-initialized recorder
  println!("Creating audio recorder...");
  let mut recorder = AudioRecorder::new().expect("Failed to create audio recorder");
  
  // Pre-initialize the audio stream to avoid delay when recording starts
  println!("Pre-initializing audio stream to avoid recording delay...");
  recorder.initialize_stream().expect("Failed to initialize audio stream");
  println!("Audio stream pre-initialized successfully!");
  
  
  let app_state = AppState {
    recorder: Arc::new(TokioMutex::new(Some(recorder))),
    transcriber: Arc::new(Transcriber::new().expect("Failed to create transcriber")),
    state: Arc::new(TokioMutex::new(RecordingState::Idle)),
  };

  let context = tauri::generate_context!();
  
  tauri::Builder::default()
    .plugin(tauri_plugin_localhost::Builder::new(port).build())
    .plugin(tauri_plugin_global_shortcut::Builder::new().build())
    .manage(app_state)
    .invoke_handler(tauri::generate_handler![
      commands::start_recording,
      commands::stop_recording,
      commands::quick_note,
      commands::transcribe_file,
      commands::get_recording_status,
      // SQLx-based API commands
      api::transcriptions::get_transcriptions,
      api::transcriptions::get_transcription,
      api::transcriptions::update_transcription,
      api::transcriptions::delete_transcription,
      api::transcriptions::search_transcriptions,
      api::transcriptions::get_database_stats,
      api::transcriptions::clear_database,
      api::transcriptions::cleanup_duplicate_transcriptions,
      sync::sync_filesystem_sqlx,
      // Queue management commands
      api::queue::get_queue_status,
      api::queue::get_queue_tasks,
      api::queue::enqueue_orphan_task,
      api::queue::pause_queue,
      api::queue::resume_queue,
      api::queue::retry_failed_task,
      api::queue::clear_completed_tasks,
      api::queue::is_queue_paused,
    ])
    .setup(move |app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      
      // Initialize database with proper path
      let app_handle = app.handle().clone();
      let database_path = app_handle.path()
        .app_data_dir()
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?
        .join("voicetextrs.db");
      
      // Ensure the directory exists
      if let Some(parent) = database_path.parent() {
        std::fs::create_dir_all(parent)?;
      }
      
      let database_url = format!("sqlite:{}", database_path.to_string_lossy());
      println!("Database path: {}", database_url);
      
      let database = tauri::async_runtime::block_on(async {
        database::Database::new(&database_url).await
      })?;
      
      // Add database to managed state
      app.manage(database.clone());
      
      // Initialize queue manager
      let app_state_ref = app.state::<AppState>();
      let mut queue_manager = QueueManager::new(app_state_ref.transcriber.clone());
      queue_manager.set_app_handle(app.handle().clone());
      let queue_manager = Arc::new(queue_manager);
      
      // Start the queue worker
      let queue_clone = queue_manager.clone();
      let db_clone = database.clone();
      tauri::async_runtime::spawn(async move {
        queue_clone.start_worker(db_clone).await;
      });
      
      // Add queue manager to managed state
      app.manage(queue_manager);
      
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
    .build(context)
    .expect("error while building tauri application")
    .run(|app_handle, event| match event {
      tauri::RunEvent::ExitRequested { .. } => {
        println!("Exit requested, performing cleanup...");
        
       
        // Perform other cleanup
        let handle = app_handle.clone();
        tauri::async_runtime::block_on(async move {
          cleanup_processes(&handle).await;
        });
        
        // Allow the app to exit
        println!("Cleanup done, exiting...");
      }
      tauri::RunEvent::Exit => {
        println!("App exiting, final cleanup...");
      }
      _ => {}
    });
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
                    // Close the window before exiting.
                    if let Some(window) = app.get_webview_window("main") {
                        if let Err(err) = window.close() {
                            eprintln!("Failed to close window before exiting: {}", err);
                        }
                    }
                    app.app_handle().exit(0);
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
    // Simply call the existing start_recording command which handles everything
    use tauri::State;
    
    let state: State<AppState> = app.state();
    commands::start_recording(app.clone(), state).await
        .map_err(|e| Box::new(std::io::Error::new(std::io::ErrorKind::Other, e)) as Box<dyn std::error::Error>)?;
    
    println!("Recording started from tray/hotkey");
    Ok(())
}

pub async fn stop_recording_from_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // Simply call the existing stop_recording command which handles everything
    use tauri::State;
    
    let state: State<AppState> = app.state();
    let _result = commands::stop_recording(app.clone(), state).await
        .map_err(|e| Box::new(std::io::Error::new(std::io::ErrorKind::Other, e)) as Box<dyn std::error::Error>)?;
    
    println!("Recording stopped and transcribed from tray/hotkey");
    Ok(())
}

async fn quick_note_from_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // Simply call the existing quick_note command which handles everything
    use tauri::State;
    
    let state: State<AppState> = app.state();
    commands::quick_note(app.clone(), state, 10).await
        .map_err(|e| Box::new(std::io::Error::new(std::io::ErrorKind::Other, e)) as Box<dyn std::error::Error>)?;
    
    println!("Quick note completed from tray");
    Ok(())
}

// Cleanup helper function
async fn cleanup_processes(app: &AppHandle) {
    println!("Cleaning up...");
    
    // Based on GitHub issue #7606 - just close, don't destroy
    if let Some(window) = app.get_webview_window("main") {
        println!("Closing main window...");
        if let Err(e) = window.destroy() {
            eprintln!("Error closing main window: {}", e);
        }
    }
    
    println!("Cleanup complete");
}