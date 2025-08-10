# Tauri Technical Notes - VoiceTextRS

## Dynamic Port Selection Solution

### The Problem
When developing with Tauri and Vite, port conflicts are common:
- Multiple developers may have different ports in use
- Running multiple instances causes conflicts
- Hardcoded ports (like 5173) may be occupied by other services
- Traditional Tauri setup requires matching ports in `tauri.conf.json` and `vite.config.js`

### Our Solution
We implemented a fully dynamic port selection system using:
1. **`portpicker` crate** - Finds available ports at runtime
2. **`tauri-plugin-localhost`** - Manages localhost server
3. **Process spawning** - Launches Vite from Rust with the selected port

### Implementation Details

#### 1. Port Selection (lib.rs)
```rust
// Find an unused port dynamically
let port = portpicker::pick_unused_port().expect("failed to find unused port");
```

#### 2. Vite Server Launch
```rust
// Start Vite with the selected port via environment variable
Command::new("cmd")
    .args(&["/C", "npm", "run", "dev"])
    .env("VITE_PORT", port.to_string())
    .current_dir("../")
    .spawn()
```

#### 3. Vite Configuration
```javascript
// vite.config.js reads port from environment
server: {
    port: process.env.VITE_PORT ? parseInt(process.env.VITE_PORT) : 5173,
    strictPort: true,
    host: 'localhost'
}
```

#### 4. Tauri Integration
```rust
// Configure Tauri to use the same port
let url = format!("http://localhost:{}", port).parse().unwrap();
context.config_mut().build.dev_url = Some(url);

// Add localhost plugin
.plugin(tauri_plugin_localhost::Builder::new(port).build())
```

### Benefits
- **Zero Configuration**: Works on any machine without port configuration
- **No Conflicts**: Automatically finds available ports
- **Developer Friendly**: No need to manually change ports
- **Production Ready**: Same code works in development and production

## Project Structure

### Directory Organization
```
voicetextrs/
├── src/                 # Core Rust application (original)
├── tauri/              # Tauri GUI application
│   ├── src/            # React frontend
│   ├── src-tauri/      # Tauri backend
│   ├── package.json    # Node dependencies
│   └── vite.config.js  # Vite configuration
└── whisper/            # Whisper binaries
```

### Key Files
- `tauri/src-tauri/src/lib.rs` - Main Tauri setup with port selection
- `tauri/src-tauri/src/commands.rs` - IPC commands for frontend-backend communication
- `tauri/src/App.jsx` - React frontend with recording controls
- `tauri/vite.config.js` - Vite configuration with dynamic port support

## Path Resolution

### Whisper Binary Discovery
The application searches for Whisper in multiple locations to handle different working directories:
```rust
let possible_paths = vec![
    PathBuf::from("whisper/Release/whisper-cli.exe"),
    PathBuf::from("../../whisper/Release/whisper-cli.exe"),
    PathBuf::from("../../../whisper/Release/whisper-cli.exe"),
];
```

This allows the app to work whether run from:
- Project root (`cargo run`)
- Tauri directory (`npm run tauri:dev`)
- Built executable location

## Dependencies

### Rust (Cargo.toml)
```toml
tauri = { version = "2.7.0" }
tauri-plugin-localhost = "2"
portpicker = "0.1"
```

### Node (package.json)
```json
"@tauri-apps/cli": "^2.7.1",
"@tauri-apps/api": "^2.7.0",
"vite": "^7.1.1",
"react": "^19.1.1"
```

## Common Issues & Solutions

### Issue: "Failed to start Vite dev server: program not found"
**Cause**: `npm` command not found when spawned from Rust
**Solution**: Use `cmd /C npm` on Windows to run through command prompt

### Issue: "Whisper binary not found"
**Cause**: Working directory different than expected
**Solution**: Search multiple relative paths for the binary

### Issue: Port already in use
**Cause**: Previous instance didn't clean up or another service using port
**Solution**: Dynamic port selection automatically finds next available

## Future Improvements

1. **WebSocket Communication**: For real-time updates without polling
2. **Settings Persistence**: Save user preferences
3. **Multi-window Support**: Allow multiple transcription windows
4. **Plugin System**: Extensible architecture for additional features

---

**Created**: August 9, 2025
**Author**: VoiceTextRS Development Team
**License**: MIT OR Apache-2.0