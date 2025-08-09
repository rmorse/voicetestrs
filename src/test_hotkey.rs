// Simple test program to verify global-hotkey is working
use global_hotkey::{GlobalHotKeyManager, hotkey::{HotKey, Code, Modifiers}};
use std::time::Duration;

fn main() {
    println!("Starting hotkey test...");
    
    let manager = GlobalHotKeyManager::new().unwrap();
    
    // Try a simple hotkey first
    let hotkey = HotKey::new(
        Some(Modifiers::CONTROL),
        Code::KeyQ
    );
    
    println!("Registering Ctrl+Q (ID: {})", hotkey.id());
    
    match manager.register(hotkey) {
        Ok(_) => println!("✓ Hotkey registered successfully!"),
        Err(e) => println!("✗ Failed to register: {:?}", e),
    }
    
    println!("Press Ctrl+Q to test (running for 30 seconds)...");
    
    // Windows message pump
    #[cfg(target_os = "windows")]
    {
        std::thread::spawn(|| {
            unsafe {
                use windows::Win32::UI::WindowsAndMessaging::{GetMessageW, TranslateMessage, DispatchMessageW, MSG};
                let mut msg = MSG::default();
                loop {
                    let ret = GetMessageW(&mut msg, None, 0, 0);
                    if ret.0 <= 0 { break; }
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        });
    }
    
    // Check for events
    for i in 0..300 {
        if let Ok(event) = global_hotkey::GlobalHotKeyEvent::receiver().try_recv() {
            println!("🎉 Hotkey event received! ID: {}, State: {:?}", event.id, event.state);
        }
        std::thread::sleep(Duration::from_millis(100));
        if i % 10 == 0 {
            print!(".");
            use std::io::Write;
            std::io::stdout().flush().unwrap();
        }
    }
    
    println!("\nTest complete!");
}