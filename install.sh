#!/bin/sh
set -eu

# agentwaste — single binary install (Go + Bubble Tea)
# Usage: curl -sL https://raw.githubusercontent.com/luoyuctl/agentwaste/master/install.sh | sh

REPO="luoyuctl/agentwaste"
BIN="agentwaste"
INSTALL_DIR=""

# — detect platform —
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)
case "$ARCH" in
  x86_64|amd64)  ARCH="amd64" ;;
  aarch64|arm64) ARCH="arm64" ;;
  armv7l)        ARCH="armv7" ;;
  *)             echo "❌ Unsupported architecture: $ARCH"; exit 1 ;;
esac
case "$OS" in
  linux|darwin)  ;;
  *)             echo "❌ Unsupported OS: $OS"; exit 1 ;;
esac

# — resolve install directory —
if [ "$OS" = "darwin" ]; then
  INSTALL_DIR="${HOME}/.local/bin"
elif [ -w /usr/local/bin ]; then
  INSTALL_DIR="/usr/local/bin"
elif [ -w "${HOME}/.local/bin" ] || [ -d "${HOME}/.local/bin" ]; then
  INSTALL_DIR="${HOME}/.local/bin"
else
  INSTALL_DIR="${HOME}/.local/bin"
fi
mkdir -p "$INSTALL_DIR"
DEST="${INSTALL_DIR}/${BIN}"

# — detect latest release —
echo "🔍 Fetching latest release..."
RELEASE_URL=$(curl -sSL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null | grep '"browser_download_url"' | grep "${OS}-${ARCH}" | head -1 | cut -d'"' -f4)

if [ -z "$RELEASE_URL" ]; then
  echo "❌ No binary found for ${OS}/${ARCH}"
  echo "   Build from source: git clone https://github.com/${REPO}.git && cd agentwaste/go && go build -ldflags='-s -w' -o agentwaste ./cmd/agentwaste/"
  exit 1
fi

# — download —
echo "⬇️  Downloading agentwaste (${OS}/${ARCH})..."
TMP=$(mktemp)
curl -sSL -o "$TMP" "$RELEASE_URL"
chmod +x "$TMP"

# — size check —
SIZE=$(wc -c < "$TMP")
echo "   Binary size: ${SIZE} bytes"

# — install —
mv "$TMP" "$DEST"
echo "✅ Installed to ${DEST}"

# — PATH hint —
if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
  echo ""
  echo "⚠️  ${INSTALL_DIR} is not in your PATH."
  echo "   Add this to your shell profile:"
  echo "     export PATH=\"${INSTALL_DIR}:\$PATH\""
  echo ""
fi

# — quick test —
echo ""
echo "🎉 agentwaste installed! Try:"
echo "   agentwaste --latest"
echo "   agentwaste            # launch TUI"
