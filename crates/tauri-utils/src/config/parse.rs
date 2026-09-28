// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use crate::config::Config;
use crate::platform::Target;
use json_patch::merge;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// All extensions that are possibly supported, but perhaps not enabled.
pub const EXTENSIONS_SUPPORTED: &[&str] = &["json", "json5", "toml"];

/// All configuration formats that are possibly supported, but perhaps not enabled.
pub const SUPPORTED_FORMATS: &[ConfigFormat] =
  &[ConfigFormat::Json, ConfigFormat::Json5, ConfigFormat::Toml];

/// All configuration formats that are currently enabled.
pub const ENABLED_FORMATS: &[ConfigFormat] = &[
  ConfigFormat::Json,
  #[cfg(feature = "config-json5")]
  ConfigFormat::Json5,
  #[cfg(feature = "config-toml")]
  ConfigFormat::Toml,
];

/// The available configuration formats.
#[derive(Debug, Copy, Clone)]
pub enum ConfigFormat {
  /// The default JSON (vitul.conf.json) format.
  Json,
  /// The JSON5 (vitul.conf.json5) format.
  Json5,
  /// The TOML (Vitul.toml file) format.
  Toml,
}

impl ConfigFormat {
  /// Maps the config format to its file name.
  pub fn into_file_name(self) -> &'static str {
    match self {
      Self::Json => "vitul.conf.json",
      Self::Json5 => "vitul.conf.json5",
      Self::Toml => "Vitul.toml",
    }
  }

  fn into_legacy_file_name(self) -> &'static str {
    match self {
      Self::Json => "tauri.conf.json",
      Self::Json5 => "tauri.conf.json5",
      Self::Toml => "Tauri.toml",
    }
  }

  fn into_platform_file_name(self, target: Target) -> &'static str {
    match self {
      Self::Json => match target {
        Target::MacOS => "vitul.macos.conf.json",
        Target::Windows => "vitul.windows.conf.json",
        Target::Linux => "vitul.linux.conf.json",
        Target::Android => "vitul.android.conf.json",
        Target::Ios => "vitul.ios.conf.json",
      },
      Self::Json5 => match target {
        Target::MacOS => "vitul.macos.conf.json5",
        Target::Windows => "vitul.windows.conf.json5",
        Target::Linux => "vitul.linux.conf.json5",
        Target::Android => "vitul.android.conf.json5",
        Target::Ios => "vitul.ios.conf.json5",
      },
      Self::Toml => match target {
        Target::MacOS => "Vitul.macos.toml",
        Target::Windows => "Vitul.windows.toml",
        Target::Linux => "Vitul.linux.toml",
        Target::Android => "Vitul.android.toml",
        Target::Ios => "Vitul.ios.toml",
      },
    }
  }

  fn into_legacy_platform_file_name(self, target: Target) -> &'static str {
    match self {
      Self::Json => match target {
        Target::MacOS => "tauri.macos.conf.json",
        Target::Windows => "tauri.windows.conf.json",
        Target::Linux => "tauri.linux.conf.json",
        Target::Android => "tauri.android.conf.json",
        Target::Ios => "tauri.ios.conf.json",
      },
      Self::Json5 => match target {
        Target::MacOS => "tauri.macos.conf.json5",
        Target::Windows => "tauri.windows.conf.json5",
        Target::Linux => "tauri.linux.conf.json5",
        Target::Android => "tauri.android.conf.json5",
        Target::Ios => "tauri.ios.conf.json5",
      },
      Self::Toml => match target {
        Target::MacOS => "Tauri.macos.toml",
        Target::Windows => "Tauri.windows.toml",
        Target::Linux => "Tauri.linux.toml",
        Target::Android => "Tauri.android.toml",
        Target::Ios => "Tauri.ios.toml",
      },
    }
  }
}

/// Represents all the errors that can happen while reading the config.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConfigError {
  /// Failed to parse from JSON.
  #[error("unable to parse JSON Vitul config file at {path} because {error}")]
  FormatJson {
    /// The path that failed to parse into JSON.
    path: PathBuf,

    /// The parsing [`serde_json::Error`].
    error: serde_json::Error,
  },

  /// Failed to parse from JSON5.
  #[cfg(feature = "config-json5")]
  #[error("unable to parse JSON5 Vitul config file at {path} because {error}")]
  FormatJson5 {
    /// The path that failed to parse into JSON5.
    path: PathBuf,

    /// The parsing [`json5::Error`].
    error: ::json5::Error,
  },

  /// Failed to parse from TOML.
  #[cfg(feature = "config-toml")]
  #[error("unable to parse toml Vitul config file at {path} because {error}")]
  FormatToml {
    /// The path that failed to parse into TOML.
    path: PathBuf,

    /// The parsing [`toml::Error`].
    error: Box<::toml::de::Error>,
  },

  /// Unknown config file name encountered.
  #[error("unsupported format encountered {0}")]
  UnsupportedFormat(String),

  /// Known file extension encountered, but corresponding parser is not enabled (cargo features).
  #[error("supported (but disabled) format encountered {extension} - try enabling `{feature}` ")]
  DisabledFormat {
    /// The extension encountered.
    extension: String,

    /// The cargo feature to enable it.
    feature: String,
  },

  /// A generic IO error with context of what caused it.
  #[error("unable to read Vitul config file at {path} because {error}")]
  Io {
    /// The path the IO error occurred on.
    path: PathBuf,

    /// The [`std::io::Error`].
    error: std::io::Error,
  },
}

/// Determines if the given folder has a configuration file.
pub fn folder_has_configuration_file(target: Target, folder: &Path) -> bool {
  does_supported_file_name_exist(target, folder.join(ConfigFormat::Json.into_file_name()))
    || does_supported_file_name_exist(
      target,
      folder.join(ConfigFormat::Json.into_platform_file_name(target)),
    )
    || does_supported_file_name_exist(
      target,
      folder.join(ConfigFormat::Json.into_legacy_platform_file_name(target)),
    )
}

/// Determines if the given file path represents a Vitul or legacy Tauri configuration file.
pub fn is_configuration_file(target: Target, path: &Path) -> bool {
  path
    .file_name()
    .map(|file_name| {
      file_name == OsStr::new(ConfigFormat::Json.into_file_name())
        || file_name == OsStr::new(ConfigFormat::Json5.into_file_name())
        || file_name == OsStr::new(ConfigFormat::Toml.into_file_name())
        || file_name == OsStr::new(ConfigFormat::Json.into_legacy_file_name())
        || file_name == OsStr::new(ConfigFormat::Json5.into_legacy_file_name())
        || file_name == OsStr::new(ConfigFormat::Toml.into_legacy_file_name())
      // platform file names
      || file_name == OsStr::new(ConfigFormat::Json.into_platform_file_name(target))
        || file_name == OsStr::new(ConfigFormat::Json5.into_platform_file_name(target))
        || file_name == OsStr::new(ConfigFormat::Toml.into_platform_file_name(target))
        || file_name == OsStr::new(ConfigFormat::Json.into_legacy_platform_file_name(target))
        || file_name == OsStr::new(ConfigFormat::Json5.into_legacy_platform_file_name(target))
        || file_name == OsStr::new(ConfigFormat::Toml.into_legacy_platform_file_name(target))
    })
    .unwrap_or_default()
}

/// Reads the configuration from the given root directory.
///
/// It first looks for a `vitul.conf.json[5]` or `Vitul.toml` file on the given directory.
/// Legacy `tauri.conf.json[5]` and `Tauri.toml` files are also recognized.
/// Then it looks for a platform-specific configuration file:
/// - `vitul.macos.conf.json[5]` or `Vitul.macos.toml` on macOS
/// - `vitul.linux.conf.json[5]` or `Vitul.linux.toml` on Linux
/// - `vitul.windows.conf.json[5]` or `Vitul.windows.toml` on Windows
/// - `vitul.android.conf.json[5]` or `Vitul.android.toml` on Android
/// - `vitul.ios.conf.json[5]` or `Vitul.ios.toml` on iOS
///   Merging the configurations using [JSON Merge Patch (RFC 7396)].
///
/// Returns the raw configuration and used config paths.
///
/// [JSON Merge Patch (RFC 7396)]: https://datatracker.ietf.org/doc/html/rfc7396.
pub fn read_from(target: Target, root_dir: &Path) -> Result<(Value, Vec<PathBuf>), ConfigError> {
  let vitul_config_path = root_dir.join(ConfigFormat::Json.into_file_name());
  let config_path = if has_supported_file_name_in_family(target, &vitul_config_path, false) {
    vitul_config_path
  } else {
    root_dir.join(ConfigFormat::Json.into_legacy_file_name())
  };
  let (mut config, config_file_path) = parse_value(target, config_path)?;
  let mut config_paths = vec![config_file_path];
  if let Some((platform_config, path)) = read_platform(target, root_dir)? {
    config_paths.push(path);
    merge(&mut config, &platform_config);
  }
  Ok((config, config_paths))
}

/// Reads the platform-specific configuration file from the given root directory if it exists.
///
/// Check [`read_from`] for more information.
pub fn read_platform(
  target: Target,
  root_dir: &Path,
) -> Result<Option<(Value, PathBuf)>, ConfigError> {
  let platform_config_path = root_dir.join(ConfigFormat::Json.into_platform_file_name(target));
  let platform_config_path =
    if has_supported_file_name_in_family(target, &platform_config_path, false) {
      platform_config_path
    } else {
      root_dir.join(ConfigFormat::Json.into_legacy_platform_file_name(target))
    };
  if does_supported_file_name_exist(target, &platform_config_path) {
    let (platform_config, path): (Value, PathBuf) = parse_value(target, platform_config_path)?;
    Ok(Some((platform_config, path)))
  } else {
    Ok(None)
  }
}

/// Check if a supported config file exists at path.
///
/// The passed path is expected to be the path to the "default" configuration format, in this case
/// JSON with `.json`.
pub fn does_supported_file_name_exist(target: Target, path: impl Into<PathBuf>) -> bool {
  let path = path.into();
  has_supported_file_name_in_family(target, &path, false)
    || has_supported_file_name_in_family(target, &path, true)
}

fn has_supported_file_name_in_family(target: Target, path: &Path, legacy: bool) -> bool {
  let source_file_name = path.file_name().unwrap();
  let lookup_platform_config = ENABLED_FORMATS.iter().any(|format| {
    source_file_name
      == if legacy {
        format.into_legacy_platform_file_name(target)
      } else {
        format.into_platform_file_name(target)
      }
  });
  ENABLED_FORMATS.iter().any(|format| {
    path
      .with_file_name(if lookup_platform_config && legacy {
        format.into_legacy_platform_file_name(target)
      } else if lookup_platform_config {
        format.into_platform_file_name(target)
      } else if legacy {
        format.into_legacy_file_name()
      } else {
        format.into_file_name()
      })
      .exists()
  })
}

/// Parse the config from path, including alternative formats.
///
/// Hierarchy:
/// 1. Check if `vitul.conf.json` exists (falling back to legacy `tauri.conf.json`)
///    a. Parse it with `serde_json`
///    b. Parse it with `json5` if `serde_json` fails
///    c. Return original `serde_json` error if all above steps failed
/// 2. Check if `vitul.conf.json5` exists
///    a. Parse it with `json5`
///    b. Return error if all above steps failed
/// 3. Check if `Vitul.toml` exists
///    a. Parse it with `toml`
///    b. Return error if all above steps failed
/// 4. Return error if all above steps failed
pub fn parse(target: Target, path: impl Into<PathBuf>) -> Result<(Config, PathBuf), ConfigError> {
  do_parse(target, path.into())
}

/// See [`parse`] for specifics, returns a JSON [`Value`] instead of [`Config`].
pub fn parse_value(
  target: Target,
  path: impl Into<PathBuf>,
) -> Result<(Value, PathBuf), ConfigError> {
  do_parse(target, path.into())
}

fn do_parse<D: DeserializeOwned>(
  target: Target,
  mut path: PathBuf,
) -> Result<(D, PathBuf), ConfigError> {
  if !has_supported_file_name_in_family(target, &path, false)
    && path
      .file_name()
      .is_some_and(|name| name.to_string_lossy().starts_with("vitul."))
  {
    let legacy_path = path.with_file_name(
      path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .replacen("vitul.", "tauri.", 1),
    );
    if has_supported_file_name_in_family(target, &legacy_path, true) {
      path = legacy_path;
    }
  }
  let file_name = path
    .file_name()
    .map(OsStr::to_string_lossy)
    .unwrap_or_default();
  let legacy = file_name.starts_with("tauri.") || file_name.starts_with("Tauri.");
  let lookup_platform_config = ENABLED_FORMATS.iter().any(|format| {
    file_name
      == if legacy {
        format.into_legacy_platform_file_name(target)
      } else {
        format.into_platform_file_name(target)
      }
  });

  let json5 = path.with_file_name(if lookup_platform_config {
    if legacy {
      ConfigFormat::Json5.into_legacy_platform_file_name(target)
    } else {
      ConfigFormat::Json5.into_platform_file_name(target)
    }
  } else if legacy {
    ConfigFormat::Json5.into_legacy_file_name()
  } else {
    ConfigFormat::Json5.into_file_name()
  });
  let toml = path.with_file_name(if lookup_platform_config {
    if legacy {
      ConfigFormat::Toml.into_legacy_platform_file_name(target)
    } else {
      ConfigFormat::Toml.into_platform_file_name(target)
    }
  } else if legacy {
    ConfigFormat::Toml.into_legacy_file_name()
  } else {
    ConfigFormat::Toml.into_file_name()
  });

  let path_ext = path
    .extension()
    .map(OsStr::to_string_lossy)
    .unwrap_or_default();

  if path.exists() {
    let raw = read_to_string(&path)?;

    // to allow us to easily use the compile-time #[cfg], we always bind
    #[allow(clippy::let_and_return)]
    let json = do_parse_json(&raw, &path);

    // we also want to support **valid** json5 in the .json extension if the feature is enabled.
    // if the json5 is not valid the serde_json error for regular json will be returned.
    // this could be a bit confusing, so we may want to encourage users using json5 to use the
    // .json5 extension instead of .json
    #[cfg(feature = "config-json5")]
    let json = {
      match do_parse_json5(&raw, &path) {
        json5 @ Ok(_) => json5,

        // assume any errors from json5 in a .json file is because it's not json5
        Err(_) => json,
      }
    };

    json.map(|j| (j, path))
  } else if json5.exists() {
    #[cfg(feature = "config-json5")]
    {
      let raw = read_to_string(&json5)?;
      do_parse_json5(&raw, &json5).map(|config| (config, json5))
    }

    #[cfg(not(feature = "config-json5"))]
    Err(ConfigError::DisabledFormat {
      extension: ".json5".into(),
      feature: "config-json5".into(),
    })
  } else if toml.exists() {
    #[cfg(feature = "config-toml")]
    {
      let raw = read_to_string(&toml)?;
      do_parse_toml(&raw, &toml).map(|config| (config, toml))
    }

    #[cfg(not(feature = "config-toml"))]
    Err(ConfigError::DisabledFormat {
      extension: ".toml".into(),
      feature: "config-toml".into(),
    })
  } else if !EXTENSIONS_SUPPORTED.contains(&path_ext.as_ref()) {
    Err(ConfigError::UnsupportedFormat(path_ext.to_string()))
  } else {
    Err(ConfigError::Io {
      path,
      error: std::io::ErrorKind::NotFound.into(),
    })
  }
}

/// "Low-level" helper to parse JSON into a [`Config`].
///
/// `raw` should be the contents of the file that is represented by `path`.
pub fn parse_json(raw: &str, path: &Path) -> Result<Config, ConfigError> {
  do_parse_json(raw, path)
}

/// "Low-level" helper to parse JSON into a JSON [`Value`].
///
/// `raw` should be the contents of the file that is represented by `path`.
pub fn parse_json_value(raw: &str, path: &Path) -> Result<Value, ConfigError> {
  do_parse_json(raw, path)
}

fn do_parse_json<D: DeserializeOwned>(raw: &str, path: &Path) -> Result<D, ConfigError> {
  serde_json::from_str(raw).map_err(|error| ConfigError::FormatJson {
    path: path.into(),
    error,
  })
}

/// "Low-level" helper to parse JSON5 into a [`Config`].
///
/// `raw` should be the contents of the file that is represented by `path`. This function requires
/// the `config-json5` feature to be enabled.
#[cfg(feature = "config-json5")]
pub fn parse_json5(raw: &str, path: &Path) -> Result<Config, ConfigError> {
  do_parse_json5(raw, path)
}

/// "Low-level" helper to parse JSON5 into a JSON [`Value`].
///
/// `raw` should be the contents of the file that is represented by `path`. This function requires
/// the `config-json5` feature to be enabled.
#[cfg(feature = "config-json5")]
pub fn parse_json5_value(raw: &str, path: &Path) -> Result<Value, ConfigError> {
  do_parse_json5(raw, path)
}

#[cfg(feature = "config-json5")]
fn do_parse_json5<D: DeserializeOwned>(raw: &str, path: &Path) -> Result<D, ConfigError> {
  ::json5::from_str(raw).map_err(|error| ConfigError::FormatJson5 {
    path: path.into(),
    error,
  })
}

#[cfg(feature = "config-toml")]
fn do_parse_toml<D: DeserializeOwned>(raw: &str, path: &Path) -> Result<D, ConfigError> {
  ::toml::from_str(raw).map_err(|error| ConfigError::FormatToml {
    path: path.into(),
    error: Box::new(error),
  })
}

/// Helper function to wrap IO errors from [`std::fs::read_to_string`] into a [`ConfigError`].
fn read_to_string(path: &Path) -> Result<String, ConfigError> {
  std::fs::read_to_string(path).map_err(|error| ConfigError::Io {
    path: path.into(),
    error,
  })
}
