// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

import { resolve } from 'node:path'
import { spawnSync } from 'node:child_process'
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { beforeAll, describe, expect, it } from 'vitest'

// Build CLI before tests, for local usage only.
// CI builds the CLI on different platforms and architectures
if (!process.env.CI) {
  beforeAll(() => {
    const cliDir = resolve(__dirname, '..')
    exec('pnpm', ['build:debug'], { cwd: cliDir })
  })
}

describe('[CLI] @litevar/vitul-cli', () => {
  it(
    'creates an OpenTool-first project with the preset React and pnpm stack',
    { timeout: 15 * 60 * 1000 },
    async () => {
      const cwd = process.cwd()
      const testRoot = mkdtempSync(resolve(tmpdir(), 'vitul-cli-test-'))
      try {
        process.chdir(testRoot)

        const cli = await import('../main.js')
        await cli.runVitul(['--version'])
        await cli.runVitul([
          'create',
          '.',
          '--yes',
          '--identifier',
          'com.litevar.agentos.timeMemo',
          '--product-name',
          'Time Memo',
          '--window-title',
          'Time Memo Desktop'
        ])

        const projectRoot = testRoot
        const projectConfig = JSON.parse(
          readFileSync(resolve(projectRoot, 'vitul.conf.json'), 'utf8')
        )
        const packageJson = JSON.parse(
          readFileSync(resolve(projectRoot, 'package.json'), 'utf8')
        )
        const cargoToml = readFileSync(
          resolve(projectRoot, 'src-vitul/Cargo.toml'),
          'utf8'
        )
        const runtimeBuildRs = readFileSync(
          resolve(projectRoot, 'src-vitul/build.rs'),
          'utf8'
        )
        const runtimeLibRs = readFileSync(
          resolve(projectRoot, 'src-vitul/src/lib.rs'),
          'utf8'
        )
        const backendMain = readFileSync(
          resolve(projectRoot, 'backend/src/main.rs'),
          'utf8'
        )

        expect(projectConfig.schemaVersion).toBe(2)
        expect(projectConfig.identifier).toBe('com.litevar.agentos.timeMemo')
        expect(projectConfig.productName).toBe('Time Memo')
        expect(projectConfig.app.windows[0].title).toBe('Time Memo Desktop')
        expect(projectConfig.backend.manifest).toBe('backend/Cargo.toml')
        expect(projectConfig.agentos.targets).toContain('agentos/arm64')
        expect(packageJson.name).not.toBe('vitul-app')
        expect(packageJson.name).toMatch(/^[a-z][a-z0-9_-]*$/)
        expect(packageJson.dependencies['agentos-sdk-ts']).toBe('^2.0.0')
        expect(packageJson.dependencies.react).toBe('^19.1.0')
        expect(packageJson.devDependencies['@vitejs/plugin-react']).toBe(
          '^6.0.2'
        )
        expect(packageJson.devDependencies.tailwindcss).toBe('^4.3.3')
        expect(packageJson.packageManager).toBe('pnpm@12.4.2')
        expect(packageJson.scripts.vitul).toBe('vitul')
        expect(existsSync(resolve(projectRoot, 'src/App.tsx'))).toBe(true)
        expect(cargoToml).toContain(`name = "${packageJson.name}"`)
        expect(existsSync(resolve(projectRoot, 'backend/Cargo.toml'))).toBe(
          true
        )
        expect(existsSync(resolve(projectRoot, 'src-vitul/src/lib.rs'))).toBe(
          true
        )
        expect(
          existsSync(resolve(projectRoot, 'src-vitul/vitul.conf.json'))
        ).toBe(false)
        expect(
          existsSync(resolve(projectRoot, 'src-vitul/tauri.conf.json'))
        ).toBe(false)
        expect(runtimeBuildRs).toContain(
          'fs::copy(vitul_config, desktop_config)'
        )
        expect(runtimeBuildRs).toContain('vitul_build::build()')
        expect(cargoToml).toContain('vitul-build = { package = "tauri-build"')
        expect(runtimeBuildRs).toContain(
          'failed to prepare the generated Vitul runtime config'
        )
        expect(runtimeLibRs).toContain('error while building Vitul application')
        expect(backendMain).toContain(
          'Box<dyn std::error::Error + Send + Sync>'
        )
        expect(backendMain).not.toContain('serde_json::{json, Value}')
        expect(backendMain).toContain('VITUL_DEV_ORIGIN')
        expect(backendMain).toContain('#[cfg(debug_assertions)]')
        expect(backendMain).toContain(
          'development_opentool_rpc_and_cors_work_on_http'
        )
      } finally {
        process.chdir(cwd)
        rmSync(testRoot, { recursive: true, force: true })
      }
    }
  )
})

function exec(
  bin: string,
  args?: string[],
  opts?: {
    cwd?: string
  }
) {
  process.platform === 'win32'
    ? spawnSync('cmd', ['/c', bin, ...(args ?? [])], { cwd: opts?.cwd })
    : spawnSync(bin, args, { cwd: opts?.cwd })
}
