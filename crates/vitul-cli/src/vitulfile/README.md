# Vitulfile: package existing web apps for AgentOS

`Vitulfile` is a packaging recipe for turning an existing web application into
an installable AgentOS `.agt` package. It is for projects that already have a
frontend, backend, or hosted website and need an AgentOS manifest, launch
description, and package archive.

This is a standalone packaging mode in the Vitul CLI. It is **not** a Rust
project format and does not require `backend/Cargo.toml`, Tauri, or a Rust
intermediary. A local app runs its own configured process. Projects created
with `vitul create` keep desktop, mobile, and AgentOS settings in the root
`vitul.conf.json`; use Vitulfile for an independent web project.

## Supported profiles

### Existing local web app

Use `type: process` for an app whose own service can listen on one HTTP port
and serve its UI and API. Vitul runs the configured build steps on the build
machine, copies the declared artifacts, writes a small launcher, and creates
the AgentOS package. Node.js, Python, Bash, and common runtimes are available
inside AgentOS; app-specific dependencies still need to be included in the
artifacts.

Vitul supports Linux ARM64 (`agentos/arm64`) and x64 (`agentos/x64`) process
packages. Declare one or both in `targets`; each target's build steps run
sequentially, then Vitul packages the artifacts before moving to the next
target. The build step does not cross-compile an application: build scripts
must use the target environment to produce binaries and dependencies for the
selected architecture. Each step receives `VITUL_TARGET`,
`VITUL_TARGET_OS=linux`, `VITUL_TARGET_ARCH`, and `AGENTOS_TARGET` for that
iteration. For example, `VITUL_TARGET_ARCH` is `arm64` or `x64`.

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

# Build all targets declared in Vitulfile
vitul build --target agentos
# Or build one declared target: vitul build --target agentos/arm64
#                           vitul build --target agentos/x64

# Process profile: dist/<app>-<version>-agentos-arm64.agt and/or
#                  dist/<app>-<version>-agentos-x64.agt
# Remote-web profile: dist/<app>-<version>-agentos-any.agt
```

Remote-web apps always produce one `architecture: any` package because they
contain no platform-specific app process. Their target declaration only
controls which Vitul target selectors are accepted.

## App icon

Set the optional `app.icon` to a project-relative PNG or SVG file. Vitul copies
it to the package root as `icon.png` or `icon.svg`; AgentOS supports both
formats. The extension is case-insensitive.

```yaml
app:
  # Other app fields...
  icon: assets/app-icon.svg
```

`vitul agentos init --path <DIR>` writes the file in another project
directory. The process starter uses `./bin/server` and `bin/server` as example
command and artifact paths. Edit these for the project before validation and
build. `inspect` reports an incomplete starter config so you can see what needs
to be changed.

Use `targets` for a multi-architecture process app. The older single-value
`target: agentos/arm64` form remains supported; a Vitulfile must use one form or
the other.

## Local app example

```yaml
schemaVersion: 1
targets:
  - agentos/arm64
  - agentos/x64

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
  openMode: window
  titleBar: system
  defaultWidth: 1200
  defaultHeight: 800
  minWidth: 720
  minHeight: 480
  permissions: [native.window-control]
  rpcBasePath: /opentool
  rpcDiscovery: runtime

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

- Local process packages target Linux ARM64 or x64 as declared in `targets`.
  Remote URL packages declare `architecture: any`. Both use manifest version 1.
- The package builder runs on a Unix host with `tar` available.
- A process app must package a Linux binary and all app-specific runtime files
  compatible with each declared architecture. Vitul does not convert a
  multi-service Compose deployment into one process.
- Vitul writes an OpenTool binding for process apps, but it does not invent
  business tools. To make AgentOS agent calls work, the app server must
  implement the OpenTool routes at its configured `rpcBasePath`. URL apps have
  no process or OpenTool binding.
- `agentos.rpcDiscovery: runtime` opts a process app into live OpenTool function
  discovery in AgentOS. Its `rpcBasePath` must serve valid OpenTool `load`,
  `call`, and `streamCall` routes. Functions start disabled and must be enabled
  individually in AgentOS Settings before Chat can call them; the default is
  `static`.
- `agentos.openMode` accepts `window` or `workspace` and defaults to `window`.
  `workspace` opens the app maximized in the current AgentOS Workspace.
- `agentos.permissions` accepts `native.window-control`, `native.file-dialogs`,
  `native.clipboard`, `native.notifications`, `native.external-links`,
  `native.sidecars`, `native.external-wayland`, and
  `native.external-wayland.network` for process apps. External Wayland sessions
  let AgentOS host independent Linux graphical processes; network access is
  granted only to sessions that request it. Remote CEF pages cannot request
  these native capabilities.
- Dockage and Gitea are candidate apps to package with this format, not
  guaranteed turnkey targets. Their host Docker access, Git/runtime
  dependencies, persistence, and architecture-specific builds need to be
  configured for the AgentOS environment.

## Relationship to Vitul projects

New projects created with `vitul create` keep desktop, mobile, and AgentOS
settings in one root `vitul.conf.json`. The CLI projects desktop fields into an
ignored runtime config under `src-vitul/`; users edit the root file.

`Vitulfile` remains available for packaging standalone web projects that do not
use the Vitul app scaffold. `vitul build --target agentos` reads the root Vitul
config when present and reads `Vitulfile` for those standalone projects.
