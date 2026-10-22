# 在 `vitul.conf.json` 中配置 AgentOS

Vitul 将桌面、移动和 AgentOS 项目设置统一保存在根目录的
`vitul.conf.json`。在已有项目中运行 `vitul agentos init` 可生成此文件；
通过 `vitul create` 新建的项目则直接编辑自动生成的 `agentos` 配置。

## 先把需求映射到配置与实现

本文维护 AgentOS 根配置的字段和关联约束。跨平台支持先看
[支持范围](../../../../docs/SUPPORT-zh_CN.md)，前端、后端与宿主的责任见
[架构](../../../../ARCHITECTURE-zh_CN.md)。先选择应用类型与运行模式，再配置单项能力。

| 需求/选择 | 根配置入口 | 必须同时完成 | 如何确认 |
| --- | --- | --- | --- |
| 打开已托管网站 | `agentos.type: "remote-web"`、`agentos.web.url` | HTTPS 网站；不打包本地进程、构建步骤和资源 | 包为 `any`，在目标 Shell 打开正确网站 |
| 打包本地服务 | `agentos.type: "process"`、`build`、`run` | 编译、收集、启动、监听、健康检查、前端入口 | 包内有服务与页面；安装后就绪并能调用业务 |
| 容器或直接宿主执行 | `agentos.execution` | 默认 container；native 需完整 profile 和持久数据；按宿主访问与隔离要求选择 | inspect 模式正确，能力与宿主环境匹配 |
| 保存数据 | `agentos.data.persistent` | 代码实际写入宿主提供的 `DATA_DIR`，配置、数据库和缓存路径按需求安排 | 重启后保留需要的数据 |
| 窗口布局 | `agentos.open` | `mode`、标题栏与窗口尺寸；宿主可能报告实际生效布局 | 多窗口、缩放和标题栏交互正确 |
| 关闭窗口后的行为 | native profile 的 `lastWindowClose`；其他模式按宿主协议实现 | 后台工作在后端、宿主入口、停止策略和子进程清理；不是开机自启 | 关闭最后窗口、重新打开及退出符合需求 |
| 系统 API | `agentos.permissions`、对应宿主调用 | 声明能力并实际调用；实现授权拒绝和不可用处理 | 获准、拒绝和缺少宿主能力时均符合预期 |
| 外部程序 | 根 `bundle.externalBin` 或 `agentos.build.artifacts` | 前者通过 host sidecar 使用时需要 `native.sidecars`；都要有正确架构文件、调用和停止逻辑 | 包含正确二进制，实际启动及退出成功 |
| Host Provider/部署 | `agentos.providers`、`deployment`、关联权限 | 协议方法、运行时接入或实现、宿主授权及服务；内置 Provider 不需要包内 executable | 校验声明，在宿主验证调用、错误与清理 |
| 系统代理/TUN | `agentos.network`、native profile、关联权限 | 可执行文件、配置快照、宿主网络服务和恢复逻辑，见下文 | 包摘要正确；实际网络效果及停止恢复正确 |
| OpenTool 接入 | `agentos.rpc`、后端路由 | 路径、发现方式、方法实现与前端调用一致 | 安装后实际调用成功 |

语言、主题和业务 UI 也需要代码接入，不能通过上述字段自动实现。入口见
[WebApp 语言与主题说明](../../../../examples/agentos/process/docs/webapp-localization-theme-zh_CN.md)。
表中的配置入口用于定位，不能把一行当作完整配置示例。

## 配置层级和路径

根 `schemaVersion: 2`、包内 `manifestVersion: 1` 和 OpenTool `rpc.version: 2`
分别定义不同格式。开发者编辑根 `vitul.conf.json`，不要把内部投影或示例的 `app.json` 原样复制进来。
以下字段均在根配置 `agentos` 下，除非明确标注为根字段。

| 配置组 | 关联规则 |
| --- | --- |
| `targets`、`buildImage` | 声明 `agentos/arm64` 或 `agentos/x64`，产物与运行时依赖需匹配所选目标；镜像不会自动使任意二进制跨架构 |
| `build.steps` | 在项目中生成产物；每步的命令、参数、工作目录和环境应完整。步骤存在不代表它一定生成了所有文件 |
| `build.artifacts` | `from` 是项目相对路径，`to` 是包内相对路径；可按 `targets` 选择架构产物 |
| `run.command`、`args` | 使用包内布局；包含路径的 command 必须由 artifacts 覆盖，参数指向的脚本和资源也必须实际打包；目标运行环境须有解释器和动态库 |
| `run.port`、`healthCheck`、`readyTimeout` | 后端监听与端口一致，健康检查对应真实路由，在超时内就绪；`frontendEntry` 对应实际前端路由 |
| `run.env` | 后端实际使用的变量与启动参数一致；native profile 禁止覆盖保留的宿主变量 |
| `permissions`、`providers`、`deployment`、`network` | 声明、授权、协议实现和宿主服务是不同依赖；具体约束见下文 |
| `data.persistent`、`execution` | 持久化与进程生命周期分别决定，开启持久化不会自动让进程在窗口关闭后运行 |

端口统一使用 `agentos.run.port`（默认 `9627`）。旧 `agentos.port` 仅在缺少
`run.port` 时回退使用并提示迁移，两个字段不同会报错。后端若使用 `run.env.PORT`，
也需同步修改；CLI 不会根据应用代码推断监听方式。

## Process 最小结构

下例演示字段布局：项目应提供 `dist/`、`service/server.js` 和 `/health` 路由，
并保证目标运行环境提供 Node.js；它不是包含服务源码的即用项目。

Process 应用在 `agentos` 下声明构建和启动配置：

```json
{
  "schemaVersion": 2,
  "productName": "示例应用",
  "identifier": "com.example.app",
  "version": "1.0.0",
  "agentos": {
    "type": "process",
    "targets": ["agentos/arm64", "agentos/x64"],
    "execution": { "mode": "container" },
    "build": {
      "steps": [],
      "artifacts": [
        { "from": "dist", "to": "web" },
        { "from": "service", "to": "service" }
      ]
    },
    "run": {
      "command": "node",
      "args": ["service/server.js"],
      "port": 9627,
      "healthCheck": "/health"
    }
  }
}
```

## 执行模式与 Native profile

省略 `agentos.execution`、将 `mode` 留空或设为 `container`，都会使用默认容器运行方式。
Vitul 只支持 AgentOS native process profile v1。设置 `mode: "native"` 时必须同时提供
`native` 对象；仅有 `{ "mode": "native" }` 的旧配置会被拒绝，避免将应用隐式打包为不可见、难以管理的旧执行形式。
Native 进程直接在 Linux 宿主机以已认证 AgentOS 账户身份运行；安装或更新时仍需要用户明确同意高风险授权。

配置 native profile 时还需开启持久数据：

```json
{
  "schemaVersion": 2,
  "productName": "通用服务",
  "identifier": "org.example.generic-service",
  "version": "1.0.0",
  "agentos": {
    "type": "process",
    "targets": ["agentos/arm64", "agentos/x64"],
    "execution": {
      "mode": "native",
      "native": {
        "version": 1,
        "identity": "desktop-user",
        "scope": "agentos-account",
        "lastWindowClose": "background",
        "stopGracePeriodMs": 20000
      }
    },
    "data": { "persistent": true },
    "build": {
      "steps": [],
      "artifacts": [
        { "from": "dist", "to": "web" },
        { "from": "service", "to": "service" }
      ]
    },
    "run": {
      "command": "node",
      "args": ["service/server.mjs"],
      "port": 9627,
      "healthCheck": "/health",
      "readyTimeout": 30,
      "autoStart": false,
      "frontendEntry": "/"
    }
  }
}
```

profile v1 必须提供 `version`、`identity` 和 `scope`，且分别固定为 `1`、
`desktop-user` 和 `agentos-account`。`lastWindowClose` 可设为 `stop` 或
`background`，默认 `stop`；`stopGracePeriodMs` 为 0–30000 的整数，默认 5000。
Vitul 会把完整字段（包括默认值）写入包内 `app.json`。该 profile 要求持久数据，
同一 bundle ID 在一个已认证 AgentOS 账户下共享单个后端，并且只能用于 manifest v1
process 应用。Vitul 始终输出
`run.autoStart: false`，profile 不允许启用开机或登录自启。`background` 只表示最后一个
窗口关闭后继续保活，不表示自启。

当 `lastWindowClose` 为 `background` 时，后端运行期间 AgentOS Shell 会显示通用托盘，
提供“打开”和“退出”操作。基础后台保活不需要应用权限或 tray helper。单独的
`native.tray` 权限用于应用通过 AgentOS tray socket 创建自定义托盘；只有应用确实调用
该接口时才需要申请。profile 应用不能覆盖 AgentOS 管理的环境变量，例如 `HOME`、`PATH`、
`XDG_RUNTIME_DIR`、Wayland/D-Bus 会话变量、`AGENTOS_TRAY_SOCKET`、`LD_PRELOAD` 和 `LD_AUDIT`。

Native 仅适用于 process 应用。使用 desktop-user profile 的 Native App 可以声明 AgentOS 内置 Host Provider（包括 External Wayland 与 system-deployment）及包内业务 Provider，并按 manifest 权限调用。Native system-deployment 可执行获准的宿主软件包、内核模块、Binderfs 和固定 systemd 服务操作；`linux.device.assign`、`linux.bind-mount` 与 LXC 操作仍仅适用于容器。需要容器设备映射或挂载的应用应保留默认容器模式。

### Native App 网络 profile

Vitul 支持 AgentOS Native App 网络 manifest v1。仅限
`execution.mode: "native"` 且 native profile v1 的 `identity` 为
`desktop-user` 的 process 应用。可执行文件和 TUN 配置文件使用最终包内的相对路径；
Vitul 会针对每个目标 staging 中的文件计算小写 SHA-256。不要在 `vitul.conf.json`
中填写摘要。

下面是通用代理配置示例。请通过 `build.artifacts` 把可执行文件放入包内的
`bin/network-client`。代理与 TUN 的完整配置示例见
[`examples/agentos/native-network/`](../../../../examples/agentos/native-network/README-zh_CN.md)：

```json
{
  "permissions": ["native.providers", "native.network", "native.system-proxy"],
  "network": {
    "protocolVersion": 1,
    "executables": [{ "id": "network-client", "path": "bin/network-client" }],
    "proxyProfiles": [{
      "id": "account-proxy",
      "executable": "network-client",
      "http": { "host": "proxy.example.invalid", "port": 8080 },
      "https": { "host": "proxy.example.invalid", "port": 8443 },
      "socks": { "host": "127.0.0.1", "port": 1080 },
      "bypass": ["localhost", "127.0.0.0/8", "::1"]
    }]
  }
}
```

TUN profile 还需声明 `native.tun`，并提供有界的固定 `arguments`、
`configuration: { "source": "runtime-snapshot", "maxBytes": 4194304 }`、
`interfaceName`、均为 `true` 的 `ipv4`/`ipv6`，以及固定的
`exitPolicy: "restore-previous"`。`maxBytes` 必须为 1024 至 4194304 的整数。
argv 必须至少包含一次 `${config}`，不能含其他占位符、宿主绝对路径或路径穿越段。
只有 package executable 需要由 `build.artifacts` 打包。App 在每次 `tun.start` 请求中
提交当前配置快照、outer endpoint 和 DNS。Vitul 会生成内置 `org.agentos.network`
Provider 和完整 v1 方法集合；不要为它声明 App 自带的 Provider。

所有网络声明都需要 `native.providers` 和 `native.network`。仅在存在
`proxyProfiles` 时添加 `native.system-proxy`，仅在存在 `tunProfiles` 时添加
`native.tun`。网络 profile 可与遵循 `native.providers` 规则的其他 App 自带 Provider
共存。`org.agentos.network` 由 Vitul 生成，使用固定 v1 方法集合，App 不能重复声明。
网络 profile 仅适用于 native process；remote-web 包不能声明网络 profile。打包前运行
`vitul agentos validate` 和 `vitul agentos inspect`。

Manifest 校验只能确认声明和包内 executable 摘要符合协议，不能证明第三方 TUN 内核或远端
隧道 endpoint 能否与 AgentOS sidecar 配合运行。代理行为和 TUN 可用性也取决于 AgentOS 主机及其网络服务。当前
Linux TUN 后端要求 AgentOS 服务运行于 systemd/systemd-resolved 主机，且具备
`iproute2`、`nft`、`/dev/net/tun` 和管理员预先启用的 IPv4/IPv6 转发；同时检查主机
resolver、策略规则、DNS 路由域以及到每个固定 IPv4 endpoint 的物理路由。系统代理
修改已认证账户的 GNOME 代理设置；忽略这些设置的程序仍可能直接连接。

托管网站可将 `agentos.type` 设为 `remote-web`，并在 `agentos.web.url` 中填写 HTTPS 地址。
remote-web 包会在 AgentOS 内嵌 CEF 窗口中打开网站，不会打包本地进程。

构建前运行 `vitul agentos validate`。使用 `vitul agentos inspect` 查看应用类型和 AgentOS 构建目标。


## 从配置到验收

1. `pnpm vitul agentos validate`：检查声明和部分源文件约束；错误应修改根配置或源文件。
2. `pnpm vitul agentos inspect`：核对最终应用类型、目标、执行模式、端口、权限和持久化；不启动应用。
3. `pnpm vitul build --target agentos`：执行构建并收集所选架构；检查 `.agt` 内 `app.json`、
   启动入口、脚本参数指向的文件、资源和二进制。打包存在不等于代码实际调用。
4. 在目标 Shell 安装并按需求验收成功、授权拒绝、资源缺失、重启和退出等相关场景。

如果只在浏览器或桌面开发环境运行过，不能据此宣称 AgentOS 安装包可用。
排错按“当前命令是否读取字段 → 生成结果是否正确 → 文件是否入包 → 组件是否启动 →
宿主是否支持及授权 → 用户可观察结果”逐层定位。完成报告应逐项注明已验证和未验证，
不以配置合法或构建成功代替功能验收。
