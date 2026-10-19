import { useSyncExternalStore } from 'react';

export type Locale = 'zh-CN' | 'en-US';
export type Theme = 'light' | 'dark';
interface HostContext { locale: Locale; theme: Theme }
const query = new URLSearchParams(location.search);
// The WebShell plugin URL carries hostOrigin/appearance/theme but may not
// include the runtime marker used by the native AgentOS launch URL.
const hostOrigin = query.get('hostOrigin');
let hosted = query.get('runtime') === 'agentos' || hostOrigin !== null;
let appearance = hosted ? query.get('appearance') || 'system' : 'system';
const media = matchMedia('(prefers-color-scheme: dark)');
const localeOf = (value: string): Locale => value.toLowerCase().startsWith('zh') ? 'zh-CN' : 'en-US';
let current: HostContext = {
  locale: localeOf(hosted ? query.get('locale') || document.documentElement.lang || navigator.language : navigator.language),
  theme: hosted && ['light', 'dark'].includes(query.get('theme') || '') ? query.get('theme') as Theme
    : hosted && ['light', 'dark'].includes(document.documentElement.dataset.theme || '') ? document.documentElement.dataset.theme as Theme
      : media.matches ? 'dark' : 'light',
};
const listeners = new Set<() => void>();
function apply() {
  const root = document.documentElement;
  root.lang = current.locale;
  root.dataset.theme = current.theme;
  root.dataset.fennTheme = current.theme;
  root.classList.toggle('dark', current.theme === 'dark');
  root.classList.toggle('theme-dark', current.theme === 'dark');
  root.style.colorScheme = current.theme;
  root.style.backgroundColor = current.theme === 'dark' ? '#101b14' : '#f6faf3';
  document.title = current.locale === 'zh-CN' ? 'AndroidTone · 安卓通' : 'AndroidTone';
  document.querySelector('meta[name="theme-color"]')?.setAttribute('content', current.theme === 'dark' ? '#101b14' : '#f6faf3');
}
function update(next: HostContext) {
  if (next.locale === current.locale && next.theme === current.theme) return;
  current = next; apply(); listeners.forEach(listener => listener());
}
window.addEventListener('message', (event: MessageEvent<unknown>) => {
  const fromParent = event.source === window.parent && window.parent !== window;
  const expectedParentOrigin = (() => {
    try { return hostOrigin ? new URL(hostOrigin).origin : location.origin; } catch { return ''; }
  })();
  if (event.source !== window && !(fromParent && event.origin === expectedParentOrigin)) return;
  if (!event.data || typeof event.data !== 'object') return;
  const value = event.data as Record<string, unknown>;
  if (value.type === 'shell:theme' && (value.appearance === 'light' || value.appearance === 'dark')) {
    hosted = true; update({ ...current, theme: value.appearance }); return;
  }
  if (value.type === 'shell:locale' && (value.locale === 'zh' || value.locale === 'en')) {
    hosted = true; update({ ...current, locale: localeOf(value.locale) }); return;
  }
  if (value.channel !== 'agentos:host' || value.type !== 'context-changed' || value.runtime !== 'agentos'
    || (value.schemaVersion !== undefined && value.schemaVersion !== 1)) return;
  hosted = true;
  if (value.appearance === 'system' || value.appearance === 'light' || value.appearance === 'dark') appearance = value.appearance;
  update({
    locale: value.locale === 'zh-CN' || value.locale === 'en-US' ? value.locale : current.locale,
    theme: value.theme === 'light' || value.theme === 'dark' ? value.theme : current.theme,
  });
});
media.addEventListener('change', event => {
  if (!hosted || appearance === 'system') update({ ...current, theme: event.matches ? 'dark' : 'light' });
});
window.addEventListener('languagechange', () => { if (!hosted) update({ ...current, locale: localeOf(navigator.language) }); });
apply();
export function useHostContext() {
  return useSyncExternalStore(listener => { listeners.add(listener); return () => { listeners.delete(listener); }; }, () => current);
}
export function contextQuery(): string {
  return hosted ? `?${new URLSearchParams({ runtime: 'agentos', locale: current.locale, theme: current.theme, appearance })}` : '';
}
