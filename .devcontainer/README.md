# Vitul VS Code Dev Container

This development container is based on Microsoft's [Ubuntu devcontainer
image](https://github.com/microsoft/vscode-dev-containers/tree/master/containers/ubuntu).
Its installed packages are listed in the [Dockerfile](./Dockerfile).

## Usage

1. Install Docker and VS Code.
2. Open the Vitul repository in VS Code.
3. Install the [Remote Development extension pack](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.vscode-remote-extensionpack).
4. Choose **Dev Containers: Reopen in Container** from the Command Palette.

The container includes the Linux dependencies needed by Vitul's compatible
desktop runtime. See the [runtime prerequisite
guide](https://v2.tauri.app/start/prerequisites/) for the upstream dependency
details.

## Filesystem performance

Docker file sharing can slow down operations such as `cargo build` and
`pnpm install`. For large workspaces, VS Code's **Dev Containers: Clone
Repository in Container Volume** command can keep the Vitul source in a
container-managed volume.

## Accessing a desktop window

To display a Vitul desktop app outside the container, run an X window server on
the host and set `DISPLAY` in the container terminal:

```bash
export DISPLAY="host.docker.internal:0"
```
