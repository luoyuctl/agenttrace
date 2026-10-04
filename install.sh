#!/bin/sh
set -eu

# agenttrace — single binary install (Rust + ratatui)
# Usage: curl -fsSL https://raw.githubusercontent.com/luoyuctl/agenttrace/master/install.sh | sh
#
# Environment:
#   AGENTTRACE_VERSION      release tag to install (default: latest), e.g. v0.9.1
#   AGENTTRACE_INSTALL_DIR  install directory (default: ~/.local/bin, or /usr/local/bin on
#                           Linux when writable)
#   AGENTTRACE_NO_MODIFY_PATH=1  do not add the install directory to your shell profile

REPO="luoyuctl/agenttrace"
BIN="agenttrace"
VERSION="${AGENTTRACE_VERSION:-latest}"
INSTALL_DIR="${AGENTTRACE_INSTALL_DIR:-}"

# — detect platform —
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)
case "$ARCH" in
  x86_64|amd64)  ARCH="amd64" ;;
  aarch64|arm64) ARCH="arm64" ;;
  *)             echo "❌ Unsupported architecture: $ARCH"; exit 1 ;;
esac
case "$OS" in
  linux|darwin)  ;;
  *)             echo "❌ Unsupported OS: $OS"; exit 1 ;;
esac

# — resolve install directory —
if [ -z "$INSTALL_DIR" ]; then
  if [ "$OS" = "linux" ] && [ -w /usr/local/bin ]; then
    INSTALL_DIR="/usr/local/bin"
  else
    INSTALL_DIR="${HOME}/.local/bin"
  fi
fi
mkdir -p "$INSTALL_DIR"
DEST="${INSTALL_DIR}/${BIN}"

# — resolve release asset —
ASSET="${BIN}-${OS}-${ARCH}"
if [ "$VERSION" = "latest" ]; then
  BASE_URL="https://github.com/${REPO}/releases/latest/download"
else
  BASE_URL="https://github.com/${REPO}/releases/download/${VERSION}"
fi

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    echo "❌ Need sha256sum or shasum to verify the download." >&2
    exit 1
  fi
}

TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT INT TERM

# — download —
echo "⬇️  Downloading agenttrace ${VERSION} (${OS}/${ARCH})..."
if ! curl -fsSL --retry 3 -o "$TMP_DIR/$ASSET" "$BASE_URL/$ASSET"; then
  echo "❌ No binary found for ${OS}/${ARCH}"
  echo "   Build from source: git clone https://github.com/${REPO}.git && cd agenttrace && cargo build --release -p agenttrace"
  exit 1
fi
if ! curl -fsSL --retry 3 -o "$TMP_DIR/$ASSET.sha256" "$BASE_URL/$ASSET.sha256"; then
  echo "❌ Could not download the checksum for ${ASSET}; refusing to install an unverified binary."
  exit 1
fi

# — verify checksum —
EXPECTED=$(awk '{print tolower($1); exit}' "$TMP_DIR/$ASSET.sha256")
ACTUAL=$(sha256_of "$TMP_DIR/$ASSET")
if [ -z "$EXPECTED" ] || [ "$EXPECTED" != "$ACTUAL" ]; then
  echo "❌ Checksum mismatch for ${ASSET}"
  echo "   expected: ${EXPECTED:-<missing>}"
  echo "   actual:   ${ACTUAL}"
  exit 1
fi
echo "🔒 SHA-256 verified"

# — install —
chmod +x "$TMP_DIR/$ASSET"
mv "$TMP_DIR/$ASSET" "$DEST"
echo "✅ Installed to ${DEST}"

# — PATH —
case ":${PATH}:" in
  *":${INSTALL_DIR}:"*) ;;
  *)
    LINE="export PATH=\"${INSTALL_DIR}:\$PATH\""
    PROFILE=""
    case "${SHELL:-}" in
      */zsh)  PROFILE="${ZDOTDIR:-$HOME}/.zshrc" ;;
      */bash) if [ "$OS" = "darwin" ]; then PROFILE="$HOME/.bash_profile"; else PROFILE="$HOME/.bashrc"; fi ;;
      */fish) LINE="fish_add_path ${INSTALL_DIR}"; PROFILE="$HOME/.config/fish/config.fish" ;;
      *)      PROFILE="$HOME/.profile" ;;
    esac
    echo ""
    if [ "${AGENTTRACE_NO_MODIFY_PATH:-0}" != "1" ] && { [ ! -e "$PROFILE" ] || [ -w "$PROFILE" ]; }; then
      mkdir -p "$(dirname "$PROFILE")"
      if ! grep -qsF "$LINE" "$PROFILE"; then
        printf '\n# agenttrace\n%s\n' "$LINE" >>"$PROFILE"
      fi
      echo "➕ Added ${INSTALL_DIR} to PATH in ${PROFILE}"
      echo "   Open a new terminal, or run: ${LINE}"
    else
      echo "⚠️  ${INSTALL_DIR} is not in your PATH. Add this to your shell profile:"
      echo "     ${LINE}"
    fi
    ;;
esac

echo ""
echo "🎉 agenttrace installed! Try:"
echo "   agenttrace --latest"
echo "   agenttrace            # launch TUI"
echo "   agenttrace update     # upgrade later"
