import { useCallback, useEffect, useRef, useState } from 'react';
import { nextPreparation, type PreparationAction } from './preparation';
import { useI18n } from './i18n';
import { act, messageOf, readSnapshot, request, type Action, type ActionResult, type Snapshot } from './api';

export function useStore() {
  const { t } = useI18n();
  const [snapshot, setSnapshot] = useState<Snapshot>({ apps: [], appError: '', tasks: [] });
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<{ message: string; kind: 'info' | 'error' | 'success'; params?: Record<string, string | number> }>();
  const controller = useRef<AbortController | null>(null);
  const actionLock = useRef(false);
  const attemptedPreparation = useRef(new Set<PreparationAction>());
  const [preparing, setPreparing] = useState<PreparationAction | null>(null);
  const [preparationError, setPreparationError] = useState('');
  const notify = useCallback((message: string, kind: 'info' | 'error' | 'success' = 'info', params?: Record<string, string | number>) => setNotice({ message, kind, params }), []);
  const refresh = useCallback(async () => {
    controller.current?.abort();
    const current = new AbortController();
    controller.current = current;
    setLoading(true);
    try {
      const next = await readSnapshot(current.signal);
      if (!current.signal.aborted) { setSnapshot(next); setError(''); }
    } catch (failure) {
      if (!current.signal.aborted) { setError(messageOf(failure)); notify(messageOf(failure), 'error'); }
    } finally { if (!current.signal.aborted) setLoading(false); }
  }, [notify]);
  useEffect(() => { void refresh(); return () => controller.current?.abort(); }, [refresh]);
  useEffect(() => {
    if (loading || busy) return;
    const installing = snapshot.tasks.some(task => ['queued', 'installing'].includes(task.status));
    const waiting = snapshot.provider && !snapshot.provider.running && snapshot.provider.state !== 'running';
    if (!installing && !waiting) return;
    const timer = setTimeout(() => void refresh(), installing ? 5000 : 15000);
    return () => clearTimeout(timer);
  }, [snapshot, loading, busy, refresh]);
  useEffect(() => { if (!notice) return; const timer = setTimeout(() => setNotice(undefined), 4600); return () => clearTimeout(timer); }, [notice]);

  async function run(operation: () => Promise<void>) {
    if (actionLock.current) return;
    actionLock.current = true; setBusy(true);
    try { await operation(); } catch (failure) { notify(messageOf(failure), 'error'); }
    finally { await refresh(); actionLock.current = false; setBusy(false); }
  }
  useEffect(() => {
    const action = nextPreparation(snapshot.provider);
    if (loading || busy || actionLock.current || preparationError || !action || attemptedPreparation.current.has(action)) return;
    attemptedPreparation.current.add(action);
    setPreparing(action);
    void run(async () => {
      try { await request<ActionResult>(`/runtime/${action}`, { method: 'POST' }); }
      catch (failure) { setPreparationError(messageOf(failure)); throw failure; }
      finally { setPreparing(null); }
    });
  }, [snapshot.provider, loading, busy, preparationError]);
  async function retryPreparation() {
    if (busy) return;
    attemptedPreparation.current.clear(); setPreparationError(''); await refresh();
  }
  async function importFiles(files: File[]) {
    await run(async () => {
      for (const file of files) {
        if (!file.name.toLowerCase().endsWith('.apk')) { notify("notApk", 'error', { file: file.name }); continue; }
        const form = new FormData(); form.append('file', file, file.name);
        try {
          const result = await request<ActionResult>('/install-tasks', { method: 'POST', body: form });
          const status = result.task?.status;
          notify(status === 'waiting-provider' ? "APK 已保存为任务；运行环境可用后可以重试。"
            : status === 'installed' ? "APK 安装完成"
              : status === 'failed' ? "APK 安装失败，可重试" : "APK 已提交安装",
            status === 'failed' ? 'error' : status === 'waiting-provider' ? 'info' : 'success');
        } catch (failure) { notify("importFailed", 'error', { message: messageOf(failure) }); }
      }
    });
  }
  async function perform(action: Action, id: string) {
    if (action === 'uninstall' && !window.confirm(t("确定卸载这个 Android 应用？"))) return;
    await run(async () => { await act(action, id); notify("操作已完成", 'success'); });
  }
  async function runtime(action: 'start' | 'stop' | 'install' | 'initialize') {
    await run(async () => {
      await request<ActionResult>(`/runtime/${action}`, { method: 'POST' });
      notify(({ start: "已请求启动环境", stop: "已请求停止环境", install: "Waydroid 安装完成", initialize: "Android 系统镜像初始化完成" }[action]), 'success');
    });
  }
  return { ...snapshot, error, loading, busy, notice, preparing, preparationError, retryPreparation, refresh, importFiles, perform, runtime };
}
