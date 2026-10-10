# Vitulfile：将现有 Web 应用打包为 AgentOS 应用

`Vitulfile` 是一个打包配置，用于把已有 Web 应用构建成可安装的 AgentOS
`.agt` 包。它面向已经有前端、后端或线上网站，并需要 AgentOS manifest、启动
描述和安装包的项目。

这是 Vitul CLI 中独立的打包模块。它**不是 Rust 项目配置**，不要求项目包含
`backend/Cargo.toml`、Tauri，也不需要 Rust 中转服务。本地应用由配置指定的项目
自身进程提供服务。原有的 `vitul init` 新建 Vitul 桌面应用流程仍然独立保留。

## 支持的应用类型

### 已有的本地 Web 应用

使用 `type: process` 描述一个可以通过单个 HTTP 端口提供界面和 API 的服务。
Vitul 在构建机器上按顺序执行构建步骤、收集指定文件、生成启动脚本，并创建
AgentOS 安装包。AgentOS 运行环境提供 Node.js、Python、Bash 和常见系统库；应用
专用依赖仍需放入打包文件。

Vitul 支持 Linux ARM64（`agentos/arm64`）和 x64（`agentos/x64`）process 包。
在 `targets` 中声明一个或两个目标后，Vitul 会依次执行各目标的构建步骤，并在进入
下一个目标前完成当前包的产物收集。构建步骤本身不会交叉编译应用；构建脚本需要根据
目标环境变量生成对应架构的二进制文件和依赖。每个步骤都会收到当前目标对应的
`VITUL_TARGET`、`VITUL_TARGET_OS=linux`、`VITUL_TARGET_ARCH` 和
`AGENTOS_TARGET`。例如 `VITUL_TARGET_ARCH` 会是 `arm64` 或 `x64`。

### 线上网站

使用 `type: remote-web` 指向已经通过 HTTPS 托管的网站，例如 GitHub、腾讯视频或
抖音。Vitul 会生成 AgentOS `appType: url` 安装包，并把网址写入 `open.url`。
AgentOS 会在内嵌的 CEF WebApp 窗口中打开网站，不会调用操作系统浏览器。Vitul 不会
下载网站，也不会打包 Node.js 或 Rust 中转服务。登录、媒体播放、页面导航和嵌入行为
取决于网站自身的实现与策略。

远程页面不会获得 AgentOS 原生能力桥接，因此 remote-web 配置不能申请
`agentos.permissions`、进程数据存储或 OpenTool 绑定。

## 开发者流程

```sh
# 本地前后端项目
vitul agentos init
# 根据项目修改 Vitulfile 中的命令和产物路径
```

线上 HTTPS 网站则改用此配置：

```sh
vitul agentos init --type remote-web --url https://github.com
```

配置好之后，查看、校验并打包项目：

```sh
# 查看项目识别结果并检查配置
vitul agentos inspect
vitul agentos validate

# 一次构建 Vitulfile 中声明的所有目标
vitul build --target agentos
# 也可以只构建一个已声明的目标：vitul build --target agentos/arm64
#                              vitul build --target agentos/x64

# process 配置生成 dist/<应用名>-<版本>-agentos-arm64.agt 和/或
#                  dist/<应用名>-<版本>-agentos-x64.agt
# remote-web 配置生成 dist/<应用名>-<版本>-agentos-any.agt
```

remote-web 不包含平台相关的应用进程，因此始终只生成一个声明
`architecture: any` 的包。其目标声明仅用于校验允许使用的 Vitul 构建目标。

## 应用图标

可选的 `app.icon` 指向项目目录内的 PNG 或 SVG 文件。Vitul 会将其复制到安装包根目录，
命名为 `icon.png` 或 `icon.svg`；AgentOS 支持这两种格式。扩展名不区分大小写。

```yaml
app:
  # 其他应用字段……
  icon: assets/app-icon.svg
```

`vitul agentos init --path <目录>` 可以将文件写入另一个项目目录。本地应用的初始
配置使用 `./bin/server` 和 `bin/server` 作为示例启动命令与产物路径。验证和构建前，
请根据项目修改它们。`inspect` 会指出初始配置中尚未满足的项目，方便继续填写。

多架构 process 应用使用 `targets` 列表。旧的单值写法 `target: agentos/arm64` 仍然
兼容；一个 Vitulfile 只能使用这两种写法中的一种。

## 本地应用配置示例

```yaml
schemaVersion: 1
targets:
  - agentos/arm64
  - agentos/x64

app:
  type: process
  id: com.example.dockage
  name: Dockage
  name_zh: Dockage
  version: 1.0.0
  description: Manage Compose projects from AgentOS.

build:
  steps:
    - command: pnpm
      args: [install, --frozen-lockfile]
    - command: pnpm
      args: [run, build]
  artifacts:
    - from: dist
      to: public
    - from: server
      to: server
    - from: node_modules
      to: node_modules

run:
  command: node
  args: [server/index.js]
  port: 9627
  healthCheck: /health
  readyTimeout: 60
  frontendEntry: /
  env:
    NODE_ENV: production

agentos:
  openMode: window
  titleBar: system
  defaultWidth: 1200
  defaultHeight: 800
  minWidth: 720
  minHeight: 480
  permissions: [native.window-control]
  rpcBasePath: /opentool
  rpcDiscovery: runtime

data:
  persistent: true
```

构建步骤的 `command` 和 `args` 会从项目根目录（或单步配置的 `cwd`）依次直接
执行，不会经过 shell 解析。需要 shell 语法时，可以用 `bash` 并传入
`args: [-lc, "你的 shell 命令"]`。产物路径相对于项目目录；Vitul 会把文件复制到包根目录
下对应的 `to` 路径。只保留目标仍位于同一产物目录内的嵌套相对符号链接；绝对链接和
逃逸到产物目录之外的链接会被拒绝。产物目录中的旧 `.agt` 包会跳过，避免重复构建时
把旧包再次打进去。

如果 `run.command` 是 `./bin/server` 这样的包内路径，它必须落在某个
`build.artifacts.to` 目标路径下。`node` 或 `python3` 这类命令可以使用 AgentOS
运行时提供的程序；对应的脚本和应用依赖仍需作为产物打包。

应用进程需要监听 `0.0.0.0`，并在适用时使用 AgentOS 注入的 `HOST`、`PORT` 和
`DATA_DIR` 环境变量。`data.persistent: true` 声明应用要使用 AgentOS 持久数据区；
应用仍需自行把用户数据写入 `DATA_DIR` 下。

## 线上网站配置示例

```yaml
schemaVersion: 1
target: agentos/arm64

app:
  type: remote-web
  id: com.example.github
  name: GitHub
  version: 1.0.0

web:
  url: https://github.com

agentos:
  titleBar: system
```

## 要求与限制

- 本地 process 安装包可面向 Linux ARM64 或 x64；remote-web 包声明
  `architecture: any`。两者都使用 manifest version 1。
- 打包机器需要是 Unix 系统，并安装 `tar`。
- 本地应用需要为声明的每个架构提供兼容 Linux 的服务及其运行文件。Vitul 不会自动把
  多服务 Compose 部署合并为单个进程。
- process 包会声明 OpenTool 绑定，但 Vitul 不会替应用生成业务工具。若要支持
  AgentOS agent 调用，应用服务需要在配置的 `rpcBasePath` 下实现 OpenTool 路由。
  URL 包没有应用进程，也不声明 OpenTool 绑定。
- `agentos.rpcDiscovery: runtime` 会让 process App 启用 AgentOS 动态 OpenTool 函数发现。
  应用需要在 `rpcBasePath` 下提供有效的 OpenTool `load`、`call` 和 `streamCall` 路由。
  函数默认关闭；用户需要先在 AgentOS 设置中逐个启用，Chat 才能调用。默认值为 `static`。
- `agentos.openMode` 可设为 `window` 或 `workspace`，默认是 `window`。
  `workspace` 会在当前 AgentOS Workspace 中最大化打开应用。
- `agentos.permissions` 支持 `native.window-control`、`native.file-dialogs`、
  `native.clipboard`、`native.notifications`、`native.external-links` 和
  `native.sidecars`，仅适用于 process 应用。远程 CEF 页面不能申请这些原生能力。
- Dockage 和 Gitea 是此格式的候选打包项目，不代表已经开箱即用。Docker 宿主访问、
  Git 和运行时依赖、数据持久化，以及 ARM64 构建都需要针对 AgentOS 配置。

## 与 Vitul 项目的关系

新建 Vitul 应用仍使用 `vitul init`、`vitul.conf.json` 和 Rust 后端。
`Vitulfile` 用于将现有 Web 应用打包为 AgentOS 应用。项目存在 `Vitulfile` 时，
`vitul build --target agentos` 或指定单个 AgentOS 架构会使用本模块。对于没有 `Vitulfile` 的旧 Vitul 项目，
原有的 `agentos-arm64` 兼容目标仍会调用其打包流程。
