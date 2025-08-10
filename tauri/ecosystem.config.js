module.exports = {
  apps: [{
    name: 'vite-dev',
    script: 'npm',
    args: 'run dev',
    cwd: './tauri',
    env: {
      VITE_PORT: process.env.VITE_PORT || '5173'
    },
    watch: false,
    autorestart: false,
    max_restarts: 0,
    kill_timeout: 3000,
    shutdown_with_message: true,
    wait_ready: true,
    listen_timeout: 10000
  }]
};