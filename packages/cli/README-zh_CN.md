# Vitul CLI

[English](README.md)

Vitul CLI 面向用户提供 `vitul` 命令。它负责运行 Vitul 应用生命周期命令，并在需要时使用从 Tauri 派生的桌面工具。

## npm 包

此包提供 `vitul` 和 `create-vitul-app` 命令包使用的原生 JavaScript 绑定。新项目也会将 `@litevar/vitul-cli` 安装为开发依赖，用于提供 `vitul` 项目脚本命令。

npm 发布所需的包配置已在此仓库准备好。发布前，维护者需要确认 `@litevar/vitul-cli` 及其所有目标平台包 `@litevar/vitul-cli-*`、`vitul` 和 `create-vitul-app` 的 npm 所有权。首次发布后，再为这些包配置可信发布者。在此之前，可从源码目录使用 `cargo install --path crates/vitul-cli --locked` 安装 Rust CLI。

项目命令使用 `vitul`，例如 `pnpm vitul dev` 和 `pnpm vitul build`。新 Vitul 项目不会提供 `tauri` 命令别名。

此 JavaScript 包通过 npm 分发 Vitul CLI。运行时边界说明请参阅[架构文档](../../ARCHITECTURE-zh_CN.md)。

## 许可证

继承代码保留其原有版权和许可证声明。
