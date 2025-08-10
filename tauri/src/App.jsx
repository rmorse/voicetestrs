import React, { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import db from './lib/database'
import './App.css'

function App() {
  const [appState, setAppState] = useState('idle') // 'idle' | 'recording' | 'processing'
  const [transcriptions, setTranscriptions] = useState([])
  const [recordingDuration, setRecordingDuration] = useState(0)
  const [error, setError] = useState(null)
  const [syncStatus, setSyncStatus] = useState(null) // Track sync status
  const [dbStats, setDbStats] = useState(null) // Database statistics

  useEffect(() => {
    console.log('App mounted, setting up event listeners...')
    
    // Run filesystem sync on startup
    const runStartupSync = async () => {
      console.log('Running filesystem sync on startup...')
      setSyncStatus('Syncing filesystem...')
      try {
        const result = await invoke('sync_filesystem')
        console.log('Sync completed:', result)
        // After sync, load transcriptions from database
        await loadTranscriptions()
      } catch (err) {
        console.error('Startup sync failed:', err)
        setSyncStatus(`Sync failed: ${err}`)
        // Even if sync fails, try to load existing transcriptions
        await loadTranscriptions()
      }
    }
    
    // Run the sync
    runStartupSync()
    
    // Check initial state on mount
    invoke('get_recording_status').then(state => {
      setAppState(state)
    }).catch(console.error)

    // Listen for transcription events from backend
    const unlisten = listen('transcription-complete', async (event) => {
      // Insert new transcription into database
      const transcription = {
        id: `${new Date().toISOString().replace(/[^0-9]/g, '').slice(0, 14)}`,
        audio_path: event.payload.audioPath,
        text_path: event.payload.textPath,
        transcription_text: event.payload.text,
        created_at: new Date().toISOString(),
        status: 'complete',
        source: 'recording',
        duration_seconds: 0,
        file_size_bytes: 0,
        language: 'en',
        model: 'base.en'
      }
      
      try {
        await db.insertTranscription(transcription)
        // Reload transcriptions to show the new one
        loadTranscriptions()
      } catch (err) {
        console.error('Failed to insert transcription:', err)
      }
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
      console.log('Received sync-transcription event:', transcription)
      try {
        // Check if transcription already exists
        const existing = await db.getTranscription(transcription.id)
        console.log('Existing transcription check:', existing)
        if (!existing) {
          console.log('Inserting new transcription:', transcription)
          try {
            const insertResult = await db.insertTranscription(transcription)
            console.log('Insert result:', insertResult)
            console.log('Successfully synced transcription:', transcription.id)
          } catch (insertErr) {
            console.error('Failed to insert transcription:', insertErr)
          }
        } else {
          console.log('Transcription already exists:', transcription.id)
        }
      } catch (err) {
        console.error('Failed to sync transcription:', err)
      }
    })
    
    // Listen for sync completion
    const unlistenSyncComplete = listen('sync-complete', async (event) => {
      const report = event.payload
      console.log('Sync complete event received:', report)
      setSyncStatus(`Sync complete: ${report.completed_transcriptions} transcriptions, ${report.orphaned_audio} orphaned files`)
      
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
      // Don't filter by status to see all transcriptions
      const data = await db.getTranscriptions({ status: null, limit: 50, offset: 0 })
      console.log('Loaded transcriptions:', data)
      setTranscriptions(data)
    } catch (err) {
      console.error('Failed to load transcriptions:', err)
    }
  }
  
  const loadDbStats = async () => {
    try {
      const stats = await db.getDatabaseStats()
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

  const toggleRecording = async () => {
    try {
      setError(null)
      if (appState === 'recording') {
        // Stop recording - state will change to 'processing' then 'idle'
        await invoke('stop_recording')
      } else if (appState === 'idle') {
        // Start recording - state will change to 'recording'
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
        await invoke('quick_note', { duration: 10 })
      }
    } catch (err) {
      setError(err.toString())
    }
  }

  const clearTranscriptions = () => {
    setTranscriptions([])
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