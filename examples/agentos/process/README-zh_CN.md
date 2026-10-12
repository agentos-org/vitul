# AgentOS process 应用示例

本目录集中放置 AgentOS process 应用包。process 描述应用的运行方式；示例使用不同的打包流程，涵盖原生宿主执行和容器沙盒。

新建或适配 WebApp 时，可先阅读[多语言与主题接入指南](docs/webapp-localization-theme-zh_CN.md)。

## 示例

- [Dockge](dockge/README-zh_CN.md)：将未修改的上游应用打包为原生 AgentOS 应用；打包脚本和上游源码位于同一目录。
- [Gitea](gitea/README-zh_CN.md)：打包官方 Gitea 二进制，并在容器沙盒中运行。关闭 Workspace 窗口后，tray helper 会让服务继续运行。
- [DeepSeek Harness](deepseek-harness/README-zh_CN.md)：将上游 Web 智能体打包为 Linux 容器应用，保存工作区数据，并适配首次登录 token。
- [ONLYOFFICE Docs](only-office/README-zh_CN.md)：构建一个小型容器沙盒适配应用，连接独立运行的官方 Community Docs 容器。
- [WinTone 温易通](wintone/README-zh_CN.md)：Windows 应用管理器原型，可导入并管理暂存的 EXE/MSI 安装包。
- [AndroidTone 安卓通](androidtone/README-zh_CN.md)：Waydroid Android 应用管理示例，包含 APK 导入、通用 Host Provider 授权和 Wayland 窗口接入。
- [Toney Music](toney-music/README-zh_CN.md)：通过 `vitul.conf.json` 打包 React/Vite 前端和 Rust `musicd` 后端，并提供运行时发现的 OpenTool 方法。
- [Frame Video](frame-video/README-zh_CN.md)：Vitul 桌面原生应用和 AgentOS process 包，使用 Rust OpenTool 后端管理本地视频库。

Toney Music 源码快照基于 AgentOS 提交 `a97448524`，来源为
`webshell/apps/toney-music`。该副本由 Vitul 仓库独立管理，不依赖相邻的 AgentOS 工作区。

进入 Toney Music 目录后，使用 Vitul CLI 构建：

```sh
vitul agentos validate
vitul build --target agentos/arm64
```

Toney Music 构建需要 Node.js 和 pnpm 来构建前端、Rust 和 Cargo 来构建后端，以及 README 中说明的
FFmpeg。Dockge 和 Gitea 使用各自的打包脚本；Frame Video 的桌面版和 AgentOS 版构建说明见其 README。
