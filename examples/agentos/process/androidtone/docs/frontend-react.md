# React 前端迁移

2026-10-12，管理界面迁移为 React 19 + TypeScript + Tailwind CSS 4 + pnpm + Vite 8。保留上方 APK 导入、下方安装任务与已安装应用的布局，以及用户提供的 Android 机器人购物袋图标。

- `frontend/src/App.tsx`：组件、响应式布局、运行环境及应用状态。
- `frontend/src/useStore.ts`：导入、操作、刷新、安装进度轮询与通知；串行操作避免重复请求，刷新取消旧请求避免过期响应覆盖状态。
- `frontend/src/api.ts`：带类型的现有 Rust HTTP API，错误响应兼容字符串与 message 对象。
- `frontend/public`：图标与 favicon；`frontend/credits.html` 为署名页入口；根目录 `public/` 是可重建的产物。
- `scripts/build-frontend.sh`：根据 pnpm 锁文件安装依赖，执行 TypeScript 检查及 Vite 构建。标准构建及预编译后端打包均调用它。

Vite `base` 固定为 `/apps/androidtone/`，JS/CSS 使用带内容哈希的资源文件。Rust 服务继续提供带斜杠首页、裸目录首页及已有 API；不需要在宿主运行 Node.js。开发服务器的 API 代理连接既有 Rust 后端。

浏览器回归直接加载生产构建，模拟 Provider API：验证不可用平台操作禁用、APK multipart 导入、非 APK 拒绝、任务重试/删除、应用启动/停止/卸载、卸载取消、文本转义、安装状态轮询、HTTP 错误提示，以及 1120/390 宽度无横向溢出。5 项 Playwright 测试通过。模拟 API 的浏览器测试不代表 Waydroid 或 Android 窗口呈现验收。

`scripts/check-frontend.py` 解析真实服务首页，逐一验证 Vite 引用的 JS/CSS/favicon，检查图标、署名、健康和状态 API，并确认缺失 JS 与图片返回 404。最终包和 VM 验证结果见后续记录。

构建配置参考 [Tailwind 官方 Vite 集成](https://tailwindcss.com/docs/installation/using-vite) 与 [Vite 构建路径说明](https://vite.dev/guide/build.html)。

最终 ARM64 包为 `dist/androidtone-0.1.0-agentos-arm64.agt`，1,665,938,544 字节，SHA-256：`bad21ed8ff65bc612d93c878c95e36510778d489f7b5b8fd889efde26eab6036`。完整归档路径、全部运行时文件和全部前端产物的 SHA-256 复验通过；815 个文件，260 个离线依赖包及两份 Android 镜像保留。

VM 正式安装接口已接收该包，但管理服务随后出现没有容器/监听端口的启动超时并回滚旧版本。用户已批准的管理服务重启曾恢复旧页面，但旧页面不属于本次迁移验收。真实 HTTP 检查已加强为必须存在 `/apps/androidtone/assets/` 下的 Vite JS/CSS，以防误将回滚旧页面当作新版本成功。React 版本的 VM 正式运行验收仍受平台启动问题阻塞；未将本地浏览器测试、安装接口成功或旧首页返回 200 计为完成。

按用户要求再次重建 ARM64 包，重新编译 React 前端、通过 Docker 运行时校验及最终逐文件校验。以上摘要为本次重建产物；此次未再次部署到 VM。

根据用户要求，打包窗口模式改为 `agentos.open.mode: window`。ARM64 包已重新生成，最终归档 manifest 的 `open.mode` 为 `window`，全部文件完整性复验通过。本次未部署到 VM。

界面配色调整为绿色主操作、浅绿色表面、深绿文字及圆角控件，保留用户提供的图标。增加 Provider 未接入状态的摘要与折叠诊断测试；全部 5 项测试通过。原因与接入边界见 [系统连接诊断](provider-connection.md)。

新增中英文与明暗主题适配，13 项回归通过，已重建 window 模式 ARM64 包；完整记录见 [语言与主题说明](localization-theme.md)。
