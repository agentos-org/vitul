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
    "execution": { "mode": "native" },
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
只有明确设置 `mode: "native"` 才会启用宿主机原生运行。Native 进程直接在 Linux
宿主机运行并拥有完整宿主机访问权限；安装或更新时需要用户明确同意高风险授权并输入本地账户密码。

Native 仅适用于 process 应用。AgentOS 当前拒绝同时声明 deployment、Host Provider 或
`native.external-wayland` 的 native manifest；Vitul 会在配置校验阶段报告这些组合。使用这些功能时须保留默认容器模式。

托管网站可将 `agentos.type` 设为 `remote-web`，并在 `agentos.web.url` 中填写 HTTPS 地址。
remote-web 包会在 AgentOS 内嵌 CEF 窗口中打开网站，不会打包本地进程。

构建前运行 `vitul agentos validate`。使用 `vitul agentos inspect` 查看应用类型和 AgentOS 构建目标。
