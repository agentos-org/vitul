export type FennFilePickerMode = "open" | "save" | "directory";

export type FennFilePickerFile = {
  name: string;
  path?: string;
  kind: string;
  size?: number;
  mimeType?: string;
  file?: File;
};

export type FennFilePickerResult = {
  type: "fenn:file-picker-result";
  source: "fenn-files";
  requestId: string;
  mode: FennFilePickerMode;
  ok: boolean;
  canceled: boolean;
  paths: string[];
  files: FennFilePickerFile[];
  savePath?: string;
  directoryPath?: string;
};

export type BrowserFilePickerResult = {
  type: "fenn:file-picker-result";
  source: "browser";
  requestId: string;
  mode: FennFilePickerMode;
  ok: boolean;
  canceled: boolean;
  paths: string[];
  files: FennFilePickerFile[];
  directoryName?: string;
  directoryHandle?: unknown;
  fileHandle?: unknown;
  suggestedName?: string;
};

export type FilePickerResult = FennFilePickerResult | BrowserFilePickerResult;

export type FennFileDragItem = {
  name: string;
  path: string;
  kind: string;
  size?: number;
  mimeType?: string;
};

export type FennFileDragPayload = {
  type: "fenn:file-drag";
  source: "fenn-files";
  version: 1;
  files: FennFileDragItem[];
};

export type FennFilePickerOptions = {
  mode?: FennFilePickerMode;
  requestId?: string;
  initialPath?: string;
  accept?: string | string[];
  multiple?: boolean;
  suggestedName?: string;
  returnOrigin?: string;
  pickerUrl?: string;
  apiBase?: string;
  presentation?: "iframe" | "popup";
  delegateToHost?: boolean;
  container?: HTMLElement;
  windowFeatures?: string;
  timeoutMs?: number;
  types?: Array<Record<string, unknown>>;
};

export type SaveFileOptions = {
  suggestedName?: string;
  accept?: string | string[];
  initialPath?: string;
  apiBase?: string;
  api?: {
    fileManagerWrite(
      path: string,
      bytes: Blob | ArrayBuffer | Uint8Array | string,
    ): Promise<unknown>;
  };
  presentation?: "iframe" | "popup";
  types?: Array<Record<string, unknown>>;
};

export type FennFilePickerRequest = {
  type: "fenn:file-picker-request";
  source: "fenn-files-sdk";
  requestId: string;
  mode: FennFilePickerMode;
  accept: string[];
  multiple: boolean;
  initialPath?: string;
  suggestedName?: string;
  returnOrigin?: string;
};

export type AgentOSTitleBarMode = "system" | "overlay";
export type AgentOSWindowTitleBarContext = {
  requestedMode: AgentOSTitleBarMode;
  actualMode: AgentOSTitleBarMode | null;
  supportStatus: "supported" | "unsupported";
  supportedModes: AgentOSTitleBarMode[];
  windowKind: "native-top-level" | "shell-internal" | "external";
  controlsProvider: "operating-system" | "agentos" | "none";
};
export type AgentOSWindowState =
  | "normal"
  | "maximized"
  | "minimized"
  | "dragging"
  | "resizing";
export type AgentOSPluginLaunchContext = Record<string, unknown> & {
  appId?: string;
  bundleId?: string;
  runtime?: "agentos" | "browser";
  windowTitleBar?: AgentOSWindowTitleBarContext;
  windowState?: AgentOSWindowState;
};
export function readPluginLaunchContext(search?: string): AgentOSPluginLaunchContext;
export function postHostMessage(
  message: Record<string, unknown>,
  targetWindow?: Window,
  targetOrigin?: string,
): void;
export type ConfirmActionOptions = {
  message: string;
  title?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  tone?: "default" | "danger";
  locale?: "zh" | "en";
};
export function confirmAction(options: string | ConfirmActionOptions): Promise<boolean>;
export type FennFileOpenRequest = {
  type: "fenn:file-open-request";
  source: "fenn-files";
  requestId: string;
  bundleId: string;
  /** @deprecated Use bundleId. */
  appId?: string;
  path: string;
  name: string;
  kind: string;
  mimeType?: string;
};
export const FENN_FILE_OPEN_REQUEST_KEY: "fenn:file-open-request";
export function requestFennFileOpen(
  bundleId: string,
  file: { path: string; name?: string; kind?: string; mimeType?: string },
): FennFileOpenRequest;
export function readFennFileOpenRequest(bundleId?: string): FennFileOpenRequest | null;
export function clearFennFileOpenRequest(requestId?: string): void;
export const FENN_FILE_DRAG_MIME: "application/x-fenn-files+json";
export function isAgentosRuntime(): boolean;
export function createFennFileDragPayload(files?: FennFileDragItem[]): FennFileDragPayload;
export function setFennFileDragData(dataTransfer: DataTransfer, files?: FennFileDragItem[]): FennFileDragPayload;
export function readFennFileDrag(dataTransfer: DataTransfer | null): FennFileDragPayload | null;
export function filePickerFileToBlob(
  file: FennFilePickerFile | FennFileDragItem,
  options?: { api?: { fileManagerRead(path: string, maxBytes?: number): Promise<Blob> }; apiBase?: string; maxBytes?: number },
): Promise<Blob>;
export function filePickerFileToMediaUrl(
  file: FennFilePickerFile | FennFileDragItem,
  options?: { api?: { fileManagerMediaUrl(path: string): string }; apiBase?: string },
): Promise<string>;
export type FennUploadedFile = FennFilePickerFile & {
  path: string;
  kind: "file";
  file: File;
  size: number;
};
export function uploadFilesToFennFolder(
  files: File[] | FileList,
  folderName: string,
  options?: {
    api?: {
      fileManagerList(options?: { path?: string; showHidden?: boolean }): Promise<unknown>;
      fileManagerMkdir(path: string): Promise<unknown>;
      fileManagerUpload(
        directory: string,
        filename: string,
        bytes: Blob | ArrayBuffer | Uint8Array | string,
        onProgress?: (progress: number) => void,
      ): { promise: Promise<unknown>; cancel(): void };
    };
    apiBase?: string;
    onProgress?: (file: File, progress: number) => void;
  },
): Promise<FennUploadedFile[]>;
export function createFennFilePickerRequest(options?: FennFilePickerOptions): FennFilePickerRequest;
export function postFennFilePickerRequest(
  options?: FennFilePickerOptions,
  targetWindow?: Window,
  targetOrigin?: string,
): FennFilePickerRequest;
export function openFennFilePicker(options?: FennFilePickerOptions): Promise<FennFilePickerResult>;
export function openFilePicker(options?: FennFilePickerOptions): Promise<FilePickerResult>;
export function saveFile(
  bytes: Blob | ArrayBuffer | Uint8Array | string,
  options?: SaveFileOptions,
): Promise<FilePickerResult & { written?: boolean; downloaded?: boolean }>;
export class FennFilePickerError extends Error {
  code: string;
}
