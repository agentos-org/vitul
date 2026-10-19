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

test('expired first deployment is restored after reload and only explicit reapply creates a fresh plan', async ({ page }) => {
  let phase: 'runtime-missing' | 'awaiting-authorization' | 'authorization-expired' = 'runtime-missing';
  let plans = 0;
  const calls: string[] = [];
  const deploymentActions = Object.fromEntries(['debian.packages.ensure', 'linux.module.load', 'linux.binderfs.ensure'].map(name => [name, { available: true, schemaVersions: name === 'debian.packages.ensure' ? [1] : undefined }]));
  await page.route('**/apps/androidtone/api/**', async route => {
    const path = new URL(route.request().url()).pathname.split('/api')[1];
    if (route.request().method() === 'POST') {
      calls.push(path);
      expect(path).toBe('/runtime/install');
      expect(route.request().postData() || '').not.toMatch(/password/i);
      plans++;
      phase = 'awaiting-authorization';
      if (plans === 2) await new Promise(resolve => setTimeout(resolve, 250));
      await route.fulfill({ json: { message: 'accepted' } });
      return;
    }
    const provider = {
      state: phase,
      installed: phase !== 'runtime-missing',
      initialized: false,
      canInstall: phase === 'runtime-missing',
      canReapply: phase === 'authorization-expired',
      deploymentKind: 'setup',
      deploymentActions,
      deploymentTask: phase === 'runtime-missing' ? undefined : {
        taskId: 'expired-setup-task',
        state: phase === 'awaiting-authorization' ? 'awaitingAuthorization' : 'expired',
        error: phase === 'authorization-expired' ? { code: 'authorization-expired', message: 'The authorization request has expired.' } : undefined,
      },
    };
    await route.fulfill({ json: path === '/status' ? { provider } : path === '/apps' ? { apps: [] } : { installTasks: [] } });
  });

  await page.goto('/apps/androidtone/');
  await expect(page.getByText('等待系统授权', { exact: true })).toBeVisible();
  expect(plans).toBe(1);

  phase = 'authorization-expired';
  await page.getByRole('button', { name: '刷新', exact: true }).click();
  await expect(page.getByText('系统授权请求已过期', { exact: true })).toBeVisible();
  await expect(page.getByTestId('expired-reapply-action')).toBeVisible();
  await expect(page.getByRole('button', { name: '重新申请授权', exact: true })).toBeEnabled();
  await expect(page.locator('body')).not.toContainText('The authorization request has expired.');
  await expect(page.locator('body')).not.toContainText('{"code"');
  expect(plans).toBe(1);

  await page.reload();
  await expect(page.getByText('系统授权请求已过期', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: '重新申请授权', exact: true })).toBeVisible();
  expect(plans).toBe(1);

  const reapply = page.getByRole('button', { name: '重新申请授权', exact: true });
  const clickResults = await Promise.allSettled([
    reapply.click(),
    reapply.click({ timeout: 1200 }),
  ]);
  expect(clickResults[0].status).toBe('fulfilled');
  expect(clickResults[1].status).toBe('fulfilled');
  await expect(page.getByText('等待系统授权', { exact: true })).toBeVisible();
  expect(plans).toBe(2);
  expect(calls).toEqual(['/runtime/install', '/runtime/install']);
  await page.getByRole('button', { name: '刷新', exact: true }).click();
  await expect(page.getByRole('button', { name: '刷新', exact: true })).toBeEnabled();
  expect(plans).toBe(2);
  await expect(page.locator('input[type=password]')).toHaveCount(0);
});

test('expired authorization recovery copy is localized in English and does not resubmit on load', async ({ page }) => {
  let posts = 0;
  await page.route('**/apps/androidtone/api/**', route => {
    const path = new URL(route.request().url()).pathname.split('/api')[1];
    if (route.request().method() === 'POST') posts++;
    return route.fulfill({ json: path === '/status' ? { provider: {
      state: 'authorization-expired', installed: false, initialized: false,
      canInstall: false, canReapply: true, deploymentKind: 'setup',
      deploymentTask: { taskId: 'expired-task', state: 'expired', error: { code: 'authorization-expired', message: 'The authorization request has expired.' } },
    } } : path === '/apps' ? { apps: [] } : { installTasks: [] } });
  });
  await page.goto('/apps/androidtone/?runtime=agentos&locale=en&appearance=light&theme=light');
  await expect(page.getByText('System authorization request expired', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Request authorization again', exact: true })).toBeVisible();
  expect(posts).toBe(0);
  await expect(page.locator('body')).not.toContainText('The authorization request has expired.');
});

test('expired deployment keeps a visible reapply action and explains when AgentOS capabilities block it', async ({ page }) => {
  let posts = 0;
  await page.route('**/apps/androidtone/api/**', route => {
    const path = new URL(route.request().url()).pathname.split('/api')[1];
    if (route.request().method() === 'POST') posts++;
    return route.fulfill({ json: path === '/status' ? { provider: {
      state: 'authorization-expired', installed: false, initialized: false,
      canInstall: false, canReapply: false, deploymentKind: 'setup',
      deploymentTask: { taskId: 'expired-blocked-task', state: 'expired' },
      deploymentActions: {
        'debian.packages.ensure': { available: false, reason: 'schema-version-1-unavailable', requiredForAndroidToneSetup: true },
        'linux.module.load': { available: true, requiredForAndroidToneSetup: true },
        'linux.binderfs.ensure': { available: true, requiredForAndroidToneSetup: true },
      },
    } } : path === '/apps' ? { apps: [] } : { installTasks: [] } });
  });
  await page.setViewportSize({ width: 560, height: 900 });
  await page.goto('/apps/androidtone/');
  await expect(page.getByTestId('expired-reapply-action')).toBeVisible();
  await expect(page.getByRole('button', { name: '重新申请授权', exact: true })).toBeDisabled();
  await expect(page.getByText('AgentOS 报告所需系统能力尚未就绪，暂不能重新提交。请查看连接详情中的阻塞原因。', { exact: true })).toBeVisible();
  expect(posts).toBe(0);
});

test('expired reauthorization child task creates a new recovery plan, never a second reauthorization request', async ({ page }) => {
  let phase: 'authorization-expired' | 'awaiting-authorization' = 'authorization-expired';
  const calls: string[] = [];
  await page.route('**/apps/androidtone/api/**', async route => {
    const path = new URL(route.request().url()).pathname.split('/api')[1];
    if (route.request().method() === 'POST') {
      calls.push(path);
      expect(path).toBe('/runtime/recover');
      phase = 'awaiting-authorization';
      await route.fulfill({ json: { message: 'submitted' } });
      return;
    }
    const provider = phase === 'authorization-expired' ? {
      state: phase, installed: true, initialized: true, canRecover: true, canReapply: true,
      deploymentKind: 'recovery',
      deploymentTask: { taskId: 'new-expired-task', state: 'expired', retryOfTaskId: 'old-denied-task', reauthorizationRequestId: 'reauth-confirmed' },
      reauthorization: { requestId: 'reauth-confirmed', deniedTaskId: 'old-denied-task', newTaskId: 'new-expired-task', state: 'confirmed' },
      deploymentLineage: { retryOfTaskId: 'old-denied-task', reauthorizationRequestId: 'reauth-confirmed', reauthorizationState: 'confirmed', newTaskId: 'new-expired-task', agentOsResponse: { state: 'confirmed', newTaskId: 'new-expired-task' } },
    } : {
      state: phase, installed: true, initialized: true,
      deploymentKind: 'recovery', deploymentTask: { taskId: 'replacement-recovery-task', state: 'awaitingAuthorization' },
    };
    await route.fulfill({ json: path === '/status' ? { provider } : path === '/apps' ? { apps: [] } : { installTasks: [] } });
  });

  await page.goto('/apps/androidtone/');
  await expect(page.getByText('重新授权创建的部署任务已过期', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: '重新申请授权', exact: true })).toBeVisible();
  await page.getByRole('button', { name: '刷新', exact: true }).click();
  await expect(page.getByRole('button', { name: '刷新', exact: true })).toBeEnabled();
  expect(calls).toEqual([]);
  await page.getByText('查看连接详情', { exact: true }).click();
  await expect(page.getByText('关联的原任务: old-denied-task')).toBeVisible();
  await expect(page.getByText('AgentOS 返回的新任务: new-expired-task')).toBeVisible();

  await page.getByRole('button', { name: '重新申请授权', exact: true }).click();
  await expect(page.getByText('等待系统授权', { exact: true })).toBeVisible();
  expect(calls).toEqual(['/runtime/recover']);
  await expect(page.locator('input[type=password]')).toHaveCount(0);
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
  await expect(page.getByText('等待 AgentOS 系统能力', { exact: true })).toBeVisible();
  expect(installs).toBe(0);
  allowed = true;
  await page.getByRole('button', { name: '检查系统状态' }).click();
  await expect(page.getByText('Android 运行环境准备失败', { exact: true }).first()).toBeVisible();
  expect(installs).toBe(1);
  await page.getByRole('button', { name: '刷新', exact: true }).click();
  await expect(page.getByRole('button', { name: '刷新', exact: true })).toBeEnabled();
  expect(installs).toBe(1);
  await page.getByRole('button', { name: '重试安装' }).click();
  await expect.poll(() => installs).toBe(2);
});

test('idempotency conflict explains an explicit runtime recheck in Chinese', async ({ page }) => {
  let plans = 0;
  let phase = 'runtime-missing';
  const deploymentActions = {
    'debian.packages.ensure': { available: true, schemaVersions: [1] },
    'linux.module.load': { available: true },
    'linux.binderfs.ensure': { available: true },
  };
  await page.route('**/apps/androidtone/api/**', route => {
    const path = new URL(route.request().url()).pathname.split('/api')[1];
    if (route.request().method() === 'POST' && path === '/runtime/install') {
      plans++;
      if (plans === 1) {
        phase = 'idempotency-conflict';
        return route.fulfill({ status: 409, json: { error: {
          code: 'idempotency-key-conflict',
          message: 'This idempotency key was already used for a different operation request.',
        } } });
      }
      phase = 'awaiting-authorization';
      return route.fulfill({ json: { message: 'ok' } });
    }
    return route.fulfill({ json: path === '/status' ? { provider: {
      state: phase, canInstall: phase !== 'awaiting-authorization', installed: false,
      deploymentActions,
    } } : path === '/apps' ? { apps: [] } : { installTasks: [] } });
  });

  await page.goto('/apps/androidtone/');
  await expect(page.getByText('部署请求幂等键冲突', { exact: true }).first()).toBeVisible();
  expect(plans).toBe(1);
  await expect(page.getByRole('button', { name: '重新检查运行环境', exact: true })).toBeVisible();
  await page.getByText('查看连接详情', { exact: true }).click();
  await expect(page.getByText(/请刷新 Android 运行环境状态，再点击“重新检查运行环境”提交新请求。/)).toBeVisible();
  await expect(page.getByText('This idempotency key was already used for a different operation request.')).toHaveCount(0);
  await expect(page.locator('body')).not.toContainText('{"error"');
  await page.getByRole('button', { name: '刷新', exact: true }).click();
  await expect(page.getByRole('button', { name: '刷新', exact: true })).toBeEnabled();
  expect(plans).toBe(1);
  await page.reload();
  await expect(page.getByRole('button', { name: '重新检查运行环境', exact: true })).toBeVisible();
  expect(plans).toBe(1);

  await page.getByRole('button', { name: '重新检查运行环境', exact: true }).click();
  await expect.poll(() => plans).toBe(2);
  await expect(page.getByText('等待系统授权', { exact: true })).toBeVisible();
});

test('capability details expose reason and host state without planning a task', async ({ page }) => {
  let posts = 0;
  await page.route('**/apps/androidtone/api/**', route => {
    const path = new URL(route.request().url()).pathname.split('/api')[1];
    if (route.request().method() === 'POST') posts++;
    return route.fulfill({ json: path === '/status' ? { provider: {
      state: 'runtime-missing', canInstall: false, installed: false,
      deploymentActions: { 'linux.binderfs.ensure': {
        available: false, reason: 'binderfs-disabled-by-host-policy',
        state: { moduleFileInstalled: true, moduleLoaded: false, binderfsMounted: false },
      } },
    } } : path === '/apps' ? { apps: [] } : { installTasks: [] } });
  });
  await page.goto('/apps/androidtone/');
  await page.getByText('查看连接详情', { exact: true }).click();
  await expect(page.getByText('binderfs-disabled-by-host-policy', { exact: true })).toBeVisible();
  await expect(page.getByText(/binderfsMounted/).first()).toBeVisible();
  await expect(page.getByText('false', { exact: true }).first()).toBeVisible();
  await page.getByRole('button', { name: '检查系统状态' }).click();
  await expect(page.getByRole('button', { name: '检查系统状态' })).toBeEnabled();
  expect(posts).toBe(0);
});

test('AArch32 image mismatch shows a concrete CPU and Android init diagnosis', async ({ page }) => {
  const diagnostic = '当前 Waydroid 配置为 arm64_only，但已初始化的 vendor 镜像仍包含 ELF32 ARM EABI5 的 boringssl_self_test32。此虚拟 CPU 的 lscpu 只报告 64-bit CPU op-mode，不提供 AArch32 EL0；Android init 执行该加密自检会收到 Exec format error，并因 reboot_on_failure 反复重启。';
  let posts = 0;
  await page.route('**/apps/androidtone/api/**', route => {
    const path = new URL(route.request().url()).pathname.split('/api')[1];
    if (route.request().method() === 'POST') posts++;
    return route.fulfill({ json: path === '/status' ? { provider: {
      state: 'image-incompatible', canInstall: false, installed: true,
      message: diagnostic,
    } } : path === '/apps' ? { apps: [] } : { installTasks: [] } });
  });
  await page.goto('/apps/androidtone/');
  await expect(page.getByText('Android 镜像与虚拟 CPU 不兼容', { exact: true })).toBeVisible();
  await expect(page.getByText(/只提供 64-bit op-mode.*Exec format error/)).toBeVisible();
  await page.getByText('查看连接详情', { exact: true }).click();
  await expect(page.locator('details p')).toContainText('boringssl_self_test32');
  await expect(page.locator('details p')).toContainText('AArch32 EL0');
  expect(posts).toBe(0);
});

test('a no-conflict ensure plan reaches Shell authorization and denial is not automatically retried', async ({ page }) => {
  let phase = 'runtime-missing';
  let plans = 0;
  const deploymentActions = {
    'debian.packages.ensure': { available: true, schemaVersions: [1] },
    'linux.module.load': { available: true },
    'linux.binderfs.ensure': { available: true },
  };
  await page.route('**/apps/androidtone/api/**', route => {
    const path = new URL(route.request().url()).pathname.split('/api')[1];
    if (route.request().method() === 'POST') {
      plans++; phase = 'awaiting-authorization';
      return route.fulfill({ json: { message: 'ok' } });
    }
    return route.fulfill({ json: path === '/status' ? { provider: {
      state: phase, canInstall: phase === 'runtime-missing', installed: false,
      deploymentActions,
    } } : path === '/apps' ? { apps: [] } : { installTasks: [] } });
  });
  await page.goto('/apps/androidtone/');
  await expect(page.getByText('等待系统授权', { exact: true })).toBeVisible();
  await expect(page.locator('input[type=password]')).toHaveCount(0);
  expect(plans).toBe(1);
  phase = 'authorization-denied';
  await page.getByRole('button', { name: '刷新', exact: true }).click();
  await expect(page.getByText('系统授权未通过', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: '重试安装', exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: '刷新', exact: true }).click();
  await expect(page.getByRole('button', { name: '刷新', exact: true })).toBeEnabled();
  expect(plans).toBe(1);
});

test('preflight package conflicts show versions and only a user click submits a new plan', async ({ page }) => {
  let plans = 0;
  const provider = {
    state: 'preflight-blocked', canInstall: true, installed: false, initialized: false,
    message: 'AgentOS 离线包预检查发现版本或依赖冲突；任务未进入系统授权队列。',
    deploymentTask: { taskId: 'blocked-task', state: 'blocked', operationVersion: 3, error: {
      code: 'preflight-blocked', message: 'The operation did not pass host preflight.', details: [{
        actionIndex: 2, type: 'debian.packages.ensure', code: 'package-version-conflict',
        package: 'androidtone-waydroid-images', installedVersion: '0.1.7', requestedVersion: '0.1.8-1',
        reasonCode: 'package-upgrade-disallowed',
      }],
    } },
    deploymentActions: {
      'debian.packages.ensure': { available: true, schemaVersions: [1] },
      'linux.module.load': { available: true }, 'linux.binderfs.ensure': { available: true },
    },
  };
  await page.route('**/apps/androidtone/api/**', route => {
    const path = new URL(route.request().url()).pathname.split('/api')[1];
    if (route.request().method() === 'POST') {
      expect(path).toBe('/runtime/install');
      plans++;
      return route.fulfill({ json: { provider } });
    }
    return route.fulfill({ json: path === '/status' ? { provider }
      : path === '/apps' ? { apps: [] } : { installTasks: [] } });
  });
  await page.goto('/apps/androidtone/');
  await expect(page.getByText('Android 运行环境安装受阻', { exact: true })).toBeVisible();
  await expect(page.getByText(/androidtone-waydroid-images.*0\.1\.7.*0\.1\.8-1/).first()).toBeVisible();
  await expect(page.getByText(/需要 AgentOS 提供通用的 Shell 确认精确包升级能力/).first()).toBeVisible();
  expect(plans).toBe(0);
  await page.getByRole('button', { name: '刷新', exact: true }).click();
  await expect(page.getByRole('button', { name: '刷新', exact: true })).toBeEnabled();
  expect(plans).toBe(0);
  await page.getByText('查看连接详情', { exact: true }).click();
  await expect(page.getByText('包版本冲突', { exact: true })).toBeVisible();
  await expect(page.getByText('AgentOS 当前策略禁止自动升级已安装的系统包。', { exact: true })).toBeVisible();
  await expect(page.locator('pre').filter({ hasText: 'preflight-blocked' })).toHaveCount(0);
  await expect(page.getByRole('button', { name: '修正后重新检查并提交', exact: true })).toBeVisible();
  await page.getByRole('button', { name: '修正后重新检查并提交', exact: true }).click();
  await expect.poll(() => plans).toBe(1);
  await page.getByRole('button', { name: '刷新', exact: true }).click();
  await expect(page.getByRole('button', { name: '刷新', exact: true })).toBeEnabled();
  expect(plans).toBe(1);
  await expect(page.locator('input[type=password]')).toHaveCount(0);
});

test('unsatisfied shared dependencies show the Debian requirement without retrying a blocked plan', async ({ page }) => {
  let plans = 0;
  const provider = {
    state: 'preflight-blocked', canInstall: true, installed: false, initialized: false,
    deploymentTask: { taskId: 'dependency-blocked-task', state: 'blocked', error: {
      code: 'preflight-blocked', details: [{ code: 'dependency-unsatisfied', package: 'python3-gbinder', depends: 'python3-gbinder (>= 1.2)', reasonCode: 'apt-solver-unsatisfied' }],
    } },
    deploymentActions: { 'debian.packages.ensure': { available: true, schemaVersions: [1] } },
  };
  await page.route('**/apps/androidtone/api/**', route => {
    const path = new URL(route.request().url()).pathname.split('/api')[1];
    if (route.request().method() === 'POST') plans++;
    return route.fulfill({ json: path === '/status' ? { provider }
      : path === '/apps' ? { apps: [] } : { installTasks: [] } });
  });
  await page.goto('/apps/androidtone/');
  await page.getByText('查看连接详情', { exact: true }).click();
  await expect(page.getByText('依赖未满足', { exact: true })).toBeVisible();
  await expect(page.getByText('python3-gbinder', { exact: true })).toBeVisible();
  await expect(page.getByText(/python3-gbinder \(>= 1\.2\)/)).toBeVisible();
  await expect(page.locator('pre')).toHaveCount(0);
  await page.getByRole('button', { name: '刷新', exact: true }).click();
  await expect(page.getByRole('button', { name: '刷新', exact: true })).toBeEnabled();
  expect(plans).toBe(0);
});

for (const outcome of ['authorization-cancelled', 'runtime-ready', 'deployment-failed']) {
  test(`awaiting authorization follows ${outcome} without duplicate plans`, async ({ page }) => {
    let phase = 'runtime-missing';
    let plans = 0;
    let reads = 0;
    const fullActions = Object.fromEntries(['debian.packages.ensure', 'linux.module.load', 'linux.binderfs.ensure'].map(name => [name, { implemented: true, available: true, schemaVersions: name === 'debian.packages.ensure' ? [1] : undefined, reason: null, state: { moduleLoaded: false } }]));
    await page.route('**/apps/androidtone/api/**', route => {
      const path = new URL(route.request().url()).pathname.split('/api')[1];
      if (route.request().method() === 'POST') {
        expect(path).toBe('/runtime/install');
        expect(route.request().postData() || '').not.toMatch(/password/i);
        plans++; phase = 'awaiting-authorization';
        return route.fulfill({ json: { provider: { state: phase } } });
      }
      if (path === '/status') reads++;
      return route.fulfill({ json: path === '/status' ? { provider: {
        state: phase, canInstall: phase === 'runtime-missing', installed: phase === 'runtime-ready',
        deploymentActions: fullActions,
        deploymentTask: plans ? { taskId: 'test-deployment-task', state: phase === 'awaiting-authorization' ? 'awaitingAuthorization' : phase === 'deploying' ? 'running' : outcome === 'runtime-ready' ? 'completed' : outcome === 'deployment-failed' ? 'failed' : 'cancelled', error: phase === 'deployment-failed' ? { code: 'deployment-failed', message: 'No space left on device', details: { systemCode: 'ENOSPC' } } : undefined } : undefined,
        message: phase === 'deployment-failed' ? 'Android runtime deployment failed.' : '',
      } } : path === '/apps' ? { apps: [] } : { installTasks: [] } });
    });
    await page.goto('/apps/androidtone/');
    await expect(page.getByText('等待系统授权', { exact: true })).toBeVisible();
    // Exercise the active polling interval, not just a manual refresh.
    const initialReads = reads;
    await expect.poll(() => reads, { timeout: 6000 }).toBeGreaterThan(initialReads);
    expect(plans).toBe(1);
    if (outcome !== 'authorization-cancelled') {
      phase = 'deploying';
      await page.getByRole('button', { name: '刷新', exact: true }).click();
      await expect(page.getByText('正在安装 Android 运行环境', { exact: true })).toBeVisible();
    }
    phase = outcome;
    await page.getByRole('button', { name: '刷新', exact: true }).click();
    await expect(page.getByText(outcome === 'authorization-cancelled' ? '系统授权已取消' : outcome === 'runtime-ready' ? 'Android 运行环境已准备' : 'Android 运行环境安装失败', { exact: true })).toBeVisible();
    await page.getByText('查看连接详情', { exact: true }).click();
    await expect(page.getByText(/部署任务: test-deployment-task/)).toBeVisible();
    if (outcome === 'deployment-failed') {
      await expect(page.getByText('ENOSPC · 磁盘空间不足', { exact: true })).toBeVisible();
      await expect(page.locator('pre')).toHaveCount(0);
    }
    await page.getByRole('button', { name: '刷新', exact: true }).click();
    await expect(page.getByRole('button', { name: '刷新', exact: true })).toBeEnabled();
    expect(plans).toBe(1);
  });
}

for (const outcome of ['completed', 'failed', 'denied', 'cancelled', 'expired']) {
  test(`installed runtime recovery ${outcome} requests no package installation and no duplicate authorization`, async ({ page }) => {
    let phase = 'recovery-required';
    const calls: string[] = [];
    await page.route('**/apps/androidtone/api/**', route => {
      const path = new URL(route.request().url()).pathname.split('/api')[1];
      if (route.request().method() === 'POST') {
        calls.push(path);
        if (path === '/runtime/recover') phase = 'awaiting-authorization';
        if (path === '/runtime/start') phase = 'process-running';
        return route.fulfill({ json: { message: 'ok' } });
      }
      return route.fulfill({ json: path === '/status' ? { provider: {
        installed: true, initialized: true, state: phase, running: phase === 'process-running',
        available: phase === 'session-ready' || phase === 'process-running',
        canRecover: phase === 'recovery-required' || phase === 'recovery-failed' || phase === 'authorization-expired',
        canReapply: phase === 'authorization-expired',
        canStart: phase === 'recovery-required' || phase === 'session-ready', canInstall: false,
        deploymentKind: 'recovery',
        deploymentTask: { taskId: 'recovery-task', state: phase === 'awaiting-authorization' ? 'awaitingAuthorization' : phase === 'deploying' ? 'running' : outcome, progress: phase === 'deploying' ? 2 : 0,
          error: phase === 'recovery-failed' ? { message: 'Service restart failed', details: { unit: 'waydroid-container.service' } } : undefined },
      } } : path === '/apps' ? { apps: [] } : { installTasks: [] } });
    });
    await page.goto('/apps/androidtone/');
    await expect(page.getByText('等待系统授权', { exact: true })).toBeVisible();
    expect(calls).toEqual(['/runtime/recover']);
    await page.getByRole('button', { name: '刷新', exact: true }).click();
    await expect(page.getByRole('button', { name: '刷新', exact: true })).toBeEnabled();
    expect(calls).toEqual(['/runtime/recover']);
    if (['completed', 'failed'].includes(outcome)) {
      phase = 'deploying';
      await page.getByRole('button', { name: '刷新', exact: true }).click();
      await expect(page.getByText('正在恢复 Android 运行环境', { exact: true })).toBeVisible();
      await expect(page.getByText(/2\/3/)).toBeVisible();
    }
    phase = outcome === 'completed' ? 'session-ready' : outcome === 'failed' ? 'recovery-failed' : outcome === 'denied' ? 'authorization-denied' : outcome === 'expired' ? 'authorization-expired' : 'authorization-cancelled';
    await page.getByRole('button', { name: '刷新', exact: true }).click();
    if (outcome === 'completed') {
      await expect(page.getByText('Android 运行环境已启动', { exact: true })).toBeVisible();
      expect(calls).toEqual(['/runtime/recover', '/runtime/start']);
    } else if (outcome === 'failed') {
      await expect(page.getByRole('button', { name: '重试恢复', exact: true })).toBeVisible();
      expect(calls).toEqual(['/runtime/recover']);
      await page.getByText('查看连接详情', { exact: true }).click();
      await expect(page.getByText(/unit.*waydroid-container\.service/)).toBeVisible();
      await page.getByRole('button', { name: '重试恢复', exact: true }).click();
      await expect(page.getByText('等待系统授权', { exact: true })).toBeVisible();
      expect(calls).toEqual(['/runtime/recover', '/runtime/recover']);
    } else if (outcome === 'expired') {
      await expect(page.getByText('系统授权请求已过期', { exact: true })).toBeVisible();
      await expect(page.getByRole('button', { name: '重新申请授权', exact: true })).toBeVisible();
      await page.getByRole('button', { name: '重新申请授权', exact: true }).click();
      await expect(page.getByText('等待系统授权', { exact: true })).toBeVisible();
      expect(calls).toEqual(['/runtime/recover', '/runtime/recover']);
    } else {
      await expect(page.getByText(outcome === 'denied' ? '系统授权未通过' : '系统授权已取消', { exact: true })).toBeVisible();
      await page.getByRole('button', { name: '刷新', exact: true }).click();
      await expect(page.getByRole('button', { name: '刷新', exact: true })).toBeEnabled();
      expect(calls).toEqual(['/runtime/recover']);
    }
    expect(calls).not.toContain('/runtime/install');
    await expect(page.locator('input[type=password]')).toHaveCount(0);
  });
}

test('recovery capability blocker explains missing fixed service action without prompting', async ({ page }) => {
  let posts = 0;
  await page.route('**/apps/androidtone/api/**', route => {
    const path = new URL(route.request().url()).pathname.split('/api')[1];
    if (route.request().method() === 'POST') posts++;
    return route.fulfill({ json: path === '/status' ? { provider: { installed: true, initialized: true, state: 'recovery-required', canRecover: false, canStart: false,
      message: 'systemd.service.control: systemd-unit-not-allowlisted (waydroid-container.service)' } } : path === '/apps' ? { apps: [] } : { installTasks: [] } });
  });
  await page.goto('/apps/androidtone/');
  await expect(page.getByText('等待 AgentOS 系统能力', { exact: true })).toBeVisible();
  await expect(page.getByText(/systemd.service.control:/).first()).toBeVisible();
  await page.getByRole('button', { name: '检查系统状态' }).click();
  await expect(page.getByRole('button', { name: '检查系统状态' })).toBeEnabled();
  expect(posts).toBe(0);
});

test('display session failure keeps diagnostics out of the toast and retries only after a click', async ({ page }) => {
  let starts = 0;
  let running = false;
  const errorCode = 'external-wayland-session-unavailable';
  const errorMessage = 'Custom { kind: Other, error: "Private Wayland socket unavailable" }';
  await page.route('**/apps/androidtone/api/**', async route => {
    const path = new URL(route.request().url()).pathname.split('/api')[1];
    if (path === '/runtime/start' && route.request().method() === 'POST') {
      starts++;
      if (starts === 1) {
        await route.fulfill({ status: 503, json: { error: {
          code: errorCode,
          providerId: 'org.agentos.external-wayland',
          message: errorMessage,
        } } });
        return;
      }
      running = true;
      await route.fulfill({ json: { running: true, state: 'running' } });
      return;
    }
    await route.fulfill({ json: path === '/status' ? { provider: {
      installed: true, initialized: true, available: running, running,
      state: running ? 'running' : 'container-running', canStart: !running,
    } } : path === '/apps' ? { apps: [] } : { installTasks: [] } });
  });

  await page.goto('/apps/androidtone/');
  await expect(page.getByText('Android 显示会话启动失败', { exact: true }).first()).toBeVisible();
  const toast = page.getByRole('status');
  await expect(toast).toContainText('Android 显示会话启动失败');
  await expect(toast).not.toContainText(errorCode);
  await expect(toast).not.toContainText('Custom { kind: Other');
  await expect(toast).not.toContainText('{"error"');
  expect(starts).toBe(1);

  await page.getByText('查看连接详情', { exact: true }).click();
  await expect(page.getByText(errorCode, { exact: true })).toBeVisible();
  await expect(page.getByText(errorMessage, { exact: true })).toBeVisible();

  await page.getByRole('button', { name: '重试启动显示会话' }).click();
  await expect(page.getByText('Android 运行环境已启动', { exact: true })).toBeVisible();
  expect(starts).toBe(2);
  await page.getByRole('button', { name: '刷新', exact: true }).click();
  await expect(page.getByRole('button', { name: '刷新', exact: true })).toBeEnabled();
  expect(starts).toBe(2);
});

test('reauthorization is user initiated, waits for Shell, and follows the confirmed task', async ({ page }) => {
  let phase: 'denied' | 'awaitingShellConfirmation' | 'confirmed' = 'denied';
  let requests = 0;
  const calls: string[] = [];
  await page.route('**/apps/androidtone/api/**', async route => {
    const path = new URL(route.request().url()).pathname.split('/api')[1];
    if (route.request().method() === 'POST') {
      calls.push(path);
      expect(path).toBe('/runtime/reauthorize');
      expect(route.request().postData() || '').toBe('');
      requests++;
      phase = 'awaitingShellConfirmation';
      await route.fulfill({ json: { requestId: 'reauth-request', state: phase } });
      return;
    }
    const provider = phase === 'confirmed' ? {
      state: 'awaiting-authorization', installed: true, initialized: true, available: false,
      deploymentKind: 'recovery',
      deploymentTask: { taskId: 'new-task', state: 'awaitingAuthorization', retryOfTaskId: 'denied-task', reauthorizationRequestId: 'reauth-request' },
      reauthorization: { requestId: 'reauth-request', deniedTaskId: 'denied-task', newTaskId: 'new-task', state: 'confirmed' },
    } : {
      state: 'authorization-denied', installed: true, initialized: true, available: false,
      deploymentKind: 'recovery', deploymentTask: { taskId: 'denied-task', state: 'denied' },
      reauthorization: phase === 'awaitingShellConfirmation'
        ? { requestId: 'reauth-request', deniedTaskId: 'denied-task', state: phase }
        : undefined,
    };
    await route.fulfill({ json: path === '/status' ? { provider } : path === '/apps' ? { apps: [] } : { installTasks: [] } });
  });

  await page.goto('/apps/androidtone/');
  await expect(page.getByRole('button', { name: '在 AgentOS 中重新授权', exact: true })).toBeVisible();
  await page.getByRole('button', { name: '刷新', exact: true }).click();
  await expect(page.getByRole('button', { name: '刷新', exact: true })).toBeEnabled();
  expect(requests).toBe(0);
  expect(calls).toEqual([]);

  await page.getByRole('button', { name: '在 AgentOS 中重新授权', exact: true }).click();
  await expect(page.getByText('等待重新授权确认', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: '在 AgentOS 中重新授权', exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: '刷新', exact: true }).click();
  await expect(page.getByRole('button', { name: '刷新', exact: true })).toBeEnabled();
  expect(requests).toBe(1);

  phase = 'confirmed';
  await page.getByRole('button', { name: '刷新', exact: true }).click();
  await expect(page.getByText('等待系统授权', { exact: true })).toBeVisible();
  await page.getByText('查看连接详情', { exact: true }).click();
  await expect(page.getByText('新部署任务: new-task')).toBeVisible();
  await expect(page.getByText('原拒绝任务: denied-task')).toBeVisible();
  expect(calls).toEqual(['/runtime/reauthorize']);
  await expect(page.locator('input[type=password]')).toHaveCount(0);
});

test('revalidation failure can start a fresh reauthorization only after an explicit click', async ({ page }) => {
  let phase: 'revalidationFailed' | 'awaitingShellConfirmation' = 'revalidationFailed';
  let requests = 0;
  const calls: string[] = [];
  await page.clock.install();
  await page.route('**/apps/androidtone/api/**', async route => {
    const path = new URL(route.request().url()).pathname.split('/api')[1];
    if (route.request().method() === 'POST') {
      calls.push(path);
      expect(path).toBe('/runtime/reauthorize');
      expect(route.request().postData() || '').toBe('');
      requests++;
      phase = 'awaitingShellConfirmation';
      await route.fulfill({ json: { requestId: 'new-reauth-request', state: phase } });
      return;
    }
    const provider = {
      state: 'authorization-denied', installed: true, initialized: true, available: false,
      deploymentKind: 'recovery',
      deploymentTask: { taskId: 'denied-task', state: 'denied' },
      reauthorization: phase === 'revalidationFailed'
        ? { requestId: 'old-reauth-request', deniedTaskId: 'denied-task', state: phase,
          canReauthorize: true, canRetryRequest: false,
          error: { code: 'reauthorization-preflight-blocked', message: 'systemd.service.control: unit-not-allowlisted' } }
        : { requestId: 'new-reauth-request', deniedTaskId: 'denied-task', state: phase,
          canReauthorize: false, canRetryRequest: false },
    };
    await route.fulfill({ json: path === '/status' ? { provider } : path === '/apps' ? { apps: [] } : { installTasks: [] } });
  });

  await page.goto('/apps/androidtone/');
  await expect(page.getByText('系统条件重新检查未通过', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: '在 AgentOS 中重新授权', exact: true })).toBeVisible();
  expect(requests).toBe(0);
  expect(calls).toEqual([]);

  await page.clock.fastForward(16_000);
  await expect(page.getByRole('button', { name: '刷新', exact: true })).toBeEnabled();
  expect(requests).toBe(0);
  expect(calls).toEqual([]);

  await page.getByRole('button', { name: '刷新', exact: true }).click();
  await expect(page.getByRole('button', { name: '刷新', exact: true })).toBeEnabled();
  expect(requests).toBe(0);
  expect(calls).toEqual([]);

  await page.getByRole('button', { name: '在 AgentOS 中重新授权', exact: true }).click();
  await expect(page.getByText('等待重新授权确认', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: '在 AgentOS 中重新授权', exact: true })).toHaveCount(0);
  expect(requests).toBe(1);
  expect(calls).toEqual(['/runtime/reauthorize']);
  await expect(page.locator('input[type=password]')).toHaveCount(0);
});

test('expired reauthorization request is restored without retry and requires a new explicit request', async ({ page }) => {
  let state: 'expired' | 'awaitingShellConfirmation' = 'expired';
  const calls: string[] = [];
  await page.route('**/apps/androidtone/api/**', async route => {
    const path = new URL(route.request().url()).pathname.split('/api')[1];
    if (route.request().method() === 'POST') {
      calls.push(path);
      expect(path).toBe('/runtime/reauthorize');
      expect(route.request().postData() || '').toBe('');
      state = 'awaitingShellConfirmation';
      await route.fulfill({ json: { requestId: 'reauth-retry', state } });
      return;
    }
    await route.fulfill({ json: path === '/status' ? { provider: {
      state: 'authorization-denied', installed: true, initialized: true, deploymentKind: 'recovery',
      deploymentTask: { taskId: 'original-denied-task', state: 'denied' },
      reauthorization: { requestId: state === 'expired' ? 'reauth-expired' : 'reauth-retry', deniedTaskId: 'original-denied-task', state,
        canReauthorize: state === 'expired', canRetryRequest: false },
    } } : path === '/apps' ? { apps: [] } : { installTasks: [] } });
  });

  await page.goto('/apps/androidtone/');
  await expect(page.getByText('重新授权已过期', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: '在 AgentOS 中重新授权', exact: true })).toBeVisible();
  await page.getByRole('button', { name: '刷新', exact: true }).click();
  await expect(page.getByRole('button', { name: '刷新', exact: true })).toBeEnabled();
  expect(calls).toEqual([]);

  await page.getByRole('button', { name: '在 AgentOS 中重新授权', exact: true }).click();
  await expect(page.getByText('等待重新授权确认', { exact: true })).toBeVisible();
  expect(calls).toEqual(['/runtime/reauthorize']);
  await expect(page.locator('input[type=password]')).toHaveCount(0);
});

for (const [state, title] of [
  ['validating', 'AgentOS 正在重新检查系统条件'],
  ['denied', '重新授权已拒绝'],
  ['expired', '重新授权已过期'],
  ['revalidationFailed', '系统条件重新检查未通过'],
] as const) {
  test('reauthorization ' + state + ' is visible and does not submit another request', async ({ page }) => {
    let posts = 0;
    await page.route('**/apps/androidtone/api/**', route => {
      const path = new URL(route.request().url()).pathname.split('/api')[1];
      if (route.request().method() === 'POST') posts++;
      return route.fulfill({ json: path === '/status' ? { provider: {
        state: 'authorization-denied', installed: true, initialized: true, deploymentKind: 'recovery',
        deploymentTask: { taskId: 'denied-task', state: 'denied' },
        reauthorization: {
          requestId: 'reauth-visible', deniedTaskId: 'denied-task', state,
          canReauthorize: state !== 'validating',
          error: state === 'revalidationFailed' ? { code: 'reauthorization-preflight-blocked', message: 'systemd.service.control: unit-not-allowlisted' } : undefined,
        },
      } } : path === '/apps' ? { apps: [] } : { installTasks: [] } });
    });
    await page.goto('/apps/androidtone/');
    await expect(page.getByText(title, { exact: true })).toBeVisible();
    if (state === 'validating') {
      await expect(page.getByRole('button', { name: '在 AgentOS 中重新授权', exact: true })).toHaveCount(0);
    } else {
      await expect(page.getByRole('button', { name: '在 AgentOS 中重新授权', exact: true })).toBeVisible();
    }
    if (state === 'revalidationFailed') {
      await expect(page.getByText('systemd.service.control: unit-not-allowlisted', { exact: true }).first()).toBeVisible();
      await page.getByText('查看连接详情', { exact: true }).click();
      await expect(page.getByText('reauthorization-preflight-blocked', { exact: true }).first()).toBeVisible();
      await expect(page.locator('pre')).toHaveCount(0);
    }
    expect(posts).toBe(0);
  });
}

test('an incompatible bundled image requests only the fixed image update and shows the CPU diagnosis', async ({ page }) => {
  let updateRequested = false;
  const calls: string[] = [];
  const diagnosis = '当前 vendor 镜像含 ELF32 ARM boringssl_self_test32；此 VM 只提供 64-bit op-mode，Android init 因 Exec format error 重启。';
  await page.route('**/apps/androidtone/api/**', async route => {
    const path = new URL(route.request().url()).pathname.split('/api')[1];
    if (route.request().method() === 'POST') {
      calls.push(path);
      expect(path).toBe('/runtime/install');
      updateRequested = true;
      await route.fulfill({ json: { message: 'submitted' } });
      return;
    }
    await route.fulfill({ json: path === '/status' ? { provider: updateRequested ? {
      installed: true, initialized: true, available: false, state: 'awaiting-authorization', canInstall: false,
      deploymentTask: { taskId: 'image-update-task', state: 'awaitingAuthorization' }, message: diagnosis,
    } : {
      installed: true, initialized: true, available: true, state: 'image-incompatible', canInstall: true,
      message: diagnosis,
    } } : path === '/apps' ? { apps: [] } : { installTasks: [] } });
  });
  await page.goto('/apps/androidtone/');
  await expect(page.getByText('等待系统授权', { exact: true })).toBeVisible();
  expect(calls).toEqual(['/runtime/install']);
  await page.getByText('查看连接详情', { exact: true }).click();
  await expect(page.getByText(diagnosis, { exact: true })).toBeVisible();
  await page.getByRole('button', { name: '刷新', exact: true }).click();
  await expect(page.getByRole('button', { name: '刷新', exact: true })).toBeEnabled();
  expect(calls).toEqual(['/runtime/install']);
  await expect(page.locator('input[type=password]')).toHaveCount(0);
});

test('image replacement waits for an owned session to stop, then submits one install request', async ({ page }) => {
  let stopped = false;
  let updateSubmitted = false;
  const calls: string[] = [];
  await page.route('**/apps/androidtone/api/**', async route => {
    const path = new URL(route.request().url()).pathname.split('/api')[1];
    if (route.request().method() === 'POST') {
      calls.push(path);
      if (path === '/runtime/stop') stopped = true;
      if (path === '/runtime/install') {
        updateSubmitted = true;
        await route.fulfill({ json: { message: 'submitted' } });
        return;
      }
    }
    await route.fulfill({ json: path === '/status' ? { provider: updateSubmitted ? {
      installed: true, initialized: true, available: false, state: 'awaiting-authorization', canInstall: false,
      deploymentTask: { taskId: 'image-update-task', state: 'awaitingAuthorization' },
    } : {
      installed: true, initialized: true, available: true, state: 'image-incompatible', canInstall: stopped, canStop: !stopped,
      message: '请先停止 Android 环境，再执行镜像更新。',
    } } : path === '/apps' ? { apps: [] } : { installTasks: [] } });
  });
  await page.goto('/apps/androidtone/');
  await expect(page.getByRole('button', { name: '停止 Android 运行环境', exact: true })).toBeVisible();
  await expect(page.getByText(/请先点击“停止 Android 运行环境”.*AgentOS Shell 显示授权/)).toBeVisible();
  await expect(page.getByRole('button', { name: '更新 Android 镜像', exact: true })).toHaveCount(0);
  await expect(page.getByRole('button', { name: '检查系统状态', exact: true })).toHaveCount(0);
  expect(calls).toEqual([]);
  await page.getByRole('button', { name: '停止 Android 运行环境', exact: true }).click();
  await expect(page.getByText('等待系统授权', { exact: true })).toBeVisible();
  expect(calls).toEqual(['/runtime/stop', '/runtime/install']);
});

test('a refreshed image requests the fixed Waydroid recovery operation, not another package install', async ({ page }) => {
  let requested = false;
  const calls: string[] = [];
  await page.route('**/apps/androidtone/api/**', async route => {
    const path = new URL(route.request().url()).pathname.split('/api')[1];
    if (route.request().method() === 'POST') {
      calls.push(path);
      expect(path).toBe('/runtime/recover');
      requested = true;
    }
    await route.fulfill({ json: path === '/status' ? { provider: requested ? {
      installed: true, initialized: true, available: false, state: 'awaiting-authorization', canRecover: false,
      deploymentKind: 'recovery', deploymentTask: { taskId: 'recovery-task', state: 'awaitingAuthorization' },
    } : {
      installed: true, initialized: true, available: true, state: 'image-restart-required', canRecover: true,
      canInstall: false, deploymentKind: 'recovery',
      message: '新镜像已安装；AgentOS 需要授权重启 Waydroid 容器服务。',
    } } : path === '/apps' ? { apps: [] } : { installTasks: [] } });
  });
  await page.goto('/apps/androidtone/');
  await expect(page.getByText('等待系统授权', { exact: true })).toBeVisible();
  expect(calls).toEqual(['/runtime/recover']);
  await page.getByRole('button', { name: '刷新', exact: true }).click();
  await expect(page.getByRole('button', { name: '刷新', exact: true })).toBeEnabled();
  expect(calls).toEqual(['/runtime/recover']);
});
