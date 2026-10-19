# AgentOS Provider 接入

AndroidTone 在 `vitul.conf.json` 中声明自己的 Host Provider、外部 Wayland Provider 和内建 `org.agentos.desktop-session` Provider。宿主 socket 由 AgentOS 注入；应用只读取 `AGENTOS_PROVIDER_SOCKET`，不猜测路径或借用其他应用的 socket。

可信桌面身份通过 `org.agentos.desktop-session/session.get` 查询。应用要求响应中的 `available` 为 true，并验证 UID/GID 与当前进程一致、runtime 目录归属正确、D-Bus 地址指向当前用户所有的 Unix socket。AgentOS 报告会话缺失、过期或不明确时，安装后续步骤保持不可用。

启动图形会话时，AndroidTone 将该已验证的上下文交给自己的 Wayland 客户端；外部 Wayland Host Provider 注入私有 `XDG_RUNTIME_DIR` 与 `WAYLAND_DISPLAY`。客户端只连接这组平台分配的私有 socket，不连接或接管已有桌面 compositor。普通 Provider 继续以桌面用户运行并保持 `no-new-privs` 与空 capability。

2026-10-12 的 0.1.4 旧包曾遇到启动健康检查 HTTP 503；该历史问题不能代表后续版本。VM 后续曾能打开 AndroidTone 并读取 system-deployment 能力详情，但这些记录不验证当前 0.1.9 的 Native 启动、授权任务或 Wayland 会话链路；仍需在目标 VM 重新部署验收。

Provider 接通和桌面会话可用只代表应用能安全识别用户环境，不能证明宿主 Android 运行时已部署或窗口已呈现。0.1.9 查询通用部署 capability；只有模块加载、Binderfs 准备和离线包安装等完整操作都可用时才自动提交固定授权计划。Capability 查询本身不会申请管理员授权。Native 后端直接访问宿主 Waydroid 服务，不再使用 App 容器 Binder 设备分配；任务状态、镜像初始化、Waydroid 服务状态和 External Wayland 呈现仍需分层验收，见[自动准备说明](automatic-setup.md)。

## Android 启动预算

从申请私有显示会话开始，容器/session 就绪、Android framework 启动（`sys.boot_completed=1`）、写入并读回 `persist.waydroid.multi_windows=true`、一次用户会话重启和重启后 framework 检查共用 180 秒总时限。属性 CLI 使用剩余预算，不受普通查询命令的 15 秒限制。轮询不会重启容器；只有属性已验证后才执行一次既有会话重启。

总时限耗尽返回结构化 `waydroid-startup-timeout`，详情包含 `phase`、启动预算和最后观测。失败后 Waydroid session 清理最多等待 8 秒，随后 AgentOS 私有显示会话释放最多等待 12 秒；无论清理成功与否都会清除持久化的 display/session ID，保留原始启动错误，并等待用户手动重试。

Waydroid `session start` 是长驻进程。AndroidTone 在共享 180 秒启动期限内轮询 `sys.boot_completed=1`；framework 就绪后撤销该启动期限，并持续监督 CLI，直到它自然退出或收到 AgentOS/user stop 信号。就绪前退出、启动期限耗尽、就绪后异常退出和停止都保留有限时清理：调用 `waydroid session stop` 最多 8 秒，之后等待 CLI 2 秒，必要时终止并再等待 2 秒。子进程 stdout/stderr 持续排空并只保留有限大小的尾部，避免长驻进程因输出管道写满而卡死。

The private display session, Container/Session readiness, Android framework boot (`sys.boot_completed=1`), setting and verifying the multi-window property, and one user-session restart share a single 180-second startup deadline. Property commands use its remaining budget. Deadline failures keep the structured `waydroid-startup-timeout` code and phase diagnostics; cleanup is separately bounded (8 seconds for the CLI, 12 seconds for the display broker) and clears stale display handles. No container restart loop or automatic retry is introduced.

The Waydroid `session start` CLI is a long-lived process. AndroidTone supervises it while waiting for `sys.boot_completed=1` within the shared startup deadline; once the framework is ready, that deadline no longer applies to the live process. Supervision continues until natural exit or an AgentOS/user stop signal. Startup timeout, early exit, abnormal post-ready exit and stop all trigger bounded cleanup: `waydroid session stop` is limited to 8 seconds, followed by a 2-second child grace period and a bounded kill/reap. Child stdout and stderr are drained continuously into capped buffers so a long-running CLI cannot block on full pipes.
