export type Locale = "zh" | "en";
export type LanguagePreference = "system" | "zh" | "en";
export type ThemePreference = "system" | "light" | "dark";
export type Theme = "light" | "dark";
export type View = "home" | "library" | "favorites" | "playlist" | "settings";
export type RepeatMode = "off" | "all" | "one";
export type DecoderFormat = "flac" | "wav" | "aiff" | "alac" | "wv" | "ape" | "other";
export type DecoderPreference = "auto" | "ffmpeg";
export type DecoderPreferences = Record<DecoderFormat, DecoderPreference>;
export type BitPerfectSettings = { enabled: boolean };

export const DEFAULT_DECODER_PREFERENCES: DecoderPreferences = {
  flac: "auto",
  wav: "auto",
  aiff: "auto",
  alac: "auto",
  wv: "auto",
  ape: "auto",
  other: "auto",
};

export type Track = {
  id: string;
  key: string;
  trackNumber?: number;
  fileName: string;
  title: string;
  artist: string;
  album: string;
  extension: string;
  size: number;
  duration: number;
  tone: number;
  addedAt: number;
  url: string;
  file: { name: string };
};

export type Playlist = {
  id: string;
  name: string;
  keys: string[];
};

export type DecoderState = {
  loading: boolean;
  available: boolean;
  engine: string;
  path: string;
  formats: string[];
  service: string;
  error: string;
  ffmpegAvailable: boolean;
  engines: Array<{ formats: string[]; engine: string }>;
  audio: { available: boolean; devices: Array<{ id: string; name: string; mode: "shared" | "exclusive"; available?: boolean }>; error: string };
};

export type PlaybackState = {
  state: "idle" | "preparing" | "playing" | "paused" | "stopped" | "ended" | "error";
  trackId: string | null;
  currentTime: number;
  duration: number;
  mode: "shared" | "exclusive";
  device: string;
  sampleRate?: number | null;
  channels?: number | null;
  bitsPerSample?: number | null;
  sampleFormat?: string | null;
  decoder?: string | null;
  decodeWarning?: string | null;
  sourceLossless?: boolean | null;
  exactDecode?: boolean | null;
  bitPerfectDecode?: boolean | null;
  bitPerfectOutput?: boolean | null;
  bitPerfectReason?: string | null;
  error?: string | null;
  autoAdvanceCue?: boolean;
};
