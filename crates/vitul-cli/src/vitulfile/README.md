# Vitulfile: package existing web apps for AgentOS

`Vitulfile` is a packaging recipe for turning an existing web application into
an installable AgentOS `.agt` package. It is for projects that already have a
frontend, backend, or hosted website and need an AgentOS manifest, launch
description, and package archive.

This is a separate Vitul CLI module. It is **not** a Rust project format and
does not require `backend/Cargo.toml`, Tauri, or a Rust intermediary. A local
app runs its own configured process. The original `vitul init` workflow for
creating a new Vitul desktop app remains separate.

## Supported profiles

### Existing local web app

Use `type: process` for an app whose own service can listen on one HTTP port
and serve its UI and API. Vitul runs the configured build steps on the build
machine, copies the declared artifacts, writes a small launcher, and creates
the AgentOS package. Node.js, Python, Bash, and common runtimes are available
inside AgentOS; app-specific dependencies still need to be included in the
artifacts.

The build step does not cross-compile an application. A build script that
produces a native executable must produce a Linux ARM64 executable itself.
`VITUL_TARGET=agentos/arm64`, `VITUL_TARGET_OS=linux`,
`VITUL_TARGET_ARCH=arm64`, and `AGENTOS_TARGET=agentos/arm64` are supplied to
each build step.

### Remote website

Use `type: remote-web` for a site already hosted on HTTPS, such as GitHub,
Tencent Video, or Douyin. Vitul creates an AgentOS `appType: url` package whose
`open.url` points to the site. AgentOS opens it in its embedded CEF WebApp
window; it does not launch the operating system browser. Vitul neither downloads
the site nor packages a Node.js or Rust intermediary. Website login, media
playback, navigation, and embedding behavior depend on the website and its own
policies.

Remote pages do not receive AgentOS's native capability bridge. A remote-web
profile cannot request `agentos.permissions`, process storage, or OpenTool
bindings.

## Workflow

```sh
# For a local frontend/backend project
vitul agentos init
# Edit Vitulfile for this project's commands and output paths
```

For a hosted HTTPS site, use this profile instead:

```sh
vitul agentos init --type remote-web --url https://github.com
```

Then inspect, validate, and package the configured project:

```sh
# Review detected files and check the config
vitul agentos inspect
vitul agentos validate

# Process profile: dist/<app>-<version>-agentos-arm64.agt
# Remote-web profile: dist/<app>-<version>-agentos-any.agt
vitul build --target agentos/arm64
```

The CLI target remains `agentos/arm64` for both profiles. A remote URL package
declares `architecture: any` because it contains no platform-specific app
process.

`vitul agentos init --path <DIR>` writes the file in another project
directory. The process starter uses `./bin/server` and `bin/server` as example
command and artifact paths. Edit these for the project before validation and
build. `inspect` reports an incomplete starter config so you can see what needs
to be changed.

## Local app example

```yaml
schemaVersion: 1
target: agentos/arm64

app:
  type: process
  id: com.example.dockage
  name: Dockage
  name_zh: Dockage
  version: 1.0.0
  description: Manage Compose projects from AgentOS.

build:
  steps:
    - command: pnpm
      args: [install, --frozen-lockfile]
    - command: pnpm
      args: [run, build]
  artifacts:
    - from: dist
      to: public
    - from: server
      to: server
    - from: node_modules
      to: node_modules

run:
  command: node
  args: [server/index.js]
  port: 9627
  healthCheck: /health
  readyTimeout: 60
  frontendEntry: /
  env:
    NODE_ENV: production

agentos:
  titleBar: system
  defaultWidth: 1200
  defaultHeight: 800
  minWidth: 720
  minHeight: 480
  permissions: [native.window-control]
  rpcBasePath: /opentool

data:
  persistent: true
```

Build `command` and `args` are executed directly, in order, from the project
root (or the step's `cwd`). They are not interpreted by a shell. Use a command
such as `bash` with `args: [-lc, "your shell command"]` when shell syntax is
needed. Artifact paths are relative to the project and copied to their `to`
paths at the package root. Nested relative symbolic links are retained only when
their targets stay inside the same artifact directory; absolute and escaping
links are rejected. Existing `.agt` archives inside artifact directories are
skipped so rebuilding does not nest previous packages.

When `run.command` is a package path such as `./bin/server`, its packaged path
must be covered by one of the `build.artifacts.to` destinations. Commands such
as `node` or `python3` can be supplied by the AgentOS runtime; their script and
application dependencies still need to be included as artifacts.

The app process must bind to `0.0.0.0` and use the injected `HOST`, `PORT`, and
`DATA_DIR` environment variables where appropriate. `data.persistent: true`
declares that the app keeps user data under AgentOS's persistent data area;
the app must still choose and use its own data files under `DATA_DIR`.

## Remote website example

```yaml
schemaVersion: 1
target: agentos/arm64

app:
  type: remote-web
  id: com.example.github
  name: GitHub
  version: 1.0.0

web:
  url: https://github.com

agentos:
  titleBar: system
```

## Requirements and limitations

- Local process packages target Linux ARM64. Remote URL packages declare
  `architecture: any`. Both use manifest version 1; x64 process output is not
  implemented by this module.
- The package builder runs on a Unix host with `tar` available.
- A process app must package a Linux ARM64-compatible service and all
  app-specific runtime files. Vitul does not convert a multi-service Compose
  deployment into one process.
- Vitul writes an OpenTool binding for process apps, but it does not invent
  business tools. To make AgentOS agent calls work, the app server must
  implement the OpenTool routes at its configured `rpcBasePath`. URL apps have
  no process or OpenTool binding.
- `agentos.permissions` accepts `native.window-control`, `native.file-dialogs`,
  `native.clipboard`, `native.notifications`, `native.external-links`, and
  `native.sidecars` for process apps. Remote CEF pages cannot request these
  native capabilities.
- Dockage and Gitea are candidate apps to package with this format, not
  guaranteed turnkey targets. Their host Docker access, Git/runtime
  dependencies, persistence, and ARM64 builds need to be configured for the
  AgentOS environment.

## Relationship to Vitul projects

The new-app workflow still uses `vitul init`, `vitul.conf.json`, and its Rust
backend. `Vitulfile` is for packaging an existing web app as an AgentOS app.
When a project has `Vitulfile`, `vitul build --target agentos/arm64` uses this
module. The existing `agentos-arm64` compatibility target continues to build
older Vitul projects that do not have a `Vitulfile`.
