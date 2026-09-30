# Vitul 项目

[English](VITUL.md)

Vitul 在 macOS、Windows、Android 和 iOS 上使用系统 WebView。AgentOS 目标会将相同的 Web 前端和 Rust OpenTool 后端打包，并通过基于 Chromium 的嵌入式 WebView 显示页面。

## 命令

- `pnpm install`：安装前端、宿主 API 和 AgentOS TypeScript SDK 依赖。Rust CLI 需单独安装；项目脚本会调用 `PATH` 中的 `vitul` 命令。
- `pnpm vitul dev`：构建并启动 OpenTool 后端，使用操作系统分配的 loopback 端口，然后启动 Vitul 桌面应用。
- `pnpm vitul build`：通过兼容 CLI 构建桌面应用。
- `pnpm vitul android dev` 和 `pnpm vitul ios dev`：通过兼容移动 CLI 在设备或模拟器上运行应用。
- `pnpm vitul android build --apk`：构建 Android APK；使用 `--aab` 构建 Android App Bundle。`pnpm vitul ios build` 构建 iOS IPA。
- Rust CLI 在 `PATH` 中时，也可以运行 `vitul dev` 和 `vitul build`。
- 桌面构建生成平台原生安装产物：默认情况下，macOS 生成 `.app` 应用包和 `.dmg` 磁盘映像；Windows 生成 `.msi` 安装包和 NSIS `.exe` 安装程序。可以在 `src-vitul/vitul.conf.json` 的 `bundle.targets` 中选择桌面打包格式。
- `vitul build --target agentos-arm64`：构建 Web 资源，并在 Docker 中构建 ARM64 Linux 后端，最后把 AgentOS `.agt` 包写入 `dist/`。
- AgentOS v2 `.agt` manifest 支持 ARM64 和 x64。当前 Vitul 打包器构建 ARM64 包。

设置 `VITUL_AGENTOS_BUILD_IMAGE` 可以覆盖容器镜像；也可以修改 `vitul.conf.json` 中的 `agentos.buildImage`。

在项目根目录的 `vitul.conf.json` 中设置 `agentos.open.titleBar` 为 `system` 或 `overlay`，选择 AgentOS App 内容相对于 Shell 标题栏的布局，默认值为 `system`。该设置会写入生成的 AgentOS `app.json`；宿主会报告实际生效的模式。Shell 管理的窗口会继续由 Shell 管理并绘制窗口控件。

## 应用 API

将业务操作放在 Rust 后端，并通过 `src/opentool.ts` 调用。脚手架中的 `greet` 示例使用 OpenTool JSON-RPC。

生成的 `package.json` 已包含 `agentos-sdk-ts`，供需要调用 AgentOS Gateway API 的应用使用。默认脚手架不会调用 Gateway；只有应用需要这些 API 时才在代码中导入 SDK。

`src/host.ts` 定义系统 API 边界；`src/desktop-host.ts` 将其映射到桌面和移动原生运行时，AgentOS provider 则调用宿主提供的 `window.agentos.host` 桥接接口。脚手架示例通过宿主 IPC 演示最小化窗口，AgentOS 仅请求 `native.window-control` 权限。只有应用确实需要时，才在 `vitul.conf.json` 中添加其他 `native.*` 权限。AgentOS 会为每个应用实例动态分配 loopback 端口来启动后端，并在同一来源下以 `/` 提供前端、以 `/opentool` 提供 OpenTool RPC。由 AgentOS WebShell 通过 `/apps/{appId}/` 托管时，生成的客户端会使用该代理前缀访问同一 App 后端。

Android 和 iOS 通过原生 IPC 调用嵌入 App 原生进程的 OpenTool 路由。请求继续使用 OpenTool JSON-RPC 消息格式，`streamCall` 事件通过原生事件通道传递；移动端不会启动 HTTP 服务。后端通过 `backend/` 链接进移动 App。桌面应用仍通过 HTTP 调用独立运行的 OpenTool 后端，桌面 IPC 处理器只向前端提供后端地址。

业务 API 应放在 OpenTool 后端中，不要为每个业务操作额外创建一个原生命令。

## Sidecar

通过兼容的 `bundle.externalBin` 配置项声明 sidecar，配置文件位于 `src-vitul/vitul.conf.json`。桌面构建会保留带目标后缀的 sidecar 文件；AgentOS ARM64 构建器会将对应的 ARM64 Linux 二进制文件收集到 `.agt` 包的 `sidecars/` 目录中。

AgentOS 中的 sidecar 逻辑名称取自配置路径的 basename，必须匹配 `[a-z][a-z0-9-]{0,31}`；例如 `binaries/worker` 对应 `worker`。通过统一宿主 API 启动 sidecar：`host.sidecar.spawn("binaries/worker", ["--version"])`，并调用返回句柄的 `kill()` 停止进程。

若 AgentOS App 声明了 `bundle.externalBin`，请在项目根目录的 `vitul.conf.json` 的 `agentos.permissions` 中增加 `native.sidecars`。打包器会在 sidecar 已声明但权限未申请时中止打包；AgentOS Shell 仍会向用户请求授权。浏览器预览不支持 sidecar。
