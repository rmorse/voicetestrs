# Offline Voice-to-Notes Transcription App

## Project Overview

A desktop application that captures audio from the microphone, transcribes it using offline Whisper, and saves the transcriptions as organized markdown notes. Built with Rust for performance, reliability, and cross-platform compatibility.

## Current Status (August 9, 2025)
- ✅ **Phase 1: Audio Recording** - COMPLETE
- ✅ **Phase 2: Transcription** - COMPLETE (using whisper.cpp binary)
- ⏳ **Phase 3: System Integration** - Next
- ⏳ **Phase 4: UI** - Pending
- ⏳ **Phase 5: VAD** - Pending
- ⏳ **Phase 6: Distribution** - Pending

## Core Requirements

- Real-time or near-real-time voice capture
- Offline transcription using Whisper
- Automatic note creation and organization in markdown format
- Cross-platform potential (initially Windows, future Android via Tauri 2.0)
- Both push-to-talk and Voice Activity Detection modes

## Tech Stack (Rust Implementation)

### Core Technology Decisions
- **Language**: Rust (performance, safety, single binary distribution)
- **UI Framework**: Tauri 2.0 (cross-platform, Android support in beta)
- **Audio**: CPAL for cross-platform audio I/O
- **Transcription**: whisper-rs bindings (with fallback to binary)
- **Platform**: Windows-first development, cross-platform architecture

### Dependencies (Latest Stable Versions)

```toml
[dependencies]
# Core Audio & Processing
cpal = "0.16.0"              # Cross-platform audio I/O
hound = "3.5.1"              # WAV file reading/writing
whisper-rs = "0.14.4"        # Whisper bindings

# Async Runtime
tokio = { version = "1.47.1", features = ["full"] }

# UI & System Integration
tauri = { version = "2.7.0", features = ["dialog", "notification", "system-tray"] }
tray-icon = "0.21.1"         # System tray
global-hotkey = "0.7.0"      # Global hotkeys
notify-rust = "4.11.7"       # Desktop notifications
winit = "0.30.12"            # Window management (for Tauri)

# Utilities
serde = { version = "1.0.219", features = ["derive"] }
serde_json = "1.0.136"
chrono = { version = "0.4.41", features = ["serde"] }
anyhow = "1.0.98"
clap = { version = "4.5.43", features = ["derive"] }
directories = "6.0.0"
tracing = "0.1"
tracing-subscriber = "0.3"

# Windows-specific
[target.'cfg(windows)'.dependencies]
windows = { version = "0.61.3", features = [
    "Win32_Foundation",
    "Win32_System_Com",
    "Win32_UI_Shell",
] }
```

## Architecture

### Layered Architecture Design

```
┌─────────────────────────────────────────────────────────────┐
│                    User Interface Layer                      │
│                  (Tauri 2.0 + Web Frontend)                 │
│              React/Vue/Vanilla JS + HTML/CSS                │
├─────────────────────────────────────────────────────────────┤
│                      Tauri IPC Bridge                        │
│                  (Commands & Event System)                   │
├─────────────────────────────────────────────────────────────┤
│                   Application Core (Rust)                    │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │Audio Module │  │ Transcription │  │  Note Manager    │  │
│  │   (cpal)    │  │  (whisper-rs) │  │   (filesystem)   │  │
│  └─────────────┘  └──────────────┘  └──────────────────┘  │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │ Hotkey Mgr  │  │  Config Mgr   │  │  Task Queue      │  │
│  │(global-hotkey)│ │(serde/TOML)  │  │   (tokio)        │  │
│  └─────────────┘  └──────────────┘  └──────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│                 Platform Abstraction Layer                   │
│        Windows API │ System Tray │ Notifications            │
├─────────────────────────────────────────────────────────────┤
│                    Operating System                          │
│     WASAPI Audio │ File System │ Process Management         │
└─────────────────────────────────────────────────────────────┘
```

### Module Responsibilities

**Core Library (`src/core/`)**
- `audio.rs`: Audio capture, recording, VAD
- `transcription.rs`: Whisper integration, model management
- `notes.rs`: Markdown generation, file organization
- `config.rs`: Settings management, persistence

**Platform Layer (`src/platform/`)**
- `windows.rs`: Windows-specific implementations
- `hotkeys.rs`: Global hotkey handling
- `tray.rs`: System tray integration
- `notifications.rs`: Desktop notifications

**UI Layer (`src/ui/` & `tauri/`)**
- Tauri backend commands
- Frontend communication
- State management

## Key Features

### 1. Audio Capture
- **Recording Modes**: 
  - Push-to-Talk (hold key to record)
  - Toggle recording (press to start/stop)
  - Voice Activity Detection (automatic)
- **Global Hotkey**: Configurable (default: Ctrl+Space)
- **Audio Monitoring**: Real-time level visualization
- **Audio Format**: 16kHz mono WAV (optimal for Whisper)
- **Buffer Management**: Ring buffer with configurable size

#### Rust Implementation Details:
```rust
// Audio configuration
const SAMPLE_RATE: u32 = 16000;  // Whisper's optimal rate
const CHANNELS: u16 = 1;          // Mono
const BUFFER_SIZE: usize = 1024;  // ~64ms chunks

// Using cpal for cross-platform audio
let config = StreamConfig {
    channels: CHANNELS,
    sample_rate: SampleRate(SAMPLE_RATE),
    buffer_size: BufferSize::Fixed(BUFFER_SIZE),
};

// Recording modes enum
enum RecordingMode {
    PushToTalk,
    Toggle,
    VoiceActivityDetection,
}
```

#### Platform Support:
- **Windows**: WASAPI (default), ASIO (optional for low latency)
- **Linux**: ALSA, PulseAudio
- **macOS**: CoreAudio (future)

### 2. Transcription Pipeline
- **Processing Mode**: Async queue-based processing
- **Model Selection**:
  - Tiny (39M): Fast processing, good for quick notes
  - Base (74M): Recommended - best speed/accuracy balance
  - Small (244M): Maximum accuracy for important recordings
- **Integration Options**:
  - Primary: whisper-rs bindings (native Rust)
  - Fallback: Whisper binary via subprocess
- **Output Processing**:
  - Parse segments with timestamps
  - Extract confidence scores
  - Handle multiple languages

#### Rust Implementation with whisper-rs:
```rust
use whisper_rs::{WhisperContext, WhisperContextParameters, FullParams, SamplingStrategy};

// Initialize Whisper context
let ctx = WhisperContext::new_with_params(
    model_path,
    WhisperContextParameters::default()
)?;

// Configure parameters
let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
params.set_n_threads(4);
params.set_language(Some("en"));
params.set_print_special(false);
params.set_print_progress(false);
params.set_print_timestamps(true);

// Process audio
ctx.full(params, &audio_data)?;

// Extract results
let num_segments = ctx.full_n_segments()?;
for i in 0..num_segments {
    let segment = ctx.full_get_segment_text(i)?;
    let start = ctx.full_get_segment_t0(i)?;
    let end = ctx.full_get_segment_t1(i)?;
}
```

#### Error Handling:
- Model validation on startup
- Timeout handling with tokio
- Graceful fallback to binary if bindings fail
- Queue persistence for retry logic

### 3. Note Management
- **File Structure** (Date-based organization):
  ```
  notes/
  ├── 2024/
  │   ├── 2024-01-15/
  │   │   ├── 143022-voice-note.md
  │   │   ├── 143022-voice-note.wav
  │   │   ├── 161245-quick-thought.md
  │   │   └── 161245-quick-thought.wav
  │   └── 2024-01-16/
  │       └── ...
  └── _archive/  # Older recordings can be moved here
  ```

#### File Naming Convention:
```python
# Format: HHMMSS-descriptive-name.ext
filename = f"{timestamp.strftime('%H%M%S')}-voice-note"
# First 5 words as filename: 143022-meeting-about-project-deadline.md
```

#### Directory Creation:
- Create year/date folders automatically
- Use os.makedirs(exist_ok=True)
- Handle permissions errors gracefully
- Default to user's Documents folder
- **Markdown Format**:
  ```markdown
  ---
  created: 2024-01-15T14:30:22Z
  duration: 45s
  confidence: 0.92
  model: base
  audio_file: 143022-voice-note.wav
  ---
  
  # Voice Note - 2:30 PM
  
  [Transcribed content here...]
  ```

### 4. User Interface (Tauri + Web)
- **System Tray**:
  - Dynamic icon (idle/recording/processing)
  - Quick access menu
  - Recording controls
  - Settings access
- **Main Window** (Tauri WebView):
  - Modern web-based UI
  - Real-time recording visualization
  - Note history and search
  - Settings panel
- **Features**:
  - Dark/Light theme
  - Keyboard shortcuts
  - Drag-and-drop audio files
  - Export options

#### Tauri Commands:
```rust
#[tauri::command]
async fn start_recording(state: State<'_, AppState>) -> Result<(), String> {
    state.audio_manager.start_recording().await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_recent_notes(count: usize) -> Result<Vec<Note>, String> {
    // Return recent notes for display
}

#[tauri::command]
async fn update_settings(settings: Settings) -> Result<(), String> {
    // Persist settings changes
}
```

## Implementation Phases

### Phase 1: Core Audio Module (Week 1)
- [ ] Set up Rust project structure
- [ ] Implement CPAL audio capture
- [ ] WAV file writing with hound
- [ ] Push-to-talk recording
- [ ] Basic CLI for testing

### Phase 2: Transcription Integration (Week 2)
- [ ] Integrate whisper-rs bindings
- [ ] Model download and management
- [ ] Async processing queue (tokio)
- [ ] Markdown note generation
- [ ] Error handling and fallbacks

### Phase 3: System Integration (Week 3)
- [ ] System tray with tray-icon
- [ ] Global hotkey handling
- [ ] Desktop notifications
- [ ] Configuration management (TOML)
- [ ] Windows-specific optimizations

### Phase 4: Tauri UI Development (Week 4)
- [ ] Tauri project setup
- [ ] Web UI design and implementation
- [ ] IPC command handlers
- [ ] Real-time status updates
- [ ] Settings interface

### Phase 5: Advanced Features (Week 5-6)
- [ ] Voice Activity Detection
- [ ] Recording mode toggle
- [ ] Note search and filtering
- [ ] Audio compression options
- [ ] Export functionality

### Phase 6: Polish & Distribution (Week 7-8)
- [ ] Performance optimization
- [ ] Windows installer (MSI/NSIS)
- [ ] Auto-updater integration
- [ ] Documentation
- [ ] Testing suite

## Project Structure

```
voicetextrs/
├── Cargo.toml                 # Project manifest
├── Cargo.lock                # Dependency lock file
├── README.md                 # Project documentation
├── config.toml               # Default configuration
├── src/
│   ├── main.rs              # Application entry point
│   ├── lib.rs               # Library root
│   ├── core/
│   │   ├── mod.rs
│   │   ├── audio.rs         # Audio capture with CPAL
│   │   ├── transcription.rs # Whisper integration
│   │   ├── notes.rs         # Note management
│   │   └── config.rs        # Configuration handling
│   ├── platform/
│   │   ├── mod.rs
│   │   ├── windows.rs       # Windows-specific code
│   │   ├── hotkeys.rs       # Global hotkey handling
│   │   ├── tray.rs          # System tray
│   │   └── notifications.rs # Desktop notifications
│   └── ui/
│       ├── mod.rs
│       └── commands.rs      # Tauri command handlers
├── tauri/
│   ├── tauri.conf.json      # Tauri configuration
│   └── icons/               # Application icons
├── web/                      # Frontend code
│   ├── index.html
│   ├── src/
│   │   ├── main.js
│   │   ├── App.vue/jsx      # Main app component
│   │   └── components/
│   └── public/
├── models/                   # Whisper models
│   └── .gitignore
└── notes/                    # Generated notes
    └── .gitignore
```

## Development Workflow

### Initial Setup
```bash
# Create new Rust project
cargo new voicetextrs
cd voicetextrs

# Add dependencies (copy from plan)
# Edit Cargo.toml

# Initialize Tauri
cargo install create-tauri-app
cargo create-tauri-app

# Test basic functionality
cargo run
```

### Incremental Development
1. **Start with CLI**: Test core audio/transcription
2. **Add system integration**: Tray, hotkeys, notifications
3. **Build UI last**: Tauri frontend after core is stable
4. **Test continuously**: Unit tests for each module

## Technical Challenges & Solutions

### 1. Global Hotkey on Different OS
**Challenge**: Platform-specific hotkey APIs
**Solution**: 
- Use `keyboard` library for cross-platform support
- Fallback to window-focus based recording

#### Platform-Specific Implementation:
```python
# Windows (no issues)
keyboard.add_hotkey('ctrl+space', on_hotkey)

# Linux (requires root)
# Alternative using pynput (no root required):
from pynput import keyboard
listener = keyboard.GlobalHotKeys({'<ctrl>+<space>': on_hotkey})

# macOS (requires accessibility permissions)
# Show dialog to guide user through System Preferences
```

### 2. System Tray Integration
**Challenge**: Different tray APIs per OS
**Solution**: 
- `pystray` handles Windows/Linux/Mac
- Consistent menu structure across platforms

### 3. Background Transcription
**Challenge**: UI freeze during Whisper processing
**Solution**:
- Use threading.Thread for Whisper subprocess
- Queue system for multiple recordings
- System notifications for completion

### 4. Audio File Management
**Challenge**: Disk space with many WAV files
**Solution**:
- Compression options in settings
- Auto-archive old recordings
- Periodic cleanup reminders

## Development Environment Setup

### Prerequisites
- **Rust**: Latest stable (1.75+)
  ```bash
  # Install Rust
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  # Or on Windows: Download from https://rustup.rs/
  ```
- **Build Tools**:
  - Windows: Visual Studio 2022 Build Tools
  - Linux: `build-essential`, `libasound2-dev`
- **Node.js**: v18+ (for Tauri frontend)
- **Whisper Models**: Download separately or auto-download

### Whisper Setup Options:

#### Option 1: whisper-rs (Recommended)
```bash
# Models will be downloaded automatically or manually place in models/
# Supported formats: GGML, GGUF
```

#### Option 2: Standalone Binary (Fallback)
```bash
# Download from: https://github.com/Purfview/whisper-standalone-win/releases
# Extract to project whisper/ folder
```

### Audio Device Testing:
```rust
// List available audio devices
use cpal::traits::{DeviceTrait, HostTrait};

fn list_devices() {
    let host = cpal::default_host();
    for device in host.input_devices().unwrap() {
        println!("Input device: {}", device.name().unwrap());
    }
}
```

## Testing Strategy

### Unit Tests
- Audio capture module
- Whisper output parsing
- Note formatting

### Integration Tests
- End-to-end recording → transcription → storage
- UI interaction tests
- Performance benchmarks

### User Testing
- Different microphone types
- Various acoustic environments
- Long-form vs short recordings

## Future Enhancements

1. **Cloud Sync**: Optional encrypted backup
2. **Multi-language**: Automatic language detection
3. **Speaker Diarization**: Identify multiple speakers
4. **Smart Summaries**: Use LLM for note summarization
5. **Voice Commands**: "New note", "Stop recording"
6. **Mobile Companion**: Android/iOS apps
7. **Collaboration**: Share notes with team members

## Security Considerations

- Local-only processing (no cloud dependency)
- Optional encryption for sensitive notes
- Secure cleanup of temporary audio files
- Configurable auto-deletion of old recordings

## Performance Targets

- Audio latency: <100ms
- Transcription speed: 2x realtime (minimum)
- Memory usage: <200MB idle, <500MB active
- Startup time: <2 seconds

## Success Metrics

- Transcription accuracy: >90% for clear speech
- User satisfaction: Faster than manual typing
- Reliability: <0.1% crash rate
- Battery efficiency: Minimal impact on laptops

## Configuration

### config.toml Structure:
```toml
[audio]
sample_rate = 16000
channels = 1
buffer_size = 1024
device = "default"  # or specific device name

[recording]
mode = "push_to_talk"  # or "toggle" or "vad"
max_duration_seconds = 300
auto_stop_silence_ms = 2000

[hotkeys]
record = "Ctrl+Space"
stop = "Escape"

[whisper]
model = "base"  # tiny, base, small, medium, large
language = "en"
threads = 4

[storage]
notes_directory = "./notes"
keep_audio_files = true
auto_archive_days = 30
compression = false

[ui]
theme = "dark"  # or "light" or "auto"
minimize_to_tray = true
show_notifications = true
```

## MVP Feature Set (Version 1.0)

### Must Have:
- [x] Push-to-talk recording (Ctrl+Space)
- [x] Post-recording Whisper transcription
- [x] Date-based note organization
- [x] System tray with minimal UI
- [x] Keep audio files alongside notes

### Nice to Have (v1.1):
- [ ] Multiple hotkey support
- [ ] Custom note templates
- [ ] Basic search functionality
- [ ] Audio compression options

### Future (v2.0):
- [ ] Full GUI for note browsing
- [ ] Voice commands
- [ ] Cloud backup
- [ ] Mobile app

## Development Timeline

### Week 1: Core Components
- Day 1-2: Audio recording module with push-to-talk
- Day 3-4: Whisper integration and testing
- Day 5-7: System tray and hotkey management

### Week 2: Integration & Polish
- Day 1-2: Note management and file organization
- Day 3-4: Settings and configuration
- Day 5-6: Testing and bug fixes
- Day 7: Package for distribution

## Sample Rust Implementation

### src/core/audio.rs
```rust
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample, Stream};
use hound::{WavWriter, WavSpec};
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use chrono::Local;
use anyhow::Result;

pub struct AudioRecorder {
    sample_rate: u32,
    channels: u16,
    buffer: Arc<Mutex<Vec<f32>>>,
    stream: Option<Stream>,
}

impl AudioRecorder {
    pub fn new() -> Result<Self> {
        Ok(Self {
            sample_rate: 16000,
            channels: 1,
            buffer: Arc::new(Mutex::new(Vec::new())),
            stream: None,
        })
    }

    pub fn start_recording(&mut self) -> Result<()> {
        let host = cpal::default_host();
        let device = host.default_input_device()
            .ok_or_else(|| anyhow::anyhow!("No input device available"))?;
        
        let config = cpal::StreamConfig {
            channels: self.channels,
            sample_rate: cpal::SampleRate(self.sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        let buffer = Arc::clone(&self.buffer);
        buffer.lock().unwrap().clear();

        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &_| {
                buffer.lock().unwrap().extend_from_slice(data);
            },
            |err| eprintln!("Audio error: {}", err),
            None,
        )?;
        
        stream.play()?;
        self.stream = Some(stream);
        Ok(())
    }

    pub fn stop_recording(&mut self) -> Result<PathBuf> {
        // Stop stream
        if let Some(stream) = self.stream.take() {
            stream.pause()?;
        }

        // Generate filename with timestamp
        let timestamp = Local::now();
        let date_dir = PathBuf::from("notes")
            .join(timestamp.format("%Y").to_string())
            .join(timestamp.format("%Y-%m-%d").to_string());
        
        std::fs::create_dir_all(&date_dir)?;
        
        let filename = format!("{}-voice-note.wav", 
            timestamp.format("%H%M%S"));
        let filepath = date_dir.join(filename);

        // Write WAV file
        let spec = WavSpec {
            channels: self.channels,
            sample_rate: self.sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut writer = WavWriter::create(&filepath, spec)?;
        let buffer = self.buffer.lock().unwrap();
        
        for &sample in buffer.iter() {
            let amplitude = (sample * i16::MAX as f32) as i16;
            writer.write_sample(amplitude)?;
        }
        
        writer.finalize()?;
        Ok(filepath)
    }
}
```


## Getting Started

### Quick Start
```bash
# Clone the repository
git clone https://github.com/yourusername/voicetextrs.git
cd voicetextrs

# Build the project
cargo build --release

# Run the application
cargo run --release

# Run tests
cargo test
```

### Building for Distribution
```bash
# Windows (MSI installer)
cargo install cargo-wix
cargo wix

# Cross-platform (Tauri bundle)
cargo tauri build
```

## Best Practices & Common Issues

### Rust-Specific Considerations
1. **Memory Safety**: Use Arc<Mutex<>> for shared audio buffers
2. **Error Handling**: Use Result<T, E> and anyhow for errors
3. **Async Operations**: Use tokio for concurrent tasks
4. **Resource Management**: RAII ensures proper cleanup
5. **Cross-platform**: Use cfg attributes for OS-specific code

### Windows-Specific
1. **WASAPI Latency**: Configure buffer sizes appropriately
2. **Permissions**: Request admin for global hotkeys if needed
3. **Antivirus**: Sign binaries to avoid false positives

## Testing Checklist

### Core Functionality
- [ ] Audio recording with CPAL
- [ ] WAV file generation (16kHz mono)
- [ ] Push-to-talk mode
- [ ] Toggle recording mode
- [ ] Voice Activity Detection

### Transcription
- [ ] whisper-rs integration
- [ ] Model loading and caching
- [ ] Async processing queue
- [ ] Error handling and retries

### System Integration
- [ ] Global hotkeys work
- [ ] System tray functionality
- [ ] Desktop notifications
- [ ] File paths on Windows

### UI/UX
- [ ] Tauri window rendering
- [ ] IPC commands work
- [ ] Settings persistence
- [ ] Theme switching

### Performance
- [ ] Memory usage < 200MB idle
- [ ] Audio latency < 100ms
- [ ] No memory leaks
- [ ] Clean shutdown

---

## Session Context & Decisions (August 2025)

### Key Decisions Made
1. **Switched from Python to Rust** - For performance, single binary distribution, and memory safety
2. **Chose Tauri 2.0 over other UI frameworks** - Best cross-platform support with Android beta
3. **Using whisper-rs bindings** - Native Rust integration, with binary fallback if needed
4. **Windows-first development** - Primary platform is Windows, not WSL
5. **Recording modes** - Implementing both push-to-talk AND VAD with toggle option

### Important Context
- **Host System**: Windows (native), not WSL - better API access
- **Crate Versions**: All verified via crates.io API (August 2025)
- **WebFetch Issue**: crates.io returns 404 with WebFetch tool, use PowerShell/curl instead
- **Architecture**: Clean separation between core/platform/UI for future portability

### Next Immediate Steps
1. ✅ Create Cargo.toml with dependencies from this plan
2. ✅ Set up basic project structure as outlined
3. ✅ Implement Phase 1 (Core Audio) - start with simple CLI test
4. ✅ Test CPAL audio capture on Windows with default microphone

### Session Progress (August 9, 2025)
- **Completed Phase 1**: Core audio recording fully functional
- **Tested**: Successfully recorded 3-second audio with Shokz headset
- **Issue Found**: whisper-rs requires CMake (not installed)
- **Next**: Either install CMake or implement whisper binary fallback

### Alternative UI Frameworks Considered
- **Slint (1.12.1)**: Native performance, Android support, but newer/smaller community
- **Dioxus (0.6.3)**: React-like, multiple targets, but mobile support is newer
- **EGUI (0.32.0)**: Immediate mode, simple, but experimental mobile support

### Why These Specific Crates
- **cpal**: Most mature cross-platform audio library for Rust
- **whisper-rs**: Active maintenance (updated July 2025), good FFI bindings
- **tray-icon**: Works well with both Tauri and standalone apps
- **global-hotkey**: Cross-platform hotkey support without admin requirements

---

*This plan is a living document and will be updated as the project evolves.*