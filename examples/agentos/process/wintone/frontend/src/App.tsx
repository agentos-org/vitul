import {
  ArrowRight,
  Download,
  Grid2X2,
  LayoutGrid,
  PanelTop,
  Plus,
  RefreshCw,
  ShieldCheck,
  ShoppingBag,
  Trash2,
  X
} from 'lucide-react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type { ChangeEvent, DragEvent, ReactNode } from 'react'
import logoUrl from './assets/icon.png'
import { messages } from './i18n'
import type { Locale, MessageKey } from './i18n'

type ThemePreference = 'system' | 'light' | 'dark'
type DashboardSection = 'installed' | 'installers'

type InstalledApp = {
  id: string
  name: string
  version?: string | null
  windowsArchitecture: string
  sessionId?: string | null
  uninstallExecutable?: string | null
  lastError?: string | null
}

type InstallTask = {
  id: string
  fileName: string
  extension: string
  kind?: 'installer' | 'portable'
  bytes: number
  addedAt: number | string
  windowsArchitecture: string
  status: string
  message?: string | null
}

type RuntimeStatus = {
  state: string
  bundled?: boolean
  message?: string | null
}

type PermissionCapability = {
  available?: boolean
  reason?: string
}

type WindowIntegration = {
  available?: boolean
  message?: string
  gpu?: PermissionCapability
  clipboard?: PermissionCapability
}

type SessionPreferences = {
  gpu: boolean
  clipboardRead: boolean
  clipboardWrite: boolean
}

type Notice = { message: string; kind: 'info' | 'error' | 'success' }
type ApiErrorBody = { error?: string }

const APP_BASE = '/apps/wintone'
const LOCALE_KEY = 'wintone.language'
const THEME_KEY = 'wintone.theme'
const MAX_INSTALLER_BYTES = 2 * 1024 * 1024 * 1024
const INITIAL_PREFERENCES: SessionPreferences = {
  gpu: false,
  clipboardRead: false,
  clipboardWrite: false
}

const ACTIVE_TASKS = new Set([
  'preparing-runtime',
  'preparing-prefix',
  'waiting-runtime',
  'waiting-window-session',
  'starting-app',
  'starting-installer',
  'awaiting-user',
  'discovering-apps'
])

const BUSY_TASKS = new Set([
  'preparing-runtime',
  'preparing-prefix',
  'starting-app',
  'starting-installer',
  'awaiting-user',
  'discovering-apps'
])

function readStoredValue(key: string): string | null {
  try {
    return window.localStorage.getItem(key)
  } catch {
    return null
  }
}

function initialLocale(): Locale {
  const saved = readStoredValue(LOCALE_KEY)
  if (saved === 'zh-CN' || saved === 'en') return saved
  return navigator.language?.toLowerCase().startsWith('en') ? 'en' : 'zh-CN'
}

function initialTheme(): ThemePreference {
  const saved = readStoredValue(THEME_KEY)
  return saved === 'light' || saved === 'dark' || saved === 'system'
    ? saved
    : 'system'
}

function hasAgentOSHostContext(): boolean {
  const params = new URLSearchParams(window.location.search)
  return params.get('runtime') === 'agentos' || params.has('hostOrigin')
}

function agentosThemeFromUrl(): 'light' | 'dark' | null {
  if (!hasAgentOSHostContext()) return null
  const params = new URLSearchParams(window.location.search)
  const theme = params.get('theme') ?? params.get('appearance')
  return theme === 'light' || theme === 'dark' ? theme : null
}

function isTrustedHostMessage(event: MessageEvent<unknown>): boolean {
  if (event.source === window) return true
  if (event.source !== window.parent || window.parent === window) return false
  const params = new URLSearchParams(window.location.search)
  const hostOrigin = params.get('hostOrigin')
  try {
    return event.origin === (hostOrigin ? new URL(hostOrigin).origin : window.location.origin)
  } catch {
    return false
  }
}

function initialSystemDark(): boolean {
  const hostTheme = agentosThemeFromUrl()
  return hostTheme
    ? hostTheme === 'dark'
    : window.matchMedia('(prefers-color-scheme: dark)').matches
}

function getErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

async function request<T>(
  path: string,
  options: RequestInit | undefined,
  requestFailed: (status: number) => string
): Promise<T> {
  const response = await fetch(`${APP_BASE}/api${path}`, options)
  const body = (await response.json().catch(() => ({}))) as ApiErrorBody & T
  if (!response.ok) {
    throw new Error(body.error || requestFailed(response.status))
  }
  return body
}

function formatBytes(value: number): string {
  if (value < 1024) return `${value} B`
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(0)} KB`
  if (value < 1024 * 1024 * 1024)
    return `${(value / 1024 / 1024).toFixed(1)} MB`
  return `${(value / 1024 / 1024 / 1024).toFixed(2)} GB`
}

function formatDate(value: number | string, locale: Locale): string {
  return new Intl.DateTimeFormat(locale, {
    month: 'short',
    day: 'numeric'
  }).format(new Date(Number(value)))
}

function architectureLabel(value: string): string {
  return value === 'arm64' ? 'ARM64' : 'x86/x64'
}

function localizeBackendMessage(
  message: string | null | undefined,
  locale: Locale,
  t: (key: MessageKey, values?: Record<string, string | number>) => string
): string {
  if (!message) return ''
  const known: Record<string, MessageKey> = {
    'Starting the Windows installer in an AgentOS Wayland session.':
      'installerStartingMessage',
    'The session was created; installer window mapping and Shell presentation remain unverified.':
      'installerCreatedMessage',
    'Recovered the existing AgentOS installer session.':
      'installerRecoveredMessage',
    'Recovered the completed AgentOS installer session.':
      'installerRecoveredCompleteMessage',
    'A mapped window was reported; Flutter presentation and input still require verification.':
      'mappedWindowMessage',
    'The AgentOS session was lost after restart.': 'installerLostMessage',
    'The installer session ended, but no launchable app was found in the Wine uninstall registry.':
      'installerEndedMessage',
    'The installer session ended, but no launchable app was found.':
      'installerEndedMessage',
    'Preparing a private Wine prefix for this installation.': 'preparingPrefix',
    'Reading the Wine prefix to find installed Windows apps.': 'discoveringApps',
    'The uninstall session ended, but the app remains in the Wine registry.':
      'uninstallStillPresentMessage',
    'The shared Wine/FEX runtime is not installed.': 'runtimeRepairMessage',
    'Preparing the shared Wine/FEX runtime.': 'runtimePreparingMessage',
    'Preparing the Windows runtime.': 'runtimePreparingMessage',
    'Starting the Windows app in an AgentOS Wayland session.':
      'portableStartingMessage',
    'The Windows app is running in an AgentOS Wayland session.':
      'portableRunningMessage',
    'The Windows app closed. You can launch it again from this task.':
      'portableClosedMessage'
  }
  const knownKey = known[message]
  if (knownKey) return t(knownKey)
  if (message.startsWith('Could not inspect Wine registry:'))
    return t('appDiscoveryFailedMessage')
  if (locale !== 'zh-CN') return message
  const discovered = message.match(
    /^Installation session ended; discovered (\d+) application entry\/entries\.$/
  )
  return discovered
    ? t('discoveredAppsMessage', { count: discovered[1] })
    : message
}

function taskStatusLabel(
  status: string,
  t: (key: MessageKey) => string
): string {
  const labels: Record<string, MessageKey> = {
    'waiting-runtime': 'waitingRuntime',
    'preparing-runtime': 'preparingRuntime',
    'preparing-prefix': 'preparingPrefix',
    'waiting-window-session': 'waitingWindow',
    'starting-app': 'startingApp',
    'starting-installer': 'startingInstaller',
    'awaiting-user': 'awaitingUser',
    'discovering-apps': 'discoveringApps',
    completed: 'completed',
    failed: 'failed',
    cancelled: 'cancelled'
  }
  return labels[status] ? t(labels[status]) : t('queued')
}

export default function App() {
  const [locale, setLocale] = useState<Locale>(initialLocale)
  const [themePreference, setThemePreference] =
    useState<ThemePreference>(initialTheme)
  const [systemDark, setSystemDark] = useState(initialSystemDark)
  const [apps, setApps] = useState<InstalledApp[]>([])
  const [installTasks, setInstallTasks] = useState<InstallTask[]>([])
  const [runtime, setRuntime] = useState<RuntimeStatus | null>(null)
  const [windowIntegration, setWindowIntegration] =
    useState<WindowIntegration | null>(null)
  const [sessionPreferences, setSessionPreferences] =
    useState<SessionPreferences>(INITIAL_PREFERENCES)
  const [notice, setNotice] = useState<Notice | null>(null)
  const [dragging, setDragging] = useState(false)
  const [uploadKind, setUploadKind] = useState<'installer' | 'portable'>('installer')
  const fileInput = useRef<HTMLInputElement>(null)
  const contentScroll = useRef<HTMLDivElement>(null)
  const installedSection = useRef<HTMLDivElement>(null)
  const integrationProbeAt = useRef(0)
  const integrationProbePending = useRef<Promise<void> | null>(null)
  const installersSection = useRef<HTMLElement>(null)
  const [activeSection, setActiveSection] = useState<DashboardSection>('installed')
  const noticeTimer = useRef<number | undefined>(undefined)

  const scrollToSection = useCallback((section: DashboardSection) => {
    const container = contentScroll.current
    const target =
      section === 'installed'
        ? installedSection.current
        : installersSection.current
    if (!container || !target) return

    const containerTop = container.getBoundingClientRect().top
    const targetTop = target.getBoundingClientRect().top
    const contentInset = Number.parseFloat(
      window.getComputedStyle(container).paddingTop
    ) || 0
    container.scrollTo({
      top: container.scrollTop + targetTop - containerTop - contentInset,
      behavior: 'smooth'
    })
    setActiveSection(section)
  }, [])

  const scrollToTop = useCallback(() => {
    contentScroll.current?.scrollTo({ top: 0, behavior: 'smooth' })
    setActiveSection('installed')
  }, [])

  const t = useCallback(
    (key: MessageKey, values: Record<string, string | number> = {}): string => {
      let message: string =
        messages[locale][key] || messages['zh-CN'][key] || key
      for (const [name, value] of Object.entries(values)) {
        message = message.replaceAll(`{${name}}`, String(value ?? ''))
      }
      return message
    },
    [locale]
  )
  const apiRequest = useCallback(
    <T,>(path: string, options?: RequestInit) =>
      request<T>(path, options, (status) => t('requestFailed', { status })),
    [t]
  )

  const showNotice = useCallback(
    (message: string, kind: Notice['kind'] = 'info') => {
      window.clearTimeout(noticeTimer.current)
      setNotice({ message, kind })
      noticeTimer.current = window.setTimeout(() => setNotice(null), 3800)
    },
    []
  )

  const loadWindowIntegration = useCallback(
    (preferencesConfigured: boolean) => {
      if (integrationProbePending.current) return integrationProbePending.current
      if (Date.now() - integrationProbeAt.current < 10_000) {
        return Promise.resolve()
      }
      integrationProbeAt.current = Date.now()

      const probe = (async () => {
        try {
          const loadCapabilities = () =>
            apiRequest<WindowIntegration>('/window-integration')
          let capabilities = await loadCapabilities()
          if (
            capabilities.message ===
            'AgentOS Wayland API request timed out.'
          ) {
            await new Promise((resolve) => window.setTimeout(resolve, 500))
            capabilities = await loadCapabilities()
          }
          setWindowIntegration(capabilities)
          if (!preferencesConfigured) {
            setSessionPreferences({
              gpu: capabilities.gpu?.available === true,
              clipboardRead: capabilities.clipboard?.available === true,
              clipboardWrite: capabilities.clipboard?.available === true
            })
          }
        } catch (error) {
          setWindowIntegration({
            available: false,
            message: getErrorMessage(error)
          })
          if (!preferencesConfigured) {
            setSessionPreferences(INITIAL_PREFERENCES)
          }
        }
      })()
      integrationProbePending.current = probe
      void probe.finally(() => {
        if (integrationProbePending.current === probe) {
          integrationProbePending.current = null
        }
      })
      return probe
    },
    [apiRequest]
  )

  const loadState = useCallback(async () => {
    try {
      const [tasks, installed, settings, status] = await Promise.all([
        apiRequest<{ installTasks?: InstallTask[] }>('/install-tasks'),
        apiRequest<{ apps?: InstalledApp[] }>('/apps'),
        apiRequest<{
          sessionPreferences?: SessionPreferences
          sessionPreferencesConfigured?: boolean
        }>('/settings'),
        apiRequest<{ runtime?: RuntimeStatus }>('/status')
      ])
      setInstallTasks(tasks.installTasks || [])
      setApps(installed.apps || [])
      setRuntime(status.runtime ?? null)
      const preferencesConfigured = settings.sessionPreferencesConfigured === true
      if (preferencesConfigured) {
        setSessionPreferences(settings.sessionPreferences || INITIAL_PREFERENCES)
      }
      // This probe talks to AgentOS over Wayland and can time out while its
      // service starts. Keep it separate so the app and task list stay usable.
      void loadWindowIntegration(preferencesConfigured)
    } catch (error) {
      showNotice(
        t('serviceUnavailable', { message: getErrorMessage(error) }),
        'error'
      )
    }
  }, [apiRequest, loadWindowIntegration, showNotice, t])

  useEffect(() => {
    void loadState()
  }, [loadState])

  useEffect(() => {
    if (
      !installTasks.some((task) => ACTIVE_TASKS.has(task.status))
      && !apps.some((app) => app.sessionId)
      && runtime?.state !== 'preparing'
      && !(runtime?.state === 'not-installed' && runtime.bundled)
    ) {
      return
    }
    const timer = window.setTimeout(() => void loadState(), 2500)
    return () => window.clearTimeout(timer)
  }, [apps, installTasks, loadState, runtime])

  useEffect(() => {
    const media = window.matchMedia('(prefers-color-scheme: dark)')
    const updateFromMedia = () => {
      if (!hasAgentOSHostContext()) setSystemDark(media.matches)
    }
    const updateFromAgentOS = (event: MessageEvent<unknown>) => {
      if (
        !isTrustedHostMessage(event)
        || typeof event.data !== 'object'
        || event.data === null
      ) {
        return
      }
      const message = event.data as {
        channel?: unknown
        type?: unknown
        runtime?: unknown
        schemaVersion?: unknown
        theme?: unknown
        appearance?: unknown
      }
      const isHostContext =
        message.channel === 'agentos:host'
        && message.type === 'context-changed'
        && message.runtime === 'agentos'
        && (message.schemaVersion === undefined || message.schemaVersion === 1)
      const isLegacyTheme = message.type === 'shell:theme'
      if (!isHostContext && !isLegacyTheme) return

      const theme = isHostContext ? message.theme : message.appearance
      if (theme === 'light' || theme === 'dark') setSystemDark(theme === 'dark')
    }
    media.addEventListener('change', updateFromMedia)
    window.addEventListener('message', updateFromAgentOS)
    return () => {
      media.removeEventListener('change', updateFromMedia)
      window.removeEventListener('message', updateFromAgentOS)
    }
  }, [])

  useEffect(() => {
    const theme =
      themePreference === 'system'
        ? systemDark
          ? 'dark'
          : 'light'
        : themePreference
    document.documentElement.dataset.theme = theme
    document.documentElement.dataset.themePreference = themePreference
    document.documentElement.lang = locale
    document.title = t('documentTitle')
    const themeColor = document.querySelector<HTMLMetaElement>(
      'meta[name="theme-color"]'
    )
    if (themeColor)
      themeColor.content = theme === 'dark' ? '#10151d' : '#f3f7fc'
  }, [locale, systemDark, themePreference, t])

  useEffect(
    () => () => {
      window.clearTimeout(noticeTimer.current)
    },
    []
  )

  const runtimePresentation = useMemo(() => {
    const states: Record<string, [string, string]> = {
      ready: [t('runtimeReady'), t('runtimeReadyMessage')],
      preparing: [t('runtimePreparing'), t('runtimePreparingMessage')],
      'not-installed': runtime?.bundled
        ? [t('runtimeBundledPending'), t('runtimeBundledFirstUse')]
        : [t('runtimeDownloadPending'), t('runtimeDownloadFirstUse')],
      'catalog-unavailable': [t('runtimePackagePending'), t('runtimeUnavailableMessage')],
      'catalog-invalid': [t('runtimePackageInvalid'), t('runtimeConfigurationMessage')],
      'runtime-invalid': [t('runtimeInvalid'), t('runtimeRepairMessage')],
      'prepare-failed': [t('runtimeInvalid'), t('runtimeRepairMessage')]
    }
    return (
      states[runtime?.state || ''] || [
        t('runtimePreparing'),
        runtime?.message || t('runtimeFirstUse')
      ]
    )
  }, [runtime, t])

  const chooseFiles = useCallback(() => fileInput.current?.click(), [])

  const uploadFile = useCallback(
    async (file: File, kind: 'installer' | 'portable' = uploadKind) => {
      const extension = file.name.split('.').at(-1)?.toLowerCase()
      if (!extension || !['exe', 'msi'].includes(extension)) {
        showNotice(t('onlyInstallers'), 'error')
        return
      }
      if (kind === 'portable' && extension !== 'exe') {
        showNotice(t('directLaunchExeOnly'), 'error')
        return
      }
      if (file.size > MAX_INSTALLER_BYTES) {
        showNotice(t('fileTooLarge'), 'error')
        return
      }
      const form = new FormData()
      form.append('kind', kind)
      form.append('file', file, file.name)
      showNotice(t('importingFile', { name: file.name }))
      try {
        await apiRequest('/install-tasks', { method: 'POST', body: form })
        await loadState()
        showNotice(
          kind === 'portable'
            ? t('importedPortable', { name: file.name })
            : t('importedFile', { name: file.name }),
          'success'
        )
      } catch (error) {
        showNotice(
          t('importFailed', { message: getErrorMessage(error) }),
          'error'
        )
      }
    },
    [apiRequest, loadState, showNotice, t, uploadKind]
  )

  const onFileChange = async (event: ChangeEvent<HTMLInputElement>) => {
    const files = Array.from(event.currentTarget.files || [])
    for (const file of files) await uploadFile(file)
    event.currentTarget.value = ''
  }

  const onDrop = async (event: DragEvent<HTMLElement>) => {
    event.preventDefault()
    setDragging(false)
    for (const file of Array.from(event.dataTransfer.files)) {
      await uploadFile(file)
    }
  }

  const onLanguageChange = (value: string) => {
    const next = value === 'en' ? 'en' : 'zh-CN'
    setLocale(next)
    try {
      window.localStorage.setItem(LOCALE_KEY, next)
    } catch {
      // Preferences remain available for the current session.
    }
    setNotice(null)
  }

  const onThemeChange = (value: string) => {
    const next: ThemePreference =
      value === 'light' || value === 'dark' ? value : 'system'
    setThemePreference(next)
    try {
      window.localStorage.setItem(THEME_KEY, next)
    } catch {
      // Preferences remain available for the current session.
    }
  }

  const saveSessionPreference = async (
    key: keyof SessionPreferences,
    checked: boolean
  ) => {
    const previous = sessionPreferences
    const next = { ...previous, [key]: checked }
    setSessionPreferences(next)
    try {
      const result = await apiRequest<{
        sessionPreferences?: SessionPreferences
      }>('/settings', {
        method: 'PUT',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(next)
      })
      setSessionPreferences(result.sessionPreferences || next)
      showNotice(t('sessionSaved'), 'success')
    } catch (error) {
      setSessionPreferences(previous)
      showNotice(
        t('sessionSaveFailed', { message: getErrorMessage(error) }),
        'error'
      )
    }
  }

  const removeTask = async (task: InstallTask) => {
    if (BUSY_TASKS.has(task.status)) return
    try {
      await apiRequest(`/install-tasks/${encodeURIComponent(task.id)}`, {
        method: 'DELETE'
      })
      await loadState()
      showNotice(t('taskRemoved'), 'success')
    } catch (error) {
      showNotice(
        t('taskRemoveFailed', { message: getErrorMessage(error) }),
        'error'
      )
    }
  }

  const runPortableTask = async (task: InstallTask) => {
    try {
      await apiRequest(`/install-tasks/${encodeURIComponent(task.id)}/run`, {
        method: 'POST'
      })
      await loadState()
      showNotice(t('portableLaunchStarted', { name: task.fileName }), 'success')
    } catch (error) {
      showNotice(
        t('portableLaunchFailed', { message: getErrorMessage(error) }),
        'error'
      )
    }
  }

  const launchApp = async (app: InstalledApp) => {
    try {
      await apiRequest(`/apps/${encodeURIComponent(app.id)}/launch`, {
        method: 'POST'
      })
      await loadState()
      showNotice(t('appLaunched'), 'success')
    } catch (error) {
      showNotice(
        t('appLaunchFailed', { message: getErrorMessage(error) }),
        'error'
      )
    }
  }

  const uninstallApp = async (app: InstalledApp) => {
    if (!window.confirm(t('uninstallConfirm', { name: app.name }))) return
    try {
      await apiRequest(`/apps/${encodeURIComponent(app.id)}`, {
        method: 'DELETE'
      })
      await loadState()
      showNotice(t('uninstallStarted', { name: app.name }), 'success')
    } catch (error) {
      showNotice(
        t('uninstallFailed', { message: getErrorMessage(error) }),
        'error'
      )
    }
  }

  const gpuAvailable = windowIntegration?.gpu?.available === true
  const clipboardAvailable = windowIntegration?.clipboard?.available === true
  let [runtimeLabel, runtimeMessage] = runtimePresentation
  if (runtime?.state === 'ready' && windowIntegration?.available === false) {
    runtimeMessage =
      localizeBackendMessage(windowIntegration.message, locale, t)
      || t('runtimeWindowUnavailable')
  }
  if (!runtime) {
    runtimeLabel = t('runtimePreparing')
    runtimeMessage = t('runtimeFirstUse')
  }

  return (
    <div className="app-shell flex min-h-screen">
      <aside className="sidebar">
        <button
          className="brand"
          type="button"
          onClick={scrollToTop}
          aria-label={t('brandHome')}
        >
          <span className="brand-mark" aria-hidden="true">
            <img src={logoUrl} alt="" />
          </span>
          <span className="brand-copy">
            <strong>WinTone</strong>
            <small>温易通</small>
          </span>
        </button>

        <div className="sidebar-label">{t('appCenter')}</div>
        <nav
          className="nav-list grid gap-[5px]"
          aria-label={t('mainNavigation')}
        >
          <button
            className={`nav-item${activeSection === 'installed' ? ' active' : ''}`}
            type="button"
            aria-current={activeSection === 'installed' ? 'page' : undefined}
            onClick={() => scrollToSection('installed')}
          >
            <LayoutGrid className="nav-icon" aria-hidden="true" />
            <span>{t('installed')}</span>
            <span className="nav-count">{apps.length}</span>
          </button>
          <button
            className={`nav-item${activeSection === 'installers' ? ' active' : ''}`}
            type="button"
            aria-current={activeSection === 'installers' ? 'page' : undefined}
            onClick={() => scrollToSection('installers')}
          >
            <Download className="nav-icon" aria-hidden="true" />
            <span>{t('installTasks')}</span>
            <span className="nav-count">{installTasks.length}</span>
          </button>
        </nav>

        <div className="sidebar-bottom">
          <div className="runtime-card">
            <div className="runtime-card-top">
              <span
                className={`status-dot ${runtime?.state === 'ready' ? 'ready' : 'pending'}`}
              />
              <span>{t('runtime')}</span>
            </div>
            <strong>{runtimeLabel}</strong>
            <p>{runtimeMessage}</p>
          </div>
          <div className="sidebar-foot">
            <span className="online-dot" />
            <span>{t('agentosLocalService')}</span>
          </div>
        </div>
      </aside>

      <main className="main ml-[240px] min-h-screen w-[calc(100%-240px)]">
        <header className="topbar">
          <div className="breadcrumb">
            <span>WinTone</span>
            <span className="breadcrumb-sep">/</span>
            <strong>{t('appCenter')}</strong>
          </div>
          <div className="topbar-actions">
            <label className="preference-control">
              <span>{t('language')}</span>
              <select
                aria-label={t('language')}
                value={locale}
                onChange={(event) =>
                  onLanguageChange(event.currentTarget.value)
                }
              >
                <option value="zh-CN">简体中文</option>
                <option value="en">English</option>
              </select>
            </label>
            <label className="preference-control theme-control">
              <span>{t('theme')}</span>
              <select
                aria-label={t('theme')}
                value={themePreference}
                onChange={(event) => onThemeChange(event.currentTarget.value)}
              >
                <option value="system">{t('themeSystem')}</option>
                <option value="light">{t('themeLight')}</option>
                <option value="dark">{t('themeDark')}</option>
              </select>
            </label>
            <button
              className="icon-button"
              type="button"
              aria-label={t('refresh')}
              title={t('refresh')}
              onClick={() => void loadState()}
            >
              <RefreshCw aria-hidden="true" />
            </button>
          </div>
        </header>

        <div
          className="content mx-auto max-w-[1120px] px-[42px] pb-10"
          ref={contentScroll}
        >
          <section className="welcome-row">
            <div>
              <div className="eyebrow">{t('eyebrow')}</div>
              <h1 dangerouslySetInnerHTML={{ __html: t('heroTitle') }} />
              <p className="intro">{t('heroDescription')}</p>
            </div>
            <div className="platform-pill">
              <span className="platform-glyph" aria-hidden="true">
                <ShoppingBag />
              </span>
              <span>{t('appCenter')}</span>
            </div>
          </section>

          <section
            className="app-journey grid grid-cols-1 gap-2 md:grid-cols-3"
            aria-label={t('appJourney')}
          >
            <JourneyStep
              number={t('journeyAdd')}
              title={t('journeyImport')}
              description={t('journeyImportHelp')}
              icon={<Download aria-hidden="true" />}
            />
            <JourneyStep
              number={t('journeyInstall')}
              title={t('journeyWizard')}
              description={t('journeyWizardHelp')}
              icon={<PanelTop aria-hidden="true" />}
            />
            <JourneyStep
              number={t('journeyManage')}
              title={t('journeyLaunch')}
              description={t('journeyManageHelp')}
              icon={<Grid2X2 aria-hidden="true" />}
            />
          </section>

          <section
            className={`drop-card${dragging ? ' dragging' : ''}`}
            aria-label={t('importInstaller')}
            onDragOver={(event) => {
              event.preventDefault()
              setDragging(true)
            }}
            onDragLeave={(event) => {
              if (
                !event.currentTarget.contains(
                  event.relatedTarget as Node | null
                )
              ) {
                setDragging(false)
              }
            }}
            onDrop={(event) => void onDrop(event)}
          >
            <div className="import-kind" role="group" aria-label={t('importKind')}>
              <button
                className={uploadKind === 'installer' ? 'selected' : ''}
                type="button"
                aria-pressed={uploadKind === 'installer'}
                onClick={() => setUploadKind('installer')}
              >
                {t('installMode')}
              </button>
              <button
                className={uploadKind === 'portable' ? 'selected' : ''}
                type="button"
                aria-pressed={uploadKind === 'portable'}
                onClick={() => setUploadKind('portable')}
              >
                {t('directRunMode')}
              </button>
            </div>
            <input
              ref={fileInput}
              className="visually-hidden"
              type="file"
              accept={uploadKind === 'portable'
                ? '.exe,application/vnd.microsoft.portable-executable'
                : '.exe,.msi,application/vnd.microsoft.portable-executable,application/x-msi'}
              multiple
              onChange={(event) => void onFileChange(event)}
            />
            <div className="drop-icon" aria-hidden="true">
              <Download />
            </div>
            <div className="drop-copy">
              <h2>{t('addWindowsApp')}</h2>
              <p>
                {uploadKind === 'portable' ? t('directRunHelp') : t('dropHelp')}{' '}
                {uploadKind === 'portable' ? (
                  <b>.exe</b>
                ) : (
                  <><b>.exe</b> {t('and')} <b>.msi</b></>
                )}
              </p>
            </div>
            <button
              className="primary-button"
              type="button"
              onClick={chooseFiles}
            >
              <Plus aria-hidden="true" />
              <span>{uploadKind === 'portable' ? t('chooseExe') : t('chooseInstaller')}</span>
            </button>
            <div className="drop-foot">
              <ShieldCheck className="shield-icon" aria-hidden="true" />
              <span>{t('localOnly')}</span>
            </div>
          </section>

          <section
            className="session-settings"
            aria-labelledby="session-settings-title"
          >
            <div className="session-settings-heading">
              <div>
                <h2 id="session-settings-title">{t('sessionPermissions')}</h2>
                <p>{t('sessionPermissionsHelp')}</p>
              </div>
            </div>
            <div className="session-settings-options grid grid-cols-1 gap-3 md:grid-cols-3">
              <SessionSetting
                checked={gpuAvailable && sessionPreferences.gpu}
                disabled={!gpuAvailable}
                title={t('gpuAcceleration')}
                description={
                  gpuAvailable
                    ? t('gpuPermissionHelp')
                    : windowIntegration?.gpu?.reason
                      ? t('hostUnavailableReason', {
                          reason: windowIntegration.gpu.reason
                        })
                      : t('hostUnavailable')
                }
                onChange={(checked) =>
                  void saveSessionPreference('gpu', checked)
                }
              />
              <SessionSetting
                checked={clipboardAvailable && sessionPreferences.clipboardRead}
                disabled={!clipboardAvailable}
                title={t('clipboardRead')}
                description={
                  clipboardAvailable
                    ? t('clipboardReadHelp')
                    : windowIntegration?.clipboard?.reason
                      ? t('hostUnavailableReason', {
                          reason: windowIntegration.clipboard.reason
                        })
                      : t('hostUnavailable')
                }
                onChange={(checked) =>
                  void saveSessionPreference('clipboardRead', checked)
                }
              />
              <SessionSetting
                checked={
                  clipboardAvailable && sessionPreferences.clipboardWrite
                }
                disabled={!clipboardAvailable}
                title={t('clipboardWrite')}
                description={
                  clipboardAvailable
                    ? t('clipboardWriteHelp')
                    : windowIntegration?.clipboard?.reason
                      ? t('hostUnavailableReason', {
                          reason: windowIntegration.clipboard.reason
                        })
                      : t('hostUnavailable')
                }
                onChange={(checked) =>
                  void saveSessionPreference('clipboardWrite', checked)
                }
              />
            </div>
          </section>

          <div className="section-heading" id="installed" ref={installedSection}>
            <div>
              <h2>{t('installedApps')}</h2>
              <p>{t('installedAppsHelp')}</p>
            </div>
            <span className="section-count">
              {t('appsCount', { count: apps.length })}
            </span>
          </div>
          <section
            className="installed-grid grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3"
            aria-live="polite"
          >
            {apps.length === 0 ? (
              <div className="empty-state">
                <EmptyArt />
                <h3>{t('noApps')}</h3>
                <p>{t('noAppsHelp')}</p>
                <button
                  className="text-button"
                  type="button"
                  onClick={chooseFiles}
                >
                  <span>{t('importFirstInstaller')}</span>
                  <ArrowRight className="text-button-arrow" aria-hidden="true" />
                </button>
              </div>
            ) : (
              apps.map((app) => (
                <article className="app-card" key={app.id}>
                  <div className="app-card-icon">
                    {(app.name || 'W').slice(0, 1)}
                  </div>
                  <div className="app-card-copy" title={app.lastError || ''}>
                    <strong>{app.name}</strong>
                    <span>
                      {app.lastError
                        || app.version
                        || t('appArchitecture', {
                          architecture: architectureLabel(
                            app.windowsArchitecture
                          )
                        })}
                    </span>
                  </div>
                  <div className="app-actions">
                    <button
                      className="app-action primary"
                      type="button"
                      disabled={Boolean(app.sessionId)}
                      onClick={() => void launchApp(app)}
                    >
                      {app.sessionId ? t('appRunning') : t('open')}
                    </button>
                    <button
                      className="app-action"
                      type="button"
                      disabled={
                        Boolean(app.sessionId) || !app.uninstallExecutable
                      }
                      title={
                        app.uninstallExecutable
                          ? t('uninstallTitle')
                          : t('uninstallUnsupported')
                      }
                      onClick={() => void uninstallApp(app)}
                    >
                      {t('uninstall')}
                    </button>
                  </div>
                </article>
              ))
            )}
          </section>

          <section
            className="installer-section"
            id="installers"
            ref={installersSection}
          >
            <div className="section-heading compact">
              <div>
                <h2>{t('installTasks')}</h2>
                <p>{t('installTasksHelp')}</p>
              </div>
              <span className="section-count">
                {t('tasksCount', { count: installTasks.length })}
              </span>
            </div>
            <div className="installer-list">
              {installTasks.length === 0 ? (
                <div className="installer-empty">{t('noTasks')}</div>
              ) : (
                installTasks.map((task) => {
                  const busy = BUSY_TASKS.has(task.status)
                  const portable = task.kind === 'portable'
                  const backendMessage = localizeBackendMessage(
                    task.message,
                    locale,
                    t
                  )
                  return (
                    <article className="installer-row" key={task.id}>
                      <div
                        className={`file-type ${task.extension === 'msi' ? 'msi' : 'exe'}`}
                      >
                        {task.extension.toUpperCase()}
                      </div>
                      <div className="installer-info">
                        <strong title={task.fileName}>{task.fileName}</strong>
                        <span>
                          {architectureLabel(task.windowsArchitecture)} <i>·</i>{' '}
                          {formatBytes(task.bytes)} <i>·</i>{' '}
                          {formatDate(task.addedAt, locale)}
                          {backendMessage && (
                            <>
                              {' '}
                              <i>·</i> {backendMessage}
                            </>
                          )}
                        </span>
                      </div>
                      <span className="task-badge" title={backendMessage}>
                        <i />
                        {portable && task.status === 'awaiting-user'
                          ? t('appRunning')
                          : portable && task.status === 'completed'
                            ? t('appReady')
                            : portable && task.status === 'failed'
                              ? t('appRunFailed')
                            : taskStatusLabel(task.status, t)}
                      </span>
                      {portable && !busy && (
                        <button
                          className="app-action primary"
                          type="button"
                          onClick={() => void runPortableTask(task)}
                        >
                          {t('runAgain')}
                        </button>
                      )}
                      <button
                        className="remove-button"
                        type="button"
                        aria-label={t('removeTask')}
                        title={busy ? (portable ? t('appRunning') : t('installerRunning')) : t('remove')}
                        disabled={busy}
                        onClick={() => void removeTask(task)}
                      >
                        <X aria-hidden="true" />
                      </button>
                    </article>
                  )
                })
              )}
            </div>
          </section>

          <div
            className={`notice${notice ? ` visible ${notice.kind}` : ''}`}
            role="status"
            aria-live="polite"
          >
            {notice?.message}
          </div>
          <footer className="footer">{t('footerText')}</footer>
        </div>
      </main>
    </div>
  )
}

function JourneyStep({
  number,
  title,
  description,
  icon
}: {
  number: string
  title: string
  description: string
  icon: ReactNode
}) {
  return (
    <div className="journey-step">
      <span className="journey-icon">{icon}</span>
      <span className="journey-copy">
        <small>{number}</small>
        <strong>{title}</strong>
        <span>{description}</span>
      </span>
    </div>
  )
}

function SessionSetting({
  checked,
  disabled,
  title,
  description,
  onChange
}: {
  checked: boolean
  disabled: boolean
  title: string
  description: string
  onChange: (checked: boolean) => void
}) {
  return (
    <label className={`session-setting${disabled ? ' disabled' : ''}`}>
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(event) => onChange(event.currentTarget.checked)}
      />
      <span>
        <strong>{title}</strong>
        <small>{description}</small>
      </span>
    </label>
  )
}

function EmptyArt() {
  return (
    <div className="empty-art" aria-hidden="true">
      <span className="empty-window back" />
      <span className="empty-window front">
        <i />
        <i />
        <i />
        <b />
      </span>
    </div>
  )
}
