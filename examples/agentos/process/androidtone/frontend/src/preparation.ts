import type { Provider } from './api';
export type PreparationAction = 'install' | 'initialize' | 'start';
export function nextPreparation(p?: Provider): PreparationAction | null {
  if (!p || ['provider-unavailable', 'platform-dependency', 'kernel-missing', 'external-session', 'error', 'failed', 'starting'].includes(p.state || '')) return null;
  const installed = p.installed === true || (p.available === true && p.state !== 'runtime-missing');
  if (!installed) return p.canInstall === true ? 'install' : null;
  if (!p.initialized) return p.canInitialize === true ? 'initialize' : null;
  if (!p.running && p.state !== 'running') return p.canStart === true ? 'start' : null;
  return null;
}
export function preparationBlocked(p?: Provider): boolean {
  if (!p) return false;
  if (p.running || p.state === 'running') return false;
  return !nextPreparation(p) && p.state !== 'starting';
}
