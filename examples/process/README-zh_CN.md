# AgentOS process 应用示例

本目录集中放置 AgentOS process 应用。process 描述应用的打包和运行方式，OpenTool 是业务接口，
可以在不同应用类型中使用。两个示例都提供 OpenTool 方法。Toney Music 通过 Vitulfile 改造已有前后端；
Frame Video 同时也是 Vitul 桌面原生应用。

源码快照基于 AgentOS 提交 `a97448524`，来源为
`webshell/apps/toney-music`。该副本由 Vitul 仓库独立管理，不依赖相邻的 AgentOS 工作区。

## 示例

- [Toney Music](toney-music/README-zh_CN.md)：React/Vite 前端、Rust `musicd` 进程和运行时发现的
  OpenTool 方法。打包还需要通过 `FFMPEG_BIN` 指定 Linux ARM64 版 FFmpeg。
- [Frame Video](frame-video/README-zh_CN.md)：Vitul 原生应用和 AgentOS process 包，Rust OpenTool 后端管理本地视频库。

进入 Toney Music 目录后，使用 Vitul CLI 构建：

```sh
vitul agentos validate
vitul build --target agentos/arm64
```

Toney Music 构建需要 Node.js 和 pnpm 来构建前端、Rust 和 Cargo 来构建后端，以及 README 中说明的
FFmpeg。Frame Video 的桌面版和 AgentOS 版构建说明见其 README。
