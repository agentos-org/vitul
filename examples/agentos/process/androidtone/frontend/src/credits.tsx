import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { useI18n } from './i18n';
import { contextQuery } from './hostContext';
import './styles.css';
function Credits() {
  const { t } = useI18n();
  return <main className="mx-auto max-w-3xl px-6 py-10 leading-relaxed text-ink">
    <h1 className="mb-6 text-2xl font-bold">{t('图形署名')}</h1>
    <article className="space-y-4 rounded-3xl border border-line bg-surface p-6">
      <p>{t('署名说明')}</p>
      <p>Android is a trademark of Google LLC.</p>
      <p>Android robot artwork by Google, adapted by AndroidTone under CC BY 3.0.</p>
      <p><a className="text-brand underline" href="https://creativecommons.org/licenses/by/3.0/">Creative Commons Attribution 3.0</a></p>
      <p><a className="text-brand underline" href="https://developer.android.com/distribute/marketing-tools/brand-guidelines">Android Brand Guidelines</a></p>
    </article>
    <a className="mt-6 inline-block text-brand underline" href={`${import.meta.env.BASE_URL}${contextQuery()}`}>{t('返回 AndroidTone')}</a>
  </main>;
}
createRoot(document.getElementById('root')!).render(<StrictMode><Credits /></StrictMode>);
