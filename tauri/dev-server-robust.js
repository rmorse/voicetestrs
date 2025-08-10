#!/usr/bin/env node

import { spawn, execSync } from 'child_process';
import net from 'net';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const PORT_FILE = path.join(__dirname, '.current-port');
const PARENT_PORT_FILE = path.join(__dirname, '..', '.current-port');
const LOCK_FILE = path.join(__dirname, '.dev-server.lock');

// Find available port
async function findAvailablePort(startPort = 5173) {
  for (let port = startPort; port < startPort + 100; port++) {
    const isAvailable = await checkPortAvailable(port);
    if (isAvailable) {
      console.log(`✓ Port ${port} is available`);
      return port;
    }
    console.log(`✗ Port ${port} is in use, trying next...`);
  }
  throw new Error('No available ports found');
}

function checkPortAvailable(port) {
  return new Promise((resolve) => {
    const server = net.createServer();
    
    server.once('error', () => resolve(false));
    
    server.once('listening', () => {
      server.close(() => {
        setTimeout(() => resolve(true), 100);
      });
    });
    
    server.listen(port);
  });
}

// Wait for Vite server to be ready
function waitForServer(port, maxAttempts = 60) {
  return new Promise((resolve, reject) => {
    let attempts = 0;
    
    const check = () => {
      const client = new net.Socket();
      client.setTimeout(200);
      
      client.once('connect', () => {
        client.destroy();
        console.log(`✓ Vite server is ready on port ${port}`);
        resolve(true);
      });
      
      client.once('timeout', () => {
        client.destroy();
        attempts++;
        if (attempts >= maxAttempts) {
          reject(new Error(`Server didn't start after ${maxAttempts} attempts`));
        } else {
          if (attempts % 10 === 0) {
            console.log(`⏳ Waiting for server... (${attempts}/${maxAttempts})`);
          }
          setTimeout(check, 500);
        }
      });
      
      client.once('error', () => {
        attempts++;
        if (attempts >= maxAttempts) {
          reject(new Error(`Server didn't start after ${maxAttempts} attempts`));
        } else {
          setTimeout(check, 500);
        }
      });
      
      client.connect(port, 'localhost');
    };
    
    setTimeout(check, 1000);
  });
}

// Clean up function
function cleanup(viteProcess) {
  console.log('\n🧹 Cleaning up...');
  
  // Clean up port files
  [PORT_FILE, PARENT_PORT_FILE, LOCK_FILE].forEach(file => {
    if (fs.existsSync(file)) {
      try {
        fs.unlinkSync(file);
        console.log(`✓ Cleaned up ${path.basename(file)}`);
      } catch (e) {
        // Ignore
      }
    }
  });
  
  // Kill Vite process
  if (viteProcess && !viteProcess.killed) {
    console.log(`Terminating Vite process (PID: ${viteProcess.pid})...`);
    
    try {
      if (process.platform === 'win32') {
        // Windows: Kill process tree
        execSync(`taskkill /pid ${viteProcess.pid} /f /t`, { stdio: 'ignore' });
      } else {
        // Unix: Kill process group
        process.kill(-viteProcess.pid, 'SIGTERM');
      }
      console.log('✓ Vite process terminated');
    } catch (err) {
      console.error('Error killing process:', err.message);
    }
  }
}

// Main function
async function main() {
  let viteProcess = null;
  
  // Create lock file with current PID
  fs.writeFileSync(LOCK_FILE, process.pid.toString());
  
  // Set up exit handlers
  const handleExit = (signal) => {
    console.log(`\n📡 Received ${signal} signal`);
    cleanup(viteProcess);
    process.exit(0);
  };
  
  process.on('SIGINT', () => handleExit('SIGINT'));
  process.on('SIGTERM', () => handleExit('SIGTERM'));
  process.on('SIGHUP', () => handleExit('SIGHUP'));
  
  if (process.platform === 'win32') {
    process.on('SIGBREAK', () => handleExit('SIGBREAK'));
  }
  
  // Cleanup on exit
  process.on('exit', () => {
    cleanup(viteProcess);
  });
  
  // Monitor lock file (for parent process death detection)
  const lockMonitor = setInterval(() => {
    // Check if our lock file still exists
    if (!fs.existsSync(LOCK_FILE)) {
      console.log('\n⚠️  Lock file deleted, assuming parent terminated');
      clearInterval(lockMonitor);
      cleanup(viteProcess);
      process.exit(0);
    }
    
    // On Windows, also check parent PID
    if (process.platform === 'win32' && process.ppid) {
      try {
        // This will throw if process doesn't exist
        process.kill(process.ppid, 0);
      } catch (e) {
        console.log('\n⚠️  Parent process terminated');
        clearInterval(lockMonitor);
        cleanup(viteProcess);
        process.exit(0);
      }
    }
  }, 1000);
  
  try {
    // Clean up any stale files from previous runs
    console.log('🔍 Checking for stale processes...');
    
    // Kill any existing Vite processes on our ports
    if (process.platform === 'win32') {
      try {
        // Find processes using port 5173-5273
        for (let port = 5173; port < 5273; port++) {
          try {
            const result = execSync(`netstat -ano | findstr :${port}`, { encoding: 'utf8' });
            const lines = result.split('\n').filter(line => line.includes('LISTENING'));
            for (const line of lines) {
              const match = line.match(/\s+(\d+)\s*$/);
              if (match) {
                const pid = match[1];
                console.log(`Found process ${pid} on port ${port}, killing...`);
                try {
                  execSync(`taskkill /pid ${pid} /f`, { stdio: 'ignore' });
                } catch (e) {
                  // Process might already be gone
                }
              }
            }
          } catch (e) {
            // No process on this port
          }
        }
      } catch (e) {
        // Ignore errors
      }
    }
    
    // Clean up old files
    [PORT_FILE, PARENT_PORT_FILE].forEach(file => {
      if (fs.existsSync(file)) {
        fs.unlinkSync(file);
      }
    });
    
    // Find available port
    console.log('\n🔍 Finding available port...');
    const port = await findAvailablePort();
    console.log(`✓ Selected port: ${port}`);
    
    // Write port files
    fs.writeFileSync(PORT_FILE, port.toString());
    fs.writeFileSync(PARENT_PORT_FILE, port.toString());
    console.log('✓ Port files created');
    
    // Start Vite
    console.log('\n🚀 Starting Vite dev server...');
    viteProcess = spawn('npm', ['run', 'dev'], {
      env: { ...process.env, VITE_PORT: port.toString() },
      cwd: __dirname,
      shell: true,
      detached: process.platform !== 'win32',
      stdio: 'pipe'
    });
    
    // Pipe output
    viteProcess.stdout.on('data', (data) => {
      process.stdout.write(`[Vite] ${data}`);
    });
    
    viteProcess.stderr.on('data', (data) => {
      process.stderr.write(`[Vite Error] ${data}`);
    });
    
    viteProcess.on('error', (error) => {
      console.error(`Failed to start Vite: ${error}`);
      cleanup(viteProcess);
      process.exit(1);
    });
    
    viteProcess.on('exit', (code, signal) => {
      console.log(`Vite process exited with code ${code} and signal ${signal}`);
      clearInterval(lockMonitor);
      cleanup(viteProcess);
      process.exit(code || 0);
    });
    
    // Wait for server
    console.log('\n⏳ Waiting for Vite server to start...');
    await waitForServer(port);
    
    console.log(`\n✅ Vite dev server is running at http://localhost:${port}`);
    console.log('🛑 Press Ctrl+C to stop\n');
    
    // Keep process alive
    process.stdin.resume();
    
  } catch (error) {
    console.error('\n❌ Error:', error.message);
    cleanup(viteProcess);
    process.exit(1);
  }
}

// Run
main().catch((err) => {
  console.error('Fatal error:', err);
  process.exit(1);
});