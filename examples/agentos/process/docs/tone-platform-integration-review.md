# Tone / AgentOS 集成审查与平台契约需求

日期：2026-10-11。来源：AndroidTone 的 Waydroid 集成及 WinTone 的 Wine/FEX 集成。目标是通用平台接口，而非由 AgentOS 内置运行环境业务逻辑。

## 职责与应用侧修正

| 责任方 | 范围 |
| --- | --- |
| AgentOS 系统平台 | 内核配置与匹配模块、桌面会话、系统授权、Provider 注册、隔离会话与窗口呈现 |
| AndroidTone | Waydroid 用户态组件安装计划、初始化/镜像计划、APK 和应用生命周期；通过平台申请受限管理员操作 |
| WinTone | Wine/FEX 及 RootFS 管理、prefix、安装程序、Windows 应用生命周期 |

AndroidTone 已移除直接 `pkexec` 调用和包内 root 安装脚本，保留普通 Provider 的权限边界。AgentOS 通用 system-deployment broker 已可报告平台能力；AndroidTone 0.1.6 只声明 `capabilities.get` 并读取这份只读报告，不请求 privileged deployment permission 或创建授权任务。安装、初始化、容器启动/停止和需 Android shell 的应用停止请求仍明确返回 `platform-dependency`，不回退 sudo、任意 root 命令或宿主脚本。APK 导入和任务持久化可独立使用。

只读内核状态始终返回（即使 Waydroid 尚未安装）：配置、模块安装、模块加载、binderfs 挂载、设备分配分别记录。不可读配置标记 unknown；未观察到设备不能推出内核 disabled。memfd 通过 Linux syscall 实测，不再从内核版本推断。Provider 的挂载命名空间可能不等于宿主全局命名空间，结果必须带观察范围；权威宿主清单是平台依赖。

会话命令只接受平台注入的可信上下文，拒绝从 HOME/USER、SSH 身份、服务账户或 UID 路径规则重建桌面环境。持久化运行状态绑定桌面 session ID，归属不匹配时不接管已有 Waydroid 会话。私有 Wayland 客户端另需平台分配的显示上下文；不使用继承的 DISPLAY/Wayland 地址。Waydroid 用户 session 的单实例约束仍是运行环境兼容性限制。

WinTone 的两个运行时启动脚本要求平台注入的 Wayland 地址及可用 socket，并清除 DISPLAY。新建会话不再提示“安装窗口已打开”；只有平台明确报告 mapped=true 后才进入等待用户操作状态。窗口对象创建、进程 running 和 Flutter 实际呈现属于不同层级。当前平台若不报告映射证据，任务保持 starting-installer，直到终止或后续获得证据。

## 通用平台契约（提案；不能视为已经实现）

### 可信桌面与私有显示上下文

普通 Provider 的进程环境可注入 `AGENTOS_SESSION_CONTEXT`，由平台从当前活动桌面会话取得，不允许由 HTTP 请求参数提供：

```json
{"version":1,"sessionId":"opaque-logind-or-platform-session-id","uid":1000,"gid":1000,"home":"/home/desktop-user","runtimeDir":"/actual/user/runtime","dbusAddress":"unix:path=/actual/user/bus"}
```

平台须验证活动会话、身份、目录/socket 的存在与访问权，并按实际上下文降权启动 Provider；注销/会话切换要撤销上下文和所属资源。安装用户、SSH 用户、Shell 服务身份不能作为默认桌面身份。应用验证版本、实际 euid/egid、runtime 所有者及 D-Bus socket；契约缺失即平台依赖。当前应用原型仅接受 unix:path 形式和同 UID 所有者；其它合法总线形式需先明确契约。

平台分配 Wayland 客户端时另注入 `AGENTOS_WAYLAND_CONTEXT`：

```json
{"version":1,"sessionId":"private-display-id","desktopSessionId":"same-desktop-context-id","runtimeDir":"/allocated/private/runtime","display":"allocated-wayland-name"}
```

平台负责私有 socket 分配、归属、访问控制与撤销；不得用当前桌面 compositor 的地址代替私有会话。应用在此契约落地前拒绝启动宿主 Wayland 客户端。上述环境变量是本次提案，不是现有 AgentOS 能力。

### 管理员操作

AgentOS 已实现独立于 no-new-privs Provider 的版本化系统授权 broker。无部署声明的 App 只可把 `org.agentos.system-deployment/capabilities.get` 用于只读探测；普通 Provider 不能提交管理员操作。获准的操作仍须声明已安装应用身份、平台批准的 operation ID、版本、按 schema 限制的参数、可信会话引用和请求 ID。禁止传入可执行路径、任意 argv、shell 字符串或任意环境变量；拒绝任意包内脚本以 root 运行。

平台可提供软件包事务、受限仓库来源、服务生命周期和经过审查的操作模板注册机制。应用拥有安装/初始化计划，平台校验计划并授权执行；不得由通用 broker 内置 Waydroid/Wine 业务分支。具体操作描述、可信模板来源、下载源/摘要验证及 manifest 声明 schema 仍需平台确定，应用不会把未经批准的字段塞入 manifest。

AndroidTone 需要注册以下应用操作标识（当前仅作为需求记录，未注册/未执行）：

| 标识后缀（前缀 com.agentos.androidtone.） | 允许参数/范围 |
| --- | --- |
| install-runtime.v1 | 空参数；经批准的 Waydroid 软件包及依赖、限定发行版仓库事务 |
| initialize-images.v1 | 空参数；批准的默认镜像来源、固定数据范围；不允许传入任意 init argv |
| start-container.v1 / stop-container.v1 | 空参数；仅应用运行环境的受限服务实例 |
| stop-application.v1 | 经 schema 校验的 Android package ID；固定 force-stop 模板，禁止任意 Android shell |

响应需要区分未提供、未注册、拒绝、用户取消、超时、执行失败和完成，提供进度、取消、审计与幂等语义。服务实例所有权由平台绑定调用应用和可信会话；stop 不得影响其它会话。当前 AgentOS v1 已提供固定动作集，但其 LXC/action schema 还不能完整表达 AndroidTone 运行 Waydroid 所需的镜像/会话挂载、网络、批准设备/capability 与容器内 APK 操作；AndroidTone 因而尚未声明上述操作并保持拒绝执行。

### 内核与窗口证据

平台的只读内核清单应包含：运行内核 release/config 来源，Binder IPC/binderfs/memfd 配置，匹配模块安装路径，模块加载状态，binderfs 挂载命名空间与挂载点，binder-control 与具体 binder/hwbinder/vndbinder 分配、权限及归属。模块加载、挂载和设备分配属于平台系统操作，应用状态查询不修改它们。

External Wayland 只承诺窗口接入协议。还需要显式的 session/window 状态或事件：进程启动、surface 创建、映射、首帧、Flutter presenter 确认、输入/焦点、scale、多个窗口、实际 GPU/CPU render path 和退出后的资源清理。应带 session/window ID、事件序列和时间，未知能力不能默认为通过。

Wine 的 Wayland driver 路径不等于兼容所有 Windows UI。平台明确不提供 XWayland，WinTone 仅使用 Wine 原生 Wayland 后端。需要 Wine X11 后端才能正常运行的 Windows 程序属于当前不支持的兼容范围；不回退 X11，不接管已有 X server，也不提出 XWayland 平台契约需求。GPU 能力探测、CPU 帧或软件渲染成功不构成 GPU 实际呈现证据。

## 可复现问题

1. 旧 AndroidTone 的 runtime.install 直接执行 pkexec，而 AgentOS 的 `_invokePackageProvider` 使用 setpriv --no-new-privs。两者冲突；已在应用侧移除。
2. 旧 Binder 探测只有 /proc/filesystems 和设备存在性，忽略尚未加载的已安装模块；旧 memfd 判断仅使用 kernel version。已分项修正。用户已确认 VM 支持 Binder IPC/binderfs/memfd 且模块与内核匹配，因此此前“VM 内核不支持”的结论撤回。
3. 当前 AgentOS `app_host_provider.dart` 拼接 /run/user/<uid>/bus，`external_wayland_session.dart` 的宿主客户端路径也拼接该地址。可对照登录会话清单与实际 socket 检查；这不能证明是当前桌面用户。应用现在等待可信上下文契约；需平台修正。
4. 原 WinTone 在 create 返回时提示 installer is open，原 AndroidTone 将 Waydroid 进程运行描述成已连接显示。现已区分过程状态，映射/呈现未证实。

可复现的只读检查（不执行 modprobe/mount）：

```sh
uname -r
grep -E '^(CONFIG_ANDROID_BINDER_IPC|CONFIG_ANDROID_BINDERFS|CONFIG_MEMFD_CREATE)=' /boot/config-$(uname -r)
modinfo -k "$(uname -r)" -F filename binder_linux
test -d /sys/module/binder_linux
findmnt -t binder,binderfs
ls -l /dev/binder /dev/binderfs /dev/hwbinder /dev/vndbinder
```

配置不可读时需要平台证据，不直接判 disabled。模块存在、加载、挂载和设备分配逐项记录，不用其中任一项替代其它项。

## 分层验证结果

| 层级 | 当前结果 | 证据/边界 |
| --- | --- | --- |
| 内核配置与匹配模块 | 用户已确认支持；本轮只读复核记录见下 | 撤回此前缺少设备即不支持的判断 |
| 模块加载/挂载/设备分配 | 分项观察；未由应用修改 | 未加载/未挂载属于运行状态 |
| 应用编译/安全边界 | 见本轮测试记录 | 受限管理员请求和缺少会话上下文拒绝 |
| ARM64 后端进程 | 旧包 smoke test 通过；更新包结果另记 | 不证明窗口可用 |
| 窗口映射 | 未验证 | 平台需要提供明确映射证据 |
| Flutter 实际呈现 | 未验证 | 必须在实际 Shell 观察首帧与窗口 |
| 输入/焦点、缩放、多窗口 | 未验证 | 需逐项交互测试 |
| GPU 实际呈现 | 未验证 | 不使用 GPU 探测或软件渲染替代 |
| 退出清理 | 未验证 | 需观察客户端/会话/texture/socket 清理 |

未完成的平台契约是平台依赖，不代表应用包已经实现完整 Waydroid/Wine 兼容性。


### 本轮 VM 只读复核

2026-10-11 对 192.168.64.34 再次执行上述只读检查，结果：

| 项目 | 结果 |
| --- | --- |
| 运行内核 | 6.8.0-100-generic |
| Binder IPC 配置 | CONFIG_ANDROID_BINDER_IPC=m |
| binderfs 配置 | CONFIG_ANDROID_BINDERFS=m |
| memfd 配置 | CONFIG_MEMFD_CREATE=y |
| 匹配模块安装 | /lib/modules/6.8.0-100-generic/kernel/drivers/android/binder_linux.ko.zst |
| 模块加载 | /sys/module/binder_linux 未观察到 |
| binderfs 挂载 | findmnt 未观察到挂载 |
| 设备分配 | 所检查的 /dev/binder、/dev/hwbinder、/dev/vndbinder 及 /dev/binderfs 节点未观察到 |
| memfd syscall | 创建并关闭匿名 fd 成功 |
| Waydroid 用户态 | command -v 未找到 |
| 会话上下文 | loginctl 有多个 active 会话；未据此选择桌面用户，可信上下文仍待平台提供 |

因此这是“内核能力存在、匹配模块已安装、运行准备尚未完成”，不应报告“内核不支持”。本轮未加载模块、挂载文件系统或分配 Binder 设备。

### 应用验证与制品范围

AndroidTone Rust 单元测试 5 项通过（配置/设备分离、挂载解析、受限参数、管理员操作拒绝、无效会话拒绝）；WinTone Rust 测试 8 项通过（含新增“进程 running / window.created 不证明 mapped”回归）。前端语法与两个运行时 shell 脚本语法通过，两个项目配置校验通过。

WinTone 本轮修改源代码与运行时构建脚本；旧的 Wine/FEX runtime 压缩包不会自动更新，需经标准运行时构建重新生成后才包含新的私有 socket 检查。本轮未宣称旧制品已包含这些修正，也未重新构建 WinTone .agt。


### 更新 ARM64 包在 VM 的验证

本轮在线构建遇到依赖索引超时；改用只读本机 Cargo 缓存在 Docker rust:1.90-bookworm / linux/arm64 中离线、locked 编译，再由 Vitul 标准 packager 打包（临时项目仅跳过重复后端编译，源项目配置未改变）。包内核实不含 root 安装脚本。

制品：androidtone/dist/androidtone-0.1.0-agentos-arm64.agt。SHA-256：ad159e2cefdc5a789da0340f7519faf7b771a066e2da5ff832756c316892b918。

已在 VM 通过 setpriv --no-new-privs 执行更新包：
- runtime.status 返回配置 binderIpc=module、binderfs=module、memfd=built-in，匹配模块 installed=true，同时加载 observed=false、挂载/设备 not-observed，memfdProbe=true。
- runtime.install 返回 platform-dependency，退出码 1，未尝试提权或安装宿主组件。
- 使用继承的 DISPLAY、WAYLAND_DISPLAY、XDG_RUNTIME_DIR 且不提供可信上下文启动客户端，明确拒绝，退出码 1。没有回退已有桌面。
- 两个 WinTone 启动脚本在只提供继承 DISPLAY、缺少平台私有 Wayland 环境时，均在 Wine 启动前拒绝。

这些是进程和安全边界验证；Waydroid 仍未安装，窗口映射、Flutter 实际呈现、输入/焦点、缩放、多窗口、GPU 呈现与退出清理全部未验证，需平台契约与实际 Shell 环境具备后继续。
