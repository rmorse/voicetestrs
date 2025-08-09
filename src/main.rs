use anyhow::Result;
use clap::Parser;
use tracing::{info, warn};
use tracing_subscriber;
use std::path::PathBuf;

mod core;
mod platform;
mod app;

use app::App;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Run in background mode with system tray
    #[arg(short, long)]
    background: bool,
    
    /// Test audio recording for N seconds
    #[arg(short, long)]
    test: Option<u64>,
    
    /// List available audio devices
    #[arg(short, long)]
    list_devices: bool,
    
    /// Use specific audio device by name
    #[arg(short, long)]
    device: Option<String>,
    
    /// Transcribe an audio file
    #[arg(long)]
    transcribe: Option<String>,
    
    /// Record and transcribe for N seconds
    #[arg(short, long)]
    record: Option<u64>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "voicetextrs=info".into()),
        )
        .init();
    
    info!("VoiceTextRS starting...");
    
    let args = Args::parse();
    
    // Check if running in background mode
    if args.background {
        info!("Starting in background mode with system tray");
        let mut app = App::new()?;
        app.run().await?;
        return Ok(());
    }
    
    // Otherwise run CLI commands
    if args.list_devices || args.test.is_some() || args.transcribe.is_some() || args.record.is_some() {
        app::run_cli_command(
            args.record,
            args.transcribe,
            args.test,
            args.list_devices,
            args.device,
        ).await?;
        return Ok(());
    }
    
    // No command specified - show help
    warn!("No command specified. Use --help for options.");
    println!("\nQuick start:");
    println!("  cargo run -- --background      # Run with system tray");
    println!("  cargo run -- --record 5        # Record and transcribe");
    println!("  cargo run -- --list-devices    # List audio devices");
    println!("\nIn background mode:");
    println!("  Ctrl+Shift+R - Toggle recording");
    println!("  Ctrl+Shift+N - Quick note (10 sec)");
    println!("  Ctrl+Shift+V - Show window");
    
    Ok(())
}