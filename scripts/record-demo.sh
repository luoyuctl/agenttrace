#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

if ! command -v vhs >/dev/null 2>&1; then
  echo "vhs is required to render docs/demo.tape."
  echo "Install: https://github.com/charmbracelet/vhs"
  exit 1
fi

vhs docs/demo.tape
echo "Wrote assets/agenttrace-demo.gif"
