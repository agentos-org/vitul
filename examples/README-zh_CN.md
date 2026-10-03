# Vitul 示例

Vitul 原生应用通过 OpenTool 处理业务操作，通过宿主 API 执行系统功能。建议从
[window-lab](window-lab/README-zh_CN.md) 开始，它用一个精简的桌面和 AgentOS 应用展示这两部分。

[frame-video](frame-video/README-zh_CN.md) 是一个更完整的原生示例。它保留 AgentOS
Frame Video 界面，并用 Rust 后端管理媒体库和视频流。

## AgentOS 应用类型

[process](process/README-zh_CN.md) 展示如何把已有前后端项目打包为 AgentOS process
应用。示例 Toney Music 使用 React/Vite 和 Go。

[url](url/README-zh_CN.md) 展示如何把已经托管的 HTTPS 网站打包为 AgentOS URL 应用。

CI 使用的桌面 API 集成夹具位于
[tests/fixtures/desktop-api](../tests/fixtures/desktop-api/README-zh_CN.md)，用于验证兼容 API 行为。

构建条件和运行方法请查看各目录中的 README。
