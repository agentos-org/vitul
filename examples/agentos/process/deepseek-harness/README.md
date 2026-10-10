# DeepSeek Harness for AgentOS

This example packages DeepSeek Harness (`dsh`) as a Linux AgentOS process app. It uses the upstream Web profile and a Linux single-file executable built by the upstream repository's Python runtime build pipeline. The copied package contains the executable and its required runtime sidecars, not a second copy of the upstream source tree.

The AgentOS container image ships an older Node.js version than DSH supports, so the package includes the matching Node 24 executable, Python runtime, Office dependencies, and ripgrep output from that pipeline. The package includes the PNG app icon used by AgentOS. A small local proxy preserves the upstream browser-session authentication: it privately captures the launch token from DSH startup output, completes the initial 303 cookie exchange internally, and keeps the signed session cookie in process memory. Since WebShell does not forward browser cookies to process apps, the proxy attaches that cookie to subsequent API and WebSocket requests. When AgentOS supplies `AGENTOS_APP_PUBLIC_URL`, the proxy marks the authenticated workspace page so DSH's model settings use their Host-backed store, and the build includes plugin SVG metadata assets in the executable. The launch token is not printed to AgentOS logs.

Build one Linux architecture from the Vitul repository root:

```sh
DSH_PLATFORM=linux/arm64 ./examples/agentos/process/deepseek-harness/build.sh
DSH_PLATFORM=linux/amd64 ./examples/agentos/process/deepseek-harness/build.sh
```

The default source is `../GitHub/deepseek-harness`. Set `DSH_SOURCE_DIR` to use another checkout. The build copies that checkout into a disposable Linux container, installs its locked dependencies, builds DSH and its Web assets, then assembles the `.agt`; it does not edit the source checkout. Docker, network access from the build container, and sufficient build disk space are required. `DSH_BUILD_IMAGE` selects the Linux build image and `DSH_OUTPUT` changes the package path. The default outputs are `examples/agentos/process/deepseek-harness/dist/deepseek-harness-arm64.agt` and `deepseek-harness-x64.agt`.

The build records the DSH version and Git commit in `build-info.json`, and preserves the upstream MIT license and third-party notices. Install the `.agt` through Agent Store. DSH settings, credentials, sessions, caches, and the writable agent workspace are stored under the app's persistent `DATA_DIR`. The first run opens the DSH Web UI; configure a model provider in its settings before starting an agent. Automatic startup is disabled.

This package runs in AgentOS's container sandbox. DSH also confines agent commands to its persistent workspace using its own Linux sandbox backend. Kernel and runtime compatibility for that nested command sandbox should be confirmed on the target AgentOS device.
