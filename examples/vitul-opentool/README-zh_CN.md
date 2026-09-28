# Vitul OpenTool 示例

这是推荐的新手示例，演示 Web 前端与原生宿主之间的两种独立通路：

- “最小化窗口”通过宿主 IPC 调用系统功能。桌面版本由桌面运行时处理；
  AgentOS 版本调用其宿主桥接接口。
- greet 表单通过 OpenTool JSON-RPC 向 Rust 后端发送业务请求，后端路由为
  `/opentool/call`。AgentOS WebShell 通过 `/apps/{appId}/` 托管 App 时，客户端
  会使用该代理前缀访问同一后端。
- Android 和 iOS 通过 Tauri IPC 传递相同的 OpenTool 请求格式，并在 App
  进程内调用后端路由。`streamCall` 事件使用 Tauri Channel，移动端不启动
  HTTP 服务。

macOS 和 Windows 使用系统 WebView。AgentOS 使用其基于 Chromium 的嵌入式
WebView 显示同一份前端资源。

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
`src-vitul/vitul.conf.json` 的 `bundle.targets` 中选择桌面打包格式。

vitul dev 会先使用操作系统分配的 loopback 端口启动 Rust OpenTool 后端，再打开
桌面应用。Vite 开发服务器会把 /opentool 请求转发给后端。

AgentOS v2 `.agt` manifest 支持 ARM64 和 x64。以下命令使用 Docker 构建 ARM64 包：

    pnpm vitul build --target agentos-arm64

打包器会从 vitul.conf.json 读取原生权限。本示例仅申请 native.window-control，
生成的 .agt 应用包写入 dist/。

## 代码结构

- src/host.ts 选择桌面或 AgentOS 宿主 provider。
- src/desktop-host.ts 将 window.minimize 映射到兼容的原生运行时。
- src/opentool.ts 创建 OpenTool JSON-RPC 请求并校验响应。
- backend/src/lib.rs 实现 greet 业务工具并创建 OpenTool 路由。
- backend/src/main.rs 在桌面开发和 AgentOS 打包中通过 HTTP 托管路由。
- src-vitul/src/lib.rs 在移动端选择 Tauri IPC，在桌面端提供动态后端地址。
