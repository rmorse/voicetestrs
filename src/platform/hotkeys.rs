use anyhow::{Result, Context};
use global_hotkey::{
    GlobalHotKeyManager, HotKeyState,
    hotkey::{HotKey, Code, Modifiers},
};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, Sender, Receiver};
use tracing::{info, error};

#[derive(Debug, Clone)]
pub enum HotkeyEvent {
    RecordingToggle,
    QuickNote,
    ShowWindow,
}

pub struct HotkeyManager {
    manager: GlobalHotKeyManager,
    hotkeys: Vec<HotKey>,
    event_sender: Sender<HotkeyEvent>,
    event_receiver: Arc<Mutex<Receiver<HotkeyEvent>>>,
    enabled: bool,
}

impl HotkeyManager {
    pub fn new() -> Result<Self> {
        let manager = GlobalHotKeyManager::new()
            .context("Failed to create global hotkey manager")?;
        
        let (tx, rx) = mpsc::channel();
        
        Ok(Self {
            manager,
            hotkeys: Vec::new(),
            event_sender: tx,
            event_receiver: Arc::new(Mutex::new(rx)),
            enabled: true,
        })
    }
    
    pub fn register_defaults(&mut self) -> Result<()> {
        // Default hotkeys
        self.register_recording_hotkey()?;
        self.register_quick_note_hotkey()?;
        self.register_show_window_hotkey()?;
        
        info!("Default hotkeys registered");
        Ok(())
    }
    
    fn register_recording_hotkey(&mut self) -> Result<()> {
        // Ctrl+Shift+R for recording toggle
        let hotkey = HotKey::new(
            Some(Modifiers::CONTROL | Modifiers::SHIFT),
            Code::KeyR
        );
        
        self.manager.register(hotkey)
            .context("Failed to register recording hotkey")?;
        
        self.hotkeys.push(hotkey);
        info!("Registered hotkey: Ctrl+Shift+R for recording toggle");
        
        Ok(())
    }
    
    fn register_quick_note_hotkey(&mut self) -> Result<()> {
        // Ctrl+Shift+N for quick note
        let hotkey = HotKey::new(
            Some(Modifiers::CONTROL | Modifiers::SHIFT),
            Code::KeyN
        );
        
        self.manager.register(hotkey)
            .context("Failed to register quick note hotkey")?;
        
        self.hotkeys.push(hotkey);
        info!("Registered hotkey: Ctrl+Shift+N for quick note");
        
        Ok(())
    }
    
    fn register_show_window_hotkey(&mut self) -> Result<()> {
        // Ctrl+Shift+V for show window
        let hotkey = HotKey::new(
            Some(Modifiers::CONTROL | Modifiers::SHIFT),
            Code::KeyV
        );
        
        self.manager.register(hotkey)
            .context("Failed to register show window hotkey")?;
        
        self.hotkeys.push(hotkey);
        info!("Registered hotkey: Ctrl+Shift+V for show window");
        
        Ok(())
    }
    
    pub fn handle_events(&self) -> Result<Option<HotkeyEvent>> {
        if !self.enabled {
            return Ok(None);
        }
        
        // Check if any hotkey was pressed
        if let Ok(event) = global_hotkey::GlobalHotKeyEvent::receiver().try_recv() {
            if event.state == HotKeyState::Pressed {
                // Map hotkey ID to event
                // For simplicity, we'll check against our registered hotkeys
                if self.hotkeys.len() > 0 && event.id == self.hotkeys[0].id() {
                    info!("Recording hotkey pressed");
                    return Ok(Some(HotkeyEvent::RecordingToggle));
                } else if self.hotkeys.len() > 1 && event.id == self.hotkeys[1].id() {
                    info!("Quick note hotkey pressed");
                    return Ok(Some(HotkeyEvent::QuickNote));
                } else if self.hotkeys.len() > 2 && event.id == self.hotkeys[2].id() {
                    info!("Show window hotkey pressed");
                    return Ok(Some(HotkeyEvent::ShowWindow));
                }
            }
        }
        
        // Check for events from other parts of the app
        if let Ok(rx) = self.event_receiver.lock() {
            if let Ok(event) = rx.try_recv() {
                return Ok(Some(event));
            }
        }
        
        Ok(None)
    }
    
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        info!("Hotkeys {}", if enabled { "enabled" } else { "disabled" });
    }
    
    pub fn unregister_all(&mut self) -> Result<()> {
        for hotkey in &self.hotkeys {
            self.manager.unregister(*hotkey)
                .context("Failed to unregister hotkey")?;
        }
        self.hotkeys.clear();
        info!("All hotkeys unregistered");
        Ok(())
    }
    
    pub fn send_event(&self, event: HotkeyEvent) -> Result<()> {
        self.event_sender.send(event)
            .context("Failed to send hotkey event")?;
        Ok(())
    }
}

impl Drop for HotkeyManager {
    fn drop(&mut self) {
        // Clean up hotkeys on drop
        if let Err(e) = self.unregister_all() {
            error!("Failed to unregister hotkeys on drop: {}", e);
        }
    }
}