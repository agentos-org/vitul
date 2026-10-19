export interface Provider {
  available?: boolean;
  running?: boolean;
  installed?: boolean;
  initialized?: boolean;
  state?: string;
  message?: string;
  providerError?: ProviderErrorDetails;
  deploymentTask?: {
    taskId?: string;
    state?: string;
    progress?: number;
    operationVersion?: number;
    retryOfTaskId?: string;
    reauthorizationRequestId?: string;
    trackingDiagnostic?: unknown;
    error?: { code?: string; message?: string; details?: unknown; data?: unknown; diagnostics?: unknown };
  };
  deploymentTrackingDiagnostic?: unknown;
  deploymentLineage?: {
    retryOfTaskId?: string;
    reauthorizationRequestId?: string;
    reauthorizationState?: string;
    newTaskId?: string;
    agentOsResponse?: unknown;
  };
  reauthorization?: {
    requestId?: string;
    deniedTaskId?: string;
    newTaskId?: string;
    state?: 'awaitingShellConfirmation' | 'validating' | 'confirmed' | 'denied' | 'expired' | 'revalidationFailed' | 'requestUnconfirmed' | string;
    message?: string;
    error?: { code?: string; message?: string; details?: unknown };
    canReauthorize?: boolean;
    canRetryRequest?: boolean;
  };
  canInstall?: boolean;
  canInitialize?: boolean;
  canStart?: boolean;
  canRecover?: boolean;
  canReapply?: boolean;
  deploymentKind?: 'setup' | 'recovery';
  canStop?: boolean;
  runtimeBundle?: { included?: boolean; waydroidVersion: string; androidVersion: string; packageCount: number };
  deploymentActions?: Record<string, {
    implemented?: boolean;
    available?: boolean;
    requiredForAndroidToneSetup?: boolean;
    reason?: string;
    state?: Record<string, unknown>;
  }>;
}
export interface ProviderErrorDetails {
  code?: string;
  message?: string;
  providerId?: string;
  details?: unknown;
  data?: unknown;
  diagnostics?: unknown;
}
export interface AndroidApp {
  id?: string;
  packageName?: string;
  name?: string;
  label?: string;
  version?: string;
  iconUrl?: string;
  running?: boolean;
  runningStateAvailable?: boolean;
}
export interface InstallTask { id: string; fileName: string; status: string; message: string; diagnostic?: string }
export interface Snapshot { provider?: Provider; apps: AndroidApp[]; appError: string | ProviderErrorDetails; tasks: InstallTask[] }
export interface ActionResult { message?: string; task?: InstallTask }
export const messageOf = (error: unknown): string => error instanceof Error ? error.message : String(error);

export class ApiRequestError extends Error {
  readonly code?: string;
  readonly providerId?: string;
  readonly diagnosticMessage: string;
  readonly status: number;

  constructor(details: ProviderErrorDetails, status: number) {
    super(details.message || `请求失败（${status}）`);
    this.name = 'ApiRequestError';
    this.code = details.code;
    this.providerId = details.providerId;
    this.diagnosticMessage = details.message || `HTTP ${status}`;
    this.details = details.details;
    this.data = details.data;
    this.diagnostics = details.diagnostics;
    this.status = status;
  }

  readonly details?: unknown;
  readonly data?: unknown;
  readonly diagnostics?: unknown;
}

export interface PreparationFailure {
  action: 'install' | 'initialize' | 'start' | 'recover';
  summary: string;
  code?: string;
  providerId?: string;
  message: string;
  details?: unknown;
  data?: unknown;
  diagnostics?: unknown;
}

export function preparationFailureOf(error: unknown, action: PreparationFailure['action']): PreparationFailure {
  const apiError = error instanceof ApiRequestError ? error : undefined;
  const externalWayland = apiError?.providerId === 'org.agentos.external-wayland'
    || apiError?.code?.toLowerCase().includes('wayland') === true;
  const idempotencyConflict = apiError?.code?.toLowerCase().includes('idempotency') === true
    && apiError.code.toLowerCase().includes('conflict');
  const summary = externalWayland && action === 'start' ? 'Android 显示会话启动失败'
    : idempotencyConflict ? '部署请求幂等键冲突'
    : action === 'recover' ? 'Android 运行环境恢复失败'
      : action === 'start' ? 'Android 运行环境启动失败'
        : 'Android 环境准备失败';
  return {
    action,
    summary,
    code: apiError?.code,
    providerId: apiError?.providerId,
    message: idempotencyConflict
      ? 'AgentOS 检测到部署请求标识冲突。请刷新 Android 运行环境状态，再点击“重新检查运行环境”提交新请求。'
      : apiError?.diagnosticMessage || messageOf(error),
    details: apiError?.details,
    data: apiError?.data,
    diagnostics: apiError?.diagnostics,
  };
}

export function userFacingFailure(error: unknown): string {
  if (error instanceof ApiRequestError) {
    if (error.code?.toLowerCase().includes('idempotency') && error.code.toLowerCase().includes('conflict')) {
      return 'AgentOS 检测到部署请求标识冲突。请刷新 Android 运行环境状态，再点击“重新检查运行环境”提交新请求。';
    }
    if (error.providerId === 'org.agentos.external-wayland' || error.code?.toLowerCase().includes('wayland')) {
      return 'Android 显示会话启动失败';
    }
    const chineseMessage = /[\u3400-\u9fff]/.test(error.diagnosticMessage);
    const debugOrJson = /Custom\s*\{\s*kind\s*:\s*Other|^\s*[\[{]|"(?:code|message|error)"\s*:/i.test(error.diagnosticMessage);
    if (chineseMessage && !debugOrJson) return error.diagnosticMessage;
    return 'Android 运行环境操作失败';
  }
  return messageOf(error);
}

export async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
  const response = await fetch(`${import.meta.env.BASE_URL}api${path}`, options);
  const body: unknown = await response.json().catch(() => null);
  if (!response.ok) {
    const error = body && typeof body === 'object' && 'error' in body ? body.error : undefined;
    if (error && typeof error === 'object') {
      const value = error as Record<string, unknown>;
      throw new ApiRequestError({
        code: typeof value.code === 'string' ? value.code : undefined,
        message: typeof value.message === 'string' ? value.message : undefined,
        providerId: typeof value.providerId === 'string' ? value.providerId : undefined,
        details: value.details,
        data: value.data,
        diagnostics: value.diagnostics,
      }, response.status);
    }
    throw new ApiRequestError({ message: typeof error === 'string' ? error : undefined }, response.status);
  }
  if (body === null) throw new Error('服务返回了无效的 JSON');
  return body as T;
}
export async function readSnapshot(signal: AbortSignal): Promise<Snapshot> {
  const [status, apps, tasks] = await Promise.all([
    request<{ provider?: Provider }>('/status', { signal }),
    request<{ available?: boolean; apps?: AndroidApp[]; error?: string | ProviderErrorDetails }>('/apps', { signal }),
    request<{ installTasks?: InstallTask[] }>('/install-tasks', { signal }),
  ]);
  return { provider: status.provider, apps: apps.available === false ? [] : apps.apps || [],
    appError: apps.available === false ? apps.error || '' : '', tasks: tasks.installTasks || [] };
}
export type Action = 'install' | 'delete-task' | 'launch' | 'stop' | 'uninstall';
export function act(action: Action, id: string): Promise<ActionResult> {
  const encoded = encodeURIComponent(id);
  const paths: Record<Action, string> = {
    install: `/install-tasks/${encoded}/install`, 'delete-task': `/install-tasks/${encoded}`,
    launch: `/apps/${encoded}/launch`, stop: `/apps/${encoded}/stop`, uninstall: `/apps/${encoded}`,
  };
  return request(paths[action], { method: action === 'uninstall' || action === 'delete-task' ? 'DELETE' : 'POST' });
}
