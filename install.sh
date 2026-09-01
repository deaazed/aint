#!/bin/sh
# Installs the latest aint release for this platform from GitHub
# Releases - no cargo, no cloning the repo. See
# https://github.com/deaazed/aint for the source, or run
# `cargo build --release` yourself if your platform isn't covered.
#
# Usage: curl -fsSL https://raw.githubusercontent.com/deaazed/aint/main/install.sh | sh

set -e

REPO="deaazed/aint"
INSTALL_DIR="${AINT_INSTALL_DIR:-$HOME/.aint}"
BIN_DIR="$INSTALL_DIR/bin"

detect_os() {
    case "$(uname -s)" in
        Linux) echo "linux" ;;
        Darwin) echo "macos" ;;
        *) echo "unsupported" ;;
    esac
}

detect_arch() {
    case "$(uname -m)" in
        x86_64 | amd64) echo "x86_64" ;;
        arm64 | aarch64) echo "aarch64" ;;
        *) echo "unsupported" ;;
    esac
}

os=$(detect_os)
arch=$(detect_arch)

if [ "$os" = "unsupported" ] || [ "$arch" = "unsupported" ]; then
    echo "error: no prebuilt aint for $(uname -s) $(uname -m)" >&2
    echo "build from source instead: https://github.com/$REPO#building" >&2
    exit 1
fi

# Linux only ships x86_64 for now - fail clearly rather than 404 on
# a download that was never going to exist.
if [ "$os" = "linux" ] && [ "$arch" = "aarch64" ]; then
    echo "error: no prebuilt aint for linux/aarch64 yet" >&2
    echo "build from source instead: https://github.com/$REPO#building" >&2
    exit 1
fi

asset="aint-$os-$arch"
url="https://github.com/$REPO/releases/latest/download/$asset.tar.gz"

echo "downloading $asset.tar.gz..."
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

if ! curl -fsSL "$url" -o "$tmp/$asset.tar.gz"; then
    echo "error: could not download $url" >&2
    echo "see https://github.com/$REPO/releases for what's actually published" >&2
    exit 1
fi

mkdir -p "$BIN_DIR"
tar -xzf "$tmp/$asset.tar.gz" -C "$tmp"
mv "$tmp/aint" "$BIN_DIR/aint"
chmod +x "$BIN_DIR/aint"

echo "installed aint to $BIN_DIR/aint"

case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *)
        echo ""
        echo "$BIN_DIR isn't on your PATH yet - add this to your shell profile:"
        echo "  export PATH=\"$BIN_DIR:\$PATH\""
        ;;
esac

echo ""
echo "verify with: aint --version"
