# Background Research & Technical Notes

## Crate Research (January 2025)

### Version Discovery Method
Due to WebFetch tool issues with crates.io, we used PowerShell to query the crates.io API:
```powershell
# Example command to get crate version
powershell -Command "(Invoke-WebRequest -Uri 'https://crates.io/api/v1/crates/CRATE_NAME').Content | ConvertFrom-Json | Select-Object -ExpandProperty crate | Select-Object name, max_stable_version"
```

### Audio Processing Crates

#### CPAL (0.16.0)
- **Purpose**: Cross-platform audio I/O
- **Docs**: https://docs.rs/cpal/latest/cpal/
- **GitHub**: https://github.com/RustAudio/cpal
- **Why chosen**: Most mature, pure Rust, supports WASAPI/ASIO on Windows
- **Key features**:
  - WASAPI support (default on Windows)
  - Optional ASIO support for low latency
  - Works on Windows, Linux, macOS, Android, iOS, WASM

#### Hound (3.5.1)
- **Purpose**: WAV file reading/writing
- **Why chosen**: Simple, well-maintained, works perfectly with CPAL

### Transcription Options

#### whisper-rs (0.14.4)
- **Last updated**: July 30, 2025
- **GitHub**: https://github.com/tazz4843/whisper-rs
- **Why chosen**: 
  - Active maintenance throughout 2024-2025
  - Rust bindings to whisper.cpp
  - CUDA/ROCm support
  - Good performance
- **Alternative considered**: 
  - Candle framework (pure Rust ML) - more complex setup
  - faster-whisper-rs - Python dependency

#### Whisper Binary Fallback
- **Source**: https://github.com/Purfview/whisper-standalone-win/releases
- **Why keep as fallback**: Proven stability, no compilation needed

### UI Framework Research

#### Tauri (2.7.0) - CHOSEN
- **Docs**: https://tauri.app/
- **Why chosen**:
  - Tauri 2.0 has Android support in beta
  - Mature ecosystem
  - Good developer experience
  - Web technologies familiar to many
- **Considerations**:
  - Larger binary size than pure Rust UI
  - Requires Node.js for development

#### Slint (1.12.1) - Alternative
- **Docs**: https://slint.dev/
- **Pros**: 
  - Native performance
  - Official Android support
  - Declarative UI language
- **Cons**: 
  - Newer framework
  - Smaller community
  - Learning curve for .slint files

#### Dioxus (0.6.3) - Alternative
- **Docs**: https://dioxuslabs.com/
- **Pros**:
  - React-like syntax
  - Multiple render targets
  - Growing community
- **Cons**:
  - Mobile support still evolving
  - Less mature than Tauri

#### EGUI (0.32.0) - Alternative
- **Docs**: https://www.egui.rs/
- **Pros**:
  - Immediate mode GUI
  - Pure Rust
  - Very fast development
- **Cons**:
  - Mobile support experimental
  - Not native look/feel

### System Integration Crates

#### tray-icon (0.21.1)
- **Purpose**: System tray functionality
- **Why chosen**: Works independently or with Tauri

#### global-hotkey (0.7.0)
- **Purpose**: Global keyboard shortcuts
- **Why chosen**: Cross-platform, doesn't require admin on Windows

#### notify-rust (4.11.7)
- **Purpose**: Desktop notifications
- **Why chosen**: Cross-platform, good Windows support

### Windows-Specific

#### windows (0.61.3)
- **Purpose**: Windows API bindings
- **Features needed**:
  - Win32_Foundation
  - Win32_System_Com
  - Win32_UI_Shell

### Async & Utilities

#### tokio (1.47.1)
- **Purpose**: Async runtime
- **Why chosen**: De facto standard for async Rust

#### Other Essential Crates
- **serde** (1.0.219): Serialization
- **serde_json** (1.0.136): JSON support
- **chrono** (0.4.41): Date/time handling
- **anyhow** (1.0.98): Error handling
- **clap** (4.5.43): CLI arguments
- **directories** (6.0.0): Platform-specific paths
- **winit** (0.30.12): Window management (used by Tauri)

## Platform Considerations

### Windows Audio
- **WASAPI**: Default, good for most use cases
- **ASIO**: Optional, lower latency but requires ASIO4ALL or device-specific drivers
- **Buffer sizes**: Important for latency vs stability trade-off

### Cross-Platform Path Handling
- Use `std::path::PathBuf` everywhere
- Use `directories` crate for user folders
- Never hardcode path separators

### Binary Distribution
- **Windows**: MSI installer via cargo-wix or NSIS
- **Single exe**: Possible but models need separate download
- **Tauri bundler**: Handles most platforms automatically

## Known Issues & Workarounds

### 1. WebFetch 404 with crates.io
- **Issue**: WebFetch tool returns 404 for crates.io URLs
- **Workaround**: Use curl or PowerShell's Invoke-WebRequest

### 2. CPAL on Windows
- **Issue**: Some USB microphones report incorrect sample rates
- **Workaround**: Allow user to manually select audio device and sample rate

### 3. Global Hotkeys
- **Issue**: May conflict with other applications
- **Solution**: Make hotkeys configurable, provide alternative activation methods

### 4. Whisper Model Sizes
- **tiny.bin**: 39 MB - Fast but less accurate
- **base.bin**: 74 MB - Best balance (RECOMMENDED)
- **small.bin**: 244 MB - Better accuracy
- **medium.bin**: 769 MB - High accuracy
- **large.bin**: 1550 MB - Best accuracy

## Performance Targets

Based on Rust implementation expectations:
- **Binary size**: ~20-50MB (without models)
- **Memory usage**: <200MB idle, <500MB active
- **Startup time**: <1 second
- **Audio latency**: <50ms achievable with ASIO, <100ms with WASAPI

## Testing Resources

### Audio Testing
- Test with different sample rates: 16kHz, 44.1kHz, 48kHz
- Test with USB microphones, built-in mics, audio interfaces
- Virtual audio cable for automated testing

### Whisper Testing
- Test with different accents and speaking speeds
- Background noise scenarios
- Multiple speakers (future feature)

## Useful Commands

### Development
```bash
# Run with verbose logging
RUST_LOG=debug cargo run

# Build optimized binary
cargo build --release

# Run tests
cargo test

# Check for common issues
cargo clippy
```

### Windows-specific
```powershell
# List audio devices
Get-PnpDevice -Class AudioEndpoint

# Check if binary is signed
Get-AuthenticodeSignature .\target\release\voicetextrs.exe
```

## References & Documentation

### Official Docs
- Rust Audio Working Group: https://rust.audio/
- Tauri Docs: https://tauri.app/v2/
- CPAL Examples: https://github.com/RustAudio/cpal/tree/master/examples
- whisper.cpp: https://github.com/ggerganov/whisper.cpp

### Tutorials & Articles
- Real-time audio processing in Rust: Various RustAudio examples
- Tauri 2.0 mobile development: https://tauri.app/v2/guides/mobile/
- Windows audio programming: WASAPI documentation

### Community Resources
- Rust Audio Discord: Active community for audio programming
- Tauri Discord: Help with Tauri-specific issues
- r/rust: General Rust help

## Future Considerations

### Android Support
- Tauri 2.0 mobile is in beta but actively developed
- CPAL supports Android via Oboe backend
- Consider PWA as alternative mobile strategy

### Voice Activity Detection (VAD)
- webrtc-vad crate for simple VAD
- Silero VAD for ML-based detection
- Custom implementation using RMS energy

### Additional Features
- **Speaker diarization**: Track multiple speakers
- **Real-time transcription**: Stream to Whisper
- **Cloud sync**: Optional encrypted backup
- **LLM integration**: Summarization, formatting

## Development Philosophy

1. **Start simple**: CLI first, GUI later
2. **Test early**: Audio is platform-specific
3. **Fail gracefully**: Always have fallbacks
4. **User control**: Make everything configurable
5. **Privacy first**: Local processing, no telemetry

---

*Last updated: January 2025*
*This document contains research findings and technical notes from the initial planning phase.*