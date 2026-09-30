# Vitul 示例

建议从 [vitul-opentool](vitul-opentool/README-zh_CN.md) 开始。它的 Window Lab 页面展示
标题栏覆盖、通过宿主 IPC 执行窗口操作、中英文界面、多种外观主题，以及通过
OpenTool JSON-RPC 执行业务操作。该示例支持桌面 WebView 和 AgentOS 内嵌 WebView。

[api](api/README-zh_CN.md) 是 CI 使用的 Vitul 桌面 API 集成验证夹具，覆盖窗口、菜单、
托盘、事件等桌面 API 行为。开发新应用请从 vitul-opentool 开始。

其余目录聚焦于特定的兼容桌面能力或运行时功能。开发新应用时，请参考
vitul-opentool 中的 OpenTool 与宿主 API 结构。

构建和运行方法请查看各目录中的 README。
