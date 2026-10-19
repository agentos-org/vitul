import { useCallback, useEffect, useRef, useState } from 'react';
import { nextPreparation, type PreparationAction } from './preparation';
import { useI18n } from './i18n';
import { act, preparationFailureOf, readSnapshot, request, userFacingFailure, type Action, type ActionResult, type PreparationFailure, type Snapshot } from './api';

export function useStore() {
  const { t } = useI18n();
  const [snapshot, setSnapshot] = useState<Snapshot>({ apps: [], appError: '', tasks: [] });
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<{ message: string; kind: 'info' | 'error' | 'success'; params?: Record<string, string | number> }>();
  const controller = useRef<AbortController | null>(null);
  const actionLock = useRef(false);
  const actionPromise = useRef<Promise<void> | null>(null);
  const attemptedPreparation = useRef(new Set<PreparationAction>());
  const [preparing, setPreparing] = useState<PreparationAction | null>(null);
  const [preparationError, setPreparationError] = useState('');
  const [preparationFailure, setPreparationFailure] = useState<PreparationFailure>();
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
      if (!current.signal.aborted) { const message = userFacingFailure(failure); setError(message); notify(message, 'error'); }
    } finally { if (!current.signal.aborted) setLoading(false); }
  }, [notify]);
  useEffect(() => { void refresh(); return () => controller.current?.abort(); }, [refresh]);
  useEffect(() => {
    if (loading || busy) return;
    const installing = snapshot.tasks.some(task => ['queued', 'installing'].includes(task.status));
    const provider = snapshot.provider;
    const terminal = ['authorization-denied', 'authorization-cancelled', 'authorization-expired', 'deployment-failed', 'preflight-blocked', 'idempotency-conflict', 'recovery-failed', 'deployment-tracking-failed'].includes(provider?.state || '');
    const reauthorizationPending = ['awaitingShellConfirmation', 'validating'].includes(provider?.reauthorization?.state || '');
    const waiting = provider && !provider.running && provider.state !== 'running' && (!terminal || reauthorizationPending);
    if (!installing && !waiting) return;
    const deploymentActive = ['awaiting-authorization', 'deploying'].includes(provider?.state || '') || reauthorizationPending;
    const timer = setTimeout(() => void refresh(), installing ? 5000 : deploymentActive ? 2500 : 15000);
    return () => clearTimeout(timer);
  }, [snapshot, loading, busy, refresh]);
  useEffect(() => { if (!notice) return; const timer = setTimeout(() => setNotice(undefined), 4600); return () => clearTimeout(timer); }, [notice]);

  function run(operation: () => Promise<void>): Promise<void> {
    if (actionPromise.current) return actionPromise.current;
    actionLock.current = true; setBusy(true);
    let pending!: Promise<void>;
    pending = (async () => {
      try { await operation(); } catch (failure) { notify(userFacingFailure(failure), 'error'); }
      finally {
        await refresh();
        actionLock.current = false;
        if (actionPromise.current === pending) actionPromise.current = null;
        setBusy(false);
      }
    })();
    actionPromise.current = pending;
    return pending;
  }
  useEffect(() => {
    const action = nextPreparation(snapshot.provider);
    if (loading || busy || actionLock.current || preparationError || !action || attemptedPreparation.current.has(action)) return;
    attemptedPreparation.current.add(action);
    setPreparing(action);
    void run(async () => {
      await requestPreparation(action);
    });
  }, [snapshot.provider, loading, busy, preparationError]);
  async function requestPreparation(action: PreparationAction) {
    setPreparing(action);
    try {
      await request<ActionResult>(`/runtime/${action}`, { method: 'POST' });
    } catch (failure) {
      const detail = preparationFailureOf(failure, action);
      setPreparationFailure(detail);
      setPreparationError(detail.summary);
      throw new Error(detail.summary);
    } finally {
      setPreparing(null);
    }
  }
  async function retryPreparation() {
    if (busy) return;
    const state = snapshot.provider?.state;
    const failedAction = preparationFailure?.action;
    if (state === 'authorization-expired' && snapshot.provider?.canReapply === true) {
      const action = snapshot.provider.deploymentKind === 'recovery' ? 'recover' : 'install';
      attemptedPreparation.current.clear(); setPreparationError(''); setPreparationFailure(undefined);
      await run(async () => { await requestPreparation(action); });
      return;
    }
    if (failedAction) {
      setPreparationFailure(undefined);
      setPreparationError('');
      await run(async () => { await requestPreparation(failedAction); });
      return;
    }
    if (snapshot.provider?.canInstall && state === 'idempotency-conflict') {
      // The previous key is known to conflict. Only this explicit click may
      // request a new key; status refreshes stay read-only.
      attemptedPreparation.current.clear(); setPreparationError(''); setPreparationFailure(undefined);
      await run(async () => { await requestPreparation('install'); });
      return;
    }
    if (snapshot.provider?.canRecover && state === 'recovery-failed') {
      attemptedPreparation.current.clear(); setPreparationError(''); setPreparationFailure(undefined);
      await run(async () => { await requestPreparation('recover'); });
      return;
    }
    if (snapshot.provider?.canInstall && state === 'deployment-failed') {
      attemptedPreparation.current.clear(); setPreparationError(''); setPreparationFailure(undefined);
      await run(async () => {
        await requestPreparation('install');
        notify("Android 环境安装请求已重新提交", 'info');
      });
      return;
    }
    if (snapshot.provider?.canInstall && state === 'preflight-blocked') {
      // A preflight block is terminal until the user has corrected the host
      // state or installed a corrected manifest. Only this explicit click
      // submits a fresh plan; status refreshes never do so.
      attemptedPreparation.current.clear(); setPreparationError(''); setPreparationFailure(undefined);
      await run(async () => { await requestPreparation('install'); });
      return;
    }
    attemptedPreparation.current.clear(); setPreparationError(''); setPreparationFailure(undefined); await refresh();
  }
  async function reauthorize() {
    const provider = snapshot.provider;
    if (busy || provider?.deploymentTask?.state !== 'denied') return;
    const state = provider.reauthorization?.state;
    if (state === 'awaitingShellConfirmation' || state === 'validating' || state === 'confirmed') return;
    await run(async () => {
      const result = await request<{ state?: string }>('/runtime/reauthorize', { method: 'POST' });
      notify(result.state === 'requestUnconfirmed' ? '重新授权请求尚未确认，可安全重试' : '已在 AgentOS 中请求重新授权',
        result.state === 'denied' || result.state === 'expired' || result.state === 'revalidationFailed' ? 'error' : 'info');
    });
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
        } catch (failure) { notify(userFacingFailure(failure), 'error'); }
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
  return { ...snapshot, error, loading, busy, notice, preparing, preparationError, preparationFailure, retryPreparation, reauthorize, refresh, importFiles, perform, runtime };
}
