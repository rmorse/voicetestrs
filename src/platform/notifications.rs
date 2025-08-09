// Placeholder for desktop notifications
use anyhow::Result;

pub fn show_notification(title: &str, message: &str) -> Result<()> {
    // TODO: Implement with notify-rust
    println!("NOTIFICATION: {} - {}", title, message);
    Ok(())
}