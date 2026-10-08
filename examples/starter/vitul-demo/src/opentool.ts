import { Channel, invoke, isTauri } from "@tauri-apps/api/core";

type JsonRpcResponse<T> = {
  jsonrpc: "2.0.0";
  id: string;
  result?: T;
  error?: { code: number; message: string };
};

type OpenToolIpcEvent = { event: string; data: unknown };
export type OpenToolStreamEvent<T> =
  | { event: "START"; data: unknown }
  | { event: "DATA"; data: T }
  | { event: "DONE"; data: unknown };
type Transport = "http" | "ipc";

export class OpenToolError extends Error {
  constructor(
    readonly code: number,
    message: string,
  ) {
    super(message);
    this.name = "OpenToolError";
  }
}

let endpoint: Promise<string> | undefined;
let transport: Promise<Transport> | undefined;
let fallbackRequestSequence = 0;

async function getTransport(): Promise<Transport> {
  transport ??= (async () => {
    if (window.agentos?.host || !(await isTauri())) return "http";
    return invoke<Transport>("opentool_transport");
  })();
  return transport;
}

async function getEndpoint(): Promise<string> {
  endpoint ??= (async () => {
    if (!window.agentos?.host && (await isTauri())) {
      return invoke<string>("opentool_endpoint");
    }
    const webShellAppRoute = /^\/apps\/([^/]+)(?:\/|$)/.exec(window.location.pathname);
    const basePath = webShellAppRoute ? `/apps/${webShellAppRoute[1]}/` : "/";
    return new URL(basePath, window.location.href).toString();
  })();
  return endpoint;
}

function createRequestId(): string {
  const cryptoApi = globalThis.crypto;
  if (typeof cryptoApi?.randomUUID === "function") {
    return cryptoApi.randomUUID();
  }
  if (typeof cryptoApi?.getRandomValues === "function") {
    const bytes = cryptoApi.getRandomValues(new Uint8Array(16));
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0"));
    return [
      hex.slice(0, 4).join(""),
      hex.slice(4, 6).join(""),
      hex.slice(6, 8).join(""),
      hex.slice(8, 10).join(""),
      hex.slice(10).join(""),
    ].join("-");
  }
  fallbackRequestSequence += 1;
  return `request-${Date.now().toString(36)}-${fallbackRequestSequence.toString(36)}`;
}

function createRequest(method: string, params: Record<string, unknown>) {
  return {
    jsonrpc: "2.0.0" as const,
    method,
    params,
    id: createRequestId(),
  };
}

async function requestHttp(operation: "version" | "load"): Promise<unknown>;
async function requestHttp(operation: "call", request: unknown): Promise<unknown>;
async function requestHttp(
  operation: "version" | "load" | "call",
  request?: unknown,
): Promise<unknown> {
  const base = await getEndpoint();
  const url = new URL(`opentool/${operation}`, base);
  const response = await fetch(url, {
    method: operation === "version" || operation === "load" ? "GET" : "POST",
    headers: { "Content-Type": "application/json" },
    body: request === undefined ? undefined : JSON.stringify(request),
  });
  if (!response.ok) {
    throw new Error(`OpenTool request failed: HTTP ${response.status}`);
  }
  return response.json();
}

function readJsonRpcResponse<T>(value: unknown, id: string): JsonRpcResponse<T> {
  if (typeof value !== "object" || value === null) {
    throw new Error("OpenTool response must be a JSON object.");
  }
  const response = value as JsonRpcResponse<T>;
  if (response.jsonrpc !== "2.0.0") {
    throw new Error("OpenTool response has an unsupported JSON-RPC version.");
  }
  if (response.id !== id) {
    throw new Error("OpenTool response ID does not match the request.");
  }
  if (response.error) {
    throw new OpenToolError(response.error.code, response.error.message);
  }
  if (!("result" in response)) {
    throw new Error("OpenTool response is missing its result.");
  }
  return response;
}

export async function getOpenToolVersion<T = unknown>(): Promise<T> {
  if ((await getTransport()) === "ipc") {
    return invoke<T>("opentool_ipc", { operation: "version" });
  }
  return (await requestHttp("version")) as T;
}

export async function loadOpenTool<T = unknown>(): Promise<T> {
  if ((await getTransport()) === "ipc") {
    return invoke<T>("opentool_ipc", { operation: "load" });
  }
  return (await requestHttp("load")) as T;
}

export async function callOpenTool<T>(
  method: string,
  params: Record<string, unknown>,
): Promise<T> {
  const request = createRequest(method, params);
  const response =
    (await getTransport()) === "ipc"
      ? await invoke<unknown>("opentool_ipc", { operation: "call", request })
      : await requestHttp("call", request);
  return readJsonRpcResponse<T>(response, request.id).result as T;
}

export async function streamCallOpenTool<T>(
  method: string,
  params: Record<string, unknown>,
  onEvent: (event: OpenToolStreamEvent<T>) => void,
): Promise<void> {
  const request = createRequest(method, params);
  let sawDone = false;
  let streamError: unknown;
  const handleEvent = (frame: OpenToolIpcEvent) => {
    // Existing OpenTool services may emit lowercase SSE event names while
    // opentool-rust emits uppercase names. Normalize the wire event once so
    // both transports deliver the same public event shape.
    const event = frame.event.toUpperCase();
    switch (event) {
      case "START":
        onEvent({ event, data: frame.data });
        return;
      case "DONE":
        sawDone = true;
        onEvent({ event, data: frame.data });
        return;
      case "DATA": {
        const response = readJsonRpcResponse<T>(frame.data, request.id);
        onEvent({ event: "DATA", data: response.result as T });
        return;
      }
      case "ERROR": {
        const response = readJsonRpcResponse<never>(frame.data, request.id);
        throw new Error(`Unexpected successful OpenTool ERROR event: ${JSON.stringify(response)}`);
      }
      default:
        throw new Error(`Unsupported OpenTool stream event: ${frame.event}`);
    }
  };
  const receiveEvent = (frame: OpenToolIpcEvent) => {
    if (streamError !== undefined) return;
    try {
      handleEvent(frame);
    } catch (error) {
      streamError = error;
    }
  };

  if ((await getTransport()) === "ipc") {
    let resolveDone!: () => void;
    const channelDone = new Promise<void>((resolve) => {
      resolveDone = resolve;
    });
    const channel = new Channel<OpenToolIpcEvent>((frame) => {
      if (frame.event === "DONE") resolveDone();
      receiveEvent(frame);
    });
    await invoke("opentool_ipc", {
      operation: "streamCall",
      request,
      onEvent: channel,
    });
    await channelDone;
    if (streamError !== undefined) throw streamError;
    return;
  }

  const base = await getEndpoint();
  const response = await fetch(new URL("opentool/streamCall", base), {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(request),
  });
  if (!response.ok) {
    throw new Error(`OpenTool stream request failed: HTTP ${response.status}`);
  }
  for await (const frame of readSse(response)) receiveEvent(frame);
  if (streamError !== undefined) throw streamError;
  if (!sawDone) onEvent({ event: "DONE", data: { DONE: method } });
}

async function* readSse(response: Response): AsyncGenerator<OpenToolIpcEvent> {
  if (!response.body) throw new Error("OpenTool stream response has no body.");
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let pending = "";

  const parseFrame = (frame: string): OpenToolIpcEvent | undefined => {
    let event = "message";
    const data: string[] = [];
    for (const line of frame.split(/\r?\n/)) {
      if (line.startsWith(":")) continue;
      if (line.startsWith("event:")) event = line.slice(6).trim();
      else if (line.startsWith("data:")) data.push(line.slice(5).replace(/^ /, ""));
    }
    if (data.length === 0) return undefined;
    return { event, data: JSON.parse(data.join("\n")) as unknown };
  };

  while (true) {
    const { done, value } = await reader.read();
    pending += decoder.decode(value, { stream: !done });
    pending = pending.replace(/\r\n/g, "\n");
    let boundary = pending.indexOf("\n\n");
    while (boundary >= 0) {
      const frame = pending.slice(0, boundary);
      pending = pending.slice(boundary + 2);
      const parsed = parseFrame(frame);
      if (parsed) yield parsed;
      boundary = pending.indexOf("\n\n");
    }
    if (done) break;
  }
  if (pending.trim()) {
    const parsed = parseFrame(pending);
    if (parsed) yield parsed;
  }
}
