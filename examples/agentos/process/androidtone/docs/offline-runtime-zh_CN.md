# AndroidTone 自包含运行时

此前 `.agt` 只有管理程序与界面，原计划通过宿主安装源获取运行时。移除普通 Provider 直接提权路径后，离线载荷尚未补齐。本次将运行时作为 AndroidTone 的包内容，不要求 AgentOS 预装 Waydroid。

## 包内内容与支持范围

首个完整载荷匹配 Ubuntu 24.04（noble）ARM64，包含官方 Waydroid Debian 包、完整 APT 依赖集合、Ubuntu 对应软件包源码、固定提交的 Waydroid 组件上游源码与仓库签名来源、Android 13 / LineageOS 20 VANILLA system.img 和 MAINLINE vendor.img。不包含 Google Apps。当前构建目标仅 `agentos/arm64`；其他发行版、架构需要单独构建并验证，不能复用此包。

AndroidTone 使用 Waydroid 官方 `arm64_only` 镜像构建 20260927（system SHA-256 `a7806741ea8ca5be917d0c66b031d97a60abb7a95031ca5b35c40620b6273ccd`；vendor SHA-256 `bd502c53ada5019e0fad3beb6feb248ce45c9ea500870f4725e0d12624baef0a`）。打包验证直接检查镜像内容：vendor 保留 `reboot_on_failure` 加密自检服务且包含 64 位程序，没有 ELF32 ARM `boringssl_self_test32`；system/vendor 的 32 位 ABI 属性为空。这套随包镜像只要求 AArch64 EL0。用户当前 VM 报告 CPU op-mode 只有 64-bit，所以旧的通用 `arm64` vendor 镜像不能用于该 VM；若自行换回含 32 位 ARM 自检程序的镜像，VM/QEMU CPU 必须向 guest 暴露 AArch32 EL0。内核 `CONFIG_COMPAT=y` 只描述 Linux 兼容配置，不能代替 guest CPU 的 AArch32 执行能力。

`runtime/catalog.json` 记录每个文件的相对路径、大小、SHA-256、软件版本和宿主适用范围。镜像 ZIP 的原始校验值来自官方 OTA 元数据，锁定在 `runtime-build/images.arm64.lock.json`。解压只提取固定名称的 system.img/vendor.img。状态接口仅检查清单与文件大小，明确标为 `sizes-checked`；安装/初始化请求以及 `--verify-runtime` 校验全部文件 SHA-256。完整性校验不能代替平台对应用包来源、注册权限和授权操作的信任检查。

## 构建与验证

```sh
vitul agentos validate
COPYFILE_DISABLE=1 python3 scripts/package-prebuilt-arm64.py
```

该入口会用 Docker 构建当前源码的 ARM64 后端和运行时，再依据 `catalog.json` 校验每个离线包并同步固定授权清单，最后由 Vitul 标准打包器生成 `.agt`。不要单独运行 Vitul 打包后直接复用静态部署清单；更换运行时包时应通过此入口让包内目录与授权摘要保持一致。

构建脚本在 Docker Ubuntu ARM64 容器中从已签名的官方 Ubuntu / Waydroid 仓库下载包。空 dpkg 状态用于计算依赖闭包，避免遗漏构建容器中已安装的库。包池、软件源码和镜像缓存在 `agentos-build/`，首次构建下载较大；镜像分块支持续传，最终必须匹配官方 SHA-256。软件包版本和 SHA-256 锁定在 `packages.noble-arm64.lock.json`；四个未提供 APT 源码包的组件使用 `upstream-sources.lock.json` 固定官方源码提交和归档摘要。仓库不再提供锁定版本时构建失败，需显式更新并重新验证，不能无声换版。构建提交清单前，另一个 `--network none` 容器从本地包池安装并执行 `apt-get check`、`waydroid --version`、`lxc-start --version` 和 Python 模块导入。

Linux 上解包后执行 `./androidtone-server --verify-runtime` 可独立复验所有载荷，文件缺失、篡改、路径越界或符号链接都会失败。Docker 中使用的服务启动抑制仅存在于测试容器，不能复制为宿主绕过授权方案。

## 当前 AgentOS 通用部署动作（AndroidTone 0.1.18）

当前固定的 `android-runtime-setup` v3 采用 `debian.packages.ensure` schema v1。Waydroid 1.6.2 与 `androidtone-waydroid-images 0.1.8-1` 是精确绑定的应用组件；259 个宿主共享包仅作为离线候选，兼容条件采用该精确 Waydroid Debian 包声明的 `Pre-Depends` / `Depends`，由 APT 按 Debian 语义解析。与旧版 `install-bundle` 把 261 个软件包全部要求精确版本相比，已安装且满足 Waydroid 声明依赖的宿主版本不会仅因与离线候选版本不同而冲突。AgentOS 仍负责预检查、禁止升级/降级/卸载和 Shell 管理员授权；AndroidTone 不自行执行 APT 或系统命令。

镜像初始化包写入 Waydroid 支持的系统级预装镜像目录，不能放在 AndroidTone 私有数据目录后仍假定 Waydroid 会本地识别。目标 VM 已有版本不同的 `androidtone-waydroid-images 0.1.7-1`，因此当前精确组件要求 `0.1.8-1` 会被 AgentOS 通用策略阻止。要继续升级只能先调整宿主版本，或由 AgentOS 增加适用于所有应用的 Shell 确认精确包升级能力；不能把镜像伪装成另一包名或复用任意系统权限。

以下 `0.1.9` 描述是早期清单的历史说明，不代表 0.1.18 当前 action schema：

## 历史部署动作（0.1.9）

AndroidTone 0.1.9 以 desktop-user Native 模式声明一次性 `android-runtime-setup` 部署：加载由宿主内核提供且策略允许的 `binder_linux` 模块、准备宿主 Binderfs，然后通过 `debian.packages.install-bundle` 安装清单内的离线包。Native 后端运行于宿主账户，不需要将 Binder 设备映射到 App 容器。当前该动作最多携带 512 个逐包摘要固定的软件包；AndroidTone 清单为 260 个 Ubuntu ARM64 Waydroid 软件包加 1 个镜像初始化包。每个包都绑定名称、版本、架构、应用包内相对路径和 SHA-256。该操作需要 AgentOS 管理员针对已安装应用摘要批准；APT 禁止联网、升级和删除。系统授权服务执行维护脚本，普通 Provider 不提权。

镜像初始化包把固定的 Android 13 system/vendor 镜像放到 Waydroid 支持的预装镜像路径，并在首次配置时调用 Waydroid `init`；该命令沿用 Waydroid 自带逻辑读取预装镜像并生成配置。已初始化的 Waydroid 数据不会被覆盖。包构建在禁网 ARM64 Docker 容器中测试离线依赖安装和初始化维护脚本；Binder 设备在这项测试中由临时文件模拟，因此不能替代目标 VM 上的真实模块、Binderfs 和设备验证。

AndroidTone 继续使用可信桌面会话与 AgentOS 分配的私有 Wayland 会话。离线载荷不包含宿主内核模块，也不改变宿主软件源。部署完成不等于 Android 应用窗口已由 Flutter 呈现；进程启动、映射、输入/焦点、缩放、多窗口、GPU 实际呈现和退出清理都要单独验收。若宿主 Waydroid 命令不可用或系统容器服务未运行，仍需要相应的通用平台生命周期接口；AndroidTone 不会以 root 自行启动宿主服务。

## 当前部署清单与版本冲突

AndroidTone 0.1.18 将宿主共享依赖改用 AgentOS 通用 `debian.packages.ensure` schema v1。Waydroid 1.6.2 与镜像初始化包仍是精确应用组件，分别绑定包名、版本和 SHA-256；其余 259 个 Ubuntu ARM64 包作为离线候选，兼容条件取自已固定 Waydroid Debian 包的 `Pre-Depends` 与 `Depends`，由 APT 按 Debian 规则解析。包池只是候选集，不表示会全部安装。构建不会把未验证的版本范围自行放宽。

`androidtone-waydroid-images` 必须安装到 Waydroid 支持的系统预装镜像目录。Waydroid 1.6.2 没有把 AndroidTone 私有目录识别为本地镜像的接口；非标准路径会转为在线 OTA 获取，因此本包不能靠私有镜像目录规避宿主组件版本冲突。VM 当前安装 `androidtone-waydroid-images 0.1.7-1`，AndroidTone 请求 `0.1.8-1`；AgentOS 通用策略禁止该精确应用组件被升级或替换，所以预检查会返回 `preflight-blocked`，不会进入 Shell 授权队列。需要 AgentOS 提供面向通用应用的、经 Shell 确认的精确包升级能力，或先通过宿主维护调整版本，再手动重新提交计划。AndroidTone 不会改用 apt/shell 或伪造包身份。

本轮只读 VM 检查：Waydroid 1.6.2 已安装，Waydroid 容器服务为 `active`，Binderfs 已挂载，但 Waydroid Session 为 `STOPPED`；根分区约有 7.1 GiB 可用空间。由于镜像组件版本冲突，本轮未重新提交真实部署计划，也未请求授权。无冲突时的 `awaitingAuthorization` 由前端回归夹具验证，不代表 VM Shell 已实际显示授权窗口。Waydroid Session `RUNNING` 仍需镜像更新能力、Shell 授权和后续 VM 验收。

## 分层验收

包完整性、断网安装和用户态入口可以独立验证。宿主授权部署、Binder 模块加载/挂载、可信用户会话、私有 Wayland、窗口映射、Flutter 呈现、输入焦点、缩放、多窗口、GPU 实际呈现和退出清理必须分别验收；缺少平台契约时这些层仍是平台依赖或未验证。
