# Vitul

[English](README.md)

Vitul 是一套以 OpenTool 为核心的应用开发工具链。macOS、Windows、Android 和 iOS 应用使用系统 WebView；AgentOS 应用使用基于 Chromium 的嵌入式 WebView。

## 运行目标

| 目标平台 | UI 运行时 | 系统 API | 应用后端 |
| --- | --- | --- | --- |
| macOS 和 Windows | 系统 WebView | 平台宿主 IPC | 基于 HTTP/JSON-RPC 的 OpenTool |
| Android 和 iOS | 系统 WebView | Tauri IPC | 通过 Tauri IPC 传递 OpenTool JSON-RPC；`streamCall` 使用 Tauri Channel |
| AgentOS | 基于 Chromium 的嵌入式 WebView | 宿主 IPC 桥接 | 基于 HTTP/JSON-RPC 的 OpenTool |

业务操作放在 OpenTool 后端中。窗口控制、对话框、剪贴板、通知等操作通过宿主 API 桥接执行。

## 熟悉 Tauri？

Vitul 是基于 Tauri 2 演进的 OpenTool 优先工具链。它复用兼容的桌面基础，增加自己的项目流程和 AgentOS 目标，并在移动端通过 Tauri IPC 承载 OpenTool 业务调用：

| 方面 | Tauri | Vitul |
| --- | --- | --- |
| 基础定位 | 桌面应用框架 | 基于 Tauri 2 兼容运行时、API 和配置模型构建的 OpenTool 优先工具链 |
| 桌面 UI | 系统 WebView | macOS 和 Windows 同样使用系统 WebView |
| 命令行 | `tauri`、`cargo tauri` | `vitul`、`cargo vitul` |
| 桌面配置路径 | `src-tauri/tauri.conf.json` | `src-vitul/vitul.conf.json` |
| 前端调用 | 通常通过 IPC 命令处理宿主和应用操作 | 宿主 IPC 处理系统操作；OpenTool JSON-RPC 处理业务操作 |
| 构建目标 | Linux、macOS、Windows 桌面应用；Android 和 iOS 移动应用 | macOS/Windows 桌面应用、Android/iOS 移动应用，以及 manifest 架构支持 ARM64 和 x64 的 AgentOS `.agt` 应用 |
| API 与插件 | Tauri API 及其插件生态 | 兼容的 Rust/JavaScript API 和受支持的兼容插件；具体功能请确认 Vitul 支持情况 |

你已有的 Rust、WebView、窗口、权限和打包经验都可以沿用。创建新应用时从 `vitul init` 开始；Vitul 使用自己的命令和项目文件，因此不会自动把 Tauri 项目的 `src-tauri` 目录识别为 Vitul 项目。Android 和 iOS 使用系统 WebView 与 Tauri IPC；Vitul 通过 IPC 传递 OpenTool 请求格式，并使用 Tauri Channel 传递流事件。移动后端链接进 App，在原生进程内运行。移动构建需要对应的平台工具链，iOS 构建需要 macOS 和 Xcode。详见 [Tauri 移动端前置要求](https://v2.tauri.app/start/prerequisites/)。

## 创建并运行应用

从本仓库安装 Vitul CLI，然后创建以 OpenTool 为核心的应用：

```sh
cargo install --path crates/vitul-cli
vitul init my-app
cd my-app
pnpm install
```

根据应用运行的平台，选择对应命令和构建环境：

| 目标平台 | 命令 | 运行时与构建产物 | 构建条件 |
| --- | --- | --- | --- |
| AgentOS（ARM64 / x64） | `pnpm vitul build --target agentos-arm64` | 使用 AgentOS 内嵌的 Chromium WebView；v2 `.agt` manifest 支持 ARM64 和 x64。Shell 启动时为后端分配动态 loopback 端口。 | 当前 Vitul 打包器可在配有 Docker 的 ARM64 Mac 上构建 ARM64 包。 |
| Android | 开发：`pnpm vitul android dev`<br>APK：`pnpm vitul android build --apk`<br>Play 商店：`pnpm vitul android build --aab` | 使用系统 Android WebView；生成 `.apk` 或 `.aab`。OpenTool 后端运行在 App 进程内，通过 Tauri IPC 通信。 | 需要 Android Studio、Android SDK/NDK 和 Android Rust target。 |
| iOS | 开发：`pnpm vitul ios dev`<br>构建：`pnpm vitul ios build` | 使用系统 WKWebView；生成 `.ipa`。OpenTool 后端运行在 App 进程内，通过 Tauri IPC 通信。 | 需要 macOS、Xcode 和 iOS Rust target。 |
| Windows | 开发：`pnpm vitul dev`<br>构建：`pnpm vitul build` | 使用系统 WebView2；默认生成 `.msi` 安装包和 NSIS `.exe` 安装程序。 | 在 Windows 上构建；需要 Rust MSVC 工具链、Node.js/pnpm、Visual Studio C++ Build Tools（“使用 C++ 的桌面开发”工作负载）和 WebView2 Runtime。 |
| macOS | 开发：`pnpm vitul dev`<br>构建：`pnpm vitul build` | 使用系统 WebView；默认生成 `.app` 应用包和 `.dmg` 磁盘映像。 | 在 macOS 上构建；需要 Rust、Node.js/pnpm 和 Xcode Command Line Tools（`xcode-select --install`）。 |

桌面产物格式可以通过 `src-vitul/vitul.conf.json` 中的 `bundle.targets` 选择。AgentOS v2
应用 manifest 支持 ARM64 和 x64 安装环境；当前 Vitul 打包器生成 ARM64 包。相关命令不
构建基于 Ubuntu 24 的系统镜像或 AgentOS Desktop 宿主应用；这两者由 AgentOS 自己的
项目构建。

## AgentOS SDK

AgentOS SDK 是调用 AgentOS Gateway API 的客户端库。建议作为对应语言的应用依赖使用，不要把 SDK 源码复制进 Vitul。`vitul init` 生成的项目会在 `package.json` 中预置 TypeScript SDK；默认脚手架不会主动调用它：

| 语言 | 包 | 适用场景 |
| --- | --- | --- |
| TypeScript | [`agentos-sdk-ts`](https://github.com/agentos-org/agentos-sdk-ts) | 需要调用本地或远程 AgentOS Gateway 的 Vitul Web 前端 |
| Dart | [`agentos_sdk`](https://github.com/agentos-org/agentos-sdk-dart) | Dart 客户端，包括 AgentOS Desktop 项目 |
| Python | [`agentos-sdk-python`](https://gitlab.litevar.com/litevar/agentos/agentos-sdk-python) | 调用 AgentOS API 的 Python 服务和自动化程序 |

Vitul 前端只有在需要调用 AgentOS Gateway API 时才需要在代码中导入 TypeScript SDK；默认脚手架通过 OpenTool 提供业务后端。SDK 不负责构建 AgentOS 系统镜像或打包 `.agt` 应用。

## 仓库结构

- `crates/vitul-cli/` 包含 Vitul 项目生成器、开发运行器和 AgentOS 打包器。
- `crates/tauri-*` 是兼容桌面引擎；其 Cargo package 和 Rust crate 名称为兼容现有运行时和插件生态而保留。
- `packages/` 包含兼容的 JavaScript API（`@tauri-apps/api`）和 Vitul CLI 包（`@vitul/cli`）。
- `vendor/create-vitul-app/` 是 `vitul init` 使用的上游项目脚手架。

Vitul 在内部复用兼容的配置 schema、API 行为和桌面工具。新项目使用 Vitul 文件名、来源地址和命令；对用户公开的 CLI 命令只有 `vitul` 和 `cargo vitul`。兼容的 JavaScript API 仍使用 `@tauri-apps/api` 导入路径，CLI 包名则是 `@vitul/cli`。Vitul 自有功能放在 `crates/vitul-*`；现有 `crates/tauri-*` 目录和 Rust crate 身份表示兼容引擎。继承代码的版权和许可证声明保持原样。
