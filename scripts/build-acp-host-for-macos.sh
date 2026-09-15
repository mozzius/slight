#!/bin/sh
set -eu

if [ "${PLATFORM_NAME:-}" != "macosx" ]; then
    exit 0
fi

export PATH="${HOME}/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:${PATH}"
for node_bin in "${HOME}"/.nvm/versions/node/*/bin; do
    if [ -d "$node_bin" ]; then
        PATH="$node_bin:$PATH"
    fi
done
export PATH

repo_root="${SRCROOT}/../.."
if ! command -v cargo >/dev/null 2>&1; then
    echo "error: Cargo is required to bundle acp-host but was not found in PATH" >&2
    exit 1
fi

cargo build --manifest-path "${repo_root}/Cargo.toml" --release --package host-cli

destination="${TARGET_BUILD_DIR}/${UNLOCALIZED_RESOURCES_FOLDER_PATH}/acp-host"
mkdir -p "$(dirname "${destination}")"
cp "${repo_root}/target/release/acp-host" "${destination}"
chmod 755 "${destination}"

resources_root="$(dirname "${destination}")"
target_triple=""
case "$(uname -s)-$(uname -m)" in
    Darwin-arm64) target_triple="aarch64-apple-darwin" ;;
    Darwin-x86_64) target_triple="x86_64-apple-darwin" ;;
    *) echo "error: unsupported macOS build host for ACP bundles" >&2; exit 1 ;;
esac

if [ ! -x "${resources_root}/codex-acp/${target_triple}/bin/node" ] || \
   [ ! -x "${resources_root}/claude-agent-acp/${target_triple}/bin/node" ]; then
    "${repo_root}/scripts/package-acp-bundles.sh" \
        --target "${target_triple}" \
        --out "${resources_root}"
fi
