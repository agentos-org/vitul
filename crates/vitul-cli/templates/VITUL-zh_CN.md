# Vitul 项目

[English](VITUL.md)

Vitul 在 macOS、Windows、Android 和 iOS 上使用系统 WebView。AgentOS 目标会将相同的 Web 前端和 Rust OpenTool 后端打包，并通过基于 Chromium 的嵌入式 WebView 显示页面。

## 实现一个用户需求

每项需求先明确目标平台和可观察的结果，再修改代码或配置。使用项目实际安装的 CLI、API
和宿主版本核对支持范围；相同名称的桌面、移动和 AgentOS 能力可能需要不同实现。

1. **描述行为**：谁在什么条件下做什么，成功、失败、重启和退出后应看到什么。
2. **选择能力**：判断由页面、OpenTool 后端、宿主 API 还是 AgentOS Gateway 承担；
   AgentOS 还要确定 process/remote-web 和 container/native，不根据单个字段随意切换。
3. **串起依赖**：逐项核对根配置、权限与授权、实现代码、资源/二进制、构建收集、
   启动入口、数据目录和退出清理。无需某一项时说明原因，不能省略未确认项。
4. **验证结果**：分别记录代码检查、配置校验、包内容检查和目标宿主上的行为验收。
   浏览器预览、桌面开发环境与安装后的 AgentOS 包不能互相代替。

| 用户需求 | 本项目的实现入口 | 要一起完成和验证的内容 |
| --- | --- | --- |
| 新增业务操作、处理数据或长任务 | `backend/src/tools/`、`src/opentool.ts` | 后端实现、前端调用、错误/进度；关闭页面后的任务需求由后端承担 |
| 保存并在下次启动恢复数据 | 后端数据读写；AgentOS 的 `agentos.data.persistent` | 声明持久化并实际使用 `DATA_DIR`，重启验收；不要写入包内目录 |
| 控制窗口、选择文件、剪贴板、通知、打开链接 | `src/host.ts`、`src/desktop-host.ts` | 对应宿主实现、权限、授权拒绝和不可用处理；不要虚构 HostProvider 方法 |
| 接入 AgentOS Gateway | `src/lib/agentos.ts` | SDK 调用、地址和认证；应用自身业务仍通过 OpenTool |
| 跟随语言或主题 | `src/host-context.ts`、`src/messages.ts`、`src/styles.css` | 初始值、运行时变化、文案和样式，切换时保留业务状态 |
| 修改名称、图标或窗口外观 | `vitul.conf.json`、`app-icon.png`、下文图标与命令说明 | 源资源、生成资源、目标平台配置；重新构建后验收 |
| 运行外部程序 | `bundle.externalBin`、`host.sidecar` 或后端启动逻辑 | 目标架构二进制、包收集、权限、实际启动与停止 |
| 改变关闭窗口后的行为、后台任务或系统集成 | 对应平台宿主能力和后端；AgentOS 的 `agentos.execution`、`permissions`、`providers` 等 | 支持范围、协议接入、进程生命周期、资源清理和宿主验收 |
| 修改 AgentOS 服务或发布配置 | `agentos.run`、`build`、`targets`、`rpc` | 后端监听、健康检查、路径映射、目标架构和包内结果 |

## 配置与排错

`vitul.conf.json` 是编辑入口。根 `schemaVersion: 2`、包内 `manifestVersion: 1`
和 OpenTool `rpc.version: 2` 是不同版本；不要把包内 `app.json` 当根配置复制。
`src-vitul/` 下的运行时配置会重新生成，应修改根配置和源资源。

AgentOS 配置更改后运行 `pnpm vitul agentos validate` 和 `pnpm vitul agentos inspect`，
检查最终端口、运行模式、权限和持久化状态。端口使用 `agentos.run.port`，
并与实际后端监听及其使用的 `agentos.run.env.PORT` 一致。
构建步骤中的项目路径和运行时包内路径不同：`build.artifacts` 的 `from` 是项目路径，
`to` 是包内路径，启动命令、参数和资源读取必须对应后者。

遇到“设置了但没生效”，按顺序定位：

1. 配置是否由当前命令和目标平台读取，字段是否属于当前版本，是否被另一字段或宿主覆盖？
2. 代码是否实现并调用了该能力，声明的权限是否在目标宿主获准？
3. 构建步骤是否生成目标资源，产物是否进入安装包，二进制是否匹配目标架构？
4. 启动入口是否实际启动相应组件，监听、路径、数据目录和环境是否一致？
5. 安装后的目标宿主是否执行了完整用户流程，包括失败、重启和退出？

完成报告按需求列出“实现入口、配置与依赖、已执行验证、未验证项”。
不支持或缺少宿主条件时明确说明，不能用相近功能替代用户要求并宣称完成。

## 应用图标

`app-icon.png` 是默认图标的铺满式方形源文件。设计新 Logo 时替换此文件，
让背景延伸到画布四边，不要添加外围透明留白、额外边框或预制圆角，以免在
Dock 中显得偏小。Logo 主体保留在画布内部，避免平台应用图标形状时被裁切。

在项目根目录重新生成各平台图标：

```sh
pnpm vitul icon app-icon.png --output src-vitul/icons
```

重新构建应用后生效。请修改源文件，再统一生成 PNG、ICO 和 ICNS，保持各分辨率一致。

## 语言和外观

默认起始页面支持中英文和浅深色主题。`index.html` 在 React 启动前初始化
`lang`、`data-theme` 和 `color-scheme`。`src/host-context.ts` 读取
AgentOS/WebShell 启动参数，并监听原生 `agentos:host` 上下文变更及父窗口的
`shell:theme` / `shell:locale` 通知，同时校验消息来源和父窗口 origin。
宿主的有效主题优先于本地系统主题；浏览器预览跟随浏览器语言和系统外观，支持动态切换。
界面文案放在 `src/messages.ts`，颜色使用 `src/styles.css` 中的语义变量。
通过 `useSyncExternalStore` 订阅上下文，切换时保留输入和后端调用结果。
后端结果、原始错误信息保持原文，应用标题保留配置的产品名称。

## 命令

- `pnpm install`：安装前端、AgentOS SDK、宿主 API 和项目本地 Vitul CLI。
- `pnpm vitul dev`：构建并启动 OpenTool 后端，使用操作系统分配的 loopback 端口，然后启动 Vitul 桌面应用。
- `pnpm vitul build`：通过兼容 CLI 构建桌面应用。
- `pnpm vitul android dev` 和 `pnpm vitul ios dev`：通过兼容移动 CLI 在设备或模拟器上运行应用。
- `pnpm vitul android build --apk`：构建 Android APK；使用 `--aab` 构建 Android App Bundle。`pnpm vitul ios build` 构建 iOS IPA。
- 桌面构建生成平台原生安装产物：默认情况下，macOS 生成 `.app` 应用包和 `.dmg` 磁盘映像；Windows 生成 `.msi` 安装包和 NSIS `.exe` 安装程序。可以在根目录 `vitul.conf.json` 的 `bundle.targets` 中选择桌面打包格式。
- `vitul build --target agentos/arm64`：构建 Web 资源，并在 Docker 中构建 ARM64 Linux 后端，最后把 AgentOS `.agt` 包写入 `dist/`。
- `vitul build --target agentos/x64`：构建 x64 Linux 后端和 `.agt` 包。
- 可安装的 AgentOS `.agt` 包使用 manifest v1，支持 ARM64 和 x64。OpenTool 使用独立的 `rpc.version: 2` 绑定版本。

预置模板通过官方 Vite 插件使用 Tailwind CSS 4。生成的 iOS 应用以 iOS 26.0
为最低系统版本，为模板提供现代 WebView 基线。

设置 `VITUL_AGENTOS_BUILD_IMAGE` 可以覆盖容器镜像；也可以修改 `vitul.conf.json` 中的 `agentos.buildImage`。

在根目录 `vitul.conf.json` 的 `agentos.open.titleBar` 中设置 `system` 或 `overlay`，选择 AgentOS App 内容相对于 Shell 标题栏的布局，默认值为 `system`。该设置会写入生成的 AgentOS `app.json`；宿主会报告实际生效的模式。Shell 管理的窗口会继续由 Shell 管理并绘制窗口控件。

AgentOS 的 `bundleId` 来自根配置的 `identifier`。可以在同一文件设置 `agentos.run.port`，指定范围为 1–65535 的容器内后端端口；默认值为 `9627`。
在根目录 `vitul.conf.json` 的 `bundle.icon` 中指定 PNG 或 SVG 文件，即可将其作为
`icon.png` 或 `icon.svg` 放入 AgentOS 安装包。
`vitul icon app-icon.png` 也会生成 macOS ICNS。已带 macOS 构图的图片使用
`--macos-mode passthrough` 保持原样。

## 应用 API

将业务操作放在 Rust 后端的 `backend/src/tools/`，并通过 `src/opentool.ts` 调用。脚手架中的 `greet` 示例使用 OpenTool JSON-RPC；`backend/src/lib.rs` 负责创建 OpenTool 服务。

项目已预置 `agentos-sdk-ts`。需要调用 AgentOS Gateway 时，从 `src/lib/agentos.ts` 导入 `createAgentOSClient()`；客户端只会在函数调用时创建。可将 `.env.example` 复制为 `.env`，设置 `VITE_AGENTOS_GATEWAY_URL` 指定网关地址。该地址会打包进前端，不能在 `VITE_*` 变量中存放密钥或其他机密。业务操作继续通过 OpenTool 后端实现。

`src/host.ts` 定义系统 API 边界；`src/desktop-host.ts` 将其映射到桌面和移动原生运行时，AgentOS provider 则调用宿主提供的 `window.agentos.host` 桥接接口。脚手架示例通过宿主 IPC 演示最小化窗口，AgentOS 仅请求 `native.window-control` 权限。只有应用确实需要时，才在 `vitul.conf.json` 中添加其他 `native.*` 权限。AgentOS 使用 `HOST=0.0.0.0` 和 `agentos.run.port` 启动容器内后端，并将分配的 loopback 宿主端口映射到该端口；同一个已安装 App 的多个窗口共享后端和宿主端口。前端在 `/` 提供，OpenTool RPC 在同一来源的 `/opentool` 提供。由 AgentOS WebShell 通过 `/apps/{appId}/` 托管时，生成的客户端会使用该代理前缀访问同一 App 后端。AgentOS App 的 OpenTool 请求体上限为 1 MiB；生成的 Rust 后端只在 AgentOS 环境应用此限制，其他宿主保留 OpenTool SDK 的 16 MiB 默认值。

Android 和 iOS 通过原生 IPC 调用嵌入 App 原生进程的 OpenTool 路由。请求继续使用 OpenTool JSON-RPC 消息格式，`streamCall` 事件通过原生事件通道传递；移动端不会启动 HTTP 服务。后端通过 `backend/` 链接进移动 App。桌面应用仍通过 HTTP 调用独立运行的 OpenTool 后端，桌面 IPC 处理器只向前端提供后端地址。

根目录的 `vitul.conf.json` 是项目配置。Vitul 会在 `src-vitul/` 下创建并忽略运行时配置投影，并在运行桌面或移动命令时更新它。生成的 `build.rs` 会为安装的 Rust 桌面 crates 准备其所需的配置文件名；请勿手动编辑 `src-vitul/` 下的生成文件。

业务 API 应放在 OpenTool 后端中，不要为每个业务操作额外创建一个原生命令。

## Sidecar

通过兼容的 `bundle.externalBin` 配置项声明 sidecar，配置文件位于根目录 `vitul.conf.json`。桌面构建会保留带目标后缀的 sidecar 文件；AgentOS 打包器会将对应架构的 Linux 二进制文件收集到 `.agt` 包的 `sidecars/` 目录中。

AgentOS 中的 sidecar 逻辑名称取自配置路径的 basename，必须匹配 `[a-z][a-z0-9-]{0,31}`；例如 `binaries/worker` 对应 `worker`。通过统一宿主 API 启动 sidecar：`host.sidecar.spawn("binaries/worker", ["--version"])`，并调用返回句柄的 `kill()` 停止进程。

若 AgentOS App 声明了 `bundle.externalBin`，请在项目根目录的 `vitul.conf.json` 的 `agentos.permissions` 中增加 `native.sidecars`。打包器会在 sidecar 已声明但权限未申请时中止打包；AgentOS Shell 仍会向用户请求授权。浏览器预览不支持 sidecar。
