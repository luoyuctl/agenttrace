# CI Integration

Use `agenttrace --overview` as a quality gate for AI agent sessions in pull requests or nightly jobs.

The goal is to catch agent workflow regressions before they become invisible cost:

- a PR's agent run starts hanging or retrying tools
- a nightly automation job burns more tokens than usual
- a team switches agent tools and loses session health visibility
- a local-first project wants CI evidence without uploading prompts or raw logs to a hosted trace service

Start with report-only artifacts, then turn on blocking thresholds once the team knows its normal health and tool failure range.

## Local Check

```bash
agenttrace --overview \
  --fail-under-health 80 \
  --fail-on-critical \
  --max-tool-fail-rate 15
```

The command exits with code `2` when a gate fails. Add `-f json -o agenttrace-overview.json` when CI should upload machine-readable data, `-f markdown -o agenttrace-overview.md` when the report should be pasted into a PR comment, or `-f html -o agenttrace-overview.html` for a self-contained visual artifact.

With `-o`, agenttrace keeps the report body on stdout while also writing the file.
Saved-file confirmations and gate diagnostics are written to stderr; this keeps JSON stdout
machine-readable and lets Markdown or HTML output remain useful for logs and previews.

For the first few runs, keep the job non-blocking while still collecting evidence:

```bash
agenttrace --overview -f markdown -o agenttrace-overview.md || true
agenttrace --overview -f html -o agenttrace-overview.html || true
```

When the output matches what the team cares about, enable blocking checks:

```bash
agenttrace --overview -f json \
  --fail-under-health 80 \
  --fail-on-critical \
  --max-tool-fail-rate 15 \
  -o agenttrace-overview.json
```

To compare a current run with a local CI baseline artifact, keep a previous
`--overview -f json` report and pass it back with explicit delta thresholds:

```bash
agenttrace --overview -f json \
  --baseline agenttrace-baseline.json \
  --baseline-max-duration-delta-pct 10 \
  --baseline-max-cost-delta-pct 15 \
  --baseline-max-token-delta-pct 20 \
  -o agenttrace-overview.json
```

The JSON report includes `baseline_comparison` with deterministic fields for
duration, cost, token deltas, new failure families, broader tool/file surfaces,
new tool authority categories, and new high-authority tool use. Baseline reports
must be local JSON artifacts from the same agenttrace version.

## GitHub Actions

```yaml
name: Agenttrace

on:
  pull_request:
  workflow_dispatch:

jobs:
  agenttrace:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
      - name: Install agenttrace
        run: |
          mkdir -p "$HOME/.local/bin"
          curl -fsSL https://raw.githubusercontent.com/luoyuctl/agenttrace/master/install.sh | AGENTTRACE_INSTALL_DIR="$HOME/.local/bin" sh
          echo "$HOME/.local/bin" >> "$GITHUB_PATH"
      - name: Check agent session health
        run: |
          agenttrace --overview -f json \
            --fail-under-health 80 \
            --fail-on-critical \
            --max-tool-fail-rate 15 \
            -o agenttrace-overview.json
      - name: Compare against local baseline
        if: hashFiles('agenttrace-baseline.json') != ''
        run: |
          agenttrace --overview -f json \
            --baseline agenttrace-baseline.json \
            --baseline-max-duration-delta-pct 10 \
            --baseline-max-cost-delta-pct 15 \
            --baseline-max-token-delta-pct 20 \
            -o agenttrace-overview.json
      - name: Write Markdown summary
        if: always()
        run: |
          agenttrace --overview -f markdown -o agenttrace-overview.md || true
          agenttrace --overview -f html -o agenttrace-overview.html || true
      - uses: actions/upload-artifact@v7
        if: always()
        with:
          name: agenttrace-overview
          path: |
            agenttrace-overview.json
            agenttrace-overview.md
            agenttrace-overview.html
```

Tune thresholds per repository. A stricter team can start with health `90` and tool failure rate `5`; early adopters may start at `70` and `25` to avoid blocking useful experimentation.

## Repository CI Gates

This repository also runs agenttrace against its own demo and docs surfaces so repetitive Agent validation becomes a stable CI contract.

The project CI builds `/tmp/agenttrace` and runs:

```bash
scripts/ci/check-output-contract.sh
scripts/ci/check-deterministic-output.sh
scripts/ci/check-report-semantics.sh
scripts/ci/check-release-surfaces.sh
scripts/ci/check-docs-commands.sh
scripts/ci/check-pages-artifact.sh site
```

These checks cover:

- demo JSON, Markdown, HTML, and doctor smoke output
- local baseline comparison JSON contract and deterministic fields
- `-o` stdout/stderr behavior and failing gate exit code `2`
- repeated demo latest/overview JSON determinism
- report cost-label and version metadata consistency
- README, Homebrew formula, site metadata, and sample report version drift
- non-interactive README/docs command smoke tests
- Pages local asset references and sample report metadata

CI uploads generated demo reports as artifacts so reviewers can inspect the JSON, Markdown, and HTML output without rerunning local commands.
