# Gitea 独立沙盒应用

本示例将官方 Gitea 二进制打包为独立 AgentOS `.agt` 应用。Gitea 运行在
AgentOS 容器沙盒中；数据库、配置、仓库、上传文件、会话、日志、用户目录和临时文件
都保存在应用持久化的 `DATA_DIR` 下。Git 也会打包进沙盒，宿主机无需安装 Git。

构建使用[官方站点](https://dl.gitea.com/gitea/1.27.3/)发布的 **Gitea 1.27.3**。
[1.27.3 版本](https://blog.gitea.com/release-of-1.27.3/)包含旧 AgentOS checkout 缓存的
Gitea 1.26.4 尚未包含的安全修复；上游将不高于 1.26.4 的版本列为
[GHSA-q9pg-jj6x-j9p6](https://github.com/go-gitea/gitea/security/advisories/GHSA-q9pg-jj6x-j9p6)
的受影响版本。打包过程不会修改上游可执行文件。Gitea 官方二进制已经包含网页资源和
SQLite 支持；构建脚本会在打包前核对各架构对应的官方 SHA-256。

一次构建一个 Linux 架构：

~~~sh
GITEA_PLATFORM=linux/arm64 ./examples/gitea/build.sh
GITEA_PLATFORM=linux/amd64 ./examples/gitea/build.sh
~~~

请在 Vitul 仓库根目录运行。默认产物为
`examples/gitea/dist/gitea-arm64.agt` 和 `examples/gitea/dist/gitea-x64.agt`。
构建需要 Docker，为对应架构的 AgentOS 容器准备 Git 和运行时库。
`GITEA_BINARY`、`GITEA_CACHE_DIR`、`GITEA_OUTPUT` 和 `GITEA_BUILD_IMAGE` 可分别覆盖
Gitea 二进制、缓存目录、输出路径和 Ubuntu 构建镜像；`GIT_RUNTIME_CACHE_DIR` 可覆盖 Git
运行时缓存目录。下载二进制时如需绕过 curl 代理，可设置 `GITEA_CURL_DIRECT=true`。

通过 Agent Store 安装生成的 `.agt`。Gitea 会在当前 AgentOS Workspace 中打开，并在
3000 端口提供 HTTP 服务。SSH 已禁用；Git clone 和 push 使用 HTTP。新建实例时首个
注册账号会成为管理员。AgentOS 只在本机回环地址提供 HTTP 端口，应用在容器内监听
`0.0.0.0`。自动启动已关闭。

包内 `LICENSE` 保留了上游 [MIT 许可证](https://github.com/go-gitea/gitea/blob/v1.27.3/LICENSE)。
