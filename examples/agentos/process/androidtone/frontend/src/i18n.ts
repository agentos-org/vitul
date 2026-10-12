import { useCallback, useRef } from 'react';
import { useHostContext } from './hostContext';
import { chinese, english } from './messages';
export type Translate = (key: string, params?: Record<string, string | number>) => string;
export function useI18n() {
  const context = useHostContext();
  const locale = useRef(context.locale);
  locale.current = context.locale;
  const t: Translate = useCallback((key, params) => {
    if (locale.current === 'en-US' && /^请求失败（\d+）$/.test(key)) return `Request failed (${key.match(/\d+/)?.[0]})`;
    if (locale.current === 'en-US' && params?.count === 1 && (key === 'appCount' || key === 'taskCount')) return key === 'appCount' ? '1 app' : '1 task';
    const canonical = Object.keys(english).find(candidate => english[candidate] === key) ?? key;
    const template = locale.current === 'zh-CN' ? chinese[canonical] ?? canonical : english[canonical] ?? key;
    return template.replace(/\{(\w+)\}/g, (match, name: string) => String(params?.[name] ?? match))
      .replace(/Waydroid/gi, locale.current === 'zh-CN' ? 'Android 运行环境' : 'Android runtime')
      .replace(/Host Provider|Provider/gi, locale.current === 'zh-CN' ? '系统服务' : 'system service')
      .replace(/Android 环境/g, 'Android 运行环境')
      .replace(/Android environment/g, 'Android runtime');
  }, []);
  return { ...context, t };
}
