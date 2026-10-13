# Vitul Releases

Publish Vitul releases on
[GitHub Releases](https://github.com/agentos-org/vitul/releases). Follow the
version, support, and release requirements in
[`docs/MAINTENANCE.md`](../docs/MAINTENANCE.md).

The Rust CLI can be installed from a Vitul source checkout with
`cargo install --path crates/vitul-cli --locked`. The npm command packages are
`@litevar/vitul` (command `vitul`) and `create-vitul-app`; both depend on the
native `@litevar/vitul-cli` package.
Generated projects use `@litevar/vitul-cli` as a development dependency.

`.github/workflows/publish-cli-js.yml` builds and tests the native bindings for
`@litevar/vitul-cli`. Its `napi prepublish` step publishes the platform packages
(`@litevar/vitul-cli-<target>`) before publishing the root package. Configure a
GitHub Actions trusted publisher for the root and every target package, using
`agentos-org/vitul` and `publish-cli-js.yml`.

The root and platform packages have already been published. For each update,
create the versioned GitHub Release first, then dispatch
`.github/workflows/publish-cli-js.yml` with that release ID. The workflow uses
the repository Actions secret `NPM_TOKEN` to publish the packages and attach
the platform binaries to the release. Keep the registry credential configured
while this workflow depends on it.

After the versioned `@litevar/vitul-cli` release is available in npm, manually dispatch
`.github/workflows/publish-vitul-commands.yml` to publish `@litevar/vitul` and
`create-vitul-app`. Bootstrap each package once so it exists, then configure
trusted publishing for both packages with `agentos-org/vitul` and
`publish-vitul-commands.yml` before dispatching subsequent releases. The workflow
defaults to publishing both packages; use its `package` input to publish just one.

The inherited Covector and Rust CLI publishing workflows still target upstream
identities. Do not use them to publish Vitul packages. A release is ready only
when its source tag, release notes, artifacts, and checksums have been reviewed
and published under Vitul's own identities.
