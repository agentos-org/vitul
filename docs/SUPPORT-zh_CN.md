# 平台与功能支持范围

Vitul 当前处于 1.0 之前的预览阶段。本文说明当前源码实现的能力及自动化检查的范围。Vitul 沿用熟悉的 Tauri 命令和开发习惯，但不承诺 Tauri 功能或插件兼容，也不会自动导入现有 Tauri 项目。

## 平台矩阵

| 目标平台      | UI 与系统 API                                    | OpenTool 通信                                                         | 构建产物                                    | 当前验证范围                                                                                                                                   |
| ------------- | ------------------------------------------------ | --------------------------------------------------------------------- | ------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| macOS 桌面    | 系统 WebView 和 Vitul 宿主 API                   | 本地后端通过 HTTP/JSON-RPC 通信                                       | `.app`；默认还生成 `.dmg`                   | 继承的工作流会检查部分桌面代码；应用签名和分发由应用开发者负责。                                                                               |
| Windows 桌面  | WebView2 和 Vitul 宿主 API                       | 本地后端通过 HTTP/JSON-RPC 通信                                       | `.msi`；默认还生成 NSIS `.exe`              | 继承的工作流会检查部分桌面代码；签名和分发由应用开发者负责。                                                                                   |
| Android       | Android 系统 WebView 和 Vitul 宿主 API           | OpenTool 路由链接进 App 进程，通过原生 IPC 调用；流式响应使用原生通道 | `.apk` 或 `.aab`                            | 现有移动端工作流会检查部分运行时代码；应用自身的 OpenTool 行为仍需在设备或模拟器上验证。                                                      |
| iOS           | WKWebView 和 Vitul 宿主 API                      | OpenTool 路由链接进 App 进程，通过原生 IPC 调用；流式响应使用原生通道 | `.ipa`                                      | 现有移动端工作流会检查部分运行时代码；应用自身的 OpenTool 行为仍需在设备或模拟器上验证。                                                      |
| AgentOS ARM64 | AgentOS Shell 内嵌的 Chromium WebView 和宿主 IPC | App 进程通过 Shell 分配的 loopback 端口提供 HTTP/JSON-RPC             | 使用 v1 manifest 的 `.agt`                  | Vitul CI 使用 Vitul CLI 把 OpenTool 示例构建为 ARM64 `.agt`，并检查 manifest 和包内容。窗口、权限和安装行为还需要在 AgentOS Shell 中冒烟验证。 |
| AgentOS x64   | 相同的 AgentOS 运行时约定                        | 相同的 AgentOS 运行时约定                                             | 使用 v1 manifest 的 `.agt`                  | CLI 夹具测试覆盖 x64 包清单；完整应用包冒烟工作流在 ARM64 上运行。                                                                                         |

Linux 桌面当前不属于 Vitul 文档承诺的目标平台。Tauri 派生源码可能具备表格以外的能力，但这些能力不属于 Vitul 的支持承诺。

## 按需求选择能力

平台矩阵说明目标平台；下表帮助拆解一个功能需要哪些层。具体字段由
[AgentOS 配置说明](../crates/vitul-cli/src/agentos_config/README-zh_CN.md)维护，
实现责任见[架构](../ARCHITECTURE-zh_CN.md)。跨平台需求需要分别验收。

| 需求类别 | 支持判断与必要依赖 | 完成的证据 |
| --- | --- | --- |
| 业务功能与数据 | OpenTool 后端；AgentOS 持久数据需同时配置和使用持久目录 | 调用结果正确，重启后的数据符合要求 |
| 系统交互 | 对应宿主 API、平台实现和权限；浏览器预览只提供部分降级能力 | 目标宿主上操作成功，拒绝或不可用时可处理 |
| 语言与主题 | 宿主初始上下文、变化订阅、文案与语义样式 | 首次打开正确，运行时切换不丢失业务状态 |
| 窗口与进程生命周期 | 运行模式、关闭策略、后台实现与宿主管理；不同平台分别确认 | 关闭、重新打开、退出及子进程清理符合需求 |
| 外部程序或 sidecar | 目标架构二进制、包声明、权限、启动调用与停止逻辑 | 安装包内文件完整，实际启动和停止成功 |
| Host Provider、部署和网络 | 对应 AgentOS 协议、声明、方法集合、权限和宿主服务 | 声明可校验，目标宿主执行成功，失败及恢复符合预期 |
| 发布与安装 | 目标平台工具链、产物格式、签名及宿主版本 | 在目标环境安装并完成用户流程 |

Process 默认使用容器；需要直接宿主访问时才评估 native profile，且需满足其限制。
Remote-web 不带本地进程，不能承担依赖包内后端的需求。
不能从继承的 Tauri 模块存在、权限名含 `native` 或一个示例可运行，推导所有平台均支持。

配置校验、包内容检查和宿主行为验收是三类证据。本文的 CI 范围只代表明确列出的检查，
不代替应用自身的功能验收。

## API 边界

窗口控制、对话框、剪贴板、通知、sidecar 等系统操作使用宿主 API。应用业务操作放在 OpenTool 后端中。桌面和 AgentOS App 通过 HTTP/JSON-RPC 调用后端；Android 和 iOS App 通过原生 IPC 调用链接在 App 进程内的 OpenTool 路由，移动端通信不会启动 HTTP 服务。

Vitul 有自己支持的配置、Rust API、JavaScript API 和桌面工具。依赖具体 API 或插件前，请先确认 Vitul 的示例和测试覆盖了相应功能；共享源码或熟悉的包名不代表可直接运行 Tauri App 或插件。

## 构建条件

桌面构建使用对应操作系统的原生工具链。Android 需要 Android Studio、Android SDK/NDK 和 Android Rust target；iOS 需要 macOS、Xcode 和 iOS Rust target。AgentOS ARM64 和 x64 打包器使用 Docker 构建 Linux 后端。命令和产物格式见项目根目录的 [README](../README-zh_CN.md)。

## CI 检查范围

`Vitul AgentOS ARM64 package smoke` 工作流会使用 Vitul CLI 构建仓库中的 OpenTool 示例，并检查产物是 ARM64 进程应用，包含 v1 manifest、容器端口、前端入口、健康检查和 OpenTool RPC 配置。工作流还会生成一个全新项目、安装依赖、构建前端，并检查项目脚本能调用 Vitul CLI。其他继承的工作流会检查部分 Tauri 派生代码；这些检查不能代表所有 Vitul 平台和插件都已覆盖。
