# AgentOS URL 应用

如果网站已经通过 HTTPS 托管，可以使用 URL 应用。Vitul 会打包网址和应用元数据，不会下载网站，也不会附带应用后端。

进入新应用目录后，创建初始 `Vitulfile`：

```sh
mkdir my-url-app
cd my-url-app
vitul agentos init --type remote-web --url https://example.com
```

检查生成的应用名称和标识符，然后校验并构建安装包：

```sh
vitul agentos validate
vitul build --target agentos
```

生成配置的基本结构如下：

```yaml
schemaVersion: 1
target: agentos/arm64

app:
  type: remote-web
  id: com.example.website
  name: Example Website
  version: 1.0.0

web:
  url: https://example.com

agentos:
  titleBar: system
```

初始配置中的目标仅用于校验 Vitul 构建目标。URL 安装包与处理器架构无关，产物为
`agentos-any.agt`。远程页面不会获得 AgentOS 原生能力、进程存储或 OpenTool 绑定。
登录、媒体播放、页面导航和嵌入行为取决于网站自身的策略。

完整配置说明请看 [Vitulfile 指南](../../crates/vitul-cli/src/vitulfile/README-zh_CN.md)。
