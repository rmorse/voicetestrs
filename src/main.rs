use anyhow::Result;
use clap::Parser;
use tracing::{info, warn};
use tracing_subscriber;

mod core;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Test audio recording for N seconds
    #[arg(short, long)]
    test: Option<u64>,
    
    /// List available audio devices
    #[arg(short, long)]
    list_devices: bool,
    
    /// Use specific audio device by name
    #[arg(short, long)]
    device: Option<String>,
}

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "voicetextrs=info".into()),
        )
        .init();
    
    info!("VoiceTextRS starting...");
    
    let args = Args::parse();
    
    if args.list_devices {
        core::audio::list_audio_devices()?;
        return Ok(());
    }
    
    if let Some(duration) = args.test {
        info!("Testing audio recording for {} seconds", duration);
        core::audio::test_recording(duration, args.device)?;
        return Ok(());
    }
    
    // TODO: Implement main application loop
    warn!("No command specified. Use --help for options.");
    
    Ok(())
}