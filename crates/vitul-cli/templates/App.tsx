import {
  useEffect,
  useState,
  useSyncExternalStore,
  type FormEvent
} from 'react'
import { getHostContext, subscribeHostContext } from './host-context'
import { messages } from './messages'
import { callOpenTool } from './opentool'
import { createHostProvider, type HostProvider } from './host'
import { Button } from './components/ui/Button'
import { Card } from './components/ui/Card'
import { TextInput } from './components/ui/Input'

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

export default function App() {
  const context = useSyncExternalStore(subscribeHostContext, getHostContext)
  const t = messages[context.locale]
  const [host, setHost] = useState<HostProvider | null>(null)
  const [hostError, setHostError] = useState<string | null>(null)
  const [name, setName] = useState('')
  const [greeting, setGreeting] = useState<string | null>(null)
  const [greetingError, setGreetingError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [windowStatus, setWindowStatus] = useState<
    'windowInitial' | 'windowSending' | 'windowDone'
  >('windowInitial')
  const [windowError, setWindowError] = useState<string | null>(null)

  useEffect(() => {
    let active = true
    void createHostProvider().then(
      (provider) => {
        if (active) setHost(provider)
      },
      (error: unknown) => {
        if (active) setHostError(errorMessage(error))
      }
    )
    return () => {
      active = false
    }
  }, [])

  async function submitGreeting(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const trimmedName = name.trim()
    if (!trimmedName) return

    setBusy(true)
    setGreetingError(null)
    try {
      const result = await callOpenTool<{ message: string }>('greet', {
        name: trimmedName
      })
      setGreeting(result.message)
    } catch (error) {
      setGreetingError(errorMessage(error))
    } finally {
      setBusy(false)
    }
  }

  async function minimizeWindow() {
    if (!host) return
    setWindowError(null)
    setWindowStatus('windowSending')
    try {
      await host.window.minimize()
      setWindowStatus('windowDone')
    } catch (error) {
      setWindowError(errorMessage(error))
    }
  }

  const hostLabel = hostError
    ? t.hostUnavailable
    : host
      ? t.hostConnected.replace('{platform}', host.platform)
      : t.hostConnecting

  return (
    <main className="min-h-screen overflow-hidden bg-[var(--page)] text-[var(--text)] selection:bg-cyan-300/30">
      <div className="pointer-events-none fixed inset-0 bg-[radial-gradient(ellipse_at_50%_-20%,rgba(34,211,238,0.13),transparent_55%)]" />
      <div className="relative mx-auto flex min-h-screen w-full max-w-6xl flex-col px-6 py-7 sm:px-10 sm:py-10">
        <header className="flex items-center justify-between">
          <a
            className="flex items-center gap-3 text-sm font-semibold tracking-wide text-[var(--text)]"
            href="#"
          >
            <span className="grid size-9 place-items-center rounded-xl border border-[var(--border)] bg-[var(--surface)] text-[var(--accent)]">
              <svg
                aria-hidden="true"
                viewBox="0 0 24 24"
                className="size-5"
                fill="none"
              >
                <path
                  d="M4 17.5 8.1 6.8l3.8 10.7 4-10.7 4.1 10.7"
                  stroke="currentColor"
                  strokeWidth="1.8"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              </svg>
            </span>
            <span>
              Vitul{' '}
              <span className="font-normal text-[var(--muted)]">
                / {t.starter}
              </span>
            </span>
          </a>
          <span className="hidden items-center gap-2 rounded-full border border-[var(--border)] bg-[var(--surface)] px-3 py-1.5 text-xs text-[var(--muted)] sm:flex">
            <span className="size-1.5 rounded-full bg-emerald-400 shadow-[0_0_12px_rgba(52,211,153,0.8)]" />
            {t.ready}
          </span>
        </header>

        <section className="grid flex-1 content-center gap-12 py-16 lg:grid-cols-[1.05fr_0.95fr] lg:gap-20 lg:py-20">
          <div className="flex flex-col justify-center">
            <p className="mb-5 flex items-center gap-2 text-xs font-semibold uppercase tracking-[0.22em] text-[var(--accent)]">
              <span className="h-px w-7 bg-cyan-300/60" />
              {t.eyebrow}
            </p>
            <h1 className="max-w-xl text-5xl font-semibold leading-[1.06] tracking-[-0.055em] text-[var(--text)] sm:text-6xl">
              {t.headline}
              <span className="block bg-gradient-to-r from-[var(--accent)] via-[var(--accent)] to-[var(--badge)] bg-clip-text pb-2 text-transparent">
                {t.headlineAccent}
              </span>
            </h1>
            <p className="mt-6 max-w-lg text-base leading-7 text-[var(--muted)]">
              {t.intro}
            </p>
            <div className="mt-8 flex flex-wrap gap-2 text-xs text-[var(--muted)]">
              {[
                ['01', 'React + TypeScript'],
                ['02', 'Tailwind CSS'],
                ['03', t.backend]
              ].map(([number, label]) => (
                <span
                  key={number}
                  className="inline-flex items-center gap-2 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 py-2"
                >
                  <span className="font-mono text-[var(--accent)]">
                    {number}
                  </span>
                  {label}
                </span>
              ))}
            </div>
          </div>

          <div className="grid content-center gap-4">
            <Card className="p-6 sm:p-7">
              <div className="mb-6 flex items-start justify-between gap-4">
                <div>
                  <p className="text-xs font-medium uppercase tracking-[0.16em] text-[var(--muted)]">
                    {t.business}
                  </p>
                  <h2 className="mt-2 text-lg font-semibold text-[var(--text)]">
                    {t.callBackend}
                  </h2>
                </div>
                <span className="rounded-lg border border-indigo-300/15 bg-indigo-300/[0.08] px-2.5 py-1.5 font-mono text-[11px] text-[var(--badge)]">
                  OpenTool
                </span>
              </div>
              <form
                className="flex flex-col gap-3 sm:flex-row"
                onSubmit={submitGreeting}
              >
                <label className="sr-only" htmlFor="greet-name">
                  {t.name}
                </label>
                <TextInput
                  id="greet-name"
                  autoComplete="name"
                  placeholder={t.namePlaceholder}
                  value={name}
                  onChange={(event) => setName(event.currentTarget.value)}
                  required
                />
                <Button type="submit" disabled={busy || !name.trim()}>
                  {busy ? t.calling : t.sayHello}
                  {!busy && <span aria-hidden="true">↗</span>}
                </Button>
              </form>
              <div
                className={`mt-4 rounded-xl border px-4 py-3 text-sm ${greetingError ? 'border-rose-300/15 bg-rose-300/[0.06] text-[var(--danger)]' : 'border-[var(--border)] bg-[var(--inset)] text-[var(--text)]'}`}
                aria-live="polite"
              >
                {greetingError ?? greeting ?? t.greetingInitial}
              </div>
              <p className="mt-3 font-mono text-[11px] text-[var(--muted)]">
                callOpenTool("greet", &#123; name &#125;)
              </p>
            </Card>

            <Card className="p-6 sm:p-7">
              <div className="mb-5 flex items-center justify-between gap-4">
                <div>
                  <p className="text-xs font-medium uppercase tracking-[0.16em] text-[var(--muted)]">
                    {t.system}
                  </p>
                  <h2 className="mt-2 text-lg font-semibold text-[var(--text)]">
                    {t.talkHost}
                  </h2>
                </div>
                <span className="rounded-lg border border-emerald-300/15 bg-emerald-300/[0.08] px-2.5 py-1.5 font-mono text-[11px] text-[var(--success)]">
                  Host API
                </span>
              </div>
              <div className="flex items-center gap-3 rounded-xl border border-[var(--border)] bg-[var(--inset)] px-4 py-3">
                <span
                  className={`size-2 rounded-full ${hostError ? 'bg-rose-400' : host ? 'bg-emerald-400' : 'animate-pulse bg-amber-300'}`}
                />
                <span className="text-sm text-[var(--text)]" role="status">
                  {hostError ?? hostLabel}
                </span>
              </div>
              <div className="mt-4 flex flex-wrap items-center gap-3">
                <Button
                  variant="secondary"
                  onClick={minimizeWindow}
                  disabled={!host}
                >
                  {t.minimize}
                </Button>
                <p
                  className="min-w-0 flex-1 text-xs leading-5 text-[var(--muted)]"
                  aria-live="polite"
                >
                  {windowError ?? t[windowStatus]}
                </p>
              </div>
            </Card>
          </div>
        </section>

        <footer className="flex flex-col gap-2 border-t border-[var(--border)] pt-5 text-xs text-[var(--muted)] sm:flex-row sm:items-center sm:justify-between">
          <span>{t.builtWith}</span>
          <span>{t.footer}</span>
        </footer>
      </div>
    </main>
  )
}
