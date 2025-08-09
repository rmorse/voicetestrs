use anyhow::{Result, Context};
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu, CheckMenuItem},
};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, Sender, Receiver};
use tracing::info;

#[derive(Debug, Clone)]
pub enum TrayCommand {
    StartRecording,
    StopRecording,
    ShowWindow,
    OpenSettings,
    ToggleHotkeys,
    Exit,
}

pub struct TrayManager {
    tray_icon: Option<TrayIcon>,
    menu: Menu,
    recording_item: MenuItem,
    hotkeys_item: CheckMenuItem,
    command_sender: Sender<TrayCommand>,
    command_receiver: Arc<Mutex<Receiver<TrayCommand>>>,
}

impl TrayManager {
    pub fn new() -> Result<Self> {
        // Create communication channel
        let (tx, rx) = mpsc::channel();
        
        // Create menu items
        let menu = Menu::new();
        
        // Recording control
        let recording_item = MenuItem::new("Start Recording", true, None);
        menu.append(&recording_item)?;
        
        menu.append(&PredefinedMenuItem::separator())?;
        
        // Settings submenu
        let settings_menu = Submenu::new("Settings", true);
        let hotkeys_item = CheckMenuItem::new("Enable Hotkeys", true, true, None);
        settings_menu.append(&hotkeys_item)?;
        settings_menu.append(&MenuItem::new("Preferences...", true, None))?;
        menu.append(&settings_menu)?;
        
        menu.append(&PredefinedMenuItem::separator())?;
        
        // Show window
        let show_item = MenuItem::new("Show Window", true, None);
        menu.append(&show_item)?;
        
        menu.append(&PredefinedMenuItem::separator())?;
        
        // Exit
        let exit_item = MenuItem::new("Exit", true, None);
        menu.append(&exit_item)?;
        
        Ok(Self {
            tray_icon: None,
            menu,
            recording_item,
            hotkeys_item,
            command_sender: tx,
            command_receiver: Arc::new(Mutex::new(rx)),
        })
    }
    
    pub fn init(&mut self) -> Result<()> {
        // Load icon (we'll create a simple one or use a default)
        let icon = self.load_icon()?;
        
        // Build tray icon
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(self.menu.clone()))
            .with_tooltip("VoiceTextRS - Click to open menu")
            .with_icon(icon)
            .build()
            .context("Failed to create tray icon")?;
        
        self.tray_icon = Some(tray);
        info!("System tray initialized");
        
        Ok(())
    }
    
    fn load_icon(&self) -> Result<Icon> {
        // For now, create a simple colored icon
        // In production, load from resources
        let rgba = Self::create_default_icon();
        Icon::from_rgba(rgba, 32, 32).context("Failed to create icon")
    }
    
    fn create_default_icon() -> Vec<u8> {
        // Create a simple 32x32 icon (microphone shape)
        let mut rgba = vec![0u8; 32 * 32 * 4];
        
        // Simple microphone icon in blue
        for y in 0..32 {
            for x in 0..32 {
                let idx = (y * 32 + x) * 4;
                
                // Microphone body (circle at top)
                let dx = x as f32 - 16.0;
                let dy = y as f32 - 10.0;
                if dx * dx + dy * dy < 64.0 && y < 20 {
                    rgba[idx] = 33;      // R
                    rgba[idx + 1] = 150;  // G
                    rgba[idx + 2] = 243;  // B
                    rgba[idx + 3] = 255;  // A
                }
                
                // Microphone stand
                if x >= 15 && x <= 17 && y >= 18 && y <= 26 {
                    rgba[idx] = 33;
                    rgba[idx + 1] = 150;
                    rgba[idx + 2] = 243;
                    rgba[idx + 3] = 255;
                }
                
                // Base
                if x >= 12 && x <= 20 && y >= 26 && y <= 28 {
                    rgba[idx] = 33;
                    rgba[idx + 1] = 150;
                    rgba[idx + 2] = 243;
                    rgba[idx + 3] = 255;
                }
            }
        }
        
        rgba
    }
    
    pub fn set_recording(&mut self, is_recording: bool) -> Result<()> {
        let text = if is_recording {
            "Stop Recording"
        } else {
            "Start Recording"
        };
        
        self.recording_item.set_text(text);
        
        // Update icon color when recording
        let icon = if is_recording {
            Self::create_recording_icon_static()?
        } else {
            Self::create_default_icon_static()?
        };
        
        if let Some(ref mut tray) = self.tray_icon {
            tray.set_icon(Some(icon))?;
        }
        
        Ok(())
    }
    
    fn create_default_icon_static() -> Result<Icon> {
        let rgba = Self::create_default_icon();
        Icon::from_rgba(rgba, 32, 32).context("Failed to create icon")
    }
    
    fn create_recording_icon_static() -> Result<Icon> {
        // Red icon when recording
        let mut rgba = vec![0u8; 32 * 32 * 4];
        
        for y in 0..32 {
            for x in 0..32 {
                let idx = (y * 32 + x) * 4;
                
                // Red circle for recording
                let dx = x as f32 - 16.0;
                let dy = y as f32 - 16.0;
                if dx * dx + dy * dy < 100.0 {
                    rgba[idx] = 239;      // R
                    rgba[idx + 1] = 68;   // G
                    rgba[idx + 2] = 68;   // B
                    rgba[idx + 3] = 255;  // A
                }
            }
        }
        
        Icon::from_rgba(rgba, 32, 32).context("Failed to create recording icon")
    }
    
    pub fn handle_events(&self) -> Result<Option<TrayCommand>> {
        // Check for menu events
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            // Map menu item IDs to commands
            // Note: In production, we'd track item IDs properly
            info!("Menu event received: {:?}", event);
            
            // For now, return commands based on menu structure
            // This is simplified - in production we'd track proper IDs
            return Ok(Some(TrayCommand::ShowWindow));
        }
        
        // Check for commands from other parts of the app
        if let Ok(rx) = self.command_receiver.lock() {
            if let Ok(cmd) = rx.try_recv() {
                return Ok(Some(cmd));
            }
        }
        
        Ok(None)
    }
    
    pub fn send_command(&self, command: TrayCommand) -> Result<()> {
        self.command_sender.send(command)
            .context("Failed to send tray command")?;
        Ok(())
    }
    
    pub fn show_notification(&self, title: &str, message: &str) -> Result<()> {
        // Delegate to notifications module
        crate::platform::notifications::show_notification(title, message)?;
        Ok(())
    }
}