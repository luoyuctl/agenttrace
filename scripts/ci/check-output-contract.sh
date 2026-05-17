#!/usr/bin/env bash
set -euo pipefail

bin="${AGENTTRACE_BIN:-/tmp/agenttrace}"
out_dir="${AGENTTRACE_CI_OUT:-/tmp/agenttrace-ci}"

fail() {
  echo "check-output-contract: $*" >&2
  exit 1
}

require_file() {
  local path="$1"
  [[ -s "$path" ]] || fail "expected non-empty file: $path"
}

require_json() {
  local path="$1"
  node -e 'JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"))' "$path" \
    || fail "invalid JSON: $path"
}

[[ -x "$bin" ]] || fail "agenttrace binary is not executable: $bin"
mkdir -p "$out_dir"

"$bin" --doctor -f json >"$out_dir/doctor.json"
require_file "$out_dir/doctor.json"
require_json "$out_dir/doctor.json"

"$bin" --demo --overview -f json >"$out_dir/agenttrace-demo.json"
require_file "$out_dir/agenttrace-demo.json"
require_json "$out_dir/agenttrace-demo.json"
node -e '
const report = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
if (!report.version) throw new Error("missing version");
if (!report.summary || report.summary.total_sessions < 1) throw new Error("missing demo summary");
if (typeof report.summary.total_duration_seconds !== "number") throw new Error("missing duration summary");
if (!Array.isArray(report.recent_sessions) || report.recent_sessions.length < 1) throw new Error("missing recent sessions");
if (!report.surfaces || !Array.isArray(report.surfaces.tools) || !Array.isArray(report.surfaces.files)) {
  throw new Error("missing comparison surfaces");
}
' "$out_dir/agenttrace-demo.json"

"$bin" --demo --overview -f json \
  --baseline "$out_dir/agenttrace-demo.json" \
  --baseline-max-duration-delta-pct 0 \
  --baseline-max-cost-delta-pct 0 \
  --baseline-max-token-delta-pct 0 \
  >"$out_dir/agenttrace-baseline-compare.json"
require_file "$out_dir/agenttrace-baseline-compare.json"
require_json "$out_dir/agenttrace-baseline-compare.json"
node -e '
const report = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
const cmp = report.baseline_comparison;
if (!cmp) throw new Error("missing baseline_comparison");
for (const key of ["slower_than_baseline", "cost_delta_pct", "token_delta_pct", "new_failure_families", "broader_tool_surface", "broader_file_surface", "new_high_authority_tool_use"]) {
  if (!(key in cmp)) throw new Error(`missing baseline comparison field ${key}`);
}
if (cmp.slower_than_baseline || cmp.cost_delta_pct !== 0 || cmp.token_delta_pct !== 0) {
  throw new Error("identical demo baseline should not regress");
}
' "$out_dir/agenttrace-baseline-compare.json"

"$bin" --demo --overview -f markdown \
  -o "$out_dir/agenttrace-demo.md" \
  >"$out_dir/agenttrace-demo.md.stdout" \
  2>"$out_dir/agenttrace-demo.md.stderr"
require_file "$out_dir/agenttrace-demo.md"
require_file "$out_dir/agenttrace-demo.md.stdout"
grep -q '^# agenttrace overview' "$out_dir/agenttrace-demo.md.stdout" \
  || fail "markdown stdout should contain the report body"
grep -q '^# agenttrace overview' "$out_dir/agenttrace-demo.md" \
  || fail "saved markdown should contain the report body"
grep -q '^Saved: ' "$out_dir/agenttrace-demo.md.stderr" \
  || fail "markdown -o should write saved-file status to stderr"
! grep -q '^Saved: ' "$out_dir/agenttrace-demo.md.stdout" \
  || fail "markdown stdout must not contain saved-file status"

"$bin" --demo --overview -f html \
  -o "$out_dir/agenttrace-demo.html" \
  >"$out_dir/agenttrace-demo.html.stdout" \
  2>"$out_dir/agenttrace-demo.html.stderr"
require_file "$out_dir/agenttrace-demo.html"
require_file "$out_dir/agenttrace-demo.html.stdout"
grep -q '^<!doctype html>' "$out_dir/agenttrace-demo.html.stdout" \
  || fail "html stdout should contain the report body"
grep -q '^<!doctype html>' "$out_dir/agenttrace-demo.html" \
  || fail "saved html should contain the report body"
grep -q '^Saved: ' "$out_dir/agenttrace-demo.html.stderr" \
  || fail "html -o should write saved-file status to stderr"
! grep -q '^Saved: ' "$out_dir/agenttrace-demo.html.stdout" \
  || fail "html stdout must not contain saved-file status"

set +e
"$bin" --demo --overview -f json \
  --fail-under-health 80 \
  --fail-on-critical \
  --max-tool-fail-rate 15 \
  -o "$out_dir/agenttrace-gate.json" \
  >"$out_dir/agenttrace-gate.json.stdout" \
  2>"$out_dir/agenttrace-gate.json.stderr"
status=$?
set -e

[[ "$status" -eq 2 ]] || fail "expected gate failure exit code 2, got $status"
require_file "$out_dir/agenttrace-gate.json"
require_file "$out_dir/agenttrace-gate.json.stdout"
require_json "$out_dir/agenttrace-gate.json"
require_json "$out_dir/agenttrace-gate.json.stdout"
node -e '
const fs = require("fs");
const saved = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
const stdout = JSON.parse(fs.readFileSync(process.argv[2], "utf8"));
if (JSON.stringify(saved) !== JSON.stringify(stdout)) {
  throw new Error("saved JSON and stdout JSON differ");
}
' "$out_dir/agenttrace-gate.json" "$out_dir/agenttrace-gate.json.stdout" \
  || fail "gated json -o stdout should match the saved report"
grep -q '^Saved: ' "$out_dir/agenttrace-gate.json.stderr" \
  || fail "gated json -o should write saved-file status to stderr"
grep -q 'Gate failed:' "$out_dir/agenttrace-gate.json.stderr" \
  || fail "gated json should write gate failures to stderr"
! grep -q '^Saved: ' "$out_dir/agenttrace-gate.json.stdout" \
  || fail "gated json stdout must not contain saved-file status"
