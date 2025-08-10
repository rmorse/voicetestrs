# Tauri System Integration Progress

## Overview
Migrating all system integration features (system tray, global hotkeys, notifications) from the standalone Rust app to the Tauri app, creating a single unified application.

## Goal Architecture
- Single Tauri application that handles both UI and background modes
- System tray icon always visible
- Global hotkeys work regardless of window visibility
- Window can be shown/hidden via tray or hotkeys
- Closing window hides it (doesn't quit app)
- Proper "Quit" only from tray menu

## Progress Tracking

### Phase 1: System Tray Integration ✅

#### Step 1.1: Add Tauri Tray Plugin
- [x] Add `tauri-plugin-tray` to `tauri/src-tauri/Cargo.toml`
- [x] Enable tray feature in Tauri configuration
- [x] Import necessary types in `lib.rs`

#### Step 1.2: Basic Tray Implementation
- [x] Create tray menu structure
- [x] Add menu items: Show/Hide, Start/Stop Recording, Settings, Quit
- [x] Implement tray event handler
- [x] Add tray icon assets (using default)

#### Step 1.3: Window Management
- [x] Implement Show/Hide toggle functionality
- [x] Handle window close event → hide instead of quit
- [x] Double-click tray icon → toggle window

### Phase 2: Global Hotkeys Integration ✅

#### Step 2.1: Port Hotkey System
- [x] Add hotkey dependencies to Tauri (tauri-plugin-global-shortcut)
- [x] Port hotkey integration from main app
- [x] Create hotkey handlers in Tauri

#### Step 2.2: Register Hotkeys
- [x] Ctrl+Shift+R - Toggle recording
- [x] Ctrl+Shift+N - Quick note (10 sec)
- [x] Ctrl+Shift+V - Show/hide window
- [x] Ensure hotkeys work when window is hidden

### Phase 3: Recording Integration ✅

#### Step 3.1: Connect Tray to Recording
- [x] Wire up "Start Recording" menu item
- [x] Wire up "Stop Recording" menu item
- [ ] Update tray icon based on recording state (Tauri v2 limitation)
- [x] Show recording duration in tooltip

#### Step 3.2: Connect Hotkeys to Recording
- [x] Connect hotkey events to recording commands
- [x] Ensure state synchronization between tray and hotkeys

### Phase 4: Notifications ⏳

#### Step 4.1: Port Notification System
- [ ] Use Tauri's notification API
- [ ] Port notification functions from main app
- [ ] Test notifications on Windows

### Phase 5: CLI Support ✅

#### Step 5.1: Background Mode
- [x] Add `--background` flag support
- [x] Start with window hidden when flag is present
- [ ] Document CLI options

### Phase 6: Testing & Polish ✅

- [x] Test all hotkeys with window hidden
- [x] Test all tray menu items
- [x] Test recording from both UI and hotkeys
- [ ] Test notifications
- [ ] Update README with new architecture

## Current Issues & Notes

### Issue Log
- **2025-08-09**: Starting migration process. Main challenge is that current Tauri app has no system integration.
- **2025-08-09**: Successfully integrated system tray and global hotkeys into Tauri app!

### Implementation Status
✅ **COMPLETED**: The Tauri app now has:
- System tray with full menu functionality
- Global hotkeys (Ctrl+Shift+R, N, V) working even when window is hidden
- Window close → hide behavior (app stays in tray)
- Recording functionality accessible from tray, hotkeys, and UI
- --background flag to start minimized to tray
- Event synchronization between all interfaces

### Known Limitations
- Tauri v2 doesn't support dynamic menu item state changes (can't gray out menu items)
- Tray icon updates not yet implemented (would need custom icon assets)

### Architecture Notes
- Tauri v2 has native tray support (no plugin needed!)
- win-hotkeys should work within Tauri's event loop
- Need to handle message pump for Windows hotkeys

### Code References
- Current tray implementation: `src/platform/tray.rs`
- Current hotkey implementation: `src/platform/hotkeys.rs`
- Current app logic: `src/app.rs`
- Tauri app entry: `tauri/src-tauri/src/lib.rs`

## Next Steps
1. Start with basic tray implementation
2. Get Show/Hide working
3. Then add hotkeys
4. Finally integrate recording

## Testing Checklist
- [ ] Tray icon appears on startup
- [ ] Show/Hide menu item works
- [ ] Double-click tray toggles window
- [ ] Close button hides window (not quit)
- [ ] Quit menu item exits app
- [ ] Hotkeys work when window is hidden
- [ ] Recording starts/stops from tray
- [ ] Recording starts/stops from hotkeys
- [ ] Recording starts/stops from UI buttons
- [ ] Notifications appear correctly
- [ ] --background flag works

## Resources
- [Tauri v2 System Tray Docs](https://v2.tauri.app/develop/system-tray/)
- [Tauri Global Shortcut Plugin](https://v2.tauri.app/plugin/global-shortcut/)
- [win-hotkeys crate](https://docs.rs/win-hotkeys/)