# Vitul Demo 示例

这是推荐的新手示例。Vitul Demo 界面展示应用与宿主之间的三部分契约：

- 标题栏覆盖模式在 macOS 桌面构建中使用原生 overlay，并在生成的 AgentOS
  manifest 中请求 `overlay`。AgentOS 会报告该布局是否生效，以及窗口控件由
  操作系统还是 Shell 管理。
- 最小化、最大化/还原、全屏和关闭按钮通过系统 IPC 调用当前宿主。桌面版本由
  桌面运行时处理；AgentOS 版本调用通用宿主桥接接口。
- greet 表单通过 OpenTool JSON-RPC 向 Rust 后端发送业务请求，后端路由为
  `/opentool/call`。AgentOS WebShell 通过 `/apps/{appId}/` 托管 App 时，客户端
  会使用该代理前缀访问同一后端。
- 语言选择支持跟随系统、英文和简体中文；外观支持跟随系统、浅色、深色和海蓝主题，
  选择会保存在当前浏览器配置中。
- Android 和 iOS 通过 Tauri IPC 传递相同的 OpenTool 请求格式，并在 App
  进程内调用后端路由。`streamCall` 事件使用 Tauri Channel，移动端不启动
  HTTP 服务。

macOS 和 Windows 使用系统 WebView。macOS 窗口使用 Tauri 原生 overlay 标题栏，
并保留系统窗口控件；Windows 保留标准系统标题栏。AgentOS 包请求由 Shell 管理的
overlay 布局，并使用基于 Chromium 的嵌入式 WebView 显示同一份前端资源。

## 在本仓库中运行

在仓库根目录安装依赖，并构建本地 API 包和 Vitul CLI：

    pnpm install
    pnpm build:api
    pnpm build:cli:debug

然后在本目录运行：

    pnpm vitul dev
    pnpm vitul build

移动端开发和构建命令：

    pnpm vitul android dev
    pnpm vitul android build --apk
    pnpm vitul android build --aab
    pnpm vitul ios dev
    pnpm vitul ios build

Android 构建可生成 `.apk` 或 `.aab`；iOS 构建生成 `.ipa`。Android 需要
Android SDK 工具链；iOS 需要 macOS 和 Xcode。

桌面构建使用平台原生打包格式：默认情况下，macOS 生成 `.app` 应用包和
`.dmg` 磁盘映像；Windows 生成 `.msi` 安装包和 NSIS `.exe` 安装程序。可以在
项目根目录 `vitul.conf.json` 的 `bundle.targets` 中选择桌面打包格式。运行桌面或
移动 CLI 时，Vitul 会在 `src-vitul/` 下生成被忽略的运行时配置。

vitul dev 会先使用操作系统分配的 loopback 端口启动 Rust OpenTool 后端，再打开
桌面应用。Vite 开发服务器会把 /opentool 请求转发给后端。

可安装的 AgentOS `.agt` 包使用 manifest v1，支持 ARM64 和 x64。OpenTool 使用独立的
`rpc.version: 2` 绑定版本。项目根目录 `vitul.conf.json` 统一设置桌面、移动和 AgentOS
选项，包括标题栏布局、容器端口和宿主权限。本示例请求 `overlay`、默认容器端口
`9627` 和 `native.window-control`。AgentOS 会分配 loopback 宿主端口并映射到容器端口；
同一已安装 App 的多个窗口共享后端。使用 Docker 构建配置中声明的两个架构：

    pnpm vitul build --target agentos

也可以使用 `--target agentos/arm64` 或 `--target agentos/x64` 单独构建一个架构。

打包器会将标题栏模式写入生成的 `app.json`，其中 `bundleId` 来自
项目根目录 `vitul.conf.json` 的 `identifier`，并将 `.agt` 应用包写入 `dist/`。
Rust 后端在 AgentOS 环境下遵守 1 MiB OpenTool 请求体上限；其他宿主仍使用
OpenTool SDK 的 16 MiB 默认值。

## 代码结构

- src/host.ts 选择宿主并处理 AgentOS 窗口上下文更新。
- src/desktop-host.ts 将窗口操作映射到兼容的原生运行时。
- src/main.ts 实现多语言、多主题选择器和窗口操作演示。
- src/opentool.ts 创建 OpenTool JSON-RPC 请求并校验响应。
- backend/src/lib.rs 实现 greet 业务工具并创建 OpenTool 路由。
- backend/src/main.rs 在桌面开发和 AgentOS 打包中通过 HTTP 托管路由。
- src-vitul/src/lib.rs 在移动端选择 Tauri IPC，在桌面端提供动态后端地址。
