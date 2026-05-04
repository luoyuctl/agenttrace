#!/usr/bin/env bash
set -euo pipefail

bin="${AGENTTRACE_BIN:-/tmp/agenttrace}"
out_dir="${AGENTTRACE_CI_OUT:-/tmp/agenttrace-ci}"
expected_cost_label="${AGENTTRACE_EXPECTED_COST_LABEL:-Total estimated cost}"

fail() {
  echo "check-report-semantics: $*" >&2
  exit 1
}

[[ -x "$bin" ]] || fail "agenttrace binary is not executable: $bin"
mkdir -p "$out_dir/semantics"

version="$("$bin" --version | sed -E 's/^agenttrace v//')"
[[ -n "$version" ]] || fail "could not read agenttrace version"

"$bin" --demo --overview >"$out_dir/semantics/overview.txt"
"$bin" --demo --overview -f json >"$out_dir/semantics/overview.json"
"$bin" --demo --overview -f markdown >"$out_dir/semantics/overview.md"
"$bin" --demo --overview -f html >"$out_dir/semantics/overview.html"

grep -q "AGENTTRACE v$version" "$out_dir/semantics/overview.txt" \
  || fail "text overview missing current version"
grep -q "$expected_cost_label" "$out_dir/semantics/overview.txt" \
  || fail "text overview missing expected cost label"
grep -q "| $expected_cost_label |" "$out_dir/semantics/overview.md" \
  || fail "markdown overview missing expected cost label"
grep -q "<span>$expected_cost_label</span>" "$out_dir/semantics/overview.html" \
  || fail "html overview missing expected cost label"
grep -q "<div class=\"meta\">v$version" "$out_dir/semantics/overview.html" \
  || fail "html overview missing current version metadata"
grep -q "Estimated session cost" "$out_dir/semantics/overview.html" \
  || fail "html overview missing cost helper text"

node -e '
const fs = require("fs");
const version = process.argv[2];
const report = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
if (report.version !== version) throw new Error(`json version ${report.version} != ${version}`);
if (!report.summary || typeof report.summary.total_cost !== "number") throw new Error("missing total_cost");
if (report.summary.total_sessions !== 3) throw new Error("demo summary should contain 3 sessions");
if (!Array.isArray(report.recent_sessions) || report.recent_sessions.length !== 3) throw new Error("demo should contain 3 recent sessions");
' "$out_dir/semantics/overview.json" "$version"
