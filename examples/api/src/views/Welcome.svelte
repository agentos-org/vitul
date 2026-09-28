<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import {
    getName,
    getVersion,
    getTauriVersion,
    getBundleType,
    exit
  } from '@tauri-apps/api/app'
  import type { ViewProps } from '../App.svelte'
  import { callOpenTool } from '../opentool'

  let { onMessage }: ViewProps = $props()

  let version = $state('1.0.0')
  let runtimeVersion = $state('1.0.0')
  let appName = $state('Unknown')
  let bundleType = $state('Unknown')
  let name = $state('Vitul developer')
  let openToolResult = $state('Try the business call below.')
  let openToolBusy = $state(false)

  getName().then((n) => {
    appName = n
  })
  getVersion().then((v) => {
    version = v
  })
  getTauriVersion().then((v) => {
    runtimeVersion = v
  })
  getBundleType().then((b) => {
    if (b) {
      bundleType = b
    }
  })

  function contextMenu() {
    invoke('plugin:app-menu|popup')
  }

  function exitApp() {
    exit().catch(onMessage)
  }

  async function greet(event: SubmitEvent) {
    event.preventDefault()
    openToolBusy = true
    openToolResult = 'Calling the OpenTool backend…'
    try {
      const result = await callOpenTool<{ message: string }>('greet', { name })
      openToolResult = result.message
    } catch (error) {
      openToolResult = error instanceof Error ? error.message : String(error)
    } finally {
      openToolBusy = false
    }
  }

  function minimizeWindow() {
    getCurrentWindow().minimize().catch(onMessage)
  }
</script>

<div class="grid gap-8 justify-items-start">
  <p>
    This Vitul example separates business operations and desktop controls:
    greet requests use OpenTool JSON-RPC, while window controls use the system IPC
    API.
  </p>
  <pre>
    App name: <code>{appName}</code>
    App version: <code>{version}</code>
    Desktop runtime: <code>{runtimeVersion}</code>
    Bundle type: <code>{bundleType}</code>
  </pre>

  <div class="flex gap-2">
    <button class="btn" onclick={contextMenu}>Context menu</button>
    <button class="btn" onclick={minimizeWindow}>Minimize window (system IPC)</button>
    <button class="btn" id="exit" onclick={exitApp}>Exit</button>
  </div>

  <section class="grid gap-3 justify-items-start rounded-lg border border-primaryBorder p-4">
    <h2>Business call: OpenTool</h2>
    <form class="flex gap-2" onsubmit={greet}>
      <input aria-label="Name" bind:value={name} placeholder="Enter a name" />
      <button class="btn" type="submit" disabled={openToolBusy}>
        {openToolBusy ? 'Calling…' : 'Call greet'}
      </button>
    </form>
    <p aria-live="polite">{openToolResult}</p>
    <p class="text-sm opacity-70">
      The request goes to <code>/opentool/call</code> with a JSON-RPC 2.0.0
      envelope. Business logic is not sent through a desktop command.
    </p>
  </section>
</div>
