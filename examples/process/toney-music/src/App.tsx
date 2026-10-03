import {
  ChevronDown,
  ChevronRight,
  Disc3,
  FileAudio,
  Heart,
  House,
  Library,
  ListMusic,
  Menu,
  MoreHorizontal,
  Music2,
  Pause,
  Play,
  Plus,
  Repeat,
  Search,
  Settings2,
  Shuffle,
  SlidersHorizontal,
  Sparkles,
  SkipBack,
  SkipForward,
  Trash2,
  Upload,
  X,
  type LucideIcon,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState, type DragEvent, type Ref } from "react";
import { filePickerFileToMediaUrl, openFilePicker, readFennFileDrag, uploadFilesToFennFolder, type FennFileDragItem, type FennFilePickerFile } from "../../shared/plugin-sdk.js";
import { deleteTrack, fetchBitPerfectSettings, fetchDecoderPreferences, fetchDecoderStatus, fetchPlayback, fetchTracks, heartbeatPlaybackUI, pausePlayback, resumePlayback, seekPlayback, setDecoderPreference, startPlayback, stopPlayback, updateBitPerfectSettings, uploadTrack } from "./api";
import { resolveLocale, resolveTheme, textFor, type CopyKey } from "./i18n";
import { DEFAULT_DECODER_PREFERENCES, type DecoderFormat, type DecoderPreference, type DecoderPreferences, type DecoderState, type LanguagePreference, type Locale, type PlaybackState, type Playlist, type RepeatMode, type Theme, type ThemePreference, type Track, type View } from "./types";

const AUDIO_EXTENSIONS = /\.(flac|wav|wave|aiff|aif|mp3|aac|m4a|ogg|oga|opus|dsf|dff|ape|wv|alac)$/i;
const CUE_EXTENSION = /\.cue$/i;
const AUDIO_PICKER_ACCEPT = ["audio/*", ".flac", ".wav", ".wave", ".aiff", ".aif", ".mp3", ".aac", ".m4a", ".ogg", ".oga", ".opus", ".dsf", ".dff", ".ape", ".wv", ".alac", ".cue", "text/plain"];
const FENN_MUSIC_FOLDER = "Documents/Music";
const DEFAULT_DECODER: DecoderState = { loading: true, available: false, engine: "", path: "", formats: [], service: "", error: "", ffmpegAvailable: false, engines: [], audio: { available: false, devices: [], error: "" } };
const DEFAULT_PLAYBACK: PlaybackState = { state: "idle", trackId: null, currentTime: 0, duration: 0, mode: "exclusive", device: "default" };
const MUSIC_ICON_URL = new URL("../icon.png", import.meta.url).href;

function cn(...classes: Array<string | false | null | undefined>): string {
  return classes.filter(Boolean).join(" ");
}

function readStored<T>(key: string, fallback: T): T {
  try {
    const value = JSON.parse(localStorage.getItem(key) ?? "null") as T | null;
    return value ?? fallback;
  } catch {
    return fallback;
  }
}

function store(key: string, value: unknown): void {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Private browsing can disable localStorage; the player remains usable.
  }
}

function preferenceFromUrl<T extends string>(key: string, values: readonly T[], fallback: T): T {
  const value = new URLSearchParams(window.location.search).get(key) as T | null;
  return value && values.includes(value) ? value : fallback;
}

type ShellContextMessage = {
  channel?: string;
  type?: string;
  appearance?: string;
  theme?: string;
  locale?: string;
  runtime?: string;
};

type ImportOptions = {
  saveToFenn?: boolean;
  autoplayFirst?: boolean;
};

function normalizeShellLocale(value: string | undefined): Locale | null {
  if (value === "zh" || value === "zh-CN") return "zh";
  if (value === "en" || value === "en-US") return "en";
  return null;
}

function normalizeShellTheme(value: string | undefined): Theme | null {
  return value === "light" || value === "dark" ? value : null;
}

function formatTime(value: number, noDuration: string): string {
  if (!Number.isFinite(value) || value <= 0) return noDuration;
  const seconds = Math.floor(value);
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

function isAudioFile(file: File): boolean {
  return file.type.startsWith("audio/") || AUDIO_EXTENSIONS.test(file.name);
}

function isImportFile(file: File): boolean {
  return isAudioFile(file) || CUE_EXTENSION.test(file.name);
}

function isAudioPickerFile(file: FennFilePickerFile): boolean {
  return CUE_EXTENSION.test(file.name) || isAudioFile(new File([], file.name, { type: file.mimeType ?? "application/octet-stream" }));
}

function toneClass(tone: number): string {
  return ["tone-blue", "tone-purple", "tone-orange", "tone-green"][Math.abs(tone || 0) % 4];
}

function IconButton({ icon: Icon, label, className, active, onClick, disabled, ref }: {
  icon: LucideIcon;
  label: string;
  className?: string;
  active?: boolean;
  onClick?: () => void;
  disabled?: boolean;
  ref?: Ref<HTMLButtonElement>;
}) {
  return (
    <button
      ref={ref}
      type="button"
      className={cn("ui-icon-button", active && "is-active", className)}
      aria-label={label}
      data-tooltip={label}
      onClick={(event) => { event.stopPropagation(); onClick?.(); }}
      disabled={disabled}
    >
      <Icon size={17} strokeWidth={1.8} aria-hidden="true" />
    </button>
  );
}

function TrackCover({ track, className }: { track?: Track | null; className?: string }) {
  return (
    <div className={cn("track-cover", track ? toneClass(track.tone) : "tone-blue", className)}>
      <Music2 size={22} strokeWidth={1.8} aria-hidden="true" />
    </div>
  );
}

function EmptyState({ icon: Icon, title, body, action, onAction, className }: { icon: LucideIcon; title: string; body: string; action?: string; onAction?: () => void; className?: string }) {
  return (
    <div className={cn("empty-state", className)}>
      <div className="empty-icon"><Icon size={24} strokeWidth={1.7} aria-hidden="true" /></div>
      <strong>{title}</strong>
      <p>{body}</p>
      {action && onAction ? <button type="button" className="secondary-button" onClick={onAction}><Plus size={16} /><span>{action}</span></button> : null}
    </div>
  );
}

function TrackActions({ locale, favorite, playlists, removeLabel, onFavorite, onAddToPlaylist, onCreatePlaylist, onRemove }: {
  locale: Locale;
  favorite: boolean;
  playlists: Playlist[];
  removeLabel?: string;
  onFavorite: () => void;
  onAddToPlaylist: (playlistId: string) => void;
  onCreatePlaylist: () => void;
  onRemove?: () => void;
}) {
  const t = (key: CopyKey) => textFor(locale, key);
  const [menuOpen, setMenuOpen] = useState(false);
  const actionsRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!menuOpen) return;
    const onPointerDown = (event: PointerEvent) => {
      if (event.target instanceof Node && !actionsRef.current?.contains(event.target)) setMenuOpen(false);
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setMenuOpen(false);
    };
    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [menuOpen]);

  return (
    <div ref={actionsRef} className="row-actions">
      <IconButton icon={Heart} label={t("action.favorite")} className={favorite ? "is-favorite" : "row-action"} onClick={onFavorite} />
      <span className="more-action">
        <IconButton icon={MoreHorizontal} label={t("action.more")} className="row-action" active={menuOpen} onClick={() => setMenuOpen((value) => !value)} />
        {menuOpen ? <div className="track-action-menu" role="menu" onClick={(event) => event.stopPropagation()}>
          <div className="track-action-menu-title">{t("action.addToPlaylist")}</div>
          {playlists.length ? playlists.map((playlist) => <button key={playlist.id} type="button" className="track-action-menu-item" role="menuitem" onClick={() => { setMenuOpen(false); onAddToPlaylist(playlist.id); }}><ListMusic size={14} />{playlist.name}</button>) : <span className="track-action-menu-empty">{t("playlist.noPlaylists")}</span>}
          <button type="button" className="track-action-menu-item is-create" role="menuitem" onClick={() => { setMenuOpen(false); onCreatePlaylist(); }}><Plus size={14} />{t("action.newPlaylist")}</button>
          {onRemove && removeLabel ? <><div className="track-action-menu-divider" /><button type="button" className="track-action-menu-item is-destructive" role="menuitem" onClick={() => { setMenuOpen(false); onRemove(); }}><Trash2 size={14} />{removeLabel}</button></> : null}
        </div> : null}
      </span>
    </div>
  );
}

function Sidebar({ locale, view, selectedPlaylist, tracks, favoriteCount, playlists, onView, onPlaylist, onCreatePlaylist, onSettings, onImport }: {
  locale: Locale;
  view: View;
  selectedPlaylist: string;
  tracks: Track[];
  favoriteCount: number;
  playlists: Playlist[];
  onView: (view: View) => void;
  onPlaylist: (id: string) => void;
  onCreatePlaylist: () => void;
  onSettings: () => void;
  onImport: () => void;
}) {
  const t = (key: CopyKey, values?: Record<string, string | number>) => textFor(locale, key, values);
  const navItems: Array<[View, LucideIcon, CopyKey]> = [["home", Sparkles, "nav.home"], ["library", Library, "nav.library"], ["favorites", Heart, "nav.favorites"]];
  return (
    <aside className="music-sidebar">
      <div className="music-brand">
        <div className="brand-mark"><img src={MUSIC_ICON_URL} alt="" /></div>
        <div><strong>{t("brand.name")}</strong><span>{t("brand.subtitle")}</span></div>
      </div>
      <nav className="music-nav" aria-label="Music navigation">
        {navItems.map(([item, Icon, label]) => (
          <button key={item} type="button" className={cn("nav-item", view === item && "is-active")} onClick={() => onView(item)}>
            <Icon size={19} /><span>{t(label)}</span>{item === "library" ? <b>{tracks.length}</b> : item === "favorites" ? <b>{favoriteCount}</b> : null}
          </button>
        ))}
      </nav>
      <div className="sidebar-section-title"><span>{t("playlist.title")}</span><IconButton icon={Plus} label="Add playlist" className="small" onClick={onCreatePlaylist} /></div>
      <div className="playlist-nav">
        <button type="button" className={cn("playlist-item", view === "playlist" && selectedPlaylist === "all" && "is-active")} onClick={() => onPlaylist("all")}>
          <span className="playlist-dot gradient-a" /><span>{t("playlist.all")}</span><b>{tracks.length}</b>
        </button>
        {playlists.map((playlist, index) => (
          <button key={playlist.id} type="button" className={cn("playlist-item", view === "playlist" && selectedPlaylist === playlist.id && "is-active")} onClick={() => onPlaylist(playlist.id)}>
            <span className={cn("playlist-dot", `tone-dot-${index % 4}`)} /><span>{playlist.name}</span><b>{playlist.keys.length}</b>
          </button>
        ))}
      </div>
      <div className="sidebar-spacer" />
      <div className="sidebar-foot">
        <div className="storage-chip"><span className="status-dot" /><div><strong>{t("storage.ready")}</strong><small>{tracks.length} {t("songs")}</small></div></div>
        <button type="button" className="import-link" onClick={onImport}><Upload size={16} /><span>{t("action.import")}</span><kbd>⌘O</kbd></button>
        <button type="button" className={cn("settings-link", view === "settings" && "is-active")} onClick={onSettings}><Settings2 size={16} /><span>{t("nav.settings")}</span><kbd>⌘,</kbd></button>
      </div>
    </aside>
  );
}

function TopBar({ locale, view, title, subtitle, search, onSearch, onClear }: { locale: Locale; view: View; title: string; subtitle: string; search: string; onSearch: (value: string) => void; onClear: () => void }) {
  const t = (key: CopyKey) => textFor(locale, key);
  return (
    <header className="music-topbar">
      <div className="topbar-heading"><span className="eyebrow">{t("eyebrow")}</span><h1>{title}</h1><p>{subtitle}</p></div>
      <div className="topbar-actions">
        <label className="search-box"><Search size={16} /><input value={search} onChange={(event) => onSearch(event.target.value)} type="search" placeholder={t("search")} aria-label={t("search")} /><kbd>⌘K</kbd>{search ? <button type="button" aria-label="Clear search" onClick={onClear}><X size={14} /></button> : null}</label>
        {view === "settings" ? null : null}
      </div>
    </header>
  );
}

function HeroTitle({ title }: { title: string }) {
  const [first, second] = title.split("<br>");
  return <h2>{first}<br /><em>{second}</em></h2>;
}

function VinylArtwork({ isPlaying = false }: { isPlaying?: boolean }) {
  return (
    <div className={cn("vinyl-artwork", isPlaying && "is-playing")} aria-hidden="true">
      <div className="vinyl-orbit vinyl-orbit-one" />
      <div className="vinyl-orbit vinyl-orbit-two" />
      <div className="vinyl-disc">
        <div className="vinyl-label"><Music2 size={27} strokeWidth={1.8} /></div>
        <div className="vinyl-groove" />
      </div>
      <span className="vinyl-note vinyl-note-one">♪</span>
      <span className="vinyl-note vinyl-note-two">♫</span>
    </div>
  );
}

function HomeView({ locale, tracks, recent, currentId, favoriteKeys, playlists, removeLabel, onImport, onBrowse, onPlay, onFavorite, onAddToPlaylist, onCreatePlaylist, onRemove }: {
  locale: Locale; tracks: Track[]; recent: Track[]; currentId: string | null; favoriteKeys: ReadonlySet<string>; playlists: Playlist[]; removeLabel: string; onImport: () => void; onBrowse: () => void; onPlay: (track: Track) => void; onFavorite: (track: Track) => void; onAddToPlaylist: (track: Track, playlistId: string) => void; onCreatePlaylist: (track: Track) => void; onRemove: (track: Track) => void;
}) {
  const t = (key: CopyKey, values?: Record<string, string | number>) => textFor(locale, key, values);
  return (
    <section className="music-view">
      <div className="music-hero">
        <div className="hero-copy"><span className="hero-kicker">{t("hero.kicker")}</span><HeroTitle title={t("hero.title")} /><p>{t("hero.body")}</p><div className="hero-actions"><button type="button" className="primary-button" onClick={onImport}><Upload size={16} />{t("action.import")}</button><button type="button" className="quiet-button" onClick={onBrowse}>{t("action.browse")}<ChevronRight size={16} /></button></div></div>
        <div className="hero-art"><VinylArtwork isPlaying /></div>
        <div className="hero-stat"><strong>{tracks.length}</strong><span>{t("hero.tracks")}</span></div>
      </div>
      <div className="section-heading"><div><span className="section-kicker">{t("section.recentKicker")}</span><h2>{t("section.recentTitle")}</h2></div><button type="button" className="text-button" onClick={onBrowse}>{t("action.viewAll")}<ChevronRight size={15} /></button></div>
      {recent.length ? <div className="recent-list">{recent.map((track) => <RecentTrack key={track.id} locale={locale} track={track} current={track.id === currentId} favorite={favoriteKeys.has(track.key)} playlists={playlists} removeLabel={removeLabel} onPlay={onPlay} onFavorite={onFavorite} onAddToPlaylist={onAddToPlaylist} onCreatePlaylist={onCreatePlaylist} onRemove={onRemove} />)}</div> : <EmptyState icon={Music2} title={t("empty.title")} body={t("empty.body")} action={t("action.addFirst")} onAction={onImport} className="compact" />}
      <div className="feature-grid"><FeatureCard icon={Library} tone="blue" title={t("feature.libraryTitle")} body={t("feature.libraryBody")} /><FeatureCard icon={ListMusic} tone="purple" title={t("feature.queueTitle")} body={t("feature.queueBody")} /><FeatureCard icon={SlidersHorizontal} tone="orange" title={t("feature.controlTitle")} body={t("feature.controlBody")} /></div>
    </section>
  );
}

function FeatureCard({ icon: Icon, tone, title, body }: { icon: LucideIcon; tone: string; title: string; body: string }) {
  return <article className={cn("feature-card", `feature-${tone}`)}><div className="feature-icon"><Icon size={18} /></div><div><strong>{title}</strong><p>{body}</p></div></article>;
}

function RecentTrack({ locale, track, current, favorite, playlists, removeLabel, onPlay, onFavorite, onAddToPlaylist, onCreatePlaylist, onRemove }: { locale: Locale; track: Track; current: boolean; favorite: boolean; playlists: Playlist[]; removeLabel: string; onPlay: (track: Track) => void; onFavorite: (track: Track) => void; onAddToPlaylist: (track: Track, playlistId: string) => void; onCreatePlaylist: (track: Track) => void; onRemove: (track: Track) => void }) {
  return <button type="button" className={cn("recent-item", current && "is-current")} onClick={() => onPlay(track)}><TrackCover track={track} /><span className="track-primary"><strong>{track.title}</strong><span>{track.artist}</span></span><span className="track-secondary">{track.album || track.file.name}</span><time className="track-duration">{formatTime(track.duration, "--:--")}</time><span onClick={(event) => event.stopPropagation()}><TrackActions locale={locale} favorite={favorite} playlists={playlists} removeLabel={removeLabel} onFavorite={() => onFavorite(track)} onAddToPlaylist={(playlistId) => onAddToPlaylist(track, playlistId)} onCreatePlaylist={() => onCreatePlaylist(track)} onRemove={() => onRemove(track)} /></span></button>;
}

function LibraryView({ locale, title, kicker, summary, tracks, favoriteKeys, playlists, removeLabel, emptyTitle, emptyBody, onImport, onPlay, onFavorite, onAddToPlaylist, onCreatePlaylist, onRemove }: { locale: Locale; title: string; kicker: string; summary: string; tracks: Track[]; favoriteKeys: ReadonlySet<string>; playlists: Playlist[]; removeLabel: string; emptyTitle: string; emptyBody: string; onImport: () => void; onPlay: (track: Track) => void; onFavorite: (track: Track) => void; onAddToPlaylist: (track: Track, playlistId: string) => void; onCreatePlaylist: (track: Track) => void; onRemove: (track: Track) => void }) {
  const t = (key: CopyKey) => textFor(locale, key);
  return <section className="music-view"><div className="library-toolbar"><div><span className="section-kicker">{kicker}</span><h2>{title}</h2></div><div className="toolbar-actions"><button type="button" className="primary-button small-button" onClick={onImport}><Plus size={16} />{t("action.addMusic")}</button></div></div><div className="library-summary"><span>{summary}</span><span className="summary-divider" /><span>{t("library.summaryHint")}</span></div>{tracks.length ? <TrackTable locale={locale} tracks={tracks} favoriteKeys={favoriteKeys} playlists={playlists} removeLabel={removeLabel} onPlay={onPlay} onFavorite={onFavorite} onAddToPlaylist={onAddToPlaylist} onCreatePlaylist={onCreatePlaylist} onRemove={onRemove} /> : <EmptyState icon={Library} title={emptyTitle} body={emptyBody} action={t("action.import")} onAction={onImport} />}</section>;
}

function TrackTable({ locale, tracks, favoriteKeys, playlists, removeLabel, onPlay, onFavorite, onAddToPlaylist, onCreatePlaylist, onRemove }: { locale: Locale; tracks: Track[]; favoriteKeys: ReadonlySet<string>; playlists: Playlist[]; removeLabel: string; onPlay: (track: Track) => void; onFavorite: (track: Track) => void; onAddToPlaylist: (track: Track, playlistId: string) => void; onCreatePlaylist: (track: Track) => void; onRemove: (track: Track) => void }) {
  const t = (key: CopyKey) => textFor(locale, key);
  return <div className="track-table-wrap"><table className="track-table"><thead><tr><th className="index-col">#</th><th>{t("table.title")}</th><th>{t("table.artist")}</th><th>{t("table.album")}</th><th>{t("table.duration")}</th><th /></tr></thead><tbody>{tracks.map((track, index) => <tr key={track.id} className={track.id === tracks[0]?.id ? "" : undefined} onClick={() => onPlay(track)}><td className="track-index">{track.trackNumber ?? index + 1}</td><td><span className="track-primary with-cover"><TrackCover track={track} className="table-cover" /><span><strong>{track.title}</strong><span>{track.file.name}</span></span></span></td><td className="track-secondary">{track.artist}</td><td className="track-secondary album-cell">{track.album || "—"}</td><td className="track-duration">{formatTime(track.duration, "--:--")}</td><td onClick={(event) => event.stopPropagation()}><TrackActions locale={locale} favorite={favoriteKeys.has(track.key)} playlists={playlists} removeLabel={removeLabel} onFavorite={() => onFavorite(track)} onAddToPlaylist={(playlistId) => onAddToPlaylist(track, playlistId)} onCreatePlaylist={() => onCreatePlaylist(track)} onRemove={() => onRemove(track)} /></td></tr>)}</tbody></table></div>;
}

function SettingsView({ locale, languagePreference, themePreference, decoder, decoderPreferences, playback, audioMode, audioDevice, bitPerfectEnabled, bitPerfectPending, onLanguage, onTheme, onAudioMode, onAudioDevice, onBitPerfectChange, onDecoderPreference }: { locale: Locale; languagePreference: LanguagePreference; themePreference: ThemePreference; decoder: DecoderState; decoderPreferences: DecoderPreferences; playback: PlaybackState; audioMode: "shared" | "exclusive"; audioDevice: string; bitPerfectEnabled: boolean; bitPerfectPending: boolean; onLanguage: (value: LanguagePreference) => void; onTheme: (value: ThemePreference) => void; onAudioMode: (value: "shared" | "exclusive") => void; onAudioDevice: (value: string) => void; onBitPerfectChange: (value: boolean) => void; onDecoderPreference: (format: DecoderFormat, preference: DecoderPreference) => void }) {
  const t = (key: CopyKey) => textFor(locale, key);
  const status = decoder.loading ? t("settings.decoderLoading") : decoder.available ? t("settings.decoderReady") : t("settings.decoderUnavailable");
  const deviceOptions = decoder.audio.devices.filter((device) => device.mode === audioMode && device.available !== false).map((device) => [device.id, device.name] as [string, string]);
  const selectedDevice = deviceOptions.some(([id]) => id === audioDevice) ? audioDevice : deviceOptions[0]?.[0] ?? "default";
  const decoderPreferenceRows: Array<[DecoderFormat, CopyKey, string]> = [
    ["flac", "settings.decoderFlac", "Claxon (Rust FLAC)"],
    ["wav", "settings.decoderWav", "Symphonia (Rust)"],
    ["aiff", "settings.decoderAiff", "Symphonia (Rust)"],
    ["alac", "settings.decoderAlac", "Symphonia (Rust)"],
    ["wv", "settings.decoderWv", "libwavpack"],
    ["ape", "settings.decoderApe", "ape-decoder (Rust)"],
    ["other", "settings.decoderOther", "Symphonia when supported; FFmpeg otherwise"],
  ];
  return <section className="music-view"><div className="settings-shell"><div className="settings-heading"><span className="section-kicker">{t("settings.kicker")}</span><h2>{t("nav.settings")}</h2><p>{t("settings.subtitle")}</p></div>
    <section className="settings-card"><div className="settings-section-title">{t("settings.appearanceHeader")}</div><SettingSelect label={t("settings.languageLabel")} description={t("settings.languageDescription")} value={languagePreference} onChange={(value) => onLanguage(value as LanguagePreference)} options={[["system", t("settings.languageSystem")], ["zh", t("settings.languageChinese")], ["en", t("settings.languageEnglish")]]} /><SettingSelect label={t("settings.themeLabel")} description={t("settings.themeDescription")} value={themePreference} onChange={(value) => onTheme(value as ThemePreference)} options={[["system", t("settings.themeSystem")], ["light", t("settings.themeLight")], ["dark", t("settings.themeDark")]]} /></section>
    <section className="settings-card"><div className="settings-section-title">{t("settings.outputHeader")}</div><p className="settings-section-description">{t("settings.outputDescription")}</p><SettingSelect label={t("settings.outputModeLabel")} description={t("settings.outputModeDescription")} value={audioMode} onChange={(value) => onAudioMode(value as "shared" | "exclusive")} options={[["exclusive", t("settings.outputExclusive")], ["shared", t("settings.outputShared")]]} /><SettingSelect label={t("settings.outputDeviceLabel")} description={t("settings.outputDeviceDescription")} value={selectedDevice} onChange={onAudioDevice} options={deviceOptions.length ? deviceOptions : [["default", t("settings.outputUnavailable")]]} /><div className="settings-row"><div><strong>{t("settings.bitPerfectLabel")}</strong><span>{t("settings.bitPerfectDescription")}</span></div><button type="button" role="switch" aria-label={t("settings.bitPerfectLabel")} aria-checked={bitPerfectEnabled} aria-busy={bitPerfectPending} disabled={bitPerfectPending} className={cn("settings-switch", bitPerfectEnabled && "is-on")} onClick={() => onBitPerfectChange(!bitPerfectEnabled)}><span className="settings-switch-knob" />{bitPerfectPending ? t("settings.bitPerfectChecking") : bitPerfectEnabled ? t("settings.bitPerfectDisable") : t("settings.bitPerfectEnable")}</button></div></section>
    <section className="settings-card decoder-card"><div className="settings-section-title">{t("settings.decoderHeader")}</div><p className="settings-section-description">{t("settings.decoderDescription")}</p><div className="decoder-status-line"><span className={cn("decoder-status-dot", decoder.loading ? "" : decoder.available ? "is-ready" : "is-error")} /><strong>{status}</strong><span>{decoder.audio.available ? t("settings.outputReady") : decoder.audio.error || t("settings.outputUnavailable")}</span></div><div className="decoder-details"><div><span>{t("settings.decoderEngineLabel")}</span><strong>{decoder.engine || t("settings.decoderUnknown")}</strong></div><div><span>{t("settings.decoderPathLabel")}</span><strong className="decoder-path">{decoder.path || t("settings.decoderUnknown")}</strong></div></div>
      <div className="decoder-engines">{decoder.engines.map((entry) => <div key={entry.engine}><strong>{entry.engine}</strong><span>{entry.formats.map((format) => format.toUpperCase()).join(" · ")}</span></div>)}</div>
      <div className="decoder-preferences"><div className="settings-section-title">{t("settings.decoderPreferencesHeader")}</div><p className="settings-section-description">{t("settings.decoderPreferencesDescription")}</p><div className="decoder-preference-list">{decoderPreferenceRows.map(([format, label, engine]) => <SettingSelect key={format} label={t(label)} description={engine} value={decoderPreferences[format]} onChange={(value) => onDecoderPreference(format, value as DecoderPreference)} options={[["auto", format === "other" ? t("settings.decoderAuto") : engine], ["ffmpeg", decoder.ffmpegAvailable ? t("settings.decoderFfmpegForced") : t("settings.decoderFfmpegUnavailable"), !decoder.ffmpegAvailable]]} />)}</div></div>
      <div className="decoder-current"><span>{t("settings.currentStream")}</span>{playback.decoder ? <><strong>{playback.decoder}</strong><span>{[playback.sampleRate ? `${playback.sampleRate} Hz` : "", playback.bitsPerSample ? `${playback.bitsPerSample}-bit` : "", playback.channels ? `${playback.channels} ch` : ""].filter(Boolean).join(" · ")}</span><span>{playback.bitPerfectDecode ? t("settings.exactDecode") : playback.sourceLossless === false ? t("settings.lossyDecode") : t("settings.fallbackDecode")}</span><span>{playback.mode === "shared" ? t("settings.sharedQuality") : t("settings.exclusiveQuality")}</span></> : <span>{t("settings.noCurrentStream")}</span>}</div>
      <div className="decoder-formats"><span>{t("settings.decoderFormatsLabel")}</span><div className="decoder-format-list">{decoder.formats.length ? decoder.formats.map((format) => <span key={format} className="decoder-format">{format.toUpperCase()}</span>) : <span className="decoder-format">{decoder.loading ? t("settings.decoderLoading") : t("settings.decoderUnknown")}</span>}</div></div></section>
  </div></section>;
}

function SettingSelect({ label, description, value, onChange, options }: { label: string; description: string; value: string; onChange: (value: string) => void; options: Array<[string, string, boolean?]> }) {
  return <div className="settings-row"><div><strong>{label}</strong><span>{description}</span></div><select className="settings-select" value={value} onChange={(event) => onChange(event.target.value)}>{options.map(([option, text, disabled]) => <option key={option} value={option} disabled={disabled}>{text}</option>)}</select></div>;
}

function NowPlayingSheet({ locale, current, isPlaying, onClose }: { locale: Locale; current: Track | null; isPlaying: boolean; onClose: () => void }) {
  const t = (key: CopyKey) => textFor(locale, key);
  return <>
    <button type="button" className="now-playing-backdrop" aria-label={t("player.collapse")} onClick={onClose} />
    <section className="now-playing-sheet" role="dialog" aria-modal="true" aria-label={t("player.nowPlaying")}>
      <div className="now-playing-header"><div><span className="section-kicker">{t("player.nowPlaying")}</span><span className="now-playing-file">{current?.file.name ?? t("player.choose")}</span></div><IconButton icon={ChevronDown} label={t("player.collapse")} className="now-playing-close" onClick={onClose} /></div>
      <div className="now-playing-body">
        <div className="now-playing-art-column"><div className={cn("now-playing-art", current ? toneClass(current.tone) : "tone-blue", isPlaying && "is-playing")}><VinylArtwork isPlaying={isPlaying} /></div></div>
        <div className="now-playing-info"><span className="section-kicker">TONEY MUSIC</span><h2>{current?.title ?? t("player.nothing")}</h2><p>{current?.artist ?? t("player.choose")}</p><span className="now-playing-meta">{current ? current.album || current.file.name : t("player.choose")}</span></div>
      </div>
    </section>
  </>;
}

function PlayerBar({ locale, current, isPlaying, shuffle, repeat, currentTime, duration, queueOpen, onOpenNowPlaying, onPlay, onPrevious, onNext, onShuffle, onRepeat, onSeek, onQueue, onFavorite, favorite, queueButtonRef }: { locale: Locale; current: Track | null; isPlaying: boolean; shuffle: boolean; repeat: RepeatMode; currentTime: number; duration: number; queueOpen: boolean; onOpenNowPlaying: () => void; onPlay: () => void; onPrevious: () => void; onNext: () => void; onShuffle: () => void; onRepeat: () => void; onSeek: (value: number) => void; onQueue: () => void; onFavorite: () => void; favorite: boolean; queueButtonRef: Ref<HTMLButtonElement> }) {
  const t = (key: CopyKey) => textFor(locale, key);
  const noDuration = t("noDuration");
  return <footer className="music-player is-expandable" data-tooltip={t("player.open")} onClick={onOpenNowPlaying}><div className="player-track"><TrackCover track={current} className="now-cover" /><div className="now-copy"><strong>{current?.title ?? t("player.nothing")}</strong><span>{current?.artist ?? t("player.choose")}</span></div><IconButton icon={Heart} label="Favorite" active={favorite} onClick={onFavorite} /></div><div className="player-center" onClick={(event) => event.stopPropagation()}><div className="player-controls"><IconButton icon={Shuffle} label="Shuffle" active={shuffle} onClick={onShuffle} className="optional-control" /><IconButton icon={SkipBack} label="Previous" onClick={onPrevious} className="optional-control" /><button type="button" className="play-button" onClick={onPlay} aria-label={isPlaying ? "Pause" : "Play"}>{isPlaying ? <Pause size={19} /> : <Play size={19} fill="currentColor" />}</button><IconButton icon={SkipForward} label="Next" onClick={onNext} className="optional-control" /><IconButton icon={Repeat} label="Repeat" active={repeat !== "off"} onClick={onRepeat} className="optional-control" /></div><div className="progress-row"><time>{formatTime(currentTime, "0:00")}</time><input className="progress-range" type="range" min="0" max={String(Math.max(duration, 0.1))} value={Math.min(currentTime, duration || 0)} step="0.1" onChange={(event) => onSeek(Number(event.target.value))} aria-label="Seek" /><time>{formatTime(duration, noDuration)}</time></div></div><div className="player-tools" onClick={(event) => event.stopPropagation()}><IconButton ref={queueButtonRef} icon={ListMusic} label={t("queue.title")} active={queueOpen} onClick={onQueue} /></div></footer>;
}

function QueuePopover({ locale, tracks, currentId, onClose, onPlay, onRemove, onClear, panelRef }: { locale: Locale; tracks: Track[]; currentId: string | null; onClose: () => void; onPlay: (track: Track) => void; onRemove: (track: Track) => void; onClear: () => void; panelRef: Ref<HTMLElement> }) {
  const t = (key: CopyKey) => textFor(locale, key);
  return <><button type="button" className="queue-backdrop" aria-label="Close queue" onClick={onClose} /><aside ref={panelRef} className="queue-panel" aria-label={t("queue.title")}><div className="queue-header"><div><span className="section-kicker">{t("queue.kicker")}</span><h2>{t("queue.title")}</h2></div><IconButton icon={X} label="Close queue" className="small" onClick={onClose} /></div><div className="queue-meta"><span>{tracks.length} {t("songs")}</span><button type="button" className="text-button" onClick={onClear}>{t("action.clear")}</button></div>{tracks.length ? <div className="queue-list">{tracks.map((track) => <button key={track.id} type="button" className={cn("queue-item", track.id === currentId && "is-current")} onClick={() => onPlay(track)}><TrackCover track={track} className="queue-cover" /><span className="queue-item-copy"><strong>{track.title}</strong><span>{track.artist}</span></span><span onClick={(event) => event.stopPropagation()}><IconButton icon={Trash2} label="Remove" className="row-action" onClick={() => onRemove(track)} /></span></button>)}</div> : <div className="queue-empty"><ListMusic size={27} /><p>{t("empty.queue")}</p></div>}</aside></>;
}

function PlaylistDialog({ locale, value, onChange, onClose, onSubmit }: { locale: Locale; value: string; onChange: (value: string) => void; onClose: () => void; onSubmit: () => void }) {
  const t = (key: CopyKey) => textFor(locale, key);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  return <>
    <button type="button" className="playlist-dialog-backdrop" aria-label={t("action.cancel")} onClick={onClose} />
    <section className="playlist-dialog" role="dialog" aria-modal="true" aria-labelledby="playlist-dialog-title">
      <span className="section-kicker">{t("playlist.kicker")}</span>
      <h2 id="playlist-dialog-title">{t("playlist.dialogTitle")}</h2>
      <form onSubmit={(event) => { event.preventDefault(); onSubmit(); }}>
        <input ref={inputRef} value={value} onChange={(event) => onChange(event.target.value)} placeholder={t("playlist.namePlaceholder")} autoComplete="off" />
        <div className="playlist-dialog-actions">
          <button type="button" className="quiet-button" onClick={onClose}>{t("action.cancel")}</button>
          <button type="submit" className="primary-button small-button" disabled={!value.trim()}>{t("action.create")}</button>
        </div>
      </form>
    </section>
  </>;
}

export default function App() {
  const launchParams = new URLSearchParams(window.location.search);
  const initialShellLocale = normalizeShellLocale(launchParams.get("locale") ?? undefined) ?? resolveLocale("system");
  const initialShellTheme = normalizeShellTheme(launchParams.get("theme") ?? undefined) ?? resolveTheme("system");
  const initialLanguagePreference = preferenceFromUrl("languagePreference", ["system", "zh", "en"] as const, readStored<LanguagePreference>("music-language", "system"));
  const initialThemePreference = preferenceFromUrl("appearance", ["system", "light", "dark"] as const, readStored<ThemePreference>("music-theme", "system"));
  const [languagePreference, setLanguagePreference] = useState<LanguagePreference>(initialLanguagePreference);
  const [themePreference, setThemePreference] = useState<ThemePreference>(initialThemePreference);
  const [locale, setLocale] = useState<Locale>(initialLanguagePreference === "system" ? initialShellLocale : initialLanguagePreference);
  const [theme, setTheme] = useState<Theme>(initialThemePreference === "system" ? initialShellTheme : initialThemePreference);
  const [view, setView] = useState<View>("home");
  const [tracks, setTracks] = useState<Track[]>([]);
  const [currentId, setCurrentId] = useState<string | null>(null);
  const [queueIds, setQueueIds] = useState<string[]>([]);
  const [favorites, setFavorites] = useState<Set<string>>(() => new Set(readStored<string[]>("music-favorites", [])));
  const [playlists, setPlaylists] = useState<Playlist[]>(() => readStored<Playlist[]>("music-playlists", []).filter((item) => item && item.id && item.name));
  const [selectedPlaylist, setSelectedPlaylist] = useState("all");
  const [search, setSearch] = useState("");
  const [shuffle, setShuffle] = useState(false);
  const [repeat, setRepeat] = useState<RepeatMode>("off");
  const [queueOpen, setQueueOpen] = useState(false);
  const [nowPlayingOpen, setNowPlayingOpen] = useState(false);
  const [decoder, setDecoder] = useState<DecoderState>(DEFAULT_DECODER);
  const [decoderPreferences, setDecoderPreferences] = useState<DecoderPreferences>(DEFAULT_DECODER_PREFERENCES);
  const [playback, setPlayback] = useState<PlaybackState>(DEFAULT_PLAYBACK);
  const [audioMode, setAudioMode] = useState<"shared" | "exclusive">(() => readStored("music-output-mode", "exclusive"));
  const [audioDevice, setAudioDevice] = useState<string>(() => readStored("music-output-device", "default"));
  const [bitPerfectEnabled, setBitPerfectEnabled] = useState(false);
  const [bitPerfectPending, setBitPerfectPending] = useState(false);
  const [currentTime, setCurrentTime] = useState(0);
  const [duration, setDuration] = useState(0);
  const [isPlaying, setIsPlaying] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const [importing, setImporting] = useState(false);
  const [playlistDialog, setPlaylistDialog] = useState<{ trackId: string | null; navigateToPlaylist: boolean } | null>(null);
  const [playlistDraft, setPlaylistDraft] = useState("");
  const scrollRef = useRef<HTMLDivElement>(null);
  const queueButtonRef = useRef<HTMLButtonElement>(null);
  const queuePanelRef = useRef<HTMLElement>(null);
  const toastTimer = useRef<number | undefined>(undefined);
  const seekTimer = useRef<number | undefined>(undefined);
  const endedTrackRef = useRef<string | null>(null);
  const playbackErrorRef = useRef<string | null>(null);
  const shellLocaleRef = useRef<Locale>(initialShellLocale);
  const shellThemeRef = useRef<Theme>(initialShellTheme);

  const t = useCallback((key: CopyKey, values?: Record<string, string | number>) => textFor(locale, key, values), [locale]);
  const current = useMemo(() => tracks.find((track) => track.id === currentId) ?? null, [tracks, currentId]);
  const playbackTracks = useMemo(() => {
    const queued = queueIds.map((id) => tracks.find((track) => track.id === id)).filter((track): track is Track => Boolean(track));
    return queued.length ? queued : tracks;
  }, [queueIds, tracks]);
  const recent = useMemo(() => [...tracks].sort((left, right) => right.addedAt - left.addedAt).slice(0, 6), [tracks]);
  const filterTracks = useCallback((targetView: View) => {
    let next = [...tracks];
    if (targetView === "favorites") next = next.filter((track) => favorites.has(track.key));
    if (targetView === "playlist" && selectedPlaylist !== "all") {
      const playlist = playlists.find((item) => item.id === selectedPlaylist);
      next = playlist ? next.filter((track) => playlist.keys.includes(track.key)) : [];
    }
    const query = search.trim().toLowerCase();
    if (query) next = next.filter((track) => `${track.title} ${track.artist} ${track.album} ${track.file.name}`.toLowerCase().includes(query));
    return next.sort((left, right) => left.addedAt - right.addedAt);
  }, [favorites, playlists, search, selectedPlaylist, tracks]);
  const libraryTracks = useMemo(() => filterTracks("library"), [filterTracks]);
  const favoriteTracks = useMemo(() => filterTracks("favorites"), [filterTracks]);
  const playlistTracks = useMemo(() => filterTracks("playlist"), [filterTracks]);

  const notify = useCallback((message: string, duration = 2400) => {
    setToast(message);
    if (toastTimer.current) window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), duration);
  }, []);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    document.documentElement.lang = locale === "zh" ? "zh-CN" : "en";
  }, [locale, theme]);

  useEffect(() => {
    const heartbeat = () => { void heartbeatPlaybackUI().catch(() => {}); };
    heartbeat();
    const timer = window.setInterval(heartbeat, 1500);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    let cancelled = false;
    const refresh = async () => {
      try {
        const next = await fetchPlayback();
        if (cancelled) return;
        setPlayback(next);
        setIsPlaying(next.state === "playing");
        setCurrentTime(next.currentTime || 0);
        setDuration(next.duration || 0);
        if (next.trackId) setCurrentId(next.trackId);
        if (next.duration > 0 && next.trackId) {
          setTracks((items) => items.map((track) => track.id === next.trackId && track.duration !== next.duration ? { ...track, duration: next.duration } : track));
        }
        if (next.error && next.error !== playbackErrorRef.current) notify(next.error);
        playbackErrorRef.current = next.error || null;
      } catch {
        // The first status load reports the service error; later polls can recover on their own.
      }
    };
    void refresh();
    const timer = window.setInterval(() => void refresh(), 400);
    return () => { cancelled = true; window.clearInterval(timer); };
  }, [notify]);

  useEffect(() => {
    let cancelled = false;
    void Promise.all([
      fetchTracks().catch((error) => { notify(t("backendUnavailable", { error: error.message })); return []; }),
      fetchDecoderStatus().catch((error) => ({ ...DEFAULT_DECODER, loading: false, error: error.message })),
      fetchDecoderPreferences().catch(() => DEFAULT_DECODER_PREFERENCES),
      fetchPlayback().catch(() => DEFAULT_PLAYBACK),
      fetchBitPerfectSettings().catch(() => ({ enabled: false })),
    ]).then(([loadedTracks, loadedDecoder, loadedDecoderPreferences, loadedPlayback, loadedBitPerfect]) => {
      if (cancelled) return;
      setTracks(loadedTracks);
      setDecoder(loadedDecoder);
      setDecoderPreferences(loadedDecoderPreferences);
      setBitPerfectEnabled(loadedBitPerfect.enabled);
      setPlayback(loadedPlayback);
      setCurrentTime(loadedPlayback.currentTime || 0);
      setDuration(loadedPlayback.duration || 0);
      setIsPlaying(loadedPlayback.state === "playing");
      if (loadedPlayback.trackId) setCurrentId(loadedPlayback.trackId);
      const preferredMode = loadedBitPerfect.enabled ? "exclusive" : audioMode;
      if (loadedBitPerfect.enabled) {
        setAudioMode("exclusive");
        store("music-output-mode", "exclusive");
      }
      const modeDevices = loadedDecoder.audio.devices.filter((device) => device.mode === preferredMode && device.available !== false);
      if (modeDevices.length && (loadedBitPerfect.enabled || audioDevice === "default" || !modeDevices.some((device) => device.id === audioDevice))) {
        setAudioDevice(modeDevices[0].id);
        if (loadedBitPerfect.enabled) store("music-output-device", modeDevices[0].id);
      }
      setQueueIds((ids) => {
        const valid = ids.filter((id) => loadedTracks.some((track) => track.id === id));
        return valid.length ? valid : loadedTracks.map((track) => track.id);
      });
      if (loadedTracks.length && !loadedPlayback.trackId && !currentId) {
        setCurrentId(loadedTracks[0].id);
      }
    });
    return () => { cancelled = true; };
    // Load the local library once when the external app opens.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const onMessage = (event: MessageEvent) => {
      const message = event.data as ShellContextMessage | null;
      if (!message) return;
      const isExternalHostContext = message.channel === "agentos:host" && message.type === "context-changed";
      const nextLocale = isExternalHostContext ? normalizeShellLocale(message.locale) : message.type === "shell:locale" ? normalizeShellLocale(message.locale) : null;
      const nextTheme = isExternalHostContext ? normalizeShellTheme(message.theme ?? message.appearance) : (message.type === "shell:theme" || message.type === "shell:appearance") ? normalizeShellTheme(message.appearance) : null;
      if (nextLocale) {
        shellLocaleRef.current = nextLocale;
        if (languagePreference === "system") setLocale(nextLocale);
      }
      if (nextTheme) {
        shellThemeRef.current = nextTheme;
        if (themePreference === "system") setTheme(nextTheme);
      }
    };
    window.addEventListener?.("message", onMessage);
    return () => window.removeEventListener?.("message", onMessage);
  }, [languagePreference, themePreference]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement;
      if (["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)) {
        if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") { event.preventDefault(); document.querySelector<HTMLInputElement>(".search-box input")?.focus(); }
        return;
      }
      if (event.key === "Escape" && nowPlayingOpen) { event.preventDefault(); setNowPlayingOpen(false); }
      else if (event.code === "Space") { event.preventDefault(); togglePlay(); }
      else if ((event.metaKey || event.ctrlKey) && event.key === ",") { event.preventDefault(); setView("settings"); }
      else if (event.key.toLowerCase() === "f") document.querySelector<HTMLInputElement>(".search-box input")?.focus();
    };
    document.addEventListener?.("keydown", onKeyDown);
    return () => document.removeEventListener?.("keydown", onKeyDown);
  });

  const setCurrent = useCallback((track: Track, autoplay: boolean) => {
    setCurrentId(track.id);
    setQueueIds((ids) => ids.includes(track.id) ? ids : [...ids, track.id]);
    if (autoplay) {
      void startPlayback(track.id, 0, audioMode, audioDevice, true)
        .then((next) => { setPlayback(next); setDuration(next.duration); setCurrentTime(0); setIsPlaying(true); })
        .catch((error) => notify(error instanceof Error ? error.message : String(error)));
    } else {
      setCurrentTime(0);
      setDuration(track.duration || 0);
    }
  }, [audioDevice, audioMode, notify]);

  const togglePlay = useCallback(() => {
    if (isPlaying) {
      void pausePlayback().then(setPlayback).catch((error) => notify(error instanceof Error ? error.message : String(error)));
    } else if (playback.state === "paused") {
      void resumePlayback().then(setPlayback).catch((error) => notify(error instanceof Error ? error.message : String(error)));
    } else {
      const next = current ?? playbackTracks[0];
      if (!next) { notify(t("empty.body")); return; }
      setCurrent(next, true);
    }
  }, [current, isPlaying, notify, playback.state, playbackTracks, setCurrent, t]);

  const move = useCallback((delta: number) => {
    if (!playbackTracks.length) return;
    const currentIndex = Math.max(0, playbackTracks.findIndex((track) => track.id === currentId));
    let nextIndex = currentIndex + delta;
    if (shuffle && delta > 0 && playbackTracks.length > 1) { nextIndex = Math.floor(Math.random() * playbackTracks.length); if (nextIndex === currentIndex) nextIndex = (nextIndex + 1) % playbackTracks.length; }
    if (nextIndex < 0) nextIndex = playbackTracks.length - 1;
    if (nextIndex >= playbackTracks.length) { if (repeat === "all") nextIndex = 0; else { void stopPlayback(); setCurrentTime(0); setIsPlaying(false); return; } }
    const next = playbackTracks[nextIndex];
    if (next) setCurrent(next, true);
  }, [currentId, playbackTracks, repeat, setCurrent, shuffle]);

  useEffect(() => {
    if (playback.state !== "ended" || !playback.trackId) {
      endedTrackRef.current = null;
      return;
    }
    if (endedTrackRef.current === playback.trackId) return;
    endedTrackRef.current = playback.trackId;
    if (repeat === "one") {
      const sameTrack = tracks.find((track) => track.id === playback.trackId);
      if (sameTrack) setCurrent(sameTrack, true);
      return;
    }
    const index = Math.max(0, playbackTracks.findIndex((track) => track.id === playback.trackId));
    let nextIndex = index + 1;
    if (shuffle && playbackTracks.length > 1) nextIndex = Math.floor(Math.random() * playbackTracks.length);
    if (nextIndex >= playbackTracks.length) {
      if (repeat === "all") nextIndex = 0;
      else { void stopPlayback(); setCurrentTime(0); setIsPlaying(false); return; }
    }
    const next = playbackTracks[nextIndex];
    if (next) setCurrent(next, true);
  }, [playback.state, playback.trackId, playbackTracks, repeat, setCurrent, shuffle, tracks]);

  const importFiles = useCallback(async (fileList: FileList | File[], options: ImportOptions = {}) => {
    const selectedFiles = Array.from(fileList).filter(isImportFile);
    const audioFiles = selectedFiles.filter(isAudioFile);
    const cueFiles = selectedFiles.filter((file) => CUE_EXTENSION.test(file.name));
    if (!audioFiles.length) { notify(cueFiles.length ? t("cueSelection") : t("unsupported")); return; }
    if (cueFiles.length && (cueFiles.length !== 1 || audioFiles.length !== 1)) {
      notify(t("cueSelection"));
      return;
    }
    setImporting(true);
    try {
      if (options.saveToFenn) {
        await uploadFilesToFennFolder(selectedFiles, FENN_MUSIC_FOLDER);
      }
      const added: Track[] = [];
      for (const file of audioFiles) {
        try {
          const imported = await uploadTrack(file, cueFiles[0]);
          for (const track of imported) {
            if (!tracks.some((item) => item.id === track.id) && !added.some((item) => item.id === track.id)) added.push(track);
          }
        } catch (error) {
          notify(t("importFailed", { name: file.name, error: error instanceof Error ? error.message : String(error) }));
        }
      }
      if (added.length) {
        setTracks((items) => [...items, ...added]);
        setQueueIds((ids) => [...ids, ...added.map((track) => track.id).filter((id) => !ids.includes(id))]);
        if (options.autoplayFirst) setCurrent(added[0], true);
        else if (!currentId) setCurrent(added[0], false);
        notify(t("added", { count: added.length }));
      } else if (!toast) notify(t("duplicate"));
    } catch (error) {
      notify(t("importFailed", {
        name: options.saveToFenn ? FENN_MUSIC_FOLDER : "musicd",
        error: error instanceof Error ? error.message : String(error),
      }));
    } finally {
      setImporting(false);
    }
  }, [currentId, notify, setCurrent, t, toast, tracks]);

  const onFiles = useCallback((fileList: FileList | File[]) => importFiles(fileList), [importFiles]);

  const importFennFiles = useCallback(async (entries: FennFileDragItem[]) => {
    const selected = entries
      .filter((entry) => entry.kind === "file")
      .filter(isAudioPickerFile);
    if (!selected.length) {
      notify(t("unsupported"));
      return;
    }
    try {
      const files: File[] = [];
      for (const selectedFile of selected) {
        const mediaUrl = await filePickerFileToMediaUrl(selectedFile);
        const response = await fetch(mediaUrl, { credentials: "include", cache: "no-store" });
        if (!response.ok) throw new Error(`${selectedFile.name}: ${response.status}`);
        const blob = await response.blob();
        files.push(new File([blob], selectedFile.name, {
          type: selectedFile.mimeType || blob.type || "application/octet-stream",
          lastModified: Date.now(),
        }));
      }
      // Fenn Files already owns the source file. The music service receives a
      // copy for its library, but the drag operation should not create a
      // second copy in Fenn Files/Music.
      await onFiles(files);
    } catch (error) {
      notify(t("importFailed", {
        name: "Fenn Files",
        error: error instanceof Error ? error.message : String(error),
      }));
    }
  }, [notify, onFiles, t]);

  const onImport = useCallback(async () => {
    try {
      const result = await openFilePicker({
        mode: "open",
        accept: AUDIO_PICKER_ACCEPT,
        multiple: true,
      });
      if (!result.ok || result.canceled) return;
      const selected = result.files.filter(isAudioPickerFile);
      if (!selected.length) {
        notify(t("unsupported"));
        return;
      }

      setImporting(true);
      const files: File[] = [];
      for (const selectedFile of selected) {
        const mediaUrl = await filePickerFileToMediaUrl(selectedFile);
        const response = await fetch(mediaUrl, { credentials: "include", cache: "no-store" });
        if (!response.ok) throw new Error(`${selectedFile.name}: ${response.status}`);
        const blob = await response.blob();
        files.push(new File([blob], selectedFile.name, {
          type: selectedFile.mimeType || blob.type || "application/octet-stream",
          lastModified: Date.now(),
        }));
      }
      await importFiles(files, { saveToFenn: true });
    } catch (error) {
      notify(t("importFailed", {
        name: "Fenn Files",
        error: error instanceof Error ? error.message : String(error),
      }));
    } finally {
      setImporting(false);
    }
  }, [importFiles, notify, t]);

  const onDrop = (event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    setDragging(false);
    const fenn = readFennFileDrag(event.dataTransfer);
    if (fenn) {
      void importFennFiles(fenn.files);
      return;
    }
    if (event.dataTransfer.files.length) void importFiles(event.dataTransfer.files, { saveToFenn: true, autoplayFirst: true });
  };
  const onDragOver = (event: DragEvent<HTMLDivElement>) => { event.preventDefault(); setDragging(true); };
  const onDragLeave = (event: DragEvent<HTMLDivElement>) => { if (event.currentTarget === event.target) setDragging(false); };

  const toggleFavorite = useCallback((track: Track | null) => {
    if (!track) return;
    setFavorites((items) => { const next = new Set(items); if (next.has(track.key)) next.delete(track.key); else next.add(track.key); store("music-favorites", Array.from(next)); return next; });
  }, []);

  const removeTrack = useCallback(async (track: Track) => {
    try { await deleteTrack(track.id); } catch (error) { notify(t("removeFailed", { error: error instanceof Error ? error.message : String(error) })); return; }
    const remaining = tracks.filter((item) => item.id !== track.id);
    const nextPlaylists = playlists.map((item) => ({ ...item, keys: item.keys.filter((key) => key !== track.key) }));
    setTracks(remaining); setQueueIds((ids) => ids.filter((id) => id !== track.id)); setPlaylists(nextPlaylists); store("music-playlists", nextPlaylists);
    if (track.id === currentId) { void stopPlayback(); setCurrentTime(0); setIsPlaying(false); const next = remaining[0]; setCurrentId(next?.id ?? null); if (next) setCurrent(next, false); }
    notify(t("removed"));
  }, [currentId, notify, playlists, setCurrent, t, tracks]);

  const openCreatePlaylist = useCallback((trackId: string | null = null, navigateToPlaylist = false) => {
    setPlaylistDraft("");
    setPlaylistDialog({ trackId, navigateToPlaylist });
  }, []);

  const createPlaylist = useCallback(() => {
    openCreatePlaylist(null, true);
  }, [openCreatePlaylist]);

  const addTrackToPlaylist = useCallback((track: Track, playlistId: string) => {
    const playlist = playlists.find((item) => item.id === playlistId);
    if (!playlist) return;
    if (playlist.keys.includes(track.key)) {
      notify(t("playlistAlreadyContains", { name: playlist.name }));
      return;
    }
    const nextPlaylists = playlists.map((item) => item.id === playlist.id ? { ...item, keys: [...item.keys, track.key] } : item);
    setPlaylists(nextPlaylists);
    store("music-playlists", nextPlaylists);
    notify(t("addedToPlaylist", { name: playlist.name }));
  }, [notify, playlists, t]);

  const removeTrackFromPlaylist = useCallback((track: Track, playlistId: string) => {
    const playlist = playlists.find((item) => item.id === playlistId);
    if (!playlist || !playlist.keys.includes(track.key)) return;
    const nextPlaylists = playlists.map((item) => item.id === playlist.id ? { ...item, keys: item.keys.filter((key) => key !== track.key) } : item);
    setPlaylists(nextPlaylists);
    store("music-playlists", nextPlaylists);
    notify(t("removedFromPlaylist", { name: playlist.name }));
  }, [notify, playlists, t]);

  const removeFavorite = useCallback((track: Track) => {
    setFavorites((items) => {
      if (!items.has(track.key)) return items;
      const next = new Set(items);
      next.delete(track.key);
      store("music-favorites", Array.from(next));
      return next;
    });
    notify(t("unfavorited"));
  }, [notify, t]);

  const confirmCreatePlaylist = useCallback(() => {
    const name = playlistDraft.trim();
    if (!name || !playlistDialog) return;
    const track = playlistDialog.trackId ? tracks.find((item) => item.id === playlistDialog.trackId) ?? null : null;
    const playlist = { id: `playlist-${Date.now()}`, name: name.slice(0, 32), keys: track ? [track.key] : [] };
    const nextPlaylists = [...playlists, playlist];
    setPlaylists(nextPlaylists);
    store("music-playlists", nextPlaylists);
    if (playlistDialog.navigateToPlaylist) {
      setSelectedPlaylist(playlist.id);
      setView("playlist");
    }
    setPlaylistDialog(null);
    setPlaylistDraft("");
    notify(track ? t("addedToPlaylist", { name: playlist.name }) : t("playlistCreated"));
  }, [notify, playlistDialog, playlistDraft, playlists, t, tracks]);

  const showView = (nextView: View) => { setView(nextView); requestAnimationFrame(() => { if (scrollRef.current) scrollRef.current.scrollTop = 0; }); };
  const selectPlaylist = (id: string) => { setSelectedPlaylist(id); showView("playlist"); };
  const setLanguage = (value: LanguagePreference) => { setLanguagePreference(value); setLocale(value === "system" ? shellLocaleRef.current : value); store("music-language", value); };
  const setThemePreferenceValue = (value: ThemePreference) => { setThemePreference(value); setTheme(value === "system" ? shellThemeRef.current : value); store("music-theme", value); };
  const setBitPerfectMode = async (enabled: boolean) => {
    if (bitPerfectPending) return;
    setBitPerfectPending(true);
    try {
      if (enabled) {
        const track = current ?? tracks[0];
        if (!track) throw new Error(t("settings.bitPerfectNoTrack"));
        const device = decoder.audio.devices.find((item) => item.mode === "exclusive" && item.available !== false);
        if (!device) throw new Error(t("settings.bitPerfectNoDevice"));
        const settings = await updateBitPerfectSettings(true, track.id, device.id);
        setBitPerfectEnabled(settings.enabled);
        setAudioMode("exclusive");
        setAudioDevice(device.id);
        store("music-output-mode", "exclusive");
        store("music-output-device", device.id);
        notify(t("settings.bitPerfectEnabled"), 4000);
      } else {
        const settings = await updateBitPerfectSettings(false);
        setBitPerfectEnabled(settings.enabled);
        notify(t("settings.bitPerfectDisabled"), 3000);
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      notify(t(enabled ? "settings.bitPerfectEnableFailed" : "settings.bitPerfectDisableFailed", { error: message }), 6000);
    } finally {
      setBitPerfectPending(false);
    }
  };
  const setOutputMode = (value: "shared" | "exclusive") => {
    if (bitPerfectEnabled && value !== "exclusive") {
      notify(t("settings.bitPerfectRequiresExclusive"), 4000);
      return;
    }
    setAudioMode(value);
    store("music-output-mode", value);
    const nextDevice = decoder.audio.devices.find((device) => device.mode === value);
    if (nextDevice) {
      setAudioDevice(nextDevice.id);
      store("music-output-device", nextDevice.id);
    } else {
      setAudioDevice("default");
      store("music-output-device", "default");
    }
  };
  const setOutputDevice = async (value: string) => {
    if (bitPerfectEnabled) {
      if (bitPerfectPending) return;
      const track = current ?? tracks[0];
      if (!track) {
        notify(t("settings.bitPerfectEnableFailed", { error: t("settings.bitPerfectNoTrack") }), 6000);
        return;
      }
      setBitPerfectPending(true);
      try {
        await updateBitPerfectSettings(true, track.id, value);
        setAudioDevice(value);
        store("music-output-device", value);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        notify(t("settings.bitPerfectEnableFailed", { error: message }), 6000);
      } finally {
        setBitPerfectPending(false);
      }
      return;
    }
    setAudioDevice(value);
    store("music-output-device", value);
  };
  const saveDecoderPreference = useCallback((format: DecoderFormat, preference: DecoderPreference) => {
    void setDecoderPreference(format, preference)
      .then(setDecoderPreferences)
      .catch((error: unknown) => notify(`${t("settings.decoderSaveError")}: ${error instanceof Error ? error.message : String(error)}`));
  }, [notify, t]);
  const onSeek = useCallback((value: number) => {
    setCurrentTime(value);
    if (seekTimer.current) window.clearTimeout(seekTimer.current);
    seekTimer.current = window.setTimeout(() => {
      void seekPlayback(value).then(setPlayback).catch((error) => notify(error instanceof Error ? error.message : String(error)));
    }, 250);
  }, [notify]);
  const viewTitle = view === "library" ? t("nav.library") : view === "favorites" ? t("nav.favorites") : view === "settings" ? t("nav.settings") : view === "playlist" ? selectedPlaylist === "all" ? t("playlist.all") : playlists.find((item) => item.id === selectedPlaylist)?.name ?? t("playlist.all") : t("nav.home");
  const viewSubtitle = view === "home" ? t("subtitle.home") : view === "library" ? t("subtitle.library") : view === "favorites" ? t("subtitle.favorites") : view === "settings" ? t("subtitle.settings") : t("subtitle.playlist");
  const queueTracks = queueIds.map((id) => tracks.find((track) => track.id === id)).filter((track): track is Track => Boolean(track));
  const favoritesCount = tracks.filter((track) => favorites.has(track.key)).length;
  const openNowPlaying = () => { setQueueOpen(false); setNowPlayingOpen(true); };
  const toggleQueue = () => setQueueOpen((value) => !value);
  const playlistIsScoped = view === "playlist" && selectedPlaylist !== "all";

  return <div className={cn("music-shell", dragging && "is-dragging")} onDrop={onDrop} onDragOver={onDragOver} onDragLeave={onDragLeave}>
    <Sidebar locale={locale} view={view} selectedPlaylist={selectedPlaylist} tracks={tracks} favoriteCount={favoritesCount} playlists={playlists} onView={showView} onPlaylist={selectPlaylist} onCreatePlaylist={createPlaylist} onSettings={() => showView("settings")} onImport={onImport} />
    <main className="music-main"><TopBar locale={locale} view={view} title={viewTitle} subtitle={viewSubtitle} search={search} onSearch={setSearch} onClear={() => setSearch("")} /><div ref={scrollRef} className="music-scroll">
      {view === "home" ? <HomeView locale={locale} tracks={tracks} recent={recent} currentId={currentId} favoriteKeys={favorites} playlists={playlists} removeLabel={t("action.removeFromLibrary")} onImport={onImport} onBrowse={() => showView("library")} onPlay={(track) => setCurrent(track, true)} onFavorite={toggleFavorite} onAddToPlaylist={addTrackToPlaylist} onCreatePlaylist={(track) => openCreatePlaylist(track.id)} onRemove={removeTrack} /> : null}
      {view === "library" ? <LibraryView locale={locale} title={t("nav.library")} kicker={t("library.kicker")} summary={`${libraryTracks.length} ${t("songs")}`} tracks={libraryTracks} favoriteKeys={favorites} playlists={playlists} removeLabel={t("action.removeFromLibrary")} emptyTitle={t("empty.libraryTitle")} emptyBody={t("empty.libraryBody")} onImport={onImport} onPlay={(track) => setCurrent(track, true)} onFavorite={toggleFavorite} onAddToPlaylist={addTrackToPlaylist} onCreatePlaylist={(track) => openCreatePlaylist(track.id)} onRemove={removeTrack} /> : null}
      {view === "favorites" ? <LibraryView locale={locale} title={t("nav.favorites")} kicker={t("favorites.kicker")} summary={`${favoriteTracks.length} ${t("songs")}`} tracks={favoriteTracks} favoriteKeys={favorites} playlists={playlists} removeLabel={t("action.removeFromFavorites")} emptyTitle={t("empty.favoriteTitle")} emptyBody={t("empty.favoriteBody")} onImport={onImport} onPlay={(track) => setCurrent(track, true)} onFavorite={toggleFavorite} onAddToPlaylist={addTrackToPlaylist} onCreatePlaylist={(track) => openCreatePlaylist(track.id)} onRemove={removeFavorite} /> : null}
      {view === "playlist" ? <LibraryView locale={locale} title={viewTitle} kicker={t("playlist.kicker")} summary={`${playlistTracks.length} ${t("songs")}`} tracks={playlistTracks} favoriteKeys={favorites} playlists={playlists} removeLabel={playlistIsScoped ? t("action.removeFromPlaylist") : t("action.removeFromLibrary")} emptyTitle={t("empty.playlistTitle")} emptyBody={t("empty.playlistBody")} onImport={onImport} onPlay={(track) => setCurrent(track, true)} onFavorite={toggleFavorite} onAddToPlaylist={addTrackToPlaylist} onCreatePlaylist={(track) => openCreatePlaylist(track.id)} onRemove={playlistIsScoped ? (track) => removeTrackFromPlaylist(track, selectedPlaylist) : removeTrack} /> : null}
      {view === "settings" ? <SettingsView locale={locale} languagePreference={languagePreference} themePreference={themePreference} decoder={decoder} decoderPreferences={decoderPreferences} playback={playback} audioMode={audioMode} audioDevice={audioDevice} bitPerfectEnabled={bitPerfectEnabled} bitPerfectPending={bitPerfectPending} onLanguage={setLanguage} onTheme={setThemePreferenceValue} onAudioMode={setOutputMode} onAudioDevice={setOutputDevice} onBitPerfectChange={setBitPerfectMode} onDecoderPreference={saveDecoderPreference} /> : null}
    </div><PlayerBar locale={locale} current={current} isPlaying={isPlaying} shuffle={shuffle} repeat={repeat} currentTime={currentTime} duration={duration} queueOpen={queueOpen} onOpenNowPlaying={openNowPlaying} onPlay={togglePlay} onPrevious={() => move(-1)} onNext={() => move(1)} onShuffle={() => { setShuffle((value) => !value); notify(t(shuffle ? "shuffleOff" : "shuffleOn")); }} onRepeat={() => { const next = repeat === "off" ? "all" : repeat === "all" ? "one" : "off"; setRepeat(next); notify(t(next === "one" ? "repeatOne" : next === "all" ? "repeatAll" : "repeatOff")); }} onSeek={onSeek} onQueue={toggleQueue} onFavorite={() => toggleFavorite(current)} favorite={Boolean(current && favorites.has(current.key))} queueButtonRef={queueButtonRef} /></main>
    {nowPlayingOpen ? <NowPlayingSheet locale={locale} current={current} isPlaying={isPlaying} onClose={() => setNowPlayingOpen(false)} /> : null}
    {queueOpen ? <QueuePopover locale={locale} tracks={queueTracks} currentId={currentId} onClose={() => setQueueOpen(false)} onPlay={(track) => setCurrent(track, true)} onRemove={removeTrack} onClear={() => { setQueueIds([]); notify(t("queueCleared")); }} panelRef={queuePanelRef} /> : null}
    {playlistDialog ? <PlaylistDialog locale={locale} value={playlistDraft} onChange={setPlaylistDraft} onClose={() => { setPlaylistDialog(null); setPlaylistDraft(""); }} onSubmit={confirmCreatePlaylist} /> : null}
    {dragging ? <div className="drop-overlay"><div className="drop-card"><Upload size={28} /><strong>{t("drop.title")}</strong><p>{t("drop.body")}</p></div></div> : null}
    {toast ? <div className="toast">{toast}</div> : null}
    {importing ? <div className="import-progress" role="status">{t("action.import")}…</div> : null}
    <span className="sr-only">{favoritesCount} favorites</span>
  </div>;
}
