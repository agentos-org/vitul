# AndroidTone 离线运行时验证记录

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

以下是较早版本的完整 VM 离线载荷验收记录，包为 `0.1.0`。它只覆盖离线用户态包及载荷校验，不代表当前 `0.1.4` 平台接入版已通过 VM 启动或会话验证；当前版本结果见上节。

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

标准构建遇到 Cargo 网络问题时，可在使用 Docker 离线编译后执行 `python3 scripts/package-prebuilt-arm64.py`。它先复验真实运行时，临时配置仅跳过已完成的编译/载荷构建步骤，随后调用 Vitul 标准打包器；源码配置保持完整构建步骤。macOS 下该脚本在 tar 创建时去除平台元数据。
