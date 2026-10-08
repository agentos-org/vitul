# Vitul desktop API test fixture

This Vitul desktop app is the integration fixture used by the desktop API
compatibility suite. It uses src-vitul/ and vitul.conf.json; its greet business
operation is served by the Rust OpenTool backend, while the suite exercises the
supported system API surface.

For a native application example, see [Vitul Demo](../../../examples/starter/vitul-demo/README.md).

## Run

From the repository root, install dependencies and build the local API package
and Vitul CLI:

    pnpm install
    pnpm build:api
    pnpm build:cli:debug

Then, from this directory:

    pnpm vitul dev
    pnpm vitul build

vitul dev starts the OpenTool backend on a dynamic loopback port before
starting the desktop app. The Welcome view includes a business call to
/opentool/call and a window-minimize action through system IPC.
