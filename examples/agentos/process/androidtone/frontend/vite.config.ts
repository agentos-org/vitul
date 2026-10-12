import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  plugins: [react(), tailwindcss()],
  base: '/apps/androidtone/',
  build: { outDir: '../public', emptyOutDir: true, rollupOptions: { input: { index: fileURLToPath(new URL('./index.html', import.meta.url)), credits: fileURLToPath(new URL('./credits.html', import.meta.url)) } } },
  server: {
    host: '127.0.0.1', port: 5187, strictPort: true,
    proxy: { '/apps/androidtone/api': 'http://127.0.0.1:3000' },
  },
});
