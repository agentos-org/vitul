# Frame Video：Vitul 原生 + OpenTool 示例

这是基于 AgentOS 提交 `a97448524` 中 Frame Video 源码
（`webshell/apps/frame-video`）改造的 Vitul 原生项目。项目保留 React/Vite 播放器界面，
将 Fenn Files 集成替换为 Vitul/AgentOS 宿主文件对话框。应用后端是 Rust OpenTool 服务；
安装后的应用不运行 Node.js 或 Python 服务。

## 示例内容

- Vitul 桌面应用和可安装的 AgentOS 应用共用同一套界面与 Rust 后端。
- OpenTool 使用 `library.list`、`library.add`、`library.remove`、
  `library.setFavorite` 和 `library.clear` 管理已保存的视频库。
- AgentOS 安装包启用运行时 OpenTool 发现。在 AgentOS 设置的“App 工具”中，
  用户可以逐个启用 `library.*` 函数，启用后 Chat 才能调用。
- Rust 服务通过字节范围响应提供本地视频，因此 WebView 可以正常 seek。
- 后端将视频库元数据写入 AgentOS `DATA_DIR` 或桌面用户的应用数据目录。移除列表项不会删除源视频。

## 构建和运行

在仓库根目录安装 JavaScript workspace 依赖，并构建 API 包和 Vitul CLI：

```sh
pnpm install
pnpm build:api
pnpm build:cli:debug
```

然后进入此目录运行：

```sh
pnpm vitul dev
pnpm vitul build
pnpm vitul build --target agentos/arm64
```

AgentOS 安装包输出到
`dist/frame-video-0.2.0-agentos-arm64.agt`。AgentOS 优先使用通用宿主文件对话框；所选文件必须对应用进程可见，
例如位于已挂载的用户 home 目录中。若宿主未开放该能力，应用会回退到浏览器文件选择器并在前端本地播放视频。
`agentos.bundleId` 保留原 AgentOS ID
`com.litevar.agentos.frameVideo`，中文名称和中英文描述也沿用原应用元数据。桌面版本使用 Vitul 原生文件对话框。

构建机需要 Node.js 和 pnpm 来运行 Vite，并需要 Rust 构建后端。安装包运行 Rust 后端和 WebView 资源，
不依赖 Python 或 Node.js 应用运行时。

## 源码结构

- `src/`：改造后的 React/Vite 播放器。
- `backend/`：OpenTool 视频库和支持字节范围的媒体服务。
- `src-vitul/`：Vitul 原生宿主和桌面文件对话框命令。
- `vitul.conf.json`：声明 OpenTool 后端和 AgentOS 文件对话框权限。
