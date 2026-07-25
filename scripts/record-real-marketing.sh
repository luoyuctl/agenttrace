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
snapshot=".tmp/redacted-sessions"
cleanup() {
  rm -rf "$tmpdir"
  rm -rf "$snapshot"
}
trap cleanup EXIT

cargo build --release -p agenttrace
cp target/release/agenttrace "$tmpdir/agenttrace-bin"
export PATH="$tmpdir:$PATH"

rm -rf "$snapshot"
mkdir -p "$snapshot"
"$tmpdir/agenttrace-bin" --sessions -f json --limit 500 >"$tmpdir/sessions.json"
node - "$tmpdir/sessions.json" "$snapshot" <<'NODE'
const fs = require("fs");
const path = require("path");
const [input, output] = process.argv.slice(2);
const sessions = JSON.parse(fs.readFileSync(input, "utf8"));
for (const [index, session] of sessions.entries()) {
  const metrics = session.metrics;
  const source = metrics.source_tool || "generic";
  const events = [
    { role: "meta", ModelUsed: metrics.model_used, SourceTool: source, Usage: {
      input_tokens: metrics.tokens_input,
      output_tokens: metrics.tokens_output,
      cache_creation_input_tokens: metrics.tokens_cache_w,
      cache_read_input_tokens: metrics.tokens_cache_r,
    }},
    { role: "session_meta", cwd: `/workspace/project-${index % 8 + 1}`, SourceTool: source },
    { role: "user", content: `Local session ${index + 1}`, timestamp: metrics.session_start, SourceTool: source },
  ];
  const calls = Math.min(metrics.tool_calls_total || 0, 100);
  const failures = Math.min(metrics.tool_calls_fail || 0, calls);
  for (let call = 0; call < calls; call++) {
    const id = `call-${index}-${call}`;
    events.push({ role: "assistant", timestamp: metrics.session_start, SourceTool: source,
      tool_calls: [{ id, name: call < failures ? "Shell" : "Read", args: {} }] });
    events.push({ role: "tool", timestamp: call + 1 === calls ? metrics.session_end : metrics.session_start,
      SourceTool: source, tool_call_id: id, is_error: call < failures });
  }
  if (!calls) events.push({ role: "assistant", content: "Completed.", timestamp: metrics.session_end, SourceTool: source });
  fs.writeFileSync(path.join(output, `session-${String(index + 1).padStart(4, "0")}.json`), JSON.stringify(events));
}
NODE

sessions="$(find "$snapshot" -type f | wc -l | tr -d ' ')"
if [[ "$sessions" -le 0 ]]; then
  echo "No real local sessions found. Refusing to record demo/test marketing assets." >&2
  exit 1
fi

if grep -R -Eqi '/Users/|/home/|@|api[_-]?key|token["=: ]' "$snapshot"; then
  echo "Redaction check failed for generated marketing snapshot." >&2
  exit 1
fi

cat >"$tmpdir/agenttrace" <<EOF
#!/bin/sh
exec "$tmpdir/agenttrace-bin" -d "$snapshot" "\$@"
EOF
chmod +x "$tmpdir/agenttrace"

record_env=(env -u NO_COLOR TERM=xterm-256color COLORTERM=truecolor CLICOLOR_FORCE=1 FORCE_COLOR=1)
"${record_env[@]}" vhs docs/demos/real-run.tape

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
