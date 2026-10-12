import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import process from 'node:process'

const host = process.env.VITUL_DEV_HOST
const backendPort = process.env.VITUL_OPENTOOL_PORT || '9627'

export default defineConfig({
  base: './',
  clearScreen: false,
  plugins: [react(), tailwindcss()],
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: 'ws', host, port: 1421 } : undefined,
    proxy: {
      '/opentool': {
        target: `http://127.0.0.1:${backendPort}`,
        changeOrigin: false
      }
    },
    watch: { ignored: ['**/src-vitul/**', '**/backend/**'] }
  }
})
