// Platform-specific modules
#[cfg(target_os = "windows")]
pub mod windows;

pub mod hotkeys;
pub mod notifications;
pub mod tray;