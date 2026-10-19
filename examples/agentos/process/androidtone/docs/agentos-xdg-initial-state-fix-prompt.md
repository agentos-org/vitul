# AgentOS 通用 xdg-shell 初始化时序修复提示词

请修复 AgentOS External Wayland 私有 compositor 的通用 xdg-shell 初始化时序问题。需求来源是 AndroidTone 启动 Waydroid 时 Android HWC 断连，但该问题可以完全脱离 Android、Waydroid 和 AndroidTone 独立复现，不应增加应用特例或权限。

## 已确认的证据

2026-10-18 在 ARM64 VM `192.168.64.34`，以普通桌面用户运行临时 compositor 和最小 Wayland 客户端：

| 请求时机 | 请求 | 结果 |
| --- | --- | --- |
| 首次 `wl_surface.commit` 后 | `xdg_toplevel.set_maximized` | roundtrip 成功，进程存活 |
| 首次 commit 前 | `set_maximized` | SIGABRT，退出码 -6 |
| 首次 commit 前 | `unset_maximized` | SIGABRT，退出码 -6 |
| 首次 commit 前 | `set_fullscreen` | SIGABRT，退出码 -6 |

三个失败用例的断言相同：

```text
types/xdg_shell/wlr_xdg_surface.c:168:
wlr_xdg_surface_schedule_configure: Assertion `surface->initialized' failed.
```

VM 已部署的 `/opt/agentos-shell/labwc/bin/agentos-wayland-host` SHA-256：
`a577f8499731597ce173b5910fa8cb4ed7fe59d839c18b4516bc1f650606ee71`。

AndroidTone 实际启动的系统调用跟踪也确认：compositor PID 4076619 在 08:30:44.146 收到 SIGABRT；随后 AgentOS 在 08:30:44.507 向显示客户端包装进程 PID 4076673 发送 SIGTERM。Android HWC 的 Broken pipe 是服务端先退出后的现象。AndroidTone 最终在共享启动预算耗尽后返回 `waydroid-startup-timeout`，阶段为 `android-framework-ready`，不是密码授权失败。

## 源码定位与修复要求

请先保留 AgentOS 未提交改动，检查 `native/external_wayland/host.c`：

- `top_maximize()`（当前约 2400 行）直接调用 `wlr_xdg_toplevel_set_maximized()`。
- `top_fullscreen()`（当前约 2405 行）直接调用 `wlr_xdg_toplevel_set_fullscreen()`。
- `new_top()` 创建对象时即注册这些 request listener；客户端在首次 commit 前设置初始窗口状态时，`xdg_surface` 尚未 initialized，上述调用调度 configure 并触发 wlroots 断言。
- `top_commit()` 当前通过 `initial_configured` 设置初始尺寸。请依据当前 wlroots 生命周期，在安全的首次 commit/configure 阶段应用暂存的初始状态，并检查 unmap/remap 后的重新初始化。

请缓存并保留最新的最大化/取消最大化、全屏/取消全屏请求，等 surface 可配置后应用。不能仅丢弃初始请求、移除断言或改变客户端协议顺序来隐藏问题。请同时审计 viewport、maximize、focus/activated 等控制路径上的 `wlr_xdg_toplevel_set_*`，防止它们在相同生命周期阶段触发 configure。

本机 `/usr/share/wayland-protocols/stable/xdg-shell/xdg-shell.xml` 可用于核对协议中的初始状态和首次空 commit/configure 顺序。

不要修改 AndroidTone 权限、Binder/LXC 策略、部署授权流程；不要关闭隔离、启用 XWayland、切换到宿主 XDG_RUNTIME_DIR 或接管桌面 compositor。

## 可重复执行的独立复现

脚本在 Vitul 仓库：
`examples/agentos/process/androidtone/diagnostics/reproduce-xdg-initial-state.py`。

将脚本复制到 VM，然后以普通用户执行：

```sh
python3 reproduce-xdg-initial-state.py \
  --library-path /opt/agentos-shell/labwc/lib/aarch64-linux-gnu:/opt/agentos-shell/libinput-1.26.0/lib/aarch64-linux-gnu \
  --output /tmp/xdg-initial-state-result.json
```

脚本只连接自己启动的临时 compositor socket，使用软件渲染，限制等待和退出清理时间，并禁用该临时子进程的 core dump；不接触平台授权 socket 或现有桌面会话。退出码 1 表示至少一项连接/协议错误或进程崩溃。完整 stderr 保存到 output，终端仅显示摘要。

本次四项实测摘要保存在相邻的 `diagnostics/xdg-initial-state-vm-summary.json`；完整 renderer 日志保存在 AndroidTone 的 `agentos-build/verification/xdg-initial-state-vm.json`（构建目录不入版本控制）。

## 验收

1. 补充通用 compositor 回归测试：初始 commit 前后最大化/取消最大化、全屏/取消全屏；验证请求的最终状态正确进入 configure，不能只断言进程没有崩溃。
2. 覆盖初始 commit 前销毁、连续状态请求、unmap/remap，以及过早到达的 viewport/focus 控制。
3. 构建并按平台正常流程部署到上述 VM，报告实际运行的 compositor 路径与 SHA-256；重新运行独立复现，所有用例应成功。
4. 再通过 AndroidTone 正常启动链路验证：Container 与 Session 稳定 RUNNING、`sys.boot_completed=1`、`persist.waydroid.multi_windows=true`，随后分别验证实际窗口呈现、输入和退出清理。不能把修复后 capability available 或瞬时 Session RUNNING 当成完整通过。

AndroidTone 当前安装包为 0.1.22，已经实现共享 180 秒启动预算、framework 就绪探测及有界失败清理。本次不需要通过重打同一应用包掩盖 compositor 崩溃。
