import { expect, test, type Page } from '@playwright/test';
async function api(page: Page) {
  await page.route('**/apps/androidtone/api/**', route => {
    const path = new URL(route.request().url()).pathname;
    return route.fulfill({ json: path.endsWith('/status') ? { provider: { available: false, state: 'provider-unavailable', canInstall: false } }
      : path.endsWith('/apps') ? { apps: [] } : { installTasks: [] } });
  });
}
for (const locale of ['zh', 'en']) for (const theme of ['light', 'dark']) {
  test(`AgentOS first render ${locale}/${theme}, credits and narrow layout`, async ({ page }) => {
    await api(page);
    await page.emulateMedia({ colorScheme: theme === 'dark' ? 'light' : 'dark' });
    await page.goto(`/apps/androidtone/?runtime=agentos&locale=${locale}&appearance=${theme}&theme=${theme}`);
    await expect(page.locator('html')).toHaveAttribute('lang', locale === 'zh' ? 'zh-CN' : 'en-US');
    await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
    await expect(page.getByRole('button', { name: locale === 'zh' ? '选择 APK' : 'Choose APK', exact: true })).toBeVisible();
    await expect(page.getByRole('heading', { name: locale === 'zh' ? '已安装应用' : 'Installed apps' })).toBeVisible();
    const background = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);
    expect(background).toBe(theme === 'dark' ? 'rgb(16, 27, 20)' : 'rgb(246, 250, 243)');
    for (const width of [1120, 390]) {
      await page.setViewportSize({ width, height: 900 });
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
      await page.screenshot({ path: `../agentos-build/verification/${locale}-${theme}-${width}.png`, fullPage: true });
    }
    const credits = page.getByRole('link', { name: locale === 'zh' ? '图形署名' : 'Artwork credits' });
    const href = await credits.getAttribute('href');
    await page.goto(href!);
    await expect(page.getByRole('heading', { name: locale === 'zh' ? '图形署名' : 'Artwork credits' })).toBeVisible();
    await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
  });
}

test('live AgentOS changes, old schema, invalid versions and foreign frames', async ({ page }) => {
  await api(page);
  await page.goto('/apps/androidtone/?runtime=agentos&locale=zh&theme=light');
  await expect(page.getByRole('button', { name: '选择 APK' })).toBeVisible();
  const message = { channel: 'agentos:host', type: 'context-changed', runtime: 'agentos', schemaVersion: 1, locale: 'en-US', appearance: 'dark', theme: 'dark' };
  await page.evaluate(value => window.postMessage(value, location.origin), message);
  await expect(page.getByRole('button', { name: 'Choose APK' })).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.evaluate(value => window.postMessage({ ...value, schemaVersion: 99, theme: 'light', locale: 'zh-CN' }, location.origin), message);
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.evaluate(value => {
    const frame = document.createElement('iframe');
    frame.srcdoc = `<script>parent.postMessage(${JSON.stringify({ ...value, theme: 'light', locale: 'zh-CN' })}, '*')<\/script>`;
    document.body.append(frame);
  }, message);
  await expect(page.locator('html')).toHaveAttribute('lang', 'en-US');
  await page.evaluate(value => { const { schemaVersion: _, ...old } = value; window.postMessage({ ...old, locale: 'zh-CN', theme: 'light' }, location.origin); }, message);
  await expect(page.getByRole('button', { name: '选择 APK' })).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
});

test('ordinary browser uses language and live system color scheme', async ({ browser }) => {
  const context = await browser.newContext({ locale: 'en-US', colorScheme: 'dark' });
  const page = await context.newPage(); await api(page);
  await page.goto('http://127.0.0.1:5188/apps/androidtone/');
  await expect(page.getByRole('button', { name: 'Choose APK' })).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.emulateMedia({ colorScheme: 'light' });
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await context.close();
});

test('host dark theme is applied before React loads', async ({ page }) => {
  await page.route('**/assets/*.js', route => route.abort());
  await page.goto('/apps/androidtone/?runtime=agentos&locale=en&theme=dark');
  await expect(page.locator('#root')).toBeEmpty();
  await expect(page.locator('html')).toHaveAttribute('lang', 'en-US');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  expect(await page.evaluate(() => getComputedStyle(document.body).backgroundColor)).toBe('rgb(16, 27, 20)');
});

test('English actions, confirmation, task status and live translated notification', async ({ page }) => {
  await page.route('**/apps/androidtone/api/**', route => {
    const path = new URL(route.request().url()).pathname;
    return route.fulfill({ json: path.endsWith('/status') ? { provider: { available: true, initialized: true } }
      : path.endsWith('/apps') ? { apps: [{ id: 'com.example.demo', name: 'Demo', runningStateAvailable: true }] }
        : { installTasks: [{ id: 'one', fileName: 'demo.apk', status: 'waiting-provider', message: '诊断原文' }] } });
  });
  await page.goto('/apps/androidtone/?runtime=agentos&locale=en&theme=dark');
  await expect(page.getByText('1 app', { exact: true })).toBeVisible();
  await expect(page.getByText('1 task', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Retry', exact: true })).toBeVisible();
  await expect(page.getByText('Waiting for runtime', { exact: true })).toBeVisible();
  page.once('dialog', async dialog => { expect(dialog.message()).toBe('Uninstall this Android app?'); await dialog.dismiss(); });
  await page.getByRole('button', { name: 'Uninstall', exact: true }).click();
  await page.getByLabel('APK files', { exact: true }).setInputFiles({ name: 'bad.txt', mimeType: 'text/plain', buffer: Buffer.from('bad') });
  await expect(page.getByRole('status')).toHaveText('bad.txt is not an APK file');
  await page.evaluate(() => window.postMessage({ channel: 'agentos:host', type: 'context-changed', runtime: 'agentos', locale: 'zh-CN', theme: 'light' }, location.origin));
  await expect(page.getByRole('status')).toHaveText('bad.txt 不是 APK 文件');
});

test('legacy Shell theme follows automatic dark changes without a runtime URL marker', async ({ page }) => {
  await api(page); await page.emulateMedia({ colorScheme: 'light' });
  await page.goto('/apps/androidtone/');
  await expect(page.getByRole('button', { name: '选择 APK' })).toBeVisible();
  await page.evaluate(() => window.postMessage({ type: 'shell:theme', appearance: 'dark' }, location.origin));
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  expect(await page.evaluate(() => getComputedStyle(document.body).backgroundColor)).toBe('rgb(16, 27, 20)');
  await page.evaluate(() => window.postMessage({ type: 'shell:locale', locale: 'en' }, location.origin));
  await expect(page.getByRole('button', { name: 'Choose APK' })).toBeVisible();
  await page.evaluate(() => window.postMessage({ type: 'shell:theme', appearance: 'light' }, location.origin));
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
});

test('modern Shell context works without launch query and system media follows hosted automatic appearance', async ({ page }) => {
  await api(page); await page.emulateMedia({ colorScheme: 'light' });
  await page.goto('/apps/androidtone/');
  await page.evaluate(() => window.postMessage({ channel: 'agentos:host', type: 'context-changed', runtime: 'agentos', theme: 'dark', appearance: 'system', locale: 'en-US' }, location.origin));
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await expect(page.getByRole('button', { name: 'Choose APK' })).toBeVisible();
  await page.emulateMedia({ colorScheme: 'dark' });
  await expect.poll(() => page.evaluate(() => matchMedia('(prefers-color-scheme: dark)').matches)).toBe(true);
  await page.evaluate(() => new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
  await page.emulateMedia({ colorScheme: 'light' });
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
});

for (const locale of ['zh', 'en']) test(`runtime terminology stays user friendly in ${locale} diagnostics`, async ({ page }) => {
  await page.route('**/apps/androidtone/api/**', route => {
    const path = new URL(route.request().url()).pathname;
    return route.fulfill({ json: path.endsWith('/status') ? { provider: { available: false, state: 'provider-unavailable', canInstall: false, message: 'Waydroid Host Provider unavailable', runtimeBundle: { included: true, waydroidVersion: '1.6.2', androidVersion: '13', packageCount: 261 } } }
      : path.endsWith('/apps') ? { apps: [] } : { installTasks: [{ id: 'one', fileName: 'demo.apk', status: 'failed', message: 'Waydroid Provider failed' }] } });
  });
  await page.goto(`/apps/androidtone/?runtime=agentos&locale=${locale}&theme=dark`);
  await expect(page.getByRole('button', { name: locale === 'zh' ? '重试' : 'Retry', exact: true })).toBeVisible();
  await page.locator('summary').evaluateAll(nodes => nodes.forEach(node => (node as HTMLElement).click()));
  await expect(page.locator('body')).not.toContainText(/waydroid|host provider/i);
  await expect(page.locator('body')).toContainText(locale === 'zh' ? 'Android 运行环境' : 'Android runtime');
});
