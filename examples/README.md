# Vitul examples

Vitul's native application flow uses OpenTool for application operations and the
host API for system features. Start with [Vitul Demo](vitul-demo/README.md),
which demonstrates both in a compact desktop and AgentOS app.

[process/frame-video](process/frame-video/README.md) is a larger native example
that also packages as an AgentOS process app. It keeps the AgentOS Frame Video
interface and uses a Rust OpenTool backend for the media library and video
streaming.

[Dockge](dockge/README.md) packages the unmodified upstream app as a standalone
Native AgentOS `.agt`; its source snapshot and build script live together in
that example directory.

[Gitea](gitea/README.md) packages the official Gitea binary as a standalone
AgentOS `.agt` that runs in the container sandbox. The example bundles Git and
keeps all writable state in the app's persistent data directory.

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
