export type HostPlatform = 'native' | 'agentos' | 'browser'
export type HostLocale = 'zh-CN' | 'en-US'
export type HostAppearance = 'system' | 'light' | 'dark'
export type WindowState =
  | 'normal'
  | 'maximized'
  | 'minimized'
  | 'dragging'
  | 'resizing'
  | 'fullscreen'

export type WindowTitleBarContext = {
  requestedMode: 'system' | 'overlay'
  actualMode: 'system' | 'overlay' | null
  supportStatus: 'supported' | 'unsupported'
  supportedModes: Array<'system' | 'overlay'>
  windowKind: 'native-top-level' | 'shell-internal' | 'external'
  controlsProvider: 'operating-system' | 'agentos' | 'none'
}

export type HostContext = {
  locale?: HostLocale
  appearance?: HostAppearance
  theme?: 'light' | 'dark'
  windowState?: WindowState
  windowTitleBar?: WindowTitleBarContext
}

export interface HostProvider {
  readonly platform: HostPlatform
  readonly context: HostContext
  window: {
    minimize(): Promise<void>
    maximize(): Promise<void>
    restore(): Promise<void>
    setFullscreen(enabled: boolean): Promise<void>
    close(): Promise<void>
    isMaximized(): Promise<boolean>
    isFullscreen(): Promise<boolean>
  }
  subscribeContext(listener: (context: HostContext) => void): () => void
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

export abstract class ObservableHostProvider implements HostProvider {
  abstract readonly platform: HostPlatform
  context: HostContext
  private readonly listeners = new Set<(context: HostContext) => void>()

  protected constructor(initialContext: HostContext = {}) {
    this.context = initialContext
  }

  subscribeContext(listener: (context: HostContext) => void): () => void {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  protected updateContext(next: Partial<HostContext>): void {
    this.context = { ...this.context, ...next }
    for (const listener of this.listeners) listener(this.context)
  }

  abstract window: HostProvider['window']
}

class AgentOSHostProvider extends ObservableHostProvider {
  readonly platform = 'agentos' as const
  private fullscreen = false

  constructor(private readonly bridge: AgentOSHostBridge) {
    super(readLaunchContext())
    window.addEventListener('message', this.handleHostMessage)
  }

  window = {
    minimize: () => this.bridge.invoke<void>('window.minimize'),
    maximize: () => this.bridge.invoke<void>('window.maximize'),
    restore: () => this.bridge.invoke<void>('window.unmaximize'),
    setFullscreen: async (enabled: boolean) => {
      await this.bridge.invoke<void>('window.setFullscreen', { enabled })
      this.fullscreen = enabled
    },
    close: () => this.bridge.invoke<void>('window.close'),
    isMaximized: async () => this.context.windowState === 'maximized',
    isFullscreen: async () =>
      this.fullscreen || this.context.windowState === 'fullscreen'
  }

  private handleHostMessage = (event: MessageEvent<unknown>) => {
    if (event.source !== window && event.source !== window.parent) return
    if (!event.data || typeof event.data !== 'object') return
    const data = event.data as Record<string, unknown>

    if (
      data.channel === 'agentos:host'
      && data.type === 'context-changed'
      && data.runtime === 'agentos'
    ) {
      this.updateContext({
        locale: normalizeLocale(data.locale),
        appearance: normalizeAppearance(data.appearance),
        theme: normalizeTheme(data.theme),
        windowState: normalizeWindowState(data.windowState),
        windowTitleBar: normalizeTitleBarContext(data.windowTitleBar)
      })
      return
    }

    if (data.type === 'shell:window-context') {
      this.updateContext({
        windowState: normalizeWindowState(data.windowState),
        windowTitleBar: normalizeTitleBarContext(data.windowTitleBar)
      })
      return
    }

    const shellContext = readShellContextMessage(data)
    if (shellContext) this.updateContext(shellContext)
  }
}

class BrowserHostProvider extends ObservableHostProvider {
  readonly platform = 'browser' as const

  constructor() {
    super(readLaunchContext())
    window.addEventListener('message', this.handleHostMessage)
  }

  private handleHostMessage = (event: MessageEvent<unknown>) => {
    if (event.source !== window && event.source !== window.parent) return
    if (!event.data || typeof event.data !== 'object') return
    const context = readShellContextMessage(event.data as Record<string, unknown>)
    if (context) this.updateContext(context)
  }

  private unavailable(): Promise<never> {
    return Promise.reject(
      new Error('Window controls are unavailable in a browser preview.')
    )
  }

  window = {
    minimize: () => this.unavailable(),
    maximize: () => this.unavailable(),
    restore: () => this.unavailable(),
    setFullscreen: () => this.unavailable(),
    close: () => this.unavailable(),
    isMaximized: async () => false,
    isFullscreen: async () => false
  }
}

function readShellContextMessage(
  data: Record<string, unknown>
): Partial<HostContext> | undefined {
  if (data.type === 'shell:locale') {
    const locale = normalizeLocale(data.locale)
    return locale ? { locale } : undefined
  }

  if (data.type === 'shell:theme') {
    const appearance = normalizeAppearance(data.appearance)
    if (!appearance) return undefined
    return {
      appearance,
      theme: normalizeTheme(data.appearance)
    }
  }

  return undefined
}

function readLaunchContext(): HostContext {
  const params = new URLSearchParams(window.location.search)
  let titleBar: WindowTitleBarContext | undefined
  const rawTitleBar = params.get('windowTitleBar')
  if (rawTitleBar) {
    try {
      titleBar = normalizeTitleBarContext(JSON.parse(rawTitleBar))
    } catch {
      titleBar = undefined
    }
  }

  return {
    locale: normalizeLocale(params.get('locale')),
    appearance: normalizeAppearance(params.get('appearance')),
    theme: normalizeTheme(params.get('theme')),
    windowState: normalizeWindowState(params.get('windowState')),
    windowTitleBar: titleBar
  }
}

function normalizeLocale(value: unknown): HostLocale | undefined {
  if (typeof value !== 'string') return undefined
  return value.toLowerCase().startsWith('zh') ? 'zh-CN' : 'en-US'
}

function normalizeAppearance(value: unknown): HostAppearance | undefined {
  return value === 'system' || value === 'light' || value === 'dark'
    ? value
    : undefined
}

function normalizeTheme(value: unknown): 'light' | 'dark' | undefined {
  return value === 'light' || value === 'dark' ? value : undefined
}

function normalizeWindowState(value: unknown): WindowState | undefined {
  return value === 'normal'
    || value === 'maximized'
    || value === 'minimized'
    || value === 'dragging'
    || value === 'resizing'
    || value === 'fullscreen'
    ? value
    : undefined
}

function normalizeTitleBarContext(
  value: unknown
): WindowTitleBarContext | undefined {
  if (!value || typeof value !== 'object') return undefined
  const context = value as Partial<WindowTitleBarContext>
  if (
    (context.requestedMode !== 'system' && context.requestedMode !== 'overlay')
    || (context.actualMode !== 'system'
      && context.actualMode !== 'overlay'
      && context.actualMode !== null)
    || (context.supportStatus !== 'supported'
      && context.supportStatus !== 'unsupported')
    || !Array.isArray(context.supportedModes)
    || !context.supportedModes.every(
      (mode) => mode === 'system' || mode === 'overlay'
    )
    || (context.windowKind !== 'native-top-level'
      && context.windowKind !== 'shell-internal'
      && context.windowKind !== 'external')
    || (context.controlsProvider !== 'operating-system'
      && context.controlsProvider !== 'agentos'
      && context.controlsProvider !== 'none')
  ) {
    return undefined
  }
  return context as WindowTitleBarContext
}
