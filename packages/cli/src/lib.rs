// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

#![cfg(any(target_os = "macos", target_os = "linux", windows))]

use std::{ffi::OsString, sync::Arc};

use napi::{
  Error, Result, Status,
  threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode},
};

#[napi_derive::napi]
pub fn run(
  args: Vec<String>,
  bin_name: Option<String>,
  callback: Arc<ThreadsafeFunction<bool>>,
) -> Result<()> {
  let args = args.into_iter().map(OsString::from).collect();
  spawn_cli_run(callback, "Vitul desktop CLI", move || {
    vitul_cli::run_compat_with_args(args, bin_name)
  })
}

#[napi_derive::napi]
pub fn run_vitul(args: Vec<String>, callback: Arc<ThreadsafeFunction<bool>>) -> Result<()> {
  let args = args.into_iter().map(OsString::from).collect();
  spawn_cli_run(callback, "Vitul CLI", move || {
    vitul_cli::run_vitul_with_args(args)
  })
}

fn spawn_cli_run<F>(
  callback: Arc<ThreadsafeFunction<bool>>,
  cli_name: &'static str,
  run: F,
) -> Result<()>
where
  F: FnOnce() -> anyhow::Result<()> + Send + 'static,
{
  // Run outside the Node.js thread so the package manager remains responsive
  // while Vitul or a compatible desktop command is running.
  std::thread::spawn(move || {
    let result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      run().inspect_err(|error| eprintln!("{error:#}"))
    })) {
      Ok(result) => result,
      Err(_) => {
        return callback.call(
          Err(Error::new(
            Status::GenericFailure,
            format!("{cli_name} unexpected panic"),
          )),
          ThreadsafeFunctionCallMode::Blocking,
        );
      }
    };

    match result {
      Ok(()) => callback.call(Ok(true), ThreadsafeFunctionCallMode::Blocking),
      Err(error) => callback.call(
        Err(Error::new(Status::GenericFailure, format!("{error:#}"))),
        ThreadsafeFunctionCallMode::Blocking,
      ),
    }
  });

  Ok(())
}

#[napi_derive::napi]
pub fn log_error(error: String) {
  log::error!("{}", error);
}
