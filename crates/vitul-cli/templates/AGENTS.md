# Instructions for coding agents

This project is a Vitul app with a preset stack: React, TypeScript, Tailwind CSS
4, Vite, pnpm, a Rust OpenTool backend, and the `agentos-sdk-ts` package.

## Requirement workflow

Apply this workflow to every feature request, including configuration-only changes.
Read [VITUL.md](VITUL.md) (or [Chinese](VITUL-zh_CN.md)) for the requirement map
and configuration diagnosis before selecting APIs or copying an example.

1. Translate the user's request into observable acceptance conditions on each
   requested platform. Include failure handling, persistence, and lifecycle when
   relevant. Resolve routine choices from project context; ask only when missing
   information changes the intended result.
2. Confirm support using the installed CLI/API and target host versions. Identify
   frontend, OpenTool backend, host, or Gateway ownership. On AgentOS, choose app
   type and execution mode from the complete need and isolation requirements.
3. Trace the entire dependency chain: root configuration, permission/authorization,
   implementation and actual call sites, assets/binaries and architecture, build
   collection, startup, data paths, and cleanup. Explain any inapplicable step.
4. Implement all required layers. A permission is not an implementation; a packaged
   executable is not a running process; declared persistence is not a data path.
   Check what an example's build and startup scripts do before reusing its config.
5. Verify configuration, code, package contents, and user behavior separately on
   the target host. Browser or desktop success does not prove another target.
6. Report results per acceptance condition with checks actually run. Mark missing
   host verification and unsupported behavior explicitly; do not claim success
   from configuration validation or a successful build alone.

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
