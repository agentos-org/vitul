# create-vitul-app

`create-vitul-app` is the frontend template engine used internally by the Vitul
CLI. Application developers should create projects with `vitul init` (or
`pnpm vitul init`), which assembles the complete Vitul project around the
selected frontend template.

The current OpenTool starter uses the `vanilla-ts` frontend. Vitul then adds
the `src-vitul/vitul.conf.json` desktop configuration, the local OpenTool
backend, the host IPC bridge, and platform-specific project documentation.

This crate lives in `crates/create-vitul-app` and is maintained as part of the
Vitul repository. It derives from the upstream `create-tauri-app` project. Its
desktop API dependencies retain compatible interfaces; the original license
and copyright notices remain with the inherited source.
