import {
  ChevronDown,
  Film,
  FolderOpen,
  Fullscreen,
  Heart,
  ListVideo,
  Maximize2,
  Minimize2,
  Monitor,
  Moon,
  MonitorPlay,
  Pause,
  PictureInPicture2,
  Play,
  RotateCcw,
  RotateCw,
  Repeat,
  Repeat1,
  Search,
  Sun,
  Trash2,
  Upload,
  Video,
  Volume2,
  VolumeX,
  X,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { CSSProperties, DragEvent, MouseEvent, PointerEvent as ReactPointerEvent, ReactNode } from "react";
import { callOpenTool, getOpenToolBaseUrl } from "./opentool";
import { openVideoSelection } from "./local-files";

const logoUrl = new URL("../icon.png", import.meta.url).href;

type Locale = "zh" | "en";
type Theme = "dark" | "light";
type ThemePreference = "system" | "light" | "dark";
type PlaylistView = "all" | "recent" | "favorites";
type FitMode = "contain" | "cover";
type PlaybackMode = "sequential" | "current" | "list";

type Track = {
  id: string;
  key: string;
  file: File;
  backendStored?: boolean;
  size: number;
  name: string;
  url: string;
  duration: number;
  addedAt: number;
  tone: number;
};

type LibraryVideo = {
  id: string;
  key: string;
  name: string;
  size: number;
  addedAt: number;
  favorite: boolean;
};

const COPY = {
  zh: {
    brand: "Frame视频",
    subtitle: "本地视频播放器",
    open: "打开视频",
    add: "添加",
    playlist: "播放列表",
    all: "全部",
    recent: "最近播放",
    favorites: "稍后观看",
    emptyTitle: "打开一个视频开始播放",
    emptyBody: "视频只在本机播放，不会上传。",
    emptyPlaylist: "播放列表还是空的",
    emptyPlaylistBody: "打开视频后，它们会按照加入顺序出现在这里。",
    noMatches: "没有找到匹配的视频",
    search: "搜索视频",
    noVideo: "尚未选择视频",
    openHint: "选择左侧画面，或从右侧列表播放",
    local: "本地播放",
    fit: "适应",
    fill: "填满",
    speed: "播放速度",
    playbackMode: "播放模式",
    sequential: "顺序播放",
    currentLoop: "当前循环",
    listLoop: "列表循环",
    clear: "清空列表",
    play: "播放",
    pause: "暂停",
    mute: "静音",
    unmute: "取消静音",
    previous: "上一个",
    next: "下一个",
    back: "后退 10 秒",
    forward: "前进 10 秒",
    pictureInPicture: "画中画",
    fullscreen: "全屏",
    exitFullscreen: "退出全屏",
    fullscreenUnavailable: "当前环境不支持全屏",
    fullscreenRequestFailed: "无法进入全屏模式",
    fullscreenExitFailed: "无法退出全屏模式",
    added: "已添加 {count} 个视频",
    unsupported: "没有找到可播放的视频文件",
    duplicate: "这些视频已经在列表里了",
    removed: "视频已移除",
    cleared: "播放列表已清空",
    pipUnsupported: "当前环境不支持画中画",
    playbackSettings: "播放设置已在底部控制栏提供",
    fileInfo: "{size} · {duration}",
    files: "个文件",
    favorite: "稍后观看",
    unfavorite: "移除稍后观看",
    dropTitle: "松开以导入视频",
    dropBody: "拖入的视频会在本机加入播放列表",
    importFailed: "导入失败：{error}",
    loading: "正在加载视频…",
    unsupportedVideo: "这个视频暂时无法播放",
    shortcuts: "Space 播放 / 暂停 · F 全屏 · ← → 快进 / 快退",
    themeSystem: "跟随系统",
    themeLight: "浅色主题",
    themeDark: "深色主题",
  },
  en: {
    brand: "Frame Video",
    subtitle: "Local video player",
    open: "Open video",
    add: "Add",
    playlist: "Playlist",
    all: "All",
    recent: "Recent",
    favorites: "Watch later",
    emptyTitle: "Open a video to start",
    emptyBody: "Videos play locally and never leave this device.",
    emptyPlaylist: "The playlist is empty",
    emptyPlaylistBody: "Open videos and they will appear here in the order you add them.",
    noMatches: "No matching videos",
    search: "Search videos",
    noVideo: "No video selected",
    openHint: "Choose the stage on the left or play from the list",
    local: "LOCAL PLAYBACK",
    fit: "Fit",
    fill: "Fill",
    speed: "Playback speed",
    playbackMode: "Playback mode",
    sequential: "Play in order",
    currentLoop: "Loop current",
    listLoop: "Loop playlist",
    clear: "Clear playlist",
    play: "Play",
    pause: "Pause",
    mute: "Mute",
    unmute: "Unmute",
    previous: "Previous",
    next: "Next",
    back: "Back 10 seconds",
    forward: "Forward 10 seconds",
    pictureInPicture: "Picture in picture",
    fullscreen: "Fullscreen",
    exitFullscreen: "Exit fullscreen",
    fullscreenUnavailable: "Fullscreen is not available in this environment",
    fullscreenRequestFailed: "Could not enter fullscreen mode",
    fullscreenExitFailed: "Could not exit fullscreen mode",
    added: "Added {count} videos",
    unsupported: "No playable video files found",
    duplicate: "Those videos are already in the list",
    removed: "Video removed",
    cleared: "Playlist cleared",
    pipUnsupported: "Picture in picture is not available here",
    playbackSettings: "Playback settings are in the bottom control bar",
    fileInfo: "{size} · {duration}",
    files: "files",
    favorite: "Add to watch later",
    unfavorite: "Remove from watch later",
    dropTitle: "Drop to import video",
    dropBody: "Dropped videos are added to this local playlist",
    importFailed: "Import failed: {error}",
    loading: "Loading video…",
    unsupportedVideo: "This video cannot be played right now",
    shortcuts: "Space play / pause · F fullscreen · ← → seek",
    themeSystem: "Follow system",
    themeLight: "Light theme",
    themeDark: "Dark theme",
  },
} as const;

type CopyKey = keyof typeof COPY.zh;

const PLAYBACK_MODE_OPTIONS = [
  { value: "sequential", label: "sequential" },
  { value: "current", label: "currentLoop" },
  { value: "list", label: "listLoop" },
] as const satisfies ReadonlyArray<{ value: PlaybackMode; label: CopyKey }>;

const PLAYBACK_RATES = [0.5, 0.75, 1, 1.25, 1.5, 2] as const;

function readPlaybackMode(): PlaybackMode {
  const stored = readStored<string>("frame-playback-mode", "sequential");
  return PLAYBACK_MODE_OPTIONS.some((option) => option.value === stored) ? stored as PlaybackMode : "sequential";
}

function PlaybackModeIcon({ mode, size = 15 }: { mode: PlaybackMode; size?: number }) {
  if (mode === "current") return <Repeat1 size={size} />;
  if (mode === "list") return <ListVideo size={size} />;
  return <Repeat size={size} />;
}

function readStored<T>(key: string, fallback: T): T {
  try {
    const value = JSON.parse(localStorage.getItem(key) || "null") as T | null;
    return value ?? fallback;
  } catch {
    return fallback;
  }
}

function store(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Private browsing can disable storage; playback still works without it.
  }
}

type ShellContext = {
  locale: Locale;
  appearance: ThemePreference;
  theme: Theme;
  hasShellTheme: boolean;
};

function normalizeThemePreference(value: string | null | undefined): ThemePreference | null {
  return value === "system" || value === "light" || value === "dark" ? value : null;
}

function normalizeTheme(value: string | null | undefined): Theme | null {
  return value === "light" || value === "dark" ? value : null;
}

function resolveSystemTheme(): Theme {
  return window.matchMedia?.("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

function getContext(): ShellContext {
  const params = new URLSearchParams(window.location.search);
  const appearance = normalizeThemePreference(params.get("appearance")) ?? "system";
  const launchTheme = normalizeTheme(params.get("theme"));
  return {
    locale: params.get("locale") === "en" ? "en" : "zh",
    appearance,
    theme: launchTheme
      ?? (appearance === "light" || appearance === "dark" ? appearance : resolveSystemTheme()),
    hasShellTheme: Boolean(launchTheme) || appearance !== "system",
  };
}

type TooltipPlacement = "top" | "bottom";

function formatTime(value: number) {
  if (!Number.isFinite(value) || value <= 0) return "0:00";
  const seconds = Math.floor(value);
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

function formatSize(value: number) {
  if (!Number.isFinite(value)) return "";
  if (value < 1024 * 1024) return `${Math.max(1, Math.round(value / 1024))} KB`;
  return `${(value / 1024 / 1024).toFixed(1)} MB`;
}

function displayName(track: Track) {
  return track.name.replace(/\.[^/.]+$/, "") || track.name;
}

function fileKey(file: File) {
  return `${file.name}:${file.size}:${file.lastModified}`;
}

function hashTone(value: string) {
  let hash = 0;
  for (let index = 0; index < value.length; index += 1) {
    hash = ((hash << 5) - hash + value.charCodeAt(index)) | 0;
  }
  return Math.abs(hash) % 5;
}

function isVideoFile(file: File) {
  return file.type.startsWith("video/") || /\.(mp4|webm|mov|mkv|avi|m4v|ogv)$/i.test(file.name);
}

function createId() {
  return window.crypto?.randomUUID?.() ?? `video-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function Tooltip({ label, children, className = "", placement = "top" }: { label: string; children: ReactNode; className?: string; placement?: TooltipPlacement }) {
  return (
    <span className={`frame-tooltip ${className}`}>
      <span className={`frame-tooltip-content ${placement === "bottom" ? "is-below" : ""}`} role="tooltip" aria-hidden="true">{label}</span>
      {children}
    </span>
  );
}

function IconButton({
  label,
  children,
  onClick,
  active = false,
  disabled = false,
  className = "",
  tooltipPlacement = "top",
}: {
  label: string;
  children: ReactNode;
  onClick: () => void;
  active?: boolean;
  disabled?: boolean;
  className?: string;
  tooltipPlacement?: TooltipPlacement;
}) {
  return (
    <Tooltip label={label} placement={tooltipPlacement}>
      <button
        type="button"
        aria-label={label}
        onClick={onClick}
        disabled={disabled}
        className={`relative grid size-9 place-items-center rounded-lg border border-[var(--video-line)] text-[var(--video-muted)] transition hover:border-[var(--video-accent)]/55 hover:bg-[var(--video-hover)] hover:text-[var(--video-text)] disabled:cursor-default disabled:opacity-35 ${active ? "border-[var(--video-accent)]/45 bg-[var(--video-accent-soft)] text-[var(--video-accent)]" : ""} ${className}`}
      >
        {children}
      </button>
    </Tooltip>
  );
}

export function App() {
  const context = useMemo(getContext, []);
  const [locale, setLocale] = useState<Locale>(context.locale);
  const [themePreference, setThemePreference] = useState<ThemePreference>(() => {
    const stored = normalizeThemePreference(readStored<string>("frame-theme-preference", "system"));
    return stored ?? "system";
  });
  const [shellTheme, setShellTheme] = useState<Theme>(context.theme);
  const shellAppearanceRef = useRef<ThemePreference>(context.appearance);
  const shellContextReceivedRef = useRef(false);
  const theme = themePreference === "system" ? shellTheme : themePreference;
  const text = COPY[locale];
  const [tracks, setTracks] = useState<Track[]>([]);
  const tracksRef = useRef<Track[]>([]);
  const [currentId, setCurrentId] = useState<string | null>(null);
  const [favorites, setFavorites] = useState<Set<string>>(() => new Set(readStored<string[]>("frame-favorites", [])));
  const [recent, setRecent] = useState<string[]>(() => readStored<string[]>("frame-recent", []));
  const [view, setView] = useState<PlaylistView>("all");
  const [search, setSearch] = useState("");
  const [fit, setFit] = useState<FitMode>("contain");
  const [isPlaying, setIsPlaying] = useState(false);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const [fullscreenChromeVisible, setFullscreenChromeVisible] = useState(true);
  const [currentTime, setCurrentTime] = useState(0);
  const [duration, setDuration] = useState(0);
  const [volume, setVolume] = useState(() => Math.min(1, Math.max(0, Number(readStored("frame-volume", 0.82)))));
  const [muted, setMuted] = useState(false);
  const [playbackRate, setPlaybackRate] = useState(1);
  const [playbackMode, setPlaybackMode] = useState<PlaybackMode>(readPlaybackMode);
  const [playbackModeMenuOpen, setPlaybackModeMenuOpen] = useState(false);
  const [speedMenuOpen, setSpeedMenuOpen] = useState(false);
  const [dragDepth, setDragDepth] = useState(0);
  const [toast, setToast] = useState("");
  const [videoError, setVideoError] = useState(false);
  const [isLoading, setIsLoading] = useState(false);

  const videoRef = useRef<HTMLVideoElement>(null);
  const appRef = useRef<HTMLDivElement>(null);
  const stageRef = useRef<HTMLDivElement>(null);
  const autoplayRef = useRef(false);
  const toastTimerRef = useRef<number | undefined>(undefined);
  const fullscreenHideTimerRef = useRef<number | undefined>(undefined);
  const fullscreenPointerRef = useRef<{ x: number; y: number } | null>(null);
  const playbackModeMenuRef = useRef<HTMLDivElement>(null);
  const speedMenuRef = useRef<HTMLDivElement>(null);

  tracksRef.current = tracks;

  const t = (key: CopyKey, values?: Record<string, string | number>) => {
    let value: string = text[key];
    Object.entries(values ?? {}).forEach(([name, replacement]) => {
      value = value.replace(`{${name}}`, String(replacement));
    });
    return value;
  };

  const currentTrack = tracks.find((track) => track.id === currentId) ?? null;
  const visibleTracks = useMemo(() => {
    let result = tracks.slice();
    if (view === "recent") {
      result = result
        .filter((track) => recent.includes(track.key))
        .sort((a, b) => recent.indexOf(a.key) - recent.indexOf(b.key));
    }
    if (view === "favorites") result = result.filter((track) => favorites.has(track.key));
    const query = search.trim().toLowerCase();
    if (query) result = result.filter((track) => track.name.toLowerCase().includes(query));
    return result;
  }, [favorites, recent, search, tracks, view]);

  function notify(message: string) {
    setToast(message);
    window.clearTimeout(toastTimerRef.current);
    toastTimerRef.current = window.setTimeout(() => setToast(""), 2400);
  }

  function remember(track: Track) {
    setRecent((current) => {
      const next = [track.key, ...current.filter((key) => key !== track.key)].slice(0, 30);
      store("frame-recent", next);
      return next;
    });
  }

  function readMetadata(track: Track) {
    const probe = document.createElement("video");
    probe.preload = "metadata";
    probe.src = track.url;
    probe.addEventListener(
      "loadedmetadata",
      () => {
        const nextDuration = Number.isFinite(probe.duration) ? probe.duration : 0;
        setTracks((current) => current.map((item) => (item.id === track.id ? { ...item, duration: nextDuration } : item)));
        probe.removeAttribute("src");
        probe.load();
      },
      { once: true },
    );
    probe.addEventListener("error", () => probe.removeAttribute("src"), { once: true });
  }

  function selectTrack(track: Track, autoplay: boolean) {
    autoplayRef.current = autoplay;
    setVideoError(false);
    setIsLoading(true);
    setCurrentId(track.id);
    remember(track);
  }

  function addSources(sources: Array<{ file: File; id?: string; key?: string; name?: string; size?: number; url?: string; backendStored?: boolean }>, autoplayFirst = false) {
    const files = sources.filter(({ file }) => isVideoFile(file));
    if (files.length === 0) {
      notify(t("unsupported"));
      return;
    }
    const existing = new Set(tracks.map((track) => track.key));
    const added: Track[] = [];
    files.forEach((source) => {
      const { file } = source;
      const key = source.key ?? fileKey(file);
      if (existing.has(key)) return;
      const track: Track = {
        id: source.id ?? createId(),
        key,
        file,
        backendStored: source.backendStored,
        size: source.size ?? file.size,
        name: source.name ?? file.name,
        url: source.url ?? URL.createObjectURL(file),
        duration: 0,
        addedAt: Date.now() + added.length,
        tone: hashTone(key),
      };
      existing.add(key);
      added.push(track);
      readMetadata(track);
    });
    if (added.length === 0) {
      notify(t("duplicate"));
      return;
    }
    setTracks((current) => [...current, ...added]);
    if (autoplayFirst || !currentId) selectTrack(added[0], autoplayFirst);
    notify(t("added", { count: added.length }));
  }

  async function openVideos() {
    try {
      const selection = await openVideoSelection();
      if (selection.paths.length === 0 && selection.files.length === 0) return;
      if (selection.files.length > 0) {
        const files = selection.files.filter(isVideoFile);
        if (files.length === 0) {
          notify(t("unsupported"));
          return;
        }
        addSources(files.map((file) => ({ file })), currentId === null);
        return;
      }

      const videos = await Promise.all(
        selection.paths.map((path) => callOpenTool<LibraryVideo>("library.add", { path })),
      );
      const endpoint = await getOpenToolBaseUrl();
      const sources = videos.map((video) => ({
        file: new File([], video.name),
        id: video.id,
        key: video.id,
        name: video.name,
        size: video.size,
        url: new URL(`media/${encodeURIComponent(video.id)}`, endpoint).toString(),
        backendStored: true,
      }));
      if (sources.length === 0) {
        notify(t("unsupported"));
        return;
      }
      addSources(sources, currentId === null);
      setFavorites((current) => {
        const next = new Set(current);
        videos.filter((video) => video.favorite).forEach((video) => next.add(video.id));
        return next;
      });
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    }
  }

  function removeTrack(track: Track) {
    if (track.backendStored) {
      void callOpenTool("library.remove", { id: track.id }).catch(() => undefined);
    }
    const wasCurrent = track.id === currentId;
    setTracks((current) => current.filter((item) => item.id !== track.id));
    setRecent((current) => {
      const next = current.filter((key) => key !== track.key);
      store("frame-recent", next);
      return next;
    });
    setFavorites((current) => {
      const next = new Set(current);
      next.delete(track.key);
      store("frame-favorites", Array.from(next));
      return next;
    });
    if (track.url.startsWith("blob:")) URL.revokeObjectURL(track.url);
    if (wasCurrent) {
      const video = videoRef.current;
      video?.pause();
      if (video) {
        video.removeAttribute("src");
        video.load();
      }
      setCurrentId(null);
      setIsPlaying(false);
      setCurrentTime(0);
      setDuration(0);
    }
    notify(t("removed"));
  }

  function clearAll() {
    if (tracks.length === 0) return;
    if (tracks.some((track) => track.backendStored)) {
      void callOpenTool("library.clear", {}).catch(() => undefined);
    }
    tracks.forEach((track) => {
      if (track.url.startsWith("blob:")) URL.revokeObjectURL(track.url);
    });
    const video = videoRef.current;
    video?.pause();
    if (video) {
      video.removeAttribute("src");
      video.load();
    }
    setTracks([]);
    setFavorites(new Set());
    store("frame-favorites", []);
    setCurrentId(null);
    setIsPlaying(false);
    setCurrentTime(0);
    setDuration(0);
    notify(t("cleared"));
  }

  function toggleFavorite(track: Track | null) {
    if (!track) return;
    const favorite = !favorites.has(track.key);
    if (track.backendStored) {
      void callOpenTool("library.setFavorite", { id: track.id, favorite }).catch(() => undefined);
    }
    setFavorites((current) => {
      const next = new Set(current);
      if (next.has(track.key)) next.delete(track.key);
      else next.add(track.key);
      store("frame-favorites", Array.from(next));
      return next;
    });
  }

  function move(delta: number) {
    if (tracks.length === 0) return;
    const index = Math.max(0, tracks.findIndex((track) => track.id === currentId));
    const next = tracks[index + delta];
    if (next) selectTrack(next, true);
    else videoRef.current?.pause();
  }

  function restartCurrentVideo() {
    const video = videoRef.current;
    if (!video) return;
    video.currentTime = 0;
    void video.play().catch(() => setVideoError(true));
  }

  function handleVideoEnded() {
    if (!currentTrack) return;
    if (playbackMode === "current") {
      restartCurrentVideo();
      return;
    }
    if (playbackMode === "list") {
      const index = Math.max(0, tracks.findIndex((track) => track.id === currentTrack.id));
      const next = tracks[(index + 1) % tracks.length];
      if (next && next.id !== currentTrack.id) selectTrack(next, true);
      else restartCurrentVideo();
      return;
    }
    move(1);
  }

  function togglePlay() {
    const video = videoRef.current;
    if (!currentTrack) {
      const first = visibleTracks[0] ?? tracks[0];
      if (first) selectTrack(first, true);
      else notify(t("emptyPlaylistBody"));
      return;
    }
    if (!video) return;
    if (video.paused) {
      void video.play().catch(() => setVideoError(true));
    } else {
      video.pause();
    }
  }

  function currentFullscreenElement() {
    const documentWithWebkit = document as Document & {
      webkitFullscreenElement?: Element | null;
    };
    return document.fullscreenElement ?? documentWithWebkit.webkitFullscreenElement ?? null;
  }

  function isFrameVideoFullscreen() {
    return appRef.current !== null && currentFullscreenElement() === appRef.current;
  }

  function waitForFullscreenElement(expected: Element | null): Promise<boolean> {
    const matches = () => currentFullscreenElement() === expected;
    if (matches()) return Promise.resolve(true);

    return new Promise((resolve) => {
      let timeout: number | undefined;
      const finish = () => {
        document.removeEventListener("fullscreenchange", onChange);
        document.removeEventListener("webkitfullscreenchange", onChange);
        window.clearTimeout(timeout);
        resolve(matches());
      };
      const onChange = () => {
        if (matches()) finish();
      };

      document.addEventListener("fullscreenchange", onChange);
      document.addEventListener("webkitfullscreenchange", onChange);
      timeout = window.setTimeout(finish, 750);
      onChange();
    });
  }

  function syncFullscreenState() {
    const active = isFrameVideoFullscreen();
    setIsFullscreen(active);
    fullscreenPointerRef.current = null;
    setFullscreenChromeVisible(true);
    clearFullscreenHideTimer();
    if (active) armFullscreenHideTimer();
    return active;
  }

  async function toggleFullscreen() {
    const documentWithWebkit = document as Document & {
      webkitExitFullscreen?: () => Promise<void> | void;
    };
    const activeElement = currentFullscreenElement();

    if (activeElement) {
      const exitFullscreen = document.exitFullscreen ?? documentWithWebkit.webkitExitFullscreen;
      if (!exitFullscreen) {
        syncFullscreenState();
        notify(t("fullscreenUnavailable"));
        return;
      }
      try {
        const result = exitFullscreen.call(document);
        if (result && typeof result.then === "function") await result;
        const exited = await waitForFullscreenElement(null);
        syncFullscreenState();
        if (!exited) notify(t("fullscreenExitFailed"));
      } catch {
        if (syncFullscreenState()) notify(t("fullscreenExitFailed"));
      }
      return;
    }

    // Fullscreen the app shell rather than just the video stage so the
    // bottom control bar remains inside the fullscreen surface. The Shell
    // listens to the resulting HTML fullscreen event and hides its top bar.
    const target = appRef.current as (HTMLDivElement & {
      webkitRequestFullscreen?: () => Promise<void> | void;
    }) | null;
    const requestFullscreen = target?.requestFullscreen ?? target?.webkitRequestFullscreen;
    if (!target || !requestFullscreen) {
      syncFullscreenState();
      notify(t("fullscreenUnavailable"));
      return;
    }

    try {
      const result = requestFullscreen.call(target);
      if (result && typeof result.then === "function") await result;
      const entered = await waitForFullscreenElement(target);
      syncFullscreenState();
      if (!entered) notify(t("fullscreenRequestFailed"));
    } catch {
      if (!syncFullscreenState()) notify(t("fullscreenRequestFailed"));
    }
  }

  function clearFullscreenHideTimer() {
    window.clearTimeout(fullscreenHideTimerRef.current);
    fullscreenHideTimerRef.current = undefined;
  }

  function armFullscreenHideTimer() {
    clearFullscreenHideTimer();
    if (!isFrameVideoFullscreen()) return;
    fullscreenHideTimerRef.current = window.setTimeout(() => {
      if (isFrameVideoFullscreen()) setFullscreenChromeVisible(false);
    }, 3000);
  }

  function revealFullscreenChrome() {
    if (!isFrameVideoFullscreen()) return;
    setFullscreenChromeVisible(true);
    armFullscreenHideTimer();
  }

  function handleFullscreenPointerMove(event: ReactPointerEvent<HTMLDivElement>) {
    if (!isFrameVideoFullscreen() || event.pointerType === "touch") return;
    const point = { x: event.clientX, y: event.clientY };
    const previous = fullscreenPointerRef.current;
    if (!previous) {
      fullscreenPointerRef.current = point;
      revealFullscreenChrome();
      return;
    }
    if (Math.hypot(point.x - previous.x, point.y - previous.y) < 8) return;
    fullscreenPointerRef.current = point;
    revealFullscreenChrome();
  }

  function togglePictureInPicture() {
    const video = videoRef.current;
    if (!video) return;
    if (document.pictureInPictureElement) {
      void document.exitPictureInPicture?.();
      return;
    }
    if (document.pictureInPictureEnabled && video.requestPictureInPicture) {
      void video.requestPictureInPicture().catch(() => notify(t("pipUnsupported")));
    } else {
      notify(t("pipUnsupported"));
    }
  }

  function importDroppedFiles(fileList: FileList | File[]) {
    const files = Array.from(fileList).filter(isVideoFile);
    if (files.length === 0) {
      notify(t("unsupported"));
      return;
    }
    addSources(files.map((file) => ({ file })), true);
  }

  function handleDrop(event: DragEvent<HTMLDivElement>) {
    event.preventDefault();
    setDragDepth(0);
    importDroppedFiles(event.dataTransfer.files);
  }

  function handleStageDoubleClick() {
    if (currentTrack) toggleFullscreen();
  }

  function choosePlaybackMode(mode: PlaybackMode) {
    setPlaybackMode(mode);
    store("frame-playback-mode", mode);
    setPlaybackModeMenuOpen(false);
  }

  function choosePlaybackRate(rate: number) {
    setPlaybackRate(rate);
    setSpeedMenuOpen(false);
  }

  function handleRemoveClick(event: MouseEvent<HTMLButtonElement>, track: Track) {
    event.stopPropagation();
    removeTrack(track);
  }

  function cycleThemePreference() {
    const next: ThemePreference = themePreference === "system"
      ? "light"
      : themePreference === "light"
        ? "dark"
        : "system";
    setThemePreference(next);
    store("frame-theme-preference", next);
  }

  useEffect(() => {
    let active = true;
    void (async () => {
      try {
        const result = await callOpenTool<{ videos: LibraryVideo[] }>("library.list", {});
        const endpoint = await getOpenToolBaseUrl();
        if (!active) return;
        setFavorites((current) => {
          const next = new Set(current);
          result.videos.filter((video) => video.favorite).forEach((video) => next.add(video.id));
          return next;
        });
        addSources(
          result.videos.map((video) => ({
            file: new File([], video.name),
            id: video.id,
            key: video.id,
            name: video.name,
            size: video.size,
            url: new URL(`media/${encodeURIComponent(video.id)}`, endpoint).toString(),
            backendStored: true,
          })),
          false,
        );
      } catch {
        // The browser-only preview does not run the Rust OpenTool backend.
      }
    })();
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    document.documentElement.lang = locale === "zh" ? "zh-CN" : "en";
  }, [locale, theme]);

  useEffect(() => {
    const handleMessage = (event: MessageEvent<unknown>) => {
      if (!event.data || typeof event.data !== "object") return;
      const message = event.data as {
        channel?: string;
        type?: string;
        appearance?: string;
        theme?: string;
        locale?: string;
        runtime?: string;
      };
      const isExternalHostContext = message.channel === "agentos:host" && message.type === "context-changed";
      if (isExternalHostContext) {
        shellContextReceivedRef.current = true;
        const nextAppearance = normalizeThemePreference(message.appearance);
        if (nextAppearance) shellAppearanceRef.current = nextAppearance;
        const nextTheme = normalizeTheme(message.theme)
          ?? (nextAppearance && nextAppearance !== "system" ? nextAppearance : resolveSystemTheme());
        if (nextTheme) {
          setShellTheme(nextTheme);
        }
        if (message.locale?.toLowerCase().startsWith("zh")) setLocale("zh");
        else if (message.locale?.toLowerCase().startsWith("en")) setLocale("en");
        return;
      }
      if ((message.type === "shell:theme" || message.type === "shell:appearance") && normalizeTheme(message.appearance)) {
        shellContextReceivedRef.current = true;
        shellAppearanceRef.current = normalizeTheme(message.appearance) as ThemePreference;
        const nextTheme = normalizeTheme(message.appearance) as Theme;
        setShellTheme(nextTheme);
      }
      if (message.type === "shell:locale" && (message.locale === "zh" || message.locale === "en")) setLocale(message.locale);
    };
    window.addEventListener("message", handleMessage);
    return () => window.removeEventListener("message", handleMessage);
  }, []);

  useEffect(() => {
    const mediaQuery = window.matchMedia?.("(prefers-color-scheme: dark)");
    if (!mediaQuery) return undefined;
    const updateSystemTheme = () => {
      // An embed can provide Shell's already-resolved theme even when the raw
      // preference is `system`; keep that value until the host sends a live
      // context update instead of replacing it with the browser's fallback.
      if (context.hasShellTheme && !shellContextReceivedRef.current) return;
      if (shellContextReceivedRef.current && shellAppearanceRef.current !== "system") return;
      const nextTheme: Theme = mediaQuery.matches ? "dark" : "light";
      setShellTheme(nextTheme);
    };
    updateSystemTheme();
    mediaQuery.addEventListener("change", updateSystemTheme);
    return () => mediaQuery.removeEventListener("change", updateSystemTheme);
  }, [context.hasShellTheme]);

  useEffect(() => {
    const video = videoRef.current;
    if (!video || !currentTrack) return;
    setIsLoading(true);
    video.src = currentTrack.url;
    video.load();
    const shouldAutoplay = autoplayRef.current;
    autoplayRef.current = false;
    if (shouldAutoplay) {
      const playWhenReady = () => {
        void video.play().catch(() => setVideoError(true));
      };
      video.addEventListener("loadeddata", playWhenReady, { once: true });
      return () => video.removeEventListener("loadeddata", playWhenReady);
    }
    return undefined;
  }, [currentTrack?.id, currentTrack?.url]);

  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    video.volume = volume;
    video.muted = muted;
  }, [muted, volume]);

  useEffect(() => {
    const video = videoRef.current;
    if (video) video.playbackRate = playbackRate;
  }, [playbackRate]);

  useEffect(() => {
    const onFullscreenChange = () => {
      syncFullscreenState();
    };
    document.addEventListener("fullscreenchange", onFullscreenChange);
    document.addEventListener("webkitfullscreenchange", onFullscreenChange);
    return () => {
      document.removeEventListener("fullscreenchange", onFullscreenChange);
      document.removeEventListener("webkitfullscreenchange", onFullscreenChange);
      clearFullscreenHideTimer();
    };
  }, []);

  useEffect(() => {
    if (!playbackModeMenuOpen && !speedMenuOpen) return undefined;
    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target as Node;
      if (!playbackModeMenuRef.current?.contains(target)) setPlaybackModeMenuOpen(false);
      if (!speedMenuRef.current?.contains(target)) setSpeedMenuOpen(false);
    };
    const handleMenuKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setPlaybackModeMenuOpen(false);
        setSpeedMenuOpen(false);
      }
    };
    document.addEventListener("pointerdown", handlePointerDown);
    document.addEventListener("keydown", handleMenuKeyDown);
    return () => {
      document.removeEventListener("pointerdown", handlePointerDown);
      document.removeEventListener("keydown", handleMenuKeyDown);
    };
  }, [playbackModeMenuOpen, speedMenuOpen]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement;
      if (["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)) {
        if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
          event.preventDefault();
          document.getElementById("video-search")?.focus();
        }
        return;
      }
      if (event.code === "Space") {
        event.preventDefault();
        togglePlay();
      } else if (event.key.toLowerCase() === "f") {
        event.preventDefault();
        toggleFullscreen();
      } else if (event.key.toLowerCase() === "m") {
        setMuted((current) => !current);
      } else if (event.key === "ArrowLeft" && videoRef.current) {
        videoRef.current.currentTime = Math.max(0, videoRef.current.currentTime - 10);
      } else if (event.key === "ArrowRight" && videoRef.current) {
        videoRef.current.currentTime = Math.min(videoRef.current.duration || Infinity, videoRef.current.currentTime + 10);
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  });

  useEffect(() => {
    return () => {
      window.clearTimeout(toastTimerRef.current);
      window.clearTimeout(fullscreenHideTimerRef.current);
      tracksRef.current.forEach((track) => {
        if (track.url.startsWith("blob:")) URL.revokeObjectURL(track.url);
      });
    };
  }, []);

  const progressMax = duration || currentTrack?.duration || 1;
  const progressValue = Math.min(currentTime, progressMax);
  const isFavorite = currentTrack ? favorites.has(currentTrack.key) : false;
  const listEmpty = visibleTracks.length === 0;
  const currentPlaybackMode = PLAYBACK_MODE_OPTIONS.find((option) => option.value === playbackMode) ?? PLAYBACK_MODE_OPTIONS[0];
  const themeLabel = themePreference === "system" ? t("themeSystem") : themePreference === "light" ? t("themeLight") : t("themeDark");

  return (
    <div
      ref={appRef}
      className={`frame-video-app relative flex h-full min-h-0 flex-col overflow-hidden bg-[var(--video-bg)] text-[var(--video-text)] ${isFullscreen ? "is-fullscreen" : ""} ${isFullscreen && !fullscreenChromeVisible ? "fullscreen-controls-hidden" : ""}`}
      onPointerMove={handleFullscreenPointerMove}
      onDragEnter={(event) => {
        event.preventDefault();
        setDragDepth((current) => current + 1);
      }}
      onDragOver={(event) => event.preventDefault()}
      onDragLeave={(event) => {
        event.preventDefault();
        setDragDepth((current) => Math.max(0, current - 1));
      }}
      onDrop={handleDrop}
    >
      <header className="flex h-14 shrink-0 items-center justify-between border-b border-[var(--video-line)] px-4 sm:px-6">
        <div className="flex min-w-0 items-center gap-3">
          <img src={logoUrl} alt="" aria-hidden="true" className="size-8 shrink-0 rounded-lg shadow-lg shadow-cyan-500/15" />
          <div className="min-w-0">
            <div className="truncate text-sm font-semibold tracking-tight">{t("brand")}</div>
            <div className="truncate text-[10px] text-[var(--video-muted)]">{t("subtitle")}</div>
          </div>
        </div>
        <div className="hidden items-center gap-3 text-[10px] text-[var(--video-faint)] sm:flex">
          <span>{t("shortcuts")}</span>
          <div className="h-4 w-px bg-[var(--video-line)]" />
          <Tooltip label={themeLabel} placement="bottom">
            <button type="button" aria-label={themeLabel} title={themeLabel} className="grid size-8 place-items-center rounded-md text-[var(--video-muted)] transition hover:bg-[var(--video-hover)] hover:text-[var(--video-text)]" onClick={cycleThemePreference}>
              {themePreference === "system" ? <Monitor size={15} /> : theme === "dark" ? <Moon size={16} /> : <Sun size={16} />}
            </button>
          </Tooltip>
        </div>
      </header>

      <main className="grid min-h-0 flex-1 grid-cols-1 overflow-hidden lg:grid-cols-[minmax(0,1fr)_320px]">
        <section className="flex min-h-0 min-w-0 flex-col p-3 sm:p-5">
          <div
            ref={stageRef}
            className={`video-stage relative flex min-h-[280px] min-w-0 flex-1 items-center justify-center overflow-hidden rounded-xl border border-[var(--video-line)] bg-[#05070d] shadow-2xl shadow-black/20 ${fit === "cover" ? "is-cover" : ""}`}
            onDoubleClick={handleStageDoubleClick}
          >
            {currentTrack ? (
              <video
                ref={videoRef}
                className={`absolute inset-0 h-full w-full bg-black ${fit === "cover" ? "object-cover" : "object-contain"}`}
                playsInline
                preload="metadata"
                onPlay={() => setIsPlaying(true)}
                onPause={() => setIsPlaying(false)}
                onTimeUpdate={(event) => setCurrentTime(event.currentTarget.currentTime)}
                onLoadedMetadata={(event) => {
                  setDuration(Number.isFinite(event.currentTarget.duration) ? event.currentTarget.duration : 0);
                  setIsLoading(false);
                }}
                onLoadedData={() => setIsLoading(false)}
                onWaiting={() => setIsLoading(true)}
                onCanPlay={() => setIsLoading(false)}
                onEnded={handleVideoEnded}
                onError={() => {
                  setIsLoading(false);
                  setVideoError(true);
                }}
              />
            ) : (
              <div className="flex max-w-sm flex-col items-center px-6 text-center">
                <div className="mb-5 grid size-16 place-items-center rounded-2xl border border-cyan-300/25 bg-cyan-400/10 text-cyan-300 shadow-[0_0_55px_rgba(34,211,238,0.12)]">
                  <MonitorPlay size={30} strokeWidth={1.4} />
                </div>
                <h1 className="text-base font-semibold tracking-tight text-white sm:text-lg">{t("emptyTitle")}</h1>
                <p className="mt-2 text-xs leading-5 text-white/45">{t("emptyBody")}</p>
                <button type="button" onClick={openVideos} className="mt-5 inline-flex h-9 items-center gap-2 rounded-lg bg-cyan-400 px-4 text-xs font-semibold text-slate-950 transition hover:bg-cyan-300">
                  <FolderOpen size={15} />
                  {t("open")}
                </button>
              </div>
            )}

            <div className={`video-stage-chrome pointer-events-none absolute inset-x-0 top-0 flex items-center justify-between bg-gradient-to-b from-black/55 to-transparent px-3 py-3 sm:px-4 ${isFullscreen && !fullscreenChromeVisible ? "is-hidden" : ""}`}>
              <div className="flex min-w-0 items-center gap-2 text-[10px] text-white/70">
                <span className="size-1.5 shrink-0 rounded-full bg-emerald-300 shadow-[0_0_0_3px_rgba(110,231,183,0.14)]" />
                <span className="font-mono uppercase tracking-[0.13em]">{t("local")}</span>
                {currentTrack ? <span className="max-w-[42vw] truncate text-white/50">· {displayName(currentTrack)}</span> : null}
              </div>
              <button type="button" aria-label={fit === "contain" ? t("fill") : t("fit")} className="pointer-events-auto inline-flex items-center gap-1.5 rounded-md border border-white/15 bg-black/25 px-2 py-1 text-[10px] text-white/70 backdrop-blur transition hover:bg-black/45 hover:text-white" onClick={() => setFit((current) => current === "contain" ? "cover" : "contain")}>
                <Maximize2 size={13} />
                {fit === "contain" ? t("fit") : t("fill")}
              </button>
            </div>

            {currentTrack && isLoading && !videoError ? <div className="absolute inset-0 grid place-items-center bg-black/20 text-xs text-white/65">{t("loading")}</div> : null}
            {videoError ? <div className="absolute inset-0 grid place-items-center gap-2 bg-[#180d17]/80 text-xs text-rose-200"><Video size={17} />{t("unsupportedVideo")}</div> : null}
          </div>
        </section>

        <aside className="playlist-panel flex min-h-0 min-w-0 flex-col border-t border-[var(--video-line)] bg-[var(--video-panel)] lg:border-l lg:border-t-0">
          <div className="flex shrink-0 items-start justify-between gap-3 border-b border-[var(--video-line)] px-4 pb-3 pt-4 sm:px-5">
            <div className="min-w-0">
              <div className="mb-1 flex items-center gap-2 text-[10px] font-medium uppercase tracking-[0.14em] text-[var(--video-accent)]"><ListVideo size={13} />{t("playlist")}</div>
              <div className="text-lg font-semibold tracking-tight">{t("playlist")}</div>
              <div className="mt-1 text-[10px] text-[var(--video-muted)]">{tracks.length} {t("files")}</div>
            </div>
            <div className="flex shrink-0 items-center gap-2">
              <IconButton label={t("clear")} tooltipPlacement="bottom" onClick={clearAll} disabled={tracks.length === 0}><Trash2 size={16} /></IconButton>
              <button type="button" onClick={openVideos} className={`inline-flex h-9 items-center gap-1.5 rounded-lg bg-[var(--video-accent)] px-3 text-[11px] font-semibold transition hover:brightness-110 ${theme === "light" ? "text-white" : "text-slate-950"}`}><Upload size={14} />{t("add")}</button>
            </div>
          </div>

          <div className="flex shrink-0 flex-col gap-2 border-b border-[var(--video-line)] px-4 py-3 sm:px-5">
            <label className="flex h-8 items-center gap-2 rounded-lg border border-[var(--video-line)] bg-[var(--video-soft)] px-2.5 text-[var(--video-muted)] focus-within:border-[var(--video-accent)]/60">
              <Search size={14} />
              <input id="video-search" value={search} onChange={(event) => setSearch(event.target.value)} placeholder={t("search")} className="min-w-0 flex-1 bg-transparent text-[11px] text-[var(--video-text)] outline-none placeholder:text-[var(--video-faint)]" />
              {search ? <button type="button" className="text-[var(--video-faint)] hover:text-[var(--video-text)]" onClick={() => setSearch("")} aria-label="Clear search"><X size={13} /></button> : null}
            </label>
            <div className="flex gap-1" role="tablist" aria-label={t("playlist")}>
              {([["all", "all"], ["recent", "recent"], ["favorites", "favorites"]] as const).map(([key, label]) => (
                <button key={key} type="button" role="tab" aria-selected={view === key} onClick={() => setView(key)} className={`flex-1 rounded-md px-2 py-1.5 text-[10px] transition ${view === key ? "bg-[var(--video-accent-soft)] text-[var(--video-accent)]" : "text-[var(--video-muted)] hover:bg-[var(--video-hover)] hover:text-[var(--video-text)]"}`}>
                  {t(label)}{key === "favorites" ? ` (${tracks.filter((track) => favorites.has(track.key)).length})` : ""}
                </button>
              ))}
            </div>
          </div>

          <div className="min-h-0 flex-1 overflow-y-auto p-2.5 sm:p-3">
            {listEmpty ? (
              <div className="grid min-h-[190px] place-items-center px-5 py-8 text-center">
                <div>
                  <div className="mx-auto grid size-11 place-items-center rounded-xl bg-[var(--video-accent-soft)] text-[var(--video-accent)]"><Film size={21} /></div>
                  <div className="mt-3 text-xs font-semibold">{tracks.length > 0 ? t("noMatches") : t("emptyPlaylist")}</div>
                  <p className="mx-auto mt-1.5 max-w-[220px] text-[10px] leading-5 text-[var(--video-muted)]">{tracks.length > 0 ? t("noMatches") : t("emptyPlaylistBody")}</p>
                  <button type="button" onClick={openVideos} className="mt-4 inline-flex h-8 items-center gap-1.5 rounded-lg border border-[var(--video-line)] bg-[var(--video-soft)] px-3 text-[10px] font-semibold text-[var(--video-text)] transition hover:border-[var(--video-accent)]/50 hover:bg-[var(--video-hover)]"><FolderOpen size={14} />{t("open")}</button>
                </div>
              </div>
            ) : (
              <div className="grid gap-1">
                {visibleTracks.map((track, index) => (
                  <div key={track.id} role="button" tabIndex={0} onClick={() => selectTrack(track, true)} onKeyDown={(event) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); selectTrack(track, true); } }} className={`group grid cursor-pointer grid-cols-[64px_minmax(0,1fr)_28px] items-center gap-2 rounded-lg border p-1.5 text-left transition ${track.id === currentId ? "border-[var(--video-accent)]/40 bg-[var(--video-accent-soft)]" : "border-transparent hover:border-[var(--video-line)] hover:bg-[var(--video-soft)]"}`}>
                    <div className={`video-thumb tone-${track.tone} relative grid h-11 w-16 place-items-center overflow-hidden rounded-md text-white/80`}><Film size={17} className="relative z-10" /><span className="absolute bottom-1 right-1 z-10 rounded bg-black/35 px-1 font-mono text-[8px] text-white/75">{formatTime(track.duration)}</span></div>
                    <div className="min-w-0">
                      <div className={`truncate text-[11px] font-medium ${track.id === currentId ? "text-[var(--video-text)]" : "text-[var(--video-muted)] group-hover:text-[var(--video-text)]"}`}>{displayName(track)}</div>
                      <div className="mt-1 truncate font-mono text-[9px] text-[var(--video-faint)]">{index + 1} · {formatSize(track.size)}</div>
                    </div>
                    <button type="button" onClick={(event) => handleRemoveClick(event, track)} aria-label={t("removed")} className="grid size-7 place-items-center rounded-md text-[var(--video-faint)] opacity-0 transition hover:bg-rose-400/10 hover:text-rose-300 group-hover:opacity-100"><Trash2 size={14} /></button>
                  </div>
                ))}
              </div>
            )}
          </div>
        </aside>
      </main>

      <footer className="video-control-bar shrink-0 border-t border-[var(--video-line)] bg-[var(--video-panel-strong)] px-3 pb-2 pt-1.5 sm:px-5">
        <input className="video-progress block h-3 w-full cursor-pointer appearance-none bg-transparent" style={{ "--progress": `${progressMax ? (progressValue / progressMax) * 100 : 0}%` } as CSSProperties} type="range" min="0" max={progressMax} step="0.1" value={progressValue} onChange={(event) => { const next = Number(event.target.value); setCurrentTime(next); if (videoRef.current) videoRef.current.currentTime = next; }} aria-label="Seek" />
        <div className="flex min-h-10 items-center justify-between gap-2">
          <div className="flex min-w-0 items-center gap-0.5">
            <IconButton label={isPlaying ? t("pause") : t("play")} onClick={togglePlay} disabled={!currentTrack} className="border-transparent"><>{isPlaying ? <Pause size={18} fill="currentColor" /> : <Play size={18} fill="currentColor" />}</></IconButton>
            <IconButton label={t("previous")} onClick={() => move(-1)} disabled={!currentTrack}><RotateCcw size={16} /></IconButton>
            <IconButton label={t("back")} onClick={() => { if (videoRef.current) videoRef.current.currentTime = Math.max(0, videoRef.current.currentTime - 10); }} disabled={!currentTrack}><span className="absolute text-[7px] font-mono">10</span><RotateCcw size={19} /></IconButton>
            <IconButton label={t("forward")} onClick={() => { if (videoRef.current) videoRef.current.currentTime = Math.min(videoRef.current.duration || Infinity, videoRef.current.currentTime + 10); }} disabled={!currentTrack}><span className="absolute text-[7px] font-mono">10</span><RotateCw size={19} /></IconButton>
            <div className="ml-1 flex items-center gap-1">
              <IconButton label={muted ? t("unmute") : t("mute")} onClick={() => setMuted((current) => !current)} disabled={!currentTrack} className="border-transparent"><>{muted || volume === 0 ? <VolumeX size={16} /> : <Volume2 size={16} />}</></IconButton>
              <input className="video-volume hidden h-3 w-16 cursor-pointer appearance-none bg-transparent sm:block" style={{ "--volume": `${(muted ? 0 : volume) * 100}%` } as CSSProperties} type="range" min="0" max="1" step="0.01" value={muted ? 0 : volume} onChange={(event) => { const next = Number(event.target.value); setVolume(next); setMuted(false); store("frame-volume", next); }} aria-label="Volume" />
            </div>
            <span className="ml-2 whitespace-nowrap font-mono text-[9px] text-[var(--video-muted)]">{formatTime(currentTime)} / {formatTime(duration || currentTrack?.duration || 0)}</span>
          </div>
          <div className="flex shrink-0 items-center gap-1">
            <div ref={playbackModeMenuRef} className="relative">
              <Tooltip className={playbackModeMenuOpen ? "is-menu-open" : ""} label={t(currentPlaybackMode.label)}>
                <button
                  type="button"
                  aria-label={t("playbackMode")}
                  aria-haspopup="listbox"
                  aria-expanded={playbackModeMenuOpen}
                  onClick={() => {
                    setPlaybackModeMenuOpen((open) => !open);
                    setSpeedMenuOpen(false);
                  }}
                  className={`grid size-9 place-items-center rounded-lg border text-[var(--video-muted)] transition hover:border-[var(--video-accent)]/55 hover:bg-[var(--video-hover)] hover:text-[var(--video-text)] ${playbackModeMenuOpen ? "border-[var(--video-accent)]/65 bg-[var(--video-accent-soft)] text-[var(--video-accent)]" : "border-[var(--video-line)]"}`}
                >
                  <PlaybackModeIcon mode={playbackMode} />
                </button>
              </Tooltip>
              {playbackModeMenuOpen ? (
                <div className="frame-menu-panel absolute bottom-[calc(100%+0.5rem)] left-0 z-50 min-w-36 rounded-xl border border-[var(--video-line)] bg-[var(--video-panel-strong)] p-1 shadow-2xl shadow-black/35 backdrop-blur-xl" role="listbox" aria-label={t("playbackMode")}>
                  {PLAYBACK_MODE_OPTIONS.map((option) => (
                    <button key={option.value} type="button" role="option" aria-selected={playbackMode === option.value} onClick={() => choosePlaybackMode(option.value)} className={`flex w-full items-center gap-2 rounded-lg px-2.5 py-2 text-left text-[10px] transition ${playbackMode === option.value ? "bg-[var(--video-accent-soft)] text-[var(--video-accent)]" : "text-[var(--video-muted)] hover:bg-[var(--video-hover)] hover:text-[var(--video-text)]"}`}>
                      <PlaybackModeIcon mode={option.value} size={14} />
                      <span className="flex-1 whitespace-nowrap">{t(option.label)}</span>
                      {playbackMode === option.value ? <span className="text-[9px]">✓</span> : null}
                    </button>
                  ))}
                </div>
              ) : null}
            </div>
            <div ref={speedMenuRef} className="relative">
              <Tooltip className={speedMenuOpen ? "is-menu-open" : ""} label={t("speed")}>
                <button
                  type="button"
                  aria-label={t("speed")}
                  aria-haspopup="listbox"
                  aria-expanded={speedMenuOpen}
                  onClick={() => {
                    setSpeedMenuOpen((open) => !open);
                    setPlaybackModeMenuOpen(false);
                  }}
                  className={`flex h-8 items-center gap-1 rounded-lg border px-2 text-[10px] transition hover:border-[var(--video-accent)]/55 hover:bg-[var(--video-hover)] hover:text-[var(--video-text)] ${speedMenuOpen ? "border-[var(--video-accent)]/65 bg-[var(--video-accent-soft)] text-[var(--video-text)]" : "border-[var(--video-line)] bg-[var(--video-soft)] text-[var(--video-muted)]"}`}
                >
                  <span>{playbackRate}×</span>
                  <ChevronDown size={14} className={`transition-transform ${speedMenuOpen ? "rotate-180" : ""}`} />
                </button>
              </Tooltip>
              {speedMenuOpen ? (
                <div className="frame-menu-panel absolute bottom-[calc(100%+0.5rem)] right-0 z-50 min-w-24 rounded-xl border border-[var(--video-line)] bg-[var(--video-panel-strong)] p-1 shadow-2xl shadow-black/35 backdrop-blur-xl" role="listbox" aria-label={t("speed")}>
                  {PLAYBACK_RATES.map((rate) => (
                    <button key={rate} type="button" role="option" aria-selected={playbackRate === rate} onClick={() => choosePlaybackRate(rate)} className={`flex w-full items-center justify-center rounded-lg px-3 py-2 text-[10px] transition ${playbackRate === rate ? "bg-[var(--video-accent-soft)] text-[var(--video-accent)]" : "text-[var(--video-muted)] hover:bg-[var(--video-hover)] hover:text-[var(--video-text)]"}`}>
                      {rate}×
                    </button>
                  ))}
                </div>
              ) : null}
            </div>
            <IconButton label={t("pictureInPicture")} onClick={togglePictureInPicture} disabled={!currentTrack}><PictureInPicture2 size={16} /></IconButton>
            <IconButton label={isFullscreen ? t("exitFullscreen") : t("fullscreen")} onClick={toggleFullscreen} disabled={!currentTrack}>{isFullscreen ? <Minimize2 size={16} /> : <Fullscreen size={16} />}</IconButton>
            <IconButton label={isFavorite ? t("unfavorite") : t("favorite")} onClick={() => toggleFavorite(currentTrack)} disabled={!currentTrack} active={isFavorite}><Heart size={16} fill={isFavorite ? "currentColor" : "none"} /></IconButton>
          </div>
        </div>
      </footer>

      {dragDepth > 0 ? <div className="pointer-events-none fixed inset-0 z-40 grid place-items-center bg-slate-950/75 backdrop-blur-sm"><div className="grid justify-items-center rounded-2xl border border-cyan-300/35 bg-slate-900/95 px-12 py-10 text-center text-white shadow-2xl"><div className="grid size-12 place-items-center rounded-xl bg-cyan-400/15 text-cyan-300"><Upload size={22} /></div><strong className="mt-4 text-base">{t("dropTitle")}</strong><span className="mt-2 text-[10px] text-white/55">{t("dropBody")}</span></div></div> : null}
      {toast ? <div role="status" className="fixed bottom-20 left-1/2 z-50 -translate-x-1/2 rounded-lg border border-[var(--video-line)] bg-[var(--video-panel-strong)] px-4 py-2.5 text-[10px] text-[var(--video-text)] shadow-xl">{toast}</div> : null}
    </div>
  );
}
