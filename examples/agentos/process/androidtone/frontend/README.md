# AndroidTone frontend

React + TypeScript + Tailwind CSS + Vite. This directory is an independent pnpm project (Node.js 22.12+). Use its `pnpm-lock.yaml`, not the repository root lockfile.

```sh
pnpm install --frozen-lockfile
pnpm dev
pnpm build
pnpm exec playwright install chromium
pnpm test
```

Development URL: `http://127.0.0.1:5187/apps/androidtone/`. Start the Rust backend on port 3000 for real API requests. Production files are generated into `../public` and served by the existing Rust process. Both packaging paths rebuild this directory before copying those files into `.agt`.

Tests load the production output and mock the provider API; build before testing. To use an existing Chromium binary instead of downloading one, set `PLAYWRIGHT_CHROMIUM_EXECUTABLE=/absolute/path/to/chromium`. Screenshots are saved into `../agentos-build/verification`.
