# ONLYOFFICE Docs sandbox app for AgentOS

This example builds a self-contained AgentOS `.agt` package from the official ONLYOFFICE Docs Community image. The app runs Docs, its converter, Nginx, and the document workspace together inside AgentOS's app container sandbox. The package requests no native host permissions, starts no Docker containers, and does not require root or package installation on the target device.

The package contains the Linux filesystem and runtime dependencies exported from the official image. During the build, ONLYOFFICE's own asset generator runs in a temporary official-image container to produce font tables, themes, and converter caches; no ONLYOFFICE source code or product binaries are modified. At startup, the app generates writable configuration, cache, logs, and document data only under AgentOS `DATA_DIR`.

## Build

Build for the AgentOS container architecture. On an Apple Silicon Mac targeting AgentOS ARM64:

    ONLYOFFICE_ARCH=arm64 ./examples/agentos/process/only-office/build.sh

`ONLYOFFICE_ARCH=x64` builds Linux/AMD64. `ONLYOFFICE_PLATFORM=linux/arm64` and `ONLYOFFICE_PLATFORM=linux/amd64` are also accepted. Docker uses the local `onlyoffice/documentserver:9.4.0` image when available; otherwise the script loads the matching `.cache/documentserver-9.4.0-<arch>.tar` archive or pulls the official image.

The resulting package is written to `examples/agentos/process/only-office/dist/onlyoffice-docs-arm64.agt` or `onlyoffice-docs-x64.agt`. The build validates the bundle contents and checks that the final `.agt` remains below the 2 GiB upload limit with 16 MiB of headroom. Set `ONLYOFFICE_OUTPUT` to choose a different destination.

## Install and use

Install the `.agt` through Agent Store and open it. The app starts the complete ONLYOFFICE Docs service inside its sandbox, then opens the document workspace. Upload DOCX, XLSX, PPTX, OpenDocument, text, CSV, or PDF files to edit them. Each file is limited to 256 MiB. Documents, configuration, caches, and logs are kept under the app's persistent `DATA_DIR`.

The package targets the Linux architecture selected at build time. It includes the official ONLYOFFICE Docs Community distribution and its notices; read [LICENSES.md](LICENSES.md) before redistributing it.
