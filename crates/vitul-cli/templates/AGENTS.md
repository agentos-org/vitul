# Instructions for coding agents

This project is a Vitul app with a preset stack: React, TypeScript, Tailwind CSS
4, Vite, pnpm, a Rust OpenTool backend, and the `agentos-sdk-ts` package.

## Project map

- `src/host-context.ts` initializes language/theme and subscribes to host changes.
- `src/messages.ts` holds UI translations; `src/styles.css` defines theme tokens.
- `src/App.tsx` is the main application screen.
- `src/components/ui/` contains small reusable interface components.
- `src/opentool.ts` calls this app's Rust business backend.
- `src/lib/agentos.ts` creates a client for AgentOS Gateway APIs on demand.
- `src/host.ts` defines operating-system and host operations; use it for windows,
  dialogs, clipboard, notifications, and sidecars.
- `backend/src/tools/` contains business operations exposed through OpenTool.
- `backend/src/lib.rs` creates the OpenTool service; `backend/src/main.rs` hosts
  it for desktop and AgentOS. Mobile apps embed the service through `src-vitul/`.
- `vitul.conf.json` is the project configuration. Do not hand-edit generated
  runtime files under `src-vitul/`.

## API boundaries

- Put app-specific business operations in `backend/src/tools/` and call them
  through `src/opentool.ts`.
- Use `createAgentOSClient()` only for AgentOS Gateway APIs. The SDK is already
  a dependency; do not add another HTTP client for those APIs.
- Use the host provider in `src/host.ts` for native or AgentOS host features.
- Do not put secrets in frontend code or `VITE_*` environment variables. Those
  values are included in the built app.

## Working with the preset

- `app-icon.png` is the editable app icon master. Keep its background full-bleed
  across the square canvas; do not add an outer frame, transparent padding, or
  pre-rounded corners. After replacing it, run
  `pnpm vitul icon app-icon.png --output src-vitul/icons` to regenerate all sizes.
- Keep the preset React, TypeScript, Tailwind CSS 4, Vite, and pnpm setup intact.
- Put screens and app-specific UI in `src/`; add shared UI primitives to
  `src/components/ui/`.
- Keep package versions and the pnpm version aligned with `package.json`.
- Prefer the existing API boundaries and components when extending the app.

## Useful commands

- `pnpm check` checks TypeScript projects.
- `pnpm build` builds the frontend assets.
- `pnpm vitul dev` starts the app and its local OpenTool backend.
- `cargo check --manifest-path backend/Cargo.toml` checks the Rust backend.
