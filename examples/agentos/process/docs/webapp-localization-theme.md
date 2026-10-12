# Localization and theme for AgentOS WebApps

This guide applies to WebApps embedded in the AgentOS WebView or opened in an AgentOS WebShell iframe. The host supplies locale and color theme. Read that context before the first paint and respond to changes while the page remains open. Standalone browsers can fall back to browser language and system appearance. The contract does not depend on React, Vitul, or `agentos-sdk-ts`.

## Host context

The launch URL may include these parameters:

| Parameter | Values | Purpose |
| --- | --- | --- |
| `runtime=agentos` | `agentos` | Native AgentOS WebApp marker |
| `hostOrigin` | Host origin | WebShell plugin marker and expected origin for parent messages |
| `locale` | `zh`, `en`, `zh-CN`, or `en-US` | Resolved host locale |
| `appearance` | `system`, `light`, or `dark` | User's appearance preference |
| `theme` | `light` or `dark` | Resolved host color scheme |

WebShell plugin URLs include `hostOrigin`, `locale`, `appearance`, and `theme`, but may omit `runtime=agentos`. Treat the page as hosted when either `runtime=agentos` or `hostOrigin` is present. `appearance=system` describes the user's preference; use the resolved `theme` for actual colors. Do not let a WebView `prefers-color-scheme: dark` result override a host-provided light theme.

The host sends live messages while the app is open:

| Message | Source and fields | Purpose |
| --- | --- | --- |
| `agentos:host` / `context-changed` | Native AgentOS context with `runtime: "agentos"`, `locale`, `appearance`, and `theme`; current `schemaVersion` is 1, older hosts may omit it | Sync host locale and resolved theme |
| `shell:theme` | WebShell parent iframe; `appearance` is resolved to `light` or `dark` | Sync theme changes |
| `shell:locale` | WebShell parent iframe; `locale` is `zh` or `en` | Sync locale changes |

Native AgentOS context messages may be delivered to the WebView document itself. WebShell `shell:*` messages come from the parent window. Do not require only `event.source === window`, and do not accept messages from arbitrary frames. For parent messages, require `event.source === window.parent` and validate `event.origin`: compare it with the origin in `hostOrigin`, or require the current page's origin when that parameter is absent.

## Initialize before first paint

Place a small synchronous script in the `<head>` of `index.html`. Set the language and theme before React, Vue, or application CSS initializes. CSS can then render the first frame using semantic variables.

```html
<script>
  (() => {
    const params = new URLSearchParams(location.search)
    const root = document.documentElement
    const hosted = params.get('runtime') === 'agentos' || params.has('hostOrigin')
    const locale = hosted ? params.get('locale') : null
    const requestedTheme = hosted ? params.get('theme') : null
    const theme = requestedTheme === 'light' || requestedTheme === 'dark'
      ? requestedTheme
      : matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'

    root.lang = locale === 'zh' || locale === 'zh-CN'
      ? 'zh-CN'
      : locale === 'en' || locale === 'en-US'
        ? 'en-US'
        : navigator.language.toLowerCase().startsWith('zh') ? 'zh-CN' : 'en-US'
    root.dataset.theme = theme
    root.style.colorScheme = theme
  })()
</script>
```

`theme` is the resolved light/dark value; `appearance` is a preference and may be `system`. If the host URL has no valid `theme`, this snippet falls back to the WebView preference. Standalone browser pages fall back to `navigator.language` and `prefers-color-scheme`.

## Sync changes at runtime

Install a host message listener when the app starts. This framework-agnostic example preserves app state and updates locale and theme only. A React app can wrap `applyContext` in its state store or `useSyncExternalStore`.

```ts
type Locale = 'zh-CN' | 'en-US'
type Theme = 'light' | 'dark'
type AppContext = { locale: Locale; theme: Theme }

const params = new URLSearchParams(location.search)
const hostOrigin = params.get('hostOrigin')
let hosted = params.get('runtime') === 'agentos' || hostOrigin !== null
const systemTheme = matchMedia('(prefers-color-scheme: dark)')

function resolveLocale(value: string | null): Locale {
  return value === 'zh' || value === 'zh-CN' ? 'zh-CN'
    : value === 'en' || value === 'en-US' ? 'en-US'
      : navigator.language.toLowerCase().startsWith('zh') ? 'zh-CN' : 'en-US'
}

const launchTheme = hosted ? params.get('theme') : null
let currentContext: AppContext = {
  locale: resolveLocale(hosted ? params.get('locale') : null),
  theme: launchTheme === 'light' || launchTheme === 'dark'
    ? launchTheme
    : systemTheme.matches ? 'dark' : 'light',
}

function isTrustedSource(event: MessageEvent<unknown>): boolean {
  // Native AgentOS delivers the context event to the WebView document.
  if (event.source === window) return true
  // WebShell delivers these messages from the containing iframe.
  if (event.source !== window.parent || window.parent === window) return false
  try {
    const expectedOrigin = hostOrigin ? new URL(hostOrigin).origin : location.origin
    return event.origin === expectedOrigin
  } catch {
    return false
  }
}

function applyContext(context: AppContext) {
  currentContext = context
  const root = document.documentElement
  root.lang = context.locale
  root.dataset.theme = context.theme
  root.dataset.fennTheme = context.theme // Keep only if a UI library uses this marker.
  root.style.colorScheme = context.theme
  // Also update your app's locale/theme store here.
}
applyContext(currentContext)

window.addEventListener('message', (event: MessageEvent<unknown>) => {
  if (!isTrustedSource(event) || !event.data || typeof event.data !== 'object') return
  const value = event.data as Record<string, unknown>

  if (value.type === 'shell:theme' && (value.appearance === 'light' || value.appearance === 'dark')) {
    hosted = true
    applyContext({ ...currentContext, theme: value.appearance })
    return
  }
  if (value.type === 'shell:locale' && (value.locale === 'zh' || value.locale === 'en')) {
    hosted = true
    applyContext({ ...currentContext, locale: value.locale === 'zh' ? 'zh-CN' : 'en-US' })
    return
  }

  if (value.channel !== 'agentos:host' || value.type !== 'context-changed'
    || value.runtime !== 'agentos'
    || (value.schemaVersion !== undefined && value.schemaVersion !== 1)) return

  hosted = true
  const next = { ...currentContext }
  if (value.locale === 'zh-CN' || value.locale === 'en-US') next.locale = value.locale
  if (value.theme === 'light' || value.theme === 'dark') next.theme = value.theme
  applyContext(next)
})

systemTheme.addEventListener('change', event => {
  if (!hosted) applyContext({ ...currentContext, theme: event.matches ? 'dark' : 'light' })
})
```

In this example, `currentContext` is the app's current context. Initialize it after the first-paint script and keep it in sync in `applyContext` or the app's state store. Standalone browser apps can also listen for `languagechange` if they should follow browser language changes. In a hosted app, the host drives updates; do not let a local media query override a later host theme message.

## CSS and translations

Describe colors with semantic variables, then switch the variables through the root `data-theme` attribute. Components can use the same tokens in both themes.

```css
:root {
  color-scheme: light;
  --page: #f5f7fa;
  --surface: #fff;
  --text: #1d2939;
  --muted: #667085;
  --border: #d0d5dd;
}

:root[data-theme='dark'] {
  color-scheme: dark;
  --page: #10151d;
  --surface: #171e28;
  --text: #e9edf3;
  --muted: #98a2b3;
  --border: #344054;
}

body { color: var(--text); background: var(--page); }
```

Keep UI strings in locale-indexed resources. When locale changes, update app state, `document.documentElement.lang`, and localized `document.title`. Use `Intl` for dates, numbers, and plural rules. Do not translate filenames, user input, or raw diagnostics. If the app has separate HTML pages, such as help or credits, preserve host parameters in internal links or load shared context there so the page does not fall back to browser appearance.

## Checklist for new apps

- The first-paint script recognizes `runtime=agentos` and WebShell `hostOrigin`, and sets `lang`, `data-theme`, and `color-scheme` before the framework starts.
- Hosted apps prefer the resolved URL `theme`; `appearance=system` is not itself a color value.
- Runtime listeners handle native `agentos:host` context messages and WebShell parent `shell:theme` / `shell:locale` messages.
- Message source and parent origin are checked; messages from other frames are ignored.
- Standalone browsers use `navigator.language` and `prefers-color-scheme`, with listeners where live changes are needed.
- Both themes cover all screens and control states; changing locale or theme does not reload the page or discard in-progress app state.

AndroidTone is a complete React example of this pattern: [host context implementation](../androidtone/frontend/src/hostContext.ts) and [first-paint initialization](../androidtone/frontend/index.html).
