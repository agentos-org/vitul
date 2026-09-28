// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

import { defineConfig } from 'vite'
import Unocss from 'unocss/vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

const host = process.env.VITUL_DEV_HOST
const backendPort = process.env.VITUL_OPENTOOL_PORT || '9627'

// https://vite.dev/config/
export default defineConfig({
  plugins: [Unocss(), svelte()],
  build: {
    emptyOutDir: false,
    rollupOptions: {
      output: {
        entryFileNames: `assets/[name].js`,
        chunkFileNames: `assets/[name].js`,
        assetFileNames: `assets/[name].[ext]`
      }
    }
  },

  // Vite options used during Vitul development and builds.
  // prevent Vite from obscuring rust errors
  clearScreen: false,
  // Vitul expects a fixed port, fail if that port is not available.
  server: {
    host: host ?? false,
    port: 1420,
    strictPort: true,
    hmr: host
      ? {
          protocol: 'ws',
          host: host,
          port: 1430
        }
      : undefined,
    proxy: {
      '/opentool': {
        target: `http://127.0.0.1:${backendPort}`,
        changeOrigin: false
      }
    },
    fs: {
      allow: ['.', '../../packages/api/dist']
    }
  }
})
