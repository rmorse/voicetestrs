# Session Resume - VoiceTextRS Project

## Quick Context
Building an offline voice-to-notes transcription app in **Rust** on **Windows**.

## Current Status (August 9, 2025)
✅ **Phase 1 COMPLETE** - Core audio recording
✅ **Phase 2 COMPLETE** - Whisper transcription integration
✅ **Phase 3 COMPLETE** - System integration (tray, hotkeys, notifications)
✅ **Phase 4 COMPLETE** - Tauri GUI application

### What's Working:
- Audio recording with CPAL (16kHz mono WAV)
- Whisper.cpp v1.7.6 binary integration
- Full transcription pipeline
- CLI commands for recording and transcription
- System tray with dynamic icon
- Global hotkeys (Ctrl+Shift+R for recording)
- Desktop notifications
- Background service mode
- **NEW: Tauri GUI with React frontend**
- **NEW: Dynamic port selection (no conflicts!)**
- **NEW: Real-time transcription display**

## Your Hardware
- **Microphone**: Headset (4- OpenRun Pro by Shokz) - 16kHz mono (DEFAULT)
- **Alternative**: HDMI (Cam Link 4K) - 48kHz stereo

## Whisper Setup (COMPLETED)
- Using whisper.cpp v1.7.6 (downloaded from ggml-org/whisper.cpp)
- Binary location: `whisper/Release/whisper-cli.exe`
- Model: `whisper/models/ggml-base.en.bin` (74MB)
- Working transcription tested successfully

## Running the Application

### GUI Mode (Tauri) - RECOMMENDED
```bash
cd tauri
npm run tauri:dev  # Development mode
npm run tauri:build # Production build
```

### CLI Commands (All Working!)
```bash
# Run in background mode with system tray
cargo run -- --background

# List audio devices
cargo run -- --list-devices

# Test recording only (X seconds)
cargo run -- --test 5

# Record and transcribe (X seconds)
cargo run -- --record 5

# Transcribe existing audio file
cargo run -- --transcribe path/to/audio.wav
```

## Hotkeys (Background Mode)
- **Ctrl+Shift+R** - Toggle recording on/off
- **Ctrl+Shift+N** - Quick note (10 sec recording)
- **Ctrl+Shift+V** - Show window (placeholder)

## Key Implementation Details
- **Whisper Integration**: Using subprocess calls to whisper-cli.exe
- **Why not whisper-rs?**: Rust 2024 edition compatibility issues with unsafe extern blocks
- **Audio Format**: 16kHz mono WAV (optimal for Whisper)
- **File Organization**: `notes/YYYY/YYYY-MM-DD/HHMMSS-voice-note.wav`

## Architecture Decisions
- **Language**: Rust (performance, single binary)
- **Audio**: CPAL 0.16.0
- **Transcription**: whisper.cpp binary (subprocess)
- **Future UI**: Tauri 2.0 (for Android support)
- **Platform**: Windows-first, cross-platform architecture

## Project Architecture

### Tauri Integration (NEW!)
The project now features a complete Tauri GUI application with:
- **Frontend**: React + Vite for modern UI
- **Backend**: Rust Tauri for native integration
- **IPC**: Commands bridge frontend to core functionality
- **Dynamic Ports**: Automatic port selection prevents conflicts

### Key Technical Solutions
1. **Dynamic Port Selection**: Using `portpicker` crate to find available ports
2. **Path Resolution**: Whisper binary path handles multiple working directories
3. **Tauri Plugin Localhost**: Ensures proper localhost server handling
4. **Process Management**: Vite dev server spawned from Rust for unified startup

### Phase 5: Voice Activity Detection
- [ ] Auto-start/stop recording
- [ ] Silence detection
- [ ] Audio level monitoring

### Phase 6: Distribution
- [ ] Windows installer
- [ ] Auto-updates
- [ ] Settings persistence

## Known Issues & Solutions
1. ✅ **SOLVED**: whisper-rs Rust 2024 compatibility → Using whisper.cpp binary
2. ✅ **SOLVED**: CMake requirement → Not needed with binary approach
3. ✅ **SOLVED**: Windows Defender flags → Fixed in previous session
4. ✅ **SOLVED**: Hotkeys not working → Switched from global-hotkey to win-hotkeys 0.5.1

## Project Structure
```
voicetextrs/
├── src/                      # Core Rust application
│   ├── core/
│   │   ├── audio.rs          # Audio recording (COMPLETE)
│   │   ├── transcription.rs  # Whisper integration (COMPLETE)
│   │   ├── config.rs         # Configuration (placeholder)
│   │   └── notes.rs          # Note management (placeholder)
│   ├── platform/             # Platform-specific (COMPLETE)
│   │   ├── tray.rs          # System tray
│   │   ├── hotkeys.rs       # Global hotkeys
│   │   └── notifications.rs # Desktop notifications
│   └── main.rs              # CLI interface (COMPLETE)
├── tauri/                    # Tauri GUI application (COMPLETE)
│   ├── src/                 # React frontend
│   ├── src-tauri/           # Tauri backend
│   └── package.json         # Node dependencies
├── whisper/
│   ├── Release/
│   │   └── whisper-cli.exe  # Whisper binary
│   └── models/
│       └── ggml-base.en.bin # Base English model
├── notes/                    # Recorded audio & transcriptions
└── Cargo.toml               # Dependencies configured

```

## Quick Questions Answered
1. **CMake installed?** Yes, but not needed anymore
2. **Whisper working?** Yes, using whisper.cpp binary
3. **Audio recording issues?** None, working perfectly

---
**Project Root**: D:\projects\claude\voicetextrs
**Last Updated**: August 9, 2025
**Phase 1**: ✅ COMPLETE (Audio Recording)
**Phase 2**: ✅ COMPLETE (Transcription)
**Phase 3**: ✅ COMPLETE (System Integration)
**Phase 4**: ✅ COMPLETE (Tauri UI with dynamic ports!)
**Phase 5**: Ready to start (Voice Activity Detection)