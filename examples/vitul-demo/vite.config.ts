import { defineConfig } from "vite";
// @ts-expect-error type error without @types/node package
import process from "node:process";

const host = process.env.VITUL_DEV_HOST;
const backendPort = process.env.VITUL_OPENTOOL_PORT || "9627";

export default defineConfig({
  base: "./",
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    proxy: {
      "/opentool": {
        target: `http://127.0.0.1:${backendPort}`,
        changeOrigin: false,
      },
    },
    watch: { ignored: ["**/src-vitul/**", "**/backend/**"] },
  },
});
