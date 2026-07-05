#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

bin="${AGENTTRACE_BIN:-$repo_root/target/release/agenttrace}"
out_dir="${AGENTTRACE_REAL_CLI_OUT:-/tmp/agenttrace-real-cli-smoke-$(date +%Y%m%d%H%M%S)}"
source_dir="${AGENTTRACE_REAL_CLI_DIR:-$HOME/.pi/agent/sessions}"
query="${AGENTTRACE_REAL_CLI_QUERY:-error}"
file_limit="${AGENTTRACE_REAL_CLI_FILE_LIMIT:-8}"

fail() {
  echo "check-rust-real-cli-smoke: $*" >&2
  exit 1
}

run_capture() {
  local name="$1"
  shift
  local status=0
  mkdir -p "$out_dir/cache-$name"
  set +e
  env AGENTTRACE_SESSION_CACHE_DIR="$out_dir/cache-$name" \
    "$bin" "$@" >"$out_dir/$name.out" 2>"$out_dir/$name.err"
  status=$?
  set -e
  printf "%s\n" "$status" >"$out_dir/$name.status"
}

expect_status() {
  local name="$1"
  local expected="$2"
  local actual
  actual="$(cat "$out_dir/$name.status")"
  [[ "$actual" == "$expected" ]] || fail "$name exit status $actual, expected $expected"
}

require_json() {
  local path="$1"
  node -e 'JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"))' "$path" \
    || fail "invalid JSON: $path"
}

[[ -x "$bin" ]] || fail "agenttrace binary is not executable: $bin"
[[ -d "$source_dir" ]] || fail "real sessions directory does not exist: $source_dir"

rm -rf "$out_dir"
mkdir -p "$out_dir"

snapshot_dir="$out_dir/real-session-snapshot"
mkdir -p "$snapshot_dir"
copied=0
while IFS= read -r file; do
  cp "$file" "$snapshot_dir/session-$copied-${file##*/}"
  copied=$((copied + 1))
  [[ "$copied" -ge "$file_limit" ]] && break
done < <(find "$source_dir" -type f 2>/dev/null | sort)
[[ "$copied" -gt 0 ]] || fail "could not copy real session files from: $source_dir"

run_capture doctor-default --doctor -f json
run_capture overview-default --overview -f json
run_capture doctor-snapshot --doctor -d "$snapshot_dir" -f json
run_capture overview-snapshot -d "$snapshot_dir" --overview -f json
run_capture latest-snapshot -d "$snapshot_dir" --latest -f json
run_capture search-snapshot -d "$snapshot_dir" --search "$query" -f json
run_capture compare-snapshot -d "$snapshot_dir" --compare -f json
run_capture waste-snapshot -d "$snapshot_dir" --waste

for name in \
  doctor-default overview-default doctor-snapshot overview-snapshot \
  latest-snapshot search-snapshot compare-snapshot waste-snapshot
do
  expect_status "$name" 0
done

for name in doctor-default overview-default doctor-snapshot overview-snapshot latest-snapshot search-snapshot compare-snapshot; do
  require_json "$out_dir/$name.out"
done

grep -q "Waste Analysis" "$out_dir/waste-snapshot.out" \
  || fail "waste report should include Waste Analysis"

node - "$out_dir" "$copied" <<'NODE'
const fs = require("fs");
const path = require("path");
const outDir = process.argv[2];
const copied = Number(process.argv[3]);

function readJson(name) {
  return JSON.parse(fs.readFileSync(path.join(outDir, `${name}.out`), "utf8"));
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

const doctor = readJson("doctor-snapshot");
assert(doctor.mode === "custom directory", `snapshot doctor mode changed: ${doctor.mode}`);
assert(doctor.session_files > 0, `snapshot doctor should parse at least one copied session file, copied ${copied}`);

const overview = readJson("overview-snapshot");
assert(overview.version && overview.summary, "overview snapshot missing version or summary");
assert(overview.summary.total_sessions > 0, "overview snapshot should include sessions");
assert(typeof overview.summary.total_tokens === "number", "overview snapshot missing total token number");
assert(Array.isArray(overview.recent_sessions), "overview snapshot missing recent_sessions");

const latest = readJson("latest-snapshot");
assert(latest.version && latest.session, "latest snapshot missing session metadata");
assert(latest.source_tool, "latest snapshot missing source_tool");

const search = readJson("search-snapshot");
assert(typeof search.count === "number" && Array.isArray(search.results), "search snapshot shape changed");

const compare = readJson("compare-snapshot");
assert(Array.isArray(compare) && compare.length > 0, "compare snapshot shape changed");
assert(compare.every((item) => item.name && item.metrics && typeof item.health === "number"), "compare snapshot entries changed");
NODE

echo "Rust real CLI smoke passed: source_dir=$source_dir sampled_files=$copied query=$query"
