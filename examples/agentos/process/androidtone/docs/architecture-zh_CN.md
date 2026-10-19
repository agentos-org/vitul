# AndroidTone 集成架构

AndroidTone 0.1.9 是 desktop-user Native process App，由包内 Host Provider 管理 Android 运行时载荷、系统镜像、APK 和应用生命周期；后端运行在 Docker App 沙盒之外。AgentOS 提供内核能力、系统授权、可信桌面会话查询、Provider 注册、私有 Wayland 分配和 Flutter 窗口呈现。

普通 Provider 保持 `no-new-privs` 和空 capability 集，不调用 `sudo` 或 `pkexec`。AndroidTone 0.1.9 注册通用 system-deployment 方法，并声明一个按已安装应用摘要和逐包 SHA-256 固定的离线安装操作。AgentOS 校验包身份、软件包摘要、版本和架构，再向管理员请求授权执行固定的宿主 Binder 模块、Binderfs 与软件包动作；不再把 Binder 设备映射给 App 容器。AndroidTone 负责 Waydroid 业务和镜像；AgentOS 提供内核、系统授权、Provider 和窗口等通用能力。

包声明 `native.desktop-session`，并调用 `org.agentos.desktop-session/session.get`。只有 AgentOS 明确报告会话可用，且返回的 UID/GID、runtime 目录和 D-Bus socket 与当前进程身份匹配时，应用才使用这份上下文。它不读取 SSH 用户，不猜测 `/run/user/<uid>/bus`，也不依赖 `AGENTOS_SESSION_CONTEXT`。

启动 Android 画面时，AndroidTone 请求 `org.agentos.external-wayland` 创建私有会话。AgentOS 将分配的 `XDG_RUNTIME_DIR` 和 `WAYLAND_DISPLAY` 传给私有客户端；客户端检查对应 socket，并使用 `session.get` 返回且验证过的 D-Bus 地址。应用不会接管已有桌面 compositor 或 Wayland 会话。

内核配置、匹配模块安装、模块加载和 Binderfs 挂载分别报告；软件包安装、Waydroid 镜像初始化、宿主服务和显示会话状态也分别报告。进程启动不代表窗口已映射、Flutter 已呈现、输入和焦点有效、缩放正确、多窗口可用或 GPU 实际呈现。若宿主 Waydroid 命令无法访问或系统容器服务未运行，AndroidTone 会报告所缺的通用平台生命周期契约，不会从普通 Provider 提权。

参见[集成审查](../../docs/tone-platform-integration-review.md)、[自动准备状态](automatic-setup.md)和[离线载荷说明](offline-runtime-zh_CN.md)。
