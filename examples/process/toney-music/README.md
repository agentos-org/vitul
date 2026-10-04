# Toney Music: Vitulfile example

This directory is a Vitulfile packaging example for the Toney Music process app.
It demonstrates adding a native Rust audio service to an existing React/Vite
music UI. It is not a Tauri or other Rust desktop application.

The React/Vite frontend is only the control surface. The Rust `musicd` process
owns the library, decoding, output session, and background playback. It sends
raw PCM directly to the AgentOS host audio broker over
`AGENTOS_AUDIO_SOCKET`; CEF does not decode or play the audio stream.

## Decoder and output path

- Claxon (Rust) decodes FLAC samples at the declared source precision.
- Symphonia (Rust) decodes WAV, AIFF, ALAC/M4A, MP3, AAC, and Vorbis.
- `ape-decoder` (Rust) decodes APE to its native integer PCM.
- `libwavpack` decodes WavPack to integer PCM.
- FFmpeg is the fallback for unsupported formats and native-decoder failures.
  FFmpeg fallback output is 32-bit PCM and is reported as a conversion path; it
  is not advertised as bit-perfect.
- Lossy formats such as MP3 and AAC cannot reproduce the original pre-encoding
  samples. DSD-to-PCM conversion also requires filtering.

For supported lossless sources, `musicd` keeps the source sample rate, channel
count, and sample precision through decoding, and applies no gain, resampling,
or dithering. AgentOS exclusive ALSA mode rejects unsupported hardware formats.
AgentOS reports exclusive output as unverified because it cannot inspect device
DSP after ALSA accepts the stream. Shared PipeWire mode can mix or resample and
is reported as not bit-perfect.

## Build

Requirements:

- Vitul CLI, Node.js, and pnpm
- Rust stable and Cargo
- CMake and a C++ compiler for the libwavpack binding
- Docker when building for an architecture different from the build host
- A Linux ARM64 FFmpeg executable for the packaged fallback decoder

Set `FFMPEG_BIN` to the Linux ARM64 executable and build:

```sh
export FFMPEG_BIN=/path/to/linux-arm64/ffmpeg
vitul agentos validate
vitul build --target agentos/arm64
```

The Rust build script uses a Linux container for cross-platform builds and reads
`VITUL_TARGET_ARCH`. The package currently targets `agentos/arm64`. Vitul builds
the frontend and `musicd`, includes FFmpeg as fallback, and writes
`dist/toney-music-0.3.10-agentos-arm64.agt`.

The generated process manifest requests `native.audio`,
`native.audio.exclusive`, and `native.tray`. AgentOS provides
`AGENTOS_AUDIO_SOCKET` and `AGENTOS_TRAY_SOCKET` to the running app. The
`musicd` backend registers the packaged Toney Music PNG as its tray image and
updates the track/status tooltip and menu while playing. The menu provides Play
or Pause and Stop. Menu entries provide AgentOS `localizedLabels` for `en-US`
and `zh-CN`; the Shell chooses the current locale and falls back to `label`.
The hover tooltip follows Toney's interface language, which is saved with the app data.
Closing the CEF window leaves `musicd`, the audio session, and the tray controls
running; stopping or uninstalling the App ends playback and removes the tray
icon.

## CUE sheets

Import one `.cue` file together with the single supported audio file it names.
Toney creates one library song per `TRACK ... AUDIO` entry and reads album,
performer, title, and `INDEX 01` positions from the sheet. UTF-8, GB18030, and
UTF-16 CUE files are supported. Multi-file sheets and data tracks are rejected
with an import error.

The audio image remains one source file. Songs share its decoded PCM cache and
play only their sample-frame range, so the source is not split or re-encoded.
Removing a song keeps the source while another CUE song still uses it. The
browser picker and drag-and-drop require selecting the CUE and audio file
together; Fenn Files does not grant the app access to unselected neighboring
files. When the UI is closed, `musicd` advances through the current CUE album in
track-number order. The app stores its library and decoded PCM cache under the
injected `DATA_DIR`; decoded cache files can be larger than their compressed
sources.
