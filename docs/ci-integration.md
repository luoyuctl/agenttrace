# CI Integration

Use `agenttrace --overview` as a quality gate for AI agent sessions in pull requests or nightly jobs.

## Local Check

```bash
agenttrace --overview \
  --fail-under-health 80 \
  --fail-on-critical \
  --max-tool-fail-rate 15
```

The command exits with code `2` when a gate fails. Add `-f json -o agenttrace-overview.json` when CI should upload the report as an artifact.

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
      - uses: actions/upload-artifact@v7
        if: always()
        with:
          name: agenttrace-overview
          path: agenttrace-overview.json
```

Tune thresholds per repository. A stricter team can start with health `90` and tool failure rate `5`; early adopters may start at `70` and `25` to avoid blocking useful experimentation.
