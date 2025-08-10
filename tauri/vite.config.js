import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  build: {
    outDir: './dist',
    emptyOutDir: true
  },
  server: {
    port: process.env.VITE_PORT ? parseInt(process.env.VITE_PORT) : 5173,
    strictPort: true,  // Use exact port since we're finding it dynamically
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