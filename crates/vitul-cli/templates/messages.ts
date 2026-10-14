import type { Locale } from './host-context'

const en = {
  starter: 'starter',
  ready: 'Ready to build',
  eyebrow: 'Your app starts here',
  headline: 'Make an idea',
  headlineAccent: 'feel real.',
  intro:
    'Your React interface, Rust business tools, and native app APIs are ready to work together. Replace this starter screen with the app you have in mind.',
  backend: 'OpenTool backend',
  business: 'Business logic',
  callBackend: 'Call your Rust backend',
  name: 'Name',
  namePlaceholder: 'Enter a name',
  calling: 'Calling…',
  sayHello: 'Say hello',
  greetingInitial: 'Your first OpenTool call will appear here.',
  system: 'System integration',
  talkHost: 'Talk to the host',
  hostUnavailable: 'Host bridge unavailable',
  hostConnecting: 'Connecting to host…',
  hostConnected: '{platform} host connected',
  minimize: 'Minimize window',
  windowInitial: 'Try a native host action.',
  windowSending: 'Sending window.minimize through host IPC…',
  windowDone: 'The native host minimized the window.',
  builtWith: 'Built with Vitul',
  footer: 'Change this screen. Keep the foundation.'
}

const zh: typeof en = {
  starter: '起始模板',
  ready: '准备就绪',
  eyebrow: '你的应用从这里开始',
  headline: '让一个想法',
  headlineAccent: '成为现实。',
  intro:
    'React 界面、Rust 业务工具与原生应用 API 已准备就绪。将这个起始页面替换为你心中的应用。',
  backend: 'OpenTool 后端',
  business: '业务逻辑',
  callBackend: '调用 Rust 后端',
  name: '姓名',
  namePlaceholder: '输入姓名',
  calling: '调用中…',
  sayHello: '打个招呼',
  greetingInitial: '首次 OpenTool 调用结果将显示在这里。',
  system: '系统集成',
  talkHost: '与宿主交互',
  hostUnavailable: '宿主桥接不可用',
  hostConnecting: '正在连接宿主…',
  hostConnected: '已连接 {platform} 宿主',
  minimize: '最小化窗口',
  windowInitial: '试试原生宿主操作。',
  windowSending: '正在通过宿主 IPC 请求最小化窗口…',
  windowDone: '宿主已最小化窗口。',
  builtWith: '使用 Vitul 构建',
  footer: '改造界面，保留基础能力。'
}

export const messages: Record<Locale, typeof en> = { 'en-US': en, 'zh-CN': zh }
