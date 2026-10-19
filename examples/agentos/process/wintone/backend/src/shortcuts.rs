use std::{
  fs,
  path::{Component, Path, PathBuf},
};

use sha2::{Digest, Sha256};

#[derive(Clone, Debug)]
pub struct DiscoveredApp {
  pub id: String,
  pub name: String,
  pub version: Option<String>,
  pub executable: String,
  pub uninstall_executable: Option<String>,
  pub uninstall_args: Vec<String>,
}

#[derive(Default)]
struct UninstallEntry {
  name: Option<String>,
  version: Option<String>,
  icon: Option<String>,
  install_location: Option<String>,
  uninstall: Option<String>,
}

pub fn discover(prefix: &Path, installation_id: &str) -> Vec<DiscoveredApp> {
  let prefix = fs::canonicalize(prefix).unwrap_or_else(|_| prefix.to_path_buf());
  let mut entries = Vec::new();
  for registry in [prefix.join("user.reg"), prefix.join("system.reg")] {
    let Ok(bytes) = fs::read(registry) else {
      continue;
    };
    let text = decode_registry(&bytes);
    entries.extend(parse_registry(&text));
  }

  let mut found = Vec::new();
  for (key, entry) in entries {
    let Some(name) = entry
      .name
      .map(|value| value.trim().to_owned())
      .filter(|v| !v.is_empty())
    else {
      continue;
    };
    let executable = entry
      .icon
      .as_deref()
      .and_then(|icon| icon_executable(icon, &prefix))
      .or_else(|| {
        entry
          .install_location
          .as_deref()
          .and_then(|location| find_main_executable(location, &name, &prefix))
      });
    let Some(executable) = executable else {
      continue;
    };
    let (uninstall_executable, uninstall_args) = entry
      .uninstall
      .as_deref()
      .and_then(|command| parse_uninstaller(command, &prefix))
      .map(|(executable, args)| (Some(executable), args))
      .unwrap_or((None, Vec::new()));
    let digest = Sha256::digest(key.to_lowercase().as_bytes());
    let suffix = digest[..6]
      .iter()
      .map(|byte| format!("{byte:02x}"))
      .collect::<String>();
    let id = format!("{installation_id}-{suffix}");
    if found.iter().any(|app: &DiscoveredApp| {
      app.name.eq_ignore_ascii_case(&name) && app.executable.eq_ignore_ascii_case(&executable)
    }) {
      continue;
    }
    found.push(DiscoveredApp {
      id,
      name,
      version: entry.version.filter(|value| !value.trim().is_empty()),
      executable,
      uninstall_executable,
      uninstall_args,
    });
  }
  found
}

fn decode_registry(bytes: &[u8]) -> String {
  if bytes.starts_with(&[0xff, 0xfe]) {
    let units = bytes[2..]
      .chunks_exact(2)
      .map(|pair| u16::from_le_bytes([pair[0], pair[1]]));
    return char::decode_utf16(units)
      .map(|character| character.unwrap_or(char::REPLACEMENT_CHARACTER))
      .collect();
  }
  String::from_utf8_lossy(bytes).into_owned()
}

fn parse_registry(text: &str) -> Vec<(String, UninstallEntry)> {
  let mut entries = Vec::new();
  let mut current_key: Option<String> = None;
  let mut current = UninstallEntry::default();
  for line in text.lines() {
    let line = line.trim();
    if line.starts_with('[') {
      let Some(end) = line.find(']') else {
        continue;
      };
      let timestamp = line[end + 1..].trim();
      if !timestamp.is_empty() && timestamp.parse::<u64>().is_err() {
        continue;
      }
      if let Some(key) = current_key.take() {
        entries.push((key, current));
        current = UninstallEntry::default();
      }
      let key = decode_escaped(&line[1..end]);
      let lowercase = key.to_ascii_lowercase();
      if lowercase.contains("\\software\\microsoft\\windows\\currentversion\\uninstall\\")
        || lowercase
          .contains("\\software\\wow6432node\\microsoft\\windows\\currentversion\\uninstall\\")
        || lowercase.starts_with("software\\microsoft\\windows\\currentversion\\uninstall\\")
        || lowercase
          .starts_with("software\\wow6432node\\microsoft\\windows\\currentversion\\uninstall\\")
      {
        current_key = Some(key);
      }
      continue;
    }
    if current_key.is_none() {
      continue;
    }
    let Some((name, value)) = parse_string_value(line) else {
      continue;
    };
    match name.to_ascii_lowercase().as_str() {
      "displayname" => current.name = Some(value),
      "displayversion" => current.version = Some(value),
      "displayicon" => current.icon = Some(value),
      "installlocation" => current.install_location = Some(value),
      "uninstallstring" | "quietuninstallstring" => {
        if current.uninstall.is_none() || name.eq_ignore_ascii_case("quietUninstallString") {
          current.uninstall = Some(value);
        }
      }
      _ => {}
    }
  }
  if let Some(key) = current_key {
    entries.push((key, current));
  }
  entries
}

fn parse_string_value(line: &str) -> Option<(String, String)> {
  let rest = line.strip_prefix('"')?;
  let name_end = rest.find('"')?;
  let name = rest[..name_end].to_owned();
  let value = rest[name_end + 1..]
    .trim_start()
    .strip_prefix('=')?
    .trim_start();
  if !value.starts_with('"') {
    return None;
  }
  Some((name, parse_quoted(value)?))
}

fn parse_quoted(value: &str) -> Option<String> {
  let mut chars = value.chars();
  if chars.next()? != '"' {
    return None;
  }
  let mut output = String::new();
  let mut escaped = false;
  for character in chars {
    if escaped {
      output.push(character);
      escaped = false;
    } else if character == '\\' {
      escaped = true;
    } else if character == '"' {
      return Some(output);
    } else {
      output.push(character);
    }
  }
  None
}

fn decode_escaped(value: &str) -> String {
  let mut output = String::new();
  let mut chars = value.chars();
  while let Some(character) = chars.next() {
    if character == '\\' {
      if let Some(next) = chars.next() {
        output.push(next);
      }
    } else {
      output.push(character);
    }
  }
  output
}

fn icon_executable(icon: &str, prefix: &Path) -> Option<String> {
  let icon = icon.trim();
  let unquoted = icon
    .strip_prefix('"')
    .and_then(|value| value.split_once('"').map(|p| p.0))
    .unwrap_or(icon);
  let path = unquoted
    .rsplit_once(',')
    .map(|(path, _)| path)
    .unwrap_or(unquoted)
    .trim();
  if !path.to_ascii_lowercase().ends_with(".exe") {
    return None;
  }
  let (windows_path, _) = resolve_windows_path(path, prefix)?;
  Some(windows_path)
}

fn find_main_executable(location: &str, display_name: &str, prefix: &Path) -> Option<String> {
  let native_dir = resolve_windows_directory(location, prefix)?;
  let root = if native_dir.is_file() {
    native_dir.parent()?
  } else {
    native_dir.as_path()
  };
  let wanted = normalize_filename(display_name);
  let mut candidates = Vec::<(i32, PathBuf)>::new();
  collect_executables(root, 0, &wanted, &mut candidates);
  candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
  let (_, path) = candidates.into_iter().next()?;
  let relative = path.strip_prefix(prefix.join("drive_c")).ok()?;
  let windows = format!("C:\\{}", relative.to_string_lossy().replace('/', "\\"));
  Some(windows)
}

fn collect_executables(root: &Path, depth: usize, wanted: &str, output: &mut Vec<(i32, PathBuf)>) {
  if depth > 4 || output.len() > 256 {
    return;
  }
  let Ok(entries) = fs::read_dir(root) else {
    return;
  };
  for entry in entries.flatten() {
    let path = entry.path();
    let Ok(file_type) = entry.file_type() else {
      continue;
    };
    if file_type.is_dir() {
      collect_executables(&path, depth + 1, wanted, output);
    } else if file_type.is_file()
      && path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
    {
      let stem = normalize_filename(
        path
          .file_stem()
          .and_then(|value| value.to_str())
          .unwrap_or(""),
      );
      if stem.contains("uninstall") || stem.contains("setup") || stem.contains("update") {
        continue;
      }
      let score = if stem == wanted {
        100
      } else if !wanted.is_empty() && stem.contains(&wanted) {
        60
      } else {
        1
      };
      output.push((score, path));
    }
  }
}

fn normalize_filename(value: &str) -> String {
  value
    .chars()
    .filter(|character| character.is_ascii_alphanumeric())
    .flat_map(char::to_lowercase)
    .collect()
}

fn resolve_windows_path(value: &str, prefix: &Path) -> Option<(String, PathBuf)> {
  let normalized = value.trim().trim_matches('"').replace('/', "\\");
  let tail = normalized
    .strip_prefix("C:\\")
    .or_else(|| normalized.strip_prefix("c:\\"))?;
  let mut path = prefix.join("drive_c");
  for component in tail.split('\\') {
    if component.is_empty() || component == "." {
      continue;
    }
    if component == ".." || component.contains(':') || component.contains('\0') {
      return None;
    }
    path.push(component);
  }
  let canonical_prefix = fs::canonicalize(prefix.join("drive_c")).ok()?;
  let canonical_path = fs::canonicalize(&path).ok()?;
  if !canonical_path.starts_with(&canonical_prefix) || !canonical_path.is_file() {
    return None;
  }
  let relative = canonical_path.strip_prefix(&canonical_prefix).ok()?;
  if relative
    .components()
    .any(|part| !matches!(part, Component::Normal(_)))
  {
    return None;
  }
  let windows = format!("C:\\{}", relative.to_string_lossy().replace('/', "\\"));
  Some((windows, canonical_path))
}

fn resolve_windows_directory(value: &str, prefix: &Path) -> Option<PathBuf> {
  let normalized = value.trim().trim_matches('"').replace('/', "\\");
  let tail = normalized
    .strip_prefix("C:\\")
    .or_else(|| normalized.strip_prefix("c:\\"))?;
  let mut path = prefix.join("drive_c");
  for component in tail.split('\\') {
    if component.is_empty() || component == "." {
      continue;
    }
    if component == ".." || component.contains(':') || component.contains('\0') {
      return None;
    }
    path.push(component);
  }
  let canonical_prefix = fs::canonicalize(prefix.join("drive_c")).ok()?;
  let canonical_path = fs::canonicalize(path).ok()?;
  if !canonical_path.starts_with(canonical_prefix) {
    return None;
  }
  Some(canonical_path)
}

fn parse_uninstaller(command: &str, prefix: &Path) -> Option<(String, Vec<String>)> {
  if command.len() > 16384 {
    return None;
  }
  let parts = split_windows_command_line(command);
  if parts.len() > 128 || parts.iter().any(|part| part.contains('\0')) {
    return None;
  }
  let first = parts.first()?;
  if first.eq_ignore_ascii_case("msiexec") || first.eq_ignore_ascii_case("msiexec.exe") {
    let product_code = command
      .split('{')
      .nth(1)
      .and_then(|tail| tail.split('}').next())?;
    let product_code = format!("{{{product_code}}}");
    return Some(("msiexec".to_owned(), vec!["/x".to_owned(), product_code]));
  }
  let (windows_executable, _) = resolve_windows_path(first, prefix)?;
  Some((windows_executable, parts.into_iter().skip(1).collect()))
}

fn split_windows_command_line(value: &str) -> Vec<String> {
  let mut args = Vec::new();
  let mut current = String::new();
  let mut quoted = false;
  for character in value.chars() {
    if character == '"' {
      quoted = !quoted;
    } else if character.is_whitespace() && !quoted {
      if !current.is_empty() {
        args.push(std::mem::take(&mut current));
      }
    } else {
      current.push(character);
    }
  }
  if !current.is_empty() {
    args.push(current);
  }
  args
}

#[cfg(test)]
mod tests {
  use super::discover;
  use std::{fs, path::PathBuf};
  use uuid::Uuid;

  #[test]
  fn discovers_apps_from_timestamped_wine_registry_sections() {
    let prefix: PathBuf =
      std::env::temp_dir().join(format!("wintone-discovery-{}", Uuid::new_v4()));
    let app_directory = prefix.join("drive_c/Program Files/Tencent/Weixin");
    fs::create_dir_all(&app_directory).unwrap();
    fs::write(app_directory.join("Weixin.exe"), b"test executable").unwrap();
    fs::write(
      prefix.join("system.reg"),
      r#"
[Software\\Wow6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Weixin] 1792027802
"DisplayIcon"="C:\\Program Files\\Tencent\\Weixin\\Weixin.exe"
"DisplayName"="WeChat"
"DisplayVersion"="4.1.15"
"UninstallString"="C:\\Program Files\\Tencent\\Weixin\\Uninstall.exe"
"#,
    )
    .unwrap();

    let apps = discover(&prefix, "test-installation");

    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].name, "WeChat");
    assert_eq!(
      apps[0].executable,
      r"C:\Program Files\Tencent\Weixin\Weixin.exe"
    );
    let _ = fs::remove_dir_all(prefix);
  }
}
