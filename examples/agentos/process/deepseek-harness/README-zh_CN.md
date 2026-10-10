# AgentOS DeepSeek Harness

本示例将 DeepSeek Harness（`dsh`）封装成 Linux AgentOS process 应用。它使用上游 Web profile，并通过上游 Python runtime 构建流程生成 Linux 单文件可执行程序。`.agt` 只包含可执行文件和所需运行时资源，不会再复制一份上游源码。

AgentOS 容器镜像中的 Node.js 版本低于 DSH 要求，因此包内包含对应的 Node 24 可执行程序、Python runtime、Office 依赖和 ripgrep。包内包含 AgentOS 使用的 PNG 应用图标。小型本地代理保留上游浏览器会话认证：它从 DSH 启动输出中私下读取登录 token，在代理内部完成首次访问的 303 cookie 交换，并在进程内保存签名会话 Cookie。由于 WebShell 不会把浏览器 Cookie 转发给应用，代理会将该 Cookie 附加到后续 API 和 WebSocket 请求中。若 AgentOS 提供 `AGENTOS_APP_PUBLIC_URL`，代理会让工作区页面启用 Host 持久化模型设置；构建时也会把插件声明的 SVG 图标打入可执行文件。启动 token 不会写入 AgentOS 日志。

请在 Vitul 仓库根目录一次构建一个 Linux 架构：

```sh
DSH_PLATFORM=linux/arm64 ./examples/agentos/process/deepseek-harness/build.sh
DSH_PLATFORM=linux/amd64 ./examples/agentos/process/deepseek-harness/build.sh
```

默认源码目录为 `../GitHub/deepseek-harness`，也可通过 `DSH_SOURCE_DIR` 指向其他 checkout。构建会在临时 Linux 容器内复制源码、按锁文件安装依赖、构建 DSH 与 Web 资源，再组装 `.agt`；不会修改上游 checkout。构建需要 Docker、容器网络和足够的磁盘空间。`DSH_BUILD_IMAGE` 可指定 Linux 构建镜像，`DSH_OUTPUT` 可更改产物路径。默认产物为 `examples/agentos/process/deepseek-harness/dist/deepseek-harness-arm64.agt` 和 `deepseek-harness-x64.agt`。

构建会在 `build-info.json` 中记录 DSH 版本和 Git commit，并保留上游 MIT 许可证及第三方声明。通过 Agent Store 安装 `.agt`。DSH 设置、凭据、会话、缓存和可写工作区保存在应用持久化 `DATA_DIR` 下。首次运行会打开 DSH Web UI；创建 Agent 前请先在设置中配置模型提供方。自动启动默认关闭。

应用运行于 AgentOS 容器沙盒中。DSH 还会使用自身的 Linux 沙箱，将 Agent 命令限制在持久化工作区。需在目标 AgentOS 设备上确认嵌套命令沙箱与内核兼容。
