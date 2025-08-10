import { invoke } from '@tauri-apps/api/core';

export const api = {
  // Transcriptions - New SQLx-based APIs
  async getTranscriptions(params = {}) {
    return invoke('get_transcriptions', params);
  },
  
  async getTranscription(id) {
    return invoke('get_transcription', { id });
  },
  
  async updateTranscription(id, updates) {
    return invoke('update_transcription', { id, updates });
  },
  
  async deleteTranscription(id) {
    return invoke('delete_transcription', { id });
  },
  
  async searchTranscriptions(query) {
    return invoke('search_transcriptions', { query });
  },
  
  async syncFilesystem() {
    return invoke('sync_filesystem_sqlx');
  },
  
  async getDatabaseStats() {
    return invoke('get_database_stats');
  },
  
  async clearDatabase() {
    return invoke('clear_database');
  },
  
  async cleanupDuplicates() {
    return invoke('cleanup_duplicate_transcriptions');
  },
  
  // Legacy database commands (will be phased out)
  async dbGetTranscriptions(limit = 50, offset = 0, statusFilter = null) {
    return invoke('db_get_transcriptions', { limit, offset, statusFilter });
  },
  
  async dbSearchTranscriptions(query) {
    return invoke('db_search_transcriptions', { query });
  },
  
  async dbInsertTranscription(transcription) {
    return invoke('db_insert_transcription', { transcription });
  },
  
  async dbUpdateTranscriptionStatus(id, status, error = null) {
    return invoke('db_update_transcription_status', { id, status, error });
  },
  
  // Background tasks - Updated to match actual implementation
  async getQueueStatus() {
    return invoke('get_queue_status');
  },
  
  async getQueueTasks(limit = 50, offset = 0) {
    return invoke('get_queue_tasks', { limit, offset });
  },
  
  async enqueueOrphanTask(transcriptionId, audioPath) {
    return invoke('enqueue_orphan_task', { transcriptionId, audioPath });
  },
  
  async pauseQueue() {
    return invoke('pause_queue');
  },
  
  async resumeQueue() {
    return invoke('resume_queue');
  },
  
  async retryFailedTask(taskId) {
    return invoke('retry_failed_task', { taskId });
  },
  
  async clearCompletedTasks() {
    return invoke('clear_completed_tasks');
  },
  
  async isQueuePaused() {
    return invoke('is_queue_paused');
  },
  
  // Filesystem sync
  async syncFilesystemLegacy() {
    return invoke('sync_filesystem');
  },
  
  async syncFilesystemForce() {
    return invoke('sync_filesystem_force');
  },
  
  // Recording
  async startRecording() {
    return invoke('start_recording');
  },
  
  async stopRecording() {
    return invoke('stop_recording');
  },
  
  async quickNote() {
    return invoke('quick_note');
  },
  
  async getRecordingStatus() {
    return invoke('get_recording_status');
  },
  
  async transcribeFile(filePath) {
    return invoke('transcribe_file', { filePath });
  }
};