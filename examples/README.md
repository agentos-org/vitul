# Vitul examples

Examples are grouped by what they demonstrate:

- [Starter](starter/README.md) contains the recommended Vitul desktop and
  AgentOS starter app.
- [AgentOS apps](agentos/README.md) groups installable AgentOS examples by
  package runtime profile.

The desktop API integration fixture used by CI lives under
[tests/fixtures/desktop-api](../tests/fixtures/desktop-api/README.md). It covers
compatibility API behavior and is intended for validation work.

See each README for build requirements and run instructions.

## Select and reuse by requirement

| Requirement | Entry point | Read together |
| --- | --- | --- |
| New app, business backend, cross-platform host calls | [Starter](starter/README.md) | Root config, frontend calls, OpenTool backend, target run instructions |
| Existing service or program | [AgentOS process examples](agentos/process/README.md) | Container/native mode, build/start scripts, dependencies, data, cleanup |
| Host language and appearance | [WebApp integration](agentos/process/docs/webapp-localization-theme.md) | Initial context, change subscriptions, translations/styles |
| Native network integration | [Network example](agentos/native-network/README.md) | Profile, permissions, binaries, host services, real network acceptance |
| Hosted website, no local service | [URL examples](agentos/url/README.md) | HTTPS website and remote-web constraints |
| Compatibility API validation | [Integration fixture](../tests/fixtures/desktop-api/README.md) | Test scope; a fixture is not a product support commitment |

Before reuse, compare platform, execution mode, CLI/host versions, and desired
behavior. Trace configuration, code, assets, build, startup, authorization, and
acceptance separately. Some examples assemble manifests directly; others use the
unified CLI. Read the README and scripts before copying JSON. For uncovered
capabilities, check [Support](../docs/SUPPORT.md) and
[Configuration](../crates/vitul-cli/src/agentos_config/README.md).
