<p align="center">
  <img src="assets/logo-icon.png" alt="agenttrace logo" width="128" height="128">
</p>

<h1 align="center">AgentTrace</h1>

<p align="center">
  Understand local AI coding agent logs: token cost, tool failures, latency, health, diffs, and CI gates.
</p>

<p align="center">
  English | <a href="README.zh-CN.md">简体中文</a>
</p>

<p align="center">
  <a href="https://github.com/luoyuctl/agenttrace/actions/workflows/ci.yml"><img src="https://github.com/luoyuctl/agenttrace/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://luoyuctl.github.io/agenttrace/"><img src="https://img.shields.io/badge/site-agenttrace-54ff00.svg" alt="Site"></a>
  <a href="https://github.com/luoyuctl/agenttrace/releases/latest"><img src="https://img.shields.io/github/v/release/luoyuctl/agenttrace?color=00ADD8" alt="Release"></a>
  <a href="https://pkg.go.dev/github.com/luoyuctl/agenttrace"><img src="https://pkg.go.dev/badge/github.com/luoyuctl/agenttrace.svg" alt="Go Reference"></a>
  <a href="https://goreportcard.com/report/github.com/luoyuctl/agenttrace"><img src="https://goreportcard.com/badge/github.com/luoyuctl/agenttrace" alt="Go Report Card"></a>
  <img src="https://img.shields.io/badge/go-1.25+-00ADD8.svg" alt="Go">
  <img src="https://img.shields.io/badge/license-MIT-green.svg" alt="License">
  <img src="https://img.shields.io/badge/Homebrew-v0.4.0-2bbc8a.svg" alt="Homebrew">
</p>

<p align="center">
  <img src="assets/readme-real-run.gif" alt="agenttrace running locally against real AI coding agent session logs" width="100%">
</p>

---

**agenttrace** is a local TUI and report generator for AI coding agent observability. It reads Claude Code, Codex CLI, Gemini CLI, Qwen Code, Cursor, Aider, OpenCode, OpenClaw, Hermes Agent, Kimi CLI, and Copilot-style logs, then shows token cost, tool failures, latency, anomalies, health regressions, diffs, and CI evidence.

## Why agenttrace?

AI coding agents now behave like small build systems: they call tools, retry, stall, and spend tokens while you only see the final answer.

**agenttrace** reads the logs your agents already write and puts the sessions worth checking first.

It helps you answer:

- **Where is the bill coming from?** See input, output, cache tokens, model pricing, and estimated cost.
- **Which run got stuck?** Catch long gaps, hanging sessions, retry loops, and repeated tool failures.
- **What should I fix next?** Rank sessions by health, cost, failures, anomalies, model, source, or text search.
- **Did the workflow regress?** Compare sessions and fail CI when health drops or tool failures spike.
- **Can I inspect this privately?** Everything runs locally; prompts, code, and logs do not need to leave your machine.

## Real local run

These screenshots were captured from a local run against real session logs. They are not `--demo` output and not test fixtures.

```bash
agenttrace
```

| Overview | Critical sessions |
|---|---|
| <img src="assets/readme-real-overview.png" alt="agenttrace overview showing real local AI coding agent sessions, token cost, errors, and health" width="100%"> | <img src="assets/readme-real-critical.png" alt="agenttrace critical session list from real local AI coding agent logs" width="100%"> |

| Session detail | Diagnostics |
|---|---|
| <img src="assets/readme-real-detail.png" alt="agenttrace detail view showing health, cost, tool failures, and next action from a real local session" width="100%"> | <img src="assets/readme-real-diagnostics.png" alt="agenttrace diagnostics view showing latency, context window, and large parameter calls from real local logs" width="100%"> |

That local run found:

```text
AGENTTRACE v0.4.0
```

| Signal | What agenttrace found |
|---|---:|
| Analyzed sessions | 1,707 |
| Total tokens | 8.68B |
| Estimated cost | $4,716.31 |
| Tool failure rate | 1.54% |
| Critical sessions | 35 |
| Average health | 90% |

## Install

```bash
curl -sL https://raw.githubusercontent.com/luoyuctl/agenttrace/master/install.sh | sh
```

Other install paths:

```bash
brew install luoyuctl/tap/agenttrace
go install github.com/luoyuctl/agenttrace/cmd/agenttrace@latest
```

Windows:

```powershell
iwr -useb https://raw.githubusercontent.com/luoyuctl/agenttrace/master/install.ps1 | iex
```

The npm wrapper is prepared in `npm/`, but the public package is not published yet.

## Common workflows

```bash
# Open the local TUI
agenttrace

# Check detected agent directories and cache state
agenttrace --doctor

# Generate machine-readable evidence
agenttrace --overview -f json

# Create a self-contained report for CI artifacts or issue links
agenttrace --overview -f html -o agenttrace-overview.html

# Fail CI on unhealthy agent runs
agenttrace --overview \
  --fail-under-health 80 \
  --fail-on-critical \
  --max-tool-fail-rate 15
```

## Supported logs

agenttrace supports local sessions from:

Claude Code, Codex CLI, Gemini CLI, Qwen Code, Cline, Aider, Cursor exports, Hermes Agent, OpenCode, OpenClaw, Oh My Pi, Kimi CLI, Copilot-style logs, and generic JSON/JSONL traces.

## What you get

| Need | agenttrace gives you |
|---|---|
| Cost audit | Token totals, cache usage, model pricing, top expensive sessions |
| Reliability triage | Health score, critical/warning buckets, failure rate, anomaly list |
| Slow-run debugging | Latency stats, long gaps, hanging-session detection |
| Prompt and tool fixes | Repeated tool failures, loops, shallow reasoning, redactions |
| Team/CI evidence | JSON, Markdown, and self-contained HTML reports |
| Local-first inspection | No hosted backend required |

## Docs

- Site: https://luoyuctl.github.io/agenttrace/
- Sample HTML report: https://luoyuctl.github.io/agenttrace/demo-report.html
- CI setup: [docs/ci-integration.md](docs/ci-integration.md)
- Cursor import: [docs/cursor-import.md](docs/cursor-import.md)
- Parser guide: [docs/parser-guide.md](docs/parser-guide.md)
- Launch notes: [docs/launch-kit.md](docs/launch-kit.md)

Listed in [Awesome Gemini CLI](https://github.com/Piebald-AI/awesome-gemini-cli), [Charm in the Wild](https://github.com/charm-and-friends/charm-in-the-wild), and [Awesome Claude Code and Skills](https://github.com/GetBindu/awesome-claude-code-and-skills).

## Contributing

Parser PRs are welcome. A good parser contribution usually includes:

- a tiny redacted fixture or synthetic sample
- format detection in `DetectFormat`
- role, timestamp, model, token usage, tool call, and tool error extraction
- tests for successful parsing and malformed input

Run before sending a PR:

```bash
go test ./...
go build -o agenttrace ./cmd/agenttrace/
./agenttrace --doctor
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full contribution flow.

## License

[MIT](LICENSE) © 2025 agenttrace contributors
