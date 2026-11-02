#!/usr/bin/env sh
# Installs the latest druk release binary for Linux, verifying its checksum
# against the SHA256SUMS.txt published alongside it. No local LLVM or Rust
# toolchain is needed: this only downloads the prebuilt JIT-capable binary
# (see README.md's "Installing a prebuilt binary" section for what druk
# build additionally needs).
#
# Usage: curl -fsSL https://raw.githubusercontent.com/Etoile-Bleu/druk-lang-v2/main/install.sh | sh
set -eu

REPO="Etoile-Bleu/druk-lang-v2"
INSTALL_DIR="${DRUK_INSTALL_DIR:-$HOME/.local/bin}"

os=$(uname -s)
arch=$(uname -m)

case "$os" in
    Linux) ;;
    Darwin)
        echo "Error: there is no macOS build of druk yet (only Linux and Windows)." >&2
        echo "Track progress or build from source: https://github.com/$REPO#building" >&2
        exit 1
        ;;
    *)
        echo "Error: unsupported OS '$os'. druk currently ships Linux and Windows binaries only." >&2
        exit 1
        ;;
esac

case "$arch" in
    x86_64 | amd64) ;;
    *)
        echo "Error: unsupported CPU architecture '$arch'. druk currently ships x86_64 binaries only." >&2
        exit 1
        ;;
esac

for tool in curl sha256sum; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "Error: this script needs '$tool' on PATH, and it wasn't found." >&2
        exit 1
    fi
done

asset="druk-linux-x86_64"
base_url="https://github.com/$REPO/releases/latest/download"

tmp_dir=$(mktemp -d)
trap 'rm -rf "$tmp_dir"' EXIT

echo "Downloading $asset..."
curl -fsSL -o "$tmp_dir/$asset" "$base_url/$asset"
curl -fsSL -o "$tmp_dir/SHA256SUMS.txt" "$base_url/SHA256SUMS.txt"

echo "Verifying checksum..."
if ! grep -q " $asset\$" "$tmp_dir/SHA256SUMS.txt"; then
    echo "Error: $asset is not listed in SHA256SUMS.txt; refusing to install an unverified binary." >&2
    exit 1
fi
if ! (cd "$tmp_dir" && grep " $asset\$" SHA256SUMS.txt | sha256sum -c - >/dev/null); then
    echo "Error: checksum verification failed for $asset; refusing to install it." >&2
    exit 1
fi

mkdir -p "$INSTALL_DIR"
mv "$tmp_dir/$asset" "$INSTALL_DIR/druk"
chmod +x "$INSTALL_DIR/druk"

echo "druk installed to $INSTALL_DIR/druk"

case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *)
        echo ""
        echo "$INSTALL_DIR is not on your PATH yet. Add this to your shell profile:"
        echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
        ;;
esac

echo ""
echo "Run 'druk --version' to confirm the install."
