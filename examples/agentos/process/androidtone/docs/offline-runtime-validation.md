# AndroidTone 离线运行时验证记录

## 0.1.18 宿主依赖预检查与镜像版本阻塞

日期：2026-10-16。ARM64 包 `dist/androidtone-0.1.18-agentos-arm64.agt`，1,266,101,386 字节。SHA-256：

```text
4612defdd8ccd573fa2e4e09620cd087fa749951785136e82c5280009e999db1
```

部署操作升级至 v3，使用 AgentOS 通用 `debian.packages.ensure` schema v1。Waydroid 1.6.2 和 `androidtone-waydroid-images 0.1.8-1` 仍是精确应用组件；其余 259 个候选包按固定 Waydroid Debian 包的 `Pre-Depends` / `Depends` 做宿主兼容性检查。归档校验器确认包身份、架构、摘要及 815 个归档文件，并确认 ARM64、window 打包模式和最新前端资源。

| 验证层 | 结果与范围 |
| --- | --- |
| 后端回归 | `cargo fmt --check` 通过；31 项 Rust 测试通过 |
| 前端回归 | TypeScript/Vite production build 通过；43 项 Playwright 测试通过，覆盖无冲突等待授权、包版本冲突、依赖不满足、结构化中文诊断和阻塞后不自动重提 |
| ARM64 构建 | Docker release 后端为 ELF64 AArch64，5,217,776 字节；禁网 Ubuntu ARM64 容器中的 `--verify-runtime` 校验全部包内载荷 SHA-256 |
| `.agt` | 261 个 Debian 包、2 个 Android 镜像、815 个文件；1,266,101,386 字节；归档路径安全、macOS 元数据缺失、前端和运行时摘要校验通过 |
| 离线安装事务 | 本次重用既有已构建运行时，归档校验本身不执行 APT 安装测试 |
| VM | 只读检查：Waydroid 1.6.2；镜像包仍为 `0.1.7-1`；Binderfs 已挂载；`waydroid-container.service` 为 `active`；Session 为 `STOPPED`；根分区可用约 7.1 GiB |
| AgentOS 授权与会话 | VM 上旧镜像版本与新包精确组件不一致，不能提交新的真实 `operation.plan`；本轮没有任务进入 AgentOS Shell `awaitingAuthorization`，也没有 Shell 确认或 Waydroid `RUNNING` 结果。Playwright 的无冲突测试只验证 UI/状态跟踪，不代表真实授权 |

当前 VM 的 `androidtone-waydroid-images 0.1.7-1` 与包内精确要求 `0.1.8-1` 冲突。共享依赖改为 `ensure` 后不会再因为 259 个离线候选包的版本字符串不同而全部冲突，但通用策略仍禁止替换镜像应用组件。继续真实部署需要 AgentOS 提供适用于所有应用、经 Shell 确认的精确包升级能力，或先由系统维护完成版本调整。Waydroid 1.6.2 只将系统级支持目录识别为预置镜像，无法用 AndroidTone 私有目录规避该冲突。

## 0.1.17 镜像更新提示修正

日期：2026-10-16。最终包 `dist/androidtone-0.1.17-agentos-arm64.agt`，1,266,073,867 字节。SHA-256：

```text
a178e0a709d20d1aeb5255ff32961be51849f6855d26565f195f888eab23ecff
```

修复旧镜像仍在使用时的操作提示：如果 AndroidTone 检测到镜像与 CPU 不兼容且运行环境尚未停止，页面会明确要求先停止运行环境，不再显示只会刷新的“更新 Android 镜像”按钮。停止后由既有状态机提交固定镜像更新请求，授权仍由 AgentOS Shell 处理。

| 验证层 | 结果与范围 |
| --- | --- |
| 前端回归 | 检查运行环境尚未停止时显示停止指引、隐藏误导性更新/刷新按钮；点击停止后仅触发一次镜像安装请求 |
| 前端构建 | TypeScript 检查与 Vite production build 通过；Playwright 41 项通过 |
| ARM64 后端 | Docker `rust:1.90-bookworm` release 编译通过，产物为 ELF64 AArch64 |
| ARM64 `.agt` | Vitul 标准打包和 `verify-package.py` 通过；815 个文件、1,266,073,867 字节，窗口模式，前端/运行时 SHA-256 校验通过 |
| VM | 未安装、未授权；由用户手动操作 |

## 0.1.16 ARM64-only 镜像兼容与启动错误诊断

日期：2026-10-16。最终包 `dist/androidtone-0.1.16-agentos-arm64.agt`，1,266,074,537 字节。SHA-256：

```text
8e1449da7e36d0ef2d4266f3e0f4973a65cb42ee249c351a528a9e81b285c4d7
```

VM 的 guest CPU 报告 `aarch64` 且 `CPU op-mode(s): 64-bit`；当前配置不能执行 ELF32 ARM 程序。内核 `CONFIG_COMPAT=y` 不改变 guest CPU 能力。随包镜像改为 Waydroid 官方 `arm64_only` 2026-09-27 版本：system ZIP SHA-256 `a7806741ea8ca5be917d0c66b031d97a60abb7a95031ca5b35c40620b6273ccd`，vendor ZIP SHA-256 `bd502c53ada5019e0fad3beb6feb248ce45c9ea500870f4725e0d12624baef0a`。ARM64 Docker 使用 `debugfs` 实查镜像：vendor 的 `boringssl_self_test32_vendor` 服务仍包含 `reboot_on_failure`，但 32 位二进制不存在、64 位自检程序存在；system/vendor 的 32 位 ABI 属性为空。没有删除或禁用加密自检。

| 验证层 | 结果与范围 |
| --- | --- |
| ARM64 后端 | `rust:1.90-bookworm`、`--network none` 下 `cargo test --locked --offline`：29 项通过，包含 AArch32 ELF/CPU mode 检测和超时终止测试；`cargo fmt --check` 通过；release 输出为 ELF64 AArch64 |
| 前端 | TypeScript 检查、Vite production build 通过；Playwright 41 项通过，包含只显示具体 CPU/`Exec format error` 兼容诊断、不在刷新时提交安装，以及停止已有会话后才更新镜像 |
| ARM64 离线运行时 | 260 个常规 Debian 依赖包与 1 个 `androidtone-waydroid-images 0.1.8-1` 镜像包；禁网安装、`apt-get check`、Waydroid/LXC 版本和 Python imports 检查通过 |
| 镜像安装脚本 | ARM64 Docker 中用临时 Binder 文件夹具验证 Waydroid 初始化维护脚本及镜像路径；另逐项检查包内 system/vendor 镜像、元数据、32/64 位自检程序和 ABI 属性。测试夹具不代表真实宿主 Binder 验证 |
| 最终 `.agt` | Vitul 标准打包器；815 个文件、1,266,074,537 字节；窗口模式、ARM64；归档路径安全，macOS 元数据缺失，所有运行时和前端资源哈希校验通过 |
| VM 部署 | 按用户安排，本轮未安装 `.agt`、未发起 AgentOS 授权，也未启动 Waydroid；Container 和 Session 均为 RUNNING 尚待用户手动验收 |

构建包位于 `dist/androidtone-0.1.16-agentos-arm64.agt`，SHA-256 同时写入 `.agt.sha256`。本次没有在 VM 上安装包或操作宿主服务。

## 0.1.8 通用授权契约更新

当前包 `dist/androidtone-0.1.8-agentos-arm64.agt`，1,452,933,023 字节。SHA-256：

```text
d6c6cb244f1e4944350f60481f94d80c664625400e8e86f76dc78855be05ddc7
```

ARM64 Docker 后端编译、Vitul window 模式打包、261 个离线包完整性检查通过；11 项 Rust 与 24 项 Playwright 测试通过。测试覆盖能力阻塞、等待授权不重复提交、拒绝/取消、任务成功/失败及 ENOSPC 分类。VM 根分区仅剩约 167 MiB，安装暂停；本轮未提交 operation.plan，也未批准或执行真实系统安装。VM 安装版本仍为 0.1.7，待授权队列为空，历史任务最终状态未能通过当前应用接口复验。详细检查见 [自动准备](automatic-setup.md)。

## 0.1.7 自动准备修复与 ARM64 包构建

日期：2026-10-13。当前包 `dist/androidtone-0.1.7-agentos-arm64.agt`，1,452,927,135 字节。SHA-256：

```text
5105a3608517e2680884e9755ac96eafb028499bdc8be9f42e1fb334f104fd1c
```

| 验证层 | 结果与范围 |
| --- | --- |
| ARM64 后端 | Docker `rust:1.90-bookworm` 从当前 0.1.7 源码以 `--locked --release` 编译；ELF64 AArch64 |
| 后端检查 | 9 项 Rust 测试通过，`cargo fmt --check` 通过 |
| 前端 | TypeScript 检查、Vite production build 通过；Playwright 19 项通过，覆盖中英文、明暗主题、系统自动主题变化、导入/管理状态及响应式布局 |
| 离线运行时 | Waydroid 1.6.2、261 个逐包锁定的 ARM64/架构无关 `.deb`、2 个 Android 13 镜像；运行时载荷共 3,870,820,258 字节 |
| Docker 初始化测试 | `--network none` 安装及 `apt-get check` 通过；Waydroid 初始化从 `/usr/share/waydroid-extra/images` 读取镜像并创建配置 |
| Binder 测试边界 | 初始化容器使用临时普通文件模拟三个 Binder 设备，`modprobe` 用仅在测试容器中的桩模拟可选 ashmem 模块缺失；未测试真实内核设备 |
| AgentOS 清单 | 261 个固定包的名称、版本、包内路径、SHA-256 从已验证 catalog 同步；动作 51,870 字节；Vitul ARM64 校验通过 |
| 最终 `.agt` | 814 个文件全部校验，包内所有运行时/前端哈希通过，无 AppleDouble/扩展属性，归档路径安全；`openMode: window`；低于 2 GiB |
| VM 授权部署与窗口 | 尚未在 VM 安装 0.1.7；宿主授权、真实 Binderfs/设备分配、Waydroid 容器、窗口映射、Flutter 呈现、输入焦点、缩放、多窗口、GPU 与退出清理均未验证 |

状态检测也已修复：Waydroid `init` 把预装镜像路径写入 `waydroid.cfg`，不一定复制到 `/var/lib/waydroid/images`；AndroidTone 现在读取配置中的受支持镜像目录再判断是否已初始化。AgentOS `appearance: system` 时，前端也会跟随宿主颜色方案变化。

## 0.1.4 平台接入构建与 VM 部署检查

日期：2026-10-12。当前候选包 `dist/androidtone-0.1.4-agentos-arm64.agt`，1,665,942,313 字节。SHA-256：

```text
f4117d7d710a45dd0f5109d666b36436bba08915cdb79918063716bf926f12eb
```

| 验证层 | 结果与范围 |
| --- | --- |
| Docker ARM64 后端编译 | 通过；AArch64 目标编译完成 |
| Rust 配置检查 | `cargo check --offline -p vitul-cli` 与 AndroidTone backend `cargo check --offline` 通过；`cargo fmt --check` 通过 |
| 前端与包构建 | Vite production build、TypeScript 检查、离线载荷检查、标准 Vitul 打包和 `runtime-build/verify-package.py` 通过；manifest 为 `openMode: window`、版本 0.1.4 |
| AgentOS 安装器解析 | VM 接受 `.agt` 安装，确认 `native.desktop-session` 权限与固定 Provider 声明通过 manifest 解析 |
| AgentOS 应用启动 | 未通过：AgentOS 健康检查反复收到 HTTP 503，启动请求超时并回滚到原版本；相同时段日志还出现 AndroidTone 容器名冲突，根因尚未确认 |
| Provider socket 注入与 `session.get` | 未验证；启动未稳定到可访问后端 API，不能据此确认 socket 注入或可信桌面会话契约可用 |
| Waydroid、窗口及输入呈现 | 未验证；VM 启动失败前未进入运行时或图形会话验收 |

本次 VM 检查没有重启 AgentOS 管理服务，也没有改变宿主 Binder、Waydroid 或 LXC 配置。后续需先排除 AgentOS 启动/容器名冲突，再验证 Provider socket 与 `session.get`，然后逐层验收运行时和窗口链路。

---

以下是较早版本的完整 VM 离线载荷验收记录，包为 `0.1.0`。它只覆盖离线用户态包及载荷校验，不代表当前 `0.1.7` 已通过 VM 启动或会话验证；当前版本结果见上节。

日期：2026-10-12。构建使用 Docker Desktop ARM64；验证机 `192.168.64.34` 为 Ubuntu 24.04.4 ARM64，运行内核 `6.8.0-100-generic`。

基础运行时验收产物：`dist/androidtone-0.1.0-agentos-arm64.agt`，1,665,406,465 字节（约 1.55 GiB），低于当前 AgentOS 2 GiB 包限制。SHA-256：

```text
621183cd562d39fcd5d27ae5ee9bdf7024038d502711a60b7f5e842f0f81612f
```

| 验证层 | 结果与范围 |
| --- | --- |
| Docker ARM64 后端编译 | 通过；Rust release，ELF ARM64 |
| 后端测试 | 7 项通过，覆盖受限参数、会话拒绝、内核分项观测、路径/符号链接及文件损坏 |
| Vitul 配置与打包 | ARM64 配置校验通过；使用标准打包器打包 Docker 编译产物，保留完整 Provider 声明 |
| 离线用户态安装 | 全新 Ubuntu ARM64 容器 `--network none`，仅本地包池安装成功；`apt-get check`、Waydroid 1.6.2、LXC 5.0.3、Python dbus/gi/gbinder 导入通过 |
| Android 镜像 | 官方 20260927 ARM64 system/vendor ZIP 均匹配 OTA SHA-256；固定成员解压、CRC 校验通过；system.img 1,994,985,472 字节，vendor.img 421,634,048 字节 |
| 包内载荷 | 260 个软件包、2 个镜像、169 组软件源码；共 3,101,155,676 字节清单载荷。最终 tar.gz 所有声明文件逐个 SHA-256 验证通过 |
| 包路径与 Linux 兼容 | 811 个文件；拒绝路径越界、链接和重复文件；无 AppleDouble 文件或 macOS 扩展属性 |
| VM 包完整性 | 最终整包 SHA-256 与本地一致，临时解包后 `--verify-runtime` 全部通过；没有解包元数据警告 |
| Provider 状态 | VM 普通用户 + no-new-privs 正确区分包内包含与宿主未部署；Docker 中另验证移除全部 capabilities 后相同行为 |
| 部署边界 | `runtime.install` 先验证载荷，再明确返回受限管理员契约 `platform-dependency`；未执行宿主安装或修改宿主服务/软件源/内核 |
| Binder 配置 | Binder IPC=m、binderfs=m、memfd=y，实际 memfd_create 探测通过 |
| Binder 模块安装 | 与运行内核匹配的 binder_linux.ko.zst 存在 |
| Binder 模块加载 / 挂载 / 设备 | 分别未观测到模块加载、binderfs 挂载、设备分配；不能据此断言内核不支持 |
| 宿主授权部署、可信会话 | 平台依赖，尚未验收 |
| Android 容器、窗口映射、Flutter 呈现 | 未验证 |
| 输入焦点、缩放、多窗口、GPU 实际呈现、退出清理 | 未验证；用户态安装成功不能替代这些验收 |

基础运行时验收包保留于 VM `/tmp/androidtone-self-contained-20261012.agt`。验证仅使用普通账户与本次临时目录，完成后清理临时解包目录，没有进行宿主安装。

原始构建与验证输出保存在 `agentos-build/verification/`（不纳入源码控制）。本机独立复验最终包：

```sh
python3 runtime-build/verify-package.py
```

在 ARM64 Docker 构建环境中执行 `python3 scripts/package-prebuilt-arm64.py`，即可从锁定源码构建后端与离线运行时、同步授权包摘要，并通过 Vitul 标准打包器生成 `.agt`。macOS 下该脚本在 tar 创建时去除平台元数据。验证记录只描述各自版本实际完成的检查；不能将 0.1.4 或 0.1.0 的 VM 结果当作 0.1.7 的端到端验证。

## 0.1.22 startup budget validation (2026-10-18)

- Backend: 43 tests passed, including a real 16-second property command (exceeding the normal 15-second CLI limit) and shared-budget exhaustion with a hung session-stop process. Cleanup still reaches the private Wayland stop request and clears persisted display handles.
- Packaging: frontend production build and ARM64 ELF/runtime/archive verification passed; `window` open mode. The existing runtime payload was reused and SHA-256 verified; this run did not repeat offline APT installation.
- VM `192.168.64.34`: installed 0.1.22 through the standard AgentOS API. Backend health passed. The compressed package was staged in `/dev/shm` after checking extraction space on `/`; no host package changes were made directly.
- An initial test was interrupted by an AgentOS service restart. A subsequent fresh user-session start observed Container and Session RUNNING, but did not reach Android framework/property readiness.
- The API returned HTTP 503 after 181.89 seconds, code `waydroid-startup-timeout`, phase `android-framework-ready`, startup budget 180 seconds. The last observation was `prop get sys.boot_completed` exceeding its remaining 175-second budget. Cleanup completed and `displaySessionId` was null. No automatic retry was performed.
- The Android 13 ARM64-only Composer tombstone showed SIGABRT in `hwc_wayland_thread(void*)+328`; kernel logs then repeatedly reported missing `android.hardware.graphics.composer@2.1::IComposer/default`. Attribution was initially unknown; the subsequent independent reproduction below identifies an AgentOS compositor initialization defect.
- **Not passed:** successful multi-window property configuration, sustained RUNNING after startup, and actual window presentation. Transient Container/Session RUNNING is not accepted as complete runtime readiness.

Detailed machine-readable evidence is generated at `agentos-build/verification/startup-budget-validation.json` and `startup-budget-vm.json`.

### Subsequent compositor isolation (2026-10-18)

The deployed private compositor aborts on initial xdg-shell window-state requests. An ordinary-user, software-rendered standalone client, without Android or Waydroid, reproduced SIGABRT for maximize, unmaximize and fullscreen before the first surface commit. A maximize request after the first commit passed. All three failures assert `surface->initialized` in `wlr_xdg_surface_schedule_configure`. Deployed binary SHA-256: `a577f8499731597ce173b5910fa8cb4ed7fe59d839c18b4516bc1f650606ee71`.

Process tracing of a real AndroidTone start confirms the compositor aborts before AgentOS terminates the display-client wrapper; the HWC broken pipe follows the server failure. The latest bounded start returned `waydroid-startup-timeout`, phase `android-framework-ready`, after 190.04 seconds including cleanup. The remaining platform defect prevents successful property and sustained session readiness verification.

Only diagnostics and this handoff documentation were added during isolation. AgentOS source, authorization policy and host configuration were not changed. See the [platform repair prompt](agentos-xdg-initial-state-fix-prompt.md), [standalone reproduction](../diagnostics/reproduce-xdg-initial-state.py) and [VM summary](../diagnostics/xdg-initial-state-vm-summary.json). No new application package was produced for this diagnostic-only change.

## 0.1.23 Waydroid CLI supervision and VM verification (2026-10-18)

The ARM64 package `dist/androidtone-0.1.23-agentos-arm64.agt` was built from the current source with Vitul's standard window-mode packager. It is 2,012,141,449 bytes and has SHA-256 `1f60d13365b20cf8c795e611ac8f41158cef8183a8dfda2552748a11764c43af`. The ARM64 Docker backend build, frontend production build, package/runtime verification and Rust test suite (47 tests) passed; `cargo fmt --check` passed.

The package was installed on VM `192.168.64.34` through the AgentOS install API as AndroidTone 0.1.23. The backend started on port 28642. AndroidTone's status API reported `process-running`, `running: true`, and a private display session ID. The API continues to mark its automated window-verification fields as `unknown`.

The first idle observation showed Container `FROZEN` while Session and the long-lived `waydroid session start` process remained alive. Waydroid's own log attributes this to `waydroidhardware` transaction 3 (`suspend`) calling `lxc-freeze`; it was not the AndroidTone startup deadline terminating the CLI. Launching `com.android.settings` through AndroidTone's app-launch API unfroze the container. From 2026-10-18 01:41:39 UTC through 01:43:52 UTC, 14 status samples taken roughly 10 seconds apart reported both Container and Session `RUNNING`, and the same session CLI PID remained present.

UTM's live VM screenshot then showed the Android Settings activity as a real window over the Agent Store, with its settings content visible. Keyboard navigation changed the visible Settings focus/scroll state, confirming that keyboard events reached the Android window. Pointer/touch input, GPU presentation, scaling, multiple Android windows, and close/exit cleanup were not verified. The earlier `sys.boot_completed=1` and `persist.waydroid.multi_windows=true` values were confirmed by the successful AndroidTone startup path; an additional root-shell property query was inconclusive because it lacked the Waydroid user-session D-Bus context.

The 120-second CLI supervision regression is verified in unit tests and on the VM: the same Waydroid session CLI remained alive beyond 120 seconds. The VM did expose Waydroid's separate idle-suspend behavior before Settings was launched; the sustained RUNNING observation above applies after an Android application was active.
