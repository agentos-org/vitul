#!/usr/bin/env bash
set -euo pipefail

rewrite_local_proxy_host() {
  local value="$1"
  value="${value//127.0.0.1/host.docker.internal}"
  value="${value//localhost/host.docker.internal}"
  printf '%s' "$value"
}

export http_proxy="$(rewrite_local_proxy_host "${http_proxy:-${HTTP_PROXY:-}}")"
export https_proxy="$(rewrite_local_proxy_host "${https_proxy:-${HTTPS_PROXY:-}}")"
export all_proxy="$(rewrite_local_proxy_host "${all_proxy:-${ALL_PROXY:-}}")"
export HTTP_PROXY="$(rewrite_local_proxy_host "${HTTP_PROXY:-}")"
export HTTPS_PROXY="$(rewrite_local_proxy_host "${HTTPS_PROXY:-}")"
export ALL_PROXY="$(rewrite_local_proxy_host "${ALL_PROXY:-}")"
export no_proxy="${no_proxy:-${NO_PROXY:-}}"
export NO_PROXY="${NO_PROXY:-}"

APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_ARCH="${VITUL_TARGET_ARCH:-arm64}"
mkdir -p "${APP_DIR}/bin"

case "${TARGET_ARCH}" in
  arm64) DOCKER_PLATFORM="linux/arm64"; EXPECTED_ARCH="aarch64" ;;
  x64) DOCKER_PLATFORM="linux/amd64"; EXPECTED_ARCH="x86_64" ;;
  *) echo "Unsupported AgentOS target architecture: ${TARGET_ARCH}" >&2; exit 1 ;;
esac

HOST_OS="$(uname -s)"
HOST_ARCH="$(uname -m)"
export CMAKE_POLICY_VERSION_MINIMUM="${CMAKE_POLICY_VERSION_MINIMUM:-3.5}"
if [ "${HOST_OS}" = "Linux" ] && [ "${HOST_ARCH}" = "${EXPECTED_ARCH}" ]; then
  cargo build --manifest-path "${APP_DIR}/backend/Cargo.toml" --release --locked
else
  if ! command -v docker >/dev/null 2>&1; then
    echo "Building AgentOS ${TARGET_ARCH} musicd on this host requires Docker." >&2
    exit 1
  fi
  DOCKER_PROXY_ARGS=()
  DOCKER_CACHE_ARGS=()
  CARGO_REGISTRY_CACHE="${CARGO_HOME:-${HOME}/.cargo}/registry"
  if [ -d "${CARGO_REGISTRY_CACHE}" ]; then
    DOCKER_CACHE_ARGS+=(-v "${CARGO_REGISTRY_CACHE}:/usr/local/cargo/registry")
  fi
  for proxy_variable in HTTP_PROXY HTTPS_PROXY ALL_PROXY NO_PROXY http_proxy https_proxy all_proxy no_proxy; do
    if [ -n "${!proxy_variable:-}" ]; then
      DOCKER_PROXY_ARGS+=(--env "${proxy_variable}")
    fi
  done
  docker run --rm "${DOCKER_PROXY_ARGS[@]}" "${DOCKER_CACHE_ARGS[@]}" --platform "${DOCKER_PLATFORM}" \
    -v "${APP_DIR}:/work" -w /work/backend \
    rust:1-bookworm bash -c \
    "apt-get update -qq && apt-get install -y -qq cmake g++ libclang-dev >/dev/null && CMAKE_POLICY_VERSION_MINIMUM=3.5 cargo build --release --locked"
fi

SOURCE_BINARY="${APP_DIR}/backend/target/release/musicd"
if [ ! -x "${SOURCE_BINARY}" ]; then
  echo "Rust build did not produce ${SOURCE_BINARY}" >&2
  exit 1
fi
cp "${SOURCE_BINARY}" "${APP_DIR}/bin/musicd"
chmod 755 "${APP_DIR}/bin/musicd"
