# create-vitul-app

`create-vitul-app` 是 Vitul CLI 内部使用的前端模板引擎。应用开发者应
使用 `vitul init`（或 `pnpm vitul init`）创建项目；该命令会围绕所选前端模板
生成完整的 Vitul 项目。

当前 OpenTool 入门模板使用 `vanilla-ts`。随后 Vitul 会生成
`src-vitul/vitul.conf.json` 桌面配置、本地 OpenTool 后端、宿主 IPC 桥接代码，
以及平台相关的项目文档。

此 crate 位于 `crates/create-vitul-app`，由 Vitul 主仓库统一维护，源自上游
`create-tauri-app`。桌面 API 依赖保留兼容接口；继承源码中的版权与许可证声明保持原样。
