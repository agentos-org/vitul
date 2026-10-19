# AndroidTone

AndroidTone manages Android applications alongside WinTone. Its Agent Store-style UI imports APKs and lists tasks, apps and actions. The package owns its Host Provider; AgentOS provides generic platform facilities.

AndroidTone 0.1.18 runs as a desktop-user Native process, outside the Docker App sandbox. Its fixed, digest-pinned deployment operation uses AgentOS's generic system-deployment Provider to prepare Binder and install the bundled Waydroid ARM64-only runtime and Android 13 images. Waydroid and the image installer remain exact application components; 259 Ubuntu 24.04 host dependencies are checked against the Debian `Depends` and `Pre-Depends` declared by the exact Waydroid package, with bundled archives available as offline candidates. A preflight block stays out of the Shell authorization queue and is not retried during refresh; the user can request a fresh plan after correcting the manifest or host state. The image installer remains an exact component because Waydroid 1.6.2 only recognizes its two system-wide preinstalled-image directories. The ordinary Provider retains `no-new-privs` and never elevates; APK imports can be saved while the environment is unavailable.

When AgentOS records a deployment task as denied, AndroidTone offers **Request authorization again in AgentOS**. This is available only after a user clicks it; refresh, background checks and OpenTool do not submit reauthorization requests. AndroidTone sends only protocol version, the denied task ID and a persisted idempotency key. AgentOS Shell confirms the request and collects any administrator password. AndroidTone follows the reauthorization request and, after Shell confirmation and platform revalidation, tracks the linked task while retaining the original denied task for audit. An uncertain network result can be retried with the same key; another flow requires another explicit click. Capability or host-policy failures remain blocking conditions.

Binder configuration, installed modules, loading, Binderfs and Waydroid host-service state are reported separately. Native execution removes the App-container Binder device-assignment step; AgentOS still controls privileged host changes through its system authorization flow. Window mapping, Flutter presentation, input, scale, multi-window and GPU presentation remain unverified.

Build: `vitul agentos validate` then `COPYFILE_DISABLE=1 python3 scripts/package-prebuilt-arm64.py`. The packager builds the ARM64 backend and offline runtime with Docker, synchronizes the digest-pinned AgentOS deployment action from the verified runtime catalog, then calls Vitul's standard AGT packager. The installed backend runs natively on the host.

See [architecture](docs/architecture.md) and [integration review](../docs/tone-platform-integration-review.md).

The runtime build bundles Waydroid userspace, an offline dependency pool, corresponding distro sources and ARM64 Android system/vendor images. This profile supports Ubuntu 24.04 ARM64 only; AgentOS does not bundle the Waydroid business runtime. The package build verifies the offline bundle and its image initialization script in a disposable network-disabled container. Host-side app-session and window rendering still require separate VM validation. See [automatic setup](docs/automatic-setup.md) and [offline runtime](docs/offline-runtime-zh_CN.md); `androidtone-server --verify-runtime` verifies the packaged payload without host mutation.

See [final artifact and verification results](docs/offline-runtime-validation.md).

## Frontend development

The store uses React 19, TypeScript, Tailwind CSS 4 and Vite 8 with pnpm. Node.js 22.12+ is required to build it. The frontend is an isolated pnpm project; its lockfile pins dependencies independently of the repository workspace.

```sh
pnpm --dir frontend install --frozen-lockfile
pnpm --dir frontend dev
pnpm --dir frontend build
pnpm --dir frontend exec playwright install chromium
pnpm --dir frontend test
```

Open `http://127.0.0.1:5187/apps/androidtone/` for development; the API proxy targets the existing Rust backend on port 3000. `frontend/src` contains the React source and typed API layer; `frontend/public` contains the user-provided logo; `frontend/credits.html` is the localized credits entry. Vite generates `public/`, which is served by Rust and included in the ARM64 package. The AGT packager rebuilds the frontend with the frozen lockfile. The deployed app needs no Node.js or Vite server. See [migration and checks](docs/frontend-react.md).

The frontend follows AgentOS host locale (Chinese/English) and light/dark appearance, including live changes. See [localization and theme behavior](docs/localization-theme.md).

Artifact retention: keep only the newest AndroidTone `.agt` and its `.sha256` in `dist/`. The prebuilt packager fully verifies the new package before removing older AndroidTone packages. Increment the release version for subsequent updates.

## System tray

AndroidTone declares `native.tray` and registers its robot shopping-bag icon through `AGENTOS_TRAY_SOCKET`. Registration runs in the backend, independently of the frontend and Android runtime readiness. AgentOS supplies localized Open / Exit actions and removes the icon when the application stops. The embedded 128 × 128 PNG is below the platform's 96 KiB limit. Missing tray access does not prevent startup; broker failures retry automatically. Upgrading requires reinstalling the `.agt` and restarting AndroidTone so AgentOS can apply the permission and per-App socket.

An APK task is marked installed only after its manifest package ID is parsed, the Waydroid command reports no error, and that exact package appears in the launchable application list within a bounded wait. Unconfirmed installs retain a manual retry action and expandable Waydroid diagnostics. APKs without a launcher activity are not claimed to be launchable from this list.
