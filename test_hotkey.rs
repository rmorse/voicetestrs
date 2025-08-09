// Simple standalone test - put this in project root
use global_hotkey::{GlobalHotKeyManager, hotkey::{HotKey, Code, Modifiers}};

fn main() {
    println!("Testing global-hotkey...");
    
    let manager = GlobalHotKeyManager::new().unwrap();
    let hotkey = HotKey::new(Some(Modifiers::CONTROL), Code::KeyQ);
    
    println!("Registering Ctrl+Q...");
    manager.register(hotkey).expect("Failed to register");
    
    println!("Press Ctrl+Q (waiting 10 seconds)");
    
    // Message pump for Windows
    #[cfg(windows)]
    std::thread::spawn(|| unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{GetMessageW, TranslateMessage, DispatchMessageW, MSG};
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
    
    for _ in 0..100 {
        if let Ok(event) = global_hotkey::GlobalHotKeyEvent::receiver().try_recv() {
            println!("✓ Hotkey pressed!");
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}