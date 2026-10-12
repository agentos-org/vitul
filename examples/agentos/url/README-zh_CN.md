# AgentOS URL 应用示例

每个目录都包含根目录 `vitul.conf.json` 和对应网站的官方图标。URL 应用会打包已托管的 HTTPS 地址和应用元数据；不会下载网站，也不会附带本地应用后端。

| 示例 | 英文名 | 中文名 | URL | Bundle ID |
| --- | --- | --- | --- | --- |
| [GitHub](github/vitul.conf.json) | GitHub | GitHub | `https://github.com` | `com.github` |
| [Tencent Video](tencent-video/vitul.conf.json) | Tencent Video | 腾讯视频 | `https://v.qq.com` | `com.qq.v` |
| [Douyin](douyin/vitul.conf.json) | Douyin | 抖音 | `https://douyin.com` | `com.douyin` |

Bundle ID 按网站主机名倒序生成。进入对应示例目录后，运行 Vitul CLI 校验和构建：

```sh
cd examples/agentos/url/github
vitul agentos validate
vitul build --target agentos/arm64
```

URL 安装包与平台架构无关，产物为 `agentos-any.agt`；初始配置中的目标仅用于校验 Vitul 构建目标。
三个示例都会在当前 AgentOS Workspace 中打开。
远程页面不会获得 AgentOS 原生能力、进程存储或 OpenTool 绑定。登录、媒体播放、页面导航和嵌入行为取决于网站自身的策略。

图标来自各网站官方页面引用的资源：

- GitHub：[fluidicon.png](https://github.com/fluidicon.png)
- Tencent Video：`https://vfiles.gtimg.cn/wuji_dashboard/xy/starter/4ea79867.png`
- Douyin：`https://lf-douyin-pc-web.douyinstatic.com/obj/douyin-pc-web/2025_0313_logo.png`

完整配置说明请看 [AgentOS 配置指南](../../../crates/vitul-cli/src/agentos_config/README-zh_CN.md)。
