# WinTone 架构与实现阶段

## 当前结构

```text
vitul.conf.json         Vitul 统一项目与 AgentOS process 配置
public/                  安装包导入、安装任务、已安装应用的启动与卸载界面
backend/src/api.rs       HTTP API、安装包导入、应用启动与卸载入口
backend/src/opentool.rs  AgentOS Chat 可发现的只读管理方法
backend/src/storage.rs   DATA_DIR 下的任务、会话和应用记录
backend/src/pe_arch.rs   读取 PE Machine 字段并选择 x86/x64 或 ARM64 路径
backend/src/runtime.rs   共享运行时目录、清单校验、内置/HTTPS 包校验和原子解包
backend/src/prefixes.rs  每个安装任务独立的 Wine prefix 目录和运行时绑定记录
backend/src/external_wayland.rs AgentOS Unix socket API 客户端
backend/src/shortcuts.rs Wine 卸载注册表读取、应用启动项和卸载器发现
backend/src/installation.rs 运行时准备、Wayland 会话启动、状态恢复和应用管理
```

`vitul build --target agentos/arm64` 或 `vitul build --target agentos/x64` 会读取根配置，按目标架构交叉编译 Rust 后端，并打包声明的 process 产物。

当前安装包导入流程仅接收 `.exe` 和 `.msi`，限制单个文件为 2 GiB，检查 EXE/MSI 文件签名并记录 SHA-256。二进制文件用随机 UUID 文件名存储，UI 只接收和展示原始文件名。每次导入都会自动创建一个持久化的安装任务，并在后台进入运行时准备；用户可移除任务及其对应安装包。旧版暂存清单会在启动时迁移为等待运行环境的安装任务。

HTTP 控制器和 OpenTool 方法读取同一份安装任务清单。OpenTool 当前提供 `runtime.status`、`installations.list` 和 `apps.list`；大文件上传走 HTTP multipart，不经 OpenTool JSON 消息。

导入 `.exe` 后，WinTone 读取 DOS/PE 头中的 Machine 字段：i386 与 x86-64 PE 使用 x86/x64 Wine，ARM64 PE 使用原生 ARM64 Wine；ARM64EC/ARM64X 暂不支持。`.msi` 目前默认选择 x86/x64 路径。每个安装任务都有独立 prefix，并记录其 Wine 架构，避免用一种 Wine 架构继续打开另一种架构初始化的 prefix。

ARM64 `.agt` 内附 SHA-256 固定的 `tar.zst` 运行时包和清单，包含 x86-64 Ubuntu RootFS 中的 Wine、FEX，以及单独的 ARM64 Ubuntu Wine rootfs；首次使用时，WinTone 将它校验并解包到 `DATA_DIR` 的版本目录。清单也支持 HTTPS 运行时包，方便后续独立更新。两条路径都启用 Wine Wayland 驱动，窗口仍由 AgentOS 会话管理。

WinTone 完成运行时和 prefix 准备后，通过 `AGENTOS_EXTERNAL_WAYLAND_SOCKET` 调用 AgentOS 会话 API。每个会话分别请求共享 Wine/FEX 运行时只读挂载、当前 Wine prefix 持久读写挂载；安装阶段另将当前安装包只读挂载。会话路径统一使用 `/grants/runtime`、`/grants/prefix` 和 `/grants/installer/content`，不会授权整个 `/data`。Wayland compositor、窗口呈现、输入和会话隔离由 AgentOS 管理。WinTone 通过 `/v1/sessions` 恢复会话并观察安装器退出，然后读取 Wine prefix 注册表整理应用入口。

## AgentOS 接口约束与待验证项

AgentOS 的 scoped file grants 使用持久挂载，按调用方数据根目录内的相对路径授予运行时、prefix 和安装包。WinTone 在创建会话前检查 `fileGrants.available`；该能力不可用时会等待并显示原因，不会回退到整个 `/data` 授权。已安装应用与卸载器只获得自己的 prefix 和共享只读运行时；安装器会额外获得导入包的只读挂载。

WinTone 设置中可为新会话选择 GPU 加速以及剪贴板读取、写入权限。GPU 只在 AgentOS 主机能力可用时申请，否则使用软件路径；剪贴板读写独立控制并默认关闭。AgentOS 还提供输入法、光标和输出管理能力，目标 Windows 应用仍需在可见桌面上验证中文输入、剪贴板交互和多显示器行为。

ARM64 构建会从 Ubuntu 24.04 AMD64 软件包安装 x86/x64 Wine 与 32 位组件，从 Ubuntu ARM64 ports 获取原生 ARM64 Wine，并从 FEX 官方 PPA 获取 ARM64 FEX；构建检查两套 Wine 的 Wayland 驱动。PE 解析与会话路由有单元测试，但真实 ARM64 Wine 图形会话及 Windows 应用仍需在目标 AgentOS 设备实测。安装器发现逻辑读取 Wine 的卸载注册表；不写卸载信息或没有可识别启动程序的安装器可能无法自动登记入口。x64 AgentOS 目标尚未附带 Wine 运行时。

## 后续模块边界

| 模块                     | 职责                                                                   |
| ------------------------ | ---------------------------------------------------------------------- |
| `runtime`                | 运行时版本清单、校验、安装、升级与回滚；按 PE 架构选择 Wine/FEX 子路径 |
| `pe_arch`                | 解析 PE Machine 字段；拒绝 ARM64EC/ARM64X 及未知架构                   |
| `prefixes`               | 每次 Windows 安装默认创建独立 prefix，维护其数据目录和所用运行时       |
| `installers`             | 启动安装向导、观察安装结束、识别快捷方式及卸载记录                     |
| `apps`                   | 管理一个安装环境中的多个启动入口、图标及卸载操作                       |
| `sessions`               | 为安装任务和已安装应用准备 Wine/FEX 命令，并通过 AgentOS 会话接口管理进程 |
| `external_wayland`       | 通过 per-App Unix socket 请求能力、创建和查询 AgentOS 会话              |
| `shortcuts`               | 从 Wine 卸载注册表发现可启动应用和可识别的卸载器                         |

WinTone 与 AgentOS 的会话文件接口按最小范围授权。ARM64 测试包包含真实运行时并固定其 SHA-256；需要在目标 AgentOS 设备完成安装器及目标应用验证，才能确认实际体验。

ARM64 上只有 x86/x64 Wine 需要 FEX 和 x86-64 RootFS；ARM64 PE 交给原生 ARM64 Wine。运行时包发布来源、许可、签名和更新策略仍需在正式分发前确定。

清单字段示例见 [runtime-catalog.example.json](runtime-catalog.example.json)。


## 集成验证与平台依赖

External Wayland 是窗口接入协议，不代表 Wine 完整兼容性。WinTone 管理 Wine/FEX 和 prefix，AgentOS 提供通用隔离会话、授权和窗口呈现。启动脚本要求平台分配的私有 Wayland socket，并清除 DISPLAY。平台明确不提供 XWayland。WinTone 仅使用 Wine 原生 Wayland 后端；需要 Wine X11 后端才能正常运行的 Windows 程序，标为当前不支持，不回退 X11，也不申请 XWayland 能力。

进程运行或窗口对象创建不等于映射、Flutter 呈现、输入/焦点、缩放、多窗口、GPU 实际呈现或退出清理通过；必须分层记录证据，未知状态保持未验证。

参见 [集成审查与通用平台契约](../../docs/tone-platform-integration-review.md)。
