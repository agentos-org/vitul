# AgentOS process app examples

This directory groups AgentOS process packages. “Process” describes how an
app runs; the examples use different packaging workflows and include both
native host execution and container sandboxing.

## Examples

- [Dockge](dockge/README.md) packages the unmodified upstream app as a native
  AgentOS app. Its build script and vendored source are kept together.
- [Gitea](gitea/README.md) packages the official Gitea binary and runs it in a
  container sandbox. A tray helper keeps the service running when its workspace
  window closes.
- [Toney Music](toney-music/README.md) adapts a React/Vite UI and Rust `musicd`
  backend through a Vitulfile, with runtime-discovered OpenTool methods.
- [Frame Video](frame-video/README.md) is a native Vitul app and an AgentOS
  process package with a Rust OpenTool backend for the local video library.

The Toney Music source snapshot is based on AgentOS commit `a97448524`, from
`webshell/apps/toney-music`. It is managed independently from the neighboring
AgentOS checkout.

Build Toney Music from its directory with the Vitul CLI:

```sh
vitul agentos validate
vitul build --target agentos/arm64
```

The Toney Music build needs Node.js and pnpm for the frontend, Rust and Cargo
for its backend, and the FFmpeg binary described in its README. Dockge and Gitea
have app-specific build scripts; Frame Video's desktop and AgentOS build
instructions are in its README.
