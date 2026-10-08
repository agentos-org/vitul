use std::path::Path;

#[derive(Clone, Debug)]
pub struct CueSheet {
  pub album: Option<String>,
  pub performer: Option<String>,
  pub tracks: Vec<CueTrack>,
}

#[derive(Clone, Debug)]
pub struct CueTrack {
  pub number: u32,
  pub title: Option<String>,
  pub performer: Option<String>,
  pub start_frame: u64,
  has_index01: bool,
}

pub fn parse(data: &[u8], selected_audio_name: &str) -> Result<CueSheet, String> {
  let text = decode(data)?;
  let mut source_file: Option<String> = None;
  let mut album = None;
  let mut album_performer = None;
  let mut tracks: Vec<CueTrack> = Vec::new();

  for (line_number, raw_line) in text.lines().enumerate() {
    let line = raw_line.trim().trim_start_matches('\u{feff}').trim();
    if line.is_empty() || line.starts_with("REM ") || line.eq_ignore_ascii_case("REM") {
      continue;
    }
    let Some((command, rest)) = line.split_once(char::is_whitespace) else {
      continue;
    };
    let command = command.to_ascii_uppercase();
    let rest = rest.trim();
    match command.as_str() {
      "FILE" => {
        if source_file.is_some() {
          return Err("This version supports one audio source per CUE sheet.".to_string());
        }
        let name =
          first_value(rest).ok_or_else(|| cue_line_error(line_number, "FILE name is missing"))?;
        source_file = Some(name);
      }
      "TRACK" => {
        let mut fields = rest.split_whitespace();
        let number = fields
          .next()
          .and_then(|value| value.parse::<u32>().ok())
          .ok_or_else(|| cue_line_error(line_number, "invalid TRACK number"))?;
        let kind = fields.next().unwrap_or_default();
        if !kind.eq_ignore_ascii_case("AUDIO") {
          return Err(cue_line_error(
            line_number,
            "only AUDIO tracks are supported",
          ));
        }
        if number == 0 || tracks.last().is_some_and(|track| number <= track.number) {
          return Err(cue_line_error(
            line_number,
            "track numbers must be positive and ascending",
          ));
        }
        tracks.push(CueTrack {
          number,
          title: None,
          performer: None,
          start_frame: 0,
          has_index01: false,
        });
      }
      "INDEX" => {
        let mut fields = rest.split_whitespace();
        let index = fields.next().unwrap_or_default();
        let time = fields
          .next()
          .ok_or_else(|| cue_line_error(line_number, "INDEX time is missing"))?;
        if index == "01" {
          let current = tracks
            .last_mut()
            .ok_or_else(|| cue_line_error(line_number, "INDEX appears before TRACK"))?;
          current.start_frame =
            parse_time(time).ok_or_else(|| cue_line_error(line_number, "invalid INDEX 01 time"))?;
          current.has_index01 = true;
        }
      }
      "TITLE" => {
        let value =
          first_value(rest).ok_or_else(|| cue_line_error(line_number, "TITLE value is missing"))?;
        if let Some(track) = tracks.last_mut() {
          track.title = Some(value);
        } else {
          album = Some(value);
        }
      }
      "PERFORMER" => {
        let value = first_value(rest)
          .ok_or_else(|| cue_line_error(line_number, "PERFORMER value is missing"))?;
        if let Some(track) = tracks.last_mut() {
          track.performer = Some(value);
        } else {
          album_performer = Some(value);
        }
      }
      _ => {}
    }
  }

  let source_file =
    source_file.ok_or_else(|| "CUE sheet does not contain a FILE entry.".to_string())?;
  let normalized_source_file = source_file.replace('\\', "/");
  let referenced_name = Path::new(&normalized_source_file)
    .file_name()
    .and_then(|name| name.to_str())
    .unwrap_or(&source_file);
  if !referenced_name
    .to_lowercase()
    .eq(&selected_audio_name.to_lowercase())
  {
    return Err(format!(
      "CUE references '{referenced_name}', but the selected audio file is '{selected_audio_name}'."
    ));
  }
  if tracks.is_empty() {
    return Err("CUE sheet does not contain any AUDIO tracks.".to_string());
  }
  if tracks.iter().any(|track| !track.has_index01) {
    return Err("Every CUE track must contain an INDEX 01 entry.".to_string());
  }
  if tracks
    .windows(2)
    .any(|pair| pair[1].start_frame <= pair[0].start_frame)
  {
    return Err("CUE INDEX 01 positions must be strictly increasing.".to_string());
  }

  Ok(CueSheet {
    album,
    performer: album_performer,
    tracks,
  })
}

fn decode(data: &[u8]) -> Result<String, String> {
  if let Some(bytes) = data.strip_prefix(&[0xff, 0xfe]) {
    return decode_utf16(bytes, true);
  }
  if let Some(bytes) = data.strip_prefix(&[0xfe, 0xff]) {
    return decode_utf16(bytes, false);
  }
  let data = data.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(data);
  if let Ok(text) = std::str::from_utf8(data) {
    return Ok(text.to_string());
  }
  let (text, had_errors) = encoding_rs::GB18030.decode_without_bom_handling(data);
  if had_errors {
    Err("CUE text encoding is not valid UTF-8, GB18030, or UTF-16.".to_string())
  } else {
    Ok(text.into_owned())
  }
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> Result<String, String> {
  if bytes.len() % 2 != 0 {
    return Err("CUE file has an incomplete UTF-16 code unit.".to_string());
  }
  let units = bytes
    .chunks_exact(2)
    .map(|pair| {
      if little_endian {
        u16::from_le_bytes([pair[0], pair[1]])
      } else {
        u16::from_be_bytes([pair[0], pair[1]])
      }
    })
    .collect::<Vec<_>>();
  char::decode_utf16(units)
    .collect::<Result<String, _>>()
    .map_err(|_| "CUE file contains invalid UTF-16 text.".to_string())
}

fn first_value(value: &str) -> Option<String> {
  let value = value.trim();
  if let Some(quoted) = value.strip_prefix('"') {
    let end = quoted.find('"')?;
    Some(quoted[..end].trim().to_string()).filter(|item| !item.is_empty())
  } else {
    value.split_whitespace().next().map(str::to_string)
  }
}

fn parse_time(value: &str) -> Option<u64> {
  let mut fields = value.split(':');
  let minutes = fields.next()?.parse::<u64>().ok()?;
  let seconds = fields.next()?.parse::<u64>().ok()?;
  let frames = fields.next()?.parse::<u64>().ok()?;
  if fields.next().is_some() || seconds >= 60 || frames >= 75 {
    return None;
  }
  minutes
    .checked_mul(60 * 75)?
    .checked_add(seconds * 75)?
    .checked_add(frames)
}

fn cue_line_error(line: usize, message: &str) -> String {
  format!("CUE line {}: {message}.", line + 1)
}
