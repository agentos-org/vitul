use std::{fs::File, path::Path, process::Command};

use symphonia::{
  core::{
    formats::{probe::Hint, FormatOptions, TrackType},
    io::MediaSourceStream,
    meta::{MetadataOptions, RawValue, StandardTag},
  },
  default::get_probe,
};

#[derive(Clone, Debug, Default)]
pub struct TrackMetadata {
  pub title: Option<String>,
  pub artist: Option<String>,
  pub album: Option<String>,
  pub duration: f64,
}

pub fn read(path: &Path, extension: &str, ffmpeg: &str) -> TrackMetadata {
  let mut result = read_with_symphonia(path, extension).unwrap_or_default();
  if !ffmpeg.is_empty()
    && (result.title.is_none()
      || result.artist.is_none()
      || result.album.is_none()
      || result.duration <= 0.0)
  {
    merge_missing(&mut result, read_with_ffmpeg(path, ffmpeg));
  }
  result
}

fn read_with_symphonia(path: &Path, extension: &str) -> Option<TrackMetadata> {
  let file = File::open(path).ok()?;
  let stream = MediaSourceStream::new(Box::new(file), Default::default());
  let mut hint = Hint::new();
  hint.with_extension(extension);
  let mut reader = get_probe()
    .probe(
      &hint,
      stream,
      FormatOptions::default(),
      MetadataOptions::default(),
    )
    .ok()?;

  let mut result = TrackMetadata::default();
  if let Some(track) = reader.default_track(TrackType::Audio) {
    result.duration = track
      .time_base
      .zip(track.duration)
      .and_then(|(time_base, duration)| time_base.calc_duration(duration))
      .map(|time| time.as_nanos() as f64 / 1_000_000_000.0)
      .filter(|duration| duration.is_finite() && *duration > 0.0)
      .unwrap_or_default();
  }

  let mut metadata = reader.metadata();
  if let Some(revision) = metadata.skip_to_latest() {
    let mut album_artist = None;
    for tag in &revision.media.tags {
      let raw_value = match &tag.raw.value {
        RawValue::String(value) => Some(value.as_str()),
        RawValue::StringList(values) => values.first().map(String::as_str),
        _ => None,
      };

      if let Some(value) = raw_value.filter(|value| !value.trim().is_empty()) {
        match tag.std.as_ref() {
          Some(StandardTag::TrackTitle(value)) => set_if_empty(&mut result.title, value),
          Some(StandardTag::Artist(value)) => set_if_empty(&mut result.artist, value),
          Some(StandardTag::AlbumArtist(value)) => set_if_empty(&mut album_artist, value),
          Some(StandardTag::Album(value)) => set_if_empty(&mut result.album, value),
          _ => {}
        }
        match tag.raw.key.trim().to_ascii_lowercase().as_str() {
          "title" | "tracktitle" | "tit2" => set_if_empty(&mut result.title, value),
          "artist" | "performer" | "tpe1" => set_if_empty(&mut result.artist, value),
          "albumartist" | "album artist" | "album_artist" | "tpe2" => {
            set_if_empty(&mut album_artist, value)
          }
          "album" | "talb" => set_if_empty(&mut result.album, value),
          _ => {}
        }
      }
    }
    if result.artist.is_none() {
      result.artist = album_artist;
    }
  }

  Some(result)
}

fn read_with_ffmpeg(path: &Path, ffmpeg: &str) -> TrackMetadata {
  let output = Command::new(ffmpeg)
    .args([
      "-hide_banner",
      "-probesize",
      "5000000",
      "-analyzeduration",
      "5000000",
      "-i",
    ])
    .arg(path)
    .output();
  let Ok(output) = output else {
    return TrackMetadata::default();
  };
  let mut result = TrackMetadata::default();
  let stderr = String::from_utf8_lossy(&output.stderr);
  for line in stderr.lines() {
    let trimmed = line.trim();
    if let Some(value) = trimmed.strip_prefix("Duration:") {
      result.duration = parse_duration(value.trim());
      continue;
    }
    let Some((key, value)) = trimmed.split_once(':') else {
      continue;
    };
    match key.trim().to_ascii_lowercase().as_str() {
      "title" => set_if_empty(&mut result.title, value.trim()),
      "artist" | "performer" => set_if_empty(&mut result.artist, value.trim()),
      "albumartist" | "album artist" => set_if_empty(&mut result.artist, value.trim()),
      "album" => set_if_empty(&mut result.album, value.trim()),
      _ => {}
    }
  }
  result
}

fn merge_missing(target: &mut TrackMetadata, fallback: TrackMetadata) {
  if target.title.is_none() {
    target.title = fallback.title;
  }
  if target.artist.is_none() {
    target.artist = fallback.artist;
  }
  if target.album.is_none() {
    target.album = fallback.album;
  }
  if target.duration <= 0.0 {
    target.duration = fallback.duration;
  }
}

fn set_if_empty(target: &mut Option<String>, value: &str) {
  if target.is_none() {
    let cleaned = value.trim();
    if !cleaned.is_empty() {
      *target = Some(cleaned.to_string());
    }
  }
}

fn parse_duration(value: &str) -> f64 {
  let Some(clock) = value.split(',').next() else {
    return 0.0;
  };
  let mut parts = clock.trim().split(':');
  let (Some(hours), Some(minutes), Some(seconds)) = (parts.next(), parts.next(), parts.next())
  else {
    return 0.0;
  };
  if parts.next().is_some() {
    return 0.0;
  }
  let (Ok(hours), Ok(minutes), Ok(seconds)) = (
    hours.parse::<f64>(),
    minutes.parse::<f64>(),
    seconds.parse::<f64>(),
  ) else {
    return 0.0;
  };
  let duration = hours * 3600.0 + minutes * 60.0 + seconds;
  if duration.is_finite() && duration > 0.0 {
    duration
  } else {
    0.0
  }
}
