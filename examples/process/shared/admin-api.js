export class AdminApiError extends Error {
  constructor(message, status, payload) {
    super(message);
    this.name = "AdminApiError";
    this.status = status;
    this.payload = payload;
  }
}

export class AdminApi {
  constructor(options = {}) {
    this.baseUrl = options.baseUrl ?? defaultBaseUrl();
    this.fetchImpl = options.fetchImpl ?? fetch.bind(globalThis);
    this.adminTokenPromise = undefined;
  }

  authSession() {
    return this.#request("/admin/api/auth/session");
  }

  services() {
    return this.#request("/admin/api/services/");
  }

  startService(serviceKey) {
    return this.#request(`/admin/api/services/${encodeURIComponent(serviceKey)}/start`, {
      method: "POST",
    });
  }

  stopService(serviceKey) {
    return this.#request(`/admin/api/services/${encodeURIComponent(serviceKey)}/stop`, {
      method: "POST",
    });
  }

  systemInfo() {
    return this.#request("/admin/api/system/info");
  }

  downloadEventsUrl() {
    return new URL("/admin/api/models/download-events", this.baseUrl).toString();
  }

  models() {
    return this.#request("/admin/api/models/");
  }

  downloadModel(id) {
    return this.#postSuccess(`/admin/api/models/${encodeURIComponent(id)}/download`);
  }

  pauseDownload(id) {
    return this.#postSuccess(`/admin/api/models/${encodeURIComponent(id)}/pause`);
  }

  resumeDownload(id) {
    return this.#postSuccess(`/admin/api/models/${encodeURIComponent(id)}/resume`);
  }

  cancelDownload(id) {
    return this.#postSuccess(`/admin/api/models/${encodeURIComponent(id)}/cancel`);
  }

  startModel(id) {
    return this.#postSuccess(`/admin/api/models/${encodeURIComponent(id)}/start`);
  }

  stopModel(id) {
    return this.#postSuccess(`/admin/api/models/${encodeURIComponent(id)}/stop`);
  }

  deleteModel(id) {
    return this.#request(`/admin/api/models/${encodeURIComponent(id)}`, {
      method: "DELETE",
    });
  }

  setModelContextSize(id, contextSize) {
    return this.#request(`/admin/api/models/${encodeURIComponent(id)}/context-size`, {
      method: "PUT",
      body: { contextSize },
    });
  }

  importModel(payload) {
    return this.#request("/admin/api/models/import", {
      method: "POST",
      body: payload,
    });
  }

  remoteModels() {
    return this.#request("/admin/api/remote-models/");
  }

  addRemoteModel(payload) {
    return this.#request("/admin/api/remote-models/", {
      method: "POST",
      body: payload,
    });
  }

  updateRemoteModel(id, payload) {
    return this.#request(`/admin/api/remote-models/${encodeURIComponent(id)}`, {
      method: "PUT",
      body: payload,
    });
  }

  deleteRemoteModel(id) {
    return this.#request(`/admin/api/remote-models/${encodeURIComponent(id)}`, {
      method: "DELETE",
    });
  }

  toggleRemoteModel(id) {
    return this.#postSuccess(`/admin/api/remote-models/${encodeURIComponent(id)}/toggle`);
  }

  togglePlatformRemoteModel(id) {
    return this.#postSuccess(`/admin/api/remote-models/platform/${encodeURIComponent(id)}/toggle`);
  }

  accountStatus() {
    return this.#request("/admin/api/account/");
  }

  accountDeviceCodeStart(payload) {
    return this.#request("/admin/api/account/device-code/start", {
      method: "POST",
      body: payload,
    });
  }

  accountDeviceCodePoll(payload) {
    return this.#request("/admin/api/account/device-code/poll", {
      method: "POST",
      body: payload,
    });
  }

  accountDeviceCodeCancel(payload = {}) {
    return this.#request("/admin/api/account/device-code/cancel", {
      method: "POST",
      body: payload,
    });
  }

  accountLogout() {
    return this.#request("/admin/api/account/logout", {
      method: "POST",
    });
  }

  accountRefreshModels() {
    return this.#request("/admin/api/account/refresh-models", {
      method: "POST",
    });
  }

  localAccountStatus() {
    return this.#request("/admin/api/local-account/");
  }

  changeLocalAccountPassword(payload) {
    return this.#request("/admin/api/local-account/password", {
      method: "PUT",
      body: payload,
    });
  }

  changeLocalAccountUsername(payload) {
    return this.#request("/admin/api/local-account/username", {
      method: "PUT",
      body: payload,
    });
  }

  settings() {
    return this.#request("/admin/api/settings/");
  }

  updateSettings(payload) {
    return this.#request("/admin/api/settings/", {
      method: "PUT",
      body: payload,
    });
  }

  fileManagerList(options = {}) {
    const params = new URLSearchParams();
    if (options.path) params.set("path", options.path);
    if (options.showHidden) params.set("showHidden", "1");
    const suffix = params.size ? `?${params}` : "";
    return this.#request(`/admin/api/file-manager/list${suffix}`);
  }

  fileManagerStat(path) {
    return this.#request(`/admin/api/file-manager/stat?path=${encodeURIComponent(path)}`);
  }

  fileManagerVolumes() {
    return this.#request("/admin/api/file-manager/volumes");
  }

  fileManagerReadUrl(path, maxBytes = 32 * 1024 * 1024) {
    const params = new URLSearchParams({ path, maxBytes: String(maxBytes) });
    return new URL(`/admin/api/file-manager/read?${params}`, this.baseUrl).toString();
  }

  fileManagerMediaUrl(path) {
    const params = new URLSearchParams({ path });
    return new URL(`/admin/api/file-manager/media?${params}`, this.baseUrl).toString();
  }

  fileManagerUploadUrl(directory, filename) {
    const params = new URLSearchParams({ directory, filename });
    return new URL(`/admin/api/file-manager/upload?${params}`, this.baseUrl).toString();
  }

  fileManagerDownloadUrl(path) {
    const params = new URLSearchParams({ path });
    return new URL(`/admin/api/file-manager/download?${params}`, this.baseUrl).toString();
  }

  async fileManagerRead(path, maxBytes = 32 * 1024 * 1024) {
    const token = await this.#resolveAdminToken();
    let response;
    try {
      response = await this.fetchImpl(this.fileManagerReadUrl(path, maxBytes), {
        method: "GET",
        headers: {
          Accept: "*/*",
          ...(token ? { "x-admin-token": token } : {}),
        },
        cache: "no-store",
        credentials: token ? "omit" : "include",
      });
    } catch (error) {
      throw new AdminApiError(resolveNetworkMessage(this.baseUrl, error), 0, error);
    }
    if (!response.ok) {
      const payload = await parseJson(response);
      throw new AdminApiError(resolveErrorMessage(payload, response.status), response.status, payload);
    }
    return response.blob();
  }

  fileManagerWrite(path, bytes) {
    return this.#request(`/admin/api/file-manager/write?path=${encodeURIComponent(path)}`, {
      method: "PUT",
      body: bytes,
    });
  }

  fileManagerUpload(directory, filename, bytes, onProgress) {
    let xhr;
    let canceled = false;
    const promise = (async () => {
      const token = await this.#resolveAdminToken();
      if (canceled) throw createAbortError();
      return new Promise((resolve, reject) => {
        xhr = new XMLHttpRequest();
        xhr.open("PUT", this.fileManagerUploadUrl(directory, filename), true);
        xhr.withCredentials = !token;
        xhr.setRequestHeader("Accept", "application/json");
        if (token) xhr.setRequestHeader("x-admin-token", token);
        xhr.upload.addEventListener("progress", (event) => {
          if (event.lengthComputable) onProgress?.(Math.max(0, Math.min(1, event.loaded / event.total)));
        });
        xhr.onload = () => {
          const payload = parseJsonText(xhr.responseText);
          if (xhr.status >= 200 && xhr.status < 300) {
            onProgress?.(1);
            resolve(payload);
            return;
          }
          reject(new AdminApiError(resolveErrorMessage(payload, xhr.status), xhr.status, payload));
        };
        xhr.onerror = () => reject(new AdminApiError(resolveNetworkMessage(this.baseUrl, new Error("Failed to fetch")), 0));
        xhr.onabort = () => reject(createAbortError());
        try {
          xhr.send(bytes);
        } catch (error) {
          reject(error);
        }
      });
    })();
    return {
      promise,
      cancel() {
        canceled = true;
        xhr?.abort();
      },
    };
  }

  fileManagerMkdir(path) {
    return this.#request("/admin/api/file-manager/mkdir", {
      method: "POST",
      body: { path },
    });
  }

  fileManagerRename(path, newPath) {
    return this.#request("/admin/api/file-manager/rename", {
      method: "POST",
      body: { path, newPath },
    });
  }

  fileManagerCopy(path, newPath) {
    return this.#request("/admin/api/file-manager/copy", {
      method: "POST",
      body: { path, newPath },
    });
  }

  fileManagerArchiveCreate(paths, outputPath) {
    return this.#request("/admin/api/file-manager/archive/create", {
      method: "POST",
      body: { paths, outputPath },
    });
  }

  fileManagerArchiveExtract(path) {
    return this.#request("/admin/api/file-manager/archive/extract", {
      method: "POST",
      body: { path },
    });
  }

  fileManagerDelete(path, recursive = false) {
    const params = new URLSearchParams({ path });
    if (recursive) params.set("recursive", "1");
    return this.#request(`/admin/api/file-manager/delete?${params}`, {
      method: "DELETE",
    });
  }

  fileManagerTrashMove(path) {
    return this.#request("/admin/api/file-manager/trash/move", {
      method: "POST",
      body: { path },
    });
  }

  fileManagerTrashList() {
    return this.#request("/admin/api/file-manager/trash/list");
  }

  fileManagerTrashRestore(id) {
    return this.#request("/admin/api/file-manager/trash/restore", {
      method: "POST",
      body: { id },
    });
  }

  fileManagerTrashDelete(id) {
    return this.#request(`/admin/api/file-manager/trash/item?id=${encodeURIComponent(id)}`, {
      method: "DELETE",
    });
  }

  fileManagerTrashEmpty() {
    return this.#request("/admin/api/file-manager/trash/empty", {
      method: "DELETE",
    });
  }

  monitorSnapshot() {
    return this.#request("/admin/api/monitor/snapshot");
  }

  monitorLogs(params = {}) {
    const q = new URLSearchParams();
    if (params.since != null) q.set("since", String(params.since));
    if (params.level) q.set("level", params.level);
    if (params.category) q.set("category", params.category);
    if (params.limit != null) q.set("limit", String(params.limit));
    const qs = q.toString();
    return this.#request(`/admin/api/monitor/logs${qs ? `?${qs}` : ""}`);
  }

  monitorModules() {
    return this.#request("/admin/api/monitor/modules");
  }

  monitorClearLogs() {
    return this.#request("/admin/api/monitor/logs/clear", {
      method: "POST",
    });
  }

  downloads() {
    return this.#request("/admin/api/downloads/");
  }

  downloadHealth() {
    return this.#request("/admin/api/downloads/health");
  }

  addDownloads(payload) {
    return this.#request("/admin/api/downloads/", {
      method: "POST",
      body: payload,
    });
  }

  addTorrent(bytes, startPaused = false, downloadDir, setDefaultDownloadDir = false) {
    const query = new URLSearchParams();
    if (startPaused) query.set("paused", "1");
    if (downloadDir) query.set("downloadDir", downloadDir);
    if (setDefaultDownloadDir) query.set("setDefaultDownloadDir", "1");
    const suffix = query.toString();
    return this.#request(`/admin/api/downloads/torrent${suffix ? `?${suffix}` : ""}`, {
      method: "POST",
      headers: { "Content-Type": "application/x-bittorrent" },
      body: bytes,
    });
  }

  pauseDownloadTask(id) {
    return this.#postSuccess(`/admin/api/downloads/${encodeURIComponent(id)}/pause`);
  }

  resumeDownloadTask(id) {
    return this.#postSuccess(`/admin/api/downloads/${encodeURIComponent(id)}/resume`);
  }

  removeDownloadTask(id, deleteFiles = false) {
    const suffix = deleteFiles ? "?deleteFiles=1" : "";
    return this.#request(`/admin/api/downloads/${encodeURIComponent(id)}${suffix}`, {
      method: "DELETE",
    });
  }

  opentoolConnected() {
    return this.#request("/admin/api/opentool/connected");
  }

  opentoolVersion() {
    return this.#request("/admin/api/opentool/version");
  }

  opentoolRestart() {
    return this.#request("/admin/api/opentool/restart", {
      method: "POST",
    });
  }

  opentoolServers() {
    return this.#request("/admin/api/opentool/servers");
  }

  opentoolHubTools(query = {}) {
    const params = new URLSearchParams();
    if (query.q) params.set("q", query.q);
    if (query.category) params.set("category", query.category);
    if (query.page) params.set("page", String(query.page));
    if (query.pageSize) params.set("pageSize", String(query.pageSize));
    const suffix = params.size ? `?${params}` : "";
    return this.#request(`/admin/api/opentool/hub/tools${suffix}`);
  }

  opentoolHubCategories(locale) {
    const params = new URLSearchParams({ locale });
    return this.#request(`/admin/api/opentool/hub/categories?${params}`);
  }

  opentoolHubTool(namespace, name) {
    return this.#request(
      `/admin/api/opentool/hub/tools/${encodeURIComponent(namespace)}/${encodeURIComponent(name)}`,
    );
  }

  async opentoolHubPull(payload, signal) {
    const token = await this.#resolveAdminToken();
    let response;
    try {
      response = await this.fetchImpl(new URL("/admin/api/opentool/hub/pull", this.baseUrl), {
        method: "POST",
        headers: {
          Accept: "text/event-stream",
          "Content-Type": "application/json",
          ...(token ? { "x-admin-token": token } : {}),
        },
        body: JSON.stringify(payload),
        cache: "no-store",
        credentials: token ? "omit" : "include",
        signal,
      });
    } catch (error) {
      throw new AdminApiError(resolveNetworkMessage(this.baseUrl, error), 0, error);
    }

    if (!response.ok || !response.body) {
      const errorPayload = await parseJson(response);
      throw new AdminApiError(
        resolveErrorMessage(errorPayload, response.status),
        response.status,
        errorPayload,
      );
    }

    return response.body;
  }

  opentoolTools(all = true) {
    return this.#request(`/admin/api/opentool/tools?all=${all ? "1" : "0"}`);
  }

  async opentoolToolFunctions(id) {
    const payload = await this.#request(`/admin/api/opentool/tools/${encodeURIComponent(id)}/functions`);
    return payload.functions ?? [];
  }

  opentoolStartTool(id, timeout = 180) {
    return this.#request(`/admin/api/opentool/tools/${encodeURIComponent(id)}/start?timeout=${timeout}`, {
      method: "POST",
    });
  }

  opentoolStopTool(id) {
    return this.#postSuccess(`/admin/api/opentool/tools/${encodeURIComponent(id)}/stop`);
  }

  opentoolDeleteTool(id) {
    return this.#request(`/admin/api/opentool/tools/${encodeURIComponent(id)}`, {
      method: "DELETE",
    });
  }

  opentoolDeleteServer(id) {
    return this.#request(`/admin/api/opentool/servers/${encodeURIComponent(id)}`, {
      method: "DELETE",
    });
  }

  opentoolRunServer(id, options = {}) {
    const host = options.host ?? "127.0.0.1";
    const timeout = options.timeout ?? 180;
    return this.#request(
      `/admin/api/opentool/servers/${encodeURIComponent(id)}/run?host=${encodeURIComponent(host)}&timeout=${timeout}`,
      { method: "POST" },
    );
  }

  opentoolUploadServer(bytes, filename) {
    const body =
      bytes instanceof Blob
        ? bytes
        : bytes instanceof ArrayBuffer
          ? new Uint8Array(bytes)
          : bytes;

    return this.#request(`/admin/api/opentool/servers/upload?filename=${encodeURIComponent(filename)}`, {
      method: "POST",
      body,
      headers: {
        "Content-Type": "application/octet-stream",
      },
    });
  }

  opentoolImportServer(path) {
    return this.#request("/admin/api/opentool/servers/import", {
      method: "POST",
      body: { path },
    });
  }

  opentoolSettings() {
    return this.#request("/admin/api/opentool/settings");
  }

  opentoolUpdateSettings(payload) {
    return this.#request("/admin/api/opentool/settings", {
      method: "PUT",
      body: payload,
    });
  }

  #postSuccess(path) {
    return this.#request(path, {
      method: "POST",
    });
  }

  async #request(path, init = {}) {
    const token = await this.#resolveAdminToken();
    const isBinaryBody =
      init.body instanceof Blob ||
      init.body instanceof ArrayBuffer ||
      init.body instanceof Uint8Array;
    let response;
    try {
      response = await this.fetchImpl(new URL(path, this.baseUrl), {
        method: init.method ?? "GET",
        headers: {
          Accept: "application/json",
          ...(init.body && !isBinaryBody ? { "Content-Type": "application/json" } : {}),
          ...(token ? { "x-admin-token": token } : {}),
          ...init.headers,
        },
        body: init.body ? (isBinaryBody ? init.body : JSON.stringify(init.body)) : undefined,
        cache: "no-store",
        credentials: token ? "omit" : "include",
      });
    } catch (error) {
      throw new AdminApiError(resolveNetworkMessage(this.baseUrl, error), 0, error);
    }

    const payload = await parseJson(response);
    if (!response.ok) {
      throw new AdminApiError(resolveErrorMessage(payload, response.status), response.status, payload);
    }

    return payload;
  }

  async #resolveAdminToken() {
    const requestOrigin = new URL(this.baseUrl, window.location.origin).origin;
    if (requestOrigin === window.location.origin) {
      return undefined;
    }

    this.adminTokenPromise ??= this.fetchImpl(resolveAdminAuthSessionUrl(this.baseUrl), {
      headers: {
        Accept: "application/json",
      },
      credentials: "include",
      cache: "no-store",
    })
      .then((response) => response.json().catch(() => ({})))
      .then((payload) => (typeof payload.adminToken === "string" && payload.adminToken ? payload.adminToken : undefined))
      .catch(() => undefined);

    return this.adminTokenPromise;
  }
}

export function createAdminApi(options) {
  return new AdminApi(options);
}

function defaultBaseUrl() {
  const params = new URLSearchParams(window.location.search);
  const apiBase = params.get("apiBase");
  if (apiBase && apiBase.trim()) {
    return apiBase.trim();
  }

  return `${window.location.origin}/admin`;
}

function resolveAdminAuthSessionUrl(baseUrl) {
  try {
    const resolved = new URL(baseUrl, window.location.origin);
    // The Flutter/CEF Shell exposes the auth namespace through the same
    // agentos-app:// scheme handler as the Admin API. Fetching a root-relative
    // URL here would incorrectly target the HTTP(S) App origin and leave the
    // native proxy without the session context it is meant to inject.
    if (resolved.protocol === "agentos-app:") {
      return new URL("/api/auth/session", resolved).toString();
    }
  } catch {
    // The normal request will surface malformed base URLs to the caller.
  }
  return "/api/auth/session";
}

async function parseJson(response) {
  const text = await response.text();
  if (!text) return {};

  try {
    return JSON.parse(text);
  } catch {
    return { message: text };
  }
}

function parseJsonText(text) {
  if (!text) return {};
  try {
    return JSON.parse(text);
  } catch {
    return { message: text };
  }
}

function createAbortError() {
  const error = new Error("Upload canceled");
  error.name = "AbortError";
  return error;
}

function resolveErrorMessage(payload, status) {
  if (payload && typeof payload === "object") {
    if (typeof payload.message === "string" && payload.message.startsWith("<!DOCTYPE")) {
      return `Request failed with status ${status}`;
    }
    if (typeof payload.error === "string" && payload.error) return payload.error;
    if (typeof payload.message === "string" && payload.message) return payload.message;
    if (
      payload.error &&
      typeof payload.error === "object" &&
      typeof payload.error.message === "string"
    ) {
      return payload.error.message;
    }
  }

  return `Request failed with status ${status}`;
}

function resolveNetworkMessage(baseUrl, error) {
  if (error instanceof Error && error.message && error.message !== "Failed to fetch") {
    return error.message;
  }

  return `无法连接到 AgentOS Admin 服务: ${baseUrl}`;
}
