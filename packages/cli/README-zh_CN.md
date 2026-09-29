# Vitul CLI

[English](README.md)

Vitul CLI 面向用户提供 `vitul` 命令。它负责运行 Vitul 应用生命周期命令，并在桌面运行时需要时于内部调用兼容 Tauri 的工具。

## 发布状态

`@vitul/cli` npm 包目前尚未发布。现阶段请从 Vitul 源码目录使用 `cargo install --path crates/vitul-cli --locked` 安装 Rust CLI。生成的项目会从 `PATH` 调用该 `vitul` 命令，不依赖此 JavaScript 包。

项目命令使用 `vitul`，例如 `pnpm vitul dev` 和 `pnpm vitul build`。新 Vitul 项目不会提供 `tauri` 命令别名。

此 JavaScript 包封装 Rust CLI，正在作为可选分发渠道准备中。运行时边界说明请参阅[架构文档](../../ARCHITECTURE-zh_CN.md)。

## 许可证

继承代码保留其原有版权和许可证声明。
