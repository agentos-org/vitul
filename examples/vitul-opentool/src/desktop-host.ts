import { getCurrentWindow } from '@tauri-apps/api/window'
import type { HostProvider } from './host'

export class DesktopHostProvider implements HostProvider {
  readonly platform = 'native' as const

  window = {
    minimize: () => getCurrentWindow().minimize()
  }
}
