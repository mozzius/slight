#!/usr/bin/env bash
# Build a self-contained Claude Agent ACP bundle for one target.
#
# The bundle contains Node, the ACP broker, its npm dependencies, and the
# platform-specific Claude executable shipped by the official Agent SDK.
# Usage: scripts/bundle-claude-agent-acp.sh [--target <triple>] [--out <dir>]

set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$repo_root/packaging/claude-agent-acp/versions.env"
out_root="$repo_root/vendor/claude-agent-acp"
target=""
work=""
cleanup() {
  if [[ -n "$work" && -d "$work" ]]; then
    rm -rf "$work"
  fi
}
trap cleanup EXIT

while [[ $# -gt 0 ]]; do
  case "$1" in
    --target) target="$2"; shift 2 ;;
    --out) out_root="$2"; shift 2 ;;
    -h|--help) sed -n '2,8p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

die() { echo "bundle-claude-agent-acp: $*" >&2; exit 1; }
command -v curl >/dev/null || die "curl is required"
command -v tar >/dev/null || die "tar is required"
command -v npm >/dev/null || die "npm is required"
command -v openssl >/dev/null || die "openssl is required"

[[ -n "$target" ]] || case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) target="aarch64-apple-darwin" ;;
  Darwin-x86_64) target="x86_64-apple-darwin" ;;
  *) die "unsupported host; pass --target explicitly" ;;
esac

case "$target" in
  aarch64-apple-darwin) node_platform="$NODE_PLATFORM_AARCH64_APPLE_DARWIN"; node_sha="$NODE_SHA256_AARCH64_APPLE_DARWIN"; platform_path="$CLAUDE_PLATFORM_ARM64_PATH"; platform_integrity="$CLAUDE_PLATFORM_ARM64_INTEGRITY" ;;
  x86_64-apple-darwin) node_platform="$NODE_PLATFORM_X86_64_APPLE_DARWIN"; node_sha="$NODE_SHA256_X86_64_APPLE_DARWIN"; platform_path="$CLAUDE_PLATFORM_X64_PATH"; platform_integrity="$CLAUDE_PLATFORM_X64_INTEGRITY" ;;
  *) die "only macOS targets are supported currently" ;;
esac

sha256_file() { if command -v sha256sum >/dev/null; then sha256sum "$1" | awk '{print $1}'; else shasum -a 256 "$1" | awk '{print $1}'; fi; }
sha512_b64_file() { openssl dgst -sha512 -binary "$1" | openssl base64 -A; }
download_verified() {
  local url="$1" dest="$2" expected="$3" actual
  curl -fsSL "$url" -o "$dest"
  case "$expected" in sha512-*) actual="sha512-$(sha512_b64_file "$dest")" ;; *) actual="$(sha256_file "$dest")" ;; esac
  [[ "$actual" == "$expected" ]] || die "hash mismatch for $url"
}
npm_url() { local package="$1" version="$2" scope name; scope="${package%%/*}"; name="${package#*/}"; echo "https://registry.npmjs.org/${scope}/${name}/-/${name}-${version}.tgz"; }

target_dir="$out_root/$target"
rm -rf "$target_dir"
mkdir -p "$target_dir/bin" "$target_dir/lib/claude-agent-acp" "$target_dir/licenses"
work="$(mktemp -d "${TMPDIR:-/tmp}/claude-agent-acp.XXXXXX")"

node_archive="node-v${NODE_VERSION}-${node_platform}.tar.gz"
download_verified "https://nodejs.org/dist/v${NODE_VERSION}/${node_archive}" "$work/$node_archive" "$node_sha"
mkdir -p "$work/node"
tar -xzf "$work/$node_archive" -C "$work/node" --strip-components=1 "node-v${NODE_VERSION}-${node_platform}/bin/node" "node-v${NODE_VERSION}-${node_platform}/LICENSE"
cp "$work/node/bin/node" "$target_dir/bin/node"
cp "$work/node/LICENSE" "$target_dir/licenses/LICENSE.node"

acp_archive="$work/claude-acp.tgz"
download_verified "$(npm_url "$CLAUDE_ACP_PATH" "$CLAUDE_ACP_VERSION")" "$acp_archive" "$CLAUDE_ACP_INTEGRITY"
mkdir -p "$work/acp"
tar -xzf "$acp_archive" -C "$work/acp"
cp -R "$work/acp/package/." "$target_dir/lib/claude-agent-acp/"
cp "$work/acp/package/LICENSE" "$target_dir/licenses/LICENSE.claude-agent-acp"

# Install the exact package graph into the bundle. Optional dependencies are
# disabled so the selected platform package below is the only native runtime.
mkdir -p "$work/runtime"
npm install --prefix "$work/runtime" --ignore-scripts --no-audit --no-fund --package-lock=false --omit=dev --omit=optional "$CLAUDE_ACP_PATH@$CLAUDE_ACP_VERSION" "$CLAUDE_SDK_PATH@$CLAUDE_SDK_VERSION" "zod@4.1.12" "@agentclientprotocol/sdk@1.4.0"
rm -rf "$work/runtime/node_modules/$CLAUDE_ACP_PATH" "$work/runtime/node_modules/$CLAUDE_SDK_PATH"
mkdir -p "$work/runtime/node_modules/$CLAUDE_ACP_PATH" "$work/runtime/node_modules/$CLAUDE_SDK_PATH"
cp -R "$target_dir/lib/claude-agent-acp/." "$work/runtime/node_modules/$CLAUDE_ACP_PATH/"
sdk_archive="$work/claude-sdk.tgz"
download_verified "$(npm_url "$CLAUDE_SDK_PATH" "$CLAUDE_SDK_VERSION")" "$sdk_archive" "$CLAUDE_SDK_INTEGRITY"
tar -xzf "$sdk_archive" -C "$work/runtime/node_modules/$CLAUDE_SDK_PATH" --strip-components=1

platform_archive="$work/claude-platform.tgz"
download_verified "$(npm_url "$platform_path" "$CLAUDE_PLATFORM_VERSION")" "$platform_archive" "$platform_integrity"
mkdir -p "$work/runtime/node_modules/$platform_path"
tar -xzf "$platform_archive" -C "$work/runtime/node_modules/$platform_path" --strip-components=1
cp -R "$work/runtime/node_modules" "$target_dir/"
cp "$target_dir/node_modules/$CLAUDE_SDK_PATH/LICENSE" "$target_dir/licenses/LICENSE.claude-agent-sdk" 2>/dev/null || true
cp "$target_dir/node_modules/$platform_path/LICENSE.md" "$target_dir/licenses/LICENSE.claude-agent-sdk-platform" 2>/dev/null || true
rm -rf "$target_dir/lib/claude-agent-acp/node_modules"
chmod +x "$target_dir/bin/node" "$target_dir/node_modules/$platform_path/claude"

cat > "$target_dir/NOTICE" <<EOF
This directory contains the self-contained Claude Agent ACP runtime bundled with
the Slight host. It includes @agentclientprotocol/claude-agent-acp
$CLAUDE_ACP_VERSION, @anthropic-ai/claude-agent-sdk $CLAUDE_SDK_VERSION, the
platform Claude runtime, and Node.js $NODE_VERSION. License texts are in
licenses/ and bundle.json records the pinned inputs and integrity values.
EOF
cat > "$target_dir/bundle.json" <<EOF
{
  "schema": 1,
  "target": "$target",
  "components": {
    "claude_acp": {"package": "$CLAUDE_ACP_PATH", "version": "$CLAUDE_ACP_VERSION", "integrity": "$CLAUDE_ACP_INTEGRITY", "license": "Apache-2.0"},
    "claude_agent_sdk": {"package": "$CLAUDE_SDK_PATH", "version": "$CLAUDE_SDK_VERSION", "integrity": "$CLAUDE_SDK_INTEGRITY"},
    "node": {"version": "$NODE_VERSION", "platform": "$node_platform", "sha256": "$node_sha", "license": "MIT"}
  },
  "layout": {"program": "bin/node", "entry": "lib/claude-agent-acp/dist/index.js", "node_modules": "node_modules/"}
}
EOF
echo "bundle ready: $target_dir" >&2
