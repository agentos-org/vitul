# Vitul Releases

Publish Vitul releases on
[GitHub Releases](https://github.com/agentos-org/vitul/releases). Follow the
version, support, and release requirements in
[`docs/MAINTENANCE.md`](../docs/MAINTENANCE.md).

The Rust CLI can be installed from a Vitul source checkout with
`cargo install --path crates/vitul-cli --locked`. The npm command packages are
`vitul` and `create-vitul-app`; both depend on the native `@litevar/vitul-cli` package.
Generated projects use `@litevar/vitul-cli` as a development dependency.

`.github/workflows/publish-cli-js.yml` builds and tests the native bindings for
`@litevar/vitul-cli`. Its `napi prepublish` step publishes the platform packages
(`@litevar/vitul-cli-<target>`) before publishing the root package. Configure a
GitHub Actions trusted publisher for the root and every target package, using
`agentos-org/vitul` and `publish-cli-js.yml`.

npm requires a package to exist before a trusted publisher can be configured,
so the first release needs an authenticated bootstrap publish for the root and
platform packages. Both publish workflows accept the repository Actions secret
`NPM_TOKEN` for this initial release. After publishing, configure trusted
publishers for every package; npm will use OIDC when the workflow is trusted.
Configure them shortly before the next release: npm expires a new trusted
publisher configuration unless its first publish succeeds within two days.
Remove the token secret after confirming an OIDC publish. Do not dispatch the
workflow before registry ownership, version mapping, and clean installation
have been checked.

After `@litevar/vitul-cli@2.12.0` is available in npm, manually dispatch
`.github/workflows/publish-vitul-commands.yml` to publish `vitul` and
`create-vitul-app`. Bootstrap each package once so it exists, then configure
trusted publishing for both packages with `agentos-org/vitul` and
`publish-vitul-commands.yml` before dispatching subsequent releases.

The inherited Covector and Rust CLI publishing workflows still target upstream
identities. Do not use them to publish Vitul packages. A release is ready only
when its source tag, release notes, artifacts, and checksums have been reviewed
and published under Vitul's own identities.
