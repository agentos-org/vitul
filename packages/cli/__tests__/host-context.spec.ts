// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

import { afterEach, describe, expect, it, vi } from 'vitest'

async function hostContext(search = '', language = 'en-US', dark = false) {
  vi.resetModules()
  const parent = new EventTarget()
  const window = Object.assign(new EventTarget(), { parent })
  const media = Object.assign(new EventTarget(), { matches: dark })
  const navigator = { language }
  const root = { lang: '', dataset: { theme: '' }, style: { colorScheme: '' } }
  const meta = { setAttribute: vi.fn() }
  vi.stubGlobal('window', window)
  vi.stubGlobal('location', new URL(`https://app.example/${search}`))
  vi.stubGlobal('navigator', navigator)
  vi.stubGlobal('matchMedia', () => media)
  vi.stubGlobal('document', {
    documentElement: root,
    querySelector: () => meta
  })
  const context =
    await import('../../../crates/vitul-cli/templates/host-context')
  const message = (
    data: unknown,
    source: unknown = parent,
    origin = 'https://shell.example'
  ) => {
    window.dispatchEvent(
      Object.assign(new Event('message'), { data, source, origin })
    )
  }
  return { ...context, window, parent, media, navigator, root, meta, message }
}

afterEach(() => vi.unstubAllGlobals())

describe('generated host context', () => {
  it('initializes locale and appearance from the host URL and applies them to the document', async () => {
    const host = await hostContext(
      '?runtime=agentos&hostOrigin=https://shell.example&locale=zh&appearance=dark'
    )
    expect(host.getHostContext()).toEqual({ locale: 'zh-CN', theme: 'dark' })
    expect(host.root).toEqual({
      lang: 'zh-CN',
      dataset: { theme: 'dark' },
      style: { colorScheme: 'dark' }
    })
    expect(host.meta.setAttribute).toHaveBeenCalledWith('content', '#0b0d12')
  })

  it('synchronizes trusted shell updates and ignores messages from other origins or windows', async () => {
    const host = await hostContext('?hostOrigin=https://shell.example')
    const changed = vi.fn()
    const unsubscribe = host.subscribeHostContext(changed)
    host.message(
      { type: 'shell:theme', appearance: 'dark' },
      host.parent,
      'https://untrusted.example'
    )
    host.message({ type: 'shell:locale', locale: 'zh-CN' }, new EventTarget())
    expect(changed).not.toHaveBeenCalled()
    host.message({ type: 'shell:theme', appearance: 'dark' })
    host.message({ type: 'shell:locale', locale: 'zh-CN' })
    expect(host.getHostContext()).toEqual({ locale: 'zh-CN', theme: 'dark' })
    expect(changed).toHaveBeenCalledTimes(2)
    host.message({ type: 'shell:locale', locale: 'zh-CN' })
    expect(changed).toHaveBeenCalledTimes(2)
    unsubscribe()
  })

  it('accepts SDK context changes and rejects unsupported schemas and invalid values', async () => {
    const host = await hostContext('?runtime=agentos')
    const unsubscribe = host.subscribeHostContext(vi.fn())
    const update = {
      channel: 'agentos:host',
      type: 'context-changed',
      runtime: 'agentos',
      locale: 'zh-CN',
      theme: 'dark'
    }
    host.message(
      { ...update, schemaVersion: 2 },
      host.window,
      'https://app.example'
    )
    expect(host.getHostContext()).toEqual({ locale: 'en-US', theme: 'light' })
    host.message(
      { ...update, schemaVersion: 1 },
      host.window,
      'https://app.example'
    )
    expect(host.getHostContext()).toEqual({ locale: 'zh-CN', theme: 'dark' })
    host.message(
      { ...update, locale: 'invalid', theme: 'invalid' },
      host.window,
      'https://app.example'
    )
    expect(host.getHostContext()).toEqual({ locale: 'zh-CN', theme: 'dark' })
    unsubscribe()
  })

  it('follows browser preferences outside AgentOS and cleans up subscriptions', async () => {
    const host = await hostContext('?locale=zh-CN&theme=dark')
    expect(host.getHostContext()).toEqual({ locale: 'en-US', theme: 'light' })
    const changed = vi.fn()
    const unsubscribe = host.subscribeHostContext(changed)
    host.navigator.language = 'zh-TW'
    host.window.dispatchEvent(new Event('languagechange'))
    host.media.matches = true
    host.media.dispatchEvent(
      Object.assign(new Event('change'), { matches: true })
    )
    expect(host.getHostContext()).toEqual({ locale: 'zh-CN', theme: 'dark' })
    expect(changed).toHaveBeenCalledTimes(2)
    unsubscribe()
    host.navigator.language = 'en-US'
    host.window.dispatchEvent(new Event('languagechange'))
    expect(changed).toHaveBeenCalledTimes(2)
    const unsubscribeAgain = host.subscribeHostContext(changed)
    expect(host.getHostContext()).toEqual({ locale: 'en-US', theme: 'dark' })
    unsubscribeAgain()
  })

  it('keeps host preferences when browser preferences change', async () => {
    const host = await hostContext('?runtime=agentos&locale=zh-CN&theme=dark')
    const unsubscribe = host.subscribeHostContext(vi.fn())
    host.window.dispatchEvent(new Event('languagechange'))
    host.media.dispatchEvent(
      Object.assign(new Event('change'), { matches: false })
    )
    expect(host.getHostContext()).toEqual({ locale: 'zh-CN', theme: 'dark' })
    unsubscribe()
  })
})
