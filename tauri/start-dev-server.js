#!/usr/bin/env node

import { spawn } from 'child_process';
import net from 'net';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import readline from 'readline';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Function to find an available port
async function findAvailablePort(startPort = 5173) {
  for (let port = startPort; port < startPort + 100; port++) {
    const isAvailable = await checkPortAvailable(port);
    if (isAvailable) {
      console.log(`Port ${port} is available`);
      return port;
    } else {
      console.log(`Port ${port} is in use, trying next...`);
    }
  }
  throw new Error('No available ports found');
}

function checkPortAvailable(port) {
  return new Promise((resolve) => {
    const server = net.createServer();
    
    server.once('error', (err) => {
      resolve(false);
    });
    
    server.once('listening', () => {
      server.close(() => {
        // Add a small delay after closing to ensure port is fully released
        setTimeout(() => resolve(true), 100);
      });
    });
    
    // Listen on all interfaces to properly detect port usage
    // server.listen(port, '0.0.0.0');
    server.listen(port);
  });
}

// Function to check if server is ready
function waitForServer(port, maxAttempts = 60) {
  return new Promise((resolve, reject) => {
    let attempts = 0;
    
    const check = () => {
      const client = new net.Socket();
      client.setTimeout(200);
      
      client.once('connect', () => {
        client.destroy();
        resolve(true);
      });
      
      client.once('timeout', () => {
        client.destroy();
        attempts++;
        if (attempts >= maxAttempts) {
          reject(new Error(`Server didn't start after ${maxAttempts} attempts`));
        } else {
          if (attempts % 10 === 0) {
            console.log(`Waiting for server... (${attempts} attempts)`);
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
    setTimeout(check, 1000);
  });
}

async function main() {
  let viteProcess = null;
  
  // Cleanup function
  const cleanup = (exitCode = 0) => {
    console.log('\nShutting down dev server...');
    
    // Clean up port files
    const portFile = path.join(__dirname, '.current-port');
    if (fs.existsSync(portFile)) {
      try {
        fs.unlinkSync(portFile);
        console.log('Cleaned up port file');
      } catch (e) {
        // Ignore errors
      }
    }
    
    const parentPortFile = path.join(__dirname, '..', '.current-port');
    if (fs.existsSync(parentPortFile)) {
      try {
        fs.unlinkSync(parentPortFile);
        console.log('Cleaned up parent port file');
      } catch (e) {
        // Ignore errors
      }
    }
    
    // Kill Vite process
    if (viteProcess && !viteProcess.killed) {
      console.log(`Terminating Vite process (PID: ${viteProcess.pid})...`);
      
      try {
        // On Windows, use taskkill to kill the entire process tree
        if (process.platform === 'win32') {
          const killProcess = spawn('taskkill', ['/pid', viteProcess.pid.toString(), '/f', '/t'], {
            stdio: 'inherit'  // Show output for debugging
          });
          
          killProcess.on('exit', () => {
            console.log('Vite process terminated');
            process.exit(exitCode);
          });
          
          // Fallback exit after timeout
          setTimeout(() => {
            console.log('Forcing exit...');
            process.exit(exitCode);
          }, 2000);
        } else {
          // On Unix, kill the process group
          process.kill(-viteProcess.pid, 'SIGTERM');
          setTimeout(() => {
            process.exit(exitCode);
          }, 1000);
        }
      } catch (err) {
        console.error('Error killing process:', err);
        process.exit(exitCode);
      }
    } else {
      console.log('No Vite process to terminate');
      process.exit(exitCode);
    }
  };
  
  // Track if cleanup has been called to prevent multiple calls
  let cleanupCalled = false;
  
  // Wrapper to ensure cleanup is only called once
  const cleanupOnce = (code) => {
    if (!cleanupCalled) {
      cleanupCalled = true;
      cleanup(code);
    }
  };
  
  // Register signal handlers for graceful shutdown
  process.on('SIGINT', () => {
    console.log('Received SIGINT signal');
    cleanupOnce(0);
  });
  
  process.on('SIGTERM', () => {
    console.log('Received SIGTERM signal');
    cleanupOnce(0);
  });
  
  // Windows-specific handling
  if (process.platform === 'win32') {
    // Handle Ctrl+C in Windows console
    process.on('SIGBREAK', () => {
      console.log('Received SIGBREAK signal (Windows Ctrl+Break)');
      cleanupOnce(0);
    });
    
    // Also try readline interface
    const rl = readline.createInterface({
      input: process.stdin,
      output: process.stdout
    });
    
    rl.on('SIGINT', () => {
      console.log('Received SIGINT via readline');
      cleanupOnce(0);
    });
  }
  
  try {
    // Clean up any stale port files from previous runs
    const portFile = path.join(__dirname, '.current-port');
    if (fs.existsSync(portFile)) {
      try {
        fs.unlinkSync(portFile);
      } catch (e) {
        // Ignore errors
      }
    }
    
    const parentPortFile = path.join(__dirname, '..', '.current-port');
    if (fs.existsSync(parentPortFile)) {
      try {
        fs.unlinkSync(parentPortFile);
      } catch (e) {
        // Ignore errors
      }
    }
    
    // Find an available port
    const port = await findAvailablePort();
    console.log(`Found available port: ${port}`);
    
    // Write port to temp file for Tauri to read  
    // Also write to parent directory where Rust expects it
    fs.writeFileSync(portFile, port.toString());
    const rustPortFile = path.join(__dirname, '..', '.current-port');
    fs.writeFileSync(rustPortFile, port.toString());
    
    // Start Vite with the selected port
    console.log(`Starting Vite dev server on port ${port}...`);
    viteProcess = spawn('npm', ['run', 'dev'], {
      env: { ...process.env, VITE_PORT: port.toString() },
      cwd: __dirname,
      shell: true,
      detached: process.platform !== 'win32',  // Detach on Unix for process group
      windowsHide: true  // Hide console window on Windows
    });
    
    viteProcess.stdout.on('data', (data) => {
      console.log(`Vite: ${data}`);
    });
    
    viteProcess.stderr.on('data', (data) => {
      console.error(`Vite Error: ${data}`);
    });
    
    viteProcess.on('error', (error) => {
      console.error(`Failed to start Vite: ${error}`);
      cleanup(1);
    });
    
    viteProcess.on('exit', (code, signal) => {
      console.log(`Vite process exited with code ${code} and signal ${signal}`);
      if (code !== null && code !== 0) {
        cleanup(code);
      }
    });
    
    // Wait for server to be ready
    console.log('Waiting for Vite to be ready...');
    await waitForServer(port);
    console.log(`Vite dev server is ready on http://localhost:${port}`);
    
    // Keep the process running
    process.stdin.resume();
    
  } catch (error) {
    console.error('Error:', error);
    cleanup(1);
  }
}

main();