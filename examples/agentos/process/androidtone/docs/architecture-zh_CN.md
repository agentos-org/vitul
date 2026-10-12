# AndroidTone 集成架构

AndroidTone 是容器化 process App，由包内 Host Provider 管理 Android 运行时载荷、系统镜像、APK 和应用生命周期。AgentOS 提供内核能力、系统授权、可信桌面会话查询、Provider 注册、私有 Wayland 分配和 Flutter 窗口呈现。

普通 Provider 保持 `no-new-privs` 和空 capability 集，不调用 `sudo` 或 `pkexec`。AgentOS 已提供版本化的通用系统部署 broker，用于执行应用声明的固定宿主动作。AndroidTone 当前还不会请求提权部署：现有动作集不能创建并分配 Binderfs 设备，也不能初始化和管理所需的 LXC 容器。平台补齐通用资源契约前，应用明确报告平台依赖，不修改宿主。

包声明 `native.desktop-session`，并调用 `org.agentos.desktop-session/session.get`。只有 AgentOS 明确报告会话可用，且返回的 UID/GID、runtime 目录和 D-Bus socket 与当前进程身份匹配时，应用才使用这份上下文。它不读取 SSH 用户，不猜测 `/run/user/<uid>/bus`，也不依赖 `AGENTOS_SESSION_CONTEXT`。

启动 Android 画面时，AndroidTone 请求 `org.agentos.external-wayland` 创建私有会话。AgentOS 将分配的 `XDG_RUNTIME_DIR` 和 `WAYLAND_DISPLAY` 传给私有客户端；客户端检查对应 socket，并使用 `session.get` 返回且验证过的 D-Bus 地址。应用不会接管已有桌面 compositor 或 Wayland 会话。

内核配置、匹配模块安装、模块加载、Binderfs 挂载和设备分配分别报告。进程启动不代表窗口已映射、Flutter 已呈现、输入和焦点有效、缩放正确、多窗口可用或 GPU 实际呈现。

参见[集成审查](../../docs/tone-platform-integration-review.md)、[自动准备状态](automatic-setup.md)和[离线载荷说明](offline-runtime-zh_CN.md)。
