# Generic Native App network example

[`vitul.conf.json.example`](vitul.conf.json.example) declares one proxy profile
and one AgentOS Native App network v1 TUN profile. The package includes each
target's network executable at its declared package path. It does not include
a fixed TUN configuration file.

The TUN profile declares `configuration.source: "runtime-snapshot"` and a
`maxBytes` limit from 1024 through 4194304. Its fixed argv contains
`${config}`, which AgentOS replaces with the configuration snapshot supplied
by the App for that `tun.start` request. The App submits the current UTF-8
configuration text, resolved outer endpoints, and DNS servers at runtime, for
example:

```json
{
  "profileId": "full-tunnel",
  "idempotencyKey": "a-new-key-for-this-user-operation",
  "configuration": "<current UTF-8 kernel configuration>",
  "outerEndpoints": [
    { "address": "203.0.113.17", "port": 443, "protocol": "udp" }
  ],
  "dnsServers": ["1.1.1.1", "2606:4700:4700::1111"]
}
```

The addresses above illustrate the request shape. An App must provide the
targets and DNS values that match its current configuration. `configuration`
is limited by the profile's `maxBytes`; AgentOS v1 accepts 1–256 unique
numeric IPv4 outer endpoints and 1–8 unique numeric IPv4/IPv6 DNS addresses.

Copy the example to a Vitul project as `vitul.conf.json`, provide the
ARM64 and x64 executable artifacts at the paths listed in `build.artifacts`,
then run `vitul agentos validate`, `vitul agentos inspect`, and
`vitul build --target agentos`. Vitul validates the package declaration and
hashes each target's staged executable. These checks do not establish whether
an upstream tunnel kernel works with the AgentOS sidecar.
