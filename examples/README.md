# Vitul examples

Vitul's native application flow uses OpenTool for application operations and the
host API for system features. Start with [window-lab](window-lab/README.md),
which demonstrates both in a compact desktop and AgentOS app.

[process/frame-video](process/frame-video/README.md) is a larger native example
that also packages as an AgentOS process app. It keeps the AgentOS Frame Video
interface and uses a Rust OpenTool backend for the media library and video
streaming.

## AgentOS app profiles

[process](process/README.md) groups AgentOS process app examples. Process is the
app packaging model; OpenTool is the business API, and both examples expose
OpenTool methods. Toney Music adapts an existing frontend and backend, while
Frame Video also builds as a native desktop app.

[url](url/README.md) shows how to package an already hosted HTTPS website as an AgentOS URL app.

The desktop API integration fixture used by CI lives under
[tests/fixtures/desktop-api](../tests/fixtures/desktop-api/README.md). It covers
compatibility API behavior and is intended for validation work.

See each README for build requirements and run instructions.
