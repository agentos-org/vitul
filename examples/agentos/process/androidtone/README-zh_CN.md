# 安卓通 AndroidTone

AndroidTone 与 WinTone 并行，管理 Android 应用。界面上方导入 APK，下方显示任务、应用、状态和操作；包内 Provider 负责运行环境业务，AgentOS 提供通用平台能力。

Android 运行环境使用宿主 Linux 内核能力。AndroidTone 已接入 AgentOS 可信桌面会话 Provider 与私有 Wayland 会话。AgentOS 的通用系统部署 broker 已可用，但当前受限动作还不能完成该运行环境所需的 Binderfs 设备准备和 LXC 生命周期管理。因此 AndroidTone 暂不启用宿主安装，避免只做部分提权操作；普通 Provider 保持 `no-new-privs`，不直接提权，APK 仍可先导入保存。

Binder 配置、模块安装/加载、挂载与设备分配分项报告，缺少 /dev/binder 不代表内核不支持。进程、窗口映射、Flutter 呈现、输入/焦点、缩放、多窗口与 GPU 实际呈现分别验证，目前窗口层尚未验证。

构建：`vitul agentos validate` 后执行 `COPYFILE_DISABLE=1 vitul build --target agentos/arm64`。

参见 [架构](docs/architecture-zh_CN.md) 和 [集成审查](../docs/tone-platform-integration-review.md)。

运行时构建现已包含用户态组件、离线依赖与 ARM64 Android 系统/厂商镜像，不要求 AgentOS 打包运行时；仅支持 Ubuntu 24.04 ARM64。宿主部署仍需要通用平台提供可信离线包事务、Binderfs 分配和 LXC 生命周期动作。参见[自动准备状态](docs/automatic-setup.md)与[离线运行时边界](docs/offline-runtime-zh_CN.md)。

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

开发入口为 `http://127.0.0.1:5187/apps/androidtone/`，API 代理到 3000 端口的 Rust 后端。源码和类型接口位于 `frontend/src`，用户提供的图标位于 `frontend/public`，双语署名入口为 `frontend/credits.html`。Vite 生成的 `public/` 由 Rust 提供并随 ARM64 包发布；标准 Vitul 构建及预编译后端打包脚本均先根据锁文件重新构建前端。安装后的应用无需 Node.js 或 Vite 服务。参见 [迁移与验证](docs/frontend-react.md)。

前端已支持跟随 AgentOS 中英文语言及明暗主题，支持运行中切换。参见 [语言与主题说明](docs/localization-theme.md)。

产物保留规则：`dist/` 仅保留最新 AndroidTone `.agt` 及其 `.sha256`。`scripts/package-prebuilt-arm64.py` 会先完整校验新包，再自动删除旧版本产物；后续发布应使用递增版本号。
