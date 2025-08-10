import React, { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import './App.css'

function App() {
  const [appState, setAppState] = useState('idle') // 'idle' | 'recording' | 'processing'
  const [transcriptions, setTranscriptions] = useState([])
  const [recordingDuration, setRecordingDuration] = useState(0)
  const [error, setError] = useState(null)

  useEffect(() => {
    // Check initial state on mount
    invoke('get_recording_status').then(state => {
      setAppState(state)
    }).catch(console.error)

    // Listen for transcription events from backend
    const unlisten = listen('transcription-complete', (event) => {
      setTranscriptions(prev => [{
        id: Date.now(),
        text: event.payload.text,
        timestamp: new Date().toLocaleString(),
        audioPath: event.payload.audioPath
      }, ...prev])
    })

    // Listen for state changes
    const unlistenStatus = listen('state-changed', (event) => {
      console.log('State changed:', event.payload.state)
      setAppState(event.payload.state)
      if (event.payload.state !== 'recording') {
        setRecordingDuration(0)
      }
    })

    // No need for separate started/stopped events anymore - state-changed handles it

    return () => {
      unlisten.then(fn => fn())
      unlistenStatus.then(fn => fn())
    }
  }, [])

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
            {transcriptions.length > 0 && (
              <button className="clear-button" onClick={clearTranscriptions}>
                Clear All
              </button>
            )}
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
                    <span className="timestamp">{item.timestamp}</span>
                  </div>
                  <div className="transcription-text">{item.text}</div>
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