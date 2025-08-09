use anyhow::Result;
use clap::Parser;
use tracing::{info, warn};
use tracing_subscriber;
use std::path::PathBuf;

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
    
    if args.list_devices {
        core::audio::list_audio_devices()?;
        return Ok(());
    }
    
    if let Some(duration) = args.test {
        info!("Testing audio recording for {} seconds", duration);
        core::audio::test_recording(duration, args.device.clone())?;
        return Ok(());
    }
    
    if let Some(audio_file) = args.transcribe {
        info!("Transcribing audio file: {}", audio_file);
        let transcriber = core::transcription::Transcriber::new()?;
        let result = transcriber.transcribe(&PathBuf::from(audio_file)).await?;
        println!("\n=== Transcription ===");
        println!("{}", result.text);
        println!("====================\n");
        info!("Language: {}, Duration: {:.1}s", result.language, result.duration);
        return Ok(());
    }
    
    if let Some(duration) = args.record {
        info!("Recording and transcribing for {} seconds", duration);
        
        // Record audio
        let audio_path = core::audio::test_recording(duration, args.device)?;
        info!("Audio saved to: {:?}", audio_path);
        
        // Transcribe the recording
        let transcriber = core::transcription::Transcriber::new()?;
        let result = transcriber.transcribe(&audio_path).await?;
        
        println!("\n=== Transcription ===");
        println!("{}", result.text);
        println!("====================\n");
        
        // Save transcription to text file
        let text_path = audio_path.with_extension("txt");
        std::fs::write(&text_path, &result.text)?;
        info!("Transcription saved to: {:?}", text_path);
        
        return Ok(());
    }
    
    // TODO: Implement main application loop
    warn!("No command specified. Use --help for options.");
    
    Ok(())
}