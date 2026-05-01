#!/bin/sh
set -e

# agenttrace installer — one-liner:
#   curl -sL https://raw.githubusercontent.com/luoyuctl/agenttrace/master/install.sh | sh
#
# Options:
#   VERSION=...     — specific version (default: latest release)
#   DIR=/usr/local/bin — install directory

REPO="luoyuctl/agenttrace"
VERSION="${VERSION:-latest}"
INSTALL_DIR="${DIR:-/usr/local/bin}"
BINARY="agenttrace"

# Detect OS/arch
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$ARCH" in
    x86_64|amd64)  ARCH="amd64" ;;
    aarch64|arm64) ARCH="arm64" ;;
    armv7l)        ARCH="arm" ;;
    *)             echo "❌ unsupported arch: $ARCH"; exit 1 ;;
esac

case "$OS" in
    linux|darwin) ;;
    *)            echo "❌ unsupported os: $OS (linux/macos only)"; exit 1 ;;
esac

TARGET="${OS}-${ARCH}"

# Resolve version
if [ "$VERSION" = "latest" ]; then
    echo "🔍 fetching latest release..."
    VERSION=$(curl -sL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/')
    if [ -z "$VERSION" ]; then
        echo "❌ could not determine latest version"
        exit 1
    fi
fi

DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/agenttrace-${TARGET}"

echo "⬇️  downloading agenttrace ${VERSION} (${TARGET})..."
curl -sL "$DOWNLOAD_URL" -o "/tmp/${BINARY}"

chmod +x "/tmp/${BINARY}"

# Verify
echo "🔍 verifying..."
/tmp/${BINARY} --version 2>/dev/null || /tmp/${BINARY} --list-models >/dev/null 2>&1 || {
    echo "❌ verification failed"
    exit 1
}

# Install
if [ -w "$INSTALL_DIR" ]; then
    mv "/tmp/${BINARY}" "${INSTALL_DIR}/${BINARY}"
else
    echo "🔐 requesting sudo for install to ${INSTALL_DIR}..."
    sudo mv "/tmp/${BINARY}" "${INSTALL_DIR}/${BINARY}"
fi

echo "✅ agenttrace ${VERSION} installed to ${INSTALL_DIR}/${BINARY}"
echo ""
echo "Try it out:"
echo "  agenttrace --list-models"
echo "  agenttrace --latest"
echo "  agenttrace                  # launch TUI"
