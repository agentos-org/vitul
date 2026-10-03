import { invoke } from "@tauri-apps/api/core";

const videoExtensions = ["mp4", "webm", "mov", "mkv", "avi", "m4v", "ogv"];

type AgentOSHostBridge = {
  invoke<T>(method: string, payload?: Record<string, unknown>): Promise<T>;
};

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
    agentos?: { host?: AgentOSHostBridge };
  }
}

export type VideoSelection = { paths: string[]; files: File[] };

export async function openVideoSelection(): Promise<VideoSelection> {
  if (window.agentos?.host) {
    const selected = await window.agentos.host.invoke<string | string[] | null>(
      "dialog.openFile",
      {
        filters: [{ name: "Video files", extensions: videoExtensions }],
      },
    );
    return { paths: normalizePaths(selected), files: [] };
  }

  if (window.__TAURI_INTERNALS__) {
    const transport = await invoke<"http" | "ipc">("opentool_transport");
    if (transport === "http") {
      const paths = await invoke<string[]>("pick_video_files");
      return { paths: normalizePaths(paths), files: [] };
    }
  }

  return { paths: [], files: await openBrowserFilePicker() };
}

function normalizePaths(value: string | string[] | null): string[] {
  if (typeof value === "string") return value ? [value] : [];
  return Array.isArray(value) ? value.filter((path) => typeof path === "string" && path.length > 0) : [];
}

function openBrowserFilePicker(): Promise<File[]> {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = `video/*,${videoExtensions.map((extension) => `.${extension}`).join(",")}`;
    input.multiple = true;
    input.addEventListener("change", () => resolve(Array.from(input.files ?? [])), { once: true });
    input.addEventListener("cancel", () => resolve([]), { once: true });
    input.click();
  });
}
