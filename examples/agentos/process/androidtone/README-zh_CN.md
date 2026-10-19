# 安卓通 AndroidTone

AndroidTone 与 WinTone 并行，管理 Android 应用。界面上方导入 APK，下方显示任务、应用、状态和操作；包内 Provider 负责运行环境业务，AgentOS 提供通用平台能力。

AndroidTone 0.1.18 以 desktop-user Native process 运行，不进入 Docker App 沙盒。固定部署操作通过 AgentOS 通用 system-deployment Provider 准备 Binder，并安装随包的 Waydroid ARM64-only 运行环境与 Android 13 镜像。Waydroid 和镜像安装包仍按包名、版本、SHA-256 精确绑定；其余 259 个 Ubuntu 24.04 宿主依赖遵循精确 Waydroid 包声明的 Debian `Depends` 与 `Pre-Depends`，随包归档只作为离线候选。预检查阻塞的任务不会进入 Shell 授权队列，也不会在刷新时自动重试；修正清单或宿主状态后，用户可主动重新提交计划。镜像安装包保持精确组件，因为 Waydroid 1.6.2 只识别两个系统级预装镜像目录。普通 Provider 保持 `no-new-privs`，不直接提权；环境不可用时仍可导入并保存 APK。

当 AgentOS 将部署任务记录为 `denied` 时，界面会显示“在 AgentOS 中重新授权”入口。只有用户点击才会调用重新授权；刷新、后台轮询和 OpenTool 都不会提交请求。AndroidTone 只发送协议版本、原拒绝任务 ID 和已持久化的幂等键。是否重新申请由 AgentOS Shell 确认，管理员密码也只由 Shell 收集。Shell 确认且平台重新检查通过后，AndroidTone 跟踪关联的新任务，同时保留原拒绝任务及其审计记录。网络结果不确定时重试会复用同一幂等键；流程结束后，只有再次主动点击才会创建新流程。能力不可用或主机策略阻止时，重新授权不会绕过限制。

Binder 配置、模块安装/加载、Binderfs 和 Waydroid 宿主服务状态分项报告。Native 执行不再需要给 App 容器映射 Binder 设备；特权宿主改动仍由 AgentOS 系统授权流程执行。缺少 /dev/binder 不代表内核不支持。窗口映射、Flutter 呈现、输入/焦点、缩放、多窗口与 GPU 实际呈现分别验证，目前窗口层尚未验证。

构建：先执行 `vitul agentos validate`，再运行 `COPYFILE_DISABLE=1 python3 scripts/package-prebuilt-arm64.py`。打包器仍使用 Docker 构建 ARM64 后端和离线运行时，从已校验的运行时清单同步 AgentOS 的摘要固定部署动作，最后调用 Vitul 标准 AGT 打包器；安装后的应用后端直接在宿主运行。

参见 [架构](docs/architecture-zh_CN.md) 和 [集成审查](../docs/tone-platform-integration-review.md)。

运行时构建包含 Waydroid 用户态组件、离线依赖与 ARM64 Android 系统/厂商镜像，不要求 AgentOS 内置 Waydroid；仅支持 Ubuntu 24.04 ARM64。网络隔离的临时容器会验证离线包事务和镜像初始化脚本；宿主 Android 会话及窗口呈现仍需单独在 VM 验收。参见[自动准备](docs/automatic-setup.md)与[离线运行时边界](docs/offline-runtime-zh_CN.md)。

最终产物与分层实测结果见 [验证记录](docs/offline-runtime-validation.md)。

## 前端开发

应用市场现使用 React 19、TypeScript、Tailwind CSS 4、Vite 8 和 pnpm。构建需要 Node.js 22.12+；前端为独立 pnpm 项目，使用自身锁文件。

```sh
pnpm --dir frontend install --frozen-lockfile
pnpm --dir frontend dev
pnpm --dir frontend build
pnpm --dir frontend exec playwright install chromium
pnpm --dir frontend test
```

开发入口为 `http://127.0.0.1:5187/apps/androidtone/`，API 代理到 3000 端口的 Rust 后端。源码和类型接口位于 `frontend/src`，用户提供的图标位于 `frontend/public`，双语署名入口为 `frontend/credits.html`。Vite 生成的 `public/` 由 Rust 提供并随 ARM64 包发布；AGT 打包器会根据锁文件重新构建前端。安装后的应用无需 Node.js 或 Vite 服务。参见 [迁移与验证](docs/frontend-react.md)。

前端已支持跟随 AgentOS 中英文语言及明暗主题，支持运行中切换。参见 [语言与主题说明](docs/localization-theme.md)。

产物保留规则：`dist/` 仅保留最新 AndroidTone `.agt` 及其 `.sha256`。`scripts/package-prebuilt-arm64.py` 会先完整校验新包，再自动删除旧版本产物；后续发布应使用递增版本号。

APK 任务只有在解析出实际包名、Waydroid 安装命令没有报告错误，并在有限等待内确认该包出现在可启动应用列表后才显示“已安装”。未确认的任务显示可手动重试的失败状态，Waydroid 原始诊断只在任务详情中展开；没有启动 Activity 的 APK 不会被宣称可从应用列表运行。
