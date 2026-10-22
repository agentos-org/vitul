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
- Runtime and API package versions may reflect Vitul's Tauri-derived source
  baseline. This records source lineage and does not promise Tauri app, API, or
  plugin compatibility. Each Vitul release must publish a version map for the
  CLI, API, runtime, and generated templates.
- The Rust CLI (`vitul-cli`) is currently version `0.2.0`; the JavaScript CLI
  binding is released independently as `@litevar/vitul-cli` and is at `3.5.0`
  in this source snapshot.

### Version map for this source snapshot

| Component                                                                   | Source version | Meaning                                                                    |
| --------------------------------------------------------------------------- | -------------- | -------------------------------------------------------------------------- |
| Vitul Rust CLI (`vitul-cli`)                                                | `0.2.0`        | Vitul's current preview CLI version.                                       |
| Core desktop runtime source (`tauri`, `tauri-runtime`, `tauri-runtime-wry`) | `2.12.0`       | Tauri-derived source baseline; not a compatibility commitment.             |
| JavaScript host API (`@tauri-apps/api`)                                     | `2.12.0`       | Inherited package identity; not a promise that Tauri apps or plugins work. |
| JavaScript CLI binding (`@litevar/vitul-cli`)                               | `3.5.0`        | Native CLI package; its version tracks the CLI binding release.            |
| Global npm command (`@litevar/vitul`)                                       | `0.5.0`        | Wrapper package for the `vitul` executable.                                |
| npm project creator (`create-vitul-app`)                                    | `0.5.0`        | Wrapper package for `npm create vitul-app`.                                |
| Generated app and backend defaults                                          | `0.1.0`        | Starter app versions, independent of the Vitul CLI version.                |
| AgentOS app manifest                                                        | `1`            | Manifest schema version, independent of the Vitul release version.         |

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

- JavaScript CLI releases use `v<CLI version>` tags, matching
  `packages/cli/package.json` (the next release is `v3.5.0`). The Rust preview
  CLI and npm command wrappers retain their independent versions from the map
  above. A CLI release tag does not change the runtime's preview support policy.
- Every release includes a changelog, supported-platform statement, tested
  toolchain information, and checksums for downloadable artifacts. GitHub
  Releases are the public distribution record.
- The `@litevar/vitul-cli` npm binding is published through the GitHub Actions
  workflow after its versioned GitHub Release has been created. The
  `@litevar/vitul` and `create-vitul-app` wrapper packages have a separate
  manual publish workflow.
- Do not publish packages using the inherited Rust CLI or Covector workflows;
  they still target upstream identities and are not Vitul release automation.
- Before each npm release, align the CLI package version map and verify
  installation from a clean generated project.

## Repository settings before opening public contributions

The repository administrator must configure branch protection for the default
branch, enable GitHub private vulnerability reporting, and publish a private
contact method for Code of Conduct reports on the maintainer's GitHub profile
or in the repository About section.
These are GitHub repository settings; files in the source tree cannot enable
them. `CODEOWNERS` assigns `@LiteVar`; grant that account the required repository
permissions before enabling owner-based review rules.

## Documentation ownership and feature completeness

Keep one owner for each class of fact: root README routes user questions, SUPPORT
owns platform/capability limits, ARCHITECTURE owns responsibility and call paths,
configuration docs own fields/defaults/combinations, project templates own local
workflows, and examples own concrete implementation/run instructions. Link to the
owner rather than duplicating the complete rules in several places.

For any new or changed capability, document observable behavior, applicable
platform/mode/version, configuration, permission/authorization, code and call
sites, resources/executables, package mapping, startup/shutdown, data, acceptance,
and common omissions in the owning document. Explain inapplicable steps. Distinguish
root fragments, runnable projects, and generated manifests.

Update both languages and generated project templates in the same change. Review
from the perspective of a developer with only a generated project: can they find
the entry point, choose support, complete dependencies, and verify user outcomes?
For field or support changes, check generated configuration, validation errors,
and inspect output too. Separate automated evidence from remaining host acceptance.
