export interface HostProvider {
  readonly platform: 'native' | 'agentos' | 'browser'
  window: {
    minimize(): Promise<void>
  }
}

export type AgentOSHostBridge = {
  invoke<T>(method: string, payload?: Record<string, unknown>): Promise<T>
}

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown
    agentos?: { host?: AgentOSHostBridge }
  }
}

export async function createHostProvider(): Promise<HostProvider> {
  if (window.agentos?.host) return new AgentOSHostProvider(window.agentos.host)
  if (window.__TAURI_INTERNALS__) {
    const { DesktopHostProvider } = await import('./desktop-host')
    return new DesktopHostProvider()
  }
  return new BrowserHostProvider()
}

class AgentOSHostProvider implements HostProvider {
  readonly platform = 'agentos' as const

  constructor(private readonly bridge: AgentOSHostBridge) {}

  window = {
    minimize: () => this.bridge.invoke<void>('window.minimize')
  }
}

class BrowserHostProvider implements HostProvider {
  readonly platform = 'browser' as const

  window = {
    minimize: () =>
      Promise.reject(new Error('Window controls are unavailable in a browser preview.'))
  }
}
