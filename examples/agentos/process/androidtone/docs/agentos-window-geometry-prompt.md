# AgentOS 通用 External Wayland 窗口几何与装饰适配

请排查并修复通用 External Wayland 的窗口边框、透明背景和客户端装饰适配。只按协议和通用窗口契约处理，不按 AndroidTone、Waydroid、微信的 bundleId / appId / 标题增加分支。

## 背景与已核实事实

需求来源：AndroidTone 在 ARM64 QEMU VM `192.168.64.34` 使用 AgentOS 分配的私有 Wayland 会话运行 Waydroid Android 13。微信包 `com.tencent.mm` 已进入实际应用列表，并呈现了手机号登录页。当前问题是外层 AgentOS 窗口比可见 Android 窗口大，存在大片透明留白和两套标题栏。

AndroidTone 的 `windowHints` 仅提供启动尺寸 `900 × 680`；这不是 Android 应用内容边界，不能作为固定裁剪尺寸。其启动仍使用 `/app/androidtone-server`、workingDirectory `/app`、空 files；不需要新增文件授权或系统权限。

应用侧安装状态校验已另行修复：按 APK manifest 中的实际包名等待 Waydroid 启动器列表确认后才显示已安装。它不解决外层 Shell 装饰。

## 源码观察（需要结合 VM 协议数据确认）

- `native/external_wayland/host.c` 的 `render()` 当前通过 `wlr_xdg_surface_for_each_surface(..., extent, ...)` 计算整棵 surface tree 的纹理范围；纹理范围不应直接等同于窗口可见几何。
- `title_event()` 当前上报标题、映射、父窗口和输出信息，未见独立的 toplevel window geometry / decoration mode 传递。
- `shell/lib/src/widgets/shell_home_page.dart` 的 `_externalWaylandDescriptor()` 使用 `frameWidth/frameHeight` 推导外框大小；`_ShellWindowLayer.build()` 在 embedded 模式绘制 Shell 标题栏、背景和外框。
- `shell/lib/src/widgets/external_wayland_surface.dart` 将纹理放在 `frameX/frameY`，输入和 IME 目前也依赖 surface 坐标。调整几何时必须同步处理坐标，不能只裁切图片。
- Waydroid 官方 lineage-20 的 `hwcomposer/wayland-hwc.cpp` 在 `window::create()` 中会建立透明背景 surface，并把 `xdg_surface.set_window_geometry` 设置为全局虚拟 display 尺寸；真正的应用 layer 可以位于其子 surface 中。因此不能假设读到 xdg geometry 后一定能裁掉这些留白，也不能凭截图就断言客户端已经提交较小几何。
- 当前 AgentOS 的 xdg-decoration 处理对参与协商的客户端要求 server-side mode；应同时核查 Waydroid 是否参与该协商，以及客户端未协商 / 保留 Android caption 时 Shell 实际采用的模式。

官方参考：
- https://github.com/waydroid/android_hardware_waydroid/blob/lineage-20/hwcomposer/wayland-hwc.cpp
- https://github.com/waydroid/android_hardware_waydroid/blob/lineage-20/hwcomposer/modes/multi-window.cpp

## 请完成

1. 先提供逐窗口诊断：session/window ID、buffer 与逻辑尺寸、effective xdg window geometry、各 subsurface 的位置/尺寸、frame union bounds、input/opaque region、装饰协商状态、Shell viewport、configure/ack/commit 序列。区分客户端上报的整屏几何与平台误用纹理范围。
2. 在 native → transport → Flutter 的通用链路中分别表示窗口几何、纹理范围和有效装饰模式。窗口几何按 commit 生效；纹理里的阴影、overflow popup 或透明背景不应反向扩大主窗口 viewport，产生 resize 反馈循环。
3. 正确处理 CSD / SSD。协商为 SSD 时 Shell 拥有外框；客户端装饰模式需要保留客户端画面透明性，避免再绘制第二层不透明背景与标题栏。若当前协议不能表达未参与 xdg-decoration 的客户端如何选择装饰模式，请先提出最小通用契约（受限枚举，而非应用特例），明确其与协议协商的优先级。
4. 所有 viewport、纹理偏移、pointer/touch、popup、光标和 IME caret 坐标须经过同一几何转换；验证拖动、缩放、最大化、焦点和多窗口，不能只让边框在截图上看似对齐。
5. 若 Waydroid 本身上报全画布几何且缺少可用内容边界，明确报告仍需运行时提供什么协议元数据。不要以 alpha 像素扫描、固定裁剪值、排除特定标题或按 appId 猜测来伪造窗口边界，也不要承诺仅修 AgentOS 就能移除客户端自己绘制的 Android caption。
6. 增加一般 Wayland 测试客户端：有非零 x/y 的 geometry、透明空 input region 根 surface 和有内容的子 surface、CSD/SSD、动态 resize、overflow popup、多输出和 fractional scale。不得使用特定 Android 包名作为回归条件。

## 同次测试中的 Shell 崩溃

2026-10-18 10:33:25 CST，VM 的 `agentos-shell.service` 出现真实 SIGSEGV（进程 2644719），退出状态 139，10:33:28 自动重启，用户被带回 AgentOS 登录页。用户确认没有自行注销或重启。

此前实验仅通过普通 Waydroid CLI 临时将 `persist.waydroid.no_background_subsurface` 设为 true，并通过 AndroidTone 正常启动 API 打开系统计算器，以检查背景 surface 与边框的关系。尚未取得稳定对照画面，不能将它视为解决方案。随后已通过同一非提权 CLI 恢复为 false，并读回确认。不要把该选项作为修复默认值。

这只是时间相关证据，未取得 native 崩溃栈，不能直接宣称它是唯一触发条件。请收集该进程的 coredump/backtrace，核查纹理大小变化、surface 销毁、frame/texture 生命周期及未完成 GPU frame 的清理。单个兼容性客户端的合法 surface 变化不应导致整个 Shell 崩溃。

只读复核示例：

```sh
journalctl -u agentos-shell.service --since '2026-10-18 10:32:00' --until '2026-10-18 10:34:00' --no-pager
coredumpctl info 2644719 --no-pager
```

验收请分别报告：实际窗口几何、单套装饰/背景、首帧及长时间呈现、输入/焦点、缩放、多窗口、关闭清理，以及 Shell 不崩溃。不能将 CLI 返回成功、Session RUNNING 或一帧画面作为全部验收通过。
