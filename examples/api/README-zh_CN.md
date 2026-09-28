# Vitul API 示例

这是 CI 桌面 API 兼容性测试使用的 Vitul 应用夹具。项目使用 src-vitul/ 和
vitul.conf.json；greet 业务操作由 Rust OpenTool 后端提供，测试套件则覆盖受支持的
系统 API。

新应用推荐从包含 AgentOS 宿主桥接的示例开始，请看
[../vitul-opentool](../vitul-opentool/README-zh_CN.md)。

## 运行

在仓库根目录安装依赖，并构建本地 API 包和 Vitul CLI：

    pnpm install
    pnpm build:api
    pnpm build:cli:debug

然后在本目录运行：

    pnpm vitul dev
    pnpm vitul build

vitul dev 会先使用动态 loopback 端口启动 OpenTool 后端，再启动桌面应用。
Welcome 页面展示了通过 /opentool/call 发起的业务调用，以及通过系统 IPC
最小化窗口的操作。
