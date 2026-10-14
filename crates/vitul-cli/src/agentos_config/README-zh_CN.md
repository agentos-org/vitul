# 在 `vitul.conf.json` 中配置 AgentOS

Vitul 将桌面、移动和 AgentOS 项目设置统一保存在根目录的
`vitul.conf.json`。在已有项目中运行 `vitul agentos init` 可生成此文件；
通过 `vitul create` 新建的项目则直接编辑自动生成的 `agentos` 配置。

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
      "artifacts": [{ "from": "dist", "to": "web" }]
    },
    "run": {
      "command": "node",
      "args": ["server.js"],
      "port": 9627,
      "healthCheck": "/health"
    }
  }
}
```

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
