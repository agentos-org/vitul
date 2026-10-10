// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

import { resolve } from 'node:path'
import { spawnSync } from 'node:child_process'
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync
} from 'node:fs'
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
  it('creates an OpenTool-first Vitul project', { timeout: 15 * 60 * 1000 }, async () => {
    const cwd = process.cwd()
    const testRoot = mkdtempSync(resolve(tmpdir(), 'vitul-cli-test-'))
    try {
      process.chdir(testRoot)

      const cli = await import('../main.js')
      await cli.runVitul(['--version'])
      await cli.runVitul([
        'create',
        'vitul-demo',
        '--manager',
        'pnpm',
        '--template',
        'vanilla-ts',
        '--yes',
        '--identifier',
        'com.vitul.demo'
      ])

      const projectRoot = resolve(testRoot, 'vitul-demo')
      const projectConfig = JSON.parse(
        readFileSync(resolve(projectRoot, 'vitul.conf.json'), 'utf8')
      )
      expect(projectConfig.schemaVersion).toBe(2)
      expect(existsSync(resolve(projectRoot, 'backend/Cargo.toml'))).toBe(true)
      expect(existsSync(resolve(projectRoot, 'src-vitul/src/lib.rs'))).toBe(true)
    } finally {
      process.chdir(cwd)
      rmSync(testRoot, { recursive: true, force: true })
    }
  })
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
