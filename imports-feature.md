# Imports Feature Documentation

## Status: ✅ FULLY FUNCTIONAL (2025-08-11)
The imports feature is now complete and working. Users can drop audio files into `imports/pending/` and they will be automatically processed, moved to the notes folder, and transcribed.

## Overview
The imports feature allows users to easily import external audio files into VoiceTextRS for transcription. Files are dropped into an imports folder and automatically processed by the background queue system.

## Architecture

### Folder Structure
```
voicetextrs/
├── imports/
│   ├── pending/     # Drop audio files here for import
│   └── processed/   # Metadata about processed imports
└── notes/          # Final destination for imported files
    └── YYYY/
        └── YYYY-MM-DD/
            └── HHMMSS-imported-{original_name}.wav
```

### Components

#### 1. Import Processor (`sync/imports.rs`)
- Scans `imports/pending` folder for new audio files
- Queues files for processing in the background tasks system
- Moves files to appropriate date-based folders in `notes/`
- Creates metadata records in `imports/processed`

#### 2. File Watcher (`sync/file_watcher.rs`)
- Monitors `imports/pending` folder in real-time
- Automatically queues new files when detected
- Also watches `notes/` folder for changes
- Handles file creation, modification, and deletion events

#### 3. Background Queue Manager (`queue_manager.rs`)
- Added `ProcessImport` task type for handling imports
- Processes imports with higher priority than orphaned files
- Moves files and queues them for transcription

#### 4. Periodic Sync Scheduler
- Runs filesystem sync every 5 minutes
- Ensures no files are missed
- Operates as a background task with low priority

## User Workflow

### Importing Audio Files
1. Drop audio files into `imports/pending/` folder
2. File watcher detects new files immediately
3. Files are queued as `ProcessImport` tasks
4. Background processor:
   - Moves file to `notes/YYYY/YYYY-MM-DD/` with timestamp
   - Creates transcription task
   - Records import metadata

### Supported Formats
- WAV (recommended)
- MP3
- M4A
- OGG
- FLAC
- WebM

## Implementation Details

### Task Types
```rust
enum TaskType {
    TranscribeOrphan,    // Existing audio without transcription
    TranscribeImported,  // Newly imported audio
    FileSystemSync,      // Periodic sync task
    ProcessImport,       // Import processing task
}
```

### Task Priorities
- **High (2)**: User-initiated recordings
- **Normal (1)**: Imported files, manual reprocessing
- **Low (0)**: Orphaned files, filesystem sync

### Database Schema
Imports are tracked in the `transcriptions` table with:
- `source`: Set to 'import' for imported files
- `original_name`: Preserved in metadata
- Standard fields: `id`, `audio_path`, `status`, `created_at`

## Performance Improvements

### Startup Optimization
- Removed full filesystem sync on app startup
- App loads only existing database entries
- Background sync handles new files asynchronously

### Background Processing
- All heavy operations moved to background tasks
- Main app remains responsive during imports
- Queue automatically pauses during active recording

### Real-time Updates
- File watcher provides instant detection
- UI receives events for import status
- No polling required for new files

## UI Integration

### Background Tasks Tab
Enhanced to show:
- Import task status with icons
- Original filename for imports
- Filesystem sync progress
- Real-time updates via events

### Event System
New events emitted:
- `import-queued`: When new import detected
- `transcription-modified`: When file content changes
- `transcription-deleted`: When file removed

## Configuration

### Sync Schedule
- Initial sync: 30 seconds after startup
- Periodic sync: Every 5 minutes
- Configurable via constants in `queue_manager.rs`

### File Watching
- Debounce: 2 seconds (prevents duplicate events)
- Mode: Recursive for notes, non-recursive for imports
- Platform-specific optimizations via `notify` crate

## Error Handling

### Import Failures
- Failed imports remain in `pending/` folder
- Retry logic with max 2 attempts
- Error details logged and stored in database

### File Conflicts
- Duplicate detection via ID generation
- Timestamp-based naming prevents overwrites
- Soft delete for removed files (marked in DB)

## Future Enhancements

1. **Drag-and-drop UI**: Direct import via app interface
2. **Batch processing**: Import multiple files simultaneously
3. **Format conversion**: Auto-convert unsupported formats
4. **Import history**: View all imported files with search
5. **Custom naming**: User-defined naming patterns
6. **Watch multiple folders**: Configure additional import locations

## Implementation Status (2025-08-11)

### ✅ Working Features
1. **Real-time detection** - File watcher detects files immediately when dropped
2. **Automatic processing** - Files are moved and renamed with timestamp prefix
3. **Transcription** - Imported files are automatically transcribed via Whisper
4. **Path handling** - Correct path resolution in both dev and production modes
5. **Background processing** - All operations handled by queue manager

### Test Results
Successfully tested with real audio files:
- File: `real-audio-test.wav`
- Detected in: `imports/pending/`
- Moved to: `notes/2025/2025-08-11/082419-imported-real-audio-test.wav`
- Transcribed: "This is some kind of testing the background, I think."

## Testing

### Manual Testing ✅ PASSED
1. Drop audio file in `imports/pending/` ✅
2. Verify file moves to `notes/` with correct naming ✅
3. Check Background Tasks tab shows import ✅
4. Confirm transcription completes ✅

### Edge Cases
- Large files (>100MB) - Not tested
- Unsupported formats - Handled gracefully
- Duplicate filenames - Timestamp prevents conflicts
- Concurrent imports - Queue handles sequentially
- App restart during import - Tasks persist in database

## Troubleshooting

### Common Issues

#### Files not being imported
- Check `imports/pending/` folder exists
- Verify file format is supported
- Check background queue is not paused
- Review logs for errors

#### Slow processing
- Check system resources
- Verify Whisper model is loaded
- Review queue priority settings

#### Missing transcriptions
- Check `notes/` folder permissions
- Verify database is accessible
- Review background task failures

## Related Documentation
- `background-tasks-plan.md`: Queue system design
- `transcription-sync-plan.md`: Sync architecture
- `SESSION_RESUME.md`: Current implementation status