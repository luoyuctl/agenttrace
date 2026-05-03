#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

if ! command -v vhs >/dev/null 2>&1; then
  echo "vhs is required to render docs/demo.tape."
  echo "Install: https://github.com/charmbracelet/vhs"
  exit 1
fi

if ! command -v ttyd >/dev/null 2>&1; then
  echo "ttyd is required by vhs to render terminal recordings."
  echo "Install: https://github.com/tsl0922/ttyd"
  exit 1
fi

tmpbin="$(mktemp -d)"
cleanup() {
  rm -rf "$tmpbin"
}
trap cleanup EXIT

go build -o "$tmpbin/agenttrace" ./cmd/agenttrace
PATH="$tmpbin:$PATH" vhs docs/demo.tape
echo "Wrote assets/agenttrace-demo.gif"

if command -v ffmpeg >/dev/null 2>&1; then
  if ffmpeg -y -ss 00:00:03 -i assets/agenttrace-demo.gif -frames:v 1 assets/tui-preview.png >/dev/null 2>&1; then
    echo "Wrote assets/tui-preview.png"
  else
    echo "Skipped assets/tui-preview.png refresh; ffmpeg could not extract a frame." >&2
  fi
fi
