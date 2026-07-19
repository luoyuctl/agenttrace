# Changelog

## v0.7.0 - 2026-07-19

### Added

- Added local governance reports for cost audits, prioritized recommendations, observed MCP usage, cross-session context trends, and read-only Git delivery evidence.
- Added optional local model aliases and per-million-token pricing overrides through `AGENTTRACE_PRICING_FILE`.
- Added report scope, pricing confidence, project-root resolution, cache-aware doctor output, and governance appendices to overview exports.
- Added Action Center, Efficiency, and Delivery workspaces to the TUI, plus bilingual UI copy, live search, paste handling, and report scrollbars.

### Changed

- Updated the CLI, TUI, reports, plugin, Homebrew Formula, Pages assets, and release metadata to v0.7.0.
- Made CLI report actions mutually exclusive and hardened validation for gate thresholds and output formats.
- Made delivery and MCP output explicit about evidence limits: commit correlation is not authorship or merge proof, and invocation logs do not imply complete MCP inventory coverage.

### Validation

- Added governance, TUI interaction, discovery, report, and release-surface coverage to the Rust test and CI paths.

## v0.6.0 - 2026-07-19

### Added

- Replaced the Go implementation with a Rust workspace while preserving one
  `agenttrace` binary for both CLI reports and the terminal TUI.
- Added cached background session loading, Hermes/OpenCode SQLite sources,
  `Detailed`/`Aggregate`/`Limited` data capability labels, coverage reporting,
  privacy-safe tool-step metadata, actionable issue filters, and shared Core
  findings/comparison rules across CLI and TUI.
- Added deterministic generated parser fixtures and CI checks for provider
  coverage, data degradation, step redaction, and single-binary entrypoints.

### Changed

- Split the Rust TUI into state, presentation, filtering, and test modules and
  moved shared data-health/comparison logic into `agenttrace-core`.
- Streamed JSONL object parsing instead of retaining whole-file JSON trees,
  reducing peak memory by about 44% on a 2.53 GiB local session corpus while
  keeping report totals unchanged.
- Added `Ctrl+d`/`Ctrl+u` half-page movement and `G` end navigation to the TUI.
- Updated public docs, Pages, plugin, and Skill surfaces to describe the Rust
  implementation and honest per-source evidence limits.

### Fixed

- Invalidated SQLite session snapshots when the database, WAL, or SHM file
  changes.
- Hardened preserved-history loading against malformed short identifiers and
  stabilized the real-data CLI/TUI release smoke checks.

### Validation

- The Rust workspace, release binary, parser fixtures, CLI/TUI PTY entrypoints,
  report contracts, Homebrew syntax, and Pages artifact are covered by the
  local release gate.

## v0.5.4 - 2026-05-24

### Changed

- Published cross-platform command-line release assets and checksums.

## v0.5.3 - 2026-05-24

### Changed

- Refreshed release-facing artifacts for the v0.5 line.

## v0.5.2 - 2026-05-24

### Fixed

- Fixed Claude Code JSONL metrics for assistant messages that include thinking,
  text, and a parallel `tool_use` batch so the report keeps one assistant turn,
  multiple tool calls, cache token attribution, and failed `tool_result`
  counting aligned. (#243)

## v0.5.1 - 2026-05-19

### Fixed

- Clarified `agenttrace --doctor` cache-state wording so users can distinguish
  parsed session cache entries, entries reusable for the current scan, and
  cached directory listings. (#239)

## v0.5.0 - 2026-05-18

### Added

- Added local baseline comparison for overview reports so a later run can be
  checked against a saved local JSON baseline. (#203)
- Added incident timeline evidence to the TUI and report surfaces. (#204)
- Added tool authority summaries to HTML, Markdown, and text overview reports.
  (#210, #212, #214, #219, #221)

### Changed

- Improved overview report readability for Unicode text, incident rows, and
  terminal-readable authority summaries. (#216, #217, #219, #221)
- Aligned public README, docs, site metadata, and discovery surfaces with the
  current local coding-agent session coverage. (#197, #202, #228, #229, #230,
  #231, #232)
- Removed stale package-channel and launch-kit surfaces so release-facing
  install guidance stays limited to available channels. (#225, #226)

### Validation

- Added and refreshed release-surface, report-semantics, Pages artifact, and
  parser-coverage checks for the v0.5.0 release train. (#178, #182, #184, #205,
  #208)

## v0.4.6 - 2026-05-10

### Fixed

- Show sessions from `~/.pi/agent/sessions` as Pi while keeping
  legacy `~/.omp/agent/sessions` sessions labeled Oh My Pi.

## v0.4.5 - 2026-05-10

### Fixed

- Added PI auto-discovery for `~/.pi/agent/sessions` while keeping the legacy
  Oh My Pi `~/.omp/agent/sessions` path for compatibility.

## v0.4.4 - 2026-05-10

### Added

- Added a real local-data marketing refresh script for README and site assets.

### Changed

- Capped overview JSON anomaly details and added anomaly total/truncation metadata
  so large real histories stay readable for automation and promotional reports.
- Refreshed README and site screenshots from a real local run.

## v0.4.3 - 2026-05-10

### Changed

- Updated release surfaces for the v0.4.3 distribution.

## v0.4.2 - 2026-05-05

### Changed

- Refreshed README GIF and screenshots from a real local run with color enabled.
- Updated release surfaces for the v0.4.2 install paths.

### Fixed

- Kept Session List table values readable when terminal colors are enabled.

## v0.4.1 - 2026-05-04

### Changed

- Refreshed the README's real local-run screenshots and summary metrics from
  the latest TUI against local session logs.
- Updated release surfaces for the v0.4.1 distribution.

## v0.4.0 - 2026-05-04

### Changed

- Polished the first-run TUI demo path with clearer selected-session context,
  scan-friendly status text, refreshed demo assets, and an updated recording
  script. (#91)
- Improved TUI feedback around loading, empty diff states, and command-mode
  results so users get immediate guidance while navigating. (#122)
- Made `--waste` use the same latest-session selection behavior as `--latest`,
  reuse loaded diagnostics, and show clearer waste-report copy. (#95)

### Fixed

- Stabilized overview report ordering for recent sessions and anomaly tie
  breakers across JSON, Markdown, and HTML outputs. (#104)
- Aligned overview aggregate metrics with TUI discovery, including cache
  read/write tokens in the exported totals. (#114)
- Clamped loop waste so reported waste cannot exceed total session cost. (#116)
- Aligned TUI cache status wording with `agenttrace --doctor`. (#117)
- Isolated auto-discovery tests from runner-specific environment configuration.
  (#120)

### Validation

- Added repeatable CI gates for output contracts, deterministic demo output,
  report semantics, release surfaces, and Pages artifacts. (#118)
- Documented the launch-kit validation gates and release consistency checklist
  for public demo and install surfaces. (#115, #121)
