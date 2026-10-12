# 首次自动准备

用户不需要管理运行时的内部组件。界面在平台明确报告 `canInstall`、`canInitialize` 或 `canStart` 后，依次请求安装、初始化和启动；每个阶段单次尝试，任务串行执行并读取真实状态。被拒绝、超时、能力缺失或已有未归属会话时不会自动重复请求。APK 仍可先导入保存。

## AgentOS v1 接入结果

AgentOS 已实现通用的版本化 system-deployment Provider 和 `native.desktop-session`。Vitul 现在可将 deployment 声明传入 `.agt` manifest，并校验两个内建 Provider 的权限与固定方法集。AndroidTone 已请求 `native.desktop-session`，后端通过 `org.agentos.desktop-session/session.get` 获取可信会话；Wayland 客户端验证 AgentOS 分配的私有 runtime/display socket，并使用同一可信会话的 D-Bus 地址。用户会话不再从 SSH 用户或 `/run/user/<uid>` 路径推断。

AndroidTone 还没有声明或调用 `native.system-deployment`。当前 AgentOS v1 动作无法完成 Android 运行环境所需的全部宿主工作：

- `linux.device.assign` 只能分配已经存在并经系统策略允许的 `/dev` 节点，不能挂载 Binderfs、创建 binder/hwbinder/vndbinder 设备或分配它们的所有权。
- `systemd.service.ensure` 以桌面 UID 运行，带 `NoNewPrivileges`、空 capability 和私有 `/dev`；它不能执行需要宿主权限的 LXC/Waydroid 初始化及容器生命周期。
- 离线 `.deb` 安装只接受与宿主已信任 APT 索引精确匹配的包文件；AndroidTone 不能把随包附带的 Waydroid key 自动加入宿主信任源。当前每个包安装动作最多 32 个包，安装闭包分批执行也不能保证离线依赖在一次 APT 事务中完整解析。
- `linux.bind-mount` 只把包内目录只读挂载到 AndroidTone 容器下一次启动的资源路径，不能把镜像初始化到 Waydroid 所需的宿主位置。

因此当前状态仍是 `platform-dependency`，不会弹出一份只能完成部分安装的授权，也不会对宿主做隐式修改。AgentOS 需要提供通用的受限 Binderfs 资源分配、离线包事务/可信来源和容器生命周期能力；这些契约不应包含 Waydroid 业务逻辑。之后 AndroidTone 才能声明完整、可复核的操作，并在系统返回任务完成后自行验证运行时状态。

## VM 部署验证

2026-10-12，VM 接受并安装了 AndroidTone 0.1.4，说明 AgentOS 能解析本次 `.agt` 的 `native.desktop-session` 权限及 Provider 声明。随后应用启动健康检查持续返回 HTTP 503，启动请求超时，AgentOS 回滚到先前版本；相同时段日志还报告容器名冲突，具体根因尚未定位。

由于应用后端没有稳定启动，本次没有验证 Provider socket 注入和 `session.get`。这也不是 Waydroid 安装或启动结果。此前对宿主的只读检查分别观察到 Binder 配置与匹配内核模块已安装、memfd 探测可用；当时 Binder 模块未加载、binderfs 未挂载且没有分配 Binder 设备。它们是不同状态，不能合并为“内核不支持”。

宿主验证需要继续分别记录 APT 可信索引匹配、软件包安装、Binderfs 挂载/设备归属、LXC 初始化/启动、Waydroid 进程、Wayland 窗口映射、Flutter 呈现、输入焦点、缩放、多窗口、实际 GPU 呈现和退出清理。进程启动、GPU 探测或软件渲染都不能代替窗口呈现验收。
