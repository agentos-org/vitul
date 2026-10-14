export type Locale = 'zh-CN' | 'en-US'
export type Theme = 'light' | 'dark'
export type HostContext = { locale: Locale; theme: Theme }

const params = new URLSearchParams(location.search)
const hostOrigin = params.get('hostOrigin')
let hosted = params.get('runtime') === 'agentos' || hostOrigin !== null
const systemTheme = matchMedia('(prefers-color-scheme: dark)')
const listeners = new Set<() => void>()

function locale(value: unknown): Locale | undefined {
  if (value === 'zh' || value === 'zh-CN') return 'zh-CN'
  if (value === 'en' || value === 'en-US') return 'en-US'
}
function theme(value: unknown): Theme | undefined {
  if (value === 'light' || value === 'dark') return value
}
function browserLocale(): Locale {
  return navigator.language.toLowerCase().startsWith('zh') ? 'zh-CN' : 'en-US'
}

let context: HostContext = {
  locale: locale(hosted ? params.get('locale') : null) ?? browserLocale(),
  theme:
    theme(hosted ? params.get('theme') : null)
    ?? theme(hosted ? params.get('appearance') : null)
    ?? (systemTheme.matches ? 'dark' : 'light')
}

function apply(next: HostContext) {
  const root = document.documentElement
  root.lang = next.locale
  root.dataset.theme = next.theme
  root.style.colorScheme = next.theme
  document
    .querySelector('meta[name="theme-color"]')
    ?.setAttribute('content', next.theme === 'dark' ? '#0b0d12' : '#f5f7fa')
  if (next.locale === context.locale && next.theme === context.theme) return
  context = next
  listeners.forEach((listener) => listener())
}
apply(context)

function trustedParent(event: MessageEvent<unknown>): boolean {
  if (window.parent === window || event.source !== window.parent) return false
  try {
    return (
      event.origin
      === (hostOrigin ? new URL(hostOrigin).origin : location.origin)
    )
  } catch {
    return false
  }
}

function onMessage(event: MessageEvent<unknown>) {
  if (!event.data || typeof event.data !== 'object') return
  const value = event.data as Record<string, unknown>
  const parent = trustedParent(event)
  if (value.type === 'shell:theme' && parent && theme(value.appearance)) {
    hosted = true
    apply({ ...context, theme: theme(value.appearance)! })
  } else if (value.type === 'shell:locale' && parent && locale(value.locale)) {
    hosted = true
    apply({ ...context, locale: locale(value.locale)! })
  } else if (
    (event.source === window || parent)
    && value.channel === 'agentos:host'
    && value.type === 'context-changed'
    && value.runtime === 'agentos'
    && (value.schemaVersion === undefined || value.schemaVersion === 1)
  ) {
    hosted = true
    apply({
      locale: locale(value.locale) ?? context.locale,
      theme: theme(value.theme) ?? context.theme
    })
  }
}
function onThemeChange(event: MediaQueryListEvent) {
  if (!hosted) apply({ ...context, theme: event.matches ? 'dark' : 'light' })
}
function onLanguageChange() {
  if (!hosted) apply({ ...context, locale: browserLocale() })
}

// React's external store subscription survives StrictMode and cleans up on unmount.
export function subscribeHostContext(listener: () => void): () => void {
  if (listeners.size === 0) {
    window.addEventListener('message', onMessage)
    window.addEventListener('languagechange', onLanguageChange)
    systemTheme.addEventListener('change', onThemeChange)
    if (!hosted)
      apply({
        locale: browserLocale(),
        theme: systemTheme.matches ? 'dark' : 'light'
      })
  }
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
    if (listeners.size === 0) {
      window.removeEventListener('message', onMessage)
      window.removeEventListener('languagechange', onLanguageChange)
      systemTheme.removeEventListener('change', onThemeChange)
    }
  }
}
export function getHostContext(): HostContext {
  return context
}
