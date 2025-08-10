#!/usr/bin/env node

import { spawn } from 'child_process';

console.log('Starting dev server test...');

const child = spawn('node', ['start-dev-server.js'], {
  stdio: 'inherit',
  shell: false
});

console.log(`Started process with PID: ${child.pid}`);

// After 10 seconds, send SIGINT (Ctrl+C)
setTimeout(() => {
  console.log('\n=== Sending SIGINT to simulate Ctrl+C ===');
  child.kill('SIGINT');
}, 10000);

child.on('exit', (code, signal) => {
  console.log(`\n=== Process exited with code ${code} and signal ${signal} ===`);
  process.exit(0);
});