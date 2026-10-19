import { expect, test, type Page } from '@playwright/test';
import type { AndroidApp, InstallTask, Provider } from '../src/api';

async function fixture(page: Page, provider: Provider, apps: AndroidApp[] = [], tasks: InstallTask[] = []) {
  const calls: { method: string; path: string; body: string }[] = [];
  let reads = 0;
  const faults: string[] = [];
  page.on('pageerror', error => faults.push(error.message));
  await page.route('**/apps/androidtone/api/**', async route => {
    const request = route.request();
    const path = new URL(request.url()).pathname.replace('/apps/androidtone/api', '');
    if (request.method() === 'GET') {
      if (path === '/status') reads++;
      await route.fulfill({ json: path === '/status' ? { provider } : path === '/apps' ? { apps } : { installTasks: tasks } });
    } else {
      calls.push({ method: request.method(), path, body: request.postData() || '' });
      if (path === '/install-tasks') {
        tasks.push({ id: 'task 1', fileName: 'demo.apk', status: 'waiting-provider', message: '等待运行环境' });
        await route.fulfill({ json: { task: tasks.at(-1) } });
      } else if (path === '/install-tasks/task%201/install') {
        tasks[0].status = 'installed'; await route.fulfill({ json: { message: '安装完成' } });
      } else if (path === '/install-tasks/task%201') {
        tasks.splice(0); await route.fulfill({ json: { message: '已移除' } });
      } else await route.fulfill({ json: { message: '操作已完成' } });
    }
  });
  await page.goto('/apps/androidtone/');
  await expect(page.getByRole('button', { name: '刷新', exact: true })).toBeEnabled();
  return { calls, faults, reads: () => reads };
}

test('offline import, constrained runtime actions, retry and remove task', async ({ page }) => {
  const state = await fixture(page, { available: false, state: 'runtime-missing', canInstall: false, message: '需要系统授权' });
  await expect(page.getByRole('button', { name: '安装 Waydroid' })).toHaveCount(0);
  await page.getByLabel('APK 文件', { exact: true }).setInputFiles({ name: 'bad.txt', mimeType: 'text/plain', buffer: Buffer.from('bad') });
  await expect(page.getByRole('status')).toContainText('不是 APK 文件');
  expect(state.calls).toHaveLength(0);
  await page.getByLabel('APK 文件', { exact: true }).setInputFiles({ name: 'demo.apk', mimeType: 'application/vnd.android.package-archive', buffer: Buffer.from('APK fixture') });
  await expect(page.getByRole('heading', { name: '安装任务' })).toBeVisible();
  expect(state.calls[0].body).toContain('filename="demo.apk"');
  await page.getByRole('button', { name: '重试' }).click();
  await expect(page.getByRole('button', { name: '重新安装' })).toBeEnabled();
  await page.getByRole('button', { name: '移除' }).click();
  await expect(page.getByRole('heading', { name: '安装任务' })).toHaveCount(0);
  expect(state.calls.map(call => [call.method, call.path])).toEqual([
    ['POST', '/install-tasks'], ['POST', '/install-tasks/task%201/install'], ['DELETE', '/install-tasks/task%201'],
  ]);
  expect(state.faults).toEqual([]);
});

test('application actions, escaped labels and uninstall confirmation', async ({ page }) => {
  const state = await fixture(page, { available: true, initialized: true, canStart: false }, [
    { id: 'com.example app', name: '<img src=x onerror=alert(1)>', runningStateAvailable: true, version: '1.0' },
    { id: 'com.example.unknown', name: '未知状态应用', runningStateAvailable: false },
  ]);
  await expect(page.getByRole('button', { name: '启动环境' })).toHaveCount(0);
  const known = page.getByRole('article').filter({ hasText: '<img src=x onerror=alert(1)>' });
  await expect(known.getByRole('heading')).toHaveText('<img src=x onerror=alert(1)>');
  expect(await known.locator('img').count()).toBe(0);
  await known.getByRole('button', { name: '启动', exact: true }).click();
  await expect(known.getByRole('button', { name: '启动', exact: true })).toBeEnabled();
  await page.getByRole('article').filter({ hasText: '未知状态应用' }).getByRole('button', { name: '停止', exact: true }).click();
  await expect(known.getByRole('button', { name: '卸载' })).toBeEnabled();
  page.once('dialog', dialog => dialog.dismiss());
  await known.getByRole('button', { name: '卸载' }).click();
  expect(state.calls).toHaveLength(2);
  page.once('dialog', dialog => dialog.accept());
  await known.getByRole('button', { name: '卸载' }).click();
  await expect.poll(() => state.calls.length).toBe(3);
  expect(state.calls.map(call => [call.method, call.path])).toEqual([
    ['POST', '/apps/com.example%20app/launch'], ['POST', '/apps/com.example.unknown/stop'], ['DELETE', '/apps/com.example%20app'],
  ]);
  expect(state.faults).toEqual([]);
});

test('active installation polling and responsive production assets', async ({ page }) => {
  const state = await fixture(page, { available: true, running: true, initialized: true },
    [{ id: 'com.example.demo', name: '示例应用', running: true }],
    [{ id: 'active', fileName: 'installing.apk', status: 'installing', message: '正在安装' }]);
  await expect(page.getByRole('button', { name: '安装中', exact: true })).toBeDisabled();
  await expect.poll(state.reads, { timeout: 10000 }).toBeGreaterThan(1);
  for (const width of [1120, 390]) {
    await page.setViewportSize({ width, height: 844 });
    await expect(page.getByRole('heading', { name: 'AndroidTone 安卓通' })).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.screenshot({ path: `../agentos-build/verification/react-store-${width}.png`, fullPage: true });
  }
  expect(state.faults).toEqual([]);
});

test('HTTP errors are shown without enabling platform actions', async ({ page }) => {
  await page.route('**/apps/androidtone/api/**', route => route.fulfill({ status: 503, json: { error: { message: '服务暂时不可用' } } }));
  await page.goto('/apps/androidtone/');
  await expect(page.getByRole('status')).toHaveText('服务暂时不可用');
  await expect(page.getByRole('button', { name: '安装 Waydroid' })).toHaveCount(0);
});

test('missing platform transport shows a helpful summary and expandable diagnostics', async ({ page }) => {
  const state = await fixture(page, {
    available: false, state: 'provider-unavailable', canInstall: false,
    message: 'AgentOS 尚未向 AndroidTone 授予 系统服务。',
    runtimeBundle: { included: true, waydroidVersion: '1.6.2', androidVersion: '13', packageCount: 261 },
  });
  await expect(page.getByText('Android 运行环境准备受阻', { exact: true })).toBeVisible();
  await expect(page.getByText('Android 运行环境还需要平台提供底层设备配置和容器管理能力。APK 可以先导入保存，系统就绪后会自动准备。')).toBeVisible();
  await expect(page.locator('details p')).not.toBeVisible();
  await page.getByText('查看连接详情').click();
  await expect(page.locator('details p')).toContainText('Android 13 运行环境');
  await expect(page.locator('details p')).toContainText('AgentOS 尚未向 AndroidTone 授予 系统服务。');
  await expect(page.getByRole('button', { name: '安装 Waydroid' })).toHaveCount(0);
  await page.screenshot({ path: '../agentos-build/verification/android-green-unavailable.png', fullPage: true });
  expect(state.faults).toEqual([]);
});

test('an unconfirmed APK is failed with manual retry and collapsed Waydroid diagnostics', async ({ page }) => {
  const summary = 'APK 安装未能确认；请展开任务详情查看诊断后重试。';
  const diagnostic = 'Waydroid exited with zero but com.example.chat was not registered. INSTALL_FAILED_TEST';
  const state = await fixture(page, { available: true, running: true, initialized: true }, [], [
    { id: 'unconfirmed-apk', fileName: 'chat.apk', status: 'failed', message: summary, diagnostic },
  ]);
  const task = page.getByRole('article').filter({ hasText: 'chat.apk' });
  await expect(task.getByRole('button', { name: '重试', exact: true })).toBeEnabled();
  await expect(task.getByRole('button', { name: '重新安装', exact: true })).toHaveCount(0);
  await expect(task.getByText(diagnostic, { exact: true })).not.toBeVisible();
  expect(state.calls).toHaveLength(0);
  await task.getByText('查看连接详情').click();
  await expect(task.getByText(summary, { exact: true })).toBeVisible();
  await expect(task.getByText(diagnostic, { exact: true })).toBeVisible();
  await page.route('**/api/install-tasks/unconfirmed-apk/install', route => route.fulfill({
    status: 503, json: { error: { code: 'apk-install-unconfirmed', message: summary } },
  }));
  await task.getByRole('button', { name: '重试', exact: true }).click();
  await expect(page.getByRole('status')).toHaveText(summary);
  await expect(page.getByRole('status')).not.toContainText('INSTALL_FAILED_TEST');
  expect(state.faults).toEqual([]);
});
