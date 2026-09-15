#!/usr/bin/env bash
# Build every bundled ACP runtime required by a shipped Slight host.
#
# Usage:
#   scripts/package-acp-bundles.sh [--target <triple>]... [--out <dir>]
#
# Output layout:
#   <out>/codex-acp/<triple>/
#   <out>/claude-agent-acp/<triple>/

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out_root="$repo_root/vendor"
targets=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --target) targets+=("$2"); shift 2 ;;
    --out) out_root="$2"; shift 2 ;;
    -h|--help) sed -n '2,11p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

if [[ ${#targets[@]} -eq 0 ]]; then
  case "$(uname -s)-$(uname -m)" in
    Darwin-arm64) targets+=("aarch64-apple-darwin") ;;
    Darwin-x86_64) targets+=("x86_64-apple-darwin") ;;
    *) echo "package-acp-bundles: unsupported host; pass --target explicitly" >&2; exit 1 ;;
  esac
fi

codex_args=("$repo_root/scripts/bundle-codex-acp.sh" "--out" "$out_root/codex-acp")
for target in "${targets[@]}"; do
  codex_args+=("--target" "$target")
done

"${codex_args[@]}"
for target in "${targets[@]}"; do
  "$repo_root/scripts/bundle-claude-agent-acp.sh" \
    --target "$target" \
    --out "$out_root/claude-agent-acp"
done

for target in "${targets[@]}"; do
  test -x "$out_root/codex-acp/$target/bin/node"
  test -f "$out_root/codex-acp/$target/lib/codex-acp/index.js"
  test -x "$out_root/claude-agent-acp/$target/bin/node"
  test -f "$out_root/claude-agent-acp/$target/lib/claude-agent-acp/dist/index.js"
  test -f "$out_root/codex-acp/$target/bundle.json"
  test -f "$out_root/claude-agent-acp/$target/bundle.json"
done

echo "ACP bundles ready under $out_root" >&2
