#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "$1 is required to refresh real marketing assets." >&2
    exit 1
  fi
}

require_cmd vhs
require_cmd ttyd
require_cmd ffmpeg
require_cmd node

tmpdir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmpdir"
}
trap cleanup EXIT

cargo build --release -p agenttrace
cp target/release/agenttrace "$tmpdir/agenttrace"
export PATH="$tmpdir:$PATH"

sessions="$(
  agenttrace --doctor -f json |
    node -e 'const fs=require("fs"); const d=JSON.parse(fs.readFileSync(0,"utf8")); process.stdout.write(String(d.sessions || 0));'
)"
if [[ "$sessions" -le 0 ]]; then
  echo "No real local sessions found. Refusing to record demo/test marketing assets." >&2
  exit 1
fi

agenttrace --overview -f html -o site/demo-report.html >/dev/null

record_env=(env -u NO_COLOR TERM=xterm-256color COLORTERM=truecolor CLICOLOR_FORCE=1 FORCE_COLOR=1)
"${record_env[@]}" vhs docs/real-run.tape

record_png() {
  local output="$1"
  local keys="$2"
  local tape="$tmpdir/capture.tape"
  local gif="$tmpdir/capture.gif"

  cat >"$tape" <<EOF
Output "$gif"
Set Shell "bash"
Set FontSize 18
Set Width 1800
Set Height 1100
Set Padding 16
Set Framerate 12
Set Theme "Dracula"
Env TERM "xterm-256color"
Env COLORTERM "truecolor"
Env FORCE_COLOR "1"
Env CLICOLOR_FORCE "1"
Env NO_COLOR ""

Type "agenttrace"
Enter
Sleep 4500ms
$keys
Sleep 900ms
EOF

  "${record_env[@]}" vhs "$tape"
  ffmpeg -y -sseof -0.2 -i "$gif" -frames:v 1 "$output" >/dev/null 2>&1
}

record_png assets/readme-real-overview.png ""
record_png assets/readme-real-critical.png "Type \"!\""
record_png assets/readme-real-detail.png $'Type "!"\nSleep 900ms\nEnter'
record_png assets/readme-real-diagnostics.png $'Type "!"\nSleep 900ms\nEnter\nSleep 900ms\nType "w"'

echo "Refreshed real marketing assets from $sessions local sessions."
