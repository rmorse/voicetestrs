# Process and Port Management Plan for VoiceTextRS

## Current Issues Analysis

### Problems Identified
1. **Ghost Processes**: Vite dev server often remains running after Tauri app shutdown
2. **Port Conflicts**: Previously used ports remain occupied, preventing restart
3. **Fragile Communication**: File-based port sharing between Node.js and Rust is unreliable
4. **Incomplete Cleanup**: Current cleanup doesn't handle all exit scenarios (SIGKILL, OS termination, etc.)
5. **Platform Inconsistencies**: Process management differs between Windows, Linux, and macOS

### Current Architecture Weaknesses
- `start-dev-server.js` spawns Vite but cleanup isn't guaranteed
- Port detection happens in Node.js, requiring file I/O for Rust to read
- No proper process group management for child processes
- Windows-specific `taskkill` may not work in all scenarios
- No recovery mechanism for stale processes from previous runs

## Proposed Solution: Rust-Managed Dev Server

### Core Strategy
Move all process management into Rust/Tauri for better control and reliability. Use Rust's stronger process management capabilities and Tauri's lifecycle hooks.

### Architecture Overview

```
┌─────────────────┐
│   Tauri App     │
│   (Main Rust)   │
├─────────────────┤
│ Process Manager │ ← Manages all child processes
├─────────────────┤
│ Port Allocator  │ ← Finds and reserves ports
├─────────────────┤
│ Vite Controller │ ← Spawns and monitors Vite
└─────────────────┘
        ↓
┌─────────────────┐
│  Vite Server    │
│  (Child Proc)   │
└─────────────────┘
```

## Implementation Plan

### Phase 1: Robust Process Management in Rust

#### 1.1 Create Process Manager Module
Create `tauri/src-tauri/src/process_manager.rs`:

```rust
use std::process::{Child, Command};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

pub struct ProcessManager {
    vite_process: Arc<Mutex<Option<Child>>>,
    shutdown_sender: Option<oneshot::Sender<()>>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            vite_process: Arc::new(Mutex::new(None)),
            shutdown_sender: None,
        }
    }
    
    pub fn spawn_vite(&mut self, port: u16) -> Result<(), Box<dyn std::error::Error>> {
        // Implementation details below
    }
    
    pub fn cleanup_all(&mut self) {
        // Force kill all managed processes
    }
}
```

#### 1.2 Add Process Group Management
Use the `command-group` crate for better process tree control:

```toml
# In Cargo.toml
command-group = "5.0"
```

This allows killing entire process trees reliably across platforms.

### Phase 2: Smart Port Allocation

#### 2.1 Port Manager Module
Create `tauri/src-tauri/src/port_manager.rs`:

```rust
use std::net::{TcpListener, SocketAddr};
use std::time::Duration;
use tokio::time::sleep;

pub struct PortManager {
    preferred_port: u16,
    max_attempts: u16,
}

impl PortManager {
    pub async fn find_available_port(&self) -> Result<u16, Box<dyn std::error::Error>> {
        // Try preferred port first
        // Fall back to OS-assigned (port 0) if needed
        // Return the allocated port
    }
    
    pub async fn wait_for_server(&self, port: u16, timeout: Duration) -> Result<(), Box<dyn std::error::Error>> {
        // Poll the port until server responds
    }
    
    pub fn release_port(&self, port: u16) {
        // Clean up any port reservations
    }
}
```

### Phase 3: Lifecycle Integration

#### 3.1 Tauri Setup Hook
Modify `lib.rs` setup function:

```rust
.setup(move |app| {
    // Initialize process manager
    let mut process_manager = ProcessManager::new();
    
    // Find available port
    let port = runtime::block_on(async {
        PortManager::new(5173).find_available_port().await
    })?;
    
    // Start Vite in development mode
    if cfg!(debug_assertions) {
        process_manager.spawn_vite(port)?;
        
        // Wait for Vite to be ready
        runtime::block_on(async {
            PortManager::new(5173).wait_for_server(port, Duration::from_secs(30)).await
        })?;
    }
    
    // Store process manager in app state
    app.manage(Arc::new(Mutex::new(process_manager)));
    
    Ok(())
})
```

#### 3.2 Window Event Handlers
Add comprehensive cleanup on all exit paths:

```rust
.on_window_event(|window, event| {
    if let tauri::WindowEvent::Destroyed = event {
        // Window is being destroyed, cleanup processes
        let process_manager = window.state::<Arc<Mutex<ProcessManager>>>();
        process_manager.lock().unwrap().cleanup_all();
    }
})
```

#### 3.3 Exit Handler
Enhance the existing exit handler:

```rust
.run(|app_handle, event| match event {
    tauri::RunEvent::Exit => {
        // Final cleanup before exit
        if let Some(manager) = app_handle.try_state::<Arc<Mutex<ProcessManager>>>() {
            manager.lock().unwrap().cleanup_all();
        }
    }
    _ => {}
})
```

### Phase 4: Cross-Platform Process Cleanup

#### 4.1 Platform-Specific Implementations

**Windows:**
```rust
#[cfg(target_os = "windows")]
fn kill_process_tree(pid: u32) {
    // Use Windows Job Objects for reliable cleanup
    // Fall back to taskkill /T /F
}
```

**Unix/Linux/macOS:**
```rust
#[cfg(unix)]
fn kill_process_tree(pid: i32) {
    // Send SIGTERM to process group
    // Follow up with SIGKILL if needed
}
```

#### 4.2 Stale Process Detection
On startup, check for and clean up stale processes:

```rust
fn cleanup_stale_processes() {
    // Check for .pid files from previous runs
    // Verify if processes are still running
    // Kill any orphaned Vite/Node processes
}
```

### Phase 5: Simplified Configuration

#### 5.1 Update vite.config.js
Remove dynamic port logic, use environment variable:

```javascript
export default defineConfig({
  server: {
    port: process.env.VITE_DEV_PORT ? parseInt(process.env.VITE_DEV_PORT) : 5173,
    strictPort: true,
    host: 'localhost'
  }
});
```

#### 5.2 Update tauri.conf.json
Remove `beforeDevCommand`, let Rust handle everything:

```json
{
  "build": {
    "frontendDist": "../dist",
    "devUrl": "http://localhost:$PORT",
    "beforeBuildCommand": "npm run build"
  }
}
```

### Phase 6: Monitoring and Recovery

#### 6.1 Process Health Monitoring
Implement continuous monitoring of child processes:

```rust
async fn monitor_vite_health(process: Arc<Mutex<Option<Child>>>) {
    loop {
        sleep(Duration::from_secs(5)).await;
        
        // Check if process is still alive
        // Restart if crashed
        // Log any issues
    }
}
```

#### 6.2 Graceful Degradation
If Vite fails to start, provide useful feedback:

```rust
enum ViteStartError {
    PortUnavailable(u16),
    ProcessSpawnFailed(String),
    TimeoutWaitingForServer,
}
```

## Implementation Steps

### Step 1: Create Rust Infrastructure (Priority: High)
1. Add `command-group` dependency to Cargo.toml
2. Create `process_manager.rs` module
3. Create `port_manager.rs` module
4. Integrate modules into `lib.rs`

### Step 2: Replace Node.js Script (Priority: High)
1. Remove `beforeDevCommand` from tauri.conf.json
2. Implement Vite spawning in Rust
3. Delete `start-dev-server.js`
4. Test on Windows, Linux, macOS

### Step 3: Add Cleanup Hooks (Priority: Critical)
1. Implement window destroy handler
2. Enhance exit handler
3. Add signal handlers for SIGTERM/SIGINT
4. Test all exit scenarios

### Step 4: Platform Testing (Priority: High)
1. Test on Windows 10/11
2. Test on Ubuntu/Debian Linux
3. Test on macOS (if available)
4. Document any platform-specific issues

### Step 5: Add Recovery Mechanisms (Priority: Medium)
1. Implement stale process detection
2. Add health monitoring
3. Create automatic restart logic
4. Add user-friendly error messages

## Alternative Approach: PM2 Integration

If the Rust-based approach proves too complex, consider using PM2 for process management:

### Pros:
- Battle-tested process management
- Built-in cleanup and monitoring
- Cross-platform support
- Easy integration

### Cons:
- Additional dependency
- Requires PM2 installation
- Less control over process lifecycle

### Implementation:
```javascript
// Use PM2 programmatically
import pm2 from 'pm2';

pm2.connect((err) => {
  pm2.start({
    script: 'vite',
    name: 'vite-dev',
    env: { VITE_DEV_PORT: port }
  });
});

// Cleanup
process.on('exit', () => {
  pm2.delete('vite-dev');
});
```

## Testing Strategy

### Test Scenarios:
1. **Normal Exit**: Close window → Verify Vite stops
2. **Force Quit**: Kill Tauri process → Verify Vite stops
3. **System Shutdown**: Shutdown/restart → No ghost processes
4. **Port Conflicts**: Start with occupied port → Finds alternative
5. **Rapid Restart**: Quick stop/start cycles → Works reliably
6. **Network Changes**: Network disconnect/reconnect → Continues working

### Verification Commands:

**Windows:**
```bash
# Check for Node processes
tasklist | findstr node

# Check port usage
netstat -ano | findstr :5173
```

**Linux/macOS:**
```bash
# Check for Node processes
ps aux | grep node

# Check port usage
lsof -i :5173
```

## Success Criteria

1. ✅ No ghost processes after any type of exit
2. ✅ Automatic port selection when default is busy
3. ✅ Works on Windows, Linux, and macOS
4. ✅ Vite starts within 5 seconds
5. ✅ Graceful error messages for failures
6. ✅ No manual cleanup required
7. ✅ Development workflow remains smooth

## Timeline

- **Week 1**: Implement Rust process management
- **Week 2**: Add cleanup hooks and testing
- **Week 3**: Platform testing and bug fixes
- **Week 4**: Documentation and edge case handling

## Conclusion

The Rust-based approach provides the most control and reliability for managing the Vite dev server. By moving process management into Tauri's Rust code, we can leverage:

1. Stronger process lifecycle guarantees
2. Direct integration with Tauri's event system  
3. Cross-platform process group management
4. Reliable cleanup on all exit paths
5. Better error handling and recovery

This solution eliminates the fragile Node.js intermediary and file-based communication, resulting in a more robust development experience that won't leave ghost processes running.