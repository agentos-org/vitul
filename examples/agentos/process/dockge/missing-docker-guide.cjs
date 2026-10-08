'use strict';

const http = require('node:http');
const { execFile } = require('node:child_process');

const port = Number(process.env.PORT || process.env.DOCKGE_PORT || 5001);
const hostname = process.env.HOST || process.env.DOCKGE_HOSTNAME || '0.0.0.0';
const initialReason = process.env.DOCKGE_GUIDE_REASON ||
  '当前主机没有可用的 Docker CLI。';

function escapeHtml(value) {
  return String(value)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;');
}

function checkDocker(callback) {
  execFile(
    'docker',
    ['compose', 'version'],
    { env: process.env, timeout: 3000 },
    (error) => {
      if (error) {
        callback({
          ready: false,
          reason: error.code === 'ENOENT'
            ? '没有找到 Docker CLI。'
            : 'Docker Compose V2 不可用。',
        });
        return;
      }

      callback({
        ready: true,
        reason: 'Docker Engine 和 Docker Compose V2 已可用。',
      });
    },
  );
}

function sendJson(response, statusCode, body) {
  response.writeHead(statusCode, {
    'content-type': 'application/json; charset=utf-8',
    'cache-control': 'no-store',
  });
  response.end(JSON.stringify(body));
}

const page = `<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Dockge - 需要 Docker</title>
  <style>
    :root { color-scheme: dark; font-family: Inter, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; }
    body { margin: 0; min-height: 100vh; display: grid; place-items: center; background: #111827; color: #e5e7eb; }
    main { width: min(680px, calc(100% - 40px)); padding: 36px; border: 1px solid #374151; border-radius: 18px; background: #1f2937; box-shadow: 0 18px 60px #0006; }
    .mark { width: 54px; height: 54px; display: grid; place-items: center; border-radius: 14px; background: #2496ed; font-size: 28px; }
    h1 { margin: 22px 0 10px; font-size: 28px; }
    p { line-height: 1.7; color: #cbd5e1; }
    .status { margin: 24px 0; padding: 14px 16px; border-radius: 10px; background: #111827; color: #fbbf24; }
    .status.ready { color: #86efac; }
    .actions { display: flex; flex-wrap: wrap; gap: 12px; margin-top: 24px; }
    a, button { border: 0; border-radius: 9px; padding: 11px 16px; font: inherit; cursor: pointer; text-decoration: none; }
    a { background: #2496ed; color: white; }
    button { background: #374151; color: #f9fafb; }
    small { display: block; margin-top: 24px; color: #94a3b8; }
  </style>
</head>
<body>
  <main>
    <div class="mark">🐳</div>
    <h1>Dockge需要 Docker</h1>
    <p>Dockge 是 Docker Compose 管理界面，当前主机还没有满足运行条件。请先在主机上安装 Docker Engine 和 Docker Compose V2。</p>
    <div id="status" class="status">正在检查 Docker 状态…</div>
    <div class="actions">
      <a href="https://docs.docker.com/engine/install/" target="_blank" rel="noreferrer">查看 Docker 安装文档</a>
      <button type="button" onclick="checkStatus()">重新检查</button>
    </div>
    <small>安装完成后，请在 Agent Store 中重启 Dockge。</small>
  </main>
  <script>
    const initialReason = ${JSON.stringify(initialReason)};
    const statusElement = document.getElementById('status');
    function renderStatus(status) {
      statusElement.textContent = status.ready
        ? status.reason + ' 请重启 Dockge 进入 Dockge。'
        : status.reason;
      statusElement.className = status.ready ? 'status ready' : 'status';
    }
    async function checkStatus() {
      statusElement.textContent = '正在检查 Docker 状态…';
      try {
        const response = await fetch('/api/docker-status', { cache: 'no-store' });
        renderStatus(await response.json());
      } catch (_) {
        renderStatus({ ready: false, reason: initialReason });
      }
    }
    checkStatus();
    window.setInterval(checkStatus, 5000);
  </script>
</body>
</html>`;

const server = http.createServer((request, response) => {
  const requestUrl = new URL(request.url || '/', `http://${request.headers.host || 'localhost'}`);

  if (requestUrl.pathname === '/api/docker-status') {
    if (request.method !== 'GET' && request.method !== 'HEAD') {
      sendJson(response, 405, { ready: false, reason: '只支持 GET 请求。' });
      return;
    }
    checkDocker((status) => sendJson(response, 200, status));
    return;
  }

  if (requestUrl.pathname === '/' || requestUrl.pathname === '/index.html') {
    response.writeHead(200, {
      'content-type': 'text/html; charset=utf-8',
      'cache-control': 'no-store',
    });
    response.end(request.method === 'HEAD' ? undefined : page);
    return;
  }

  response.writeHead(404, { 'content-type': 'text/plain; charset=utf-8' });
  response.end('Not found');
});

server.listen(port, hostname, () => {
  console.log(`Dockge prerequisite guide listening on ${hostname}:${port}`);
});
