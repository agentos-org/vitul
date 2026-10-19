import type { Provider } from './api';
import type { Translate } from './i18n';

export interface DeploymentDiagnostic {
  code?: string;
  reasonCode?: string;
  package?: string;
  installedVersion?: string;
  requestedVersion?: string;
  message?: string;
  field?: string;
  value?: unknown;
  [key: string]: unknown;
}

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export function diagnosticEntries(details: unknown): DeploymentDiagnostic[] {
  if (Array.isArray(details)) return details.filter(record) as DeploymentDiagnostic[];
  if (!record(details)) return [];
  for (const key of ['details', 'diagnostics', 'items']) {
    const nested = details[key];
    if (Array.isArray(nested)) return nested.filter(record) as DeploymentDiagnostic[];
    if (record(nested)) return diagnosticEntries(nested);
  }
  if (['code', 'reasonCode', 'package', 'installedVersion', 'requestedVersion', 'systemCode'].some(key => key in details)) {
    return [details as DeploymentDiagnostic];
  }

  // Render small diagnostic maps as labeled rows instead of dumping raw JSON.
  return Object.entries(details).slice(0, 24).flatMap(([field, value]) => {
    if (value === null || ['string', 'number', 'boolean'].includes(typeof value)) return [{ field, value }];
    if (record(value)) {
      return Object.entries(value).slice(0, 24).map(([childField, childValue]) => ({
        field: `${field}.${childField}`,
        value: childValue,
      }));
    }
    return [];
  });
}

export function deploymentDiagnostics(provider?: Provider): DeploymentDiagnostic[] {
  const error = provider?.deploymentTask?.error;
  return diagnosticEntries(error?.details ?? error?.data);
}

export function preflightSummary(provider: Provider | undefined, t: Translate): string {
  const diagnostics = deploymentDiagnostics(provider);
  const imageConflict = diagnostics.find(item =>
    item.package === 'androidtone-waydroid-images'
      && item.code === 'package-version-conflict');
  if (imageConflict) {
    const sentinel = '__ANDROIDTONE_IMAGE_PACKAGE__';
    return t('镜像组件 {package} 版本冲突：宿主已安装 {installedVersion}，AndroidTone 声明版本 {requestedVersion}。AgentOS 当前通用包策略禁止升级或替换，因此任务未进入授权队列。需要 AgentOS 提供通用的 Shell 确认精确包升级能力，或先由系统维护完成版本调整，再手动重新检查。', {
      package: sentinel,
      installedVersion: String(imageConflict.installedVersion ?? t('未知')),
      requestedVersion: String(imageConflict.requestedVersion ?? t('未知')),
    }).replace(sentinel, String(imageConflict.package));
  }
  const conflicts = diagnostics.filter(item => item.code === 'package-version-conflict');
  if (conflicts.length) {
    return t('有 {count} 个宿主包版本不满足 AndroidTone 声明的兼容范围。AgentOS 当前策略禁止升级或降级，因此任务未进入授权队列。请查看受影响包，修正宿主状态后再手动重新检查。', { count: conflicts.length });
  }
  if (diagnostics.some(item => item.code === 'dependency-unsatisfied')) {
    return t('离线依赖预检查未通过，任务未进入 AgentOS 授权队列。请查看缺失或未配置的依赖，修正宿主状态后再手动重新检查。');
  }
  return t('AgentOS 离线包预检查发现版本或依赖冲突；任务未进入系统授权队列。请查看详情，修正清单或宿主状态后，再手动重新检查。');
}

export function diagnosticTitle(code: unknown, t: Translate): string {
  switch (code) {
    case 'package-version-conflict': return t('包版本冲突');
    case 'package-upgrade-disallowed': return t('AgentOS 策略禁止升级');
    case 'package-downgrade-disallowed': return t('AgentOS 策略禁止降级');
    case 'package-replacement-disallowed': return t('AgentOS 策略禁止替换');
    case 'dependency-unsatisfied': return t('依赖未满足');
    case 'insufficient-disk-space': return t('磁盘空间不足');
    case 'package-status-unavailable': return t('无法读取宿主包状态');
    default: return t('部署诊断');
  }
}

export function diagnosticReason(reason: unknown, t: Translate): string | undefined {
  switch (reason) {
    case 'package-upgrade-disallowed': return t('AgentOS 当前策略禁止自动升级已安装的系统包。');
    case 'package-downgrade-disallowed': return t('AgentOS 当前策略禁止自动降级已安装的系统包。');
    case 'package-replacement-disallowed': return t('AgentOS 当前策略禁止替换已安装的系统包。');
    case 'package-not-configured': return t('软件包已安装，但尚未完成配置。');
    case 'package-status-unavailable': return t('AgentOS 暂时无法确认软件包状态。');
    default: return undefined;
  }
}
