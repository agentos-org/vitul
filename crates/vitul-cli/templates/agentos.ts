import { AgentOSSDK } from 'agentos-sdk-ts'

/**
 * Create a client for AgentOS Gateway APIs. The client is created only when
 * this function is called, so apps that do not use the Gateway need no setup.
 * Set VITE_AGENTOS_GATEWAY_URL in a local .env file to override the SDK default.
 */
export function createAgentOSClient(): AgentOSSDK {
  const baseUrl = import.meta.env.VITE_AGENTOS_GATEWAY_URL
  return baseUrl ? new AgentOSSDK({ baseUrl }) : new AgentOSSDK()
}
