#!/usr/bin/env bash
set -euo pipefail

REPO="dazarodev/aft-mcp"
BIN_NAME="aft-mcp"
INSTALL_DIR="${HOME}/.claude/bin"

echo "Installing aft-mcp..."

# Detect OS and architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Darwin)
    case "$ARCH" in
      arm64|aarch64) PLATFORM="darwin-arm64" ;;
      x86_64)        PLATFORM="darwin-x64" ;;
      *) echo "Error: unsupported arch $ARCH"; exit 1 ;;
    esac
    ;;
  Linux)
    case "$ARCH" in
      aarch64|arm64) PLATFORM="linux-arm64" ;;
      x86_64)        PLATFORM="linux-x64" ;;
      *) echo "Error: unsupported arch $ARCH"; exit 1 ;;
    esac
    ;;
  MINGW*|MSYS*|CYGWIN*)
    PLATFORM="win32-x64"
    BIN_NAME="aft-mcp.exe"
    ;;
  *) echo "Error: unsupported OS $OS"; exit 1 ;;
esac

ASSET_NAME="aft-mcp-${PLATFORM}"
[ "$PLATFORM" = "win32-x64" ] && ASSET_NAME="aft-mcp-win32-x64.exe"
BINARY_PATH="${INSTALL_DIR}/${BIN_NAME}"

# ---------------------------------------------------------------------------
# Step 1: Download or build
# ---------------------------------------------------------------------------

mkdir -p "$INSTALL_DIR"

# Try GitHub Releases first
TAG=$(curl -sL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null \
  | grep '"tag_name"' | head -1 | sed 's/.*: "\(.*\)".*/\1/' || true)

if [ -n "$TAG" ]; then
  URL="https://github.com/${REPO}/releases/download/${TAG}/${ASSET_NAME}"
  echo "Downloading ${TAG} for ${PLATFORM}..."
  if curl -fsSL "$URL" -o "$BINARY_PATH"; then
    chmod +x "$BINARY_PATH"
    echo "Downloaded to $BINARY_PATH"
  else
    echo "Download failed. Falling back to build from source..."
    TAG=""
  fi
fi

if [ -z "$TAG" ]; then
  if ! command -v cargo >/dev/null 2>&1; then
    echo ""
    echo "Error: no pre-built binary available and cargo is not installed."
    echo "Either:"
    echo "  1. Install Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    echo "  2. Download a binary manually from: https://github.com/${REPO}/releases"
    exit 1
  fi
  echo "Building from source (this takes 1-2 minutes)..."
  cargo install --git "https://github.com/${REPO}.git" --root "${INSTALL_DIR%/bin}" 2>&1 | tail -5
  echo "Built and installed to $BINARY_PATH"
fi

# ---------------------------------------------------------------------------
# Step 2: Verify binary works
# ---------------------------------------------------------------------------

if [ ! -x "$BINARY_PATH" ]; then
  echo "Error: binary not found at $BINARY_PATH"
  exit 1
fi

# Quick smoke test — send initialize and check for response
RESPONSE=$(echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
  | "$BINARY_PATH" 2>/dev/null | head -1 || true)

if echo "$RESPONSE" | grep -q '"protocolVersion"'; then
  echo "Binary verified: MCP server responds correctly."
else
  echo "Warning: binary installed but MCP handshake failed. It may still work — check logs."
fi

# ---------------------------------------------------------------------------
# Step 3: Register MCP server in Claude Code
# ---------------------------------------------------------------------------

if command -v claude >/dev/null 2>&1; then
  claude mcp add -s user aft-mcp -- "$BINARY_PATH" 2>/dev/null \
    && echo "Registered aft MCP server in Claude Code." \
    || echo "Warning: could not register MCP server automatically."
else
  echo ""
  echo "Claude Code CLI not found. Register manually:"
  echo "  claude mcp add -s user aft-mcp -- $BINARY_PATH"
fi

echo ""
echo "Done! Restart Claude Code to use aft tools."
