import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'
import { fileURLToPath } from 'node:url'

export default defineConfig({
  root: fileURLToPath(new URL('.', import.meta.url)),
  plugins: [react(), tailwindcss()],
  base: './',
  clearScreen: false,
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    sourcemap: false
  },
  server: {
    host: '127.0.0.1',
    port: 5186,
    strictPort: true,
    proxy: {
      '/apps/wintone/api': {
        target: 'http://127.0.0.1:3000',
        changeOrigin: false
      }
    }
  }
})
