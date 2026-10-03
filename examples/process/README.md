# AgentOS process packaging with Vitulfile

This is a source snapshot of the existing AgentOS Toney Music app, adapted for
Vitul's `Vitulfile` packaging module. It builds an installable AgentOS process
`.agt`; it is not a Tauri/Vitul desktop project and does not add a Rust
intermediary.

The source snapshot is based on AgentOS commit `a97448524`, from
`webshell/apps/toney-music`. It is managed independently from the neighboring
AgentOS checkout.

## Examples

- [Toney Music](toney-music/README.md): React/Vite UI with the existing Go
  `musicd` process. Its package also needs a Linux ARM64 FFmpeg binary, supplied
  through `FFMPEG_BIN` when building.

Build from the example directory with the Vitul CLI:

```sh
vitul agentos validate
vitul build --target agentos/arm64
```

The build machine needs Node.js and pnpm for the frontend; Toney Music also
needs Go and the FFmpeg binary described in its README. For the native Vitul
Frame Video port, see [frame-video](../frame-video/README.md).
