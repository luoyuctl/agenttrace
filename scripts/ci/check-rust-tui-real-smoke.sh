#!/usr/bin/env bash
set -euo pipefail

bin="${AGENTTRACE_BIN:-target/release/agenttrace}"
source_dir="${AGENTTRACE_TUI_REAL_DIR:-$HOME/.pi/agent/sessions}"
file_limit="${AGENTTRACE_TUI_REAL_FILE_LIMIT:-2}"
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
export bin_abs tmp_sessions tmp_cache timeout_seconds pty_log

expect >"$pty_log" 2>&1 <<'EXPECT'
set timeout $env(timeout_seconds)
log_user 1
spawn -noecho sh -c "stty rows 50 columns 196; exec env TERM=xterm-256color AGENTTRACE_SESSION_CACHE_DIR='$env(tmp_cache)' '$env(bin_abs)' -d '$env(tmp_sessions)'"
expect -re {AgentTrace}
expect -re {Look here first}
expect -re {Why look here}
send "\r"
expect -re {Summary}
send "\033\[C"
expect -re {What happened}
send "\033"
after 400
send "v"
expect -re {Switch view}
send "\033"
after 400
send "f"
expect -re {Filter sessions}
expect -re {Context risk}
send "\033"
after 400
send "\013"
expect -re {Open Look here first}
send "\033"
after 400
send "?"
expect -re {Keys}
send "\033"
after 400
send "q"
expect {
  eof {}
  timeout {
    puts stderr "check-rust-tui-real-smoke: TUI did not exit after scripted q"
    exit 1
  }
}
EXPECT

echo "Rust TUI real-data smoke passed: source_dir=$source_dir sampled_files=$copied source_files=$source_count"
