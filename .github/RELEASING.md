# Vitul Releases

Publish Vitul releases on
[GitHub Releases](https://github.com/agentos-org/vitul/releases). Follow the
version, support, and release requirements in
[`docs/MAINTENANCE.md`](../docs/MAINTENANCE.md).

The Rust CLI is currently installed from a Vitul source checkout with
`cargo install --path crates/vitul-cli --locked`. The optional `@vitul/cli` npm
package is not published yet, and generated projects do not depend on it.

`.github/workflows/publish-cli-js.yml` builds and tests the native bindings for
`@vitul/cli`. After the npm package is created and its GitHub Actions trusted
publisher is configured, a maintainer can publish it by creating a GitHub
release and manually dispatching the workflow with that release ID. Do not
dispatch it before the package version mapping, registry ownership, and clean
installation have been checked.

The inherited Covector and Rust CLI publishing workflows still target upstream
identities. Do not use them to publish Vitul packages. A release is ready only
when its source tag, release notes, artifacts, and checksums have been reviewed
and published under Vitul's own identities.
