# AgentOS WebApp 多语言与主题接入

本指南适用于嵌入 AgentOS WebView、通过 AgentOS WebShell iframe 打开的 WebApp。语言和颜色主题由宿主提供，应用应在首次绘制前读取宿主上下文，并在页面保持打开时响应宿主变更。普通浏览器仍可回退到浏览器语言和系统配色偏好。协议不依赖 React、Vitul 或 `agentos-sdk-ts`。

## 宿主提供什么

启动 URL 可能带有以下参数：

| 参数 | 值 | 用途 |
| --- | --- | --- |
| `runtime=agentos` | `agentos` | 原生 AgentOS WebApp 启动标记 |
| `hostOrigin` | 宿主的 origin | WebShell 插件 URL 中的宿主标记，也用于校验父窗口消息来源 |
| `locale` | `zh`、`en`、`zh-CN` 或 `en-US` | 宿主解析后的界面语言 |
| `appearance` | `system`、`light` 或 `dark` | 用户选择的外观偏好 |
| `theme` | `light` 或 `dark` | 宿主解析后的有效配色 |

WebShell 插件 URL 会提供 `hostOrigin`、`locale`、`appearance` 和 `theme`，但可能不带 `runtime=agentos`。因此，只要有 `runtime=agentos` 或 `hostOrigin`，就应把页面视为宿主环境。`appearance=system` 表示用户选择跟随系统；实际颜色应优先采用宿主解析后的 `theme`。不要只因为 WebView 的 `prefers-color-scheme` 返回 dark，就覆盖宿主传入的 light。

宿主会在应用运行期间发送消息：

| 消息 | 来源与字段 | 用途 |
| --- | --- | --- |
| `agentos:host` / `context-changed` | 原生 AgentOS 上下文；`runtime: "agentos"`、`locale`、`appearance`、`theme`；`schemaVersion` 当前为 1，旧宿主可能省略 | 同步宿主语言和有效主题 |
| `shell:theme` | WebShell 父 iframe；`appearance` 为解析后的 `light` 或 `dark` | 同步主题切换 |
| `shell:locale` | WebShell 父 iframe；`locale` 为 `zh` 或 `en` | 同步语言切换 |

原生 AgentOS 上下文通知可能由当前 WebView 文档接收；WebShell 的 `shell:*` 消息由父窗口发出。因此，不能只接受 `event.source === window`，也不能接受任意 iframe 发来的消息。处理父窗口通知时，应要求 `event.source === window.parent`，并校验 `event.origin`：有 `hostOrigin` 时与其 origin 比较；没有时仅接受同源父窗口。

## 首屏初始化

在 `index.html` 的 `<head>` 中放置小型同步脚本，在 React/Vue 等框架和应用 CSS 初始化前设置语言及主题。把 `theme` 写入根元素，CSS 即可按语义变量绘制正确首屏。

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

`theme` 是宿主解析后的 light/dark 配色；`appearance` 是用户偏好，也可能为 `system`。宿主 URL 没有有效 `theme` 时，示例回退到 WebView 配色；普通浏览器则回退到 `navigator.language` 和 `prefers-color-scheme`。

## 运行期间同步

在应用启动时安装一个宿主消息监听器。下面的框架无关示例保留应用状态，只更新语言和主题；React 应用可以在 `applyContext` 外包一层状态 store 或 `useSyncExternalStore`。

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

示例中的 `currentContext` 是应用自己的当前上下文，需要在首屏初始化后赋初值，并在 `applyContext` 或状态 store 更新时同步。实际应用也应在普通浏览器中监听 `languagechange`（若要动态跟随浏览器语言）。宿主模式下由宿主通知驱动更新；不要让本地 media query 在宿主通知后再次覆盖有效主题。

## CSS 与翻译资源

用语义变量描述颜色，并根据根元素的 `data-theme` 切换变量。这样应用组件不必分别处理每种颜色。

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

把界面文案集中在按 locale 索引的资源中；语言变化时更新应用状态、`document.documentElement.lang` 和需要本地化的 `document.title`。日期、数字和复数形式使用 `Intl` API。不要翻译文件名、用户输入或原始诊断信息。若应用有单独的 HTML 页面（例如帮助或署名页），内部链接应保留宿主参数，或让页面读取共享上下文，避免新页面回退到浏览器主题。

## 新应用检查清单

- 首屏脚本识别 `runtime=agentos` 和 WebShell 的 `hostOrigin`，并在框架启动前设置 `lang`、`data-theme` 和 `color-scheme`。
- 宿主环境优先采用 URL 的有效 `theme`；`appearance=system` 本身不是颜色值。
- 运行时同时处理原生 `agentos:host` 上下文通知和 WebShell 父窗口的 `shell:theme`、`shell:locale` 通知。
- 校验消息来源与父窗口 origin；拒绝其他 frame 的消息。
- 普通浏览器使用 `navigator.language` 与 `prefers-color-scheme`，并按需监听变化。
- 深浅配色覆盖全部页面和控件状态；切换语言或主题时不重载页面、不丢失进行中的应用状态。

AndroidTone 是此模式的完整 React 示例，见 [host context 实现](../androidtone/frontend/src/hostContext.ts) 和 [首屏初始化](../androidtone/frontend/index.html)。
