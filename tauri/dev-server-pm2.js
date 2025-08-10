#!/usr/bin/env node

import pm2 from 'pm2';
import net from 'net';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import { spawn } from 'child_process';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const PM2_APP_NAME = 'voicetextrs-vite-dev';
const PORT_FILE = path.join(__dirname, '.current-port');
const PARENT_PORT_FILE = path.join(__dirname, '..', '.current-port');

// Find available port
async function findAvailablePort(startPort = 5173) {
  for (let port = startPort; port < startPort + 100; port++) {
    const isAvailable = await checkPortAvailable(port);
    if (isAvailable) {
      console.log(`✓ Port ${port} is available`);
      return port;
    } else {
      console.log(`✗ Port ${port} is in use, trying next...`);
    }
  }
  throw new Error('No available ports found');
}

function checkPortAvailable(port) {
  return new Promise((resolve) => {
    const server = net.createServer();
    
    server.once('error', () => resolve(false));
    
    server.once('listening', () => {
      server.close(() => {
        // Small delay to ensure port is fully released
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
    
    // Start checking after a small delay
    setTimeout(check, 1500);
  });
}

// Clean up function
async function cleanup(exitCode = 0) {
  console.log('\n🧹 Cleaning up PM2 processes...');
  
  return new Promise((resolve) => {
    pm2.connect((err) => {
      if (err) {
        console.error('Failed to connect to PM2:', err);
        cleanupPortFiles();
        process.exit(exitCode);
        return;
      }
      
      // Stop and delete the Vite process
      pm2.delete(PM2_APP_NAME, (err) => {
        if (err && !err.message.includes('not found')) {
          console.error('Error stopping PM2 process:', err);
        } else {
          console.log('✓ PM2 process stopped');
        }
        
        // Disconnect from PM2
        pm2.disconnect();
        
        // Clean up port files
        cleanupPortFiles();
        
        // Exit after a short delay
        setTimeout(() => {
          console.log('✓ Cleanup complete');
          process.exit(exitCode);
        }, 500);
      });
    });
  });
}

function cleanupPortFiles() {
  // Clean up port files
  [PORT_FILE, PARENT_PORT_FILE].forEach(file => {
    if (fs.existsSync(file)) {
      try {
        fs.unlinkSync(file);
        console.log(`✓ Cleaned up ${path.basename(file)}`);
      } catch (e) {
        // Ignore errors
      }
    }
  });
}

// Main function
async function main() {
  // Set up signal handlers
  let cleanupInProgress = false;
  
  const handleExit = async (signal) => {
    if (cleanupInProgress) return;
    cleanupInProgress = true;
    
    console.log(`\n📡 Received ${signal} signal`);
    await cleanup(0);
  };
  
  process.on('SIGINT', () => handleExit('SIGINT'));
  process.on('SIGTERM', () => handleExit('SIGTERM'));
  process.on('SIGHUP', () => handleExit('SIGHUP'));
  
  // Windows-specific signals
  if (process.platform === 'win32') {
    process.on('SIGBREAK', () => handleExit('SIGBREAK'));
  }
  
  // Monitor if parent process dies (cross-platform)
  const parentPid = process.ppid;
  if (parentPid) {
    setInterval(() => {
      try {
        // Check if parent process still exists
        // This works on both Windows and Unix
        process.kill(parentPid, 0);
      } catch (e) {
        // Parent process is gone, cleanup and exit
        console.log('\n⚠️  Parent process terminated, cleaning up...');
        handleExit('PARENT_EXIT');
      }
    }, 1000);
  }
  
  // Handle process exit
  process.on('exit', () => {
    console.log('\n📡 Process exiting, ensuring cleanup...');
    if (!cleanupInProgress) {
      // Synchronous cleanup attempt
      try {
        const { execSync } = require('child_process');
        execSync(`npx pm2 delete ${PM2_APP_NAME}`, { stdio: 'ignore' });
      } catch (e) {
        // Ignore errors
      }
    }
  });
  
  // Handle uncaught errors
  process.on('uncaughtException', (err) => {
    console.error('Uncaught exception:', err);
    cleanup(1);
  });
  
  process.on('unhandledRejection', (err) => {
    console.error('Unhandled rejection:', err);
    cleanup(1);
  });
  
  try {
    // Clean up any existing PM2 processes and port files from previous runs
    console.log('🔍 Checking for existing processes...');
    
    await new Promise((resolve) => {
      pm2.connect((err) => {
        if (err) {
          console.log('PM2 not running, starting fresh');
          resolve();
          return;
        }
        
        pm2.describe(PM2_APP_NAME, (err, processDescription) => {
          if (!err && processDescription && processDescription.length > 0) {
            console.log('⚠️  Found existing PM2 process, cleaning up...');
            pm2.delete(PM2_APP_NAME, () => {
              console.log('✓ Cleaned up existing process');
              pm2.disconnect();
              resolve();
            });
          } else {
            pm2.disconnect();
            resolve();
          }
        });
      });
    });
    
    // Clean up stale port files
    cleanupPortFiles();
    
    // Find available port
    console.log('\n🔍 Finding available port...');
    const port = await findAvailablePort();
    console.log(`✓ Selected port: ${port}`);
    
    // Write port to files for Tauri to read
    fs.writeFileSync(PORT_FILE, port.toString());
    fs.writeFileSync(PARENT_PORT_FILE, port.toString());
    console.log('✓ Port files created');
    
    // Start Vite using PM2
    console.log('\n🚀 Starting Vite dev server with PM2...');
    
    await new Promise((resolve, reject) => {
      pm2.connect((err) => {
        if (err) {
          reject(err);
          return;
        }
        
        // Create a simple runner script for PM2
        const runnerScript = path.join(__dirname, 'run-vite.js');
        const runnerContent = `
import { spawn } from 'child_process';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const vite = spawn('npm', ['run', 'dev'], {
  cwd: __dirname,
  env: { ...process.env, VITE_PORT: process.env.VITE_PORT },
  stdio: 'inherit',
  shell: true
});

vite.on('error', (err) => {
  console.error('Failed to start Vite:', err);
  process.exit(1);
});

vite.on('exit', (code) => {
  process.exit(code || 0);
});
`;
        fs.writeFileSync(runnerScript, runnerContent);
        
        pm2.start({
          name: PM2_APP_NAME,
          script: runnerScript,
          cwd: __dirname,
          env: {
            ...process.env,
            VITE_PORT: port.toString(),
            FORCE_COLOR: '1'
          },
          autorestart: false,
          max_restarts: 0,
          min_uptime: 0,
          watch: false,
          merge_logs: true,
          log_type: 'json',
          error_file: path.join(__dirname, 'logs', 'vite-error.log'),
          out_file: path.join(__dirname, 'logs', 'vite-out.log'),
          time: true
        }, (err, apps) => {
          if (err) {
            reject(err);
            return;
          }
          
          console.log('✓ PM2 process started');
          
          // Stream logs from PM2
          pm2.launchBus((err, bus) => {
            if (!err) {
              bus.on('log:out', (packet) => {
                if (packet.process.name === PM2_APP_NAME) {
                  console.log(`[Vite] ${packet.data}`);
                }
              });
              
              bus.on('log:err', (packet) => {
                if (packet.process.name === PM2_APP_NAME) {
                  console.error(`[Vite Error] ${packet.data}`);
                }
              });
            }
          });
          
          resolve();
        });
      });
    });
    
    // Wait for server to be ready
    console.log('\n⏳ Waiting for Vite server to start...');
    await waitForServer(port);
    
    console.log(`\n✅ Vite dev server is running at http://localhost:${port}`);
    console.log('📝 PM2 process name:', PM2_APP_NAME);
    console.log('💡 Use "npx pm2 status" to check process status');
    console.log('🛑 Press Ctrl+C to stop\n');
    
    // Keep process alive
    process.stdin.resume();
    
  } catch (error) {
    console.error('\n❌ Error:', error.message);
    await cleanup(1);
  }
}

// Run the main function
main().catch(async (err) => {
  console.error('Fatal error:', err);
  await cleanup(1);
});