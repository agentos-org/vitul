use super::schema::NetworkConfig;
use anyhow::{Context, Result, bail};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
  collections::BTreeSet,
  fs::{self, File},
  io::Read,
  net::{IpAddr, Ipv4Addr, Ipv6Addr},
  path::{Component, Path},
};

pub(crate) const NETWORK_PROVIDER_ID: &str = "org.agentos.network";
pub(crate) const NETWORK_PROVIDER_METHODS: &[&str] = &[
  "capabilities.get",
  "authorization.request",
  "authorization.get",
  "authorization.cancel",
  "proxy.get",
  "proxy.enable",
  "proxy.disable",
  "tun.get",
  "tun.start",
  "tun.stop",
  "lease.list",
  "lease.release",
  "events.poll",
];

impl NetworkConfig {
  pub(crate) fn validate(&self) -> Result<()> {
    if self.protocol_version != 1 {
      bail!("agentos.network.protocolVersion must be 1; set it to 1 for manifest v1");
    }
    if self.executables.is_empty() || self.executables.len() > 8 {
      bail!("agentos.network.executables must contain 1 to 8 items; add 1–8 package executables");
    }
    if self.proxy_profiles.len() > 16 {
      bail!("agentos.network.proxyProfiles accepts at most 16 items; remove extra profiles");
    }
    if self.tun_profiles.len() > 16 {
      bail!("agentos.network.tunProfiles accepts at most 16 items; remove extra profiles");
    }
    if self.proxy_profiles.is_empty() && self.tun_profiles.is_empty() {
      bail!("agentos.network must declare at least one proxyProfiles or tunProfiles entry");
    }

    let mut ids = BTreeSet::new();
    for (index, executable) in self.executables.iter().enumerate() {
      validate_id(
        &executable.id,
        &format!("agentos.network.executables[{index}].id"),
      )?;
      if !ids.insert(&executable.id) {
        bail!(
          "agentos.network.executables[{index}].id duplicates another network id; use unique IDs across all profiles and executables"
        );
      }
      validate_package_path(
        &executable.path,
        &format!("agentos.network.executables[{index}].path"),
      )?;
    }

    for (index, profile) in self.proxy_profiles.iter().enumerate() {
      let field = format!("agentos.network.proxyProfiles[{index}]");
      validate_id(&profile.id, &format!("{field}.id"))?;
      if !ids.insert(&profile.id) {
        bail!(
          "{field}.id duplicates another network id; use unique IDs across all profiles and executables"
        );
      }
      validate_executable_reference(&profile.executable, &field, &self.executables)?;
      if profile.http.is_none() && profile.https.is_none() && profile.socks.is_none() {
        bail!("{field} must declare at least one of http, https, or socks");
      }
      if profile.http.is_some() && profile.https.is_none() {
        bail!(
          "{field}.https is required when http is declared; add an HTTPS endpoint to avoid protocol v1 silently extending HTTP settings"
        );
      }
      for (name, endpoint) in [
        ("http", profile.http.as_ref()),
        ("https", profile.https.as_ref()),
        ("socks", profile.socks.as_ref()),
      ] {
        if let Some(endpoint) = endpoint {
          validate_proxy_endpoint(endpoint, &format!("{field}.{name}"))?;
        }
      }
      if profile.bypass.len() > 64 {
        bail!("{field}.bypass accepts at most 64 rules; remove extra entries");
      }
      let mut bypass = BTreeSet::new();
      for (bypass_index, rule) in profile.bypass.iter().enumerate() {
        if rule.len() > 253 || !valid_bypass_rule(rule) {
          bail!(
            "{field}.bypass[{bypass_index}] is invalid; use a hostname, *.domain pattern, IP literal, or CIDR"
          );
        }
        if !bypass.insert(rule) {
          bail!("{field}.bypass[{bypass_index}] duplicates a bypass rule; remove the duplicate");
        }
      }
    }

    for (index, profile) in self.tun_profiles.iter().enumerate() {
      let field = format!("agentos.network.tunProfiles[{index}]");
      validate_id(&profile.id, &format!("{field}.id"))?;
      if !ids.insert(&profile.id) {
        bail!(
          "{field}.id duplicates another network id; use unique IDs across all profiles and executables"
        );
      }
      validate_executable_reference(&profile.executable, &field, &self.executables)?;
      validate_package_path(
        &profile.configuration.path,
        &format!("{field}.configuration.path"),
      )?;
      if profile.arguments.len() > 64 {
        bail!("{field}.arguments accepts at most 64 strings; remove extra arguments");
      }
      let mut total_argument_units = 0usize;
      for (argument_index, argument) in profile.arguments.iter().enumerate() {
        let argument_field = format!("{field}.arguments[{argument_index}]");
        let utf16_len = argument.encode_utf16().count();
        total_argument_units += utf16_len;
        if utf16_len > 4096
          || argument.chars().any(|character| character.is_control())
          || argument.starts_with('/')
          || argument.split('/').any(|part| part == "..")
          || has_unsupported_placeholder(argument)
        {
          bail!(
            "{argument_field} is invalid; use a bounded fixed argv string, with `${{config}}` as the only placeholder and no absolute path or `..` segment"
          );
        }
      }
      if total_argument_units > 24 * 1024 {
        bail!("{field}.arguments exceeds 24 KiB; shorten or remove arguments");
      }
      if profile.interface_name.len() > 15
        || profile.interface_name == "."
        || profile.interface_name == ".."
        || !profile
          .interface_name
          .bytes()
          .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
        || profile.interface_name.is_empty()
      {
        bail!(
          "{field}.interfaceName must be a Linux interface name of 1 to 15 ASCII letters, digits, `.`, `_`, or `-`"
        );
      }
      if profile.outer_endpoints.is_empty() || profile.outer_endpoints.len() > 8 {
        bail!(
          "{field}.outerEndpoints must contain 1 to 8 fixed IPv4 endpoints; add a fixed endpoint or remove extras"
        );
      }
      let mut endpoint_ids = BTreeSet::new();
      for (endpoint_index, endpoint) in profile.outer_endpoints.iter().enumerate() {
        let endpoint_field = format!("{field}.outerEndpoints[{endpoint_index}]");
        let address = parse_ip(&endpoint.address, &format!("{endpoint_field}.address"))?;
        if !matches!(address, IpAddr::V4(_)) || !usable_unicast(address) {
          bail!(
            "{endpoint_field}.address must be a usable fixed IPv4 literal; hostnames, IPv6, loopback, link-local, multicast, unspecified, and broadcast addresses are unsupported"
          );
        }
        if !(1..=65535).contains(&endpoint.port) {
          bail!("{endpoint_field}.port must be from 1 to 65535");
        }
        if !matches!(endpoint.protocol.as_str(), "tcp" | "udp") {
          bail!("{endpoint_field}.protocol must be `tcp` or `udp`");
        }
        if !endpoint_ids.insert((address, endpoint.port, endpoint.protocol.as_str())) {
          bail!("{endpoint_field} duplicates an outer endpoint; remove the duplicate");
        }
      }
      if !profile.ipv4 || !profile.ipv6 {
        bail!(
          "{field}.ipv4 and {field}.ipv6 must both be true for the protocol v1 dual-stack host capture"
        );
      }
      if profile.dns_servers.is_empty() || profile.dns_servers.len() > 8 {
        bail!("{field}.dnsServers must contain 1 to 8 unique unicast IP literals");
      }
      let mut dns = BTreeSet::new();
      for (dns_index, address) in profile.dns_servers.iter().enumerate() {
        let dns_field = format!("{field}.dnsServers[{dns_index}]");
        let parsed = parse_ip(address, &dns_field)?;
        if !usable_unicast(parsed) {
          bail!(
            "{dns_field} must be a usable unicast IPv4 or IPv6 literal; hostnames, loopback, link-local, multicast, unspecified, and broadcast addresses are unsupported"
          );
        }
        if !dns.insert(parsed) {
          bail!("{dns_field} duplicates a DNS server after IP normalization; remove the duplicate");
        }
        if profile
          .outer_endpoints
          .iter()
          .any(|endpoint| endpoint.address.parse::<IpAddr>().ok() == Some(parsed))
        {
          bail!(
            "{dns_field} cannot reuse an outer tunnel endpoint address; choose a separate DNS server"
          );
        }
      }
      if profile.exit_policy != "restore-previous" {
        bail!(
          "{field}.exitPolicy must be `restore-previous`; this is the only protocol v1 cleanup policy"
        );
      }
    }
    Ok(())
  }

  /// Build the protocol manifest from final staged files so every digest is
  /// bound to the exact architecture-specific bytes placed in the package.
  pub(crate) fn manifest_for_staging(&self, staging_root: &Path) -> Result<Value> {
    self.validate()?;
    let root = staging_root
      .canonicalize()
      .context("could not resolve AgentOS package staging directory")?;
    let mut manifest = serde_json::to_value(self).context("could not encode agentos.network")?;
    for (index, executable) in self.executables.iter().enumerate() {
      let digest = staged_digest(&root, &executable.path, true).with_context(|| {
        format!(
          "invalid agentos.network.executables[{index}].path `{}`; add this executable as a regular, executable build artifact under that package path",
          executable.path
        )
      })?;
      manifest["executables"][index]["sha256"] = Value::String(digest);
    }
    for (index, profile) in self.tun_profiles.iter().enumerate() {
      let digest = staged_digest(&root, &profile.configuration.path, false).with_context(|| {
        format!(
          "invalid agentos.network.tunProfiles[{index}].configuration.path `{}`; add a regular build artifact under that package path",
          profile.configuration.path
        )
      })?;
      manifest["tunProfiles"][index]["configuration"]["sha256"] = Value::String(digest);
      manifest["tunProfiles"][index]["configuration"]["path"] =
        Value::String(profile.configuration.path.clone());
      for (endpoint_index, endpoint) in profile.outer_endpoints.iter().enumerate() {
        let address = endpoint
          .address
          .parse::<IpAddr>()
          .expect("validated outer endpoint address")
          .to_string();
        manifest["tunProfiles"][index]["outerEndpoints"][endpoint_index]["address"] =
          Value::String(address);
      }
      for (dns_index, address) in profile.dns_servers.iter().enumerate() {
        manifest["tunProfiles"][index]["dnsServers"][dns_index] = Value::String(
          address
            .parse::<IpAddr>()
            .expect("validated DNS address")
            .to_string(),
        );
      }
    }
    Ok(manifest)
  }
}

fn validate_id(id: &str, field: &str) -> Result<()> {
  let bytes = id.as_bytes();
  if bytes.is_empty()
    || bytes.len() > 64
    || !bytes[0].is_ascii_alphanumeric()
    || !bytes
      .iter()
      .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(byte))
  {
    bail!("{field} must match `[A-Za-z0-9][A-Za-z0-9._-]{{0,63}}`");
  }
  Ok(())
}

fn validate_package_path(path: &str, field: &str) -> Result<()> {
  if path.is_empty()
    || path.len() > 512
    || path.starts_with('/')
    || !path.is_ascii()
    || path
      .split('/')
      .any(|part| part.is_empty() || part == "." || part == "..")
    || !path
      .bytes()
      .all(|byte| byte.is_ascii_alphanumeric() || b"._/-".contains(&byte))
    || Path::new(path)
      .components()
      .any(|component| !matches!(component, Component::Normal(_)))
  {
    bail!(
      "{field} must be a normalized package-relative path of at most 512 ASCII characters using letters, digits, `.`, `_`, `-`, and `/`; use a path covered by build.artifacts"
    );
  }
  Ok(())
}

fn validate_executable_reference(
  executable_id: &str,
  profile_field: &str,
  executables: &[super::schema::NetworkExecutable],
) -> Result<()> {
  validate_id(executable_id, &format!("{profile_field}.executable"))?;
  if !executables
    .iter()
    .any(|executable| executable.id == executable_id)
  {
    bail!(
      "{profile_field}.executable references undeclared ID `{executable_id}`; add it to agentos.network.executables"
    );
  }
  Ok(())
}

fn validate_proxy_endpoint(
  endpoint: &super::schema::NetworkProxyEndpoint,
  field: &str,
) -> Result<()> {
  if !valid_network_host(&endpoint.host) {
    bail!(
      "{field}.host must be a hostname or IP literal without a URL scheme, credentials, path, or whitespace"
    );
  }
  if !(1..=65535).contains(&endpoint.port) {
    bail!("{field}.port must be an integer from 1 to 65535");
  }
  Ok(())
}

fn valid_network_host(host: &str) -> bool {
  if host.is_empty()
    || host.len() > 253
    || host.trim() != host
    || host
      .bytes()
      .any(|byte| byte <= 0x20 || b"/@?#".contains(&byte))
  {
    return false;
  }
  if host.parse::<IpAddr>().is_ok() {
    return true;
  }
  let normalized = host.strip_suffix('.').unwrap_or(host);
  normalized.split('.').all(|label| {
    let bytes = label.as_bytes();
    !bytes.is_empty()
      && bytes.len() <= 63
      && bytes[0].is_ascii_alphanumeric()
      && bytes[bytes.len() - 1].is_ascii_alphanumeric()
      && bytes
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-')
  })
}

fn valid_bypass_rule(rule: &str) -> bool {
  if rule == "localhost" {
    return true;
  }
  if let Some(domain) = rule.strip_prefix("*.") {
    return valid_network_host(domain);
  }
  if rule.parse::<IpAddr>().is_ok() {
    return true;
  }
  if let Some((address, prefix)) = rule.rsplit_once('/') {
    let Ok(address) = address.parse::<IpAddr>() else {
      return false;
    };
    let Ok(prefix) = prefix.parse::<u8>() else {
      return false;
    };
    return match address {
      IpAddr::V4(_) => prefix <= 32,
      IpAddr::V6(_) => prefix <= 128,
    };
  }
  valid_network_host(rule)
}

fn parse_ip(value: &str, field: &str) -> Result<IpAddr> {
  value
    .parse::<IpAddr>()
    .with_context(|| format!("{field} must be an IP literal; use a numeric IPv4 or IPv6 address"))
}

fn usable_unicast(address: IpAddr) -> bool {
  match address {
    IpAddr::V4(address) => usable_ipv4(address),
    IpAddr::V6(address) => usable_ipv6(address),
  }
}

fn usable_ipv4(address: Ipv4Addr) -> bool {
  let octets = address.octets();
  !address.is_unspecified()
    && !address.is_loopback()
    && !address.is_multicast()
    && octets != [255, 255, 255, 255]
    && !(octets[0] == 169 && octets[1] == 254)
}

fn usable_ipv6(address: Ipv6Addr) -> bool {
  let segments = address.segments();
  !address.is_unspecified()
    && !address.is_loopback()
    && !address.is_multicast()
    && (segments[0] & 0xffc0) != 0xfe80
}

fn has_unsupported_placeholder(argument: &str) -> bool {
  let without_config = argument.replace("${config}", "");
  without_config.contains("${") || without_config.contains('{') || without_config.contains('}')
}

fn staged_digest(staging_root: &Path, package_path: &str, executable: bool) -> Result<String> {
  validate_package_path(package_path, "network package path")?;
  let path = staging_root.join(package_path);
  let metadata = fs::symlink_metadata(&path)
    .with_context(|| format!("package file is missing: {}", path.display()))?;
  if !metadata.file_type().is_file() {
    bail!(
      "package path must resolve to a regular file without a symlink leaf: {}",
      path.display()
    );
  }
  let canonical = path
    .canonicalize()
    .with_context(|| format!("could not resolve staged file {}", path.display()))?;
  if !canonical.starts_with(staging_root) {
    bail!(
      "package path escapes the staging directory: {}",
      path.display()
    );
  }
  if executable {
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      if metadata.permissions().mode() & 0o111 == 0 {
        bail!(
          "package executable has no executable permission bit: {}",
          path.display()
        );
      }
    }
    #[cfg(not(unix))]
    bail!("cannot verify executable permission bits on this build host");
  }
  let mut file =
    File::open(&path).with_context(|| format!("could not open staged file {}", path.display()))?;
  let mut hasher = Sha256::new();
  let mut buffer = [0u8; 64 * 1024];
  loop {
    let read = file
      .read(&mut buffer)
      .with_context(|| format!("could not read staged file for SHA-256: {}", path.display()))?;
    if read == 0 {
      break;
    }
    hasher.update(&buffer[..read]);
  }
  Ok(format!("{:x}", hasher.finalize()))
}
