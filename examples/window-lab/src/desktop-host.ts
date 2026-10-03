import { getCurrentWindow } from '@tauri-apps/api/window'
import { ObservableHostProvider, type HostContext } from './host'

export class DesktopHostProvider extends ObservableHostProvider {
  readonly platform = 'native' as const
  private readonly nativeWindow = getCurrentWindow()

  constructor() {
    super(createDesktopContext())
  }

  window = {
    minimize: () => this.nativeWindow.minimize(),
    maximize: () => this.nativeWindow.maximize(),
    restore: () => this.nativeWindow.unmaximize(),
    setFullscreen: (enabled: boolean) =>
      this.nativeWindow.setFullscreen(enabled),
    close: () => this.nativeWindow.close(),
    isMaximized: () => this.nativeWindow.isMaximized(),
    isFullscreen: () => this.nativeWindow.isFullscreen()
  }
}

function createDesktopContext(): HostContext {
  const isMobile = /iPhone|iPad|Android/i.test(navigator.userAgent)
  const isMacOS = !isMobile && /Mac/.test(navigator.platform)
  const dark = window.matchMedia('(prefers-color-scheme: dark)').matches

  return {
    locale: navigator.language.toLowerCase().startsWith('zh')
      ? 'zh-CN'
      : 'en-US',
    appearance: 'system',
    theme: dark ? 'dark' : 'light',
    windowTitleBar: {
      requestedMode: isMacOS ? 'overlay' : 'system',
      actualMode: isMacOS ? 'overlay' : 'system',
      supportStatus: 'supported',
      supportedModes: isMacOS ? ['system', 'overlay'] : ['system'],
      windowKind: 'native-top-level',
      controlsProvider: isMobile ? 'none' : 'operating-system'
    }
  }
}
