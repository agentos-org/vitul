# AgentOS process app examples

This directory groups AgentOS process apps. “Process” describes the package
and runtime model; OpenTool is the business API and can be used by any app
profile. Both examples expose OpenTool methods. Toney Music adapts an existing
frontend and backend through Vitulfile. Frame Video is also a native Vitul
desktop app.

The source snapshot is based on AgentOS commit `a97448524`, from
`webshell/apps/toney-music`. It is managed independently from the neighboring
AgentOS checkout.

## Examples

- [Toney Music](toney-music/README.md): React/Vite UI with a Rust `musicd`
  process and runtime-discovered OpenTool methods. Its package also needs a
  Linux ARM64 FFmpeg binary, supplied through `FFMPEG_BIN` when building.
- [Frame Video](frame-video/README.md): native Vitul app and AgentOS process
  package with a Rust OpenTool backend for the local video library.

Build Toney Music from its directory with the Vitul CLI:

```sh
vitul agentos validate
vitul build --target agentos/arm64
```

The Toney Music build needs Node.js and pnpm for the frontend, Rust and Cargo
for its backend, and the FFmpeg binary described in its README. Frame Video's
desktop and AgentOS build instructions are in its README.
