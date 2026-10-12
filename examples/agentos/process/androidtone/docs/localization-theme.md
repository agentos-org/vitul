# 中英文与系统主题

AndroidTone 遵循本仓库的[AgentOS WebApp 多语言与主题接入指南](../../docs/webapp-localization-theme-zh_CN.md)，并提供一份 React 实现示例。应用不增加独立语言或主题设置。

初始启动：从 AgentOS 原生启动参数（`runtime=agentos`）或 WebShell 插件参数（`hostOrigin`）读取 `locale`、`theme`；支持中文和英文，以及 light/dark 两套配色。`appearance=system` 是系统偏好，实际呈现使用平台已解析的 `theme`。同步首屏脚本在样式及 React 加载前设置根节点语言、主题和背景，防止首屏主题闪烁。

实时更新：监听 AgentOS 的 `agentos:host` / `context-changed` 通知，以及 WebShell 父窗口发来的 `shell:theme`、`shell:locale`。父窗口消息需匹配启动参数中的 `hostOrigin`（没有该参数时要求同源）；拒绝其他 frame 来源。AgentOS 消息支持 schemaVersion 1 及省略版本的旧格式。更新语言、`data-theme`、`data-fenn-theme`、dark/theme-dark 类和 color-scheme，不重载页面，不丢失导入任务或应用状态。

普通浏览器：语言按 navigator.language 解析，中文之外回退英文；主题按 prefers-color-scheme，并监听系统明暗及语言变化。即使启动 URL 不含 runtime 标记，也接受来自同一窗口的有效平台通知。兼容性依赖实际部署的 AgentOS Shell 已实现该契约；本次不修改或部署 AgentOS。

文案覆盖：标题、导入、运行状态、任务状态、应用操作、通知、卸载确认、数量及署名页。应用名称、文件名和原始后端诊断不强行翻译；运行时与任务的原始诊断折叠展示。署名页改为 Vite 多页面构建，使用同一套语言和主题模块，并在内部链接传递当前启动上下文。

配色使用语义变量定义背景、表面、文字、边框、主操作、状态与错误色。两种主题均保留 Android 绿色品牌色，深色主题主按钮使用浅绿背景与深色文字。

验证：13 项 Playwright 测试覆盖已有业务操作、中英文×明暗主题、1120/390 宽度、署名页、实时切换、旧版本消息、无效版本/跨 frame 消息、普通浏览器回退、React 加载前深色首屏、英文操作确认与语言切换时通知更新。浏览器测试通过模拟平台消息与 API 验证，不等同于 VM Shell 端到端验收。

源码：`frontend/src/hostContext.ts`、`i18n.ts`、`messages.ts`、`styles.css`。

真实服务检查：在临时 ARM64 Docker 容器中以 UID/GID 1000、只读根文件系统、移除 capabilities 和 no-new-privileges 运行既有 Rust 后端，首页、署名页、所有直接引用的 Vite 资源、图标、健康与状态 API 均通过；缺失资源仍返回 404。临时容器已清理。

最终包：`dist/androidtone-0.1.0-agentos-arm64.agt`，1,665,938,544 字节，SHA-256 为 `bad21ed8ff65bc612d93c878c95e36510778d489f7b5b8fd889efde26eab6036`。完整前端、运行时文件及 window 模式 manifest 已复验；本轮未部署 VM。

版本升级至 0.1.1，重新编译 ARM64 后端并生成 `dist/androidtone-0.1.1-agentos-arm64.agt`（1,665,936,640 字节）。SHA-256：`ab6b19ad27b0db373ca277713ce85f3feaaa2ec7eb85384f7a105aa79cdf725f`。包内版本、window 模式及全部前端/运行时文件完整性复验通过。

自动主题切换修复：WebShell 传入 `hostOrigin`、`appearance` 和已解析的 `theme`，但不一定包含 `runtime=agentos`；现已将 `hostOrigin` 识别为宿主上下文，并接收父窗口主题通知。宿主运行时以宿主通知为准；普通浏览器仍跟随并监听 prefers-color-scheme。

此前重建因自动审批服务返回 429 未执行；用户再次要求构建后已完成。

主题与语言通知兼容修复包已重建：`dist/androidtone-0.1.1-agentos-arm64.agt`，1,665,938,440 字节，SHA-256：`ec662d19003381c6a500b75380d42818f251ce3297fa1dc07a4ae3483526f0a7`。版本 0.1.1、window 模式、全部前端及运行时文件完整性校验通过；本轮未部署 VM。

最新版升级为 0.1.2，完整校验通过后已清理 dist/ 内旧 .agt 与对应摘要，仅保留 `androidtone-0.1.2-agentos-arm64.agt` 及 `.sha256`。新包大小 1,665,939,675 字节，SHA-256 为 `156578b09f035eb6ef1e66b12b76697d5194ce77bf104d93f386a71377fb1b55`。

0.1.3 用户文案：统一显示“Android 运行环境”/“Android runtime”，不向用户展示 Waydroid 和 Host Provider 名称；状态、操作提示和展开诊断使用同一转换层，包内载荷详情改为 Android 版本与系统授权说明。后端协议、运行时目录和日志保留实际技术标识。中英文诊断展开检查新增两项，当前 19 项浏览器测试通过。
