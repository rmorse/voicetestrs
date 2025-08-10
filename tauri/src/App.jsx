import React, { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { api } from './lib/api'
import BackgroundTasksTab from './BackgroundTasksTab'
import './App.css'

function App() {
  const [appState, setAppState] = useState('idle') // 'idle' | 'recording' | 'processing'
  const [transcriptions, setTranscriptions] = useState([])
  const [recordingDuration, setRecordingDuration] = useState(0)
  const [error, setError] = useState(null)
  const [syncStatus, setSyncStatus] = useState(null) // Track sync status
  const [dbStats, setDbStats] = useState(null) // Database statistics
  const [showSettings, setShowSettings] = useState(false) // For dropdown visibility
  const [activeTab, setActiveTab] = useState('transcriptions') // 'transcriptions' | 'background-tasks'

  useEffect(() => {
    console.log('App mounted, setting up event listeners...')
    
    // Run filesystem sync on startup
    const runStartupSync = async () => {
      console.log('Running filesystem sync on startup...')
      setSyncStatus('Syncing filesystem...')
      
      // Add timeout to prevent infinite hang
     /*  const timeoutPromise = new Promise((_, reject) => 
        setTimeout(() => reject(new Error('Sync timeout after 10s')), 10000)
      ) */
      
      // Backend handles all database operations now
      
      try {
        // Use new SQLx-based sync with timeout
        /* const syncPromise = api.syncFilesystem()
        const result = await Promise.race([syncPromise, timeoutPromise])
        console.log('Sync completed:', result) */
        const result = await api.syncFilesystem()
        // After sync, load transcriptions from database
        await loadTranscriptions()
      } catch (err) {
        console.error('Startup sync failed:', err)
        setSyncStatus(`Sync failed: ${err}`)
        // Even if sync fails, try to load existing transcriptions
        await loadTranscriptions()
      } finally {
        // Clear sync status after a moment
        setTimeout(() => setSyncStatus(null), 3000)
      }
    }
    
    // Initialize app state first
    const initializeApp = async () => {
      try {
        // Get initial recording state FIRST
        const initialState = await invoke('get_recording_status')
        console.log('Initial recording state:', initialState)
        setAppState(initialState)
      } catch (err) {
        console.error('Failed to get initial state:', err)
      }
      
      // Then run the sync
      runStartupSync()
    }
    
    initializeApp()

    // Listen for transcription events from backend
    const unlisten = listen('transcription-complete', async (event) => {
      console.log('Transcription complete event:', event.payload)
      
      // Extract relative path from absolute path if needed
      let audioPath = event.payload.audio_path || event.payload.audioPath
      let textPath = event.payload.text_path || event.payload.textPath
      
      // Remove Windows path prefix and convert to relative path
      if (audioPath && audioPath.includes('\\?\\')) {
        // Extract just the relative path from notes folder
        const match = audioPath.match(/notes[\\/](.+)/)
        if (match) {
          audioPath = match[1].replace(/\\/g, '/')
        }
      }
      
      if (textPath && textPath.includes('\\?\\')) {
        const match = textPath.match(/notes[\\/](.+)/)
        if (match) {
          textPath = match[1].replace(/\\/g, '/')
        }
      }
      
      // Use the timestamp from backend (extracted using our robust method)
      const createdAt = event.payload.created_at || new Date().toISOString()
      
      // Generate ID from the timestamp
      const date = new Date(createdAt)
      const dateStr = date.toISOString().slice(0, 10).replace(/-/g, '')
      const timeStr = date.toTimeString().slice(0, 8).replace(/:/g, '')
      const id = `${dateStr}-${timeStr}`
      
      // Insert new transcription into database
      const transcription = {
        id: id,
        audio_path: audioPath,
        text_path: textPath,
        transcription_text: event.payload.text,
        created_at: createdAt,  // Use the timestamp from backend (file metadata)
        status: 'complete',
        source: 'recording',
        duration_seconds: 0,
        file_size_bytes: 0,
        language: 'en',
        model: 'base.en'
      }
      
      // Backend now handles database insertion
      // Just reload the transcriptions to show the new one
      await loadTranscriptions()
      // Also reload database stats to update counts
      await loadDbStats()
    })

    // Listen for state changes
    const unlistenStatus = listen('state-changed', (event) => {
      console.log('State changed:', event.payload.state)
      setAppState(event.payload.state)
      if (event.payload.state !== 'recording') {
        setRecordingDuration(0)
      }
    })
    
    // Listen for individual transcription sync events
    const unlistenSyncTranscription = listen('sync-transcription', async (event) => {
      const transcription = event.payload
      console.log('Syncing transcription:', transcription.id, 'status:', transcription.status)
      // Backend handles all database operations now
    })
    
    // Listen for sync completion
    const unlistenSyncComplete = listen('sync-complete', async (event) => {
      const report = event.payload
      console.log('Sync complete event received:', report)
      setSyncStatus(`Sync complete: ${report.new_transcriptions || 0} new, ${report.updated_transcriptions || 0} updated, ${report.total_files_found || 0} total files`)
      
      // Wait a bit for all inserts to complete
      setTimeout(async () => {
        console.log('Loading transcriptions after sync complete...')
        // Reload transcriptions after sync
        await loadTranscriptions()
        // Load database stats
        await loadDbStats()
      }, 500)
      
      // Clear sync status after a few seconds
      setTimeout(() => setSyncStatus(null), 5000)
    })

    return () => {
      unlisten.then(fn => fn())
      unlistenStatus.then(fn => fn())
      unlistenSyncTranscription.then(fn => fn())
      unlistenSyncComplete.then(fn => fn())
    }
  }, [])
  
  const loadTranscriptions = async () => {
    try {
      console.log('Loading transcriptions from database...')
      const data = await api.getTranscriptions({ 
        limit: 50, 
        offset: 0,
        status: null
      })
      console.log('Loaded transcriptions:', data)
      setTranscriptions(data)
    } catch (err) {
      console.error('Failed to load transcriptions:', err)
    }
  }
  
  const loadDbStats = async () => {
    try {
      // Use new API for database stats
      const stats = await api.getDatabaseStats()
      setDbStats(stats)
    } catch (err) {
      console.error('Failed to load database stats:', err)
    }
  }

  useEffect(() => {
    let interval
    if (appState === 'recording') {
      interval = setInterval(() => {
        setRecordingDuration(prev => prev + 1)
      }, 1000)
    }
    return () => clearInterval(interval)
  }, [appState])
  
  // Update database stats whenever transcriptions list changes
  useEffect(() => {
    if (transcriptions.length > 0 || (dbStats && dbStats.total_transcriptions > 0)) {
      // Reload stats when transcriptions change
      loadDbStats()
    }
  }, [transcriptions])

  const toggleRecording = async () => {
    try {
      setError(null)
      if (appState === 'recording') {
        // Stop recording - backend will ignore if not actually recording
        await invoke('stop_recording')
      } else if (appState === 'idle') {
        // Start recording - backend will ignore if not actually idle
        await invoke('start_recording')
      }
      // If processing, do nothing (button should be disabled)
    } catch (err) {
      setError(err.toString())
    }
  }

  const quickNote = async () => {
    try {
      setError(null)
      if (appState === 'idle') {
        // Backend will ignore if not actually idle
        await invoke('quick_note', { duration: 10 })
      }
    } catch (err) {
      setError(err.toString())
    }
  }

  const clearTranscriptions = () => {
    setTranscriptions([])
  }
  
  const fullResync = async () => {
    try {
      setSyncStatus('Starting full resync...')
      console.log('Starting full resync - clearing database...')
      
      // Clear all transcriptions from database using backend API
      await api.clearDatabase()
      console.log('Database cleared')
      
      // Clear the UI
      setTranscriptions([])
      
      // Trigger filesystem sync to repopulate
      console.log('Triggering filesystem sync...')
      const result = await api.syncFilesystem()
      console.log('Resync completed:', result)
      
      // Reload transcriptions
      await loadTranscriptions()
      await loadDbStats()
      
      setSyncStatus('Full resync completed!')
      setTimeout(() => setSyncStatus(null), 3000)
    } catch (err) {
      console.error('Full resync failed:', err)
      setSyncStatus(`Resync failed: ${err}`)
      setTimeout(() => setSyncStatus(null), 5000)
    }
  }

  const formatDuration = (seconds) => {
    const mins = Math.floor(seconds / 60)
    const secs = seconds % 60
    return `${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`
  }

  return (
    <div className="app">
      <header className="app-header">
        <h1>VoiceTextRS</h1>
        <p className="subtitle">Offline Voice-to-Text Transcription</p>
      </header>

      <main className="app-main">
        {error && (
          <div className="error-message">
            {error}
          </div>
        )}

        <div className="recording-section">
          <div className="recording-controls">
            <button 
              className={`record-button ${appState === 'recording' ? 'recording' : ''} ${appState === 'processing' ? 'processing' : ''}`}
              onClick={toggleRecording}
              disabled={appState === 'processing'}
            >
              <span className={`record-icon ${appState === 'processing' ? 'spinning' : ''}`}>
                {appState === 'idle' ? '🎤' : appState === 'recording' ? '⏹' : '⚙️'}
              </span>
              {appState === 'idle' ? 'Start Recording' : 
               appState === 'recording' ? 'Stop Recording' : 
               'Processing...'}
            </button>
            
            <button 
              className="quick-note-button"
              onClick={quickNote}
              disabled={appState !== 'idle'}
            >
              <span className="icon">⚡</span>
              Quick Note (10s)
            </button>
          </div>

          {appState === 'recording' && (
            <div className="recording-status">
              <div className="recording-indicator"></div>
              <span>Recording... {formatDuration(recordingDuration)}</span>
            </div>
          )}
          
          {appState === 'processing' && (
            <div className="processing-status">
              <div className="spinner"></div>
              <span>Transcribing audio...</span>
            </div>
          )}
        </div>

        <div className="tabs-container">
          <div className="tabs-header">
            <button 
              className={`tab-button ${activeTab === 'transcriptions' ? 'active' : ''}`}
              onClick={() => setActiveTab('transcriptions')}
            >
              Transcriptions
              {transcriptions.length > 0 && (
                <span className="tab-badge">{transcriptions.length}</span>
              )}
            </button>
            <button 
              className={`tab-button ${activeTab === 'background-tasks' ? 'active' : ''}`}
              onClick={() => setActiveTab('background-tasks')}
            >
              Background Tasks
            </button>
          </div>

        {activeTab === 'transcriptions' && (
        <div className="transcriptions-section">
          <div className="section-header">
            <h2>Transcriptions</h2>
            <div className="header-info">
              {syncStatus && <span className="sync-status">{syncStatus}</span>}
              {dbStats && (
                <span className="db-stats">
                  {dbStats.total_transcriptions} total, {dbStats.completed} completed
                </span>
              )}
              <div className="settings-dropdown">
                <button 
                  className="settings-button"
                  onClick={() => setShowSettings(!showSettings)}
                  title="Settings"
                >
                  ⚙️
                </button>
                {showSettings && (
                  <div className="dropdown-menu">
                    <button 
                      className="dropdown-item"
                      onClick={async () => {
                        setShowSettings(false)
                        await fullResync()
                      }}
                    >
                      🔄 Full Resync
                    </button>
                  </div>
                )}
              </div>
            </div>
          </div>

          <div className="transcriptions-list">
            {transcriptions.length === 0 ? (
              <div className="empty-state">
                <p>No transcriptions yet</p>
                <p className="hint">Press the record button or use Ctrl+Shift+R to start</p>
              </div>
            ) : (
              transcriptions.map(item => (
                <div key={item.id} className="transcription-item">
                  <div className="transcription-header">
                    <span className="timestamp">
                      {item.created_at ? new Date(item.created_at).toLocaleString() : 'Unknown'}
                    </span>
                    <span className="status-badge {item.status}">
                      {item.status || 'complete'}
                    </span>
                  </div>
                  <div className="transcription-text">
                    {item.transcription_text || item.text || 'No transcription available'}
                  </div>
                  <div className="transcription-footer">
                    <span className="audio-path">{item.audioPath}</span>
                  </div>
                </div>
              ))
            )}
          </div>
        </div>
        )}
        
        {activeTab === 'background-tasks' && (
          <BackgroundTasksTab />
        )}
        </div>

        <div className="shortcuts-info">
          <h3>Keyboard Shortcuts</h3>
          <ul>
            <li><kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>R</kbd> - Toggle Recording</li>
            <li><kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>N</kbd> - Quick Note (10s)</li>
            <li><kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>V</kbd> - Show Window</li>
          </ul>
        </div>
      </main>
    </div>
  )
}

export default App