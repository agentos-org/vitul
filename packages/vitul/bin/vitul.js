#!/usr/bin/env node

const cli = require('@litevar/vitul-cli')

cli.runVitul(process.argv.slice(2)).catch((error) => {
  cli.logError(error.message)
  process.exitCode = 1
})
