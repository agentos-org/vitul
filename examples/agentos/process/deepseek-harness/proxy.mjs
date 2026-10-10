import { createHash } from 'node:crypto'
import { existsSync, readFileSync } from 'node:fs'
import { createServer, request as httpRequest } from 'node:http'
import { connect as netConnect } from 'node:net'

const listenPort = Number(process.env.PORT)
const upstreamPort = Number(process.env.DSH_UPSTREAM_PORT)
const tokenPath = process.env.DSH_LAUNCH_TOKEN_FILE
// WebShell strips browser Cookie headers before they reach process apps.
const authSessions = new Map()

if (!Number.isInteger(listenPort) || listenPort < 1 || listenPort > 65535
  || !Number.isInteger(upstreamPort) || upstreamPort < 1 || upstreamPort > 65535
  || typeof tokenPath !== 'string' || tokenPath === '') {
  throw new Error('DSH proxy requires valid PORT, DSH_UPSTREAM_PORT, and DSH_LAUNCH_TOKEN_FILE values')
}

function launchToken() {
  if (!existsSync(tokenPath)) return undefined
  const token = readFileSync(tokenPath, 'utf8').trim()
  return /^[A-Za-z0-9_-]+$/.test(token) ? token : undefined
}

function hasSessionCookie(request) {
  const host = request.headers.host
  if (typeof host !== 'string') return false
  let authority
  try {
    authority = new URL(`http://${host}`).host
  } catch {
    return false
  }
  const cookieName = `dsh-auth-${createHash('sha256').update(authority).digest('base64url')}`
  return (request.headers.cookie ?? '').split(';').some(part => part.trimStart().startsWith(`${cookieName}=`))
}

function upstreamCookieHeader(request, upstreamHost) {
  const sessionCookie = authSessions.get(upstreamHost)
  if (sessionCookie === undefined) return request.headers.cookie
  const cookieName = sessionCookie.split('=', 1)[0]
  const cookies = (request.headers.cookie ?? '').split(';')
    .map(value => value.trim())
    .filter(value => value !== '' && !value.startsWith(`${cookieName}=`))
  cookies.push(sessionCookie)
  return cookies.join('; ')
}

function upstreamRequestHeaders(request, upstreamHost) {
  const headers = { ...request.headers, host: upstreamHost }
  // The workspace HTML is tiny; request it uncompressed so the AgentOS marker
  // can be injected without touching DSH's JavaScript or asset responses.
  if (request.method === 'GET' && new URL(request.url ?? '/', 'http://agentos.invalid').pathname === '/') {
    headers['accept-encoding'] = 'identity'
  }
  const cookie = upstreamCookieHeader(request, upstreamHost)
  if (cookie === undefined || cookie === '') delete headers.cookie
  else headers.cookie = cookie
  return headers
}

function normalizePluginComboUrl(rawUrl) {
  let url
  try {
    url = new URL(rawUrl, 'http://agentos.invalid')
  } catch {
    return rawUrl
  }
  // DSH addresses a combined plugin script as `/plugins/??<resources>&rev=…`.
  // Some app proxies normalize the leading `??` to one query delimiter while
  // forwarding. Restore the second delimiter so the request still matches the
  // exact combo URL registered by DSH.
  const pluginPath = '/plugins/'
  const pluginPathIndex = url.pathname.lastIndexOf(pluginPath)
  if (pluginPathIndex < 0 || url.pathname.slice(pluginPathIndex) !== pluginPath) return rawUrl

  let pathname = url.pathname
  // Some WebShell builds forward the app mount prefix to the process instead
  // of stripping it. DSH registers combo routes at its root, so remove that
  // prefix for this endpoint when it is still present.
  const mountPrefix = pathname.slice(0, pluginPathIndex)
  if (/^\/apps\/ai\.deepseek\.harness$/iu.test(mountPrefix)) pathname = pluginPath

  // Some app proxies normalize the initial `??` into one query delimiter.
  // URLSearchParams can also encode the resource's second `?` into the first
  // key and append `=` for its empty value. Restore the exact DSH combo key.
  let search = url.search
  if (search.startsWith('?') && !search.startsWith('??')) {
    const fields = search.slice(1).split('&')
    const [encodedResource, ...rest] = fields
    let resource = encodedResource ?? ''
    if (/^%3f/iu.test(resource)) {
      try {
        resource = decodeURIComponent(resource.replaceAll('+', ' '))
      } catch {
        return rawUrl
      }
      if (!resource.startsWith('?@deepseek-ai/') || !resource.includes('/client.js')) return rawUrl
      resource = resource.slice(1).replace(/=$/u, '')
      search = `??${resource}${rest.length > 0 ? `&${rest.join('&')}` : ''}`
    } else {
      search = `?${search}`
    }
  }
  // WebShell app mounts append their instance-routing marker to every process
  // request. DSH keys combo bundles by the complete pathname + query, so that
  // shell-only field makes an otherwise current resource look unknown (404).
  // Keep the restored `??` resource key byte-for-byte and remove only that field.
  if (search.startsWith('??')) {
    const fields = search.slice(2).split('&')
    const forwarded = fields.filter(field => {
      const rawName = field.split('=', 1)[0] ?? ''
      let name = rawName
      try {
        name = decodeURIComponent(rawName.replaceAll('+', ' '))
      } catch {
        // Leave malformed application query names unchanged.
      }
      return name !== 'agentosInstanceId'
    })
    if (forwarded.length !== fields.length) {
      search = forwarded.length === 0 ? '' : `??${forwarded.join('&')}`
    }
  }
  if (pathname === url.pathname && search === url.search) return rawUrl
  return `${pathname}${search}${url.hash}`
}

function requestPath(request, hasProxySession = false) {
  const rawUrl = normalizePluginComboUrl(request.url ?? '/')
  if (request.method !== 'GET' || hasProxySession || hasSessionCookie(request)) return rawUrl
  let pathname
  try {
    pathname = new URL(rawUrl, 'http://agentos.invalid').pathname
  } catch {
    return rawUrl
  }
  if (pathname !== '/') return rawUrl
  const token = launchToken()
  if (token === undefined) return rawUrl
  const url = new URL(rawUrl, 'http://agentos.invalid')
  if (!url.searchParams.has('token')) url.searchParams.set('token', token)
  return `${url.pathname}${url.search}${url.hash}`
}

function responseCookies(value) {
  if (Array.isArray(value)) return value
  return typeof value === 'string' ? [value] : []
}

function agentOSRemoteSettingsEnabled() {
  return typeof process.env.AGENTOS_APP_PUBLIC_URL === 'string'
    && process.env.AGENTOS_APP_PUBLIC_URL.trim() !== ''
}

function sendUpstreamResponse(request, response, upstreamResponse, extraHeaders = {}) {
  const headers = { ...upstreamResponse.headers, ...extraHeaders }
  const requestUrl = new URL(request.url ?? '/', 'http://agentos.invalid')
  const isWorkspaceHtml = agentOSRemoteSettingsEnabled()
    && request.method === 'GET'
    && requestUrl.pathname === '/'
    && String(headers['content-type'] ?? '').toLowerCase().includes('text/html')
  if (!isWorkspaceHtml) {
    response.writeHead(upstreamResponse.statusCode ?? 502, headers)
    upstreamResponse.pipe(response)
    return
  }

  const chunks = []
  upstreamResponse.on('data', chunk => chunks.push(chunk))
  upstreamResponse.on('end', () => {
    let html = Buffer.concat(chunks).toString('utf8')
    if (!html.includes('__DSH_AGENTOS_REMOTE_SETTINGS__')) {
      const bootstrap = '<script src="agentos-runtime-compat.js"></script>'
      html = /<head\b[^>]*>/iu.test(html)
        ? html.replace(/<head\b[^>]*>/iu, match => `${match}${bootstrap}`)
        : `${bootstrap}${html}`
    }
    delete headers['content-encoding']
    delete headers['content-length']
    delete headers['content-md5']
    delete headers.etag
    delete headers['transfer-encoding']
    headers['content-length'] = Buffer.byteLength(html)
    response.writeHead(upstreamResponse.statusCode ?? 502, headers)
    response.end(html)
  })
}

function relay(request, response) {
  const requestUrl = new URL(request.url ?? '/', 'http://agentos.invalid')
  const initialWorkspaceRequest = request.method === 'GET'
    && requestUrl.pathname === '/'
    && !hasSessionCookie(request)
  const upstreamHost = request.headers.host ?? `127.0.0.1:${upstreamPort}`
  const upstreamPath = requestPath(request, authSessions.has(upstreamHost))
  const upstream = httpRequest({
    host: '127.0.0.1',
    port: upstreamPort,
    method: request.method,
    path: upstreamPath,
    headers: upstreamRequestHeaders(request, upstreamHost),
  }, upstreamResponse => {
    const loginCookies = responseCookies(upstreamResponse.headers['set-cookie'])
    const sessionCookie = loginCookies.find(cookie => cookie.startsWith('dsh-auth-'))
    if (sessionCookie !== undefined) {
      authSessions.set(upstreamHost, sessionCookie.split(';', 1)[0])
    }
    if (initialWorkspaceRequest
      && upstreamResponse.statusCode === 303
      && upstreamResponse.headers.location === './'
      && sessionCookie !== undefined) {
      // AgentOS Workspace surfaces can expose this first-load auth exchange as
      // a 303 instead of following it. Complete the same-origin cookie exchange
      // here so the first page load is a normal 200 response.
      upstreamResponse.resume()
      const authenticated = httpRequest({
        host: '127.0.0.1',
        port: upstreamPort,
        method: 'GET',
        path: '/',
        headers: upstreamRequestHeaders(request, upstreamHost),
      }, authenticatedResponse => {
        const headers = { ...authenticatedResponse.headers }
        const retryCookies = responseCookies(headers['set-cookie'])
        const updatedSessionCookie = retryCookies.find(cookie => cookie.startsWith('dsh-auth-'))
        if (updatedSessionCookie !== undefined) {
          authSessions.set(upstreamHost, updatedSessionCookie.split(';', 1)[0])
        }
        headers['set-cookie'] = [...loginCookies, ...retryCookies]
        if (upstreamResponse.headers['cache-control'] !== undefined) {
          headers['cache-control'] = upstreamResponse.headers['cache-control']
        }
        if (upstreamResponse.headers['referrer-policy'] !== undefined) {
          headers['referrer-policy'] = upstreamResponse.headers['referrer-policy']
        }
        sendUpstreamResponse(request, response, authenticatedResponse, headers)
      })
      authenticated.on('error', () => {
        if (!response.headersSent) response.writeHead(502, { 'content-type': 'text/plain; charset=utf-8' })
        response.end('DeepSeek Harness is starting or unavailable.\n')
      })
      authenticated.end()
      return
    }
    sendUpstreamResponse(request, response, upstreamResponse)
  })
  upstream.on('error', () => {
    if (!response.headersSent) response.writeHead(502, { 'content-type': 'text/plain; charset=utf-8' })
    response.end('DeepSeek Harness is starting or unavailable.\n')
  })
  request.pipe(upstream)
}

const server = createServer((request, response) => {
  if (request.url === '/healthz') {
    const upstream = httpRequest({ host: '127.0.0.1', port: upstreamPort, path: '/' }, upstreamResponse => {
      upstreamResponse.resume()
      response.writeHead(launchToken() === undefined ? 503 : 200, { 'content-type': 'text/plain; charset=utf-8' })
      response.end(launchToken() === undefined ? 'starting\n' : 'ok\n')
    })
    upstream.on('error', () => {
      response.writeHead(503, { 'content-type': 'text/plain; charset=utf-8' })
      response.end('starting\n')
    })
    upstream.end()
    return
  }
  const requestPathname = new URL(request.url ?? '/', 'http://agentos.invalid').pathname
  if (agentOSRemoteSettingsEnabled() && requestPathname.endsWith('/agentos-runtime-compat.js')) {
    response.writeHead(200, {
      'cache-control': 'no-store',
      'content-type': 'text/javascript; charset=utf-8',
    })
    response.end('globalThis.__DSH_AGENTOS_REMOTE_SETTINGS__=true;\n')
    return
  }
  relay(request, response)
})

server.on('upgrade', (request, client, head) => {
  const upstream = netConnect(upstreamPort, '127.0.0.1', () => {
    const upstreamHost = request.headers.host ?? `127.0.0.1:${upstreamPort}`
    const headers = { ...request.headers }
    const cookie = upstreamCookieHeader(request, upstreamHost)
    if (cookie === undefined || cookie === '') delete headers.cookie
    else headers.cookie = cookie
    const lines = [
      `${request.method ?? 'GET'} ${request.url ?? '/'} HTTP/${request.httpVersion}`,
      ...Object.entries(headers).flatMap(([name, value]) => {
        const values = Array.isArray(value) ? value : [value]
        return values.map(item => `${name}: ${item}`)
      }),
      '',
      '',
    ]
    upstream.write(lines.join('\r\n'))
    if (head.length > 0) upstream.write(head)
    client.pipe(upstream).pipe(client)
  })
  upstream.on('error', () => client.destroy())
  client.on('error', () => upstream.destroy())
})

server.listen(listenPort, '0.0.0.0', () => {
  process.stdout.write(`DeepSeek Harness AgentOS proxy listening on ${listenPort}\n`)
})
