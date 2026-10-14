import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { stripTypeScriptTypes } from 'node:module'
import { test } from 'node:test'
import vm from 'node:vm'

const template = new URL('../templates/', import.meta.url)
const source = stripTypeScriptTypes(
  readFileSync(new URL('host-context.ts', template), 'utf8')
).replaceAll('export ', '')
const html = readFileSync(new URL('index.html', template), 'utf8')
const firstPaint = html.match(/<script>([\s\S]*?)<\/script>/)[1]

function boot(search = '', dark = true, language = 'en-US', framed = false) {
  const events = new Map()
  const mediaEvents = new Map()
  const root = { lang: 'en', dataset: {}, style: {} }
  const meta = {
    content: '',
    setAttribute(_, value) {
      this.content = value
    }
  }
  const media = {
    matches: dark,
    addEventListener: (type, fn) => mediaEvents.set(type, fn),
    removeEventListener: (type) => mediaEvents.delete(type)
  }
  const window = {
    addEventListener: (type, fn) => events.set(type, fn),
    removeEventListener: (type) => events.delete(type)
  }
  window.parent = framed ? {} : window
  const navigator = { language }
  const sandbox = vm.createContext({
    window,
    navigator,
    document: { documentElement: root, querySelector: () => meta },
    location: { search, origin: 'https://app.test' },
    URL,
    URLSearchParams,
    matchMedia: () => media
  })
  vm.runInContext(firstPaint, sandbox)
  const painted = { locale: root.lang, theme: root.dataset.theme }
  vm.runInContext(
    source + '\nglobalThis.store = { getHostContext, subscribeHostContext }',
    sandbox
  )
  const changes = []
  const stop = sandbox.store.subscribeHostContext(() =>
    changes.push(sandbox.store.getHostContext())
  )
  return {
    painted,
    root,
    meta,
    events,
    mediaEvents,
    window,
    navigator,
    changes,
    stop,
    subscribe: sandbox.store.subscribeHostContext,
    snapshot: () => JSON.parse(JSON.stringify(sandbox.store.getHostContext())),
    message: (data, source = window, origin = 'https://app.test') =>
      events.get('message')?.({ data, source, origin })
  }
}

test('host URL sets first paint and live native context overrides browser preferences', () => {
  const app = boot('?runtime=agentos&locale=zh&appearance=system&theme=light')
  assert.deepEqual(app.painted, { locale: 'zh-CN', theme: 'light' })
  assert.deepEqual(app.snapshot(), app.painted)
  const data = {
    channel: 'agentos:host',
    type: 'context-changed',
    runtime: 'agentos',
    schemaVersion: 1,
    locale: 'en-US',
    theme: 'dark'
  }
  app.message(data)
  assert.deepEqual(app.snapshot(), { locale: 'en-US', theme: 'dark' })
  app.mediaEvents.get('change')({ matches: false })
  assert.equal(app.snapshot().theme, 'dark')
  app.message({ ...data, schemaVersion: 2, theme: 'light' })
  app.message({ ...data, theme: 'light' }, {})
  assert.equal(app.snapshot().theme, 'dark')
  app.message({
    ...data,
    schemaVersion: undefined,
    locale: 'zh',
    theme: 'light'
  })
  assert.equal(app.snapshot().locale, 'zh-CN')
  app.stop()
})

test('WebShell initializes without runtime and checks parent source and origin', () => {
  const app = boot(
    '?hostOrigin=https%3A%2F%2Fshell.test&locale=en&theme=light',
    true,
    'zh-CN',
    true
  )
  assert.deepEqual(app.painted, { locale: 'en-US', theme: 'light' })
  app.message(
    { type: 'shell:locale', locale: 'zh' },
    app.window.parent,
    'https://evil.test'
  )
  app.message({ type: 'shell:locale', locale: 'zh' })
  assert.equal(app.snapshot().locale, 'en-US')
  app.message(
    { type: 'shell:locale', locale: 'zh' },
    app.window.parent,
    'https://shell.test'
  )
  app.message(
    { type: 'shell:theme', appearance: 'dark' },
    app.window.parent,
    'https://shell.test'
  )
  assert.deepEqual(app.snapshot(), { locale: 'zh-CN', theme: 'dark' })
  app.message(
    { type: 'shell:theme', appearance: 'system' },
    app.window.parent,
    'https://shell.test'
  )
  assert.equal(app.snapshot().theme, 'dark')
  app.stop()
})

test('browser changes update context and subscriptions clean up and reconnect', () => {
  const app = boot('?locale=en&theme=light', true, 'zh-CN')
  assert.deepEqual(app.painted, { locale: 'zh-CN', theme: 'dark' })
  app.mediaEvents.get('change')({ matches: false })
  app.navigator.language = 'en-GB'
  app.events.get('languagechange')()
  assert.deepEqual(app.snapshot(), { locale: 'en-US', theme: 'light' })
  assert.equal(app.meta.content, '#f5f7fa')
  assert.equal(app.changes.length, 2)
  app.stop()
  assert.equal(app.events.size, 0)
  assert.equal(app.mediaEvents.size, 0)
  const unsubscribe = app.subscribe(() => {})
  assert.equal(app.events.size, 2)
  unsubscribe()
  assert.equal(app.events.size, 0)
})

test('explicit appearance fallback agrees between first paint and store', () => {
  const app = boot(
    '?runtime=agentos&theme=invalid&appearance=light&locale=invalid',
    true,
    'zh-TW'
  )
  assert.deepEqual(app.painted, { locale: 'zh-CN', theme: 'light' })
  assert.deepEqual(app.snapshot(), app.painted)
  app.message({
    channel: 'agentos:host',
    type: 'context-changed',
    runtime: 'agentos',
    locale: null,
    theme: 'system'
  })
  assert.deepEqual(app.snapshot(), app.painted)
  app.stop()
})

test('parent messages use same-origin fallback and reject invalid host origins', () => {
  const app = boot('', false, 'en-US', true)
  app.message(
    { type: 'shell:locale', locale: 'zh' },
    app.window.parent,
    'https://evil.test'
  )
  assert.equal(app.snapshot().locale, 'en-US')
  app.message({ type: 'shell:locale', locale: 'zh' }, app.window.parent)
  assert.equal(app.snapshot().locale, 'zh-CN')
  app.stop()
  const invalid = boot('?hostOrigin=invalid', false, 'en-US', true)
  invalid.message({ type: 'shell:locale', locale: 'zh' }, invalid.window.parent)
  assert.equal(invalid.snapshot().locale, 'en-US')
  invalid.stop()
})
