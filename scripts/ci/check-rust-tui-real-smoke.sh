#!/usr/bin/env bash
set -euo pipefail

bin="${AGENTTRACE_BIN:-target/release/agenttrace}"
source_dir="${AGENTTRACE_TUI_REAL_DIR:-$HOME/.pi/agent/sessions}"
query="${AGENTTRACE_TUI_REAL_QUERY:-pi}"
file_limit="${AGENTTRACE_TUI_REAL_FILE_LIMIT:-8}"
timeout_seconds="${AGENTTRACE_TUI_TIMEOUT:-30}"

fail() {
  echo "check-rust-tui-real-smoke: $*" >&2
  exit 1
}

[[ -x "$bin" ]] || fail "agenttrace binary is not executable: $bin"
[[ -d "$source_dir" ]] || fail "real sessions directory does not exist: $source_dir"
command -v expect >/dev/null 2>&1 || fail "expect is required for real TUI smoke"

source_count="$(find "$source_dir" -type f 2>/dev/null | wc -l | tr -d ' ')"
[[ "$source_count" -gt 0 ]] || fail "real sessions directory has no files: $source_dir"

tmp_cache="$(mktemp -d "${TMPDIR:-/tmp}/agenttrace-tui-real-cache.XXXXXX")"
tmp_sessions="$(mktemp -d "${TMPDIR:-/tmp}/agenttrace-tui-real-sessions.XXXXXX")"
pty_log="$(mktemp "${TMPDIR:-/tmp}/agenttrace-tui-real-pty.XXXXXX")"
cleanup() {
  rm -rf "$tmp_cache" "$tmp_sessions" "$pty_log"
}
trap cleanup EXIT

copied=0
while IFS= read -r file; do
  cp "$file" "$tmp_sessions/session-$copied-${file##*/}"
  copied=$((copied + 1))
  [[ "$copied" -ge "$file_limit" ]] && break
done < <(find "$source_dir" -type f 2>/dev/null | sort)
[[ "$copied" -gt 0 ]] || fail "could not copy real session files from: $source_dir"

bin_abs="$(cd "$(dirname "$bin")" && pwd)/$(basename "$bin")"
export bin_abs tmp_sessions tmp_cache query timeout_seconds pty_log

expect >"$pty_log" 2>&1 <<'EXPECT'
set timeout $env(timeout_seconds)
log_user 1
spawn -noecho sh -c "stty rows 44 columns 120; exec env TERM=xterm-256color AGENTTRACE_SESSION_CACHE_DIR='$env(tmp_cache)' '$env(bin_abs)' -d '$env(tmp_sessions)'"
after 1500
send "0"
after 500
send "1"
after 500
send "\r"
after 500
send "3"
after 500
send "1"
after 500
send "4"
after 500
send "1"
after 500
send "/"
after 200
send -- "$env(query)\r"
after 500
send "c"
after 500
send ":reload\r"
after 1200
send "\022"
after 1200
send "?"
after 500
send ":health >=0\r"
after 500
send ":source pi\r"
after 500
send ":model default\r"
after 500
send ":cost >=0\r"
after 500
send ":anomaly\r"
after 500
send ":clear\r"
after 500
send ":top cost\r"
after 500
send ":sort health asc\r"
after 500
send ":sort source asc\r"
after 500
send ":$env(query)\r"
after 500
send ":reset\r"
after 500
send ":model definitely-no-match\r"
after 500
send ":clear\r"
after 500
send "f"
after 500
send "\033"
after 500
send "s"
after 500
send "\033"
send "0"
after 500
send "$"
after 500
send "!"
after 500
send "q"
expect {
  eof {}
  timeout {
    puts stderr "check-rust-tui-real-smoke: TUI did not exit after scripted q"
    exit 1
  }
}
EXPECT

python3 - "$pty_log" "$copied" "$query" <<'PY'
import re
import sys
from pathlib import Path

raw = Path(sys.argv[1]).read_bytes().decode("utf-8", "ignore")
copied = int(sys.argv[2])
query = sys.argv[3]

def visible_text(stream: str) -> str:
    stream = re.sub(r"\x1b\[[0-9;?]*[ -/]*[@-~]", " ", stream)
    stream = re.sub(r"\x1b\][^\x07]*(?:\x07|\x1b\\)", " ", stream)
    stream = stream.replace("\r", "\n")
    stream = re.sub(r"[^\x20-\x7e\n\t]", " ", stream)
    stream = re.sub(r"[ \t]+", " ", stream)
    return stream

text = visible_text(raw)

checks = [
    ("startup header", r"AGENTTRACE v[0-9]+\.[0-9]+\.[0-9]+"),
    ("footer keymap", r"q\s*quit[\s\S]{0,140}\?\s*help|!\s*critical[\s\S]{0,140}\?\s*help"),
    ("footer help key", r"\?\s*help"),
    ("overview render", r"Scoreboard"),
    ("loading status panel", r"Loading Status|phase=\s*(idle|discovering|parsing|ready)"),
    ("loading mode", r"mode=normal load|mode=force reload|normal load|force reload"),
    ("loading parsed total", r"parsed=\s*[0-9,]+/[0-9,]+|loaded\s+[0-9,]+/[0-9,]+\s+files"),
    ("loading progress bar", r"[0-9,]+/[0-9,]+\s+files[\s\S]{0,240}[0-9]{1,3}%"),
    ("loading cache state", r"cache hits=\s*[0-9,]+\s+(cache warm|cache empty|cache bypass)|[0-9,]+\s+cache hits,\s+(cache warm|cache empty|cache bypass)"),
    ("loading source distribution", r"sources=(none|[A-Za-z0-9_ ./:,-]+:[0-9]+)|[A-Za-z0-9_ ./:,-]+:[0-9]+"),
    ("overview health score", r"health\s*[0-9]+(?:\.[0-9]+)?"),
    ("overview next action", r"next:\s*[a-z ]+|next=\s*[a-z ]+"),
    ("overview key metrics", r"cost\s*\$|tokens\s*[0-9,]+|elapsed\s*[0-9]+|p95\s*[0-9]+"),
    ("overview driver summary", r"Driver Summary[\s\S]{0,240}Source\s+[A-Za-z0-9_ ./:,-]+"),
    ("overview inspect first", r"Inspect\s*First[\s\S]{0,700}(action:|no priority sessions)"),
    ("overview recent sessions", r"Recent\s*Sessions[\s\S]{0,500}(health|cost|\$|[0-9]{1,3})"),
    ("real sessions loaded", rf"sessions\s*{copied}|Sessions -\s*{copied}\s+visible"),
    ("list view", r"Sessions -\s*[1-9][0-9]*\s+visible"),
    ("list status panel", r"List Status|visible=\s*[0-9,]+/[0-9,]+"),
    ("list status filter sort", r"filters\s+(none|[a-z]+=)|sort\s+(Recent|Cost|Health|Source|Anomalies|Failures|Name|Turns)\s+(asc|desc)"),
    ("list status hint", r"Enter\s+detail|Esc/:clear resets filters"),
    ("list driver summary", r"Driver Summary[\s\S]{0,80}Visible:\s+[0-9,]+\s+sessions"),
    ("list source driver", r"Source\s+[A-Za-z0-9_ ./:,-]+"),
    ("list model driver", r"Model\s+[a-z0-9_./:-]+"),
    ("list anomaly driver", r"Anomaly\s+(none|[a-z0-9_./:-]+)"),
    ("selected triage panel", r"Selected Triage|selected:\s*session"),
    ("selected triage reason", r"reason=[a-z0-9_ .:-]+"),
    ("selected triage metrics", r"ok=\d+%|fail=\d+|anom=\d+|health=\d+"),
    ("selected triage action", r"action=(open detail|open diagnostics|inspect|compare)|Selected Triage"),
    ("detail view", r"Detail -|Detail"),
    ("detail context signal", r"health=\d+|cost=\$|fail=\d+|anom=\d+"),
    ("detail triage reason", r"reason=[a-z0-9_ .:-]+"),
    ("diagnostics view", r"Diagnostics -|Diagnostics|Waste Analysis"),
    ("diagnostics context signal", r"source=[A-Za-z0-9_ ./:-]+|source [A-Za-z0-9_ ./:-]+"),
    ("diff context signal", r"4\s+Diff[\s\S]{0,360}C\s*o\s*n\s*t\s*(?:e\s*)?x\s*t:[\s\S]{0,360}(top\s+source|主要来源)\s*="),
    ("filter path", rf"filter:\s*{re.escape(query)}"),
    ("sort path", r"sorted by Cost|cost"),
    ("reload path", r"reloading sessions|sessions=[1-9][0-9]*|Sessions -\s*[1-9][0-9]*\s+visible"),
    ("help view", r"Help|Triage workflow|Command mode"),
    ("health command filter", r"filter health:\s*>=0|health=>=0"),
    ("source command filter", r"filter source:\s*pi|source=pi"),
    ("model command filter", r"filter model:\s*default|model=default"),
    ("cost command filter", r"filter cost:\s*>=0|cost>=0"),
    ("anomaly command filter", r"filter anomalies|anomaly=any"),
    ("command clear", r"filter cleared|filt\s*r\s*cleared"),
    ("top command sort", r"sorted by Cost desc"),
    ("explicit sort direction", r"sorted by Health asc|sort Health asc"),
    ("source sort command", r"sorted by Source asc|sort Source asc"),
    ("bare command text filter", rf"filter:\s*{re.escape(query)}"),
    ("empty filter state", r"No visible[\s\S]{0,90}match the active filt|Active filters:[\s\S]{0,120}definitel[\s\S]{0,40}no-match"),
    ("quick health filter key", r"quick health filter:\s*good|health=good"),
    ("quick source filter key", r"quick source filter:\s*[a-z0-9_/-]+|source=[a-z0-9_/-]+"),
    ("quick cost filter key", r"quick\s*cost\s*filter:\s*>\s*0|co\s*t\s*>\s*0|cost\s*>\s*0"),
    ("quick critical filter key", r"quick\s*critical\s*filter|ri\s*ica\s*filter|health:\s*critical|health\s*=\s*crit|Active filters:[\s\S]{0,120}health=crit"),
]

missing = [name for name, pattern in checks if not re.search(pattern, text, re.I)]
if missing:
    raise SystemExit("missing TUI evidence: " + ", ".join(missing))
PY

echo "Rust TUI real-data smoke passed: source_dir=$source_dir sampled_files=$copied source_files=$source_count query=$query"
