# AndroidTone

AndroidTone manages Android applications alongside WinTone. Its Agent Store-style UI imports APKs and lists tasks, apps and actions. The package owns its Host Provider; AgentOS provides generic platform facilities.

The Android runtime runs against host Linux kernel facilities. AndroidTone now uses AgentOS's trusted desktop-session Provider and private Wayland session. The generic system-deployment broker is available, but its current bounded actions do not yet cover the Binderfs device setup and LXC lifecycle needed by this runtime. AndroidTone therefore keeps host installation disabled and does not attempt partial privileged setup. Its ordinary Provider retains `no-new-privs` and never elevates; APK imports can still be saved.

Binder configuration, installed modules, loading, mounts and device allocation are reported separately. Missing /dev/binder does not imply an unsupported kernel. Window mapping, Flutter presentation, input, scale, multi-window and GPU presentation remain unverified.

Build: `vitul agentos validate` then `COPYFILE_DISABLE=1 vitul build --target agentos/arm64`.

See [architecture](docs/architecture.md) and [integration review](../docs/tone-platform-integration-review.md).

The runtime build bundles Waydroid userspace, an offline dependency pool, corresponding distro sources and ARM64 Android system/vendor images. This profile supports Ubuntu 24.04 ARM64 only; AgentOS does not need to bundle this runtime. Host deployment remains blocked on generic platform actions for trusted offline package transactions, Binderfs allocation and LXC lifecycle. See [offline runtime](docs/offline-runtime-zh_CN.md). The build verifies installation in a network-disabled disposable container; `androidtone-server --verify-runtime` verifies the packaged payload without host mutation.

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

Open `http://127.0.0.1:5187/apps/androidtone/` for development; the API proxy targets the existing Rust backend on port 3000. `frontend/src` contains the React source and typed API layer; `frontend/public` contains the user-provided logo; `frontend/credits.html` is the localized credits entry. Vite generates `public/`, which is served by Rust and included in the ARM64 package. Both standard Vitul builds and `scripts/package-prebuilt-arm64.py` rebuild the frontend with the frozen lockfile. The deployed app needs no Node.js or Vite server. See [migration and checks](docs/frontend-react.md).

The frontend follows AgentOS host locale (Chinese/English) and light/dark appearance, including live changes. See [localization and theme behavior](docs/localization-theme.md).

Artifact retention: keep only the newest AndroidTone `.agt` and its `.sha256` in `dist/`. The prebuilt packager fully verifies the new package before removing older AndroidTone packages. Increment the release version for subsequent updates.
