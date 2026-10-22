# Vitul JavaScript runtime API (`@tauri-apps/api`)

This directory contains the TypeScript API inherited from Tauri. Its package name
and module names are retained for the runtime. Their presence does not promise
that every API or Tauri plugin works on every Vitul target.

## Choose the API from the requirement

For a generated Vitul app, start with its `VITUL.md` and `AGENTS.md`:

| Requested behavior | App entry point |
| --- | --- |
| Application business operations | `src/opentool.ts` and `backend/src/tools/` |
| Windows, dialogs, clipboard, notifications, links, sidecars | `src/host.ts` and its target provider |
| AgentOS Gateway APIs | `src/lib/agentos.ts` and `agentos-sdk-ts` |

The native provider can use modules from this package internally. Importing a
runtime module directly does not create an AgentOS host implementation. Before
using an API, confirm target support, permission/authorization, runtime code, and
required packaged resources. Follow the project's generated dependency versions;
do not infer a compatible dependency from this inherited package name alone.

See [Support](../../docs/SUPPORT.md), [Architecture](../../ARCHITECTURE.md), and
[Examples](../../examples/README.md). Browser preview covers only its implemented
fallbacks; verify the requested behavior on the target host. Vitul's version and
support policies are in [Maintenance](../../docs/MAINTENANCE.md).

## Licenses

Code: (c) 2019 - 2021 - The Tauri Programme within The Commons Conservancy.

MIT or MIT/Apache 2.0 where applicable.

Logo: CC-BY-NC-ND

- Original Tauri Logo Designs by [Daniel Thompson-Yvetot](https://github.com/nothingismagick) and [Guillaume Chau](https://github.com/akryum)
