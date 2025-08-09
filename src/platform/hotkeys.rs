use anyhow::{Result, Context};
use win_hotkeys::{HotkeyManager as WinHotkeyManager, VKey};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, Sender, Receiver};
use std::thread;
use tracing::{info, error};

#[derive(Debug, Clone)]
pub enum HotkeyEvent {
    RecordingToggle,
    QuickNote,
    ShowWindow,
}

pub struct HotkeyManager {
    event_sender: Sender<HotkeyEvent>,
    event_receiver: Arc<Mutex<Receiver<HotkeyEvent>>>,
    pub enabled: bool,
}

impl HotkeyManager {
    pub fn new() -> Result<Self> {
        let (tx, rx) = mpsc::channel();
        
        Ok(Self {
            event_sender: tx,
            event_receiver: Arc::new(Mutex::new(rx)),
            enabled: true,
        })
    }
    
    pub fn register_defaults(&mut self) -> Result<()> {
        // Clone sender for the thread
        let tx1 = self.event_sender.clone();
        let tx2 = self.event_sender.clone();
        let tx3 = self.event_sender.clone();
        
        // Start hotkey manager in a separate thread (required for event loop)
        thread::spawn(move || {
            let mut manager = WinHotkeyManager::new();
            
            // Register Ctrl+Shift+R for recording toggle
            let result1 = manager.register_hotkey(
                VKey::R,
                &[VKey::LControl, VKey::LShift],
                {
                    let tx = tx1.clone();
                    move || {
                        if let Err(e) = tx.send(HotkeyEvent::RecordingToggle) {
                            error!("Failed to send recording event: {}", e);
                        }
                    }
                }
            );
            
            match result1 {
                Ok(_) => {},
                Err(e) => error!("Failed to register Ctrl+Shift+R: {:?}", e),
            }
            
            // Register Ctrl+Shift+N for quick note
            let result2 = manager.register_hotkey(
                VKey::N,
                &[VKey::LControl, VKey::LShift],
                {
                    let tx = tx2.clone();
                    move || {
                        if let Err(e) = tx.send(HotkeyEvent::QuickNote) {
                            error!("Failed to send quick note event: {}", e);
                        }
                    }
                }
            );
            
            match result2 {
                Ok(_) => {},
                Err(e) => error!("Failed to register Ctrl+Shift+N: {:?}", e),
            }
            
            // Register Ctrl+Shift+V for show window
            let result3 = manager.register_hotkey(
                VKey::V,
                &[VKey::LControl, VKey::LShift],
                {
                    let tx = tx3.clone();
                    move || {
                        if let Err(e) = tx.send(HotkeyEvent::ShowWindow) {
                            error!("Failed to send show window event: {}", e);
                        }
                    }
                }
            );
            
            match result3 {
                Ok(_) => {},
                Err(e) => error!("Failed to register Ctrl+Shift+V: {:?}", e),
            }
            
            info!("Hotkeys registered: Ctrl+Shift+R (record), Ctrl+Shift+N (quick note), Ctrl+Shift+V (show)");
            
            // This blocks and processes Windows messages for hotkeys
            manager.event_loop();
        });
        
        Ok(())
    }
    
    pub fn handle_events(&self) -> Result<Option<HotkeyEvent>> {
        if !self.enabled {
            return Ok(None);
        }
        
        // Check for events from hotkey callbacks
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
        // Note: With the thread-based approach, we'd need a different mechanism
        // to stop the event loop and unregister hotkeys
        info!("Hotkeys will be unregistered when thread exits");
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
        // Hotkeys are automatically unregistered when the thread exits
    }
}