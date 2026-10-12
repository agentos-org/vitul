export interface Provider {
  available?: boolean;
  running?: boolean;
  installed?: boolean;
  initialized?: boolean;
  state?: string;
  message?: string;
  canInstall?: boolean;
  canInitialize?: boolean;
  canStart?: boolean;
  canStop?: boolean;
  runtimeBundle?: { included?: boolean; waydroidVersion: string; androidVersion: string; packageCount: number };
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
export interface InstallTask { id: string; fileName: string; status: string; message: string }
export interface Snapshot { provider?: Provider; apps: AndroidApp[]; appError: string; tasks: InstallTask[] }
export interface ActionResult { message?: string; task?: InstallTask }
export const messageOf = (error: unknown): string => error instanceof Error ? error.message : String(error);

export async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
  const response = await fetch(`${import.meta.env.BASE_URL}api${path}`, options);
  const body: unknown = await response.json().catch(() => null);
  if (!response.ok) {
    const error = body && typeof body === 'object' && 'error' in body ? body.error : undefined;
    const detail = typeof error === 'string' ? error
      : error && typeof error === 'object' && 'message' in error && typeof error.message === 'string' ? error.message : undefined;
    throw new Error(detail || `请求失败（${response.status}）`);
  }
  if (body === null) throw new Error('服务返回了无效的 JSON');
  return body as T;
}
export async function readSnapshot(signal: AbortSignal): Promise<Snapshot> {
  const [status, apps, tasks] = await Promise.all([
    request<{ provider?: Provider }>('/status', { signal }),
    request<{ available?: boolean; apps?: AndroidApp[]; error?: string }>('/apps', { signal }),
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
