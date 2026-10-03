import { cpSync, existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const pluginRoot = fileURLToPath(new URL(".", import.meta.url));
const distDir = fileURLToPath(new URL("dist", import.meta.url));
const publicDir = fileURLToPath(new URL("public", import.meta.url));
const rootIndex = fileURLToPath(new URL("index.html", import.meta.url));
const pluginVersion = (JSON.parse(readFileSync(new URL("./app.json", import.meta.url), "utf8")) as { version: string }).version;

function publishBuild() {
  return {
    name: "publish-toney-music-plugin-build",
    transformIndexHtml(html: string) {
      const versionQuery = `?v=${encodeURIComponent(pluginVersion)}`;
      return html
        .replaceAll("/apps/toney-music/assets/music.js", `/apps/toney-music/assets/music.js${versionQuery}`)
        .replaceAll("/apps/toney-music/assets/music.css", `/apps/toney-music/assets/music.css${versionQuery}`);
    },
    writeBundle() {
      if (!existsSync(distDir)) return;
      rmSync(publicDir, { recursive: true, force: true });
      cpSync(distDir, publicDir, { recursive: true });
      cpSync(distDir, pluginRoot, { recursive: true });
      const publishedIndex = fileURLToPath(new URL("public/index.html", import.meta.url));
      const html = readFileSync(publishedIndex, "utf8").replaceAll("__MUSIC_VERSION__", pluginVersion);
      writeFileSync(publishedIndex, html, "utf8");
      writeFileSync(rootIndex, html, "utf8");
      rmSync(distDir, { recursive: true, force: true });
    },
  };
}

export default defineConfig({
  plugins: [react(), tailwindcss(), publishBuild()],
  root: "src",
  base: "/apps/toney-music/",
  build: {
    outDir: "../dist",
    emptyOutDir: true,
    sourcemap: false,
    rollupOptions: {
      output: {
        entryFileNames: "assets/music.js",
        chunkFileNames: "assets/music.js",
        assetFileNames: (assetInfo) => assetInfo.name?.endsWith(".css") ? "assets/music.css" : "assets/[name][extname]",
      },
    },
  },
});
