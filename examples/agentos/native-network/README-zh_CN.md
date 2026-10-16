# 通用 Native App 网络示例

[`vitul.conf.json.example`](vitul.conf.json.example) 声明一个代理 profile 和一个
AgentOS Native App 网络协议 v1 TUN profile。包内包含各目标对应的网络 executable；
不包含固定 TUN 配置文件。

TUN profile 声明 `configuration.source: "runtime-snapshot"`，并将 `maxBytes`
限制在 1024 至 4194304。固定 argv 必须包含 `${config}`；AgentOS 会将它替换为 App
在该次 `tun.start` 请求中提交的配置快照。App 在运行时提交当前 UTF-8 配置文本、已解析
的 outer endpoint 和 DNS，例如：

```json
{
  "profileId": "full-tunnel",
  "idempotencyKey": "本次用户操作使用的新 key",
  "configuration": "<当前 UTF-8 内核配置>",
  "outerEndpoints": [
    { "address": "203.0.113.17", "port": 443, "protocol": "udp" }
  ],
  "dnsServers": ["1.1.1.1", "2606:4700:4700::1111"]
}
```

上述地址仅用于说明请求结构。App 应提交与当前配置相符的目标和 DNS。配置正文受 profile
的 `maxBytes` 限制；AgentOS v1 接受 1–256 个唯一的数字 IPv4 outer endpoint，以及
1–8 个唯一的数字 IPv4/IPv6 DNS 地址。

将示例复制到 Vitul 项目并命名为 `vitul.conf.json`，按 `build.artifacts` 中的路径准备
ARM64 和 x64 executable，然后运行 `vitul agentos validate`、`vitul agentos inspect`
和 `vitul build --target agentos`。Vitul 校验包声明，并根据各目标最终 staging 中的
executable 计算摘要。这些检查不能证明某个上游隧道内核能否与 AgentOS sidecar 配合运行。
