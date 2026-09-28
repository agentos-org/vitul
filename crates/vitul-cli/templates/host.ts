export type HostFileFilter = { name: string; extensions: string[] };

export type HostSidecar = {
  pid: number;
  kill(): Promise<void>;
};

export interface HostProvider {
  readonly platform: "native" | "agentos" | "browser";
  window: {
    minimize(): Promise<void>;
    maximize(): Promise<void>;
    unmaximize(): Promise<void>;
    setFullscreen(enabled: boolean): Promise<void>;
    close(): Promise<void>;
  };
  dialog: {
    openFile(filters?: HostFileFilter[]): Promise<string | string[] | null>;
    openDirectory(): Promise<string | null>;
    saveFile(filters?: HostFileFilter[]): Promise<string | null>;
  };
  clipboard: {
    readText(): Promise<string | null>;
    writeText(text: string): Promise<void>;
  };
  notification: {
    show(title: string, body: string): Promise<void>;
  };
  opener: {
    openUrl(url: string): Promise<void>;
  };
  sidecar: {
    spawn(name: string, args?: string[]): Promise<HostSidecar>;
  };
}

export type AgentOSHostBridge = {
  invoke<T>(method: string, payload?: Record<string, unknown>): Promise<T>;
};

function agentOSSidecarName(name: string): string {
  if (
    name.length === 0 ||
    name.length > 512 ||
    name.startsWith("/") ||
    name.startsWith("\\") ||
    /^[a-zA-Z]:/.test(name)
  ) {
    throw new Error("AgentOS sidecar must be a relative bundle.externalBin path.");
  }
  const parts = name.split(/[\\/]/);
  if (parts.some((part) => part === "" || part === "." || part === "..")) {
    throw new Error("AgentOS sidecar path cannot contain empty or traversal segments.");
  }
  const logicalName = parts[parts.length - 1] ?? "";
  if (!/^[a-z][a-z0-9-]{0,31}$/.test(logicalName)) {
    throw new Error(
      "AgentOS sidecar names must use a lowercase basename matching [a-z][a-z0-9-]{0,31}.",
    );
  }
  return logicalName;
}

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
    agentos?: { host?: AgentOSHostBridge };
  }
}

export async function createHostProvider(): Promise<HostProvider> {
  if (window.agentos?.host) return new AgentOSHostProvider(window.agentos.host);
  if (window.__TAURI_INTERNALS__) {
    const { DesktopHostProvider } = await import("./desktop-host");
    return new DesktopHostProvider();
  }
  return new BrowserHostProvider();
}

class AgentOSHostProvider implements HostProvider {
  readonly platform = "agentos" as const;

  constructor(private readonly bridge: AgentOSHostBridge) {}

  window = {
    minimize: () => this.bridge.invoke<void>("window.minimize"),
    maximize: () => this.bridge.invoke<void>("window.maximize"),
    unmaximize: () => this.bridge.invoke<void>("window.unmaximize"),
    setFullscreen: (enabled: boolean) =>
      this.bridge.invoke<void>("window.setFullscreen", { enabled }),
    close: () => this.bridge.invoke<void>("window.close"),
  };

  dialog = {
    openFile: (filters?: HostFileFilter[]) =>
      this.bridge.invoke<string | string[] | null>("dialog.openFile", { filters }),
    openDirectory: () =>
      this.bridge.invoke<string | null>("dialog.openDirectory"),
    saveFile: (filters?: HostFileFilter[]) =>
      this.bridge.invoke<string | null>("dialog.saveFile", { filters }),
  };

  clipboard = {
    readText: () => this.bridge.invoke<string | null>("clipboard.readText"),
    writeText: (text: string) =>
      this.bridge.invoke<void>("clipboard.writeText", { text }),
  };

  notification = {
    show: (title: string, body: string) =>
      this.bridge.invoke<void>("notification.show", { title, body }),
  };

  opener = {
    openUrl: (url: string) => this.bridge.invoke<void>("opener.openUrl", { url }),
  };

  sidecar = {
    spawn: async (name: string, args: string[] = []): Promise<HostSidecar> => {
      const started = await this.bridge.invoke<{
        processId?: unknown;
        pid?: unknown;
      }>("sidecar.spawn", { name: agentOSSidecarName(name), args });
      if (
        !started ||
        typeof started.processId !== "string" ||
        !/^[a-f0-9]{32}$/.test(started.processId) ||
        typeof started.pid !== "number" ||
        !Number.isSafeInteger(started.pid) ||
        started.pid <= 0
      ) {
        throw new Error("AgentOS returned an invalid sidecar process handle.");
      }

      const processId = started.processId;
      let stopped = false;
      let stopping: Promise<void> | undefined;
      return {
        pid: started.pid,
        kill: async () => {
          if (stopped) return;
          if (!stopping) {
            const request = this.bridge
              .invoke<{ processId?: unknown; stopped?: unknown }>("sidecar.kill", {
                processId,
              })
              .then((result) => {
                if (result?.processId !== processId || result.stopped !== true) {
                  throw new Error("AgentOS could not confirm the sidecar stop request.");
                }
                stopped = true;
              });
            stopping = request;
          }
          const request = stopping;
          try {
            await request;
          } finally {
            if (stopping === request) stopping = undefined;
          }
        },
      };
    },
  };
}

class BrowserHostProvider implements HostProvider {
  readonly platform = "browser" as const;

  private unsupported<T>(): Promise<T> {
    return Promise.reject(new Error("This host API is unavailable in a browser preview."));
  }

  window = {
    minimize: () => this.unsupported<void>(),
    maximize: () => this.unsupported<void>(),
    unmaximize: () => this.unsupported<void>(),
    setFullscreen: async (enabled: boolean) => {
      if (enabled) await document.documentElement.requestFullscreen();
      else if (document.fullscreenElement) await document.exitFullscreen();
    },
    close: () => this.unsupported<void>(),
  };

  dialog = {
    openFile: () => this.unsupported<string | string[] | null>(),
    openDirectory: () => this.unsupported<string | null>(),
    saveFile: () => this.unsupported<string | null>(),
  };

  clipboard = {
    readText: () => navigator.clipboard.readText(),
    writeText: (text: string) => navigator.clipboard.writeText(text),
  };

  notification = {
    show: async (title: string, body: string) => {
      if (!("Notification" in window)) throw new Error("Notifications are unavailable.");
      if (Notification.permission === "granted") new Notification(title, { body });
      else throw new Error("Notification permission has not been granted.");
    },
  };

  opener = {
    openUrl: async (url: string) => {
      const parsed = new URL(url);
      if (parsed.protocol !== "https:" && parsed.protocol !== "http:") {
        throw new Error("Only HTTP and HTTPS URLs can be opened.");
      }
      window.open(parsed.toString(), "_blank", "noopener,noreferrer");
    },
  };

  sidecar = {
    spawn: () => this.unsupported<HostSidecar>(),
  };
}
