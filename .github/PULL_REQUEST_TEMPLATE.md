## Summary

What changed and why?

## Coordination

- Linked issue:
- Protected public surface touched? no
- Parser PR mergeability checked after latest `master`? n/a

## Validation

- [ ] `cargo fmt --check`
- [ ] `cargo clippy -- -D warnings`
- [ ] `cargo test`
- [ ] `cargo build --release -p agenttrace`
- [ ] `scripts/ci/check-output-contract.sh`
- [ ] `scripts/ci/check-deterministic-output.sh`
- [ ] `scripts/ci/check-report-semantics.sh`
- [ ] `scripts/ci/check-release-surfaces.sh`
- [ ] `scripts/ci/check-docs-commands.sh`
- [ ] `AGENTTRACE_BIN="$PWD/target/release/agenttrace" scripts/ci/check-rust-real-cli-smoke.sh`
- [ ] `AGENTTRACE_BIN="$PWD/target/release/agenttrace" scripts/ci/check-rust-tui-real-smoke.sh`
- [ ] `scripts/ci/check-rust-release-local.sh`
- [ ] `scripts/ci/check-cargo-manifests.sh`
- [ ] `ruby -c homebrew/Formula/agenttrace.rb`

## Notes

Mention any parser fixtures, screenshots, or privacy-sensitive test data that were intentionally omitted.
