# Vitul 项目

[English](VITUL.md)

Vitul 在 macOS、Windows、Android 和 iOS 上使用系统 WebView。AgentOS 目标会将相同的 Web 前端和 Rust OpenTool 后端打包，并通过基于 Chromium 的嵌入式 WebView 显示页面。

## 命令

- `pnpm install`：安装前端、宿主 API 和项目本地 Vitul CLI。
- 使用 npm 时，执行 `npm install`，之后可运行 `npm run vitul dev` 和 `npm run build`。
- `pnpm vitul dev`：构建并启动 OpenTool 后端，使用操作系统分配的 loopback 端口，然后启动 Vitul 桌面应用。
- `pnpm vitul build`：通过兼容 CLI 构建桌面应用。
- `pnpm vitul android dev` 和 `pnpm vitul ios dev`：通过兼容移动 CLI 在设备或模拟器上运行应用。
- `pnpm vitul android build --apk`：构建 Android APK；使用 `--aab` 构建 Android App Bundle。`pnpm vitul ios build` 构建 iOS IPA。
- 桌面构建生成平台原生安装产物：默认情况下，macOS 生成 `.app` 应用包和 `.dmg` 磁盘映像；Windows 生成 `.msi` 安装包和 NSIS `.exe` 安装程序。可以在根目录 `vitul.conf.json` 的 `bundle.targets` 中选择桌面打包格式。
- `vitul build --target agentos/arm64`：构建 Web 资源，并在 Docker 中构建 ARM64 Linux 后端，最后把 AgentOS `.agt` 包写入 `dist/`。
- `vitul build --target agentos/x64`：构建 x64 Linux 后端和 `.agt` 包。
- 可安装的 AgentOS `.agt` 包使用 manifest v1，支持 ARM64 和 x64。OpenTool 使用独立的 `rpc.version: 2` 绑定版本。

设置 `VITUL_AGENTOS_BUILD_IMAGE` 可以覆盖容器镜像；也可以修改 `vitul.conf.json` 中的 `agentos.buildImage`。

在根目录 `vitul.conf.json` 的 `agentos.open.titleBar` 中设置 `system` 或 `overlay`，选择 AgentOS App 内容相对于 Shell 标题栏的布局，默认值为 `system`。该设置会写入生成的 AgentOS `app.json`；宿主会报告实际生效的模式。Shell 管理的窗口会继续由 Shell 管理并绘制窗口控件。

AgentOS 的 `bundleId` 来自根配置的 `identifier`。可以在同一文件设置 `agentos.port`，指定范围为 1–65535 的容器内后端端口；默认值为 `9627`。
在根目录 `vitul.conf.json` 的 `bundle.icon` 中指定 PNG 或 SVG 文件，即可将其作为
`icon.png` 或 `icon.svg` 放入 AgentOS 安装包。

## 应用 API

将业务操作放在 Rust 后端，并通过 `src/opentool.ts` 调用。脚手架中的 `greet` 示例使用 OpenTool JSON-RPC。

默认脚手架不会调用 AgentOS Gateway API。应用需要这些 API 时，再使用 `pnpm add agentos-sdk-ts` 或 `npm install agentos-sdk-ts` 安装 SDK。

`src/host.ts` 定义系统 API 边界；`src/desktop-host.ts` 将其映射到桌面和移动原生运行时，AgentOS provider 则调用宿主提供的 `window.agentos.host` 桥接接口。脚手架示例通过宿主 IPC 演示最小化窗口，AgentOS 仅请求 `native.window-control` 权限。只有应用确实需要时，才在 `vitul.conf.json` 中添加其他 `native.*` 权限。AgentOS 使用 `HOST=0.0.0.0` 和 `agentos.port` 启动容器内后端，并将分配的 loopback 宿主端口映射到该端口；同一个已安装 App 的多个窗口共享后端和宿主端口。前端在 `/` 提供，OpenTool RPC 在同一来源的 `/opentool` 提供。由 AgentOS WebShell 通过 `/apps/{appId}/` 托管时，生成的客户端会使用该代理前缀访问同一 App 后端。AgentOS App 的 OpenTool 请求体上限为 1 MiB；生成的 Rust 后端只在 AgentOS 环境应用此限制，其他宿主保留 OpenTool SDK 的 16 MiB 默认值。

Android 和 iOS 通过原生 IPC 调用嵌入 App 原生进程的 OpenTool 路由。请求继续使用 OpenTool JSON-RPC 消息格式，`streamCall` 事件通过原生事件通道传递；移动端不会启动 HTTP 服务。后端通过 `backend/` 链接进移动 App。桌面应用仍通过 HTTP 调用独立运行的 OpenTool 后端，桌面 IPC 处理器只向前端提供后端地址。

根目录的 `vitul.conf.json` 是项目配置。Vitul 会在 `src-vitul/` 下创建并忽略运行时配置投影，并在运行桌面或移动命令时更新它。生成的 `build.rs` 会为安装的 Rust 桌面 crates 准备其所需的配置文件名；请勿手动编辑 `src-vitul/` 下的生成文件。

业务 API 应放在 OpenTool 后端中，不要为每个业务操作额外创建一个原生命令。

## Sidecar

通过兼容的 `bundle.externalBin` 配置项声明 sidecar，配置文件位于根目录 `vitul.conf.json`。桌面构建会保留带目标后缀的 sidecar 文件；AgentOS 打包器会将对应架构的 Linux 二进制文件收集到 `.agt` 包的 `sidecars/` 目录中。

AgentOS 中的 sidecar 逻辑名称取自配置路径的 basename，必须匹配 `[a-z][a-z0-9-]{0,31}`；例如 `binaries/worker` 对应 `worker`。通过统一宿主 API 启动 sidecar：`host.sidecar.spawn("binaries/worker", ["--version"])`，并调用返回句柄的 `kill()` 停止进程。

若 AgentOS App 声明了 `bundle.externalBin`，请在项目根目录的 `vitul.conf.json` 的 `agentos.permissions` 中增加 `native.sidecars`。打包器会在 sidecar 已声明但权限未申请时中止打包；AgentOS Shell 仍会向用户请求授权。浏览器预览不支持 sidecar。
