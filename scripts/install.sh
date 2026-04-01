#!/usr/bin/env bash
set -euo pipefail

REPO="dazarodev/aft-mcp"
BIN_NAME="aft-mcp"
INSTALL_DIR="${1:-$(cd "$(dirname "$0")/.." && pwd)/bin}"

# Detect OS and architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Darwin)
    case "$ARCH" in
      arm64|aarch64) PLATFORM="darwin-arm64" ;;
      x86_64)        PLATFORM="darwin-x64" ;;
      *) echo "Unsupported arch: $ARCH"; exit 1 ;;
    esac
    ;;
  Linux)
    case "$ARCH" in
      aarch64|arm64) PLATFORM="linux-arm64" ;;
      x86_64)        PLATFORM="linux-x64" ;;
      *) echo "Unsupported arch: $ARCH"; exit 1 ;;
    esac
    ;;
  MINGW*|MSYS*|CYGWIN*)
    PLATFORM="win32-x64"
    BIN_NAME="aft-mcp.exe"
    ;;
  *) echo "Unsupported OS: $OS"; exit 1 ;;
esac

ASSET_NAME="aft-mcp-${PLATFORM}"
[ "$PLATFORM" = "win32-x64" ] && ASSET_NAME="aft-mcp-win32-x64.exe"

# Get latest release tag
TAG=$(curl -sL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | head -1 | sed 's/.*: "\(.*\)".*/\1/')

if [ -z "$TAG" ]; then
  echo "Could not determine latest release. Falling back to cargo build..."
  if command -v cargo >/dev/null 2>&1; then
    echo "Building from source..."
    cd "$(dirname "$0")/.."
    cargo build --release
    mkdir -p "$INSTALL_DIR"
    cp "target/release/$BIN_NAME" "$INSTALL_DIR/$BIN_NAME"
    chmod +x "$INSTALL_DIR/$BIN_NAME"
    echo "Installed $BIN_NAME to $INSTALL_DIR (built from source)"
    exit 0
  else
    echo "No release found and cargo not available. Install Rust: https://rustup.rs"
    exit 1
  fi
fi

URL="https://github.com/${REPO}/releases/download/${TAG}/${ASSET_NAME}"

echo "Downloading ${BIN_NAME} ${TAG} for ${PLATFORM}..."
mkdir -p "$INSTALL_DIR"
curl -fsSL "$URL" -o "$INSTALL_DIR/$BIN_NAME"
chmod +x "$INSTALL_DIR/$BIN_NAME"

echo "Installed $BIN_NAME ${TAG} to $INSTALL_DIR"
