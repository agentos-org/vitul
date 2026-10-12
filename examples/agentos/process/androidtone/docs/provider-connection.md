# AgentOS Provider 接入

AndroidTone 在 `vitul.conf.json` 中声明自己的 Host Provider、外部 Wayland Provider 和内建 `org.agentos.desktop-session` Provider。宿主 socket 由 AgentOS 注入；应用只读取 `AGENTOS_PROVIDER_SOCKET`，不猜测路径或借用其他应用的 socket。

可信桌面身份通过 `org.agentos.desktop-session/session.get` 查询。应用要求响应中的 `available` 为 true，并验证 UID/GID 与当前进程一致、runtime 目录归属正确、D-Bus 地址指向当前用户所有的 Unix socket。AgentOS 报告会话缺失、过期或不明确时，安装后续步骤保持不可用。

启动图形会话时，AndroidTone 将该已验证的上下文交给自己的 Wayland 客户端；外部 Wayland Host Provider 注入私有 `XDG_RUNTIME_DIR` 与 `WAYLAND_DISPLAY`。客户端只连接这组平台分配的私有 socket，不连接或接管已有桌面 compositor。普通 Provider 继续以桌面用户运行并保持 `no-new-privs` 与空 capability。

2026-10-12 的 AndroidTone 0.1.4 `.agt` 已被 VM 安装器接受，表明权限和 Provider 声明可解析；但 AgentOS 启动健康检查返回 HTTP 503 并回滚了应用。因此尚未确认运行中的 Provider socket 注入，也未取得 `session.get` 响应。此前“未挂载 `AGENTOS_PROVIDER_SOCKET`”是更早版本的历史快照，不代表本次接入后的状态。

Provider 接通和桌面会话可用只代表应用能安全识别用户环境，不能证明宿主 Android 运行时已部署或窗口已呈现。当前通用部署 v1 仍缺少 Binderfs 设备准备、完整离线依赖事务和 LXC 容器生命周期所需的动作，见[自动准备说明](automatic-setup.md)。
