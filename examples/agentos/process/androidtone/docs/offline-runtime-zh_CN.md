# AndroidTone 自包含运行时

此前 `.agt` 只有管理程序与界面，原计划通过宿主安装源获取运行时。移除普通 Provider 直接提权路径后，离线载荷尚未补齐。本次将运行时作为 AndroidTone 的包内容，不要求 AgentOS 预装 Waydroid。

## 包内内容与支持范围

首个完整载荷匹配 Ubuntu 24.04（noble）ARM64，包含官方 Waydroid Debian 包、完整 APT 依赖集合、Ubuntu 对应软件包源码、固定提交的 Waydroid 组件上游源码与仓库签名来源、Android 13 / LineageOS 20 VANILLA system.img 和 MAINLINE vendor.img。不包含 Google Apps。当前构建目标仅 `agentos/arm64`；其他发行版、架构需要单独构建并验证，不能复用此包。

`runtime/catalog.json` 记录每个文件的相对路径、大小、SHA-256、软件版本和宿主适用范围。镜像 ZIP 的原始校验值来自官方 OTA 元数据，锁定在 `runtime-build/images.arm64.lock.json`。解压只提取固定名称的 system.img/vendor.img。状态接口仅检查清单与文件大小，明确标为 `sizes-checked`；安装/初始化请求以及 `--verify-runtime` 校验全部文件 SHA-256。完整性校验不能代替平台对应用包来源、注册权限和授权操作的信任检查。

## 构建与验证

```sh
bash scripts/build-arm64-runtime.sh
COPYFILE_DISABLE=1 vitul build --target agentos/arm64
```

构建脚本在 Docker Ubuntu ARM64 容器中从已签名的官方 Ubuntu / Waydroid 仓库下载包。空 dpkg 状态用于计算依赖闭包，避免遗漏构建容器中已安装的库。包池、软件源码和镜像缓存在 `agentos-build/`，首次构建下载较大；镜像分块支持续传，最终必须匹配官方 SHA-256。软件包版本和 SHA-256 锁定在 `packages.noble-arm64.lock.json`；四个未提供 APT 源码包的组件使用 `upstream-sources.lock.json` 固定官方源码提交和归档摘要。仓库不再提供锁定版本时构建失败，需显式更新并重新验证，不能无声换版。构建提交清单前，另一个 `--network none` 容器从本地包池安装并执行 `apt-get check`、`waydroid --version`、`lxc-start --version` 和 Python 模块导入。

Linux 上解包后执行 `./androidtone-server --verify-runtime` 可独立复验所有载荷，文件缺失、篡改、路径越界或符号链接都会失败。Docker 中使用的服务启动抑制仅存在于测试容器，不能复制为宿主绕过授权方案。

## AgentOS 通用部署动作与当前缺口

AgentOS 已提供版本化系统部署 Provider，包含固定的 Debian 包安装、已签名内核模块加载、已存在设备分配、包目录只读挂载和受限 systemd 服务动作。AndroidTone 还没有声明该权限：现有动作无法完成整个 Android 运行环境，提前安装其中一部分会留下无法初始化或启动的宿主状态。

- Debian 包安装要求每个包与宿主已信任的 APT 索引精确匹配；随包附带的软件源密钥不会自动获得宿主信任。每个包安装动作最多包含 32 个包，AgentOS v1 没有一次性安装 260 个离线包闭包的单事务接口。
- 内核模块动作只加载已安装并由宿主策略允许的模块；设备动作只分配已存在且允许的 `/dev` 节点。它们不能挂载 Binderfs、创建 binder/hwbinder/vndbinder 节点或设置这些节点的归属。
- 服务动作以桌面 UID 运行，带 `NoNewPrivileges`、空 capability 与私有 `/dev`。它不能替代宿主级 LXC 容器初始化和生命周期管理。
- 只读 bind-mount 目标位于 AndroidTone 容器内，不会将系统镜像部署到宿主路径，也不会初始化 Waydroid 数据目录。

应用已接入可信桌面会话 Provider，并在 AgentOS 分配的私有 Wayland 会话中启动客户端；这不会授予 Binder 设备或 LXC 管理权限。包内载荷完整也不代表宿主已部署、Android 容器已运行或 Flutter 已呈现。应用不会添加宿主软件源、修改内核或以 root 执行包内脚本。

## 分层验收

包完整性、断网安装和用户态入口可以独立验证。宿主授权部署、Binder 模块加载/挂载/设备分配、可信用户会话、私有 Wayland、窗口映射、Flutter 呈现、输入焦点、缩放、多窗口、GPU 实际呈现和退出清理必须分别验收；缺少平台契约时这些层仍是平台依赖或未验证。
