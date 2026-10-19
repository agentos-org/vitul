# 首次自动准备

AndroidTone 0.1.18 以 desktop-user Native process 运行，首次启动时读取 AgentOS 的通用 `system-deployment` 能力。系统能力满足后，AndroidTone 会提交固定的 `android-runtime-setup` v3：Waydroid 与镜像安装包按精确版本和摘要绑定；宿主共享依赖通过 Waydroid 1.6.2 Debian 包内的 `Depends` / `Pre-Depends` 表达式校验，随包 `.deb` 仅作为离线候选。Native App 不需要容器专属的 Binder 设备分配。通过完整预检查后，任务才会进入 AgentOS Shell 等待管理员授权；预检查阻塞时不会出现密码窗口，也不会因刷新自动重提。详情会列出阻塞包及版本，用户修正清单或宿主状态后，可点击“修正后重新检查并提交”。

镜像安装包仍是精确绑定的应用组件。Waydroid 1.6.2 只自动识别 `/etc/waydroid-extra/images` 和 `/usr/share/waydroid-extra/images` 两个预装目录；把镜像留在 AndroidTone 私有目录不会被当作本地预装镜像，传入任意 `--images_path` 还会走在线 OTA 获取。若宿主已有不同版本的 `androidtone-waydroid-images`，AgentOS 的通用 APT 策略会阻止替换，任务保持阻塞；需要宿主先完成版本调整，或由 AgentOS 提供通用且经 Shell 确认的精确包升级能力。

安装包内包含 Waydroid 官方 2026-09-27 Android 13 `arm64_only` system/vendor 镜像。镜像校验会检查 vendor 的 BoringSSL 自检服务仍启用、64 位自检程序存在、32 位程序不存在，并确认 system/vendor 的 32 位 ABI 列表为空；因此不要求虚拟 CPU 提供 AArch32 EL0。当前 ARM64 QEMU VM 只报告 64-bit CPU op-mode，内核 `CONFIG_COMPAT=y` 不会为虚拟 CPU 增加 AArch32 指令执行能力。旧的通用 `arm64` 镜像若保留 ELF32 ARM `boringssl_self_test32`，就要求 VM/QEMU CPU 模型向 Android guest 暴露 AArch32 EL0；当前 VM 不满足这一条件。AndroidTone 检测到旧 rootfs 时会报告具体的 `Exec format error`/重启原因，提示更新固定版本镜像，再经 AgentOS 授权刷新 Waydroid 容器服务。不会删除或禁用 Android 加密自检；镜像更新保留 Waydroid 用户数据。

## 安装范围与授权

授权计划绑定当前已安装 AndroidTone 包的身份与 SHA-256，并列出包名、版本、架构、包内相对路径和每个 `.deb` 的 SHA-256。操作最多安装 512 个固定软件包；当前清单含 260 个 Ubuntu ARM64 Waydroid 运行时包及 1 个 AndroidTone 镜像包。APT 仅使用随应用固定的本地文件，不联网、不升级、不删除软件包。系统授权服务执行包维护脚本，普通 Provider 仍为 `no-new-privs`，不调用 `sudo` 或 `pkexec`。

同一授权事务按顺序加载宿主已安装的 `binder_linux` 模块、准备 Binderfs，然后安装软件包。Binderfs 和设备节点由宿主服务使用，不再映射进 App 容器。最后的镜像包维护脚本调用 Waydroid 的 `init`，从包内的 Android 13 system/vendor 镜像创建 Waydroid 配置。已有初始化数据会保留。AndroidTone 使用 AgentOS `task.get` 轮询同一任务。提交前会先保存幂等键，响应丢失后继续使用该键；已存在的待授权、运行中、完成、拒绝、取消和过期任务不会重新创建。查询任务失败时停止本轮操作并报告错误，不把它当作任务不存在。

当 AgentOS capability 不可用、宿主策略未允许请求的模块或 Binderfs、模块文件缺失或 system-deployment Provider 尚未注入时，AndroidTone 只展示具体平台原因；“检查系统状态”只重新读取能力，不会发出安装请求。管理员拒绝、关闭授权窗口或任务取消/过期后不自动再次申请；安装任务明确失败时，用户可以查看错误详情并显式重试。Shell 的关闭行为以平台返回的 denied/cancelled 状态为准。完整能力返回保留在状态接口的 `platformDependencies.systemDeployment` 中；主界面展示阶段和简短阻塞原因，详细 action state 和任务错误在可展开区域中。ENOSPC/No space left on device 归类为磁盘不足，不归类为授权失败。

## 用户主动重新授权

如果部署任务的 AgentOS 状态确实为 `denied`，页面会显示“在 AgentOS 中重新授权”。该按钮是唯一触发入口；启动、刷新、后台轮询和 OpenTool 不会调用重新授权，也不会重发 `operation.plan`。AndroidTone 只调用 `authorization.reauthorize`，参数严格为 `protocolVersion`、原拒绝任务的 `taskId` 和稳定的 `idempotencyKey`，不发送密码、操作定义、能力或参数。按钮调用经 AndroidTone Host Provider 转发到 AgentOS system-deployment Provider。

网络响应不确定时，幂等键先写入 AndroidTone 私有状态，再供同一次操作重试；有 `requestId` 后只轮询 `authorization.reauthorization.get`。`awaitingShellConfirmation` 等待 Shell 确认，`validating` 显示平台复核，`confirmed` 后切换到 AgentOS 返回的新任务并按普通 `task.get` 跟踪。Shell 拒绝、请求过期或平台复核失败会展示对应状态和原因并停止。流程结束后，只有再次点击才能开始新流程并生成新幂等键。AgentOS 保留原 `denied` 任务；能力和主机策略仍由 AgentOS 重新检查，重新授权不会绕过限制。

## 状态与边界

AndroidTone 分开报告内核配置、匹配模块安装、模块加载、Binderfs 挂载、离线包事务、Waydroid 初始化、宿主服务状态和私有显示会话。包任务完成不代表 Android 窗口已在 Shell 呈现。

部署后，AndroidTone 通过可信桌面会话 Provider 获取会话身份，并请求 AgentOS 为 Waydroid 客户端创建私有 External Wayland 会话。AndroidTone 不接管桌面 compositor，也不从 SSH 用户或 `/run/user/<uid>/bus` 推断会话。窗口映射、Flutter 实际呈现、输入/焦点、缩放、多窗口、GPU 实际呈现与退出清理仍需逐项在目标 VM 验收。

AndroidTone 的 Native 后端直接运行在宿主账户下；若 Waydroid 命令或系统服务不可用，状态会明确显示平台依赖。AndroidTone 不会从普通 Provider 提权启动宿主服务。完整窗口链路还依赖 AgentOS 已部署并启用对应的通用 Provider 契约。

## 已知 VM 检查记录

2026-10-12 的 0.1.4 包曾被 VM 安装器接受，但当时应用健康检查返回 HTTP 503 并回滚；该结果不代表后续版本。VM 后续截图显示应用 UI 可以打开并能读取系统能力详情。那次 0.1.7 源码和打包产物尚未在该 VM 上完成部署验证。

VM 曾分别观察到 Binder 配置、与运行内核匹配的模块文件和 memfd 可用；检查时模块尚未加载、Binderfs 未挂载且无设备分配。上述各项必须以新的系统授权部署任务逐一核对，不能从 `/dev/binder` 是否存在推断内核支持。

## 0.1.8 验证与空间阻塞

2026-10-13：本地 11 项 Rust 测试、24 项 Playwright 测试、TypeScript/Vite 生产构建及 ARM64 包校验通过。只修改 AndroidTone，operation.plan 继续按现有 schema 引用包内声明操作与空参数，不提交任意动作或命令。这是 0.1.8 的历史验证记录；当前 0.1.9 清单包含包安装、模块加载和 Binderfs 准备，不再包含容器设备分配。

本轮 VM 检查显示根文件系统只剩 175,607,808 字节（约 167 MiB），使用率 100%；新 AGT 为 1,452,933,023 字节，连上传及解包都无法安全完成，因此暂停安装和真实系统部署，没有创建新的 operation.plan。AgentOS 应用 API 报告已安装版本仍为 0.1.7、status=installed、无运行端口；待授权任务列表为空，Shell 位于登录页。新版本的授权窗口和最终部署状态尚未在 VM 复验。

此前 0.1.7 VM 实测四项 capability 均 available=true，AgentOS 修复回调后曾报告任务 65db6c85afb37-a16a899 进入 awaitingAuthorization 且 Shell 弹窗；这些是历史结果，不等于当前 0.1.9 的验证。继续前需要恢复足够空间（建议至少 8 GiB 可用，覆盖上传包、解包、离线缓存和镜像安装）；AndroidTone 不自行删除宿主数据、改变 socket 权限或调用 root 命令。

## 0.1.10：已安装环境的恢复

Waydroid 命令及初始化镜像已存在，但容器停止时，自动准备改为 `runtime.recover`，不重复安装离线包。恢复能力只检查 `linux.module.load`、`linux.binderfs.ensure` 和 `systemd.service.control`，并校验服务白名单包含 `waydroid-container.service`、支持 `restart`。不依赖 LXC 能力。服务显示 active 不能替代 Waydroid 状态检查。

包内固定声明 `android-runtime-recover` 第 1 版，参数为空；依次加载 `binder_linux`、准备 `binder/hwbinder/vndbinder`、重启 `waydroid-container.service`。后端只提交操作 ID/版本和幂等键，不能从 App 参数接受 unit 或命令。恢复请求使用独立的持久化幂等键和 task ID，通过 AgentOS Provider 查询同一任务；待授权和运行中不重复计划，拒绝、取消、过期停止自动准备，失败提供“重试恢复”。查询失败也不会清除任务或当作授权成功。

`awaitingAuthorization` 显示“等待系统授权”，`running` 显示恢复进度；完整 task error 保留在可展开详情。`completed` 后重新读取 Waydroid 状态。Waydroid 在用户 session 创建前可能只输出带制表符的 `Session: STOPPED`，不提供 Container 行；已完成恢复且当前 Binder 模块与 Binderfs 仍就绪时允许继续创建平台私有显示会话，不把这份输出误判为需要再次授权。若 Binder 在重启或升级后不再就绪，则重新请求恢复；若 Binder 已就绪但明确报告 Container STOPPED，则显示恢复失败并等待用户重试。部署完成和进程运行均不代表窗口已经呈现。

本地验证：17 项 Rust 测试、30 项 Playwright 测试、TypeScript/Vite 构建通过；ARM64 包校验通过，固定恢复声明、window 模式、Native 身份、运行时与前端资源完整性均已校验。真实 VM 授权结果另见下方最新验证记录。

### 2026-10-16 VM 真实恢复验证

VM `192.168.64.34` 安装前约有 6.8 GiB 可用空间，上传后约 5.4 GiB；通过 AgentOS 正常安装并启动 AndroidTone 0.1.10 成功。三项恢复能力均返回 `available: true`，服务白名单为 `waydroid-container.service`，支持 `restart`；LXC 两项仍不可用，且均标记为非必需。

应用自动提交的真实任务 `65dec8c24d506-22f5cd9a` 已读取到 `state=awaitingAuthorization`、`authorization.required=true`、`progress=0`。本次没有向 Provider 提供密码或调用授权批准 API。后续查询时任务已完成，Binder 模块和应用私有 Binderfs 挂载均已观测到；未捕捉 Shell 弹窗，不能宣称目视验证了弹窗。

复验发现，旧实现对完成后再次直接调用 `runtime.recover` 仍会创建任务 `65dec8f4dab62-613ec199`（该任务由 AgentOS 按已有授权完成）。最终实现补充当前状态保护：只有 `recovery-required` 或可重试的 `recovery-failed` 才允许提交，已就绪、待授权、运行中及拒绝/取消状态均返回原状态。该保护有独立单元测试。反复状态查询保持同一任务 ID，没有创建新计划。

这里只验证了恢复任务和 Binder 准备，不将 `session-ready` 视为 Android 应用窗口可用；Flutter 呈现、输入及 GPU 尚待实际窗口验收。

升级复验同时确认 AgentOS 会释放旧包的 Binderfs 资源，因此历史 completed 任务不能替代当前 Binder 就绪检查；状态逻辑及测试已覆盖该情况。显示会话启动另返回 HTTP 503，错误为 `AgentOS could not complete the Host Provider request`；没有更具体错误证据前，不归类为密码或授权失败，也不宣称窗口启动通过。

最终产物 SHA-256 为 `e95121c16509b820a50e29abc047d84933970d4a50ce6c8f5e96c3c9a51217f0`，AgentOS 安装与启动均成功；VM 后端二进制 SHA-256 与本地 ARM64 产物一致（`7d1a4db06275bc85ab834ccc941bf4b44a694212e1a81af32b97922fb90b1b8d`）。升级释放 Binderfs 后，新版正确报告 `recovery-required`，未受历史 completed 任务阻挡。

最终包创建任务 `65decbd7dbae1-27197f5f`，真实返回 `awaitingAuthorization`、`authorization.required=true`、进度 0；连续查询保持同一任务 ID。已通过 UTM 目视确认 AgentOS Shell“系统权限申请”窗口，包版本、SHA-256、固定 Binder 与服务操作均匹配。此任务保持待管理员决定，没有通过 Provider 提供密码或替用户批准。
