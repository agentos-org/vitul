import { createHostProvider, type HostContext, type HostProvider } from './host'
import { callOpenTool } from './opentool'

type Locale = 'en' | 'zh'
type LanguagePreference = 'system' | Locale
type ThemePreference = 'system' | 'light' | 'dark' | 'ocean'
type WindowAction = 'minimize' | 'maximize' | 'fullscreen' | 'close'

const copy = {
  en: {
    language: 'Language',
    theme: 'Theme',
    system: 'System',
    light: 'Light',
    dark: 'Dark',
    ocean: 'Ocean',
    eyebrow: 'HOST CAPABILITIES',
    title: 'A window into your OS.',
    description:
      'Explore an overlay title bar, native window actions, and an OpenTool business API in one small app.',
    connectedHost: 'CONNECTED HOST',
    detectingHost: 'Detecting host…',
    systemApi: 'SYSTEM API',
    windowControls: 'Window controls',
    windowDescription:
      'These actions are handled by the current OS or AgentOS window host.',
    minimize: 'Minimize',
    maximize: 'Maximize',
    restore: 'Restore',
    enterFullscreen: 'Enter full screen',
    exitFullscreen: 'Exit full screen',
    closeWindow: 'Close window',
    chooseAction: 'Choose an action to call the host.',
    windowReady: 'Window ready',
    windowMaximized: 'Window maximized',
    windowFullscreen: 'Full screen',
    windowNormal: 'Windowed',
    titlebarLabel: 'Title bar layout',
    titlebarWaiting: 'Overlay requested · waiting for host status',
    titlebarOverlayNative: 'Overlay active · operating system controls',
    titlebarOverlayAgentOS: 'Overlay active · AgentOS window controls',
    titlebarOverlayNoControls: 'Overlay active · no host window controls',
    titlebarSystemNative: 'System title bar · operating system controls',
    titlebarSystemAgentOS: 'AgentOS title bar · AgentOS window controls',
    titlebarSystemNoControls: 'System layout · no host window controls',
    titlebarUnsupported: 'Overlay requested · unsupported for this window',
    titlebarUnavailable: 'Title bar status is not available in this preview',
    active: 'Active',
    unsupported: 'Unsupported',
    browserHost: 'Browser preview',
    nativeHost: 'Desktop runtime',
    agentOSHost: 'AgentOS',
    agentOSWebUIHost: 'AgentOS WebUI',
    browserWindowNote: 'Window operations need a native host.',
    actionSending: 'Sending {action} through host IPC…',
    actionDone: '{action} completed by the host.',
    actionFailed: 'The host could not complete {action}: {error}',
    businessApi: 'BUSINESS API',
    greetTitle: 'OpenTool greeting',
    greetDescription:
      'This request goes to the Rust backend through OpenTool JSON-RPC.',
    yourName: 'Your name',
    greet: 'Call greet',
    responsePlaceholder: 'The response will appear here.',
    callingOpenTool: 'Calling the OpenTool backend…',
    footer: 'One UI · multiple hosts · same app contract',
    ariaHome: 'Vitul Demo home',
    ariaLanguage: 'Choose language',
    ariaTheme: 'Choose theme',
    ariaWindowActions: 'Window actions'
  },
  zh: {
    language: '语言',
    theme: '主题',
    system: '跟随系统',
    light: '浅色',
    dark: '深色',
    ocean: '海蓝',
    eyebrow: '宿主能力体验',
    title: '在窗口里感受操作系统。',
    description:
      '在同一个小应用中体验标题栏覆盖、系统窗口操作和 OpenTool 业务接口。',
    connectedHost: '当前宿主',
    detectingHost: '正在检测宿主…',
    systemApi: '系统 API',
    windowControls: '窗口操作',
    windowDescription: '这些操作由当前操作系统或 AgentOS 窗口宿主处理。',
    minimize: '最小化',
    maximize: '最大化',
    restore: '还原',
    enterFullscreen: '进入全屏',
    exitFullscreen: '退出全屏',
    closeWindow: '关闭窗口',
    chooseAction: '选择一个操作，调用当前宿主。',
    windowReady: '窗口就绪',
    windowMaximized: '窗口已最大化',
    windowFullscreen: '全屏模式',
    windowNormal: '普通窗口',
    titlebarLabel: '标题栏布局',
    titlebarWaiting: '已请求覆盖模式 · 正在等待宿主状态',
    titlebarOverlayNative: '覆盖模式已启用 · 由操作系统管理窗口按钮',
    titlebarOverlayAgentOS: '覆盖模式已启用 · AgentOS 窗口控件',
    titlebarOverlayNoControls: '覆盖模式已启用 · 宿主未提供窗口控件',
    titlebarSystemNative: '系统标题栏 · 由操作系统管理窗口按钮',
    titlebarSystemAgentOS: 'AgentOS 标题栏 · 由 AgentOS 管理窗口控件',
    titlebarSystemNoControls: '系统布局 · 宿主未提供窗口控件',
    titlebarUnsupported: '已请求覆盖模式 · 当前窗口不支持',
    titlebarUnavailable: '此预览中无法读取标题栏状态',
    active: '可用',
    unsupported: '不支持',
    browserHost: '浏览器预览',
    nativeHost: '桌面运行时',
    agentOSHost: 'AgentOS',
    agentOSWebUIHost: 'AgentOS WebUI',
    browserWindowNote: '窗口操作需要原生宿主。',
    actionSending: '正在通过宿主 IPC 发送：{action}…',
    actionDone: '宿主已完成操作：{action}。',
    actionFailed: '宿主无法完成操作 {action}：{error}',
    businessApi: '业务 API',
    greetTitle: 'OpenTool 问候',
    greetDescription: '请求通过 OpenTool JSON-RPC 发送到 Rust 后端。',
    yourName: '你的名字',
    greet: '调用问候接口',
    responsePlaceholder: '接口响应会显示在这里。',
    callingOpenTool: '正在调用 OpenTool 后端…',
    footer: '一套界面 · 多种宿主 · 相同应用接口',
    ariaHome: 'Vitul Demo 首页',
    ariaLanguage: '选择语言',
    ariaTheme: '选择主题',
    ariaWindowActions: '窗口操作'
  }
} as const

const app = document.querySelector<HTMLElement>('#app')!
const hostStatus = document.querySelector<HTMLElement>('#host-status')!
const systemMessage = document.querySelector<HTMLElement>('#system-message')!
const titlebarCopy = document.querySelector<HTMLElement>('#titlebar-copy')!
const titlebarBadge = document.querySelector<HTMLElement>('#titlebar-badge')!
const windowStateLabel = document.querySelector<HTMLElement>(
  '#window-state-label'
)!
const languageSelect =
  document.querySelector<HTMLSelectElement>('#language-select')!
const themeSelect = document.querySelector<HTMLSelectElement>('#theme-select')!
const maximizeLabel = document.querySelector<HTMLElement>('#maximize-label')!
const fullscreenLabel =
  document.querySelector<HTMLElement>('#fullscreen-label')!
const minimizeButton =
  document.querySelector<HTMLButtonElement>('#minimize-button')!
const maximizeButton =
  document.querySelector<HTMLButtonElement>('#maximize-button')!
const fullscreenButton =
  document.querySelector<HTMLButtonElement>('#fullscreen-button')!
const closeButton = document.querySelector<HTMLButtonElement>('#close-button')!
const form = document.querySelector<HTMLFormElement>('#greet-form')!
const input = document.querySelector<HTMLInputElement>('#greet-input')!
const businessMessage =
  document.querySelector<HTMLElement>('#business-message')!

const host = await createHostProvider()
const launchParams = new URLSearchParams(window.location.search)
const isAgentOSWebUI =
  launchParams.get('agentosInstanceId')?.startsWith('webshell:') === true
  || launchParams.get('runtime') === 'webshell'
  || launchParams.get('agentosRuntime') === 'webshell'
const systemDarkPreference = window.matchMedia('(prefers-color-scheme: dark)')
let languagePreference = readPreference<LanguagePreference>(
  'vitul-demo-language',
  'system',
  ['system', 'en', 'zh'],
  'window-lab-language'
)
let themePreference = readPreference<ThemePreference>(
  'vitul-demo-theme',
  host.context.appearance ?? 'system',
  ['system', 'light', 'dark', 'ocean'],
  'window-lab-theme'
)
let locale = resolveLocale(languagePreference, host.context)
let maximized = false
let fullscreen = false
const windowActionsSupported =
  host.platform !== 'browser'
  && host.context.windowTitleBar?.controlsProvider !== 'none'

app.dataset.host = isAgentOSWebUI
  ? 'agentos-webui'
  : host.platform === 'native' && /Mac|iPhone|iPad/.test(navigator.platform)
    ? 'native-macos'
    : host.platform
document.documentElement.dataset.host = app.dataset.host

function renderTitlebarOverlaySafeArea(context: HostContext) {
  const usesAgentOSTitlebarOverlay =
    (app.dataset.host === 'agentos' || app.dataset.host === 'agentos-webui')
    && context.windowTitleBar?.actualMode === 'overlay'
  app.dataset.titlebarOverlay = usesAgentOSTitlebarOverlay ? 'true' : 'false'
}

renderTitlebarOverlaySafeArea(host.context)
for (const button of [
  minimizeButton,
  maximizeButton,
  fullscreenButton,
  closeButton
]) {
  button.disabled = !windowActionsSupported
  if (button.disabled) button.title = copy[locale].browserWindowNote
}

function readPreference<T extends string>(
  key: string,
  fallback: T,
  values: readonly T[],
  legacyKey?: string
): T {
  try {
    const value = localStorage.getItem(key)
    if (value && values.includes(value as T)) return value as T
    const legacyValue = legacyKey ? localStorage.getItem(legacyKey) : null
    if (legacyValue && values.includes(legacyValue as T)) {
      try {
        localStorage.setItem(key, legacyValue)
      } catch {
        /* Keep using the saved preference if migration is unavailable. */
      }
      return legacyValue as T
    }
  } catch {
    // Use the host or system preference when local storage is unavailable.
  }
  return fallback
}

function resolveLocale(
  preference: LanguagePreference,
  context: HostContext
): Locale {
  if (preference === 'zh' || preference === 'en') return preference
  if (context.locale) return context.locale === 'zh-CN' ? 'zh' : 'en'
  return navigator.language.toLowerCase().startsWith('zh') ? 'zh' : 'en'
}

function effectiveTheme(): 'system' | 'light' | 'dark' | 'ocean' {
  if (themePreference !== 'system') return themePreference
  return host.context.theme ?? (systemDarkPreference.matches ? 'dark' : 'light')
}

function renderLocale() {
  const text = copy[locale]
  document.documentElement.lang = locale === 'zh' ? 'zh-CN' : 'en'
  document.title = 'Vitul Demo'
  const staticText: Record<string, string> = {
    'language-label': text.language,
    'theme-label': text.theme,
    eyebrow: text.eyebrow,
    'page-title': text.title,
    'page-description': text.description,
    'host-caption': text.connectedHost,
    'window-eyebrow': text.systemApi,
    'window-title': text.windowControls,
    'window-description': text.windowDescription,
    'minimize-label': text.minimize,
    'fullscreen-label': fullscreen ? text.exitFullscreen : text.enterFullscreen,
    'close-label': text.closeWindow,
    'titlebar-label': text.titlebarLabel,
    'business-eyebrow': text.businessApi,
    'business-title': text.greetTitle,
    'business-description': text.greetDescription,
    'name-label': text.yourName,
    'greet-button': text.greet,
    'footer-copy': text.footer
  }
  for (const [id, value] of Object.entries(staticText)) {
    const element = document.getElementById(id)
    if (element) element.textContent = value
  }

  setOptionLabel(languageSelect, 'system', text.system)
  setOptionLabel(languageSelect, 'en', 'English')
  setOptionLabel(languageSelect, 'zh', '简体中文')
  setOptionLabel(themeSelect, 'system', text.system)
  setOptionLabel(themeSelect, 'light', text.light)
  setOptionLabel(themeSelect, 'dark', text.dark)
  setOptionLabel(themeSelect, 'ocean', text.ocean)
  languageSelect.value = languagePreference
  themeSelect.value = themePreference
  languageSelect.setAttribute('aria-label', text.ariaLanguage)
  themeSelect.setAttribute('aria-label', text.ariaTheme)
  document
    .querySelector('.window-actions')
    ?.setAttribute('aria-label', text.ariaWindowActions)
  document.querySelector('.brand')?.setAttribute('aria-label', text.ariaHome)
  hostStatus.textContent = isAgentOSWebUI
    ? text.agentOSWebUIHost
    : getHostLabel(host.platform, text)
  if (host.platform === 'browser')
    systemMessage.textContent = text.browserWindowNote
  else if (!systemMessage.dataset.actionResult)
    systemMessage.textContent = text.chooseAction
  businessMessage.textContent =
    businessMessage.dataset.result ?? text.responsePlaceholder
  renderWindowState()
  renderWindowLabels()
  renderTitlebarState()
}

function setOptionLabel(
  select: HTMLSelectElement,
  value: string,
  label: string
) {
  const option = select.querySelector<HTMLOptionElement>(
    `option[value="${value}"]`
  )
  if (option) option.textContent = label
}

function getHostLabel(
  platform: HostProvider['platform'],
  text: (typeof copy)[Locale]
): string {
  switch (platform) {
    case 'agentos':
      return text.agentOSHost
    case 'native':
      return text.nativeHost
    case 'browser':
      return text.browserHost
  }
}

function renderWindowLabels() {
  const text = copy[locale]
  maximizeLabel.textContent = maximized ? text.restore : text.maximize
  fullscreenLabel.textContent = fullscreen
    ? text.exitFullscreen
    : text.enterFullscreen
}

function renderWindowState() {
  const text = copy[locale]
  windowStateLabel.textContent = fullscreen
    ? text.windowFullscreen
    : maximized
      ? text.windowMaximized
      : text.windowNormal
}

function renderTheme() {
  const theme = effectiveTheme()
  document.documentElement.dataset.theme = theme
  document.documentElement.style.colorScheme =
    theme === 'light' ? 'light' : 'dark'
}

function renderTitlebarState() {
  const state = host.context.windowTitleBar
  const text = copy[locale]
  titlebarBadge.className = 'status-badge'

  if (!state) {
    if (host.platform === 'browser') {
      titlebarCopy.textContent = text.titlebarUnavailable
      titlebarBadge.textContent = text.unsupported
      titlebarBadge.classList.add('unsupported')
      return
    }
    titlebarCopy.textContent =
      host.platform === 'agentos'
        ? text.titlebarWaiting
        : text.titlebarUnavailable
    titlebarBadge.textContent = text.active
    titlebarBadge.classList.add('pending')
    return
  }

  if (state.supportStatus === 'unsupported' || state.actualMode === null) {
    titlebarCopy.textContent = text.titlebarUnsupported
    titlebarBadge.textContent = text.unsupported
    titlebarBadge.classList.add('unsupported')
    return
  }

  if (state.actualMode === 'overlay') {
    titlebarCopy.textContent = controlProviderMessage(state, text, 'overlay')
  } else {
    titlebarCopy.textContent = controlProviderMessage(state, text, 'system')
  }
  titlebarBadge.textContent = text.active
}

function controlProviderMessage(
  state: NonNullable<HostContext['windowTitleBar']>,
  text: (typeof copy)[Locale],
  mode: 'overlay' | 'system'
) {
  if (state.controlsProvider === 'operating-system') {
    return mode === 'overlay'
      ? text.titlebarOverlayNative
      : text.titlebarSystemNative
  }
  if (state.controlsProvider === 'agentos') {
    return mode === 'overlay'
      ? text.titlebarOverlayAgentOS
      : text.titlebarSystemAgentOS
  }
  return mode === 'overlay'
    ? text.titlebarOverlayNoControls
    : text.titlebarSystemNoControls
}

async function runWindowAction(action: WindowAction) {
  if (host.platform === 'browser') return
  const text = copy[locale]
  const actionName = {
    minimize: text.minimize,
    maximize: maximized ? text.restore : text.maximize,
    fullscreen: fullscreen ? text.exitFullscreen : text.enterFullscreen,
    close: text.closeWindow
  }[action]
  systemMessage.dataset.actionResult = 'true'
  systemMessage.textContent = text.actionSending.replace('{action}', actionName)

  try {
    switch (action) {
      case 'minimize':
        await host.window.minimize()
        break
      case 'maximize':
        if (maximized) await host.window.restore()
        else await host.window.maximize()
        maximized = !maximized
        break
      case 'fullscreen':
        await host.window.setFullscreen(!fullscreen)
        fullscreen = !fullscreen
        break
      case 'close':
        await host.window.close()
        break
    }
    systemMessage.textContent = text.actionDone.replace('{action}', actionName)
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error)
    systemMessage.textContent = text.actionFailed
      .replace('{action}', actionName)
      .replace('{error}', reason)
  }
  renderWindowLabels()
  renderWindowState()
  systemMessage.dataset.actionResult = 'true'
}

minimizeButton.addEventListener('click', () => void runWindowAction('minimize'))
maximizeButton.addEventListener('click', () => void runWindowAction('maximize'))
fullscreenButton.addEventListener(
  'click',
  () => void runWindowAction('fullscreen')
)
closeButton.addEventListener('click', () => void runWindowAction('close'))

languageSelect.addEventListener('change', () => {
  languagePreference = languageSelect.value as LanguagePreference
  try {
    localStorage.setItem('vitul-demo-language', languagePreference)
  } catch {
    /* Optional preference. */
  }
  locale = resolveLocale(languagePreference, host.context)
  renderLocale()
})

themeSelect.addEventListener('change', () => {
  themePreference = themeSelect.value as ThemePreference
  try {
    localStorage.setItem('vitul-demo-theme', themePreference)
  } catch {
    /* Optional preference. */
  }
  renderTheme()
})

host.subscribeContext((context) => {
  renderTitlebarOverlaySafeArea(context)
  if (languagePreference === 'system')
    locale = resolveLocale(languagePreference, context)
  if (context.windowState === 'maximized') maximized = true
  else if (context.windowState === 'normal') maximized = false
  if (context.windowState === 'fullscreen') fullscreen = true
  else if (context.windowState === 'normal') fullscreen = false
  renderTheme()
  renderLocale()
})

systemDarkPreference.addEventListener('change', () => {
  if (themePreference === 'system' && !host.context.theme) renderTheme()
})

form.addEventListener('submit', async (event) => {
  event.preventDefault()
  businessMessage.dataset.result = copy[locale].callingOpenTool
  businessMessage.textContent = businessMessage.dataset.result
  try {
    const result = await callOpenTool<{ message: string }>('greet', {
      name: input.value
    })
    businessMessage.dataset.result = result.message
  } catch (error) {
    businessMessage.dataset.result =
      error instanceof Error ? error.message : String(error)
  }
  businessMessage.textContent = businessMessage.dataset.result
})

renderTheme()
renderLocale()
void Promise.allSettled([
  host.window.isMaximized(),
  host.window.isFullscreen()
]).then((results) => {
  if (results[0].status === 'fulfilled') maximized = results[0].value
  if (results[1].status === 'fulfilled') fullscreen = results[1].value
  renderWindowLabels()
  renderWindowState()
})
