// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const { run, runVitul, logError } = require('./index')

function runWith(runFn, args, ...nativeArgs) {
  return new Promise((resolve, reject) => {
    const callback = (error, res) => {
      if (error) {
        reject(error)
      } else {
        resolve(res)
      }
    }
    runFn(args, ...nativeArgs, callback)
  })
}

module.exports.run = (args, binName) => runWith(run, args, binName ?? null)
module.exports.runVitul = (args) => runWith(runVitul, args)
module.exports.logError = logError
