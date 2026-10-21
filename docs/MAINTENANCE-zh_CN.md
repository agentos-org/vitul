# 维护与发布规则

Vitul 当前处于 1.0 之前的预览阶段。指定的公开项目主页、问题反馈、Pull Request、发布说明和产物入口为 [github.com/agentos-org/vitul](https://github.com/agentos-org/vitul)。项目负责人为 [@LiteVar](https://github.com/LiteVar)。

## 支持承诺

- 1.0 之前属于预览阶段。API、配置、项目模板和包格式可能在次版本间变化；影响现有项目的变更会附带迁移说明。
- 仅最新的已发布预览版本纳入修复范围。修复按维护者时间尽力处理，不承诺响应时限，也不维护长期支持分支。
- 1.0 之后，Vitul 对外项目版本遵循语义化版本：次版本增加兼容功能，修订版本修复问题，主版本允许不兼容变更。维护最新的稳定次版本；较旧主版本不再常规修复。
- 安全修复面向最新受支持版本处理，不承诺披露或修复的固定时限。
- 运行时和 API 依赖包的版本号可能跟随 Vitul 的 Tauri 派生源码基线。这表示源码来源，不承诺 Tauri App、API 或插件兼容。每个 Vitul 发布版本都必须列出 CLI、API、运行时和项目模板的版本对应关系。
- 当前 Rust CLI（`vitul-cli`）版本为 `0.2.0`；JavaScript CLI 绑定通过 `@litevar/vitul-cli` 独立发布，当前源码版本为 `3.5.0`。

### 当前源码的版本对应关系

| 组件                                                            | 源码版本 | 含义                                              |
| --------------------------------------------------------------- | -------- | ------------------------------------------------- |
| Vitul Rust CLI（`vitul-cli`）                                   | `0.2.0`  | 当前 Vitul 预览版 CLI 版本。                      |
| 桌面运行时源码（`tauri`、`tauri-runtime`、`tauri-runtime-wry`） | `2.12.0` | Tauri 派生源码基线，不代表兼容承诺。              |
| JavaScript 宿主 API（`@tauri-apps/api`）                        | `2.12.0` | 继承的包身份，不承诺 Tauri App 或插件可运行。     |
| JavaScript CLI 绑定（`@litevar/vitul-cli`）                     | `3.5.0`  | 原生 CLI 包；版本跟随 CLI 绑定发布。              |
| 全局 npm 命令（`@litevar/vitul`）                               | `0.5.0`  | `vitul` 命令的包装包。                            |
| npm 项目创建器（`create-vitul-app`）                            | `0.5.0`  | `npm create vitul-app` 使用的包装包。             |
| 生成的 App 和后端默认版本                                       | `0.1.0`  | 脚手架应用版本，与 Vitul CLI 版本相互独立。       |
| AgentOS App manifest                                            | `1`      | manifest schema 版本，与 Vitul 发布版本相互独立。 |

精确依赖版本以 workspace lockfile 为准。每次 Vitul 发布若变更表中版本，都应同步更新此表。

## 贡献与审查

- 在 GitHub Issues 和 Pull Requests 中反馈问题、提出功能需求和代码变更。运行时问题请提供操作系统、Vitul 版本和最小复现。
- 每项变更进入默认分支前都由维护者审查。运行时行为、权限、打包或协议约定的变更需要相应测试，并更新支持范围或兼容性文档。
- 默认分支应禁止强制推送和删除，要求通过 Pull Request 合并、至少一位维护者批准并通过必要检查。开放外部贡献前，应将 `Vitul AgentOS ARM64 package smoke` 配置为必需检查。
- 合并前解决所有审查意见。保留继承上游代码的来源和许可证声明。

## 发布

- JavaScript CLI 发布使用 `v<CLI 版本>` tag，与 `packages/cli/package.json` 一致（下个版本为 `v3.5.0`）。Rust 预览 CLI 和 npm 命令包装包保持上表中的独立版本号；CLI tag 不改变运行时的预览支持承诺。
- 每次发布都包含变更记录、支持平台说明、已测试的工具链信息，以及可下载产物的校验和。GitHub Releases 是公开发布记录。
- `@litevar/vitul-cli` 的 npm 原生绑定通过 GitHub Actions 发布，需先创建对应版本的 GitHub Release。`@litevar/vitul`（命令名 `vitul`）和 `create-vitul-app` 包由另一条手动发布工作流处理。
- 不要使用继承自 Tauri 的 Rust CLI 或 Covector 工作流发布 Vitul 包；它们仍然面向上游身份，不属于 Vitul 发布流程。
- 每次 npm 发布前，维护者需要同步 CLI 包版本映射，并在干净生成项目中验证安装流程。

## 开放外部贡献前需要配置的仓库设置

仓库管理员需要为默认分支配置保护规则、启用 GitHub 私密漏洞报告，并在负责人 GitHub 个人资料或仓库 About 区域公布行为准则问题的私密联系途径。这些属于 GitHub 仓库设置，无法由源码文件启用。`CODEOWNERS` 已指定 `@LiteVar`；启用基于 CODEOWNERS 的审查规则前，请确认该账号具有所需的仓库权限。
