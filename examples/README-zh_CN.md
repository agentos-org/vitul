# Vitul 示例

示例按用途分组：

- [入门示例](starter/README-zh_CN.md) 包含推荐的 Vitul 桌面和 AgentOS 入门应用。
- [AgentOS 应用](agentos/README-zh_CN.md) 按运行方式归类可安装的 AgentOS 示例。

CI 使用的桌面 API 集成夹具位于
[tests/fixtures/desktop-api](../tests/fixtures/desktop-api/README-zh_CN.md)，用于验证兼容 API 行为。

构建条件和运行方法请查看各目录中的 README。

## 按需求选择和复用示例

| 需求 | 入口 | 阅读时必须一起检查 |
| --- | --- | --- |
| 新应用、业务后端与跨平台宿主调用 | [入门示例](starter/README-zh_CN.md) | 根配置、前端调用、OpenTool 后端与目标平台运行方法 |
| 打包已有服务或程序 | [AgentOS process 示例](agentos/process/README-zh_CN.md) | container/native、构建与启动脚本、依赖、数据目录和退出清理 |
| 接入宿主语言和主题 | [WebApp 接入](agentos/process/docs/webapp-localization-theme-zh_CN.md) | 初始上下文、变化订阅、文案与样式 |
| Native 网络集成 | [网络示例](agentos/native-network/README-zh_CN.md) | profile、权限、包内程序、宿主服务与实际网络验收 |
| 已有网站，无本地服务 | [URL 示例](agentos/url/README-zh_CN.md) | HTTPS 网站与 remote-web 限制 |
| 检查兼容 API | [集成夹具](../tests/fixtures/desktop-api/README-zh_CN.md) | 测试范围；夹具不是产品能力承诺 |

复用前先比较平台、执行模式、CLI/宿主版本和用户预期。把示例拆成配置、代码、
资源、构建、启动、授权和验收，逐项确认自己的项目需要哪些部分。
部分示例直接组装 manifest，部分使用统一 CLI；先读其 README 和脚本，不要只复制 JSON。
未被示例覆盖的能力应回到[支持范围](../docs/SUPPORT-zh_CN.md)和
[配置说明](../crates/vitul-cli/src/agentos_config/README-zh_CN.md)确认。
