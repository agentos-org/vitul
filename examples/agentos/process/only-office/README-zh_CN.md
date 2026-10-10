# AgentOS 沙盒版 ONLYOFFICE Docs

此示例从 ONLYOFFICE 官方 Docs Community 镜像构建完整、自包含的 AgentOS `.agt`。安装后，Docs、文档转换器、Nginx 和文档工作区都在 AgentOS 为应用创建的容器沙盒内运行。应用不申请 native 主机权限、不启动 Docker 容器，目标设备也不需要 root 或在线安装依赖。

包内包含从官方镜像导出的 Linux 文件系统和运行依赖。构建时会在临时官方镜像容器中调用 ONLYOFFICE 自带的资源生成器，生成字体表、主题和转换器缓存；不修改 ONLYOFFICE 源码或产品二进制。启动时，应用只在 AgentOS 提供的持久化 `DATA_DIR` 下生成可写配置、缓存、日志和文档数据。

## 构建

构建架构必须与 AgentOS 容器架构一致。Apple Silicon 开发机上构建 Linux/ARM64：

    ONLYOFFICE_ARCH=arm64 ./examples/agentos/process/only-office/build.sh

`ONLYOFFICE_ARCH=x64` 构建 Linux/AMD64；也可以设置 `ONLYOFFICE_PLATFORM=linux/arm64` 或 `linux/amd64`。Docker 优先使用本地 `onlyoffice/documentserver:9.4.0` 镜像；本地没有时，脚本会加载对应的 `.cache/documentserver-9.4.0-<arch>.tar`，或拉取官方镜像。

构建产物位于 `examples/agentos/process/only-office/dist/onlyoffice-docs-arm64.agt` 或 `onlyoffice-docs-x64.agt`。构建脚本会检查包内文件，并确保 `.agt` 比 2 GiB 上传上限至少小 16 MiB。可通过 `ONLYOFFICE_OUTPUT` 指定其他输出路径。

## 安装和使用

通过 Agent Store 安装 `.agt` 并打开。应用会先在沙盒内启动完整 ONLYOFFICE Docs 服务，再显示文档工作区。上传 DOCX、XLSX、PPTX、OpenDocument、文本、CSV 或 PDF 文件即可编辑；单个文件上限为 256 MiB。文档、配置、缓存和日志保存在应用持久化的 `DATA_DIR`。

`.agt` 只适用于构建时选择的 Linux 架构。包内包含官方 ONLYOFFICE Docs Community 发行内容及其许可声明；再分发前请阅读 [LICENSES.md](LICENSES.md)。
