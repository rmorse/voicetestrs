#!/usr/bin/env node

import { execSync } from 'child_process';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const LOCK_FILE = path.join(__dirname, '.dev-server.lock');

console.log('🧹 Cleaning up dev server...');

// Remove lock file to signal dev server to exit
if (fs.existsSync(LOCK_FILE)) {
  try {
    const pid = fs.readFileSync(LOCK_FILE, 'utf8');
    fs.unlinkSync(LOCK_FILE);
    console.log(`✓ Removed lock file for PID ${pid}`);
    
    // Try to kill the process directly too
    if (process.platform === 'win32') {
      try {
        execSync(`taskkill /pid ${pid} /f`, { stdio: 'ignore' });
        console.log(`✓ Killed process ${pid}`);
      } catch (e) {
        // Process might already be gone
      }
    } else {
      try {
        process.kill(parseInt(pid), 'SIGTERM');
        console.log(`✓ Sent SIGTERM to process ${pid}`);
      } catch (e) {
        // Process might already be gone
      }
    }
  } catch (e) {
    console.log('Lock file cleanup error:', e.message);
  }
}

// Also try to clean up PM2 if it's running
try {
  execSync('npx pm2 delete vite-dev', { stdio: 'ignore' });
  console.log('✓ Cleaned up PM2 process');
} catch (e) {
  // PM2 might not be running
}

// Clean up port files
['.current-port', '../.current-port'].forEach(file => {
  const filePath = path.join(__dirname, file);
  if (fs.existsSync(filePath)) {
    try {
      fs.unlinkSync(filePath);
      console.log(`✓ Cleaned up ${file}`);
    } catch (e) {
      // Ignore
    }
  }
});

console.log('✅ Cleanup complete');
process.exit(0);