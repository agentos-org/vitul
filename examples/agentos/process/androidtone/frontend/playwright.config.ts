import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './tests',
  use: {
    baseURL: 'http://127.0.0.1:5188', locale: 'zh-CN',
    launchOptions: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE
      ? { executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE } : {},
  },
  webServer: {
    command: 'pnpm preview --host 127.0.0.1 --port 5188 --strictPort',
    url: 'http://127.0.0.1:5188/apps/androidtone/', reuseExistingServer: false,
  },
});
