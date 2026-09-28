// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const { run, runVitul, logError } = require('./index')

function runWith(runFn, args, binName) {
  return new Promise((resolve, reject) => {
    const callback = (error, res) => {
      if (error) {
        reject(error)
      } else {
        resolve(res)
      }
    }
    if (binName === undefined) {
      runFn(args, callback)
    } else {
      runFn(args, binName, callback)
    }
  })
}

module.exports.run = (args, binName) => runWith(run, args, binName)
module.exports.runVitul = (args) => runWith(runVitul, args)
module.exports.logError = logError
