#!/usr/bin/env node

// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const cli = require('./main')

const [, , ...args] = process.argv

cli.runVitul(args).catch((err) => {
  cli.logError(err.message)
  process.exit(1)
})
