# Vitul 架构

[English](ARCHITECTURE.md)

Vitul 将应用的业务后端与操作系统宿主分开。业务调用使用 OpenTool 消息格式，具体传输方式由运行环境决定；系统操作通过宿主 IPC 桥接执行，由宿主负责权限和平台行为。

## 从需求到实现的责任链

| 需求涉及的行为 | 主要实现位置 | 必须同时核对 |
| --- | --- | --- |
| 页面、交互、语言与外观 | `src/`，宿主上下文适配层 | 初始状态、宿主变化事件、翻译和样式变量 |
| 业务运算、数据读写、长任务 | `backend/src/tools/`，通过 OpenTool 调用 | 数据目录、任务生命周期、错误和进度处理 |
| 窗口、系统对话框、剪贴板、通知、外部链接 | `src/host.ts` 与对应宿主实现 | 目标平台支持、权限、授权拒绝和不可用处理 |
| 调用 AgentOS Gateway | `src/lib/agentos.ts` 与 SDK | Gateway 地址、认证边界；不是应用自身 OpenTool 接口 |
| 外部进程、系统集成、平台服务 | 后端或宿主能力，按协议选择 | 可执行文件、架构、权限、启动入口、退出清理 |

配置声明期望，应用代码实现业务和协议接入，打包器收集产物，宿主执行授权并管理平台行为。
这四者不能相互替代。某项能力如果不在 `HostProvider` 接口中，不能凭空添加调用并假定宿主支持；
需先查该平台的协议与现有实现。相同的功能名称在桌面、移动、AgentOS 上也不保证同一实现。

开发者编辑根 `vitul.conf.json`；`src-vitul/` 中的运行时配置和 `.agt` 内的 `app.json`
是投影或构建结果。验证时应沿“根字段 → 生成字段/文件 → 启动入口 → 宿主效果”检查，
遇到问题先定位在哪个环节失效，再修改对应源文件。

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
- `crates/tauri-*` 包含兼容的桌面运行时、CLI、构建支持和 bundler。Vitul 保留这些底层名称；共享源码不代表现有 Tauri 项目或插件可直接兼容。
- `packages/` 包含兼容的前端 API 和 CLI 包。

## AgentOS 应用包约定

根目录 `vitul.conf.json` 使用 `schemaVersion: 2`；生成的 `.agt` 包使用
`manifestVersion: 1`。Process 包声明 `architecture`（`arm64` 或 `x64`），
打包器支持两种架构；remote-web 包使用 `any`。这些版本与 OpenTool 的
`rpc.version: 2` 分别定义不同协议，不能互相替换。

Process 应用的 `agentos.run.port`（默认 `9627`）写入 manifest 的 `run.port`，
必须与后端监听端口一致。容器模式由 Shell 分配 loopback 宿主端口并映射到容器端口。
包内声明前端入口、可选 OpenTool RPC 配置以及请求的 `native.*` 能力。
根配置是开发者编辑入口，包内 `app.json` 是生成产物。


## 兼容性与来源说明

Vitul 更新面向用户的品牌，同时保留 Tauri 兼容的 crate 名称、宏、JavaScript 包名、配置格式和 IPC 标识符。这些名称属于兼容接口。继承的源代码仍受其原版权和许可证声明约束，详见仓库中的许可证文件。
