#!/usr/bin/env bash
set -euo pipefail

bin="${AGENTTRACE_BIN:-/tmp/agenttrace}"
out_dir="${AGENTTRACE_CI_OUT:-/tmp/agenttrace-ci}"

fail() {
  echo "check-deterministic-output: $*" >&2
  exit 1
}

[[ -x "$bin" ]] || fail "agenttrace binary is not executable: $bin"
mkdir -p "$out_dir/determinism"

for i in 1 2 3; do
  "$bin" --demo --latest -f json >"$out_dir/determinism/latest-$i.json"
  "$bin" --demo --overview -f json >"$out_dir/determinism/overview-$i.json"
done

for path in "$out_dir"/determinism/*.json; do
  node -e 'JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"))' "$path" \
    || fail "invalid JSON: $path"
done

cmp -s "$out_dir/determinism/latest-1.json" "$out_dir/determinism/latest-2.json" \
  || fail "--demo --latest -f json changed between run 1 and 2"
cmp -s "$out_dir/determinism/latest-1.json" "$out_dir/determinism/latest-3.json" \
  || fail "--demo --latest -f json changed between run 1 and 3"
cmp -s "$out_dir/determinism/overview-1.json" "$out_dir/determinism/overview-2.json" \
  || fail "--demo --overview -f json changed between run 1 and 2"
cmp -s "$out_dir/determinism/overview-1.json" "$out_dir/determinism/overview-3.json" \
  || fail "--demo --overview -f json changed between run 1 and 3"
