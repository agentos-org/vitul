# Frame Video

This is a native Vitul project adapted from the AgentOS Frame Video app in
AgentOS commit `a97448524` (`webshell/apps/frame-video`). It keeps the React/Vite
player UI and replaces the AgentOS Fenn Files integration with Vitul/AgentOS
host file dialogs. Its app backend is a Rust OpenTool server; the installed app
does not run a Node.js or Python server.

## What it demonstrates

- A Vitul desktop app and an installable AgentOS app share the same UI and Rust
  backend.
- OpenTool manages the saved library with `library.list`, `library.add`,
  `library.remove`, `library.setFavorite`, and `library.clear`.
- The AgentOS package opts into runtime OpenTool discovery. In AgentOS Settings,
  each `library.*` function can be enabled individually before Chat can call it.
- Rust serves selected local video files with byte-range responses so seeking
  works in the WebView.
- The backend saves library metadata under AgentOS `DATA_DIR` or the desktop
  user's application data directory. Removing a library entry never deletes its
  source video.

## Build and run

From the repository root, install the JavaScript workspace dependencies and
build the API package and Vitul CLI once:

```sh
pnpm install
pnpm build:api
pnpm build:cli:debug
```

Then, from this directory:

```sh
pnpm vitul dev
pnpm vitul build
pnpm vitul build --target agentos/arm64
```

The AgentOS package is written to
`dist/frame-video-0.2.0-agentos-arm64.agt`. AgentOS uses the generic host file
dialog when available; selected paths must be accessible to the app process,
such as files under the mounted user home directory. If the host does not
expose that capability, the app falls back to the browser file picker and plays
the video locally in the frontend. `agentos.bundleId` keeps the original AgentOS ID
`com.litevar.agentos.frameVideo`, and the AgentOS metadata retains the source
app's Chinese name and descriptions. The desktop build uses the Vitul native
dialog.

The build machine needs Node.js and pnpm for Vite, plus Rust for the backend.
The installed package runs the Rust backend and WebView assets; it has no
Python or Node.js application runtime dependency.

## Source map

- `src/` contains the adapted React/Vite player.
- `backend/` implements the OpenTool library and byte-range media route.
- `src-vitul/` contains the native Vitul host and desktop file dialog command.
- `vitul.conf.json` declares the OpenTool backend and AgentOS file-dialog
  permission.
