import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import './BackgroundTasksTab.css';

function BackgroundTasksTab() {
  const [queueStatus, setQueueStatus] = useState(null);
  const [tasks, setTasks] = useState([]);
  const [isPaused, setIsPaused] = useState(false);
  const [loading, setLoading] = useState(true);
  const [activeTab, setActiveTab] = useState('pending');

  useEffect(() => {
    loadQueueStatus();
    loadTasks();
    
    // Listen for task updates
    const unlisten = listen('background-task-update', (event) => {
      console.log('Task update:', event.payload);
      loadQueueStatus();
      loadTasks();
    });
    
    // Refresh periodically
    const interval = setInterval(() => {
      loadQueueStatus();
      if (!isPaused) {
        loadTasks();
      }
    }, 5000);
    
    return () => {
      unlisten.then(fn => fn());
      clearInterval(interval);
    };
  }, [isPaused]);

  const loadQueueStatus = async () => {
    try {
      const status = await invoke('get_queue_status');
      setQueueStatus(status);
      setIsPaused(status.is_paused);
      setLoading(false);
    } catch (error) {
      console.error('Failed to load queue status:', error);
      setLoading(false);
    }
  };

  const loadTasks = async () => {
    try {
      const taskList = await invoke('get_queue_tasks', { limit: 100, offset: 0 });
      setTasks(taskList);
    } catch (error) {
      console.error('Failed to load tasks:', error);
    }
  };

  const togglePause = async () => {
    try {
      if (isPaused) {
        await invoke('resume_queue');
      } else {
        await invoke('pause_queue');
      }
      setIsPaused(!isPaused);
      loadQueueStatus();
    } catch (error) {
      console.error('Failed to toggle pause:', error);
    }
  };

  const retryTask = async (taskId) => {
    try {
      await invoke('retry_failed_task', { taskId });
      loadTasks();
      loadQueueStatus();
    } catch (error) {
      console.error('Failed to retry task:', error);
    }
  };

  const clearCompleted = async () => {
    try {
      const count = await invoke('clear_completed_tasks');
      console.log(`Cleared ${count} completed tasks`);
      loadTasks();
      loadQueueStatus();
    } catch (error) {
      console.error('Failed to clear completed tasks:', error);
    }
  };

  const getTasksByStatus = (status) => {
    return tasks.filter(task => {
      if (status === 'pending') return task.status === 'Pending';
      if (status === 'processing') return task.status.Processing !== undefined;
      if (status === 'completed') return task.status === 'Completed';
      if (status === 'failed') return task.status.Failed !== undefined;
      return false;
    });
  };

  const formatTaskType = (taskType) => {
    if (taskType.TranscribeOrphan) {
      const path = taskType.TranscribeOrphan.audio_path;
      const filename = path.split(/[/\\]/).pop();
      return `Transcribe: ${filename}`;
    }
    if (taskType.TranscribeImported) {
      return `Import: ${taskType.TranscribeImported.original_name}`;
    }
    return 'Unknown Task';
  };

  const formatDateTime = (dateTime) => {
    if (!dateTime) return '-';
    const date = new Date(dateTime);
    return date.toLocaleString();
  };

  const formatDuration = (startTime, endTime) => {
    if (!startTime) return '-';
    const start = new Date(startTime);
    const end = endTime ? new Date(endTime) : new Date();
    const diff = Math.floor((end - start) / 1000);
    const minutes = Math.floor(diff / 60);
    const seconds = diff % 60;
    return `${minutes}m ${seconds}s`;
  };

  if (loading) {
    return (
      <div className="background-tasks-tab">
        <div className="loading">Loading queue status...</div>
      </div>
    );
  }

  return (
    <div className="background-tasks-tab">
      <div className="tasks-header">
        <div className="header-left">
          <h2>Background Tasks</h2>
          <div className="task-counts">
            <span className="badge pending">{queueStatus?.pending_count || 0} pending</span>
            <span className="badge processing">{queueStatus?.processing_count || 0} processing</span>
            <span className="badge completed">{queueStatus?.completed_count || 0} completed</span>
            <span className="badge failed">{queueStatus?.failed_count || 0} failed</span>
          </div>
        </div>
        <div className="header-right">
          <button 
            className={`control-button ${isPaused ? 'resume' : 'pause'}`}
            onClick={togglePause}
          >
            {isPaused ? '▶ Resume' : '⏸ Pause'}
          </button>
          <button 
            className="control-button clear"
            onClick={clearCompleted}
            disabled={!queueStatus?.completed_count}
          >
            Clear Completed
          </button>
        </div>
      </div>

      {queueStatus?.active_task && (
        <div className="active-task-section">
          <h3>Currently Processing</h3>
          <div className="task-card active">
            <div className="task-icon">⚙️</div>
            <div className="task-info">
              <div className="task-name">{formatTaskType(queueStatus.active_task.task_type)}</div>
              <div className="task-meta">
                Started: {formatDateTime(queueStatus.active_task.started_at)}
              </div>
              {queueStatus.active_task.status.Processing && (
                <div className="progress-bar">
                  <div 
                    className="progress-fill"
                    style={{ width: `${(queueStatus.active_task.status.Processing.progress || 0) * 100}%` }}
                  />
                </div>
              )}
            </div>
          </div>
        </div>
      )}

      <div className="tasks-tabs">
        <button 
          className={`tab ${activeTab === 'pending' ? 'active' : ''}`}
          onClick={() => setActiveTab('pending')}
        >
          Pending ({getTasksByStatus('pending').length})
        </button>
        <button 
          className={`tab ${activeTab === 'failed' ? 'active' : ''}`}
          onClick={() => setActiveTab('failed')}
        >
          Failed ({getTasksByStatus('failed').length})
        </button>
        <button 
          className={`tab ${activeTab === 'completed' ? 'active' : ''}`}
          onClick={() => setActiveTab('completed')}
        >
          Completed ({getTasksByStatus('completed').length})
        </button>
      </div>

      <div className="tasks-list">
        {activeTab === 'pending' && (
          <TaskList 
            tasks={getTasksByStatus('pending')}
            title="Pending Tasks"
            emptyMessage="No pending tasks"
          />
        )}
        
        {activeTab === 'failed' && (
          <TaskList 
            tasks={getTasksByStatus('failed')}
            title="Failed Tasks"
            emptyMessage="No failed tasks"
            onRetry={retryTask}
          />
        )}
        
        {activeTab === 'completed' && (
          <TaskList 
            tasks={getTasksByStatus('completed')}
            title="Completed Tasks"
            emptyMessage="No completed tasks"
          />
        )}
      </div>
    </div>
  );
}

function TaskList({ tasks, title, emptyMessage, onRetry }) {
  if (tasks.length === 0) {
    return (
      <div className="empty-state">
        <p>{emptyMessage}</p>
      </div>
    );
  }

  return (
    <div className="task-list">
      {tasks.map(task => (
        <TaskCard key={task.id} task={task} onRetry={onRetry} />
      ))}
    </div>
  );
}

function TaskCard({ task, onRetry }) {
  const getStatusIcon = () => {
    if (task.status === 'Pending') return '⏳';
    if (task.status.Processing) return '⚙️';
    if (task.status === 'Completed') return '✅';
    if (task.status.Failed) return '❌';
    return '❓';
  };

  const formatTaskType = (taskType) => {
    if (taskType.TranscribeOrphan) {
      const path = taskType.TranscribeOrphan.audio_path;
      const filename = path.split(/[/\\]/).pop();
      return filename;
    }
    if (taskType.TranscribeImported) {
      return taskType.TranscribeImported.original_name;
    }
    return 'Unknown';
  };

  const formatDateTime = (dateTime) => {
    if (!dateTime) return '-';
    const date = new Date(dateTime);
    return date.toLocaleString();
  };

  return (
    <div className="task-card">
      <span className="status-icon">{getStatusIcon()}</span>
      <div className="task-info">
        <div className="task-name">{formatTaskType(task.task_type)}</div>
        <div className="task-meta">
          <span>Created: {formatDateTime(task.created_at)}</span>
          {task.retry_count > 0 && (
            <span className="retry-count">Retries: {task.retry_count}/{task.max_retries}</span>
          )}
        </div>
        {task.status.Failed && (
          <div className="error-message">{task.status.Failed.error || task.error_message}</div>
        )}
      </div>
      {task.status.Failed && onRetry && task.status.Failed.can_retry && (
        <button className="retry-button" onClick={() => onRetry(task.id)}>
          Retry
        </button>
      )}
    </div>
  );
}

export default BackgroundTasksTab;