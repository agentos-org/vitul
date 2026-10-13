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

Native 仅适用于 process 应用。AgentOS 当前拒绝同时声明 deployment、Host Provider 或
`native.external-wayland` 的 native manifest；Vitul 会在配置校验阶段报告这些组合。使用这些功能时须保留默认容器模式。

托管网站可将 `agentos.type` 设为 `remote-web`，并在 `agentos.web.url` 中填写 HTTPS 地址。
remote-web 包会在 AgentOS 内嵌 CEF 窗口中打开网站，不会打包本地进程。

构建前运行 `vitul agentos validate`。使用 `vitul agentos inspect` 查看应用类型和 AgentOS 构建目标。
