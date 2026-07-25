#!/usr/bin/env bash
set -euo pipefail

bin="${AGENTTRACE_BIN:-/tmp/agenttrace}"
out_dir="${AGENTTRACE_CI_OUT:-/tmp/agenttrace-ci}"

fail() {
  echo "check-docs-commands: $*" >&2
  exit 1
}

[[ -x "$bin" ]] || fail "agenttrace binary is not executable: $bin"
mkdir -p "$out_dir/docs"

"$bin" --version >"$out_dir/docs/version.txt"
"$bin" --doctor -f json >"$out_dir/docs/doctor.json"
"$bin" --demo --latest -f json >"$out_dir/docs/latest.json"
"$bin" --demo --latest --lang zh -f json >"$out_dir/docs/latest-zh.json"
"$bin" --demo --overview -f json >"$out_dir/docs/overview.json"
"$bin" --demo --search billing >"$out_dir/docs/search.txt"
"$bin" --demo --search internal/ws -f json >"$out_dir/docs/search.json"
"$bin" --demo --overview -f markdown -o "$out_dir/docs/overview.md" >/tmp/agenttrace-docs-md.stdout
"$bin" --demo --overview -f html -o "$out_dir/docs/overview.html" >/tmp/agenttrace-docs-html.stdout
for path in "$out_dir"/docs/*.json; do
  node -e 'JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"))' "$path" \
    || fail "invalid JSON from documented command: $path"
done

set +e
"$bin" --demo --overview \
  --fail-under-health 80 \
  --fail-on-critical \
  --max-tool-fail-rate 15 \
  >"$out_dir/docs/gate.stdout" \
  2>"$out_dir/docs/gate.stderr"
status=$?
set -e
[[ "$status" -eq 2 ]] || fail "documented CI gate command should exit 2 for demo data, got $status"
grep -q 'Gate failed:' "$out_dir/docs/gate.stderr" \
  || fail "documented CI gate command should explain gate failures on stderr"
