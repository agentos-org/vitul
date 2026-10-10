# WinTone 温易通

WinTone 是基于 Vitulfile 的 AgentOS process 应用。用户拖入 `.exe` 或 `.msi` 安装包后，WinTone 准备 Wine/FEX 与独立 prefix，通过 AgentOS Wayland 会话打开安装向导；安装完成后读取 Wine 卸载注册表发现应用，并提供启动和卸载入口。

## 当前实现

- 使用 `DATA_DIR` 持久保存安装包、安装任务、共享运行时和独立 Wine prefix；上传时检查扩展名、文件头并计算 SHA-256。
- 首次导入时自动读取运行时清单，下载并校验 Wine/FEX 包，随后为该安装分配独立 prefix 目录。
- 使用 AgentOS 的 `native.external-wayland` 会话 API 启动安装器和应用；ARM64 通过 FEX 调用 Wine，x64 直接调用 Wine。
- 通过 Wine prefix 中的卸载注册表识别应用入口和卸载程序；同一安装任务可登记多个应用入口。
- 监视和恢复 AgentOS 会话状态；管理界面提供应用启动、卸载和安装任务状态。
- 提供运行时准备、安装任务、应用列表 API，并暴露 `runtime.status`、`installations.list` 和 `apps.list` OpenTool 方法。
- 迁移旧版暂存安装包记录，并在温易通界面展示任务状态。
- Vitulfile 面向 AgentOS ARM64 和 x64；非原生目标通过 Docker 构建。

当前仓库还没有实际的 Wine/FEX 运行时包清单，因此无法在此仓库中真实运行 Windows 安装程序。AgentOS Wayland API 已接入，运行时包配置并发布后，导入任务会自动启动交互式安装向导。清单格式见 [示例](docs/runtime-catalog.example.json)。

## 目录结构

```text
backend/src/       Rust HTTP 与 OpenTool 服务、安装任务记录和运行时状态
public/            WinTone 管理界面
scripts/           AgentOS 目标架构构建脚本
docs/              模块边界及后续运行时接入说明
Vitulfile          AgentOS process 打包配置
```

## 构建

需要 Vitul CLI、Rust stable、Cargo；交叉构建需要 Docker。

```sh
vitul agentos validate
vitul build --target agentos/arm64
# 或 vitul build --target agentos/x64
```

外部 Wayland 表面协议尚未由 AgentOS 定义。接通 Wine/FEX 运行时和安装流程前，需要先确认 WinTone 与 AgentOS 的会话、窗口表面、输入及权限接口。

详细模块边界见 [架构说明](docs/architecture-zh_CN.md)。
