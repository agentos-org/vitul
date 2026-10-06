# Dockge 独立 App

本示例将 `vendor/dockge/` 中未修改的上游 Dockge 源码打包为可安装的 AgentOS `.agt`。构建脚本和 AgentOS 启动器是单独的打包文件；编译时会把源码复制到临时目录，不会修改它。

源码版本为 **1.5.0**，对应[上游官方标签](https://github.com/louislam/dockge/tree/1.5.0)。源码归档 SHA-256：`a907b54fafeab95bf4de637a43234d54fad2b77c453d8378f67488fc2b3826d9`。

每次构建一个 Linux 架构：

~~~sh
DOCKGE_PLATFORM=linux/arm64 ./examples/dockge/build.sh
DOCKGE_PLATFORM=linux/amd64 ./examples/dockge/build.sh
~~~

请在 Vitul 仓库根目录运行命令。默认产物为 `examples/dockge/dist/dockge-arm64.agt` 和 `examples/dockge/dist/dockge-x64.agt`。容器构建使用 Docker 和固定版本的 Node.js 22 镜像；npm 会为所选 Linux 架构安装生产依赖。可用 `DOCKGE_NPM_CACHE_DIR` 和 `DOCKGE_OUTPUT` 覆盖缓存与输出路径。

在 Agent Store 中安装生成的 `.agt`。Dockge 会在当前 AgentOS Workspace 中最大化打开。Native 进程以 AgentOS 管理员权限运行，安装时需要明确确认高风险授权。Dockge 使用宿主 Docker Engine 和 Docker Compose V2。后端只监听回环地址的 15001 端口，数据库和 stacks 保存在应用持久化 `DATA_DIR` 下。
