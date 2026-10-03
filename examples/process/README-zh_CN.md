# 使用 Vitulfile 打包 AgentOS 应用的示例

这里是现有 AgentOS 应用的源码快照，整理为 Vitul `Vitulfile` process 打包示例，
用于生成可安装的 AgentOS `.agt` 包。它不是 Vitul 桌面原生项目，也不需要 Rust 中转服务。

源码快照基于 AgentOS 提交 `a97448524`，来源为
`webshell/apps/toney-music`。该副本由 Vitul 仓库独立管理，不依赖相邻的 AgentOS 工作区。

## 示例

- [Toney Music](toney-music/README-zh_CN.md)：React/Vite 前端和现有 Go `musicd` 进程。
  打包还需要通过 `FFMPEG_BIN` 指定 Linux ARM64 版 FFmpeg。

进入示例目录后，使用 Vitul CLI 构建：

```sh
vitul agentos validate
vitul build --target agentos/arm64
```

构建机需要 Node.js 和 pnpm 来构建前端；Toney Music 还需要 Go 以及 README 中说明的
FFmpeg。Frame Video 的 Vitul 原生移植见 [frame-video](../frame-video/README-zh_CN.md)。
