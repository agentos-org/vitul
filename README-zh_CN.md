# Vitul

[English](README.md)

[GitHub 仓库](https://github.com/agentos-org/vitul) · [平台支持范围](docs/SUPPORT-zh_CN.md) · [维护与发布规则](docs/MAINTENANCE-zh_CN.md)

Vitul 是一套以 OpenTool 为核心的应用开发工具链。macOS、Windows、Android 和 iOS 应用使用系统 WebView；AgentOS 应用使用基于 Chromium 的嵌入式 WebView。

## 运行目标

| 目标平台         | UI 运行时                      | 系统 API      | 应用后端                                                               |
| ---------------- | ------------------------------ | ------------- | ---------------------------------------------------------------------- |
| macOS 和 Windows | 系统 WebView                   | 平台宿主 IPC  | 基于 HTTP/JSON-RPC 的 OpenTool                                         |
| Android 和 iOS   | 系统 WebView                   | Tauri IPC     | 通过 Tauri IPC 传递 OpenTool JSON-RPC；`streamCall` 使用 Tauri Channel |
| AgentOS          | 基于 Chromium 的嵌入式 WebView | 宿主 IPC 桥接 | 基于 HTTP/JSON-RPC 的 OpenTool                                         |

业务操作放在 OpenTool 后端中。窗口控制、对话框、剪贴板、通知等操作通过宿主 API 桥接执行。

## 熟悉 Tauri？

Vitul 是一套独立维护、源自 [Tauri 2 上游源码仓库](https://github.com/tauri-apps/tauri) 的 OpenTool 优先工具链，并保留继承代码中的版权和许可证声明。Vitul 沿用熟悉的 Tauri 命令和项目习惯，同时定义自己的项目格式、API、插件支持范围和 AgentOS 流程。它不是 Tauri 官方项目，也不会自动导入已有 Tauri App：

| 方面         | Tauri                                                   | Vitul                                                                                                    |
| ------------ | ------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| 基础定位     | 桌面应用框架                                            | 源自 Tauri 2 代码的 OpenTool 优先工具链                                                                  |
| 桌面 UI      | 系统 WebView                                            | macOS 和 Windows 同样使用系统 WebView                                                                    |
| 命令行       | `tauri`、`cargo tauri`                                  | `vitul`、`cargo vitul`                                                                                   |
| 项目配置路径 | `src-tauri/tauri.conf.json`                             | 项目根目录的 `vitul.conf.json`                                                                           |
| 前端调用     | 通常通过 IPC 命令处理宿主和应用操作                     | 宿主 IPC 处理系统操作；OpenTool JSON-RPC 处理业务操作                                                    |
| 构建目标     | Linux、macOS、Windows 桌面应用；Android 和 iOS 移动应用 | macOS/Windows 桌面应用、Android/iOS 移动应用，以及 manifest 架构支持 ARM64 和 x64 的 AgentOS `.agt` 应用 |
| API 与插件   | Tauri API 及其插件生态                                  | 以 Vitul 自身支持范围为准；共享源码不代表兼容 Tauri 功能或插件                                            |

你已有的 Rust、WebView、窗口、权限、打包和 CLI 使用习惯都可以沿用。使用 `vitul create` 或 `npm create vitul-app` 创建新应用。Vitul 使用自己的项目格式，不会导入或自动转换已有的 Tauri 项目。Android 和 iOS 使用系统 WebView 与 Tauri IPC；Vitul 通过 IPC 传递 OpenTool 请求格式，并使用 Tauri Channel 传递流事件。移动后端链接进 App，在原生进程内运行。移动构建需要对应的平台工具链，iOS 构建需要 macOS 和 Xcode。详见 [Tauri 移动端前置要求](https://v2.tauri.app/start/prerequisites/)。

## 创建并运行应用

npm 包名为 `vitul` 和 `create-vitul-app`。包发布后，可这样创建以 OpenTool 为核心的应用：

```sh
npm create vitul-app@latest my-app
cd my-app
npm install
npm run vitul dev
```

也可以全局安装 CLI，再使用熟悉的 `vitul create` 流程：

```sh
npm install --global vitul
vitul create my-app
```

npm 包正式发布前，可从源码构建 CLI：

```sh
git clone https://github.com/agentos-org/vitul.git
cd vitul
cargo install --path crates/vitul-cli --locked
vitul create my-app
cd my-app
# npm 包发布前，请从 package.json 的 devDependencies 中移除 @litevar/vitul-cli
pnpm install
```

`vitul` 命令和脚手架生成的 `vitul` 项目脚本使用同一套 CLI。npm 包的源码位于 `packages/vitul`、`packages/create-vitul-app` 和 `packages/cli`；完成发布后即可通过 npm 安装。

根据应用运行的平台，选择对应命令和构建环境：

| 目标平台               | 命令                                                                                                                   | 运行时与构建产物                                                                                                       | 构建条件                                                                                                                                    |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| AgentOS（ARM64 / x64） | `pnpm vitul build --target agentos`                                                                                    | 使用 AgentOS 内嵌的 Chromium WebView；v1 `.agt` manifest 支持 ARM64 和 x64。Shell 启动时为后端分配动态 loopback 端口。 | Docker 会为每个已声明的 Linux 架构构建 Rust 后端。                                                                                          |
| Android                | 开发：`pnpm vitul android dev`<br>APK：`pnpm vitul android build --apk`<br>Play 商店：`pnpm vitul android build --aab` | 使用系统 Android WebView；生成 `.apk` 或 `.aab`。OpenTool 后端运行在 App 进程内，通过 Tauri IPC 通信。                 | 需要 Android Studio、Android SDK/NDK 和 Android Rust target。                                                                               |
| iOS                    | 开发：`pnpm vitul ios dev`<br>构建：`pnpm vitul ios build`                                                             | 使用系统 WKWebView；生成 `.ipa`。OpenTool 后端运行在 App 进程内，通过 Tauri IPC 通信。                                 | 需要 macOS、Xcode 和 iOS Rust target。                                                                                                      |
| Windows                | 开发：`pnpm vitul dev`<br>构建：`pnpm vitul build`                                                                     | 使用系统 WebView2；默认生成 `.msi` 安装包和 NSIS `.exe` 安装程序。                                                     | 在 Windows 上构建；需要 Rust MSVC 工具链、Node.js/pnpm、Visual Studio C++ Build Tools（“使用 C++ 的桌面开发”工作负载）和 WebView2 Runtime。 |
| macOS                  | 开发：`pnpm vitul dev`<br>构建：`pnpm vitul build`                                                                     | 使用系统 WebView；默认生成 `.app` 应用包和 `.dmg` 磁盘映像。                                                           | 在 macOS 上构建；需要 Rust、Node.js/pnpm 和 Xcode Command Line Tools（`xcode-select --install`）。                                          |

新建项目在根目录的 `vitul.conf.json` 中统一配置桌面、移动和 AgentOS 设置。
使用 `bundle.targets` 选择桌面产物格式，使用 `agentos.targets` 声明 AgentOS 架构。
运行桌面或移动工具时，Vitul 会在 `src-vitul/` 下生成运行时配置；开发者只需编辑
根目录配置。已有项目可以继续使用原有运行时配置。独立的 Vitulfile 打包流程支持
为现有项目构建 ARM64 和 x64 包。AgentOS `.agt` 使用 manifest v1，支持 ARM64 和 x64；
AgentOS 会将动态分配的 loopback 宿主端口映射到 `agentos.port` 指定的容器内端口
（默认 `9627`）。相关命令不
构建基于 Ubuntu 24 的系统镜像或 AgentOS Desktop 宿主应用；这两者由 AgentOS 自己的
项目构建。

### 使用 Vitulfile 打包已有 Web 项目

`Vitulfile` 面向独立的已有前后端项目和线上网站，不要求项目使用 Rust 或 Tauri。process
项目可在 `targets` 中声明 `agentos/arm64`、`agentos/x64` 或两者，然后执行：

```sh
cd path/to/existing-project
vitul agentos init
# 配置 Vitulfile 中的 build、artifacts 和 run
vitul agentos validate
vitul build --target agentos
```

线上 HTTPS 网站使用 `vitul agentos init --type remote-web --url https://example.com`；
AgentOS 会在内嵌 CEF WebApp 窗口中打开它。详见
[Vitulfile 中文说明](crates/vitul-cli/src/vitulfile/README-zh_CN.md)。Vitulfile 的
`app.icon` 和旧版 AgentOS 构建中的 `bundle.icon` 均支持 PNG 与 SVG 图标。

## AgentOS SDK

AgentOS SDK 是调用 AgentOS Gateway API 的客户端库。建议作为对应语言的应用依赖使用，不要把 SDK 源码复制进 Vitul。`vitul create` 生成的项目会在 `package.json` 中预置 TypeScript SDK；默认脚手架不会主动调用它：

| 语言       | 包                                                                | 适用场景                                             |
| ---------- | ----------------------------------------------------------------- | ---------------------------------------------------- |
| TypeScript | [`agentos-sdk-ts`](https://github.com/agentos-org/agentos-sdk-ts) | 需要调用本地或远程 AgentOS Gateway 的 Vitul Web 前端 |
| Dart       | [`agentos_sdk`](https://github.com/agentos-org/agentos-sdk-dart)  | Dart 客户端，包括 AgentOS Desktop 项目               |

Vitul 前端只有在需要调用 AgentOS Gateway API 时才需要在代码中导入 TypeScript SDK；默认脚手架通过 OpenTool 提供业务后端。此处只列出已经提供公开 GitHub 仓库的 SDK。SDK 不负责构建 AgentOS 系统镜像或打包 `.agt` 应用。

## 仓库结构

- `crates/vitul-cli/` 包含 Vitul 项目生成器、开发运行器和 AgentOS 打包器。
- `crates/tauri-*` 包含从 Tauri 派生的桌面运行时代码；Cargo package 和 Rust crate 名称为当前引擎保留。
- `packages/` 包含 JavaScript API 包（`@tauri-apps/api`）和 Vitul CLI 包（`@litevar/vitul-cli`）。
- `crates/create-vitul-app/` 是 `vitul create` 使用的内部脚手架 crate，源自上游 `create-tauri-app`，由 Vitul 仓库统一维护。

Vitul 在内部复用部分 Tauri 源码和工具。新项目使用 Vitul 文件名和命令；对用户公开的 CLI 入口为 `vitul`、`cargo vitul` 和 `create-vitul-app`。当前 JavaScript API 包为继承的运行时模块保留了 `@tauri-apps/api` 包名，但这不表示现有 Tauri App 或插件可以不经修改直接运行。Vitul 自有功能放在 `crates/vitul-*`；继承代码保留原有版权和许可证声明。

## 项目状态

Vitul 当前处于 1.0 之前的预览阶段。平台能力和验证范围见[支持矩阵](docs/SUPPORT-zh_CN.md)，版本与发布承诺见[维护规则](docs/MAINTENANCE-zh_CN.md)。外部问题反馈和代码贡献请通过 [GitHub 仓库](https://github.com/agentos-org/vitul)提交。
