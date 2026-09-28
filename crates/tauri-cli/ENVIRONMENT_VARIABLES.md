# Vitul Desktop Runtime Environment Variables

[简体中文](ENVIRONMENT_VARIABLES-zh_CN.md)

This page documents environment variables used by Vitul's compatible desktop runtime and CLI. Existing `TAURI_*` names are retained as compatibility interfaces; `VITUL_*` names are used for Vitul-specific settings.

## CLI Input Variables

These variables provide input to the CLI; some have an equivalent CLI flag. If both an environment variable and CLI flag are set, the CLI flag takes priority.

- `CI` — Run the CLI in CI mode without user interaction.
- `TAURI_CLI_CONFIG_DEPTH` — Number of directory levels to search for a Vitul config file.
- `TAURI_CLI_PORT` — Port used by the built-in development server.
- `TAURI_CLI_WATCHER_IGNORE_FILENAME` — Name of a `.gitignore`-style file that controls which files the `dev` command watches. The CLI looks for this file in each directory.
- `TAURI_CLI_NO_DEV_SERVER_WAIT` — Skip waiting for the frontend development server before building the app.
- `TAURI_APP_PATH` — Path to the Vitul application crate, usually `<project>/src-vitul`. When set, the CLI uses it instead of searching for a config file from the current directory.
- `TAURI_FRONTEND_PATH` — Path to the frontend directory, usually `<project>`. When set, the CLI uses it instead of inferring the frontend directory from the Vitul app directory.
- `VITUL_CONFIG` — JSON values to merge into `vitul.conf.json`. Vitul sets it from `--config` so build and code generation use the same merged config. You can also set it when compiling with `cargo` directly. The compatibility alias `TAURI_CONFIG` is also recognized.
- `TAURI_LINUX_AYATANA_APPINDICATOR` — Set to `true` or `1` to force `libayatana-appindicator` for the Linux system tray.
- `TAURI_BUNDLER_WIX_FIPS_COMPLIANT` — Set the bundler's WiX `FipsCompliant` option.
- `TAURI_BUNDLER_TOOLS_GITHUB_MIRROR` — GitHub mirror used to download bundler tools.
- `TAURI_BUNDLER_TOOLS_GITHUB_MIRROR_TEMPLATE` — GitHub mirror URL template, such as `https://mirror.example.com/<owner>/<repo>/releases/download/<version>/<asset>`.
- `TAURI_BUNDLER_DMG_IGNORE_CI` — Disable the `.dmg` bundler's `CI: true` check.
- `TAURI_SKIP_SIDECAR_SIGNATURE_CHECK` — Skip sidecar signature checks.
- `TAURI_SIGNING_PRIVATE_KEY` — Private key used to sign app bundles. `build` and `bundle` accept a key string or file path; `signer sign` requires the key string itself.
- `TAURI_SIGNING_PRIVATE_KEY_PATH` — Private-key file path for `signer sign`. It cannot be set together with `TAURI_SIGNING_PRIVATE_KEY` for that command.
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — Password for `TAURI_SIGNING_PRIVATE_KEY`.
- `TAURI_SIGNING_RPM_KEY` — ASCII-armored GPG private key used to sign RPM packages.
- `TAURI_SIGNING_RPM_KEY_PASSPHRASE` — Passphrase for `TAURI_SIGNING_RPM_KEY`, if required.
- `TAURI_WINDOWS_SIGNTOOL_PATH` — Path to `signtool.exe` used for Windows code signing.
- `STATIC_VCRUNTIME` — Deprecated. Set `build > windows > staticVCRuntime` in `vitul.conf.json` instead.
- `VCTOOLS_REDIST_DIR` — Override the Visual C++ redistributable directory used when `bundle > windows > bundleVCRuntime` is enabled.
- `APPLE_CERTIFICATE` — Base64-encoded `.p12` certificate for code signing.
- `APPLE_CERTIFICATE_PASSWORD` — Password used when exporting the certificate.
- `APPLE_ID` — Apple ID used for notarization. Also set `APPLE_PASSWORD` and `APPLE_TEAM_ID`, or use `APPLE_API_KEY` and `APPLE_API_ISSUER`.
- `APPLE_PASSWORD` — Apple password used for notarization. An app-specific password or `@keychain:` / `@env:` reference can be used.
- `APPLE_TEAM_ID` — Developer team ID, available on the [Apple Developer account page](https://developer.apple.com/account).
- `APPLE_API_KEY` — JWT-based alternative to `APPLE_ID` and `APPLE_PASSWORD`; it can also enable automated iOS certificate and provisioning-profile management.
- `APPLE_API_ISSUER` — Issuer ID required when `APPLE_API_KEY` is set.
- `APPLE_API_KEY_PATH` — Path to the API key `.p8` file. For macOS, the bundler searches standard private-key directories if unset. iOS builds require this variable.
- `APPLE_SIGNING_IDENTITY` — Code-signing identity. Overrides `vitul.conf.json > bundle > macOS > signingIdentity`.
- `APPLE_PROVIDER_SHORT_NAME` — Notarization team provider short name. Overrides `vitul.conf.json > bundle > macOS > providerShortName`.
- `APPLE_DEVELOPMENT_TEAM` — iOS code-signing team ID. Overrides `vitul.conf.json > bundle > iOS > developmentTeam`.
- `TAURI_WEBVIEW_AUTOMATION` — Enable WebView automation (Linux only).
- `TAURI_DEV_ROOT_CERTIFICATE_PATH` — PEM root certificate trusted by the app during `vitul android dev` or `vitul ios dev`, useful with an HTTPS development server using a self-signed certificate.
- `TAURI_ANDROID_PROJECT_PATH` — Android project path, usually `<project>/src-vitul/gen/android`.
- `TAURI_IOS_PROJECT_PATH` — iOS project path, usually `<project>/src-vitul/gen/apple`.

## Vitul CLI Hook Variables

These variables are set for hook commands such as `beforeDevCommand` and `beforeBuildCommand`.

- `TAURI_ENV_DEBUG` — `true` for `dev` or `build --debug`; unset otherwise.
- `TAURI_ENV_TARGET_TRIPLE` — Target triple being built.
- `TAURI_ENV_ARCH` — Target architecture.
- `TAURI_ENV_PLATFORM` — Target platform (`windows`, `darwin`, `linux`, etc.).
- `TAURI_ENV_PLATFORM_VERSION` — Build platform version.
- `TAURI_ENV_FAMILY` — Target platform family (`unix` or `windows`).
- `VITUL_DEV_HOST` — IP address the frontend development server must listen on so the app can reach it. Set by `vitul android dev` and `vitul ios dev` when `--host` is used, the app runs on a physical device, or `build > devUrl` points to `0.0.0.0`.
