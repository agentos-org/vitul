const appBase = '/apps/wintone'
const api = (path) => `${appBase}/api${path}`
const state = { apps: [], installTasks: [], runtime: null, windowIntegration: null }
const elements = {
  appCount: document.querySelector('#app-count'),
  taskCount: document.querySelector('#task-count'),
  installedCount: document.querySelector('#installed-count'),
  installedList: document.querySelector('#installed-list'),
  taskList: document.querySelector('#task-list'),
  installTaskCount: document.querySelector('#install-task-count'),
  runtimeLabel: document.querySelector('#runtime-label'),
  runtimeMessage: document.querySelector('#runtime-message'),
  notice: document.querySelector('#notice'),
  fileInput: document.querySelector('#file-input'),
  dropZone: document.querySelector('#drop-zone')
}

let noticeTimeout
let refreshTimeout

async function request(path, options) {
  const response = await fetch(api(path), options)
  const body = await response.json().catch(() => ({}))
  if (!response.ok)
    throw new Error(body.error || `请求失败（${response.status}）`)
  return body
}

function formatBytes(value) {
  if (value < 1024) return `${value} B`
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(0)} KB`
  if (value < 1024 * 1024 * 1024)
    return `${(value / 1024 / 1024).toFixed(1)} MB`
  return `${(value / 1024 / 1024 / 1024).toFixed(2)} GB`
}

function formatDate(value) {
  return new Intl.DateTimeFormat('zh-CN', {
    month: 'short',
    day: 'numeric'
  }).format(new Date(Number(value)))
}

function escapeHtml(value) {
  return String(value).replace(
    /[&<>"']/g,
    (character) =>
      ({
        '&': '&amp;',
        '<': '&lt;',
        '>': '&gt;',
        '"': '&quot;',
        "'": '&#39;'
      })[character]
  )
}

function showNotice(message, kind = 'info') {
  clearTimeout(noticeTimeout)
  elements.notice.textContent = message
  elements.notice.className = `notice visible ${kind}`
  noticeTimeout = setTimeout(() => {
    elements.notice.className = 'notice'
  }, 3800)
}

async function loadState() {
  try {
    const [status, installTasks, apps] = await Promise.all([
      request('/status'),
      request('/install-tasks'),
      request('/apps')
    ])
    state.runtime = status.runtime
    state.windowIntegration = status.windowIntegration
    state.installTasks = installTasks.installTasks || []
    state.apps = apps.apps || []
    render()
    scheduleRefresh()
  } catch (error) {
    showNotice(`无法连接 WinTone 服务：${error.message}`, 'error')
  }
}

function scheduleRefresh() {
  clearTimeout(refreshTimeout)
  if (
    state.installTasks.some((task) =>
      [
        'preparing-runtime',
        'preparing-prefix',
        'waiting-runtime',
        'waiting-window-session',
        'starting-installer',
        'awaiting-user',
        'discovering-apps'
      ].includes(task.status)
    ) || state.apps.some((app) => app.sessionId)
  ) {
    refreshTimeout = setTimeout(loadState, 2500)
  }
}

function render() {
  const appTotal = state.apps.length
  const taskTotal = state.installTasks.length
  elements.appCount.textContent = appTotal
  elements.taskCount.textContent = taskTotal
  elements.installedCount.textContent = `${appTotal} 个应用`
  elements.installTaskCount.textContent = `${taskTotal} 个任务`

  const runtimeStates = {
    ready: ['已就绪', 'Wine/FEX 已准备，可在 AgentOS 窗口会话中安装和启动应用。'],
    'not-installed': ['待下载', state.runtime?.message],
    'catalog-unavailable': ['等待运行时包', state.runtime?.message],
    'catalog-invalid': ['运行包配置异常', state.runtime?.message],
    'runtime-invalid': ['运行环境异常', state.runtime?.message]
  }
  const [runtimeLabel, runtimeMessage] =
    runtimeStates[state.runtime?.state] || ['待准备', state.runtime?.message]
  elements.runtimeLabel.textContent = runtimeLabel
  elements.runtimeMessage.textContent =
    runtimeMessage || 'WinTone 会在首次安装应用时准备运行环境。'
  if (state.runtime?.state === 'ready' && state.windowIntegration?.available === false) {
    elements.runtimeMessage.textContent =
      state.windowIntegration.message ||
      'AgentOS Wayland 窗口会话当前不可用；安装任务会在服务就绪后继续。'
  }

  if (!appTotal) {
    elements.installedList.innerHTML = `<div class="empty-state">
      <div class="empty-art" aria-hidden="true"><span class="empty-window back"></span><span class="empty-window front"><i></i><i></i><i></i><b></b></span></div>
      <h3>还没有安装应用</h3><p>导入安装包后，WinTone 会自动创建安装任务。</p>
      <button class="text-button" type="button" data-choose-file>导入第一个安装包 <span>→</span></button>
    </div>`
  } else {
    elements.installedList.innerHTML = state.apps
      .map(
        (app) => `<article class="app-card">
      <div class="app-card-icon">${escapeHtml((app.name || 'W').slice(0, 1))}</div>
      <div class="app-card-copy" title="${escapeHtml(app.lastError || '')}"><strong>${escapeHtml(app.name)}</strong><span>${escapeHtml(app.lastError || app.version || 'Windows 应用')}</span></div>
      <div class="app-actions">
        <button class="app-action primary" type="button" data-launch="${escapeHtml(app.id)}" ${app.sessionId ? 'disabled' : ''}>${app.sessionId ? '运行中' : '打开'}</button>
        <button class="app-action" type="button" data-uninstall="${escapeHtml(app.id)}" ${app.sessionId || !app.uninstallExecutable ? 'disabled' : ''} title="${app.uninstallExecutable ? '卸载应用' : '安装包没有提供可识别的卸载程序'}">卸载</button>
      </div>
    </article>`
      )
      .join('')
  }

  if (!taskTotal) {
    elements.taskList.innerHTML = `<div class="installer-empty">导入安装包后，安装任务会显示在这里。</div>`
  } else {
    elements.taskList.innerHTML = state.installTasks
      .map(
        (task) => {
          const active = [
            'preparing-runtime',
            'preparing-prefix',
            'starting-installer',
            'awaiting-user',
            'discovering-apps'
          ].includes(task.status)
          return `<article class="installer-row">
      <div class="file-type ${task.extension}">${escapeHtml(task.extension.toUpperCase())}</div>
      <div class="installer-info"><strong title="${escapeHtml(task.fileName)}">${escapeHtml(task.fileName)}</strong>
        <span>${formatBytes(task.bytes)} <i>·</i> ${formatDate(task.addedAt)}${task.message ? ` <i>·</i> ${escapeHtml(task.message)}` : ''}</span></div>
      <span class="task-badge" title="${escapeHtml(task.message || '')}"><i></i>${escapeHtml(taskStatusLabel(task.status))}</span>
      <button class="remove-button" type="button" data-remove="${escapeHtml(task.id)}" aria-label="移除安装任务" title="${active ? '安装程序运行中' : '移除'}" ${active ? 'disabled' : ''}>×</button>
    </article>`
        }
      )
      .join('')
  }
}

function taskStatusLabel(status) {
  return (
    {
      'waiting-runtime': '等待运行环境',
      'preparing-runtime': '准备运行环境',
      'preparing-prefix': '准备独立环境',
      'waiting-window-session': '等待窗口会话',
      'starting-installer': '启动安装程序',
      'awaiting-user': '等待安装向导',
      'discovering-apps': '整理应用信息',
      completed: '安装完成',
      failed: '安装失败',
      cancelled: '已取消'
    }[status] || '安装排队中'
  )
}

async function uploadFile(file) {
  if (!file) return
  const extension = file.name.split('.').at(-1)?.toLowerCase()
  if (!['exe', 'msi'].includes(extension)) {
    showNotice('目前只接受 .exe 和 .msi 安装包。', 'error')
    return
  }
  if (file.size > 2 * 1024 * 1024 * 1024) {
    showNotice('安装包超过 2 GB 限制。', 'error')
    return
  }

  const form = new FormData()
  form.append('file', file, file.name)
  showNotice(`正在导入 ${file.name}…`)
  try {
    await request('/install-tasks', { method: 'POST', body: form })
    await loadState()
    showNotice(`${file.name} 已导入并创建任务，WinTone 正在自动准备运行环境。`, 'success')
  } catch (error) {
    showNotice(`导入失败：${error.message}`, 'error')
  }
}

document
  .querySelector('#choose-file')
  .addEventListener('click', () => elements.fileInput.click())
document
  .querySelectorAll('[data-choose-file]')
  .forEach((button) =>
    button.addEventListener('click', () => elements.fileInput.click())
  )
elements.fileInput.addEventListener('change', async (event) => {
  for (const file of event.target.files || []) await uploadFile(file)
  event.target.value = ''
})
document.querySelector('#refresh-button').addEventListener('click', loadState)

elements.dropZone.addEventListener('dragover', (event) => {
  event.preventDefault()
  elements.dropZone.classList.add('dragging')
})
elements.dropZone.addEventListener('dragleave', (event) => {
  if (!elements.dropZone.contains(event.relatedTarget))
    elements.dropZone.classList.remove('dragging')
})
elements.dropZone.addEventListener('drop', async (event) => {
  event.preventDefault()
  elements.dropZone.classList.remove('dragging')
  for (const file of event.dataTransfer?.files || []) await uploadFile(file)
})

elements.taskList.addEventListener('click', async (event) => {
  const removeButton = event.target.closest('[data-remove]')
  if (removeButton) {
    try {
      await request(
        `/install-tasks/${encodeURIComponent(removeButton.dataset.remove)}`,
        { method: 'DELETE' }
      )
      await loadState()
      showNotice('已移除安装任务。', 'success')
    } catch (error) {
      showNotice(`移除失败：${error.message}`, 'error')
    }
    return
  }
})

elements.installedList.addEventListener('click', async (event) => {
  const launchButton = event.target.closest('[data-launch]')
  const uninstallButton = event.target.closest('[data-uninstall]')
  if (launchButton) {
    try {
      await request(`/apps/${encodeURIComponent(launchButton.dataset.launch)}/launch`, {
        method: 'POST'
      })
      await loadState()
      showNotice('已在 AgentOS 窗口中启动应用。', 'success')
    } catch (error) {
      showNotice(`启动失败：${error.message}`, 'error')
    }
  } else if (uninstallButton) {
    const app = state.apps.find((candidate) => candidate.id === uninstallButton.dataset.uninstall)
    if (!app || !window.confirm(`确定卸载“${app.name}”吗？`)) return
    try {
      await request(`/apps/${encodeURIComponent(app.id)}`, { method: 'DELETE' })
      await loadState()
      showNotice(`已启动 ${app.name} 的卸载程序。`, 'success')
    } catch (error) {
      showNotice(`卸载失败：${error.message}`, 'error')
    }
  }
})

loadState()
