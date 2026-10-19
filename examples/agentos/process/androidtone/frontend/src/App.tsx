import { useRef, useState, type ButtonHTMLAttributes, type ReactNode } from 'react';
import { Grid2X2, Package, Upload } from 'lucide-react';
import { preparationBlocked } from './preparation';
import type { AndroidApp, Provider } from './api';
import { useStore } from './useStore';
import { useI18n, type Translate } from './i18n';
import { contextQuery } from './hostContext';
import { diagnosticEntries, diagnosticReason, diagnosticTitle, deploymentDiagnostics, preflightSummary } from './deploymentDiagnostics';

function Button({ variant = 'outline', className = '', ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: 'primary' | 'outline' | 'danger' | 'danger-outline' }) {
  const colors = { primary: 'bg-brand text-on-brand', outline: 'border border-line bg-surface text-brand',
    danger: 'bg-danger-soft text-danger', 'danger-outline': 'border border-danger-line bg-surface text-danger' };
  return <button type="button" {...props} className={`cursor-pointer rounded-full px-4 py-2.5 text-sm font-bold transition hover:brightness-95 active:translate-y-px disabled:cursor-not-allowed disabled:opacity-50 ${colors[variant]} ${className}`} />;
}
function safeDiagnosticMessage(message?: string, t?: Translate): string | undefined {
  if (!message || message.length > 500 || /^\s*[\[{]/.test(message) || /Custom\s*\{\s*kind\s*:\s*Other/i.test(message)) return undefined;
  if (t && /ENOSPC|no space left on device/i.test(message)) return t("宿主磁盘空间不足，无法完成 Android 运行环境部署。");
  return message;
}
function AndroidIcon() {
  return <svg viewBox="0 0 24 24" aria-hidden="true" className="size-7"><g fill="currentColor"><path d="M6 9a6 6 0 0 1 12 0v1H6Zm0 2h12v6a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2Z" /><rect x="2.5" y="11" width="2.5" height="7" rx="1.25" /><rect x="19" y="11" width="2.5" height="7" rx="1.25" /><rect x="8" y="17" width="2.5" height="5" rx="1.25" /><rect x="13.5" y="17" width="2.5" height="5" rx="1.25" /></g><path d="m8 4-2-3m10 3 2-3" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" /><circle cx="9" cy="7" r=".8" fill="white" /><circle cx="15" cy="7" r=".8" fill="white" /></svg>;
}
function IconTile({ children }: { children: ReactNode }) {
  return <div className="grid size-[54px] shrink-0 place-items-center overflow-hidden rounded-2xl bg-soft text-market max-sm:size-[46px]">{children}</div>;
}
function Empty({ title, children }: { title: string; children: ReactNode }) {
  return <div className="rounded-3xl border border-line bg-surface px-4 py-9 text-center"><div className="mx-auto mb-2.5 grid size-11 place-items-center rounded-xl bg-soft text-market"><AndroidIcon /></div><strong className="text-sm">{title}</strong><p className="mt-1.5 text-xs leading-relaxed text-muted">{children}</p></div>;
}
function Heading({ title, subtitle, count, icon }: { title: string; subtitle: string; count: string; icon?: boolean }) {
  return <div className="mt-8 mb-3 flex items-end justify-between gap-3"><div><h2 className="flex items-center gap-2 text-[19px] font-bold">{icon && <Grid2X2 className="size-5 text-market" />} {title}</h2><p className="mt-1 text-xs text-muted">{subtitle}</p></div><span className="shrink-0 text-xs text-muted">{count}</span></div>;
}
function DiagnosticCards({ items, t }: { items: ReturnType<typeof diagnosticEntries>; t: Translate }) {
  if (!items.length) return null;
  return <ul className="grid gap-2" aria-label={t("部署诊断列表")}>
    {items.map((item, index) => <li key={`${String(item.code)}-${String(item.package)}-${index}`} className="rounded-md border border-line p-2">
      <strong>{item.code || item.reasonCode ? diagnosticTitle(item.code ?? item.reasonCode, t) : t("诊断字段 {field}", { field: String(item.field ?? t('详细信息')) })}</strong>
      {item.package && <div><span className="font-semibold">{t("软件包")}: </span><code>{item.package}</code></div>}
      {(item.installedVersion || item.requestedVersion) && <div>{t("版本状态", {
        installed: String(item.installedVersion ?? t('未安装')),
        requested: String(item.requestedVersion ?? t('未指定')),
      })}</div>}
      {item.field && <div>{t("诊断字段 {field}", { field: item.field })}: <code className="break-all">{String(item.value ?? '').slice(0, 500)}</code></div>}
      {typeof item.depends === 'string' && <div className="mt-1"><span className="font-semibold">{t("依赖条件")}: </span><code className="break-all">{item.depends}</code></div>}
      {diagnosticReason(item.reasonCode ?? item.code, t) && <p className="mt-1">{diagnosticReason(item.reasonCode ?? item.code, t)}</p>}
      {typeof item.message === 'string' && safeDiagnosticMessage(item.message, t) && <p className="mt-1 break-words">{safeDiagnosticMessage(item.message, t)}</p>}
      {item.systemCode === 'ENOSPC' && <p className="mt-1">ENOSPC · {t("磁盘空间不足")}</p>}
    </li>)}
  </ul>;
}
function DeploymentErrorDetails({ provider, t }: { provider?: Provider; t: Translate }) {
  const task = provider?.deploymentTask;
  const error = task?.error;
  if (!task) return null;
  const diagnostics = deploymentDiagnostics(provider);
  const authorizationTerminal = ['denied', 'expired', 'cancelled', 'canceled'].includes(task.state || '');
  const plainMessage = authorizationTerminal ? undefined : safeDiagnosticMessage(error?.message, t);
  return <div className="mt-2 grid gap-2 rounded-lg bg-soft p-2">
    <div>{t("部署任务")}: <span className="break-all">{task.taskId}</span> · {t(`taskState.${task.state || 'unknown'}`)}</div>
    {error?.code && <div><span className="font-semibold">{t("错误码")}: </span><code className="break-all">{error.code}</code></div>}
    {provider?.state === 'preflight-blocked'
      ? <p className="leading-relaxed">{preflightSummary(provider, t)}</p>
      : plainMessage && <p className="break-words leading-relaxed">{plainMessage}</p>}
    <DiagnosticCards items={diagnostics} t={t} />
    {diagnostics.length === 0 && Boolean(error?.details) && <p>{t("平台返回了结构化诊断；请联系管理员查看 AgentOS 系统日志。")}</p>}
  </div>;
}
function runtimeView(t: Translate, provider?: Provider) {
  const p = provider || {};
  const running = p.running === true || p.state === 'running';
  const bundle = p.runtimeBundle;
  const detail = (bundle?.included ? t("bundle", { waydroid: bundle.waydroidVersion, android: bundle.androidVersion, count: bundle.packageCount }) : '') + (p.message ? t(p.message) : '');
  const reauthorization = p.reauthorization;
  if (p.state === 'image-incompatible') return { title: t("Android 镜像与虚拟 CPU 不兼容"), message: p.canInstall !== true && p.canStop === true
    ? t("当前虚拟 CPU 只提供 64-bit op-mode，已初始化的 vendor 镜像却包含 ELF32 ARM 的 boringssl_self_test32；Android init 会遇到 Exec format error 并重启。请先停止 Android 运行环境；停止后 AndroidTone 会提交适配镜像的更新请求，并由 AgentOS Shell 请求授权。")
    : t("当前虚拟 CPU 只提供 64-bit op-mode，已初始化的 vendor 镜像却包含 ELF32 ARM 的 boringssl_self_test32；Android init 会遇到 Exec format error 并重启。请更新至 AndroidTone 随包的 ARM64-only 镜像。"), detail, dot: 'bg-danger' };
  if (p.state === 'runtime-update-required') return { title: t("Android 镜像需要更新"), message: p.canInstall !== true && p.canStop === true
    ? t("AndroidTone 已准备与当前虚拟 CPU 匹配的离线镜像。请先停止 Android 运行环境；停止后会提交镜像更新请求，并由 AgentOS Shell 请求授权。更新会保留 Waydroid 用户数据。")
    : t("AndroidTone 已准备与当前虚拟 CPU 匹配的离线镜像；更新会保留 Waydroid 用户数据。"), detail, dot: 'bg-warning' };
  if (p.state === 'preflight-blocked') return { title: t("Android 运行环境安装受阻"), message: preflightSummary(p, t), detail, dot: 'bg-danger' };
  if (p.state === 'authorization-expired') {
    const child = Boolean(p.deploymentTask?.retryOfTaskId || p.deploymentLineage?.retryOfTaskId);
    return child
      ? { title: t("重新授权创建的部署任务已过期"), message: t("AgentOS 已确认重新授权，但新部署任务仍已过期。旧任务和授权记录已保留；点击“重新申请”会创建新的部署计划，不会重复使用旧授权流程。"), detail, dot: 'bg-warning' }
      : { title: t("系统授权请求已过期"), message: t("本次部署的授权请求已过期，AndroidTone 已停止跟踪且不会自动重提。点击“重新申请”后会使用新的请求标识提交同一项部署。"), detail, dot: 'bg-warning' };
  }
  if (p.state === 'deployment-tracking-failed') return { title: t("无法读取部署状态"), message: t("暂时无法读取 AgentOS 部署任务状态；AndroidTone 未重新提交部署。请展开详情查看诊断并手动刷新。"), detail, dot: 'bg-danger' };
  if (p.state === 'image-restart-required') return { title: t("Android 镜像已更新，等待刷新容器"), message: t("新镜像已安装；AgentOS 需要授权重启 Waydroid 容器服务，随后再启动 Android 会话。"), detail, dot: 'bg-warning' };
  if (p.providerError) {
    const external = p.providerError.providerId === 'org.agentos.external-wayland' || p.providerError.code?.toLowerCase().includes('wayland');
    return { title: t(external ? "Android 显示会话启动失败" : "Android 运行环境操作失败"), message: t("系统服务操作未完成。请展开连接详情查看错误码和说明，再手动重试。"), detail, dot: 'bg-danger' };
  }
  if (reauthorization?.state === 'awaitingShellConfirmation') return { title: t("等待重新授权确认"), message: t("请在 AgentOS Shell 中确认是否重新申请；密码只由 Shell 收集。"), detail, dot: 'bg-warning' };
  if (reauthorization?.state === 'validating') return { title: t("AgentOS 正在重新检查"), message: t("AgentOS 正在重新检查系统能力和主机策略。"), detail, dot: 'bg-warning' };
  if (reauthorization?.state === 'requestUnconfirmed') return { title: t("重新授权请求未确认"), message: t("AgentOS 尚未确认请求；可以安全重试，重试会复用同一请求。"), detail, dot: 'bg-warning' };
  if (reauthorization?.state === 'denied') return { title: t("重新授权已拒绝"), message: t("AgentOS Shell 拒绝了重新授权；如需重试，请再次主动点击。"), detail, dot: 'bg-danger' };
  if (reauthorization?.state === 'expired') return { title: t("重新授权已过期"), message: t("重新授权请求已过期；如需重试，请再次主动点击。"), detail, dot: 'bg-warning' };
  if (reauthorization?.state === 'revalidationFailed') return { title: t("重新检查未通过"), message: safeDiagnosticMessage(reauthorization.error?.message || reauthorization.message, t) || t("系统能力或主机策略仍阻止此操作。请查看连接详情。"), detail, dot: 'bg-danger' };
  if (p.state === 'awaiting-authorization') return { title: t("等待系统授权"), message: t("AgentOS 正在等待管理员确认。请在系统授权窗口中批准本次操作。"), detail, dot: 'bg-warning' };
  if (p.state === 'authorization-cancelled') return { title: t("系统授权已取消"), message: t("系统授权已取消，本次操作已停止。"), detail, dot: 'bg-warning' };
  if (p.state === 'deploying' && p.deploymentKind === 'recovery') return { title: t("正在恢复 Android 环境"), message: `${t("正在准备 Binder 并重启 Android 容器服务。")} ${p.deploymentTask?.progress ?? 0}/3`, detail, dot: 'bg-warning' };
  if (p.state === 'recovery-required') return { title: t(p.canRecover ? "正在准备恢复 Android 环境" : "等待 AgentOS 系统能力"), message: p.message ? t(p.message) : '', detail, dot: 'bg-warning' };
  if (p.state === 'recovery-failed') return { title: t("Android 环境恢复失败"), message: deploymentDiagnostics(p).some(item => item.systemCode === 'ENOSPC') ? t("ENOSPC：设备磁盘空间不足，Android 环境部署失败；这不是授权失败。") : t("Android 环境恢复失败。请展开连接详情查看系统诊断，修正问题后再手动重试。"), detail, dot: 'bg-danger' };
  if (p.state === 'deploying') return { title: t("正在安装 Android 环境"), message: t("AgentOS 正在安装离线组件并初始化 Android 系统镜像。"), detail, dot: 'bg-warning' };
  if (p.state === 'runtime-ready') return { title: t("Android 运行环境已准备"), message: t("运行时和系统镜像已安装；AgentOS 应用会话控制接口仍需接入。"), detail, dot: 'bg-market' };
  if (p.state === 'authorization-denied') return { title: t("系统授权未通过"), message: t("AgentOS 未批准安装请求，因此宿主环境没有被更改。"), detail, dot: 'bg-danger' };
  if (p.state === 'deployment-failed') return { title: t("Android 环境安装失败"), message: deploymentDiagnostics(p).some(item => item.systemCode === 'ENOSPC') || /ENOSPC|no space left/i.test(p.deploymentTask?.error?.message || '') ? t("ENOSPC：设备磁盘空间不足，Android 环境部署失败；这不是授权失败。") : t("Android 环境安装失败。请展开连接详情查看 AgentOS 诊断，修正问题后再手动重试。"), detail, dot: 'bg-danger' };
  if ((p.state === 'runtime-missing' || p.state === 'platform-dependency') && p.canInstall !== true) return {
    title: t("等待 AgentOS 系统能力"),
    message: p.message ? t(p.message).slice(0, 240) : t("AgentOS 尚未开放完整的离线组件安装和 Binder 准备能力。刷新或重新检查只会读取当前状态，不会安装组件或弹出授权；平台能力接入后，AndroidTone 会自动提交授权请求。"),
    detail, dot: 'bg-warning'
  };
  return { title: running ? t("Android 环境已启动") : preparationBlocked(p) ? t("Android 环境准备受阻") : t("正在准备 Android 环境"),
    message: running ? t("运行状态不能证明窗口已呈现，需继续验证。") : preparationBlocked(p) ? t("Android 环境还需要平台提供底层设备配置和容器管理能力。APK 可以先导入保存，系统就绪后会自动准备。") : t("AndroidTone 会自动安装、初始化并启动所需环境，无需手动配置。"),
    detail, dot: running ? 'bg-market' : 'bg-warning' };
}
function AppCard({ app, store }: { app: AndroidApp; store: ReturnType<typeof useStore> }) {
  const { t } = useI18n();
  const id = app.id || app.packageName || '';
  const [brokenIcon, setBrokenIcon] = useState(false);
  return <article className="flex min-h-[88px] items-center gap-4 rounded-3xl border border-line bg-surface px-4 py-3.5 max-sm:flex-wrap max-sm:gap-2.5 max-sm:p-3">
    <IconTile>{app.iconUrl && !brokenIcon ? <img className="size-full object-cover" src={app.iconUrl} alt="" onError={() => setBrokenIcon(true)} /> : <AndroidIcon />}</IconTile>
    <div className="min-w-0 flex-1"><div className="flex flex-wrap items-baseline gap-2"><h3 className="break-words text-[15px] font-bold">{app.name || app.label || id}</h3><span className="text-xs text-muted">{app.version || t("版本未知")}</span></div><p className="mt-1 truncate text-xs text-muted">{app.packageName || id}</p></div>
    <div className="flex flex-wrap items-center justify-end gap-2 max-sm:ml-14 max-sm:basis-full max-sm:justify-start">
      <span className="rounded-full bg-badge px-2.5 py-1.5 text-[11px] font-bold text-badge-ink">{app.running ? t("运行中") : app.runningStateAvailable ? t("已安装") : t("状态未知")}</span>
      <Button variant="primary" className="px-3 py-2 text-xs" disabled={store.busy || !id} onClick={() => void store.perform('launch', id)}>{app.running ? t("打开") : t("启动")}</Button>
      {(app.running || !app.runningStateAvailable) && <Button className="px-3 py-2 text-xs" disabled={store.busy || !id} onClick={() => void store.perform('stop', id)}>{t("停止")}</Button>}
      <Button variant="danger" className="px-3 py-2 text-xs" disabled={store.busy || !id} onClick={() => void store.perform('uninstall', id)}>{t("卸载")}</Button>
    </div>
  </article>;
}
export function App() {
  const { t } = useI18n();
  const store = useStore();
  const input = useRef<HTMLInputElement>(null);
  const [dragging, setDragging] = useState(false);
  const runtime = runtimeView(t, store.provider);
  const choose = () => { if (!store.busy) input.current?.click(); };
  const blocked = preparationBlocked(store.provider);
  const canRetryDeployment = store.provider?.canInstall === true && store.provider.state === 'deployment-failed';
  const canReapplyExpired = store.provider?.state === 'authorization-expired';
  const idempotencyConflict = store.preparationFailure?.code?.toLowerCase().includes('idempotency') === true
    && store.preparationFailure.code.toLowerCase().includes('conflict');
  const reauthorizationState = store.provider?.reauthorization?.state;
  const deniedDeployment = store.provider?.deploymentTask?.state === 'denied';
  const canRetryReauthorizationRequest = deniedDeployment
    && reauthorizationState === 'requestUnconfirmed'
    && store.provider?.reauthorization?.canRetryRequest !== false;
  const canStartReauthorization = deniedDeployment
    && (['denied', 'expired', 'revalidationFailed', undefined].includes(reauthorizationState)
      || (reauthorizationState === 'confirmed' && store.provider?.deploymentTask?.taskId !== store.provider?.reauthorization?.deniedTaskId))
    && store.provider?.reauthorization?.canReauthorize !== false;
  const showReauthorizationAction = canRetryReauthorizationRequest || canStartReauthorization;
  const imageTransition = ['image-incompatible', 'runtime-update-required', 'image-restart-required'].includes(store.provider?.state || '');
  const canStopForImageUpdate = imageTransition && store.provider?.canStop === true;
  const waitingForRuntimeStop = imageTransition && store.provider?.canInstall !== true && store.provider?.canStop === true;
  const small = 'px-3 py-2 text-xs whitespace-nowrap';
  const unavailableTitle = t("Android 应用即将可用");
  return <main className="mx-auto max-w-[1120px] px-[34px] pt-9 pb-6 max-sm:px-4 max-sm:pt-6">
    <header className="mb-6 flex items-center justify-between gap-5 max-sm:flex-wrap">
      <div className="flex min-w-0 items-center gap-4 max-sm:gap-3"><img src={`${import.meta.env.BASE_URL}icon.png`} alt="" className="size-[68px] shrink-0 rounded-2xl max-sm:size-14" /><div className="min-w-0">
        <p className="mb-2 flex flex-wrap items-center gap-2 text-[11px] font-bold tracking-widest text-muted"><span className="rounded-full border border-line bg-badge px-2 py-1 tracking-normal text-badge-ink">{t("应用市场")}</span>ANDROID APPS</p>
        <h1 className="text-[28px] font-bold tracking-tight max-sm:text-2xl">AndroidTone <span className="ml-1 text-[19px] font-medium text-muted max-sm:text-base">{t("安卓通")}</span></h1>
        <p className="mt-2 text-sm text-muted">{t("导入 APK，安装与管理你的 Android 应用")}</p>
      </div></div>
      <div className="flex flex-wrap justify-end gap-2.5 max-sm:ml-auto">
        <Button onClick={choose} disabled={store.busy} aria-label={t("导入 APK")}>{t("＋ 导入")}</Button>
        <Button variant="primary" onClick={() => void store.refresh()} disabled={store.loading || store.busy}>{store.loading ? t("刷新中…") : t("刷新")}</Button>

      </div>
    </header>
    <section aria-live="polite" className="mb-4 flex min-h-[68px] flex-wrap items-center gap-3 rounded-3xl border border-line bg-surface px-4 py-3">
      <span className={`size-2.5 shrink-0 rounded-full ${store.error ? 'bg-danger' : runtime.dot}`} />
      <div className="grid min-w-0 flex-1 gap-1"><strong className="text-[13px]">{store.error ? t("无法连接 AndroidTone 服务") : !store.provider && store.loading ? t("正在检查运行环境") : store.preparing ? t(`preparing.${store.preparing}`) : store.preparationFailure ? t(store.preparationFailure.summary) : store.preparationError ? t("Android 环境准备失败") : runtime.title}</strong><span className="text-xs leading-relaxed text-muted">{store.error ? t(store.error) : store.preparing ? t("首次准备可能需要一些时间，请保持窗口打开。") : store.preparationFailure ? t(store.preparationFailure.summary) : runtime.message}</span>{(store.preparationError || store.provider?.message || store.provider?.providerError || store.provider?.runtimeBundle?.included || store.provider?.deploymentActions || store.provider?.deploymentTask || store.provider?.reauthorization || Boolean(store.provider?.deploymentTrackingDiagnostic) || Boolean(store.provider?.deploymentLineage)) && !store.error && <details className="mt-1 text-xs text-muted"><summary className="w-fit cursor-pointer text-brand">{t("查看连接详情")}</summary><p className="mt-2 max-w-3xl leading-relaxed">{store.preparationFailure ? t(store.preparationFailure.summary) : store.preparationError ? t(store.preparationError) : runtime.detail}</p>{store.preparationFailure && <dl className="mt-2 grid max-w-3xl gap-1 rounded-lg bg-soft p-2"><dt className="font-semibold text-ink">{t("诊断信息")}</dt>{store.preparationFailure.providerId && <div><dt className="inline font-semibold">{t("服务标识")}: </dt><dd className="inline break-all">{store.preparationFailure.providerId}</dd></div>}{store.preparationFailure.code && <div><dt className="inline font-semibold">{t("错误码")}: </dt><dd className="inline break-all">{store.preparationFailure.code}</dd></div>}<div><dt className="font-semibold">{t("错误说明")}</dt><dd className="mt-0.5 break-words">{t(store.preparationFailure.message)}</dd></div></dl>}{store.preparationFailure && Boolean(store.preparationFailure.details || store.preparationFailure.data || store.preparationFailure.diagnostics) && <DiagnosticCards items={diagnosticEntries(store.preparationFailure.details ?? store.preparationFailure.data ?? store.preparationFailure.diagnostics)} t={t} />}{store.provider?.providerError && !store.preparationFailure && <><dl className="mt-2 grid max-w-3xl gap-1 rounded-lg bg-soft p-2"><dt className="font-semibold text-ink">{t("诊断信息")}</dt>{store.provider.providerError.providerId && <div><dt className="inline font-semibold">{t("服务标识")}: </dt><dd className="inline break-all">{store.provider.providerError.providerId}</dd></div>}{store.provider.providerError.code && <div><dt className="inline font-semibold">{t("错误码")}: </dt><dd className="inline break-all">{store.provider.providerError.code}</dd></div>}{safeDiagnosticMessage(store.provider.providerError.message, t) && <div><dt className="font-semibold">{t("错误说明")}</dt><dd className="mt-0.5 break-words">{safeDiagnosticMessage(store.provider.providerError.message, t)}</dd></div>}</dl><DiagnosticCards items={diagnosticEntries(store.provider.providerError.details ?? store.provider.providerError.data ?? store.provider.providerError.diagnostics)} t={t} /></>}{store.provider?.deploymentTask && <DeploymentErrorDetails provider={store.provider} t={t} />}{Boolean(store.provider?.deploymentTrackingDiagnostic) && <div className="mt-2 rounded-lg bg-soft p-2"><strong>{t("部署状态读取诊断")}</strong><DiagnosticCards items={diagnosticEntries(store.provider?.deploymentTrackingDiagnostic)} t={t} /></div>}{store.provider?.deploymentLineage && <div className="mt-2 rounded-lg bg-soft p-2"><strong>{t("重新授权关联记录")}</strong>{store.provider.deploymentLineage.retryOfTaskId && <div>{t("关联的原任务")}: {store.provider.deploymentLineage.retryOfTaskId}</div>}{store.provider.deploymentLineage.reauthorizationRequestId && <div>{t("重新授权请求")}: {store.provider.deploymentLineage.reauthorizationRequestId}</div>}{store.provider.deploymentLineage.reauthorizationState && <div>{t("此前重新授权状态")}: {t(`reauth.${store.provider.deploymentLineage.reauthorizationState}`)}</div>}{store.provider.deploymentLineage.newTaskId && <div>{t("AgentOS 返回的新任务")}: {store.provider.deploymentLineage.newTaskId}</div>}<DiagnosticCards items={diagnosticEntries(store.provider.deploymentLineage.agentOsResponse)} t={t} /></div>}{store.provider?.reauthorization && <div className="mt-2 rounded-lg bg-soft p-2"><div>{t("重新授权状态")}: {t(`reauth.${store.provider.reauthorization.state || 'unknown'}`)}</div>{store.provider.reauthorization.deniedTaskId && <div>{t("原拒绝任务")}: {store.provider.reauthorization.deniedTaskId}</div>}{store.provider.reauthorization.requestId && <div>{t("重新授权请求")}: {store.provider.reauthorization.requestId}</div>}{store.provider.reauthorization.newTaskId && <div>{t("新部署任务")}: {store.provider.reauthorization.newTaskId}</div>}{(store.provider.reauthorization.error || store.provider.reauthorization.message) && <div className="mt-2 grid gap-1">{store.provider.reauthorization.error?.code && <div>{t("错误码")}: <code className="break-all">{store.provider.reauthorization.error.code}</code></div>}{safeDiagnosticMessage(store.provider.reauthorization.error?.message, t) && <p>{safeDiagnosticMessage(store.provider.reauthorization.error?.message, t)}</p>}<DiagnosticCards items={diagnosticEntries(store.provider.reauthorization.error?.details)} t={t} /></div>}</div>}{store.provider?.deploymentActions && <div className="mt-3 grid max-w-3xl gap-1.5">{Object.entries(store.provider.deploymentActions).map(([name, action]) => <div key={name} className="flex flex-wrap gap-x-2"><span className="font-semibold text-ink">{name}</span><span>{action.available === true ? t("平台能力可用") : action.available === false ? action.requiredForAndroidToneSetup === true ? t("平台能力不可用") : t("安卓通非必需") : name === "providerIsolation" ? t("安全策略已配置") : t("状态未报告")}</span>{action.reason && <code className="break-all">{action.reason}</code>}{action.state && <DiagnosticCards items={diagnosticEntries(action.state)} t={t} />}</div>)}</div>}</details>}</div>
      {canReapplyExpired && <div data-testid="expired-reapply-action" className="flex w-full flex-wrap items-center justify-between gap-3 rounded-2xl border border-line bg-soft px-3 py-2.5">
        <div className="min-w-0 flex-1"><strong className="text-xs font-bold">{t("授权请求已过期")}</strong><p className="mt-0.5 text-xs leading-relaxed text-muted">{store.provider?.canReapply === true ? t("点击后将重新提交部署请求，并由 AgentOS Shell 再次确认。") : t("AgentOS 报告所需系统能力尚未就绪，暂不能重新提交。请查看连接详情中的阻塞原因。")}</p></div>
        <Button variant="primary" className={small} disabled={store.busy || store.loading || store.provider?.canReapply !== true} onClick={() => void store.retryPreparation()}>{t("重新申请授权")}</Button>
      </div>}
      {(blocked || store.preparationError) && !waitingForRuntimeStop && !canReapplyExpired && <Button className={small} disabled={store.busy || store.loading} onClick={() => void store.retryPreparation()}>{t(idempotencyConflict || store.provider?.state === 'idempotency-conflict' ? "重新检查运行环境" : store.provider?.state === 'preflight-blocked' && store.provider.canInstall ? "修正后重新检查并提交" : store.preparationFailure?.action === 'start' ? "重试启动显示会话" : store.preparationFailure?.action === 'recover' || store.provider?.state === 'recovery-failed' && store.provider.canRecover ? "重试恢复" : store.preparationFailure?.action === 'install' || canRetryDeployment ? "重试安装" : "检查系统状态")}</Button>}
      {canStopForImageUpdate && <Button className={small} disabled={store.busy || store.loading} onClick={() => void store.runtime('stop')}>{t("停止 Android 环境")}</Button>}
      {showReauthorizationAction && <Button className={small} disabled={store.busy || store.loading} onClick={() => void store.reauthorize()}>{t(canRetryReauthorizationRequest ? "重试重新授权请求" : "在 AgentOS 中重新授权")}</Button>}
    </section>
    <input ref={input} className="hidden" type="file" multiple accept=".apk,application/vnd.android.package-archive" aria-label={t("APK 文件")} onChange={event => { const files = Array.from(event.currentTarget.files || []); event.currentTarget.value = ''; void store.importFiles(files); }} />
    <section role="button" tabIndex={store.busy ? -1 : 0} aria-disabled={store.busy} aria-label={t("点击或拖入 APK 文件")} onClick={choose}
      onKeyDown={event => { if (event.target === event.currentTarget && ['Enter', ' '].includes(event.key)) { event.preventDefault(); choose(); } }}
      onDragOver={event => { event.preventDefault(); setDragging(true); }} onDragLeave={event => { if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setDragging(false); }}
      onDrop={event => { event.preventDefault(); setDragging(false); void store.importFiles(Array.from(event.dataTransfer.files)); }}
      className={`flex min-h-[220px] cursor-pointer flex-col items-center justify-center rounded-3xl border-2 border-dashed p-7 text-center transition ${dragging ? 'border-market bg-[var(--drop-active)]' : 'border-[var(--drop-line)] bg-linear-to-br from-surface to-[var(--drop-end)]'}`}>
      <div className="mb-3 grid size-12 place-items-center rounded-3xl bg-badge text-brand"><Upload className="size-7" /></div>
      <h2 className="text-base font-bold text-ink max-sm:text-sm">{t("点击或拖拽 .apk 文件到此处安装应用")}</h2><p className="mt-2 mb-4 text-[13px] text-muted">{t("APK 将保存在此 AgentOS 设备上")}</p>
      <Button variant="primary" disabled={store.busy} onClick={event => { event.stopPropagation(); choose(); }}>{store.busy ? t("处理中…") : t("选择 APK")}</Button>
    </section>
    {store.tasks.length > 0 && <section><Heading title={t("安装任务")} subtitle={t("查看 APK 导入和安装状态")} count={t("taskCount", { count: store.tasks.length })} /><div className="grid gap-3">{store.tasks.map(task => <article key={task.id} className="flex flex-wrap items-center gap-3 rounded-3xl border border-line bg-surface px-4 py-3">
      <IconTile><Package className="size-7" /></IconTile><div className="min-w-0 flex-1 max-sm:min-w-[70%]"><strong className="block truncate text-[13px]">{task.fileName}</strong><div className="mt-1 break-words text-xs text-muted"><span>{t(task.status)}</span>{(task.message || task.diagnostic) && <details className="mt-1"><summary className="cursor-pointer">{t("查看连接详情")}</summary>{task.message && <p className="mt-1 break-words">{t(task.message)}</p>}{task.diagnostic && <div className="mt-2"><strong>{t("Waydroid 诊断")}</strong><pre className="mt-1 max-h-40 overflow-auto whitespace-pre-wrap break-words rounded-lg bg-soft p-2 font-mono text-[11px]">{task.diagnostic}</pre></div>}</details>}</div></div>
      <Button variant="primary" className={small} disabled={store.busy || task.status === 'installing'} onClick={() => void store.perform('install', task.id)}>{task.status === 'installed' ? t("重新安装") : task.status === 'installing' ? t("安装中") : ['waiting-provider', 'failed'].includes(task.status) ? t("重试") : t("安装")}</Button>
      <Button className={small} disabled={store.busy} onClick={() => void store.perform('delete-task', task.id)}>{t("移除")}</Button>
    </article>)}</div></section>}
    <section><Heading title={t("已安装应用")} subtitle={t("你的 Android 应用库 · 启动、停止与卸载")} count={t("appCount", { count: store.apps.length })} icon /><div className="grid gap-3" aria-live="polite">
      {store.error ? <Empty title={t("无法连接 AndroidTone 服务")}>{t(store.error)}</Empty>
        : !store.provider && store.loading ? <Empty title={t("正在读取应用列表")}>{t("请稍候…")}</Empty>
          : !store.provider?.available ? <Empty title={unavailableTitle}>{t("导入的 APK 会保存为安装任务，环境准备完成后可以安装。")}</Empty>
              : store.appError ? <Empty title={t("无法读取 Waydroid 应用列表")}>{typeof store.appError === 'string'
                ? t(store.appError)
                : <><p>{t("系统服务操作未完成。请展开连接详情查看错误码和说明，再手动重试。")}</p><details className="mt-2 text-left"><summary className="cursor-pointer text-brand">{t("查看连接详情")}</summary><dl className="mt-2 grid gap-1 rounded-lg bg-soft p-2">{store.appError.providerId && <div><dt className="inline font-semibold">{t("服务标识")}: </dt><dd className="inline break-all">{store.appError.providerId}</dd></div>}{store.appError.code && <div><dt className="inline font-semibold">{t("错误码")}: </dt><dd className="inline break-all">{store.appError.code}</dd></div>}{store.appError.message && <div><dt className="font-semibold">{t("错误说明")}</dt><dd className="mt-0.5 break-words">{store.appError.message}</dd></div>}</dl></details></>}</Empty>
              : !store.apps.length ? <Empty title={t("还没有安装应用")}>{t("导入 APK 后，安装完成的应用会显示在这里。")}</Empty>
                : store.apps.map((app, index) => <AppCard key={app.id || app.packageName || index} app={app} store={store} />)}
    </div></section>
    {store.notice && <div role="status" className={`fixed right-4 bottom-5 z-10 max-w-[min(440px,calc(100vw-32px))] rounded-xl border bg-surface px-4 py-3 text-[13px] shadow-lg ${store.notice.kind === 'error' ? 'border-danger-line text-danger' : store.notice.kind === 'success' ? 'border-line text-success' : 'border-line text-muted'}`}>{t(store.notice.message, store.notice.params)}</div>}
    <footer className="mt-7 text-center text-[11px] text-muted">AndroidTone · {t("为 AgentOS 管理 Android 应用")} · <a className="underline underline-offset-2" href={`${import.meta.env.BASE_URL}credits.html${contextQuery()}`} target="_blank" rel="noopener">{t("图形署名")}</a></footer>
  </main>;
}
