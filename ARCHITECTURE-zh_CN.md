# Vitul 架构

[English](ARCHITECTURE.md)

Vitul 将应用的业务后端与操作系统宿主分开。业务调用使用 OpenTool 消息格式，具体传输方式由运行环境决定；系统操作通过宿主 IPC 桥接执行，由宿主负责权限和平台行为。

## 运行目标

### macOS 和 Windows

桌面目标使用兼容 Tauri 的运行时和操作系统提供的 WebView。Vitul 宿主适配器将窗口、对话框、剪贴板、通知和 URL 打开等请求映射到桌面 API。`vitul dev` 会在 loopback 端口启动 OpenTool 后端，并通过桌面 IPC 将服务地址提供给前端。

### Android 和 iOS

移动目标使用平台 WebView 和 Tauri IPC。原生 App 将 OpenTool 后端库链接进自身进程，并在进程内调用现有 OpenTool 路由。普通调用通过 `invoke` 传递相同的 JSON-RPC 请求和响应；`streamCall` 通过 Tauri Channel 转发 OpenTool 事件。移动端不会启动 HTTP 监听服务。

### AgentOS

AgentOS 使用基于 Chromium 的嵌入式 WebView 承载 Vitul 前端。宿主通过 IPC 桥接执行系统操作，并检查应用声明的 `native.*` 权限。AgentOS 为每个应用实例动态分配 loopback 端口，等待后端健康检查通过后，再在 WebView 中加载 manifest 声明的前端入口。

AgentOS 应用后端与前端使用同一个服务来源。manifest 声明 `rpc.protocol: "opentool"`、绑定版本 `2` 和服务路径，通常是 `/opentool`。RPC 使用 OpenTool HTTP/JSON-RPC；宿主 IPC 是处理系统操作的独立通道。

## 工具链

- `crates/vitul-cli/` 实现 `vitul init`、`vitul dev` 和 AgentOS `.agt` 打包目标。
- `vitul` 与 `cargo vitul` 是仅有的面向用户的命令行入口。
- 桌面运行时需要时，Vitul 在内部调用兼容 Tauri 的工具。新项目使用 Vitul 文件名、来源地址、包名和命令名，同时保留兼容的配置 schema 与 API 行为。
- `crates/create-vitul-app/` 是源自上游 `create-tauri-app` 的内部脚手架 crate，Vitul 生成器会在其基础上定制 OpenTool 优先的项目。
- `crates/tauri-*` 包含兼容的桌面运行时、CLI、构建支持和 bundler。Vitul 保留这些底层名称与接口，以兼容现有项目和插件。
- `packages/` 包含兼容的前端 API 和 CLI 包。

## AgentOS 应用包约定

可安装的 `.agt` 应用包使用 `manifestVersion: 2`，并显式声明 `architecture`（`arm64` 或 `x64`）。进程型应用使用端口 `0`，由 Shell 管理启动流程，同时声明前端入口、OpenTool RPC 配置以及应用可能请求的 `native.*` 宿主能力。manifest 格式支持 AgentOS 的 ARM64 和 x64；当前 Vitul 打包器生成 ARM64 包。

## 兼容性与来源说明

Vitul 更新面向用户的品牌，同时保留 Tauri 兼容的 crate 名称、宏、JavaScript 包名、配置格式和 IPC 标识符。这些名称属于兼容接口。继承的源代码仍受其原版权和许可证声明约束，详见仓库中的许可证文件。
