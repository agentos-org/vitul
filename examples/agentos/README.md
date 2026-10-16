# AgentOS app examples

This directory groups installable AgentOS packages by their runtime profile:

- The [WebApp localization and theme guide](process/docs/webapp-localization-theme.md)
  explains how to read host locale and appearance and handle live changes.

- [Process apps](process/README.md) run an app process in AgentOS. The examples
  include both native host execution and container sandboxing.
- [Generic Native App network configuration](native-network/README.md) shows
  the manifest-v1 proxy and TUN profile declarations supported by Vitul.
- [URL apps](url/README.md) open an existing HTTPS website in an AgentOS app
  window without packaging a local server.
