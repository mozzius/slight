#!/usr/bin/env bash
# Build a self-contained Codex ACP bundle for one or more target triples.
#
# The bundle contains a pinned Node runtime, the codex-acp broker bundle, and
# the pinned native Codex tree. Every downloaded artifact is verified against
# packaging/codex-acp/versions.env before it is unpacked; the resulting layout
# is what acp-adapters::codex resolves for a shipped host.
#
# Usage:
#   scripts/bundle-codex-acp.sh [--target <triple>]... [--out <dir>] [--minimal]
#
#   --target   Target triple (repeatable). Defaults to the host triple.
#   --out      Output root. Defaults to vendor/codex-acp. Bundles land in
#              <out>/<triple>/.
#   --minimal  Drop the optional codex-code-mode-host binary (~62 MB).
#
# Requirements: bash, curl, tar, openssl, and one of sha256sum/shasum.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
versions_file="$repo_root/packaging/codex-acp/versions.env"
# shellcheck source=/dev/null
source "$versions_file"

out_root="$repo_root/vendor/codex-acp"
targets=()
minimal=0
work=""

cleanup() {
  if [[ -n "$work" && -d "$work" ]]; then
    rm -rf "$work"
  fi
}
trap cleanup EXIT

while [[ $# -gt 0 ]]; do
  case "$1" in
    --target) targets+=("$2"); shift 2 ;;
    --out) out_root="$2"; shift 2 ;;
    --minimal) minimal=1; shift ;;
    -h|--help) sed -n '2,18p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

die() { echo "bundle-codex-acp: $*" >&2; exit 1; }
info() { echo "==> $*" >&2; }

command -v curl >/dev/null || die "curl is required"
command -v tar >/dev/null || die "tar is required"
command -v openssl >/dev/null || die "openssl is required"

sha256_file() {
  if command -v sha256sum >/dev/null; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

sha512_b64_file() {
  openssl dgst -sha512 -binary "$1" | openssl base64 -A
}

host_triple() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os-$arch" in
    Darwin-arm64) echo "aarch64-apple-darwin" ;;
    Darwin-x86_64) echo "x86_64-apple-darwin" ;;
    Linux-aarch64|Linux-arm64) echo "aarch64-unknown-linux-musl" ;;
    Linux-x86_64) echo "x86_64-unknown-linux-musl" ;;
    *) die "unsupported host: $os-$arch" ;;
  esac
}

# aarch64-apple-darwin -> AARCH64_APPLE_DARWIN
triple_key() {
  echo "$1" | tr '[:lower:]-' '[:upper:]_'
}

# aarch64-apple-darwin -> arm64; x86_64-apple-darwin -> x64
node_arch_for() {
  case "$1" in
    aarch64-*) echo "arm64" ;;
    x86_64-*) echo "x64" ;;
    *) die "unsupported target triple: $1" ;;
  esac
}

node_os_for() {
  case "$1" in
    *-apple-darwin) echo "darwin" ;;
    *-unknown-linux-musl) echo "linux" ;;
    *-pc-windows-msvc) die "Windows bundles are not implemented yet" ;;
    *) die "unsupported target triple: $1" ;;
  esac
}

download_verified() {
  # download_verified <url> <dest> <expected: sha256-hex | sha512-base64>
  local url="$1" dest="$2" expected="$3" actual
  info "download $(basename "$url")"
  curl -fsSL "$url" -o "$dest" || die "failed to download $url"
  case "$expected" in
    sha512-*)
      actual="sha512-$(sha512_b64_file "$dest")"
      ;;
    *)
      actual="$(sha256_file "$dest")"
      ;;
  esac
  [[ "$actual" == "$expected" ]] || {
    rm -f "$dest"
    die "hash mismatch for $url
  expected: $expected
  actual:   $actual"
  }
}

npm_url() {
  # npm_url <package> <version>
  local package="$1" version="$2" scope name
  if [[ "$package" == @*/* ]]; then
    scope="${package%%/*}"
    name="${package#*/}"
    echo "https://registry.npmjs.org/${scope}/${name}/-/${name}-${version}.tgz"
  else
    echo "https://registry.npmjs.org/${package}/-/${package}-${version}.tgz"
  fi
}

build_target() {
  local target="$1" key node_os node_arch node_platform
  key="$(triple_key "$target")"
  node_arch="$(node_arch_for "$target")"
  node_os="$(node_os_for "$target")"
  node_platform="$(eval "echo \"\${NODE_PLATFORM_${key}:-}\"")"
  [[ -n "$node_platform" ]] || node_platform="${node_os}-${node_arch}"

  local node_sha codex_version codex_integrity
  node_sha="$(eval "echo \"\${NODE_SHA256_${key}:-}\"")"
  [[ -n "$node_sha" ]] || die "no pinned Node sha256 for $target in versions.env"
  codex_version="$(eval "echo \"\${CODEX_PLATFORM_VERSION_${key}:-}\"")"
  codex_integrity="$(eval "echo \"\${CODEX_PLATFORM_INTEGRITY_${key}:-}\"")"
  [[ -n "$codex_version" && -n "$codex_integrity" ]] || die "no pinned @openai/codex entry for $target in versions.env"

  local target_dir="$out_root/$target"
  if [[ -e "$target_dir" ]]; then
    info "removing existing $target_dir"
    rm -rf "$target_dir"
  fi
  mkdir -p "$target_dir/bin" "$target_dir/lib/codex-acp" "$target_dir/licenses"

  work="$(mktemp -d "${TMPDIR:-/tmp}/codex-acp-bundle.XXXXXX")"
  # 1. Node runtime.
  local node_archive="node-v${NODE_VERSION}-${node_platform}.tar.gz"
  local node_url="https://nodejs.org/dist/v${NODE_VERSION}/${node_archive}"
  download_verified "$node_url" "$work/$node_archive" "$node_sha"
  mkdir -p "$work/node"
  tar -xzf "$work/$node_archive" -C "$work/node" --strip-components=1 \
    "node-v${NODE_VERSION}-${node_platform}/bin/node" \
    "node-v${NODE_VERSION}-${node_platform}/LICENSE"
  cp "$work/node/bin/node" "$target_dir/bin/node"
  [[ -f "$work/node/LICENSE" ]] || die "Node tarball is missing its LICENSE"
  cp "$work/node/LICENSE" "$target_dir/licenses/LICENSE.node"

  # 2. codex-acp broker.
  local acp_url acp_archive
  acp_url="$(npm_url "$CODEX_ACP_PATH" "$CODEX_ACP_VERSION")"
  acp_archive="$work/codex-acp.tgz"
  download_verified "$acp_url" "$acp_archive" "$CODEX_ACP_INTEGRITY"
  mkdir -p "$work/acp"
  tar -xzf "$acp_archive" -C "$work/acp"
  cp "$work/acp/package/dist/index.js" "$target_dir/lib/codex-acp/index.js"
  [[ -f "$work/acp/package/LICENSE" ]] || die "codex-acp tarball is missing its LICENSE"
  cp "$work/acp/package/LICENSE" "$target_dir/licenses/LICENSE.codex-acp"

  # 3. Native Codex tree.
  local codex_url codex_archive
  codex_url="$(npm_url "$OPENAI_CODEX_PATH" "$codex_version")"
  codex_archive="$work/codex.tgz"
  download_verified "$codex_url" "$codex_archive" "$codex_integrity"
  mkdir -p "$work/codex"
  tar -xzf "$codex_archive" -C "$work/codex"
  local vendor="$work/codex/package/vendor/$target"
  [[ -d "$vendor" ]] || die "platform package for $codex_version has no vendor/$target"
  cp -R "$vendor/." "$target_dir/"
  # The npm platform package ships only `vendor/`; the Apache-2.0 text comes
  # from the pinned Codex source tree and is tracked in-repo.
  local codex_license="$repo_root/packaging/codex-acp/licenses/LICENSE.openai-codex"
  [[ -f "$codex_license" ]] || die "missing tracked Codex license at $codex_license"
  cp "$codex_license" "$target_dir/licenses/LICENSE.openai-codex"
  if [[ "$minimal" == "1" ]]; then
    rm -f "$target_dir/bin/codex-code-mode-host"
  fi

  chmod +x "$target_dir/bin/node" "$target_dir/bin/codex"

  # 4. Notices and provenance.
  cat > "$target_dir/NOTICE" <<EOF
This directory contains the self-contained Codex ACP runtime bundled with the
Slight host. It ships the following third-party components:

  @agentclientprotocol/codex-acp ${CODEX_ACP_VERSION}  Apache-2.0
  @openai/codex ${codex_version}                       Apache-2.0
  Node.js ${NODE_VERSION}                              MIT

The full license text for each component is in licenses/. The version, source
URL, and integrity of each pinned artifact are recorded in bundle.json.

codex-acp is licensed under the Apache License 2.0. Codex is a product of
OpenAI, licensed under the Apache License 2.0. Node.js is licensed under the
MIT license.
EOF

  cat > "$target_dir/bundle.json" <<EOF
{
  "schema": 1,
  "target": "$target",
  "generated_at_utc": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "components": {
    "codex_acp": {
      "package": "$CODEX_ACP_PATH",
      "version": "$CODEX_ACP_VERSION",
      "url": "$acp_url",
      "integrity": "$CODEX_ACP_INTEGRITY",
      "license": "Apache-2.0"
    },
    "codex": {
      "package": "$OPENAI_CODEX_PATH",
      "version": "$codex_version",
      "url": "$codex_url",
      "integrity": "$codex_integrity",
      "license": "Apache-2.0"
    },
    "node": {
      "version": "$NODE_VERSION",
      "platform": "$node_platform",
      "url": "$node_url",
      "sha256": "$node_sha",
      "license": "MIT"
    }
  },
  "layout": {
    "program": "bin/node",
    "entry": "lib/codex-acp/index.js",
    "codex_path": "bin/codex"
  }
}
EOF

  info "bundle ready: $target_dir"
  du -sh "$target_dir" 2>/dev/null >&2 || true

  rm -rf "$work"
  work=""
}

if [[ ${#targets[@]} -eq 0 ]]; then
  targets=("$(host_triple)")
fi

for target in "${targets[@]}"; do
  build_target "$target"
done
