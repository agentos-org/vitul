# WinTone 温易通

WinTone 是使用根目录 `vitul.conf.json` 配置的 AgentOS process 应用。首次启动时，WinTone 会在后台准备 Windows 运行环境。用户拖入 `.exe` 或 `.msi` 安装包后，WinTone 为安装创建独立环境，并通过 AgentOS Wayland 会话打开安装向导；安装完成后即可启动或卸载应用。

## 当前实现

- 使用 `DATA_DIR` 持久保存安装包、安装任务、共享运行时和独立 Wine prefix；上传时检查扩展名、文件头并计算 SHA-256。
- ARM64 `.agt` 自带两种架构路径所需的运行文件：x86/x64 应用在 x86-64 Ubuntu RootFS 中运行，ARM64 PE 文件使用原生 ARM64 路径。首次启动 WinTone 时会在后台校验并解包共享运行环境；每个安装有独立的 prefix 目录。
- 导入 `.exe` 时读取 PE Machine 字段，自动选择 x86/x64 或 ARM64 Wine prefix 和启动路径；ARM64EC/ARM64X 暂不支持。`.msi` 目前默认走 x86/x64 路径。
- 使用 AgentOS 的 `native.external-wayland` 会话 API 启动安装器和应用；ARM64 Windows PE 由原生 ARM64 Wine 启动，不经过 FEX。
- 通过 Wine prefix 中的卸载注册表识别应用入口和卸载程序；同一安装任务可登记多个应用入口。
- 监视和恢复 AgentOS 会话状态；管理界面提供应用启动、卸载和安装任务状态。
- 管理界面使用 React、TypeScript、Tailwind CSS、Vite 和 pnpm 构建，支持简体中文和 English，并提供跟随系统、浅色和深色主题；配色以 Windows 蓝色为主，偏好保存在当前浏览器中。
- 提供运行时准备、安装任务、应用列表 API，并暴露 `runtime.status`、`installations.list` 和 `apps.list` OpenTool 方法。
- 迁移旧版暂存安装包记录，并在温易通界面展示任务状态。
- `vitul.conf.json` 声明 AgentOS ARM64 和 x64 目标；Vitul 使用 Docker 为所选目标编译 Rust 后端并打包。

ARM64 包包含两条架构路径所需的运行环境；首次启动 WinTone 时会在后台从 `.agt` 解包到应用的持久数据目录，不要求用户预装到 AgentOS。它仍需 AgentOS 外部 Wayland 会话可用，且 Windows 应用兼容性会因应用而异。当前架构自动识别针对 `.exe`；`.msi` 仍走 x86/x64 路径。x64 目标尚未附带完整 Windows 运行环境。清单格式见 [示例](docs/runtime-catalog.example.json)。

## 目录结构

```text
backend/src/       Rust HTTP 与 OpenTool 服务、安装任务记录和运行时状态
frontend/src/      React、TypeScript、Tailwind CSS 界面与双语资源
frontend/dist/     Vite 生成的静态资源，打包时放入 public/
docs/              模块边界及后续运行时接入说明
vitul.conf.json    Vitul 统一项目与 AgentOS process 配置
```

## 构建

需要 Vitul CLI、Docker Buildx 和网络。ARM64 构建会从 Ubuntu arm64/amd64 软件源和 FEX 官方 PPA 组装双架构运行时，然后根据 `vitul.conf.json` 编译并打包 WinTone。

```sh
cd frontend
pnpm install --ignore-workspace
cd ..
vitul agentos validate
vitul build --target agentos/arm64
# 或 vitul build --target agentos/x64
```

详细模块边界见 [架构说明](docs/architecture-zh_CN.md)。


## 集成验证与平台依赖

External Wayland 是窗口接入协议，不代表 Wine 完整兼容性。WinTone 管理 Wine/FEX 和 prefix，AgentOS 提供通用隔离会话、授权和窗口呈现。启动脚本要求平台分配的私有 Wayland socket，并清除 DISPLAY。平台明确不提供 XWayland。WinTone 仅使用 Wine 原生 Wayland 后端；需要 Wine X11 后端才能正常运行的 Windows 程序，标为当前不支持，不回退 X11，也不申请 XWayland 能力。

进程运行或窗口对象创建不等于映射、Flutter 呈现、输入/焦点、缩放、多窗口、GPU 实际呈现或退出清理通过；必须分层记录证据，未知状态保持未验证。

参见 [集成审查与通用平台契约](../docs/tone-platform-integration-review.md)。
