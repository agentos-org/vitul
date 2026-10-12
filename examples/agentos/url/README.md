# AgentOS URL app examples

Each directory contains a root `vitul.conf.json` and the website's official icon. A URL
app packages the hosted HTTPS address and app metadata; it does not download
the website or bundle a local app backend.

| Example | English name | 中文名 | URL | Bundle ID |
| --- | --- | --- | --- | --- |
| [GitHub](github/vitul.conf.json) | GitHub | GitHub | `https://github.com` | `com.github` |
| [Tencent Video](tencent-video/vitul.conf.json) | Tencent Video | 腾讯视频 | `https://v.qq.com` | `com.qq.v` |
| [Douyin](douyin/vitul.conf.json) | Douyin | 抖音 | `https://douyin.com` | `com.douyin` |

The bundle IDs reverse each site's hostname. To validate and build an example,
run the Vitul CLI from that example's directory:

```sh
cd examples/agentos/url/github
vitul agentos validate
vitul build --target agentos/arm64
```

URL packages are architecture independent and produce an `agentos-any.agt`
package. The target declaration only validates the selected Vitul build target.
All three examples open in the current AgentOS Workspace.
Remote pages do not receive AgentOS native capabilities, process storage, or
OpenTool bindings. Login, media playback, navigation, and embedding behavior
depend on each website's policies.

Icons are copied from official website assets:

- GitHub: [fluidicon.png](https://github.com/fluidicon.png)
- Tencent Video: `https://vfiles.gtimg.cn/wuji_dashboard/xy/starter/4ea79867.png`
- Douyin: `https://lf-douyin-pc-web.douyinstatic.com/obj/douyin-pc-web/2025_0313_logo.png`

See the [AgentOS config guide](../../../crates/vitul-cli/src/agentos_config/README.md)
for the full configuration reference.
