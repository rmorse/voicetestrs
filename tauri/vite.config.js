import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  build: {
    outDir: './dist',
    emptyOutDir: true
  },
  server: {
    port: 5173,
    strictPort: false,  // Allow Vite to find next available port if 5173 is busy
    host: 'localhost',
    warmup: {
      clientFiles: ['./src/**/*']  // Pre-bundle client files
    }
  },
  optimizeDeps: {
    include: ['react', 'react-dom', '@tauri-apps/api', '@tauri-apps/plugin-sql'],  // Pre-bundle dependencies
    force: true  // Force optimization on startup
  }
})