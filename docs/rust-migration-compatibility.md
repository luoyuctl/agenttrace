# Rust Compatibility Checklist

This document tracks the external behavior the Rust-only implementation must keep stable after the Go implementation was removed for `v0.6.0`.

## Command Contracts

- `agenttrace --version`
- `agenttrace --doctor -f json`
- `agenttrace --doctor -d testdata/opencode/storage/session/project_alpha -f json`
- `agenttrace --list-models`
- `agenttrace --test-match`
- `agenttrace --demo --latest -f json`
- `agenttrace --demo --latest --lang zh -f json`
- `agenttrace --demo --overview -f json`
- `agenttrace --demo --overview -f markdown`
- `agenttrace --demo --overview -f html`
- `agenttrace --demo --search billing`
- `agenttrace --demo --search internal/ws -f json`
- `agenttrace --demo --overview -f json --baseline <baseline> --baseline-max-duration-delta-pct <pct> --baseline-max-cost-delta-pct <pct> --baseline-max-token-delta-pct <pct>`
- `agenttrace --demo --overview --fail-under-health 80 --fail-on-critical --max-tool-fail-rate 15`
- `agenttrace -d testdata --latest`
- `agenttrace -d testdata --overview`
- `agenttrace -d testdata --overview -f json`
- `agenttrace -d testdata --overview -f markdown`
- `agenttrace -d testdata --overview -f html`
- `agenttrace -d testdata --search internal/ws`
- `agenttrace -d testdata --search internal/ws -f json`
- `agenttrace -d testdata --search read_file`
- `agenttrace -d testdata --search read_file -f json`
- `agenttrace -d <invalid-args-fixture-dir> --search invalid_args -f json`
- `agenttrace -d <invalid-args-fixture-dir> --search malformed`
- `agenttrace testdata/claude-code-preamble.jsonl`
- `agenttrace testdata/cursor-composer-messages.json`
- `agenttrace testdata/gemini-current-chat.json`
- `agenttrace testdata/kimi-tool-args.json`
- `agenttrace testdata/opencode/storage/session/project_alpha/ses_abc.json`
- `agenttrace -f json testdata/claude-code-preamble.jsonl`
- `agenttrace -f json testdata/cursor-composer-messages.json`
- `agenttrace -f json testdata/gemini-current-chat.json`
- `agenttrace -f json testdata/kimi-tool-args.json`
- `agenttrace -f json testdata/opencode/storage/session/project_alpha/ses_abc.json`
- `agenttrace -d testdata --compare`
- `agenttrace -d testdata --compare -f json`
- `agenttrace -d testdata --waste`

## Behavioral Invariants

- CLI flags, defaults, positional path behavior, output file behavior, stdout/stderr separation, and exit codes remain stable.
- JSON field names, numeric rounding, list ordering, and omitted fields remain stable for machine-readable outputs.
- Text, Markdown, and HTML reports keep the documented sections used by CI checks.
- Demo mode remains deterministic across repeated runs.
- Search returns metadata/tool/file/anomaly evidence without indexing prompt text.
- Baseline comparison reports zero deltas for identical demo baselines and keeps deterministic regression fields.
- Gate failures return exit code 2 and write failure evidence to stderr.
- Session discovery keeps auto-discovery, custom directories, Cline task directories, Gemini temp chats, OpenCode storage sessions, Pi/Oh My Pi, and Aider history behavior.
- Hermes and OpenCode SQLite-backed sessions are read-only and remain part of auto-discovery.
- Session cache uses the same default OS cache path, remains safe to discard, and refreshes stale derived metrics when its schema version changes.
- TUI preserves overview, list, detail, diagnostics, diff, filter, sort, search, reload, force reload, help, command, and quit interactions.

## Rust Shape

- `crates/agenttrace-core`: data model, detection, parsers, analysis, pricing, reports, cache, SQLite.
- `crates/agenttrace-cli`: clap-based CLI and process-level behavior.
- `crates/agenttrace-tui`: ratatui/crossterm TUI.

## Release Gate

The Rust-only release gate is `scripts/ci/check-rust-release-local.sh`. It must not require `go`, `go.mod`, `go.sum`, `cmd/`, `internal/`, Go golden files, or Go/Rust parity scripts.
