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
