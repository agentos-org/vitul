import { invoke, isTauri } from '@tauri-apps/api/core'

type JsonRpcResponse<T> = {
  jsonrpc: string
  id: string
  result?: T
  error?: { code: number; message: string }
}

export class OpenToolError extends Error {
  constructor(
    message: string,
    readonly code?: number
  ) {
    super(message)
    this.name = 'OpenToolError'
  }
}

let endpoint: Promise<string> | undefined

async function getEndpoint(): Promise<string> {
  endpoint ??= (async () => {
    if (await isTauri()) {
      return invoke<string>('opentool_endpoint')
    }

    // Vite proxies this path during web development. AgentOS routes it to the
    // app's backend through the same per-instance origin.
    return new URL('/', window.location.href).toString()
  })()
  return endpoint
}

export async function callOpenTool<T>(
  method: string,
  params: Record<string, unknown>
): Promise<T> {
  const id = crypto.randomUUID()
  const url = new URL('opentool/call', await getEndpoint())
  const response = await fetch(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0.0', method, params, id })
  })

  if (!response.ok) {
    throw new OpenToolError(`OpenTool request failed: HTTP ${response.status}`)
  }

  const body = (await response.json()) as JsonRpcResponse<T>
  if (body.jsonrpc !== '2.0.0') {
    throw new OpenToolError('OpenTool response has an unsupported JSON-RPC version.')
  }
  if (body.id !== id) {
    throw new OpenToolError('OpenTool response ID does not match the request.')
  }
  if (body.error) throw new OpenToolError(body.error.message, body.error.code)
  if (!('result' in body)) {
    throw new OpenToolError('OpenTool response is missing its result.')
  }

  return body.result as T
}
