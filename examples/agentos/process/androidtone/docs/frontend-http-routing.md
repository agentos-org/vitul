# 前端目录首页 404 修复

2026-10-12，在 VM 安装 AndroidTone 后，Shell 请求 `http://127.0.0.1:10003/apps/androidtone/` 得到 404，导致管理界面打不开。同期健康接口、`/index.html`、`/apps/androidtone/index.html`、CSS 和 API 均返回 200。

原因是 Axum 0.7 的嵌套路由没有将带末尾斜杠的目录入口映射到首页。`ServeDir` 的目录索引设置不能代替显式注册该路由。现在为 `/apps/androidtone/` 显式提供 `index.html`，并为嵌套的裸目录根路径提供相同首页；静态文件和 API 保持原路径。

此前浏览器渲染预览使用独立的资源响应，没有覆盖实际后端目录入口。新增 `scripts/check-frontend.py` 对真实服务检查配置入口、裸目录、显式首页、CSS、JS、PNG 图标、favicon、健康接口和状态 API；不存在的资源必须返回 404。

```sh
python3 scripts/check-frontend.py http://127.0.0.1:10003
```

本机真实后端 HTTP 检查通过。ARM64 后端由 Docker 编译，修复随完整 `.agt` 打包，继续保留用户提供的图标与离线 Waydroid 载荷。当前包摘要以 `dist/androidtone-0.1.0-agentos-arm64.agt.sha256` 为准。

应用管理界面 HTTP 加载与 Android 应用窗口呈现是独立验收项，本次修复针对前者。

VM 验证：修复后的 ARM64 后端在 `agentos/webapp-runtime:1` 中运行，使用 UID/GID 1000、只读根文件系统、全部 capabilities 移除、no-new-privileges、完整应用目录和用户主目录挂载，通过全部 HTTP 检查。正式安装接口成功接收修复包，但受管理启动出现容器停在 `Created`（Pid=0、无 OOM）和 60 秒健康检查超时，随后 AgentOS 回滚了旧后端。重复清理本应用未启动容器、正式更新后，仍出现相同启动问题；不能将独立容器验证当作已恢复 Shell 的正常启动。用户批准后，已重启 AgentOS 管理服务并通过正式接口重新安装、启动修复包。实际受管理容器运行正常，端口为 10001；9 个正向 HTTP 路径全部通过，不存在的静态资源仍返回 404。原目录首页 404 已消除；未将这项检查当作 Android 应用窗口呈现验收。
