import net from 'net';

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

// Find port and output it
const port = await findAvailablePort();
console.log(port);