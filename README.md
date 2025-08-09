# VoiceTextRS

An offline voice-to-notes transcription application built with Rust, featuring OpenAI's Whisper for high-quality speech recognition.

## Features

- 🎤 **Multiple Recording Modes**: Push-to-talk, toggle, and voice activity detection
- 🔤 **Offline Transcription**: Uses Whisper for completely offline speech-to-text
- 📝 **Markdown Notes**: Automatically creates organized, timestamped markdown notes
- ⌨️ **Global Hotkeys**: System-wide keyboard shortcuts for quick recording
- 🖥️ **System Tray**: Minimal UI with system tray integration
- 🚀 **High Performance**: Built with Rust for speed and efficiency
- 🔒 **Privacy-First**: All processing happens locally, no cloud dependencies

## Quick Start

### Prerequisites

- Rust 1.75+ (install from [rustup.rs](https://rustup.rs/))
- Visual Studio 2022 Build Tools (Windows)
- Git

### Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/voicetextrs.git
cd voicetextrs

# Build the project
cargo build --release

# Run the application
cargo run --release
```

### Basic Usage

1. Press `Ctrl+Space` to start recording (default hotkey)
2. Speak your note
3. Release to stop recording and begin transcription
4. Find your transcribed note in the `notes/` folder

## Configuration

Edit `config.toml` to customize:
- Recording hotkeys
- Whisper model (tiny/base/small/medium/large)
- Audio settings
- Storage location
- UI preferences

## Project Structure

```
voicetextrs/
├── src/
│   ├── core/           # Core functionality (audio, transcription, notes)
│   ├── platform/       # Platform-specific code
│   └── ui/             # User interface (future Tauri integration)
├── models/             # Whisper models (auto-downloaded)
├── notes/              # Generated markdown notes
└── config.toml         # User configuration
```

## Development

See [plan.md](plan.md) for the detailed implementation plan and [background-research.md](background-research.md) for technical research notes.

### Running Tests

```bash
cargo test
```

### Building for Distribution

```bash
# Windows installer
cargo install cargo-wix
cargo wix

# Or use Tauri (when UI is ready)
cargo tauri build
```

## Roadmap

- [x] Core audio recording (CPAL working!)
- [ ] Whisper integration (needs CMake for whisper-rs)
- [ ] System tray implementation
- [ ] Global hotkey support
- [ ] Tauri UI
- [ ] Voice Activity Detection
- [ ] Android support (via Tauri 2.0)

## Current Status

✅ **Phase 1 Complete**: Core audio recording is working!
- Audio capture with CPAL
- WAV file generation with proper format (16kHz mono)
- CLI interface for testing
- Directory structure with date organization

### Known Requirements
- **CMake**: Required for building whisper-rs. Install from https://cmake.org/download/
- **Visual Studio Build Tools**: Already installed ✓

## License

MIT OR Apache-2.0

## Acknowledgments

- [OpenAI Whisper](https://github.com/openai/whisper) for speech recognition
- [whisper.cpp](https://github.com/ggerganov/whisper.cpp) for efficient C++ implementation
- [RustAudio](https://github.com/RustAudio) for CPAL and audio tools

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

---

**Note**: This project is under active development. See the [plan.md](plan.md) file for current status and next steps.