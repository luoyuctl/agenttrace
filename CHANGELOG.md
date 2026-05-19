# Changelog

## Unreleased

v0.5.1 release candidate notes:

### Fixed

- Clarified `agenttrace --doctor` cache-state wording so users can distinguish
  parsed session cache entries, entries reusable for the current scan, and
  cached directory listings. (#239)

v0.5.0 release candidate notes:

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
