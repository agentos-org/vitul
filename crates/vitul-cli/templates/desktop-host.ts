import { getCurrentWindow } from "@tauri-apps/api/window";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { open, save } from "@tauri-apps/plugin-dialog";
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Command } from "@tauri-apps/plugin-shell";
import type { HostFileFilter, HostProvider, HostSidecar } from "./host";

export class DesktopHostProvider implements HostProvider {
  readonly platform = "native" as const;

  window = {
    minimize: async () => getCurrentWindow().minimize(),
    maximize: async () => getCurrentWindow().maximize(),
    unmaximize: async () => getCurrentWindow().unmaximize(),
    setFullscreen: async (enabled: boolean) => getCurrentWindow().setFullscreen(enabled),
    close: async () => getCurrentWindow().close(),
  };

  dialog = {
    openFile: (filters?: HostFileFilter[]) =>
      open({ multiple: false, directory: false, filters }),
    openDirectory: async () => {
      const result = await open({ multiple: false, directory: true });
      return Array.isArray(result) ? result[0] ?? null : result;
    },
    saveFile: (filters?: HostFileFilter[]) => save({ filters }),
  };

  clipboard = { readText, writeText };

  notification = {
    show: async (title: string, body: string) => {
      let granted = await isPermissionGranted();
      if (!granted) granted = (await requestPermission()) === "granted";
      if (!granted) throw new Error("Notification permission was denied.");
      sendNotification({ title, body });
    },
  };

  opener = { openUrl };

  sidecar = {
    spawn: async (name: string, args: string[] = []): Promise<HostSidecar> => {
      const child = await Command.sidecar(name, args).spawn();
      return { pid: child.pid, kill: () => child.kill() };
    },
  };
}
