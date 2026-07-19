#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

out_dir="${AGENTTRACE_CI_OUT:-/tmp/agenttrace-rust-release-local}"
bin="$repo_root/target/release/agenttrace"

fail() {
  echo "check-rust-release-local: $*" >&2
  exit 1
}

run() {
  echo "+ $*"
  "$@"
}

run_env() {
  echo "+ $*"
  env "$@"
}

rm -rf "$out_dir"
mkdir -p "$out_dir"

run cargo fmt --check
run cargo clippy -p agenttrace-core -p agenttrace-tui -p agenttrace -- -D warnings
run cargo test -p agenttrace-core -p agenttrace-tui -p agenttrace
run python3 scripts/generate-testdata.py --check
run cargo build --release -p agenttrace
run scripts/ci/check-cargo-manifests.sh

[[ -x "$bin" ]] || fail "release binary is not executable: $bin"
run "$bin" --version
run_env AGENTTRACE_BIN="$bin" python3 scripts/ci/check-single-binary-entrypoints.py

run_env AGENTTRACE_BIN="$bin" AGENTTRACE_CI_OUT="$out_dir/contracts" \
  scripts/ci/check-output-contract.sh
run_env AGENTTRACE_BIN="$bin" AGENTTRACE_CI_OUT="$out_dir/contracts" \
  scripts/ci/check-deterministic-output.sh
run_env AGENTTRACE_BIN="$bin" AGENTTRACE_CI_OUT="$out_dir/contracts" \
  scripts/ci/check-report-semantics.sh
run_env AGENTTRACE_BIN="$bin" AGENTTRACE_CI_OUT="$out_dir/contracts" \
  scripts/ci/check-docs-commands.sh
run_env AGENTTRACE_BIN="$bin" AGENTTRACE_REAL_CLI_OUT="$out_dir/real-cli-smoke" \
  scripts/ci/check-rust-real-cli-smoke.sh

run scripts/ci/check-release-surfaces.sh
run scripts/ci/check-pages-artifact.sh site
run ruby -c homebrew/Formula/agenttrace.rb
run bash -n scripts/record-demo.sh scripts/record-real-marketing.sh scripts/ci/*.sh

if [[ "${AGENTTRACE_SKIP_TUI_REAL_SMOKE:-}" == "1" ]]; then
  echo "Skipping Rust TUI real-data smoke because AGENTTRACE_SKIP_TUI_REAL_SMOKE=1"
else
  run_env AGENTTRACE_BIN="$bin" scripts/ci/check-rust-tui-real-smoke.sh
fi

echo "Rust local release gate passed: $bin"
