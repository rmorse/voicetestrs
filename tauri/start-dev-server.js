#!/usr/bin/env node

import { spawn } from 'child_process';
import net from 'net';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Function to find an available port
async function findAvailablePort(startPort = 5173) {
  for (let port = startPort; port < startPort + 100; port++) {
    const isAvailable = await checkPortAvailable(port);
    if (isAvailable) {
      return port;
    }
  }
  throw new Error('No available ports found');
}

function checkPortAvailable(port) {
  return new Promise((resolve) => {
    const server = net.createServer();
    server.once('error', () => resolve(false));
    server.once('listening', () => {
      server.close();
      resolve(true);
    });
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
  try {
    // Find an available port
    const port = await findAvailablePort();
    console.log(`Found available port: ${port}`);
    
    // Write port to temp file for Tauri to read
    const portFile = path.join(__dirname, '.current-port');
    fs.writeFileSync(portFile, port.toString());
    
    // Start Vite with the selected port
    console.log(`Starting Vite dev server on port ${port}...`);
    const viteProcess = spawn('npm', ['run', 'dev'], {
      env: { ...process.env, VITE_PORT: port.toString() },
      cwd: __dirname,
      shell: true
    });
    
    viteProcess.stdout.on('data', (data) => {
      console.log(`Vite: ${data}`);
    });
    
    viteProcess.stderr.on('data', (data) => {
      console.error(`Vite Error: ${data}`);
    });
    
    viteProcess.on('error', (error) => {
      console.error(`Failed to start Vite: ${error}`);
      process.exit(1);
    });
    
    // Wait for server to be ready
    console.log('Waiting for Vite to be ready...');
    await waitForServer(port);
    console.log(`Vite dev server is ready on http://localhost:${port}`);
    
    // Keep the process running
    process.stdin.resume();
    
  } catch (error) {
    console.error('Error:', error);
    process.exit(1);
  }
}

main();