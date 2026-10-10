# WinTone 架构与实现阶段

## 当前结构

```text
public/                  安装包导入、安装任务、已安装应用的启动与卸载界面
backend/src/api.rs       HTTP API、安装包导入、应用启动与卸载入口
backend/src/opentool.rs  AgentOS Chat 可发现的只读管理方法
backend/src/storage.rs   DATA_DIR 下的任务、会话和应用记录
backend/src/runtime.rs   共享运行时目录、清单校验、下载、SHA-256 校验和原子解包
backend/src/prefixes.rs  每个安装任务独立的 Wine prefix 目录和运行时绑定记录
backend/src/external_wayland.rs AgentOS Unix socket API 客户端
backend/src/shortcuts.rs Wine 卸载注册表读取、应用启动项和卸载器发现
backend/src/installation.rs 运行时准备、Wayland 会话启动、状态恢复和应用管理
scripts/build-server.sh  ARM64/x64 Linux 服务构建
```

当前安装包导入流程仅接收 `.exe` 和 `.msi`，限制单个文件为 2 GiB，检查 EXE/MSI 文件签名并记录 SHA-256。二进制文件用随机 UUID 文件名存储，UI 只接收和展示原始文件名。每次导入都会自动创建一个持久化的安装任务，并在后台进入运行时准备；用户可移除任务及其对应安装包。旧版暂存清单会在启动时迁移为等待运行环境的安装任务。

HTTP 控制器和 OpenTool 方法读取同一份安装任务清单。OpenTool 当前提供 `runtime.status`、`installations.list` 和 `apps.list`；大文件上传走 HTTP multipart，不经 OpenTool JSON 消息。

导入后，WinTone 会后台准备共享 Wine/FEX 运行时，再为该安装任务分配独立 prefix 目录。Wine 会在 AgentOS 窗口会话接通、启动安装器时初始化 prefix。运行时包清单从进程旁的 `runtime-catalog.json` 读取，也可通过 `WINTONE_RUNTIME_CATALOG` 指定；清单固定目标架构、HTTPS 下载地址、SHA-256、包格式和 Wine/FEX 启动命令。下载包使用 `tar.zst`，在校验哈希后解包到版本目录。当前仓库还没有随应用发布实际的 Wine/FEX 包清单，所以首次导入会在“等待运行环境”显示原因。

WinTone 完成运行时和 prefix 准备后，通过 `AGENTOS_EXTERNAL_WAYLAND_SOCKET` 调用 AgentOS 会话 API。Wine/FEX 命令、环境、prefix 路径和 `/data` 读写授权由 WinTone 提交；Wayland compositor、窗口呈现、输入和会话隔离由 AgentOS 管理。WinTone 通过 `/v1/sessions` 恢复会话并观察安装器退出，然后读取 Wine prefix 注册表整理应用入口。

## AgentOS 接口约束与待验证项

WinTone 已调用 AgentOS v1 会话 API。该版本的数据授权只能把 WinTone 的整个持久化数据目录挂载到会话的 `/data`，因此会话中的 Windows 程序可读写所有 Wine prefix、运行时和安装包。prefix 目前是逻辑隔离；若要把不同 Windows 应用当作互不信任的边界，AgentOS 需要增加子目录级挂载授权。

当前会话使用 Wayland 和 `wl_shm`，不使用 XWayland；AgentOS 文档列出的 DMA-BUF、客户端光标图像、IME/text-input、跨会话剪贴板、多输出和小数缩放能力仍有限或未支持。这些能力会影响需要中文输入、剪贴板、GPU 加速或多显示器的 Windows 应用，需要结合目标应用逐项验证。

这份 Vitul 仓库尚未提供 Wine/FEX 实际发布包，也没有在 AgentOS 设备上跑通安装器。发布包需要包含与目标架构匹配的 Wine、ARM64 所需 FEX 与 x86-64 RootFS，并确保 Wine Wayland 驱动及动态库适配 AgentOS 会话容器。当前安装器发现逻辑读取 Wine 的卸载注册表；对不写卸载信息或不提供可识别图标/启动程序的安装器，WinTone 可能无法自动登记启动入口。

## 后续模块边界

| 模块                     | 职责                                                                   |
| ------------------------ | ---------------------------------------------------------------------- |
| `runtime`                | 运行时版本清单、校验、安装、升级与回滚；Wine/FEX 在设备间共享          |
| `prefixes`               | 每次 Windows 安装默认创建独立 prefix，维护其数据目录和所用运行时       |
| `installers`             | 启动安装向导、观察安装结束、识别快捷方式及卸载记录                     |
| `apps`                   | 管理一个安装环境中的多个启动入口、图标及卸载操作                       |
| `sessions`               | 为安装任务和已安装应用准备 Wine/FEX 命令，并通过 AgentOS 会话接口管理进程 |
| `external_wayland`       | 通过 per-App Unix socket 请求能力、创建和查询 AgentOS 会话              |
| `shortcuts`               | 从 Wine 卸载注册表发现可启动应用和可识别的卸载器                         |

AgentOS v1 的 `/data` 全目录读写授权是目前最需要收紧的边界；同时，缺少实际 Wine/FEX 发布包和目标应用验证意味着代码接通还不等于已经能安装、启动任意 Windows 应用。

ARM64 上使用 Linux FEX 命令时，运行时包必须一并提供 x86-64 RootFS；[FEX 官方 RootFS 文档](https://wiki.fex-emu.com/index.php/Development:Setting_up_RootFS)说明了这一要求。运行时包发布来源、许可、签名和更新策略仍需在正式分发前确定。

清单字段示例见 [runtime-catalog.example.json](runtime-catalog.example.json)。
