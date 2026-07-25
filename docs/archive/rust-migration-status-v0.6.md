# Rust Migration Status — v0.6 archive

> Historical record. The Rust migration completed and `v0.6.0` was released on 2026-07-19. Current release and compatibility guidance lives in the repository README, [distribution guide](../maintainers/distribution.md), and current CI scripts.

The Rust migration is complete in the `v0.6.0` source tree. Rust is the only implementation in the repository. This document preserves the migration-era release-readiness record.

## Final State

- `cmd/`, `internal/`, `go.mod`, and `go.sum` were removed.
- Go golden generation and Go/Rust parity scripts were removed.
- CI, release, install, Homebrew, and docs checks use Rust/Cargo surfaces only.
- The local release gate no longer invokes `go`.
- Real local-data coverage is kept through `scripts/ci/check-rust-real-cli-smoke.sh` and `scripts/ci/check-rust-tui-real-smoke.sh`.

## Rust Workspace

- `crates/agenttrace-core`: data model, discovery, parsers, analysis, pricing, reports, session cache, and SQLite-backed sources.
- `crates/agenttrace-cli`: CLI argument handling, output routing, gates, and TUI launch.
- `crates/agenttrace-tui`: ratatui/crossterm terminal UI.

## Current Gate

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo build --release -p agenttrace
AGENTTRACE_BIN="$PWD/target/release/agenttrace" scripts/ci/check-output-contract.sh
AGENTTRACE_BIN="$PWD/target/release/agenttrace" scripts/ci/check-deterministic-output.sh
AGENTTRACE_BIN="$PWD/target/release/agenttrace" scripts/ci/check-report-semantics.sh
AGENTTRACE_BIN="$PWD/target/release/agenttrace" scripts/ci/check-docs-commands.sh
AGENTTRACE_BIN="$PWD/target/release/agenttrace" scripts/ci/check-rust-real-cli-smoke.sh
scripts/ci/check-release-surfaces.sh
ruby -c homebrew/Formula/agenttrace.rb
bash -n scripts/record-demo.sh scripts/record-real-marketing.sh scripts/ci/*.sh
AGENTTRACE_BIN="$PWD/target/release/agenttrace" scripts/ci/check-rust-tui-real-smoke.sh
scripts/ci/check-rust-release-local.sh
```

## Historical release note

`v0.6.0` was the first Rust-only release. It was published with the Rust target asset matrix and followed by Homebrew tap synchronization.
