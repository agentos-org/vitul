import type { BitPerfectSettings, DecoderFormat, DecoderPreference, DecoderPreferences, DecoderState, PlaybackState, Track } from "./types";

const pluginBase = window.location.pathname === "/apps/toney-music" || window.location.pathname.startsWith("/apps/toney-music/")
  ? "/apps/toney-music"
  : "";

export function apiUrl(path: string): string {
  return `${pluginBase}/api${path}`;
}

async function requestJSON<T>(path: string, options?: RequestInit): Promise<T> {
  const response = await fetch(apiUrl(path), options);
  let payload: unknown = null;
  try {
    payload = await response.json();
  } catch {
    // Some delete responses intentionally have no JSON body.
  }
  if (!response.ok) {
    const error = payload && typeof payload === "object" && "error" in payload ? String(payload.error) : `Request failed: ${response.status}`;
    throw new Error(error);
  }
  return payload as T;
}

type TrackRecord = Omit<Track, "url" | "file">;
type TrackPayload = { tracks?: TrackRecord[]; track?: TrackRecord; duplicate?: boolean };

export function normalizeTrack(record: Omit<Track, "url" | "file">): Track {
  return {
    ...record,
    album: record.album ?? "",
    file: { name: record.fileName || record.title || "audio" },
    url: apiUrl(`/tracks/${encodeURIComponent(record.id)}/stream`),
  };
}

export async function fetchTracks(): Promise<Track[]> {
  const payload = await requestJSON<TrackPayload>("/tracks");
  return Array.isArray(payload.tracks) ? payload.tracks.map(normalizeTrack) : [];
}

export async function fetchDecoderStatus(): Promise<DecoderState> {
  const payload = await requestJSON<{
    service?: string;
    decoder?: { available?: boolean; engine?: string; path?: string; formats?: string[]; ffmpegAvailable?: boolean };
    audio?: { available?: boolean; devices?: Array<{ id: string; name: string; mode: "shared" | "exclusive"; available?: boolean }>; error?: string };
  }>("/status");
  const decoder = payload.decoder ?? {};
  return {
    loading: false,
    available: Boolean(decoder.available),
    engine: decoder.engine ?? "",
    path: decoder.path ?? "",
    formats: Array.isArray(decoder.formats) ? decoder.formats : [],
    service: payload.service ?? "",
    error: "",
    ffmpegAvailable: Boolean(decoder.ffmpegAvailable),
    engines: Array.isArray((payload.decoder as { engines?: unknown })?.engines) ? (payload.decoder as { engines: DecoderState["engines"] }).engines : [],
    audio: {
      available: Boolean(payload.audio?.available),
      devices: Array.isArray(payload.audio?.devices) ? payload.audio!.devices! : [],
      error: payload.audio?.error ?? "",
    },
  };
}

export async function fetchDecoderPreferences(): Promise<DecoderPreferences> {
  return requestJSON<DecoderPreferences>("/decoder-settings");
}

export async function setDecoderPreference(format: DecoderFormat, preference: DecoderPreference): Promise<DecoderPreferences> {
  return requestJSON<DecoderPreferences>("/decoder-settings", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ format, preference }),
  });
}

export async function fetchBitPerfectSettings(): Promise<BitPerfectSettings> {
  return requestJSON<BitPerfectSettings>("/bit-perfect");
}

export async function updateBitPerfectSettings(enabled: boolean, trackId?: string, device?: string): Promise<BitPerfectSettings> {
  return requestJSON<BitPerfectSettings>("/bit-perfect", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ enabled, trackId, device }),
  });
}

export async function fetchPlayback(): Promise<PlaybackState> {
  return requestJSON<PlaybackState>("/playback");
}

export async function heartbeatPlaybackUI(): Promise<void> {
  const response = await fetch(apiUrl("/client-heartbeat"), { method: "POST" });
  if (!response.ok) throw new Error(`Request failed: ${response.status}`);
}

export async function startPlayback(trackId: string, position = 0, mode: "shared" | "exclusive" = "exclusive", device = "default", autoplay = true): Promise<PlaybackState> {
  return requestJSON<PlaybackState>("/playback", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ trackId, position, mode, device, autoplay }),
  });
}

export async function pausePlayback(): Promise<PlaybackState> {
  return requestJSON<PlaybackState>("/playback/pause", { method: "POST" });
}

export async function resumePlayback(): Promise<PlaybackState> {
  return requestJSON<PlaybackState>("/playback/resume", { method: "POST" });
}

export async function stopPlayback(): Promise<void> {
  await requestJSON("/playback/stop", { method: "POST" });
}

export async function seekPlayback(position: number): Promise<PlaybackState> {
  return requestJSON<PlaybackState>("/playback/seek", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ position }),
  });
}

export async function uploadTrack(file: File, cueFile?: File): Promise<Track[]> {
  const form = new FormData();
  form.append("file", file, file.name);
  if (cueFile) form.append("cue", cueFile, cueFile.name);
  const payload = await requestJSON<TrackPayload>("/tracks", { method: "POST", body: form });
  const records = Array.isArray(payload.tracks) ? payload.tracks : payload.track ? [payload.track] : [];
  if (!records.length) throw new Error("The music service did not return any tracks");
  return records.map(normalizeTrack);
}

export async function deleteTrack(id: string): Promise<void> {
  await requestJSON(`/tracks/${encodeURIComponent(id)}`, { method: "DELETE" });
}
