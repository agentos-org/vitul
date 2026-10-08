# Gitea standalone sandboxed app

This example packages the official Gitea binary as a standalone AgentOS `.agt`
app. Gitea runs in the AgentOS container sandbox; its database, configuration,
repositories, uploads, sessions, logs, home directory, and temporary files stay
under the app's persistent `DATA_DIR`. Git is bundled for use inside the
sandbox, so the host does not need Git installed.

The package declares AgentOS's `native.tray` capability. A small statically
compiled Go helper registers the packaged Gitea icon with the AgentOS Shell
and consumes tray events while the backend runs. Closing the Workspace window
leaves Gitea and its tray icon running; the Shell tray menu can reopen Gitea or
exit it. The helper does not modify the upstream Gitea executable.

The build uses **Gitea 1.27.3** from the [official download host](https://dl.gitea.com/gitea/1.27.3/).
The [1.27.3 release](https://blog.gitea.com/release-of-1.27.3/) includes security
fixes that are absent from the cached Gitea 1.26.4 copy in the older AgentOS
checkout; upstream marks versions through 1.26.4 as affected by
[GHSA-q9pg-jj6x-j9p6](https://github.com/go-gitea/gitea/security/advisories/GHSA-q9pg-jj6x-j9p6).
The upstream executable is not patched. Gitea binaries include the web assets
and SQLite support. The build script verifies the official binary SHA-256 for
each supported architecture before packaging it.

Build one Linux architecture at a time:

~~~sh
GITEA_PLATFORM=linux/arm64 ./examples/gitea/build.sh
GITEA_PLATFORM=linux/amd64 ./examples/gitea/build.sh
~~~

Run these commands from the Vitul repository root. The default outputs are
`examples/gitea/dist/gitea-arm64.agt` and `examples/gitea/dist/gitea-x64.agt`.
The build needs Go to compile the tray helper and Docker to stage Git and its
runtime libraries for the matching AgentOS container architecture. `GITEA_BINARY`, `GITEA_CACHE_DIR`,
`GIT_RUNTIME_CACHE_DIR`, `GITEA_OUTPUT`, and `GITEA_BUILD_IMAGE` can override
the downloaded binary, caches, output path, and Ubuntu build image. Set
`GITEA_CURL_DIRECT=true` to bypass configured curl proxies when downloading
the binary.

Install the resulting `.agt` through Agent Store. Gitea opens in the current
AgentOS Workspace and serves HTTP on port 3000. SSH is disabled; Git clone and
push use HTTP. The first account registered on a fresh instance is the
administrator. The HTTP endpoint is provided by AgentOS on loopback, and the
app itself binds to `0.0.0.0` inside its container. Automatic startup is off.

The package includes the upstream [MIT license](https://github.com/go-gitea/gitea/blob/v1.27.3/LICENSE) in `LICENSE`.
