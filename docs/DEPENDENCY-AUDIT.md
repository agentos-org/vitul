# Dependency audit notes

The following issues remained in the workspace audit on 2026-11-10. Re-run
`pnpm audit` and `cargo audit` before a release and update or remove these notes
when compatible upstream fixes become available.

- `extract-zip` 2.0.1 is a development dependency of WebdriverIO's browser
  downloader. Two unpatched symlink traversal advisories,
  [GHSA-jmr9-qjv8-65gv](https://github.com/advisories/GHSA-jmr9-qjv8-65gv)
  and [GHSA-7pqw-9j4j-h8q3](https://github.com/advisories/GHSA-7pqw-9j4j-h8q3),
  are already ignored in `pnpm-workspace.yaml`. Disabling automatic driver
  postinstalls does not prevent a manual browser download from using it.
- `deepmerge` 4.3.1 is used by the desktop API test fixture's Svelte Vite plugin.
  The low-severity [GHSA-gcqx-w48m-w2h4](https://github.com/advisories/GHSA-gcqx-w48m-w2h4)
  has no patched release and remains visible in `pnpm audit`.
- `apple-codesign` 0.29 pulls in `zip` 2.4 and `object` 0.36, which in turn
  pulls in `ruzstd` 0.7. Their affected versions remain in `Cargo.lock` because
  the published `apple-codesign` version does not accept the fixed major
  versions. Vitul uses this dependency for certificate generation and disables
  its optional notarization feature. `cargo audit` reports the old ZIP and
  `ruzstd` versions regardless of which code paths are used.
- `crypto-bigint` 0.5 is also retained by the Apple certificate and RPM signing
  dependency chains. Its fix requires 0.7.6 or newer, outside their current
  compatible version ranges.

These notes describe dependency paths, not a determination that every
advisory is exploitable in Vitul. The Rust audit also reports unmaintained
dependencies that require upstream replacements.
