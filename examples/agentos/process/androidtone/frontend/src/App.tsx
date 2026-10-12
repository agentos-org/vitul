import { useRef, useState, type ButtonHTMLAttributes, type ReactNode } from 'react';
import { Grid2X2, Package, Upload } from 'lucide-react';
import { preparationBlocked } from './preparation';
import type { AndroidApp, Provider } from './api';
import { useStore } from './useStore';
import { useI18n, type Translate } from './i18n';
import { contextQuery } from './hostContext';

function Button({ variant = 'outline', className = '', ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: 'primary' | 'outline' | 'danger' | 'danger-outline' }) {
  const colors = { primary: 'bg-brand text-on-brand', outline: 'border border-line bg-surface text-brand',
    danger: 'bg-danger-soft text-danger', 'danger-outline': 'border border-danger-line bg-surface text-danger' };
  return <button type="button" {...props} className={`cursor-pointer rounded-full px-4 py-2.5 text-sm font-bold transition hover:brightness-95 active:translate-y-px disabled:cursor-not-allowed disabled:opacity-50 ${colors[variant]} ${className}`} />;
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
function runtimeView(t: Translate, provider?: Provider) {
  const p = provider || {};
  const running = p.running === true || p.state === 'running';
  const bundle = p.runtimeBundle;
  const detail = (bundle?.included ? t("bundle", { waydroid: bundle.waydroidVersion, android: bundle.androidVersion, count: bundle.packageCount }) : '') + (p.message ? t(p.message) : '');
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
      <div className="grid min-w-0 flex-1 gap-1"><strong className="text-[13px]">{store.error ? t("无法连接 AndroidTone 服务") : !store.provider && store.loading ? t("正在检查运行环境") : store.preparing ? t(`preparing.${store.preparing}`) : store.preparationError ? t("Android 环境准备失败") : runtime.title}</strong><span className="text-xs leading-relaxed text-muted">{store.error ? t(store.error) : store.preparing ? t("首次准备可能需要一些时间，请保持窗口打开。") : runtime.message}</span>{(store.preparationError || store.provider?.message || store.provider?.runtimeBundle?.included) && !store.error && <details className="mt-1 text-xs text-muted"><summary className="w-fit cursor-pointer text-brand">{t("查看连接详情")}</summary><p className="mt-2 max-w-3xl leading-relaxed">{store.preparationError ? t(store.preparationError) : runtime.detail}</p></details>}</div>
      {(blocked || store.preparationError) && <Button className={small} disabled={store.busy || store.loading} onClick={() => void store.retryPreparation()}>{t("重新检查")}</Button>}
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
      <IconTile><Package className="size-7" /></IconTile><div className="min-w-0 flex-1 max-sm:min-w-[70%]"><strong className="block truncate text-[13px]">{task.fileName}</strong><div className="mt-1 break-words text-xs text-muted"><span>{t(task.status)}</span>{task.message && <details className="mt-1"><summary className="cursor-pointer">{t("查看连接详情")}</summary><p className="mt-1 break-words">{t(task.message)}</p></details>}</div></div>
      <Button variant="primary" className={small} disabled={store.busy || task.status === 'installing'} onClick={() => void store.perform('install', task.id)}>{task.status === 'installed' ? t("重新安装") : task.status === 'installing' ? t("安装中") : ['waiting-provider', 'failed'].includes(task.status) ? t("重试") : t("安装")}</Button>
      <Button className={small} disabled={store.busy} onClick={() => void store.perform('delete-task', task.id)}>{t("移除")}</Button>
    </article>)}</div></section>}
    <section><Heading title={t("已安装应用")} subtitle={t("你的 Android 应用库 · 启动、停止与卸载")} count={t("appCount", { count: store.apps.length })} icon /><div className="grid gap-3" aria-live="polite">
      {store.error ? <Empty title={t("无法连接 AndroidTone 服务")}>{t(store.error)}</Empty>
        : !store.provider && store.loading ? <Empty title={t("正在读取应用列表")}>{t("请稍候…")}</Empty>
          : !store.provider?.available ? <Empty title={unavailableTitle}>{t("导入的 APK 会保存为安装任务，环境准备完成后可以安装。")}</Empty>
            : store.appError ? <Empty title={t("无法读取 Waydroid 应用列表")}>{t(store.appError)}</Empty>
              : !store.apps.length ? <Empty title={t("还没有安装应用")}>{t("导入 APK 后，安装完成的应用会显示在这里。")}</Empty>
                : store.apps.map((app, index) => <AppCard key={app.id || app.packageName || index} app={app} store={store} />)}
    </div></section>
    {store.notice && <div role="status" className={`fixed right-4 bottom-5 z-10 max-w-[min(440px,calc(100vw-32px))] rounded-xl border bg-surface px-4 py-3 text-[13px] shadow-lg ${store.notice.kind === 'error' ? 'border-danger-line text-danger' : store.notice.kind === 'success' ? 'border-line text-success' : 'border-line text-muted'}`}>{t(store.notice.message, store.notice.params)}</div>}
    <footer className="mt-7 text-center text-[11px] text-muted">AndroidTone · {t("为 AgentOS 管理 Android 应用")} · <a className="underline underline-offset-2" href={`${import.meta.env.BASE_URL}credits.html${contextQuery()}`} target="_blank" rel="noopener">{t("图形署名")}</a></footer>
  </main>;
}
