# Transcription Synchronization Architecture Plan

## Overview
Create a robust system where the `notes/` folder is the single source of truth for all transcriptions, with the UI always reflecting the current state of the file system.

## Current State Analysis

### File Structure
```
notes/
└── YYYY/
    └── YYYY-MM-DD/
        ├── HHMMSS-voice-note.wav       # Audio file
        ├── HHMMSS-voice-note.txt       # Plain text transcription
        ├── HHMMSS-voice-note.json      # Our metadata
        └── HHMMSS-voice-note.wav.json  # Whisper raw output
```

### Metadata Format (.json)
```json
{
  "audio_file": "path/to/audio.wav",
  "text_file": "path/to/text.txt",
  "timestamp": "2025-08-10T12:12:01.889199600+02:00",
  "language": "en",
  "duration": 0.0
}
```

### Current Issues
1. UI only shows transcriptions from current session
2. No persistence across app restarts
3. Missing transcriptions for incomplete recordings
4. No way to delete/manage old transcriptions
5. No search or filtering capabilities

## Proposed Architecture

### Core Principles
1. **File System as Source of Truth** - All data comes from disk
2. **Lazy Loading** - Load metadata first, content on demand
3. **Real-time Sync** - Watch for file system changes
4. **Graceful Degradation** - Handle missing/corrupt files
5. **Performance First** - Cache intelligently, paginate when needed

## Implementation Design

### 1. Backend: Transcription Service

#### A. Core Data Structure
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcription {
    pub id: String,              // HHMMSS from filename
    pub date: String,             // YYYY-MM-DD from path
    pub timestamp: DateTime<Local>,
    pub text: Option<String>,    // Lazy loaded
    pub audio_path: PathBuf,
    pub text_path: Option<PathBuf>,
    pub metadata_path: Option<PathBuf>,
    pub duration: f64,
    pub language: String,
    pub file_size: u64,          // Audio file size
    pub status: TranscriptionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TranscriptionStatus {
    Complete,      // Has all files (.wav, .txt, .json)
    Processing,    // Has .wav but missing .txt
    Failed,        // Has .wav and error marker
    Corrupted,     // Missing required files
}
```

#### B. Transcription Manager
```rust
pub struct TranscriptionManager {
    notes_dir: PathBuf,
    cache: Arc<RwLock<HashMap<String, Transcription>>>,
    watcher: Option<FileWatcher>,
}

impl TranscriptionManager {
    // Core operations
    pub async fn load_all(&self) -> Vec<Transcription>
    pub async fn load_recent(&self, limit: usize) -> Vec<Transcription>
    pub async fn load_by_date_range(&self, start: Date, end: Date) -> Vec<Transcription>
    pub async fn get_transcription_text(&self, id: &str) -> Option<String>
    pub async fn delete_transcription(&self, id: &str) -> Result<()>
    pub async fn search(&self, query: &str) -> Vec<Transcription>
    pub async fn refresh(&self) -> Result<()>
    
    // File system operations
    fn scan_directory(&self) -> Vec<Transcription>
    fn parse_transcription_files(&self, dir: &Path) -> Option<Transcription>
    fn watch_for_changes(&mut self) -> Result<()>
}
```

### 2. File System Watcher

Use `notify` crate for cross-platform file watching:
```rust
- Watch notes/ directory recursively
- Emit events: FileCreated, FileModified, FileDeleted
- Debounce rapid changes (e.g., during transcription save)
- Update cache and notify UI of changes
```

### 3. Backend API (Tauri Commands)

#### New Commands:
```rust
#[tauri::command]
async fn load_transcriptions(
    state: State<TranscriptionManager>,
    limit: Option<usize>,
    offset: Option<usize>
) -> Result<Vec<Transcription>>

#[tauri::command]
async fn get_transcription_text(
    state: State<TranscriptionManager>,
    id: String
) -> Result<String>

#[tauri::command]
async fn delete_transcription(
    state: State<TranscriptionManager>,
    id: String
) -> Result<()>

#[tauri::command]
async fn search_transcriptions(
    state: State<TranscriptionManager>,
    query: String
) -> Result<Vec<Transcription>>

#[tauri::command]
async fn export_transcriptions(
    state: State<TranscriptionManager>,
    format: ExportFormat  // JSON, CSV, TXT, MD
) -> Result<String>
```

### 4. Frontend: React State Management

#### A. Transcription Store
```javascript
// State structure
const [transcriptions, setTranscriptions] = useState([])
const [loading, setLoading] = useState(true)
const [filter, setFilter] = useState({ 
  dateRange: null, 
  searchQuery: '', 
  status: 'all' 
})
const [selectedTranscription, setSelectedTranscription] = useState(null)
const [page, setPage] = useState(1)
const [hasMore, setHasMore] = useState(true)
```

#### B. Data Flow
```javascript
// Initial load
useEffect(() => {
  loadTranscriptions({ limit: 50, offset: 0 })
}, [])

// Listen for file system changes
useEffect(() => {
  const unlisten = listen('transcription-changed', (event) => {
    // event.payload: { action: 'created'|'updated'|'deleted', transcription: {...} }
    handleTranscriptionChange(event.payload)
  })
  return () => unlisten()
}, [])

// Infinite scroll
const loadMore = () => {
  if (!loading && hasMore) {
    loadTranscriptions({ 
      limit: 50, 
      offset: transcriptions.length 
    })
  }
}
```

### 5. UI Components

#### A. Transcription List View
```jsx
<TranscriptionList>
  <SearchBar onSearch={handleSearch} />
  <FilterBar>
    <DateRangePicker />
    <StatusFilter />
    <SortOptions />
  </FilterBar>
  
  <VirtualizedList
    items={filteredTranscriptions}
    onScroll={handleInfiniteScroll}
    renderItem={(item) => (
      <TranscriptionCard
        transcription={item}
        onSelect={handleSelect}
        onDelete={handleDelete}
        onExport={handleExport}
      />
    )}
  />
</TranscriptionList>
```

#### B. Transcription Card States
- **Complete**: Show text preview, duration, timestamp
- **Processing**: Show spinner, estimated time
- **Failed**: Show error, retry option
- **Selected**: Expand to show full text

### 6. Advanced Features

#### A. Full-Text Search
- Index transcription text for fast searching
- Support fuzzy matching
- Highlight search terms in results

#### B. Export Functionality
- Export single or multiple transcriptions
- Formats: Plain text, Markdown, JSON, CSV
- Include audio files in ZIP export

#### C. Batch Operations
- Select multiple transcriptions
- Batch delete, export, or tag
- Undo/redo support

#### D. Smart Caching
```rust
struct CacheStrategy {
    max_items: usize,        // Maximum cached items
    max_memory: usize,       // Maximum memory usage
    ttl: Duration,           // Time to live
    preload_recent: usize,   // Number of recent items to preload
}
```

### 7. Migration & Backward Compatibility

#### Handle Existing Files
1. Scan for orphaned .wav files without transcriptions
2. Option to re-transcribe failed recordings
3. Clean up duplicate or corrupted files
4. Generate missing metadata files

### 8. Performance Optimizations

#### A. Lazy Loading Strategy
```
1. Load metadata only (fast) -> Display list
2. Load text on demand -> When card expands
3. Preload adjacent items -> For smooth scrolling
4. Cache recent items -> For quick access
```

#### B. Virtual Scrolling
- Only render visible items
- Recycle DOM nodes
- Fixed height items for performance

#### C. Debouncing & Throttling
- Debounce search input (300ms)
- Throttle scroll events (100ms)
- Batch file system events (500ms)

### 9. Error Handling

#### Graceful Failures
```rust
// If metadata is missing, reconstruct from filename
// If text is missing, show "Transcription unavailable"
// If audio is missing, disable playback button
// Log errors but don't crash
```

### 10. Implementation Phases

#### Phase 1: Core Infrastructure (4-6 hours)
- [ ] Create TranscriptionManager service
- [ ] Implement file scanning and parsing
- [ ] Add basic Tauri commands
- [ ] Load historical transcriptions on startup

#### Phase 2: Real-time Sync (2-3 hours)
- [ ] Implement file system watcher
- [ ] Add change events and UI updates
- [ ] Handle concurrent modifications

#### Phase 3: UI Enhancements (3-4 hours)
- [ ] Add search and filtering
- [ ] Implement virtual scrolling
- [ ] Add loading states and error handling
- [ ] Create transcription detail view

#### Phase 4: Advanced Features (3-4 hours)
- [ ] Add export functionality
- [ ] Implement batch operations
- [ ] Add keyboard shortcuts
- [ ] Create settings for cache management

#### Phase 5: Polish & Optimization (2-3 hours)
- [ ] Performance profiling
- [ ] Memory optimization
- [ ] UI/UX improvements
- [ ] Comprehensive testing

## Benefits of This Architecture

1. **Single Source of Truth**: File system is authoritative
2. **Offline First**: Works without network, syncs when needed
3. **Scalable**: Handles thousands of transcriptions efficiently
4. **Resilient**: Gracefully handles missing/corrupt files
5. **User-Friendly**: Fast, responsive, intuitive interface
6. **Maintainable**: Clear separation of concerns
7. **Extensible**: Easy to add new features

## Future Enhancements

1. **Cloud Sync**: Optional backup to cloud storage
2. **Collaboration**: Share transcriptions with others
3. **AI Features**: Summarization, categorization, insights
4. **Voice Commands**: Control app with voice
5. **Mobile Sync**: Companion mobile app
6. **Plugins**: Extensibility through plugin system

## Success Metrics

- Load 1000+ transcriptions in < 500ms
- Search results in < 100ms
- File system changes reflected in < 1s
- Memory usage < 100MB for 10,000 transcriptions
- Zero data loss during operations