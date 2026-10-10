#!/usr/bin/env node
'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const http = require('node:http');
const https = require('node:https');
const path = require('node:path');
const { Readable, Transform } = require('node:stream');
const { pipeline } = require('node:stream/promises');

const dataDir = path.resolve(process.env.DATA_DIR || '');
const documentsDir = path.join(dataDir, 'documents');
const port = Number(process.env.PORT || 3000);
const maxFileBytes = 256 * 1024 * 1024;
const allowedExtensions = new Set(['doc', 'docm', 'docx', 'dot', 'dotx', 'odt', 'ott', 'rtf', 'txt', 'pdf', 'xls', 'xlsm', 'xlsx', 'xltx', 'ods', 'ots', 'csv', 'ppt', 'pptm', 'pptx', 'odp', 'otp']);
const settings = {
  docsUrl: process.env.ONLYOFFICE_DOCS_URL || '',
  jwtSecret: process.env.ONLYOFFICE_JWT_SECRET || '',
};
const browserEndpointByDocumentKey = new Map();

if (!dataDir || !Number.isInteger(port) || port < 1 || port > 65535) {
  console.error('ONLYOFFICE: missing or invalid DATA_DIR or PORT.');
  process.exit(1);
}
fs.mkdirSync(documentsDir, { recursive: true });

const homePage = `<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>ONLYOFFICE Docs</title><style>
:root{font:15px system-ui,sans-serif;color:#18212f;background:#f3f5f8}*{box-sizing:border-box}body{margin:0}.bar{height:64px;background:#fff;border-bottom:1px solid #dfe4eb;display:flex;align-items:center;padding:0 28px;gap:14px}.mark{width:30px;height:34px;border-radius:5px;background:#ff6f3d;color:#fff;font-weight:700;display:grid;place-items:center}.bar strong{font-size:18px}.wrap{max-width:1000px;margin:42px auto;padding:0 24px}.intro{display:flex;align-items:end;justify-content:space-between;margin-bottom:22px}.intro h1{font-size:28px;margin:0 0 6px}.intro p{color:#637083;margin:0}.card{background:#fff;border:1px solid #e0e5ec;border-radius:10px;padding:22px;margin-bottom:18px}.button{border:0;border-radius:7px;background:#ff6f3d;color:#fff;font:inherit;font-weight:650;padding:11px 16px;cursor:pointer}.item{display:flex;align-items:center;justify-content:space-between;padding:16px 4px;border-bottom:1px solid #edf0f4}.item:last-child{border-bottom:0}.filename{font-weight:600;overflow-wrap:anywhere}.meta{color:#718096;font-size:13px;margin-top:4px}.open{color:#e55d31;text-decoration:none;font-weight:650;margin-left:16px}.empty{padding:30px 12px;text-align:center;color:#718096}.foot{font-size:12px;color:#718096;margin-top:14px}
</style></head><body><header class="bar"><span class="mark">O</span><strong>ONLYOFFICE Docs</strong></header>
<main class="wrap"><div class="intro"><div><h1>我的文档</h1><p>上传 Office 文档后即可在工作区中编辑。</p></div><label class="button">上传文档<input id="upload" type="file" hidden></label></div><div id="files" class="card"><div class="empty">正在加载文档…</div></div><p class="foot">ONLYOFFICE Docs 在 AgentOS 容器沙盒中运行；文档保存在此应用的持久化数据目录中。</p></main>
<script>
const files=document.querySelector('#files');
function esc(text){return String(text).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]))}
async function refreshFiles(){const r=await fetch('/api/files');if(!r.ok)throw new Error(await r.text());const rows=await r.json();files.replaceChildren();if(!rows.length){files.innerHTML='<div class="empty">暂无文档。可上传 DOCX、XLSX、PPTX、PDF 或其他支持的格式。</div>';return}for(const row of rows){const item=document.createElement('div');item.className='item';item.innerHTML='<div><div class="filename">'+esc(row.name)+'</div><div class="meta">'+esc(row.size)+' · '+esc(row.modified)+'</div></div><a class="open" href="/edit/'+encodeURIComponent(row.name)+'">打开</a>';files.append(item)}}
document.querySelector('#upload').addEventListener('change',async event=>{const file=event.target.files[0];if(!file)return;const r=await fetch('/api/files/'+encodeURIComponent(file.name),{method:'PUT',body:file});if(!r.ok){alert((await r.text())||'上传失败');return}event.target.value='';refreshFiles()});
refreshFiles().catch(error=>{files.textContent='无法加载文档列表：'+error.message});
</script></body></html>`;

function send(res, status, body, type = 'application/json; charset=utf-8') {
  res.writeHead(status, { 'content-type': type, 'x-content-type-options': 'nosniff', 'referrer-policy': 'same-origin' });
  res.end(body);
}

function json(res, status, value) {
  send(res, status, JSON.stringify(value));
}

function safeFilename(value) {
  const name = String(value || '');
  if (!name || name === '.' || name === '..' || name !== path.basename(name) || /[\0/\\]/.test(name)) return null;
  const ext = path.extname(name).slice(1).toLowerCase();
  if (!allowedExtensions.has(ext)) return null;
  return name;
}

function filePath(name) {
  return path.join(documentsDir, name);
}

function signToken(payload) {
  const header = Buffer.from(JSON.stringify({ alg: 'HS256', typ: 'JWT' })).toString('base64url');
  const body = Buffer.from(JSON.stringify(payload)).toString('base64url');
  const signature = crypto.createHmac('sha256', settings.jwtSecret).update(header + '.' + body).digest('base64url');
  return header + '.' + body + '.' + signature;
}

function verifyToken(token) {
  if (typeof token !== 'string') return false;
  const parts = token.split('.');
  if (parts.length !== 3) return false;
  const expected = crypto.createHmac('sha256', settings.jwtSecret).update(parts[0] + '.' + parts[1]).digest();
  let actual;
  try { actual = Buffer.from(parts[2], 'base64url'); } catch { return false; }
  return actual.length === expected.length && crypto.timingSafeEqual(actual, expected);
}

function editorType(ext) {
  if (['xls', 'xlsm', 'xlsx', 'xltx', 'ods', 'ots', 'csv'].includes(ext)) return 'cell';
  if (['ppt', 'pptm', 'pptx', 'odp', 'otp'].includes(ext)) return 'slide';
  return 'word';
}

async function listFiles(res) {
  const entries = await fs.promises.readdir(documentsDir, { withFileTypes: true });
  const files = await Promise.all(entries.filter((entry) => entry.isFile()).map(async (entry) => {
    const stat = await fs.promises.stat(filePath(entry.name));
    return { name: entry.name, size: Math.max(1, Math.round(stat.size / 1024)) + ' KB', modified: stat.mtime.toLocaleString() };
  }));
  files.sort((a, b) => a.name.localeCompare(b.name));
  json(res, 200, files);
}

async function readJson(req, maximum = 1024 * 1024) {
  let body = '';
  for await (const chunk of req) {
    body += chunk.toString('utf8');
    if (body.length > maximum) throw new Error('Request body is too large.');
  }
  return JSON.parse(body || '{}');
}

function parseBrowserEndpoint(value) {
  if (!value) return null;
  try {
    const url = new URL(value);
    if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password || url.pathname !== '/' || url.search || url.hash) return null;
    return { host: url.host, protocol: url.protocol.slice(0, -1) };
  } catch {
    return null;
  }
}

function browserEndpointForRequest(req) {
  const requestUrl = new URL(req.url, 'http://' + (req.headers.host || 'localhost'));
  const match = requestUrl.pathname.match(/\/(?:doc|cache\/files\/data)\/([a-f\d]{24})(?:\/|$)/i);
  return (match && browserEndpointByDocumentKey.get(match[1])) || {
    host: req.headers.host || '',
    protocol: req.headers['x-forwarded-proto'] || 'http',
  };
}

async function editorConfig(req, res, name) {
  if (!settings.docsUrl || !settings.jwtSecret) return json(res, 503, { error: 'ONLYOFFICE Docs 尚未就绪，请稍后重试。' });
  try {
    const stat = await fs.promises.stat(filePath(name));
    if (!stat.isFile()) return json(res, 404, { error: 'Document not found.' });
    const ext = path.extname(name).slice(1).toLowerCase();
    const browserEndpoint = parseBrowserEndpoint(new URL(req.url, 'http://' + (req.headers.host || 'localhost')).searchParams.get('publicOrigin'));
    // Docs and the workspace adapter run in the same AgentOS app container.
    // Loopback avoids relying on a bridge IP that may be filtered or unroutable
    // from the document service's network namespace.
    const callbackBase = 'http://127.0.0.1:' + port;
    const encoded = encodeURIComponent(name);
    const documentKey = crypto.createHash('sha256').update(name + ':' + stat.size + ':' + stat.mtimeMs).digest('hex').slice(0, 24);
    if (browserEndpoint) {
      browserEndpointByDocumentKey.set(documentKey, browserEndpoint);
      if (browserEndpointByDocumentKey.size > 256) browserEndpointByDocumentKey.delete(browserEndpointByDocumentKey.keys().next().value);
    }
    const config = {
      documentType: editorType(ext),
      document: {
        fileType: ext,
        key: documentKey,
        title: name,
        url: callbackBase + '/api/download/' + encoded,
      },
      editorConfig: {
        mode: 'edit',
        callbackUrl: callbackBase + '/api/callback/' + encoded,
        lang: 'zh-CN',
        user: { id: 'agentos-' + (process.getuid?.() ?? 'user'), name: 'AgentOS user' },
      },
      height: '100%',
      width: '100%',
      type: 'desktop',
    };
    config.token = signToken(config);
    json(res, 200, config);
  } catch {
    json(res, 404, { error: 'Document not found.' });
  }
}

async function uploadFile(req, res, name) {
  const destination = filePath(name);
  const temporary = destination + '.upload-' + process.pid;
  let bytes = 0;
  const limit = new Transform({
    transform(chunk, _encoding, callback) {
      bytes += chunk.length;
      if (bytes > maxFileBytes) callback(new Error('File exceeds the 256 MB upload limit.'));
      else callback(null, chunk);
    },
  });
  try {
    await pipeline(req, limit, fs.createWriteStream(temporary, { mode: 0o600 }));
    await fs.promises.rename(temporary, destination);
    json(res, 201, { name });
  } catch (error) {
    await fs.promises.rm(temporary, { force: true });
    send(res, error.message.includes('256 MB') ? 413 : 400, error.message, 'text/plain; charset=utf-8');
  }
}

async function serveDocument(req, res, name) {
  const file = filePath(name);
  try {
    const stat = await fs.promises.stat(file);
    if (!stat.isFile()) return send(res, 404, 'Not found', 'text/plain; charset=utf-8');
    const encodedName = encodeURIComponent(name).replace(/['()]/g, escape);
    res.writeHead(200, {
      'content-type': 'application/octet-stream',
      'content-length': stat.size,
      'content-disposition': "attachment; filename*=UTF-8''" + encodedName,
      'x-content-type-options': 'nosniff',
    });
    console.log('ONLYOFFICE document download served; bytes=' + stat.size + '; remote=' + (req.socket.remoteAddress || 'unknown'));
    if (req.method === 'HEAD') return res.end();
    fs.createReadStream(file).pipe(res);
  } catch {
    console.error('ONLYOFFICE document download failed: requested file is unavailable');
    send(res, 404, 'Not found', 'text/plain; charset=utf-8');
  }
}

async function callback(req, res, name) {
  let payload;
  try { payload = await readJson(req); } catch { return json(res, 400, { error: 1 }); }
  const token = payload.token || String(req.headers.authorization || '').replace(/^Bearer\s+/i, '');
  if (!verifyToken(token)) return json(res, 403, { error: 1 });
  if (payload.status !== 2 && payload.status !== 6) return json(res, 200, { error: 0 });
  if (typeof payload.url !== 'string') return json(res, 400, { error: 1 });

  const temporary = filePath(name) + '.save-' + process.pid;
  try {
    const response = await fetch(payload.url, { signal: AbortSignal.timeout(120000) });
    if (!response.ok || !response.body) throw new Error('Docs returned HTTP ' + response.status);
    let bytes = 0;
    const limit = new Transform({
      transform(chunk, _encoding, done) {
        bytes += chunk.length;
        if (bytes > maxFileBytes) done(new Error('Saved document exceeds the 256 MB limit.'));
        else done(null, chunk);
      },
    });
    await pipeline(Readable.fromWeb(response.body), limit, fs.createWriteStream(temporary, { mode: 0o600 }));
    await fs.promises.rename(temporary, filePath(name));
    json(res, 200, { error: 0 });
  } catch (error) {
    await fs.promises.rm(temporary, { force: true });
    console.error('ONLYOFFICE save callback failed: ' + error.message);
    json(res, 500, { error: 1 });
  }
}

function docsTarget(req) {
  const base = new URL(settings.docsUrl);
  const target = new URL(req.url, base);
  return { base, target, transport: target.protocol === 'https:' ? https : http };
}

function proxy(req, res) {
  if (!settings.docsUrl) return send(res, 503, 'Connect an ONLYOFFICE Docs service in the app settings.', 'text/plain; charset=utf-8');
  let details;
  try { details = docsTarget(req); } catch { return send(res, 502, 'Invalid ONLYOFFICE Docs URL.', 'text/plain; charset=utf-8'); }
  const browserEndpoint = browserEndpointForRequest(req);
  const headers = { ...req.headers, host: details.target.host };
  headers['x-forwarded-host'] = browserEndpoint.host;
  headers['x-forwarded-proto'] = browserEndpoint.protocol;
  const upstream = details.transport.request({
    hostname: details.target.hostname,
    port: details.target.port || (details.target.protocol === 'https:' ? 443 : 80),
    method: req.method,
    path: details.target.pathname + details.target.search,
    headers,
  }, (upstreamResponse) => {
    const responseHeaders = { ...upstreamResponse.headers };
    if (responseHeaders.location) {
      try {
        const location = new URL(responseHeaders.location, details.base);
        if (location.origin === details.base.origin) responseHeaders.location = browserEndpoint.protocol + '://' + browserEndpoint.host + location.pathname + location.search;
      } catch {}
    }
    res.writeHead(upstreamResponse.statusCode || 502, responseHeaders);
    upstreamResponse.pipe(res);
  });
  upstream.on('error', (error) => {
    console.error('ONLYOFFICE proxy failed: ' + error.message);
    if (!res.headersSent) send(res, 502, 'ONLYOFFICE Docs is unavailable.', 'text/plain; charset=utf-8');
    else res.destroy(error);
  });
  req.pipe(upstream);
}

function proxyUpgrade(req, socket, head) {
  if (!settings.docsUrl) return socket.destroy();
  let details;
  try { details = docsTarget(req); } catch { return socket.destroy(); }
  const browserEndpoint = browserEndpointForRequest(req);
  const headers = { ...req.headers, host: details.target.host };
  // Keep the public app origin when forwarding the editor's WebSocket. Docs
  // uses X-Forwarded-Host to build browser-facing URLs; without it, it emits
  // links to the container-only Docs listener (127.0.0.1:8080).
  headers['x-forwarded-host'] = browserEndpoint.host;
  headers['x-forwarded-proto'] = browserEndpoint.protocol;
  const upstream = details.transport.request({
    hostname: details.target.hostname,
    port: details.target.port || (details.target.protocol === 'https:' ? 443 : 80),
    method: req.method,
    path: details.target.pathname + details.target.search,
    headers,
  });
  upstream.on('upgrade', (response, upstreamSocket, upstreamHead) => {
    let header = 'HTTP/' + req.httpVersion + ' ' + response.statusCode + ' ' + response.statusMessage + '\r\n';
    for (let index = 0; index < response.rawHeaders.length; index += 2) {
      header += response.rawHeaders[index] + ': ' + response.rawHeaders[index + 1] + '\r\n';
    }
    socket.write(header + '\r\n');
    if (head.length) upstreamSocket.write(head);
    if (upstreamHead.length) socket.write(upstreamHead);
    upstreamSocket.pipe(socket);
    socket.pipe(upstreamSocket);
  });
  upstream.on('response', (response) => {
    socket.end('HTTP/' + req.httpVersion + ' ' + response.statusCode + ' ' + response.statusMessage + '\r\nConnection: close\r\n\r\n');
  });
  upstream.on('error', () => socket.destroy());
  socket.on('error', () => upstream.destroy());
  upstream.end();
}

async function route(req, res) {
  const url = new URL(req.url, 'http://localhost');
  const pathname = url.pathname;
  if (req.method === 'GET' && pathname === '/health') {
    try {
      const response = await fetch(new URL('/healthcheck', settings.docsUrl), { signal: AbortSignal.timeout(1500) });
      if (response.ok) return send(res, 200, 'ok', 'text/plain; charset=utf-8');
    } catch {}
    return send(res, 503, 'ONLYOFFICE Docs is starting.', 'text/plain; charset=utf-8');
  }
  if (req.method === 'GET' && pathname === '/') return send(res, 200, homePage, 'text/html; charset=utf-8');
  if (req.method === 'GET' && pathname === '/api/files') return listFiles(res);

  let match = pathname.match(/^\/api\/files\/([^/]+)$/);
  if (match) {
    let name;
    try { name = safeFilename(decodeURIComponent(match[1])); } catch { name = null; }
    if (!name) return send(res, 400, 'Unsupported filename.', 'text/plain; charset=utf-8');
    if (req.method === 'PUT') return uploadFile(req, res, name);
  }
  match = pathname.match(/^\/api\/download\/([^/]+)$/);
  if ((req.method === 'GET' || req.method === 'HEAD') && match) {
    let name;
    try { name = safeFilename(decodeURIComponent(match[1])); } catch { name = null; }
    if (!name) return send(res, 400, 'Unsupported filename.', 'text/plain; charset=utf-8');
    return serveDocument(req, res, name);
  }
  match = pathname.match(/^\/api\/config\/([^/]+)$/);
  if (req.method === 'GET' && match) {
    let name;
    try { name = safeFilename(decodeURIComponent(match[1])); } catch { name = null; }
    if (!name) return json(res, 400, { error: 'Unsupported filename.' });
    return editorConfig(req, res, name);
  }
  match = pathname.match(/^\/api\/callback\/([^/]+)$/);
  if (req.method === 'POST' && match) {
    let name;
    try { name = safeFilename(decodeURIComponent(match[1])); } catch { name = null; }
    if (!name) return json(res, 400, { error: 1 });
    return callback(req, res, name);
  }
  match = pathname.match(/^\/edit\/([^/]+)$/);
  if (req.method === 'GET' && match) {
    let name;
    try { name = safeFilename(decodeURIComponent(match[1])); } catch { name = null; }
    if (!name) return send(res, 400, 'Unsupported filename.', 'text/plain; charset=utf-8');
    const encoded = JSON.stringify(name).replace(/</g, '\\u003c');
    const html = '<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Editing document</title><style>html,body{margin:0;height:100%;font:14px system-ui,sans-serif}header{height:48px;display:flex;align-items:center;padding:0 16px;border-bottom:1px solid #ddd;gap:16px}a{color:#d9572b;text-decoration:none}#editor{height:calc(100% - 49px)}#state{color:#667}</style><script src="/web-apps/apps/api/documents/api.js"></script></head><body><header><a href="/">← Documents</a><span id="title"></span><span id="state">Loading editor…</span></header><div id="editor"></div><script>const name=' + encoded + ';document.querySelector("#title").textContent=name;const publicOrigin=encodeURIComponent(window.location.origin);fetch("/api/config/"+encodeURIComponent(name)+"?publicOrigin="+publicOrigin).then(r=>{if(!r.ok)throw new Error("Could not load document settings");return r.json()}).then(config=>{document.querySelector("#state").textContent="";new DocsAPI.DocEditor("editor",config)}).catch(e=>document.querySelector("#state").textContent=e.message)</script></body></html>';
    return send(res, 200, html, 'text/html; charset=utf-8');
  }
  return proxy(req, res);
}

const server = http.createServer((req, res) => {
  route(req, res).catch((error) => {
    console.error('ONLYOFFICE request failed: ' + (error.stack || error));
    if (!res.headersSent) send(res, 500, 'Internal server error.', 'text/plain; charset=utf-8');
    else res.destroy(error);
  });
});
server.on('upgrade', proxyUpgrade);
const host = process.env.HOST || '0.0.0.0';
server.listen(port, host, () => console.log('ONLYOFFICE AgentOS adapter listening on ' + host + ':' + port));
process.on('SIGTERM', () => server.close());
process.on('SIGINT', () => server.close());
