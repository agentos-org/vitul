# Vitul CLI

[English](README.md)

Vitul CLI 面向用户提供 `vitul` 命令。它负责运行 Vitul 应用生命周期命令，并在桌面运行时需要时于内部调用兼容 Tauri 的工具。

## 使用方式

在 Vitul 项目中安装此包并使用 Vitul 命令：

```sh
pnpm add -D @vitul/cli
pnpm vitul dev
pnpm vitul build
```

项目命令使用 `vitul`，例如 `pnpm vitul dev` 和 `pnpm vitul build`。新 Vitul 项目不会提供 `tauri` 命令别名。

此 JavaScript 包封装 Rust CLI，兼容 npm、pnpm、yarn 和 bun。运行时边界说明请参阅[架构文档](../../ARCHITECTURE-zh_CN.md)。

## 许可证

继承代码保留其原有版权和许可证声明。
