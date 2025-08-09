# VoiceTextRS 🎤

An offline voice-to-notes transcription application built with Rust, featuring OpenAI's Whisper for high-quality speech recognition. Record with hotkeys, transcribe offline, and manage notes - all from your system tray!

## ✨ Features

- 🎤 **Voice Recording**: High-quality audio capture at 16kHz (optimal for Whisper)
- 🤖 **Offline Transcription**: Uses Whisper.cpp for completely offline speech-to-text
- 📝 **Organized Notes**: Automatically saves recordings and transcriptions by date
- ⌨️ **Global Hotkeys**: System-wide shortcuts for hands-free recording
- 🖥️ **System Tray**: Runs in background with easy access from system tray
- 🔔 **Desktop Notifications**: Get notified when recording starts/stops and transcription completes
- 🚀 **High Performance**: Built with Rust for speed and single-binary distribution
- 🔒 **Privacy-First**: All processing happens locally, no cloud dependencies

## 🚀 Quick Start

### Prerequisites

- Windows 10/11 (primary platform)
- Rust 1.75+ (install from [rustup.rs](https://rustup.rs/))
- Visual Studio 2022 Build Tools (for Windows compilation)
- ~200MB disk space for Whisper model

### Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/voicetextrs.git
cd voicetextrs

# Build the project
cargo build --release

# Download Whisper binary (see WHISPER_SETUP.md for details)
# 1. Download from: https://github.com/ggerganov/whisper.cpp/releases
# 2. Extract to whisper/Release/ folder
# 3. Model will auto-download on first use
```

## 📖 Usage

### Background Mode (Recommended)

```bash
# Run in system tray
cargo run -- --background
```

**Available Hotkeys:**
- `Ctrl+Shift+R` - Toggle recording on/off
- `Ctrl+Shift+N` - Quick note (10-second recording)
- `Ctrl+Shift+V` - Show window (coming soon)

**System Tray Features:**
- Blue microphone icon (turns red when recording)
- Right-click menu for controls
- Enable/disable hotkeys
- Exit application

### CLI Mode

```bash
# Record and transcribe for 5 seconds
cargo run -- --record 5

# Transcribe existing audio file
cargo run -- --transcribe path/to/audio.wav

# List available audio devices
cargo run -- --list-devices

# Test recording without transcription
cargo run -- --test 3
```

## 📁 Project Structure

```
voicetextrs/
├── src/
│   ├── core/           # Core functionality
│   │   ├── audio.rs        # Audio recording (CPAL)
│   │   ├── transcription.rs # Whisper integration
│   │   ├── notes.rs        # Note management
│   │   └── config.rs       # Configuration
│   ├── platform/       # Platform-specific code
│   │   ├── tray.rs         # System tray
│   │   ├── hotkeys.rs      # Global hotkeys
│   │   └── notifications.rs # Desktop notifications
│   ├── app.rs          # Main application controller
│   └── main.rs         # Entry point
├── whisper/
│   ├── Release/        # Whisper.cpp binaries
│   │   └── whisper-cli.exe
│   └── models/         # Whisper models
│       └── ggml-base.en.bin (74MB)
├── notes/              # Your recordings and transcriptions
│   └── YYYY/
│       └── YYYY-MM-DD/
│           ├── HHMMSS-voice-note.wav
│           └── HHMMSS-voice-note.txt
└── Cargo.toml          # Rust dependencies
```

## ⚙️ Configuration

### Audio Settings
- **Sample Rate**: 16kHz (optimal for Whisper)
- **Channels**: Mono
- **Format**: WAV
- **Default Device**: Auto-detected (can be specified with `--device`)

### Whisper Models

Available models (download as needed):
- `tiny.en` (39 MB) - Fastest, lower quality
- `base.en` (74 MB) - **Default**, good balance
- `small.en` (244 MB) - Better quality
- `medium.en` (769 MB) - High quality
- `large` (1550 MB) - Best quality, slowest

## 🛠️ Development

### Building from Source

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test
```

### Current Implementation Status

✅ **Phase 1: Audio Recording** - Complete
- CPAL integration for cross-platform audio
- WAV file generation with proper formatting
- Device selection and listing

✅ **Phase 2: Transcription** - Complete  
- Whisper.cpp integration via subprocess
- Automatic model management
- JSON output parsing

✅ **Phase 3: System Integration** - Complete
- System tray with dynamic icon
- Global hotkeys for recording control
- Desktop notifications
- Background service mode

⏳ **Phase 4: UI** - Next
- Tauri-based GUI
- Transcription history viewer
- Settings panel

⏳ **Phase 5: Voice Activity Detection** - Planned
- Auto-start/stop recording
- Silence detection

## 🐛 Troubleshooting

### Common Issues

**"Whisper binary not found"**
- Download whisper.cpp from [releases](https://github.com/ggerganov/whisper.cpp/releases)
- Extract to `whisper/Release/` folder
- Ensure `whisper-cli.exe` exists

**"No input device available"**
- Check microphone is connected
- Run `cargo run -- --list-devices` to see available devices
- Specify device with `--device "Device Name"`

**Hotkeys not working**
- Ensure no other application is using the same hotkeys
- Run as administrator if needed
- Check hotkeys are enabled in system tray menu

## 📝 Documentation

- [WHISPER_SETUP.md](WHISPER_SETUP.md) - Detailed Whisper setup instructions
- [SESSION_RESUME.md](SESSION_RESUME.md) - Current development status
- [plan.md](plan.md) - Full implementation plan
- [background-research.md](background-research.md) - Technical research notes
- [PHASE3_NOTES.md](PHASE3_NOTES.md) - System integration details

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## 📄 License

MIT OR Apache-2.0

## 🙏 Acknowledgments

- [OpenAI Whisper](https://github.com/openai/whisper) for the amazing speech recognition model
- [whisper.cpp](https://github.com/ggerganov/whisper.cpp) for the efficient C++ implementation
- [RustAudio](https://github.com/RustAudio) for CPAL audio library
- [Tauri](https://tauri.app/) for the upcoming GUI framework

---

**Built with Rust** 🦀 for performance, reliability, and single-binary distribution.