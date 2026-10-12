use std::path::Path;

use serde::{Deserialize, Serialize};
use tokio::{
  fs::File,
  io::{AsyncReadExt, AsyncSeekExt, SeekFrom},
};

/// Runtime family selected for a Windows PE executable.
///
/// `X86_64` covers both i386 and x86-64 PE files: both run in the existing
/// x86-64 Wine environment, with FEX translating its Linux userspace on ARM64.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowsArchitecture {
  #[default]
  X86_64,
  Arm64,
}

impl WindowsArchitecture {
  pub fn display_name(self) -> &'static str {
    match self {
      Self::X86_64 => "x86/x64",
      Self::Arm64 => "ARM64",
    }
  }
}

pub async fn detect_pe_architecture(path: &Path) -> Result<WindowsArchitecture, String> {
  let mut file = File::open(path)
    .await
    .map_err(|error| format!("Could not read Windows executable: {error}"))?;
  let file_size = file
    .metadata()
    .await
    .map_err(|error| format!("Could not inspect Windows executable: {error}"))?
    .len();
  let mut dos_header = [0_u8; 64];
  file
    .read_exact(&mut dos_header)
    .await
    .map_err(|_| "The EXE file has a truncated DOS header.".to_owned())?;
  if &dos_header[..2] != b"MZ" {
    return Err("The EXE file does not have a valid DOS signature.".to_owned());
  }

  let pe_offset = u32::from_le_bytes(dos_header[0x3c..0x40].try_into().unwrap()) as u64;
  let header_end = pe_offset.saturating_add(6);
  if pe_offset < dos_header.len() as u64 || header_end > file_size {
    return Err("The EXE file has an invalid or truncated PE header.".to_owned());
  }
  file
    .seek(SeekFrom::Start(pe_offset))
    .await
    .map_err(|error| format!("Could not read PE header: {error}"))?;
  let mut header = [0_u8; 6];
  file
    .read_exact(&mut header)
    .await
    .map_err(|_| "The EXE file has a truncated PE header.".to_owned())?;
  if &header[..4] != b"PE\0\0" {
    return Err("The EXE file does not contain a valid PE signature.".to_owned());
  }

  let machine = u16::from_le_bytes([header[4], header[5]]);
  match machine {
    0x014c | 0x8664 => Ok(WindowsArchitecture::X86_64),
    0xaa64 => Ok(WindowsArchitecture::Arm64),
    0xa641 | 0xa64e => Err(
      "ARM64EC/ARM64X executables need Wine's mixed-architecture emulator integration, which WinTone does not bundle yet."
        .to_owned(),
    ),
    other => Err(format!(
      "This Windows PE machine type 0x{other:04x} is not supported. WinTone currently supports x86, x64, and native ARM64 PE files."
    )),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
  };

  static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(0);

  fn pe(machine: u16) -> Vec<u8> {
    let mut bytes = vec![0_u8; 0x80];
    bytes[..2].copy_from_slice(b"MZ");
    bytes[0x3c..0x40].copy_from_slice(&0x40_u32.to_le_bytes());
    bytes[0x40..0x44].copy_from_slice(b"PE\0\0");
    bytes[0x44..0x46].copy_from_slice(&machine.to_le_bytes());
    bytes
  }

  async fn detect(bytes: &[u8]) -> Result<WindowsArchitecture, String> {
    let filename = format!(
      "wintone-pe-{}-{}.exe",
      std::process::id(),
      NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed)
    );
    let path = std::env::temp_dir().join(filename);
    fs::write(&path, bytes).unwrap();
    let result = detect_pe_architecture(&path).await;
    let _ = fs::remove_file(path);
    result
  }

  #[tokio::test]
  async fn routes_x86_and_x64_to_the_legacy_wine_runtime() {
    assert_eq!(
      detect(&pe(0x014c)).await.unwrap(),
      WindowsArchitecture::X86_64
    );
    assert_eq!(
      detect(&pe(0x8664)).await.unwrap(),
      WindowsArchitecture::X86_64
    );
  }

  #[tokio::test]
  async fn routes_arm64_pe_to_native_wine() {
    assert_eq!(
      detect(&pe(0xaa64)).await.unwrap(),
      WindowsArchitecture::Arm64
    );
  }

  #[tokio::test]
  async fn classifies_the_checked_in_windows_samples() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/hello-world");
    assert_eq!(
      detect_pe_architecture(&examples.join("hello-windows-arm64.exe"))
        .await
        .unwrap(),
      WindowsArchitecture::Arm64
    );
    assert_eq!(
      detect_pe_architecture(&examples.join("hello-windows-x64.exe"))
        .await
        .unwrap(),
      WindowsArchitecture::X86_64
    );
  }

  #[tokio::test]
  async fn rejects_arm64ec_until_mixed_architecture_emulation_is_packaged() {
    assert!(detect(&pe(0xa641))
      .await
      .unwrap_err()
      .contains("ARM64EC/ARM64X"));
  }

  #[tokio::test]
  async fn rejects_truncated_and_unknown_pe_files() {
    assert!(detect(b"MZ").await.unwrap_err().contains("truncated DOS"));
    assert!(detect(&pe(0x01c0))
      .await
      .unwrap_err()
      .contains("not supported"));
  }
}
