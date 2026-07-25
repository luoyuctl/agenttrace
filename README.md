<p align="center">
  <img src="assets/logo-icon.png" alt="agenttrace logo" width="256" height="256">
</p>

<h1 align="center">AgentTrace</h1>

<p align="center">
  Local-first TUI and reports for AI coding-agent session history, cost, tokens, time, and slow-run diagnosis.
</p>

<p align="center">
  English | <a href="README.zh-CN.md">简体中文</a>
</p>

<p align="center">
  <a href="https://github.com/luoyuctl/agenttrace/actions/workflows/ci.yml"><img src="https://github.com/luoyuctl/agenttrace/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/luoyuctl/agenttrace/releases/latest"><img src="https://img.shields.io/github/v/release/luoyuctl/agenttrace?color=00ADD8" alt="Release"></a>
  <img src="https://img.shields.io/badge/Rust-stable-f74c00.svg" alt="Rust">
  <img src="https://img.shields.io/badge/license-MIT-green.svg" alt="License">
  <a href="https://github.com/luoyuctl/homebrew-tap"><img src="https://img.shields.io/badge/Homebrew-tap-2bbc8a.svg" alt="Homebrew tap"></a>
  <a href="https://www.npmjs.com/package/@zack78/agenttrace"><img src="https://img.shields.io/npm/v/@zack78/agenttrace?label=npm" alt="npm"></a>
  <a href="https://github.com/microsoft/winget-pkgs"><img src="https://img.shields.io/badge/WinGet-Luoyuctl.AgentTrace-0078D4.svg" alt="WinGet"></a>
</p>

<p align="center">
  <img src="assets/readme-real-run.gif" alt="agenttrace running locally against real AI coding agent session logs" width="100%">
</p>

---

**agenttrace** is a local-first terminal TUI and report generator for AI coding-agent session history. It reads Claude Code, Codex CLI, Gemini CLI, Qwen Code, Cline, Aider, Cursor exports, Hermes Agent, OpenCode, OpenClaw, Pi, Oh My Pi, Kimi CLI, Copilot-style logs, and generic JSON/JSONL traces, then helps with two daily jobs: see what multiple agents spent across cost, tokens, and time; and diagnose why a task ran slowly.

One `agenttrace` binary provides both interfaces: run it without a report action to open the TUI, or pass flags such as `--sessions` and `--overview` for CLI output.

## Why agenttrace?

AI coding agents now behave like small build systems: they call tools, retry, stall, and spend tokens while you only see the final answer.

**agenttrace** reads the logs your agents already write and puts cost-heavy or slow sessions first.

It helps you answer:

- **What did my agents spend?** Compare historical sessions by agent source, model, input/output/cache tokens, estimated cost, and wall-clock time.
- **Why was this task slow?** Catch long gaps, hanging sessions, retry loops, slow tool calls, large parameters, and context pressure.
- **Did a run regress?** Compare against a local baseline when supplied, then inspect incident timelines and conservative tool authority categories in reports.
- **What should I inspect first?** Rank sessions by cost, duration, turns, health, failures, anomalies, model, source, or text search.
- **Can I inspect this privately?** Everything runs locally; prompts, code, and logs do not need to leave your machine.

## Real local run

These screenshots and figures are a redacted sample of the latest 500 real local sessions captured for the v0.7.1 source tree. They are not `--demo` output, current telemetry, or test fixtures.

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
AGENTTRACE v0.7.1
```

| Signal | What agenttrace found |
| --- | ---: |
| Analyzed sessions | 500 |
| Total tokens | 4.0B |
| Estimated cost | $2.6K |
| Tool failure rate | 2.9% |
| Critical sessions | 2 |
| Average health | 77.7% |

## Install

Install the latest public release with your package manager. Check the installed
version with `agenttrace --version`.

```bash
# macOS and Linux
brew install luoyuctl/tap/agenttrace

# macOS, Linux, and Windows (requires Node.js 18+)
npm install -g @zack78/agenttrace
```

Windows:

```powershell
winget install --id Luoyuctl.AgentTrace --exact
```

The package names above become available once the corresponding release has
been published. Manual installs remain available without a package manager:

```bash
curl -fsSL https://raw.githubusercontent.com/luoyuctl/agenttrace/master/install.sh | sh
cargo install --git https://github.com/luoyuctl/agenttrace agenttrace
```

```powershell
iwr -useb https://raw.githubusercontent.com/luoyuctl/agenttrace/master/install.ps1 | iex
```

## Quickstart

```bash
agenttrace
```

### Governance reports

```bash
# Audit raw token components, normalized pricing, and fallback confidence.
agenttrace --audit --range 30d -f json

# Optional local model aliases and per-million-token price overrides.
AGENTTRACE_PRICING_FILE=pricing-overrides.json agenttrace --audit -f json

# Rank evidence-backed actions by severity and estimated impact.
agenttrace --recommend --range 30d -f json

# Inspect observed MCP invocations. Loaded-server coverage is intentionally
# reported as unavailable unless the source log actually records it.
agenttrace --mcp-governance --range 30d -f json

# Review cross-session context, cache, repeat-read, and read/write trends.
agenttrace --context-trends --range 30d -f json

# Correlate local Git commit timestamps with sessions (heuristic, read-only).
agenttrace --delivery-evidence --range 30d -f json
```

`pricing-overrides.json` accepts `aliases` plus per-million-token `prices`:

```json
{"aliases":{"provider/raw-model":"my-model"},"prices":{"my-model":{"input":1,"output":2,"cw":0,"cr":0}}}
```

`--overview` now includes scope, parse and pricing confidence, cost audit,
prioritized recommendations, MCP governance, context trends, and delivery
signals in JSON, Markdown, and HTML output. All cost and delivery fields are
explicitly estimates or heuristics; they are not provider billing or proof that
a commit reached `main`.

## What you get

| Need | agenttrace gives you |
| --- | --- |
| Historical spend review | Sessions grouped across projects, agents, and models with Today/7d/30d/All ranges |
| Data confidence | Report scope, per-source coverage, parse skips, cache hits, unknown sources/models, pricing fallbacks, and latest observed session |
| Cost audit and action plan | Token component rates, pricing source/status, estimated cost confidence, and prioritized, evidence-backed remediation suggestions |
| Governance trends | Canonical project grouping, observed MCP invocation governance, cross-session context/cache/read-write trends, and read-only Git delivery correlation |
| Honest capability levels | `Detailed`, `Aggregate`, or `Limited` per session so missing event-level evidence is never presented as a complete trace |
| Privacy-safe steps | Tool-step metadata and duration when the source provides call IDs and timestamps; no prompt, response, result, or tool-argument body is stored in steps |
| Slow-task diagnosis | Latency stats, long gaps, hanging sessions, retry loops, slow tools, large params, and context pressure |
| Regression evidence | Local baseline comparison when supplied, incident timelines, and conservative tool authority categories in reports |
| First-session triage | Sort and filter by cost, duration, health, failures, anomalies, model, source, or text search |
| Shareable evidence | JSON, Markdown, and self-contained HTML reports |
| Local-first inspection | No hosted backend required |

## Docs

- Documentation index: [docs/README.md](docs/README.md)
- CI setup: [docs/guides/ci-integration.md](docs/guides/ci-integration.md)
- Governance reports: [docs/guides/governance-reports.md](docs/guides/governance-reports.md)
- Cursor import: [docs/guides/cursor-import.md](docs/guides/cursor-import.md)
- Parser guide: [docs/guides/parser-guide.md](docs/guides/parser-guide.md)
- Maintainer distribution guide: [docs/maintainers/distribution.md](docs/maintainers/distribution.md)

Listed in these open source projects:

- [awesome-mac](https://github.com/jaywcjlove/awesome-mac)
- [antigravity-awesome-skills](https://github.com/sickn33/antigravity-awesome-skills)
- [awesome-claude-skills](https://github.com/BehiSecc/awesome-claude-skills)

## Contributing

Parser PRs are welcome. A good parser contribution usually includes:

- a tiny redacted fixture or synthetic sample
- format detection in `crates/agenttrace-core/src/parser.rs`
- role, timestamp, model, token usage, tool call, and tool error extraction
- tests for successful parsing and malformed input

Run before sending a PR:

```bash
cargo test
cargo build --release -p agenttrace
target/release/agenttrace --doctor
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full contribution flow.

## License

[MIT](LICENSE) © 2026 agenttrace contributors
