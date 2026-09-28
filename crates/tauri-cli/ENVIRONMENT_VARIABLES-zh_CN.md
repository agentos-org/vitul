# Vitul 与底层兼容工具使用的环境变量

[English](ENVIRONMENT_VARIABLES.md)

本文整理兼容桌面工具及 CLI 使用的环境变量。许多变量沿用 Tauri 的名字，因为它们属于底层兼容接口。

## CLI 输入变量

这些变量可用于向 CLI 提供输入；部分变量有对应的命令行选项。如果同时设置环境变量和命令行选项，以命令行选项为准。

- `CI`：设置后以 CI 模式运行 CLI，不需要用户交互。
- `TAURI_CLI_CONFIG_DEPTH`：CLI 搜索配置文件时遍历的目录层数。
- `TAURI_CLI_PORT`：CLI 内置开发服务器使用的端口。
- `TAURI_CLI_WATCHER_IGNORE_FILENAME`：控制 `dev` 命令文件监听范围的 `.gitignore` 风格文件名；CLI 会在各目录中查找该文件。
- `TAURI_CLI_NO_DEV_SERVER_WAIT`：构建应用前跳过等待前端开发服务器启动。
- `TAURI_APP_PATH`：Vitul 应用（Rust crate）目录路径，通常为 `<project>/src-vitul`。设置后，CLI 会使用此路径，而不从当前目录搜索配置文件。
- `TAURI_FRONTEND_PATH`：前端目录路径，通常为 `<project>`。设置后，CLI 会使用此路径，而不根据应用目录推断前端目录。
- `VITUL_CONFIG`：要合并到 `vitul.conf.json` 的 JSON 字符串。Vitul 会根据 `--config` 参数设置它，使构建脚本和代码生成步骤使用相同的合并配置；直接运行 `cargo` 编译时也可以自行设置。底层同时识别旧名 `TAURI_CONFIG`。
- `TAURI_LINUX_AYATANA_APPINDICATOR`：设为 `true` 或 `1` 时，Linux 系统托盘强制使用 `libayatana-appindicator`。
- `TAURI_BUNDLER_WIX_FIPS_COMPLIANT`：指定 bundler 的 WiX `FipsCompliant` 选项。
- `TAURI_BUNDLER_TOOLS_GITHUB_MIRROR`：指定 bundler 下载工具时使用的 GitHub 镜像。
- `TAURI_BUNDLER_TOOLS_GITHUB_MIRROR_TEMPLATE`：指定 GitHub 镜像 URL 模板，例如 `https://mirror.example.com/<owner>/<repo>/releases/download/<version>/<asset>`。
- `TAURI_BUNDLER_DMG_IGNORE_CI`：关闭 `.dmg` bundler 对 `CI: true` 的检查。
- `TAURI_SKIP_SIDECAR_SIGNATURE_CHECK`：跳过 sidecar 签名检查。
- `TAURI_SIGNING_PRIVATE_KEY`：用于签署应用包的私钥。`build` 和 `bundle` 命令接受私钥字符串或文件路径；`signer sign` 命令要求传入私钥字符串本身。
- `TAURI_SIGNING_PRIVATE_KEY_PATH`：`signer sign` 命令使用的私钥文件路径。运行签名命令时不能与 `TAURI_SIGNING_PRIVATE_KEY` 同时设置。
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`：`TAURI_SIGNING_PRIVATE_KEY` 对应的私钥密码。
- `TAURI_SIGNING_RPM_KEY`：用于签署 RPM 包的 ASCII-armored GPG 私钥。
- `TAURI_SIGNING_RPM_KEY_PASSPHRASE`：`TAURI_SIGNING_RPM_KEY` 对应的 GPG 密码。
- `TAURI_WINDOWS_SIGNTOOL_PATH`：用于代码签名的 `signtool.exe` 路径。
- `STATIC_VCRUNTIME`：已弃用。请改为在 `vitul.conf.json` 中设置 `build > windows > staticVCRuntime`。
- `VCTOOLS_REDIST_DIR`：启用 `bundle > windows > bundleVCRuntime` 时，覆盖 Visual C++ 可再发行组件的根目录。未设置时，Tauri 会使用随附的 `vswhere.exe` 定位 Visual Studio 并推导该目录。
- `APPLE_CERTIFICATE`：用于代码签名的 `.p12` 证书的 Base64 内容。可以运行 `openssl base64 -in MyCertificate.p12 -out MyCertificate-base64.txt` 生成。
- `APPLE_CERTIFICATE_PASSWORD`：导出证书时设置的密码。
- `APPLE_ID`：用于应用公证的 Apple ID。设置后还必须设置 `APPLE_PASSWORD` 和 `APPLE_TEAM_ID`；也可以改用 `APPLE_API_KEY` 和 `APPLE_API_ISSUER` 进行认证。
- `APPLE_PASSWORD`：应用公证所需的 Apple 密码。可使用 App 专用密码；也可以用 `@keychain:` 或 `@env:` 前缀引用钥匙串项目或环境变量，避免明文填写。
- `APPLE_TEAM_ID`：Apple Developer 团队 ID，可在 Apple Developer 网站的[账户页面](https://developer.apple.com/account)查看。
- `APPLE_API_KEY`：使用 JWT 进行公证认证时，替代 `APPLE_ID` 和 `APPLE_PASSWORD`。也可用于自动管理 iOS 证书和 provisioning profile。详细信息请参阅 Apple 的[创建 API 密钥文档](https://developer.apple.com/documentation/appstoreconnectapi/creating_api_keys_for_app_store_connect_api)。
- `APPLE_API_ISSUER`：设置 `APPLE_API_KEY` 时必须提供的 Issuer ID。
- `APPLE_API_KEY_PATH`：API 密钥 `.p8` 文件路径。macOS 未设置时，bundler 会依次搜索 `./private_keys`、`~/private_keys`、`~/.private_keys` 和 `~/.appstoreconnect/private_keys` 中名为 `AuthKey_<api_key>.p8` 的文件；iOS 构建必须设置此变量。
- `APPLE_SIGNING_IDENTITY`：用于代码签名的身份，覆盖 `vitul.conf.json > bundle > macOS > signingIdentity`。未设置时，如果提供了 `APPLE_CERTIFICATE`，则会据此推导。
- `APPLE_PROVIDER_SHORT_NAME`：Apple ID 关联多个团队时，用来指定应用公证团队的 provider short name；覆盖 `vitul.conf.json > bundle > macOS > providerShortName`。
- `APPLE_DEVELOPMENT_TEAM`：iOS 代码签名使用的团队 ID；覆盖 `vitul.conf.json > bundle > iOS > developmentTeam`。
- `TAURI_WEBVIEW_AUTOMATION`：启用 WebView 自动化（仅 Linux）。
- `TAURI_DEV_ROOT_CERTIFICATE_PATH`：受信任的 PEM 根证书路径。`vitul android dev` 或 `vitul ios dev` 使用 HTTPS 自签名开发服务器时可用。
- `TAURI_ANDROID_PROJECT_PATH`：Android 项目路径，通常为 `<project>/src-vitul/gen/android`。
- `TAURI_IOS_PROJECT_PATH`：iOS 项目路径，通常为 `<project>/src-vitul/gen/apple`。

## CLI 钩子变量

CLI 执行 `beforeDevCommand`、`beforeBuildCommand` 等钩子命令时会设置以下变量，可用于条件式构建前端或执行特定操作：

- `TAURI_ENV_DEBUG`：运行 `dev` 或 `build --debug` 时为 `true`；其他情况下不会设置。请检查变量是否存在，不要将其缺失视为 `false` 字符串。
- `TAURI_ENV_TARGET_TRIPLE`：CLI 正在构建的目标 triple。
- `TAURI_ENV_ARCH`：目标架构，例如 `x86_64`、`aarch64`。
- `TAURI_ENV_PLATFORM`：目标平台，例如 `windows`、`darwin`、`linux`。
- `TAURI_ENV_PLATFORM_VERSION`：构建平台版本。
- `TAURI_ENV_FAMILY`：目标平台族，为 `unix` 或 `windows`。
- `VITUL_DEV_HOST`：前端开发服务器需要监听的 IP 地址，以便设备通过网络访问。仅在 `vitul android dev` 或 `vitul ios dev` 使用 `--host`、运行在实体设备上，或 `build > devUrl` 指向 `0.0.0.0` 时设置。
