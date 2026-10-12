import { expect, test } from '@playwright/test';

test('setup automatically installs, initializes and starts once, without Waydroid controls', async ({ page }) => {
  const calls: string[] = [];
  let phase = 0;
  await page.route('**/apps/androidtone/api/**', async route => {
    const path = new URL(route.request().url()).pathname.split('/api')[1];
    if (route.request().method() === 'POST') {
      calls.push(path); phase++;
      await route.fulfill({ json: { message: 'ok' } }); return;
    }
    await route.fulfill({ json: path === '/status' ? { provider: {
      state: phase === 0 ? 'runtime-missing' : phase === 1 ? 'uninitialized' : phase === 2 ? 'stopped' : 'running',
      available: phase >= 2, installed: phase >= 1, initialized: phase >= 2, running: phase === 3,
      canInstall: phase === 0, canInitialize: phase === 1, canStart: phase === 2,
    } } : path === '/apps' ? { apps: [] } : { installTasks: [] } });
  });
  await page.goto('/apps/androidtone/');
  await expect(page.getByText('Android 运行环境已启动', { exact: true })).toBeVisible();
  expect(calls).toEqual(['/runtime/install', '/runtime/initialize', '/runtime/start']);
  await expect(page.getByRole('button', { name: '安装 Waydroid' })).toHaveCount(0);
  await page.getByRole('button', { name: '刷新', exact: true }).click();
  await expect(page.getByRole('button', { name: '刷新', exact: true })).toBeEnabled();
  expect(calls).toHaveLength(3);
});

test('blocked capabilities make no privileged requests; failed setup needs explicit retry', async ({ page }) => {
  let allowed = false;
  let installs = 0;
  await page.route('**/apps/androidtone/api/**', route => {
    const path = new URL(route.request().url()).pathname.split('/api')[1];
    if (route.request().method() === 'POST') {
      installs++; return route.fulfill({ status: 403, json: { error: 'Authorization declined' } });
    }
    return route.fulfill({ json: path === '/status' ? { provider: { state: 'runtime-missing', installed: false, available: false, canInstall: allowed } }
      : path === '/apps' ? { apps: [] } : { installTasks: [] } });
  });
  await page.goto('/apps/androidtone/');
  await expect(page.getByText('Android 运行环境准备受阻', { exact: true })).toBeVisible();
  expect(installs).toBe(0);
  allowed = true;
  await page.getByRole('button', { name: '重新检查' }).click();
  await expect(page.getByText('Android 运行环境准备失败', { exact: true })).toBeVisible();
  expect(installs).toBe(1);
  await page.getByRole('button', { name: '刷新', exact: true }).click();
  await expect(page.getByRole('button', { name: '刷新', exact: true })).toBeEnabled();
  expect(installs).toBe(1);
  await page.getByRole('button', { name: '重新检查' }).click();
  await expect.poll(() => installs).toBe(2);
});
