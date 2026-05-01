#!/usr/bin/env bash
set -euo pipefail

# agenttrace install script — auto-detect OS/arch, download latest binary
# Usage: curl -sL https://raw.githubusercontent.com/luoyuctl/agenttrace/master/install.sh | sh

REPO="luoyuctl/agenttrace"
BINARY="agenttrace"
VERSION="${AGENTRACE_VERSION:-latest}"
INSTALL_DIR="${AGENTRACE_INSTALL_DIR:-/usr/local/bin}"

# --- detect OS/Arch ---
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$ARCH" in
    x86_64|amd64)  ARCH="amd64" ;;
    aarch64|arm64) ARCH="arm64" ;;
    *)             echo "❌ Unsupported architecture: $ARCH"; exit 1 ;;
esac

case "$OS" in
    linux|darwin)  EXT="" ;;
    mingw*|msys*|cygwin*) OS="windows"; EXT=".exe" ;;
    *)             echo "❌ Unsupported OS: $OS"; exit 1 ;;
esac

# --- determine download URL ---
if [ "$VERSION" = "latest" ]; then
    URL="https://github.com/$REPO/releases/latest/download/${BINARY}-${OS}-${ARCH}${EXT:-}"
else
    URL="https://github.com/$REPO/releases/download/${VERSION}/${BINARY}-${OS}-${ARCH}${EXT:-}"
fi

echo "📦 agenttrace install"
echo "   OS/Arch: $OS/$ARCH"
echo "   Target:  $INSTALL_DIR"
echo "   URL:     $URL"
echo ""

# --- download ---
TMPFILE=$(mktemp)
trap 'rm -f "$TMPFILE"' EXIT

echo "⬇️  Downloading..."
if command -v curl > /dev/null 2>&1; then
    curl -fsSL "$URL" -o "$TMPFILE" || {
        echo "❌ Download failed. Check if version exists:"
        echo "   $URL"
        exit 1
    }
elif command -v wget > /dev/null 2>&1; then
    wget -q "$URL" -O "$TMPFILE" || {
        echo "❌ Download failed."
        exit 1
    }
else
    echo "❌ Neither curl nor wget found. Install one first."
    exit 1
fi

# --- verify ---
if [ ! -s "$TMPFILE" ]; then
    echo "❌ Downloaded file is empty"
    exit 1
fi

FILE_SIZE=$(stat -c%s "$TMPFILE" 2>/dev/null || stat -f%z "$TMPFILE" 2>/dev/null || echo 0)
echo "   Size: $(numfmt --to=iec $FILE_SIZE 2>/dev/null || echo "${FILE_SIZE} bytes")"

# --- install ---
if [ -w "$INSTALL_DIR" ]; then
    DEST="$INSTALL_DIR/$BINARY$EXT"
else
    DEST="$INSTALL_DIR/$BINARY$EXT"
    SUDO="sudo"
    echo "   (using sudo to write to $INSTALL_DIR)"
fi

${SUDO:-} mkdir -p "$INSTALL_DIR"
${SUDO:-} cp "$TMPFILE" "$DEST"
${SUDO:-} chmod +x "$DEST"

echo ""
echo "✅ agenttrace installed to $DEST"
echo ""
echo "🚀 Try it out:"
echo "   agenttrace --list-models"
echo "   agenttrace --latest"
echo "   agenttrace            # launch TUI"
