# Dockge standalone App

This example packages the unmodified upstream Dockge source from `vendor/dockge/` as an installable AgentOS `.agt`. The build script and AgentOS launcher are local packaging files; the vendored source is only copied to a temporary directory for compilation.

The vendored source version is **1.5.0**, from the [official upstream tag](https://github.com/louislam/dockge/tree/1.5.0). Its source archive SHA-256 is `a907b54fafeab95bf4de637a43234d54fad2b77c453d8378f67488fc2b3826d9`.

Build one Linux architecture at a time:

~~~sh
DOCKGE_PLATFORM=linux/arm64 ./examples/dockge/build.sh
DOCKGE_PLATFORM=linux/amd64 ./examples/dockge/build.sh
~~~

Run these commands from the Vitul repository root. The default outputs are `examples/dockge/dist/dockge-arm64.agt` and `examples/dockge/dist/dockge-x64.agt`. Container builds use Docker and the pinned Node.js 22 image; npm downloads production dependencies for the selected Linux architecture. `DOCKGE_NPM_CACHE_DIR` and `DOCKGE_OUTPUT` can override the cache and output paths.

Install the resulting `.agt` through Agent Store. Dockge opens maximized in the current AgentOS Workspace. Native execution runs with AgentOS administrator privileges and requires explicit high-risk approval. Dockge uses the host Docker Engine and Docker Compose V2. Its backend binds to loopback on port 15001, and its database and stacks are stored under the app's persistent `DATA_DIR`.
