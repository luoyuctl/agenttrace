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

vhs docs/demo.tape
echo "Wrote assets/agenttrace-demo.gif"
