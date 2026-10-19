import type { Provider } from './api';
export type PreparationAction = 'install' | 'initialize' | 'start' | 'recover';
export function nextPreparation(p?: Provider): PreparationAction | null {
  if (!p || ['provider-unavailable', 'platform-dependency', 'kernel-missing', 'external-session', 'error', 'failed', 'starting', 'awaiting-authorization', 'deploying', 'runtime-ready', 'authorization-denied', 'authorization-cancelled', 'authorization-expired', 'deployment-failed', 'preflight-blocked', 'idempotency-conflict', 'recovery-failed', 'deployment-tracking-failed'].includes(p.state || '')) return null;
  if (p.state === 'recovery-required' || p.state === 'image-restart-required') return p.canRecover === true ? 'recover' : null;
  if (p.state === 'image-incompatible' || p.state === 'runtime-update-required') return p.canInstall === true ? 'install' : null;
  const installed = p.installed === true || (p.available === true && p.state !== 'runtime-missing');
  if (!installed) return p.canInstall === true ? 'install' : null;
  if (!p.initialized) return p.canInitialize === true ? 'initialize' : null;
  if (!p.running && p.state !== 'running') return p.canStart === true ? 'start' : null;
  return null;
}
export function preparationBlocked(p?: Provider): boolean {
  if (!p) return false;
  if (p.running || p.state === 'running') return false;
  if (['awaiting-authorization', 'deploying', 'runtime-ready', 'authorization-denied', 'authorization-cancelled'].includes(p.state || '')) return false;
  return !nextPreparation(p) && p.state !== 'starting';
}
