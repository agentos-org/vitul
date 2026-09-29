# Maintenance and Release Policy

Vitul is a pre-1.0 project. Its designated public home for the project, issue
tracker, pull requests, release notes, and artifacts is
[github.com/agentos-org/vitul](https://github.com/agentos-org/vitul).
The project maintainer is [@LiteVar](https://github.com/LiteVar).

## Support commitment

- Before 1.0, Vitul is a preview. APIs, configuration, generated projects, and
  package formats may change between minor releases. Migration notes will be
  included when a change affects existing projects.
- Only the latest published preview release is in scope for fixes. Fixes are
  best effort; there is no response-time SLA or long-term-support branch.
- After 1.0, Vitul will follow SemVer for the public project release: compatible
  changes in minor releases, fixes in patch releases, and breaking changes in
  major releases. The latest stable minor line will be supported; older major
  lines will not receive routine fixes.
- Security fixes are handled through the latest supported release. There is no
  guaranteed disclosure or patch SLA.
- Runtime and API package versions may reflect the compatible Tauri baseline.
  They are not, by themselves, a statement that Vitul is a stable 2.x product.
  Each Vitul release must publish a version map for the CLI, API, runtime, and
  generated templates.
- The Rust CLI (`vitul-cli`) is currently version `0.1.0`; the optional
  JavaScript wrapper source is at `2.12.0` and is not published on npm. Generated
  projects use the Rust CLI and do not depend on the unpublished wrapper.

### Version map for this source snapshot

| Component                                                                          | Source version | Meaning                                                                     |
| ---------------------------------------------------------------------------------- | -------------- | --------------------------------------------------------------------------- |
| Vitul Rust CLI (`vitul-cli`)                                                       | `0.1.0`        | Vitul's current preview CLI version.                                        |
| Core desktop compatibility runtime (`tauri`, `tauri-runtime`, `tauri-runtime-wry`) | `2.12.0`       | Compatible runtime baseline; supporting crates can have their own versions. |
| JavaScript host API (`@tauri-apps/api`)                                            | `2.12.0`       | Compatible import and package identity retained for existing API users.     |
| Optional JavaScript CLI (`@vitul/cli`)                                             | `2.12.0`       | Source wrapper only; not published to npm.                                  |
| Generated app and backend defaults                                                 | `0.1.0`        | Starter app versions, independent of the Vitul CLI version.                 |
| AgentOS app manifest                                                               | `2`            | Manifest schema version, independent of the Vitul release version.          |

The workspace lockfiles are authoritative for exact dependency versions. Each
Vitul release must update this map when any listed version changes.

## Contribution and review

- Report bugs, request features, and propose changes in GitHub Issues and Pull
  Requests. Include the affected operating system, Vitul version, and a minimal
  reproduction for runtime issues.
- A maintainer reviews each change before it reaches the default branch. Changes
  to runtime behavior, permissions, packaging, or protocol contracts need a
  focused test and an update to the support or compatibility documentation.
- The default branch must reject force pushes and deletion, require pull
  requests, at least one maintainer approval, and passing required checks. The
  `Vitul AgentOS ARM64 package smoke` job must be configured as a required
  check before accepting external contributions.
- Resolve review conversations before merge. Keep generated and inherited
  upstream code attribution and licenses intact.

## Releases

- Project releases use `v0.x.y` tags while in preview. The first stable release
  is `v1.0.0`; subsequent public project releases follow the SemVer policy
  above.
- Every release includes a changelog, supported-platform statement, tested
  toolchain information, and checksums for downloadable artifacts. GitHub
  Releases are the public distribution record.
- The `@vitul/cli` npm binding workflow is configured but requires the package
  to be created and its GitHub Actions trusted publisher to be configured. The
  Rust CLI is installed from a Vitul source checkout until then.
- Do not publish packages using the inherited Rust CLI or Covector workflows;
  they still target upstream identities and are not Vitul release automation.
- Before publishing the first release, align the Rust CLI and JavaScript CLI
  distribution version map, configure the npm package and its GitHub Actions
  trusted publisher, and verify installation from a clean machine.

## Repository settings before opening public contributions

The repository administrator must configure branch protection for the default
branch, enable GitHub private vulnerability reporting, and publish a private
contact method for Code of Conduct reports on the maintainer's GitHub profile
or in the repository About section.
These are GitHub repository settings; files in the source tree cannot enable
them. `CODEOWNERS` assigns `@LiteVar`; grant that account the required repository
permissions before enabling owner-based review rules.
