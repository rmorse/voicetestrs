import { spawn } from 'child_process';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import net from 'net';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

/**
 * Find an available port starting from the given port
 */
async function findAvailablePort(startPort = 5173) {
  let port = startPort;
  const maxAttempts = 100;
  
  for (let i = 0; i < maxAttempts; i++) {
    const isAvailable = await checkPort(port);
    if (isAvailable) {
      return port;
    }
    port++;
  }
  
  throw new Error(`Could not find available port after ${maxAttempts} attempts`);
}

/**
 * Check if a port is available
 */
function checkPort(port) {
  return new Promise((resolve) => {
    const server = net.createServer();
    
    server.once('error', () => {
      resolve(false);
    });
    
    server.once('listening', () => {
      server.close();
      resolve(true);
    });
    
    server.listen(port, '127.0.0.1');
  });
}

async function main() {
  try {
    // Find an available port
    console.log('Finding available port...');
    const port = await findAvailablePort();
    console.log(`Using port: ${port}`);
    
    // Set environment variables
    const env = {
      ...process.env,
      VITE_PORT: port.toString(),
      TAURI_DEV_URL: `http://localhost:${port}`
    };
    
    // Run tauri dev with the environment variables
    const tauriProcess = spawn('npm', ['run', 'tauri:dev:internal'], {
      env,
      stdio: 'inherit',
      shell: true,
      cwd: join(__dirname, '..')
    });
    
    tauriProcess.on('error', (error) => {
      console.error('Failed to start Tauri:', error);
      process.exit(1);
    });
    
    tauriProcess.on('exit', (code) => {
      process.exit(code);
    });
    
  } catch (error) {
    console.error('Error:', error);
    process.exit(1);
  }
}

main();