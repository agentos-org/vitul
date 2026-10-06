export function readPluginLaunchContext(search = window.location.search) {
  const params = new URLSearchParams(search);
  const localeParam = params.get("locale");
  const locale = localeParam === "en"
    ? "en"
    : localeParam === "zh"
      ? "zh"
      : hostLocale ?? "zh";
  const appearanceParam = params.get("appearance");
  const appearance =
    appearanceParam === "light" || appearanceParam === "dark" || appearanceParam === "system"
      ? appearanceParam
      : "system";
  const theme = params.get("theme") === "dark" ? "dark" : "light";
  const windowStateParam = params.get("windowState");
  const windowState = windowStateParam === "normal"
    || windowStateParam === "maximized"
    || windowStateParam === "minimized"
    || windowStateParam === "dragging"
    || windowStateParam === "resizing"
    ? windowStateParam
    : undefined;
  const timezoneParam = params.get("timezone")?.trim();
  const zoomValue = Number(params.get("zoom"));
  const runtimeParam = params.get("runtime") ?? params.get("agentosRuntime");
  const bundleIdParam = (params.get("bundleId") ?? params.get("appId"))?.trim();
  const bundleId = window.location.protocol === "agentos-app:"
    ? window.location.hostname
    : isValidAppIdentity(bundleIdParam)
      ? bundleIdParam
      : undefined;
  const agentosRuntime = window.location.protocol === "agentos-app:"
    || runtimeParam === "agentos"
    || hostRuntime === "agentos";

  return {
    appId: bundleId,
    bundleId,
    locale,
    appearance,
    theme,
    timezone: timezoneParam || hostTimezone || undefined,
    version: params.get("version") ?? undefined,
    apiBase: params.get("apiBase") ?? undefined,
    section: params.get("section") ?? undefined,
    hostOrigin: params.get("hostOrigin") ?? undefined,
    zoom: Number.isFinite(zoomValue) ? zoomValue : undefined,
    windowTitleBar: parseWindowTitleBarContext(params.get("windowTitleBar")),
    windowState,
    runtime: agentosRuntime ? "agentos" : "browser",
  };
}

function parseWindowTitleBarContext(value) {
  if (!value) return undefined;
  let context;
  try {
    context = JSON.parse(value);
  } catch {
    return undefined;
  }
  if (!context || typeof context !== "object") return undefined;
  if (
    (context.requestedMode !== "system" && context.requestedMode !== "overlay")
    || (context.actualMode !== null && context.actualMode !== "system" && context.actualMode !== "overlay")
    || (context.supportStatus !== "supported" && context.supportStatus !== "unsupported")
    || !Array.isArray(context.supportedModes)
    || !context.supportedModes.every((mode) => mode === "system" || mode === "overlay")
    || (context.windowKind !== "native-top-level" && context.windowKind !== "shell-internal" && context.windowKind !== "external")
    || (context.controlsProvider !== "operating-system" && context.controlsProvider !== "agentos" && context.controlsProvider !== "none")
  ) {
    return undefined;
  }
  const supportsRequestedMode = context.supportedModes.includes(context.requestedMode);
  if (
    (context.actualMode !== null && !context.supportedModes.includes(context.actualMode))
    || (context.supportStatus === "supported" && (context.actualMode !== context.requestedMode || !supportsRequestedMode))
    || (context.supportStatus === "unsupported" && supportsRequestedMode)
  ) {
    return undefined;
  }
  return context;
}

export function postHostMessage(message, targetWindow = window.parent, targetOrigin) {
  // A production Flutter/CEF App is top-level on its own agentos-app://
  // origin, so there is no iframe parent to receive the legacy postMessage.
  // Keep the browser WebShell path unchanged and use the origin-scoped native
  // endpoint only for that top-level runtime.
  const launch = readPluginLaunchContext();
  if (
    typeof window !== "undefined" &&
    launch.runtime === "agentos" &&
    typeof launch.appId === "string" &&
    targetWindow === window
  ) {
    postNativeHostAction(message, launch.bundleId ?? launch.appId);
    return;
  }
  const resolvedTargetOrigin = targetOrigin ?? resolveHostTargetOrigin();
  const safeTargetOrigin = resolvedTargetOrigin === "null" || resolvedTargetOrigin.startsWith("file:")
    ? "*"
    : resolvedTargetOrigin;
  targetWindow.postMessage(message, safeTargetOrigin);
}

function postNativeHostAction(message, bundleId) {
  try {
    if (!isValidAppIdentity(bundleId)) return;
    const body = JSON.stringify(message);
    if (!body || body.length > 64 * 1024) return;
    const endpoint = window.location.protocol === "agentos-app:"
      ? new URL("/host/action", window.location.href)
      : new URL(`agentos-app://${bundleId}/host/action`);
    void fetch(endpoint.toString(), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      credentials: "omit",
      cache: "no-store",
      keepalive: true,
      body,
    }).catch(() => undefined);
  } catch {
    // Host communication is best-effort and must not break the plugin UI.
  }
}

function isValidAppIdentity(value) {
  if (typeof value !== "string" || value.length === 0 || value.length > 253) {
    return false;
  }
  return /^[A-Za-z0-9](?:[A-Za-z0-9_-]{0,61}[A-Za-z0-9])?(?:\.[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?)*$/.test(value);
}

let activeConfirmationCancel;

/**
 * Show the shared, non-native confirmation prompt used by embedded plugins.
 *
 * This is intentionally asynchronous. Native window.confirm() blocks the JS
 * thread and cannot be restyled; replacing it with a Promise would also break
 * every synchronous `if (confirm(...))` caller. Consumers must await the
 * boolean result instead.
 */
export function confirmAction(options) {
  if (typeof document === "undefined" || !document.body) return Promise.resolve(false);

  const normalized = typeof options === "string" ? { message: options } : (options ?? {});
  const message = typeof normalized.message === "string" ? normalized.message.trim() : "";
  if (!message) return Promise.resolve(false);

  activeConfirmationCancel?.();

  const launch = readPluginLaunchContext();
  const locale = normalized.locale === "en" || normalized.locale === "zh" ? normalized.locale : launch.locale;
  const confirmLabel = normalized.confirmLabel || (locale === "en" ? "Confirm" : "确认");
  const cancelLabel = normalized.cancelLabel || (locale === "en" ? "Cancel" : "取消");
  const title = typeof normalized.title === "string" ? normalized.title.trim() : "";
  const dark = document.documentElement.classList.contains("dark")
    || document.documentElement.dataset.theme === "dark"
    || launch.theme === "dark";
  const previouslyFocused = document.activeElement;

  return new Promise((resolve) => {
    const host = document.createElement("div");
    host.dataset.agentosConfirm = "";
    const shadow = host.attachShadow({ mode: "open" });
    shadow.innerHTML = `
      <style>
        :host { position: fixed; z-index: 2147483647; inset: 0; display: block; color-scheme: light; }
        * { box-sizing: border-box; }
        .layer {
          position: absolute; inset: 0; display: grid; place-items: center; overflow: auto;
          padding: 24px; background: rgba(3, 7, 18, .18); color: #17202a;
          font-family: "Noto Sans CJK SC", "Noto Sans", Inter, ui-sans-serif, system-ui, sans-serif;
        }
        .layer.dark { color-scheme: dark; color: #f7f8fa; }
        .prompt {
          width: min(420px, calc(100vw - 48px)); border: 1px solid rgba(20, 32, 44, .12);
          border-radius: 14px; padding: 20px;
          background: #fff; box-shadow: 0 12px 36px rgba(20, 32, 44, .22); animation: enter .16s ease-out;
        }
        .dark .prompt { border-color: rgba(255, 255, 255, .12); background: #2a2d30; box-shadow: 0 12px 36px rgba(0, 0, 0, .38); }
        .copy { min-width: 0; font-size: 14px; line-height: 1.6; }
        .title { margin-bottom: 4px; font-size: 15px; font-weight: 700; }
        .message { color: #17202a; overflow-wrap: anywhere; white-space: pre-line; }
        .dark .message { color: #f7f8fa; }
        .actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 18px; }
        button {
          min-width: 76px; min-height: 36px; border: 1px solid rgba(20, 32, 44, .12); border-radius: 9px;
          padding: 0 14px; color: #17202a; background: #fff; font: inherit; font-size: 13px; font-weight: 600; cursor: pointer;
        }
        button:hover { background: #eef2f6; }
        button:focus-visible { outline: 2px solid #2563eb; outline-offset: 2px; }
        .dark button { border-color: rgba(255, 255, 255, .12); color: #f7f8fa; background: #34383c; }
        .dark button:hover { background: #40454a; }
        .submit, .dark .submit { border-color: transparent; color: #fff; background: #2563eb; }
        .submit:hover, .dark .submit:hover { background: #1d4ed8; }
        @keyframes enter { from { opacity: 0; transform: translateY(-6px) scale(.985); } }
        @media (max-width: 640px) {
          .layer { padding: 16px; }
          .prompt { width: min(420px, calc(100vw - 32px)); padding: 18px; }
        }
        @media (prefers-reduced-motion: reduce) { .prompt { animation: none; } }
      </style>
      <div class="layer${dark ? " dark" : ""}">
        <div class="prompt" role="alertdialog" aria-modal="true" aria-describedby="agentos-confirm-message">
          <div class="copy"><div id="agentos-confirm-title" class="title"></div><div id="agentos-confirm-message" class="message"></div></div>
          <div class="actions"><button class="cancel" type="button"></button><button class="submit" type="button"></button></div>
        </div>
      </div>
    `;

    const layer = shadow.querySelector(".layer");
    const prompt = shadow.querySelector(".prompt");
    const titleElement = shadow.querySelector(".title");
    const messageElement = shadow.querySelector(".message");
    const cancelButton = shadow.querySelector(".cancel");
    const submitButton = shadow.querySelector(".submit");
    titleElement.textContent = title;
    titleElement.hidden = !title;
    if (title) prompt.setAttribute("aria-labelledby", "agentos-confirm-title");
    else prompt.setAttribute("aria-label", message);
    messageElement.textContent = message;
    cancelButton.textContent = cancelLabel;
    submitButton.textContent = confirmLabel;

    let settled = false;
    const settle = (confirmed) => {
      if (settled) return;
      settled = true;
      window.removeEventListener("keydown", onKeyDown, true);
      host.remove();
      if (activeConfirmationCancel === cancel) activeConfirmationCancel = undefined;
      if (previouslyFocused instanceof HTMLElement && previouslyFocused.isConnected) previouslyFocused.focus();
      resolve(confirmed);
    };
    const cancel = () => settle(false);
    const onKeyDown = (event) => {
      if (event.key === "Escape") {
        event.preventDefault();
        cancel();
        return;
      }
      if (event.key !== "Tab") return;
      const next = event.shiftKey ? submitButton : cancelButton;
      if ((event.shiftKey && shadow.activeElement === cancelButton) || (!event.shiftKey && shadow.activeElement === submitButton)) {
        event.preventDefault();
        next.focus();
      }
    };

    layer.addEventListener("pointerdown", (event) => { if (event.target === layer) cancel(); });
    prompt.addEventListener("pointerdown", (event) => event.stopPropagation());
    cancelButton.addEventListener("click", cancel);
    submitButton.addEventListener("click", () => settle(true));
    window.addEventListener("keydown", onKeyDown, true);
    activeConfirmationCancel = cancel;
    document.body.appendChild(host);
    queueMicrotask(() => cancelButton.focus());
  });
}

export const FENN_FILE_OPEN_REQUEST_KEY = "fenn:file-open-request";

/**
 * Ask the host to open an existing file in another embedded app. The request
 * is also kept in same-origin storage so the target app can consume it after
 * the host creates its iframe (and so older WebShell hosts remain compatible).
 */
export function requestFennFileOpen(bundleId, file) {
  const request = {
    type: "fenn:file-open-request",
    source: "fenn-files",
    requestId: createPickerRequestId(),
    bundleId,
    appId: bundleId,
    path: file?.path ?? "",
    name: file?.name ?? "",
    kind: file?.kind ?? "file",
    ...(file?.mimeType ? { mimeType: file.mimeType } : {}),
  };

  try {
    window.localStorage.setItem(FENN_FILE_OPEN_REQUEST_KEY, JSON.stringify(request));
  } catch {
    // Storage can be unavailable in restrictive iframe contexts.
  }

  if (
    window.parent &&
    (window.parent !== window || window.location.protocol === "agentos-app:")
  ) {
    postHostMessage({
      type: "shell:open-app",
      appKey: bundleId,
      openRequestId: request.requestId,
      openPath: request.path,
      openName: request.name,
      ...(request.mimeType ? { openMimeType: request.mimeType } : {}),
    });
  }
  return request;
}

export function readFennFileOpenRequest(bundleId) {
  try {
    const raw = window.localStorage.getItem(FENN_FILE_OPEN_REQUEST_KEY);
    if (!raw) return null;
    const request = JSON.parse(raw);
    if (request?.type !== "fenn:file-open-request" || request?.source !== "fenn-files") return null;
    if (bundleId && (request.bundleId ?? request.appId) !== bundleId) return null;
    if (typeof request.path !== "string" || !request.path) return null;
    return request;
  } catch {
    return null;
  }
}

export function clearFennFileOpenRequest(requestId) {
  try {
    const current = readFennFileOpenRequest();
    if (!requestId || !current || current.requestId === requestId) {
      window.localStorage.removeItem(FENN_FILE_OPEN_REQUEST_KEY);
    }
  } catch {
    // Storage can be unavailable in restrictive iframe contexts.
  }
}

const FILE_PICKER_REQUEST_TYPE = "fenn:file-picker-request";
const FILE_PICKER_RESULT_TYPE = "fenn:file-picker-result";
const FILE_PICKER_SOURCE = "fenn-files";
const FILE_PICKER_SDK_SOURCE = "fenn-files-sdk";
const BROWSER_FILE_PICKER_SOURCE = "browser";
const FENN_FILE_DRAG_TYPE = "fenn:file-drag";
export const FENN_FILE_DRAG_MIME = "application/x-fenn-files+json";
const FENN_FILE_DRAG_MIME_ALIAS = "application/vnd.fenn.files+json";
const FENN_FILE_DRAG_VERSION = 1;
const DEFAULT_PICKER_WIDTH = 680;
const DEFAULT_PICKER_HEIGHT = 480;
const DEFAULT_PICKER_FEATURES = `popup,width=${DEFAULT_PICKER_WIDTH},height=${DEFAULT_PICKER_HEIGHT},resizable=yes,scrollbars=yes`;
const DEFAULT_PICKER_TIMEOUT = 30 * 60 * 1000;

let hostRuntime = typeof window !== "undefined" && window.location.protocol === "agentos-app:"
  ? "agentos"
  : undefined;
let hostLocale;
let hostTimezone;
let hostFennFileManagerState;

if (typeof window !== "undefined") {
  window.addEventListener("message", (event) => {
    const data = event.data;
    if (
      data?.channel === "agentos:host" &&
      data?.type === "context-changed" &&
      data?.runtime === "agentos"
    ) {
      hostRuntime = "agentos";
      if (data.locale?.toLowerCase?.().startsWith("zh")) hostLocale = "zh";
      else if (data.locale?.toLowerCase?.().startsWith("en")) hostLocale = "en";
      if (typeof data.timezone === "string" && data.timezone.trim()) hostTimezone = data.timezone.trim();
      hostFennFileManagerState = normalizeHostFennFileManagerState(data.fennFileManagerState);
    }
  });
}

/**
 * Return true when the current app is hosted by AgentOS.
 * Browser WebShell pages deliberately remain on the native browser file APIs.
 */
export function isAgentosRuntime() {
  if (typeof window === "undefined") return false;
  if (window.location.protocol === "agentos-app:") return true;
  const params = new URLSearchParams(window.location.search);
  return params.get("runtime") === "agentos"
    || params.get("agentosRuntime") === "agentos"
    || hostRuntime === "agentos";
}

/**
 * Create the self-describing drag payload used by Fenn Files in the AgentOS
 * Shell. It contains metadata and paths only; the receiver must read content
 * through the protected file-manager API after the user explicitly drops it.
 */
export function createFennFileDragPayload(files = []) {
  const normalized = (Array.isArray(files) ? files : [])
    .map(normalizeFennFileDragItem)
    .filter(Boolean);
  return {
    type: FENN_FILE_DRAG_TYPE,
    source: FILE_PICKER_SOURCE,
    version: FENN_FILE_DRAG_VERSION,
    files: normalized,
  };
}

/**
 * Put a Fenn Files drag payload on a DataTransfer. text/plain is kept for
 * Fenn Files' internal move operation and for older consumers.
 */
export function setFennFileDragData(dataTransfer, files = []) {
  const payload = createFennFileDragPayload(files);
  const serialized = JSON.stringify(payload);
  dataTransfer.setData(FENN_FILE_DRAG_MIME, serialized);
  dataTransfer.setData(FENN_FILE_DRAG_MIME_ALIAS, serialized);
  dataTransfer.setData("text/plain", payload.files.map((file) => file.path).join("\n"));
  return payload;
}

/** Read and validate a Fenn Files drag payload, or return null for native files. */
export function readFennFileDrag(dataTransfer) {
  if (!dataTransfer) return null;
  const raw = dataTransfer.getData(FENN_FILE_DRAG_MIME) || dataTransfer.getData(FENN_FILE_DRAG_MIME_ALIAS);
  if (!raw) return null;
  try {
    const payload = JSON.parse(raw);
    if (payload?.type !== FENN_FILE_DRAG_TYPE || payload?.source !== FILE_PICKER_SOURCE || payload?.version !== FENN_FILE_DRAG_VERSION) return null;
    const files = (Array.isArray(payload.files) ? payload.files : [])
      .map(normalizeFennFileDragItem)
      .filter(Boolean);
    return files.length > 0 ? { ...payload, files } : null;
  } catch {
    return null;
  }
}

/**
 * Materialize a selected Fenn Files item for an app. Browser selections carry
 * a native File directly; AgentOS shell selections carry a path and are read via
 * the protected Fenn Files API.
 */
export async function filePickerFileToBlob(file, options = {}) {
  if (file?.file instanceof Blob) return file.file;
  if (!file?.path) throw new FennFilePickerError("选择结果没有可读取的文件路径", "missing-path");
  const apiModule = await import("./admin-api.js");
  const launch = readPluginLaunchContext();
  const launchApiBase = typeof launch.apiBase === "string" ? launch.apiBase.trim() : "";
  const apiBase = options.apiBase?.trim()
    || launchApiBase
    || defaultFennFilesApiBase();
  const api = options.api ?? apiModule.createAdminApi({ baseUrl: apiBase });
  return api.fileManagerRead(file.path, options.maxBytes);
}

/**
 * Return an authenticated media URL for a Fenn Files item. The URL supports
 * byte ranges, so media elements can seek without loading the whole file.
 */
export async function filePickerFileToMediaUrl(file, options = {}) {
  if (file?.file instanceof Blob) return URL.createObjectURL(file.file);
  if (!file?.path) throw new FennFilePickerError("选择结果没有可读取的文件路径", "missing-path");
  const apiModule = await import("./admin-api.js");
  const launch = readPluginLaunchContext();
  const launchApiBase = typeof launch.apiBase === "string" ? launch.apiBase.trim() : "";
  const apiBase = options.apiBase?.trim()
    || launchApiBase
    || defaultFennFilesApiBase();
  const api = options.api ?? apiModule.createAdminApi({ baseUrl: apiBase });
  if (typeof api.fileManagerMediaUrl === "function") return api.fileManagerMediaUrl(file.path);
  const params = new URLSearchParams({ path: file.path });
  return new URL(`/admin/api/file-manager/media?${params}`, apiBase).toString();
}

const fennUploadFolderPromises = new Map();

/**
 * Upload browser-native files into a folder under the current Fenn Files
 * user's home directory. The returned items contain the actual saved paths,
 * including the collision-safe name chosen by the file manager.
 */
export async function uploadFilesToFennFolder(files, folderName, options = {}) {
  const selectedFiles = Array.from(files ?? []).filter((file) => file instanceof Blob && typeof file.name === "string");
  if (!selectedFiles.length) return [];
  const folderSegments = normalizeFennFolderPath(folderName);

  const apiModule = await import("./admin-api.js");
  const launch = readPluginLaunchContext();
  const launchApiBase = typeof launch.apiBase === "string" ? launch.apiBase.trim() : "";
  const apiBase = options.apiBase?.trim()
    || launchApiBase
    || defaultFennFilesApiBase();
  const api = options.api ?? apiModule.createAdminApi({ baseUrl: apiBase });
  const folderPath = await ensureFennUploadFolder(api, apiBase, folderSegments);
  const uploaded = [];

  for (const file of selectedFiles) {
    const payload = await api.fileManagerUpload(
      folderPath,
      file.name,
      file,
      (progress) => options.onProgress?.(file, progress),
    ).promise;
    const entry = payload?.entry;
    if (!entry || typeof entry.path !== "string" || !entry.path) {
      throw new FennFilePickerError(`${file.name}: Fenn Files 未返回上传路径`, "upload-response");
    }
    uploaded.push({
      file,
      name: typeof entry.name === "string" && entry.name ? entry.name : file.name,
      path: entry.path,
      kind: "file",
      size: typeof entry.size === "number" ? entry.size : file.size,
      ...(typeof entry.mimeType === "string" && entry.mimeType ? { mimeType: entry.mimeType } : file.type ? { mimeType: file.type } : {}),
    });
  }

  return uploaded;
}

async function ensureFennUploadFolder(api, apiBase, folderSegments) {
  const relativePath = folderSegments.join("/");
  const key = `${apiBase}\u0000${relativePath}`;
  const pending = fennUploadFolderPromises.get(key);
  if (pending) return pending;

  const promise = (async () => {
    let listing = await api.fileManagerList();
    let currentPath = typeof listing?.defaultPath === "string" && listing.defaultPath
      ? listing.defaultPath
      : typeof listing?.path === "string" && listing.path
        ? listing.path
        : "";
    if (!currentPath) throw new FennFilePickerError("Fenn Files 未返回当前用户 Home 路径", "missing-home");

    for (const folderName of folderSegments) {
      const existing = Array.isArray(listing.entries)
        ? listing.entries.find((entry) => entry?.name === folderName)
        : undefined;
      if (existing) {
        if (existing.kind !== "directory" || typeof existing.path !== "string" || !existing.path) {
          throw new FennFilePickerError(`${folderName} 不是文件夹`, "folder-conflict");
        }
        currentPath = existing.path;
      } else {
        const targetPath = joinFennPath(currentPath, folderName);
        try {
          await api.fileManagerMkdir(targetPath);
          currentPath = targetPath;
        } catch (error) {
          // Another tab/app may have created this path between list and mkdir.
          const refreshed = await api.fileManagerList({ path: currentPath });
          const created = Array.isArray(refreshed.entries)
            ? refreshed.entries.find((entry) => entry?.name === folderName)
            : undefined;
          if (created?.kind === "directory" && typeof created.path === "string" && created.path) {
            currentPath = created.path;
          } else {
            throw error;
          }
        }
      }
      listing = await api.fileManagerList({ path: currentPath });
    }
    return currentPath;
  })();

  fennUploadFolderPromises.set(key, promise);
  try {
    return await promise;
  } finally {
    if (fennUploadFolderPromises.get(key) === promise) fennUploadFolderPromises.delete(key);
  }
}

function joinFennPath(base, child) {
  const trimmedBase = base.replace(/[\\/]+$/, "");
  const separator = trimmedBase.includes("\\") && !trimmedBase.includes("/") ? "\\" : "/";
  return `${trimmedBase}${separator}${child}`;
}

function normalizeFennFolderPath(value) {
  const rawPath = typeof value === "string" ? value.trim() : "";
  const segments = rawPath.split(/[\\/]/);
  if (
    !rawPath ||
    rawPath.startsWith("/") ||
    rawPath.startsWith("\\") ||
    /^[A-Za-z]:/.test(rawPath) ||
    segments.some((segment) => !segment || segment === "." || segment === ".." || segment.includes("\u0000"))
  ) {
    throw new FennFilePickerError("Fenn Files 目标文件夹路径无效", "invalid-folder");
  }
  return segments;
}

/**
 * Build the app-to-host request that a native Shell/WebShell host can route to
 * an already managed Fenn Files window. The current host also supports the
 * iframe/popup helper below, so callers do not need to wait for host routing.
 */
export function createFennFilePickerRequest(options = {}) {
  const mode = normalizePickerMode(options.mode);
  const requestId = typeof options.requestId === "string" && options.requestId.trim()
    ? options.requestId.trim()
    : createPickerRequestId();
  const accept = normalizeAccept(options.accept);

  return {
    type: FILE_PICKER_REQUEST_TYPE,
    source: FILE_PICKER_SDK_SOURCE,
    requestId,
    mode,
    accept,
    multiple: mode === "open" && options.multiple === true,
    ...(options.initialPath ? { initialPath: options.initialPath } : {}),
    ...(options.suggestedName ? { suggestedName: options.suggestedName } : {}),
    ...(options.returnOrigin ? { returnOrigin: options.returnOrigin } : {}),
  };
}

/**
 * Open a file dialog using the host's native implementation. WebShell uses
 * browser APIs; the AgentOS Shell opens the Fenn Files Embed App.
 */
export function openFilePicker(options = {}) {
  return isAgentosRuntime()
    ? openFennFilePicker(options)
    : openBrowserFilePicker(options);
}

/**
 * Save bytes through the host's native path. The AgentOS Shell confirms a Fenn
 * Files save target and writes through the protected API; WebShell uses the
 * browser save handle when available and otherwise falls back to a download.
 */
export async function saveFile(bytes, options = {}) {
  const suggestedName = options.suggestedName?.trim() || "download";
  if (isAgentosRuntime()) {
    const result = await openFennFilePicker({
      mode: "save",
      suggestedName,
      accept: options.accept,
      initialPath: options.initialPath,
      presentation: options.presentation,
    });
    if (!result.ok || result.canceled) return result;
    const savePath = result.savePath || result.paths[0];
    if (!savePath) throw new FennFilePickerError("保存结果没有目标路径", "missing-save-path");
    const apiModule = await import("./admin-api.js");
    const apiBase = options.apiBase?.trim() || "agentos-app://file-manager/admin";
    const api = options.api ?? apiModule.createAdminApi({ baseUrl: apiBase });
    await api.fileManagerWrite(savePath, bytes);
    return { ...result, written: true };
  }

  const blob = toBlob(bytes);
  if (typeof window.showSaveFilePicker === "function") {
    try {
      const fileHandle = await window.showSaveFilePicker({
        suggestedName,
        ...(options.types ? { types: options.types } : {}),
      });
      const writable = await fileHandle.createWritable();
      await writable.write(blob);
      await writable.close();
      return {
        ...createBrowserPickerResult({ requestId: createPickerRequestId(), mode: "save" }),
        ok: true,
        fileHandle,
        suggestedName: fileHandle.name,
        written: true,
      };
    } catch (error) {
      if (error?.name === "AbortError") return createBrowserCanceledPickerResult({ requestId: createPickerRequestId(), mode: "save" });
      throw new FennFilePickerError("浏览器保存文件失败", "native-picker-failed");
    }
  }

  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = suggestedName;
  anchor.style.display = "none";
  document.body?.append(anchor);
  anchor.click();
  anchor.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 0);
  return {
    ...createBrowserPickerResult({ requestId: createPickerRequestId(), mode: "save" }),
    ok: true,
    suggestedName,
    downloaded: true,
  };
}

/**
 * Send the same request shape to a host that implements native window routing.
 * It is intentionally fire-and-forget; use openFennFilePicker for a working
 * result Promise on the current Shell/WebShell versions.
 */
export function postFennFilePickerRequest(options = {}, targetWindow = window.parent, targetOrigin) {
  const request = createFennFilePickerRequest(options);
  if (targetWindow && targetWindow !== window) {
    targetWindow.postMessage(request, targetOrigin ?? resolveHostTargetOrigin());
  }
  return request;
}

/**
 * Open Fenn Files as a nested Embed App (default) or a popup and resolve with
 * the same `fenn:file-picker-result` payload used by direct URL embedding.
 * The call should normally happen inside a user gesture, especially for the
 * popup presentation.
 */
export function openFennFilePicker(options = {}) {
  const request = createFennFilePickerRequest(options);
  // CEF OSR has one Flutter texture per App and intentionally does not create
  // native popup windows. Keep the picker as an in-document iframe in the
  // AgentOS runtime; browser WebShell callers retain the real popup path.
  const presentation = isAgentosRuntime()
    ? "iframe"
    : options.presentation === "popup" ? "popup" : "iframe";
  if (presentation === "iframe" && options.delegateToHost !== false && shouldDelegateFilePickerToShellHost()) {
    return openShellFilePickerHost(request, options);
  }
  const pickerUrl = buildFennPickerUrl(request, options);

  return presentation === "popup"
    ? openPickerPopup(request, pickerUrl, options)
    : openPickerIframe(request, pickerUrl, options);
}

export class FennFilePickerError extends Error {
  constructor(message, code = "picker-error") {
    super(message);
    this.name = "FennFilePickerError";
    this.code = code;
  }
}

function openBrowserFilePicker(options = {}) {
  const request = createFennFilePickerRequest(options);
  if (request.mode === "directory") return openBrowserDirectoryPicker(request);
  if (request.mode === "save") return openBrowserSavePicker(request, options);
  return openBrowserFileInput(request);
}

function openBrowserFileInput(request) {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.multiple = request.multiple;
    if (request.accept.length > 0) input.accept = request.accept.join(",");
    input.style.position = "fixed";
    input.style.left = "-10000px";
    input.style.top = "-10000px";
    let settled = false;
    const finish = (result) => {
      if (settled) return;
      settled = true;
      input.remove();
      window.removeEventListener("focus", onWindowFocus, true);
      resolve(result);
    };
    const completeSelection = () => {
      const files = [...(input.files ?? [])].map(browserFileToPickerFile);
      finish({
        ...createBrowserPickerResult(request),
        ok: files.length > 0,
        canceled: files.length === 0,
        files,
      });
    };
    const onWindowFocus = () => {
      window.setTimeout(() => {
        if (settled) return;
        // Native dialogs can restore focus before the browser has populated
        // the input and dispatched `change`. Give that handoff time to finish;
        // otherwise a valid selection is mistaken for a cancellation and the
        // import caller silently returns.
        if (input.files?.length) completeSelection();
        else finish(createBrowserCanceledPickerResult(request));
      }, 1000);
    };
    input.addEventListener("change", completeSelection, { once: true });
    input.addEventListener("cancel", () => finish(createBrowserCanceledPickerResult(request)), { once: true });
    window.addEventListener("focus", onWindowFocus, true);
    document.body?.append(input);
    input.click();
  });
}

async function openBrowserDirectoryPicker(request) {
  if (typeof window.showDirectoryPicker === "function") {
    try {
      const directoryHandle = await window.showDirectoryPicker({ mode: "read" });
      return {
        ...createBrowserPickerResult(request),
        ok: true,
        canceled: false,
        directoryHandle,
        directoryName: directoryHandle.name,
      };
    } catch (error) {
      if (error?.name === "AbortError") return createBrowserCanceledPickerResult(request);
      throw new FennFilePickerError("浏览器目录选择器打开失败", "native-picker-failed");
    }
  }

  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.multiple = true;
    input.setAttribute("webkitdirectory", "");
    input.style.position = "fixed";
    input.style.left = "-10000px";
    input.style.top = "-10000px";
    let settled = false;
    const finish = (result) => {
      if (settled) return;
      settled = true;
      input.remove();
      window.removeEventListener("focus", onWindowFocus, true);
      resolve(result);
    };
    const onWindowFocus = () => {
      window.setTimeout(() => {
        if (!settled && input.files?.length === 0) finish(createBrowserCanceledPickerResult(request));
      }, 250);
    };
    input.addEventListener("change", () => {
      const first = input.files?.[0];
      const directoryName = first?.webkitRelativePath?.split("/")[0] || undefined;
      finish({
        ...createBrowserPickerResult(request),
        ok: Boolean(first),
        canceled: !first,
        directoryName,
      });
    }, { once: true });
    input.addEventListener("cancel", () => finish(createBrowserCanceledPickerResult(request)), { once: true });
    window.addEventListener("focus", onWindowFocus, true);
    document.body?.append(input);
    input.click();
  });
}

async function openBrowserSavePicker(request, options) {
  if (typeof window.showSaveFilePicker !== "function") {
    throw new FennFilePickerError("当前浏览器不支持原生保存位置选择器，请使用下载功能", "unsupported");
  }
  try {
    const fileHandle = await window.showSaveFilePicker({
      suggestedName: request.suggestedName || "untitled",
      ...(options.types ? { types: options.types } : {}),
    });
    return {
      ...createBrowserPickerResult(request),
      ok: true,
      canceled: false,
      fileHandle,
      suggestedName: request.suggestedName || fileHandle.name,
    };
  } catch (error) {
    if (error?.name === "AbortError") return createBrowserCanceledPickerResult(request);
    throw new FennFilePickerError("浏览器保存位置选择器打开失败", "native-picker-failed");
  }
}

function createBrowserPickerResult(request) {
  return {
    type: FILE_PICKER_RESULT_TYPE,
    source: BROWSER_FILE_PICKER_SOURCE,
    requestId: request.requestId,
    mode: request.mode,
    ok: false,
    canceled: false,
    paths: [],
    files: [],
  };
}

function createBrowserCanceledPickerResult(request) {
  return {
    ...createBrowserPickerResult(request),
    canceled: true,
  };
}

function browserFileToPickerFile(file) {
  return {
    name: file.name,
    kind: "file",
    size: file.size,
    ...(file.type ? { mimeType: file.type } : {}),
    file,
  };
}

function toBlob(bytes) {
  return bytes instanceof Blob ? bytes : new Blob([bytes]);
}

function resolveHostTargetOrigin() {
  const { hostOrigin } = readPluginLaunchContext();
  if (hostOrigin && hostOrigin !== "null" && !hostOrigin.startsWith("file:")) {
    return hostOrigin;
  }

  const ancestorOrigin = window.location.ancestorOrigins?.[0];
  if (ancestorOrigin) {
    return ancestorOrigin;
  }

  if (document.referrer) {
    try {
      return new URL(document.referrer).origin;
    } catch {
      // Ignore invalid referrer values and continue falling back.
    }
  }

  return window.location.origin;
}

function shouldDelegateFilePickerToShellHost() {
  return isAgentosRuntime()
    && typeof window !== "undefined"
    && window.parent
    && window.parent !== window;
}

function openShellFilePickerHost(request, options) {
  return new Promise((resolve, reject) => {
    const parentWindow = window.parent;
    let settled = false;
    let timeoutId;
    const cleanup = () => {
      window.removeEventListener("message", onMessage);
      if (timeoutId) window.clearTimeout(timeoutId);
    };
    const finish = (result) => {
      if (settled) return;
      settled = true;
      cleanup();
      resolve(result);
    };
    const fail = (error) => {
      if (settled) return;
      settled = true;
      cleanup();
      reject(error);
    };
    const onMessage = (event) => {
      if (event.source !== parentWindow || !isPickerResult(event.data, request.requestId)) return;
      finish(event.data);
    };

    window.addEventListener("message", onMessage);
    timeoutId = window.setTimeout(() => {
      fail(new FennFilePickerError("Shell 文件选择器等待超时", "host-timeout"));
    }, options.timeoutMs ?? DEFAULT_PICKER_TIMEOUT);
    try {
      const resolvedOrigin = resolveHostTargetOrigin();
      const targetOrigin = resolvedOrigin === "null" || resolvedOrigin.startsWith("file:") ? "*" : resolvedOrigin;
      parentWindow.postMessage(request, targetOrigin);
    } catch (error) {
      fail(new FennFilePickerError(
        error instanceof Error ? error.message : "Shell 文件选择器不可用",
        "host-unavailable",
      ));
    }
  });
}

function buildFennPickerUrl(request, options) {
  const context = readPluginLaunchContext();
  const launchParams = new URLSearchParams(window.location.search);
  const rawUrl = options.pickerUrl?.trim() || defaultFennPickerUrl();
  const pickerUrl = new URL(rawUrl, window.location.href);
  const params = pickerUrl.searchParams;

  params.set("mode", request.mode);
  params.set("requestId", request.requestId);
  if (request.initialPath) params.set("initialPath", request.initialPath);
  if (request.accept.length > 0) params.set("accept", request.accept.join(","));
  if (request.multiple) params.set("multiple", "1");
  if (request.suggestedName) params.set("suggestedName", request.suggestedName);

  const apiBase = options.apiBase?.trim() || new URL("/admin", pickerUrl).toString();
  params.set("apiBase", apiBase);

  const returnOrigin = normalizeHttpOrigin(options.returnOrigin ?? window.location.origin);
  if (returnOrigin) params.set("returnOrigin", returnOrigin);

  if (!params.has("locale") && launchParams.has("locale") && context.locale) params.set("locale", context.locale);
  if (!params.has("locale")) {
    const synchronizedLocale = hostFennFileManagerState?.locale ?? hostLocale;
    if (synchronizedLocale) params.set("locale", synchronizedLocale);
  }
  if (!params.has("appearance") && launchParams.has("appearance") && context.appearance) params.set("appearance", context.appearance);
  if (!params.has("theme") && launchParams.has("theme") && context.theme) params.set("theme", context.theme);
  if (!params.has("zoom") && launchParams.has("zoom") && context.zoom !== undefined) params.set("zoom", String(context.zoom));
  if (!params.has("favorites") && hostFennFileManagerState) {
    params.set("favorites", JSON.stringify(hostFennFileManagerState.favorites));
  }

  return pickerUrl;
}

function defaultFennPickerUrl() {
  if (isAgentosRuntime()) {
    return "agentos-app://file-manager/index.html";
  }
  const { hostOrigin } = readPluginLaunchContext();
  const normalizedHostOrigin = normalizeHttpOrigin(hostOrigin);
  if (normalizedHostOrigin) {
    return new URL("/apps/file-manager/index.html", `${normalizedHostOrigin}/`).toString();
  }
  return new URL("/apps/file-manager/index.html", window.location.href).toString();
}

function defaultFennFilesApiBase() {
  if (isAgentosRuntime()) {
    return "agentos-app://file-manager/admin";
  }
  const { hostOrigin } = readPluginLaunchContext();
  const normalizedHostOrigin = normalizeHttpOrigin(hostOrigin);
  if (normalizedHostOrigin) {
    return new URL("/admin", `${normalizedHostOrigin}/`).toString();
  }
  return `${window.location.origin}/admin`;
}

function openPickerIframe(request, pickerUrl, options) {
  return new Promise((resolve, reject) => {
    const host = options.container instanceof HTMLElement ? options.container : document.body;
    if (!host) {
      reject(new FennFilePickerError("无法创建 Fenn Files 选择器容器", "no-container"));
      return;
    }

    const overlay = document.createElement("div");
    overlay.setAttribute("role", "presentation");
    Object.assign(overlay.style, {
      position: "fixed",
      inset: "0",
      zIndex: "2147483000",
      display: "grid",
      placeItems: "center",
      padding: "24px",
      background: "rgba(0, 0, 0, 0.48)",
    });

    const panel = document.createElement("div");
    panel.setAttribute("role", "dialog");
    panel.setAttribute("aria-label", "Fenn Files");
    Object.assign(panel.style, {
      position: "relative",
      width: `min(${DEFAULT_PICKER_WIDTH}px, calc(100% - 32px))`,
      height: `min(${DEFAULT_PICKER_HEIGHT}px, calc(100% - 32px))`,
      overflow: "hidden",
      borderRadius: "14px",
      background: "#fff",
      boxShadow: "0 24px 80px rgba(0, 0, 0, 0.32)",
    });

    const iframe = document.createElement("iframe");
    iframe.title = "Fenn Files";
    iframe.src = pickerUrl.toString();
    iframe.setAttribute("allow", "clipboard-read; clipboard-write");
    Object.assign(iframe.style, {
      display: "block",
      width: "100%",
      height: "100%",
      border: "0",
      background: "#fff",
    });

    panel.append(iframe);
    overlay.append(panel);
    host.append(overlay);

    const expectedOrigin = normalizePickerOrigin(pickerUrl.origin);
    let settled = false;
    let timeoutId;

    const cleanup = () => {
      window.removeEventListener("message", onMessage);
      iframe.removeEventListener("error", onError);
      if (timeoutId) window.clearTimeout(timeoutId);
      overlay.remove();
    };
    const finish = (result) => {
      if (settled) return;
      settled = true;
      cleanup();
      resolve(result);
    };
    const fail = (error) => {
      if (settled) return;
      settled = true;
      cleanup();
      reject(error);
    };
    const onMessage = (event) => {
      if (event.source !== iframe.contentWindow || !isPickerResult(event.data, request.requestId)) return;
      if (expectedOrigin && event.origin && event.origin !== "null" && event.origin !== expectedOrigin) return;
      finish(event.data);
    };
    const onError = () => fail(new FennFilePickerError("Fenn Files 选择器加载失败", "load-failed"));

    window.addEventListener("message", onMessage);
    iframe.addEventListener("error", onError, { once: true });
    timeoutId = window.setTimeout(() => {
      fail(new FennFilePickerError("Fenn Files 选择器等待超时", "timeout"));
    }, options.timeoutMs ?? DEFAULT_PICKER_TIMEOUT);
  });
}

function openPickerPopup(request, pickerUrl, options) {
  return new Promise((resolve, reject) => {
    const expectedOrigin = normalizePickerOrigin(pickerUrl.origin);
    const windowName = `fenn-files-${request.requestId.replace(/[^a-z0-9_-]/gi, "").slice(0, 40) || "picker"}`;
    const features = options.windowFeatures || DEFAULT_PICKER_FEATURES;
    let pickerWindow;
    let settled = false;
    let timeoutId;
    let closePollId;

    const cleanup = () => {
      window.removeEventListener("message", onMessage);
      if (timeoutId) window.clearTimeout(timeoutId);
      if (closePollId) window.clearInterval(closePollId);
    };
    const finish = (result) => {
      if (settled) return;
      settled = true;
      cleanup();
      try {
        pickerWindow?.close();
      } catch {
        // A cross-origin popup may reject close; the result is still valid.
      }
      resolve(result);
    };
    const fail = (error) => {
      if (settled) return;
      settled = true;
      cleanup();
      reject(error);
    };
    const onMessage = (event) => {
      if (event.source !== pickerWindow || !isPickerResult(event.data, request.requestId)) return;
      if (expectedOrigin && event.origin && event.origin !== "null" && event.origin !== expectedOrigin) return;
      finish(event.data);
    };

    window.addEventListener("message", onMessage);
    pickerWindow = window.open(pickerUrl.toString(), windowName, features);
    if (!pickerWindow) {
      fail(new FennFilePickerError("浏览器阻止了 Fenn Files 弹窗，请允许弹窗或改用 iframe 模式", "popup-blocked"));
      return;
    }

    timeoutId = window.setTimeout(() => {
      fail(new FennFilePickerError("Fenn Files 选择器等待超时", "timeout"));
    }, options.timeoutMs ?? DEFAULT_PICKER_TIMEOUT);
    closePollId = window.setInterval(() => {
      if (pickerWindow.closed) finish(createCanceledPickerResult(request));
    }, 250);
  });
}

function isPickerResult(value, requestId) {
  return Boolean(
    value &&
      typeof value === "object" &&
      value.type === FILE_PICKER_RESULT_TYPE &&
      value.source === FILE_PICKER_SOURCE &&
      value.requestId === requestId,
  );
}

function createCanceledPickerResult(request) {
  return {
    type: FILE_PICKER_RESULT_TYPE,
    source: FILE_PICKER_SOURCE,
    requestId: request.requestId,
    mode: request.mode,
    ok: false,
    canceled: true,
    paths: [],
    files: [],
  };
}

function normalizeAccept(value) {
  const values = Array.isArray(value) ? value : typeof value === "string" ? value.split(",") : [];
  return values.map((item) => String(item).trim()).filter(Boolean);
}

function normalizeHostFennFileManagerState(value) {
  if (!value || typeof value !== "object") return undefined;
  if (value.locale !== "zh" && value.locale !== "en") return undefined;
  if (!Array.isArray(value.favorites)) return undefined;
  return {
    locale: value.locale,
    favorites: [...new Set(value.favorites.filter((path) => typeof path === "string" && path.length > 0))],
  };
}

function normalizePickerMode(value) {
  if (value === "save") return "save";
  if (value === "directory" || value === "folder" || value === "pick-directory" || value === "pick-folder") return "directory";
  return "open";
}

function normalizeFennFileDragItem(value) {
  if (!value || typeof value !== "object") return null;
  const path = typeof value.path === "string" ? value.path.trim() : "";
  if (!path) return null;
  const name = typeof value.name === "string" && value.name.trim() ? value.name : path.split(/[\\/]/).pop() || path;
  const kind = typeof value.kind === "string" && value.kind ? value.kind : "file";
  const size = typeof value.size === "number" && Number.isFinite(value.size) && value.size >= 0 ? value.size : undefined;
  const mimeType = typeof value.mimeType === "string" && value.mimeType ? value.mimeType : undefined;
  return {
    name,
    path,
    kind,
    ...(size === undefined ? {} : { size }),
    ...(mimeType ? { mimeType } : {}),
  };
}

function normalizeHttpOrigin(value) {
  if (!value || value === "null" || value.startsWith("file:")) return undefined;
  try {
    const url = new URL(value);
    return url.protocol === "http:" || url.protocol === "https:" ? url.origin : undefined;
  } catch {
    return undefined;
  }
}

function normalizePickerOrigin(value) {
  if (!value || value === "null" || value.startsWith("file:")) return undefined;
  try {
    const url = new URL(value);
    return url.origin === "null" ? undefined : url.origin;
  } catch {
    return undefined;
  }
}

function createPickerRequestId() {
  const random = Math.random().toString(36).slice(2, 10);
  return `fenn-picker-${Date.now()}-${random}`;
}
