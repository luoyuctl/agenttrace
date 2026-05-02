# agenttrace Launch Kit

agenttrace is a terminal observability dashboard for AI coding agent sessions. It helps developers see where Claude Code, Codex CLI, Gemini CLI, Aider, Cursor exports, Hermes Agent, OpenCode, OpenClaw, Kimi CLI, and Copilot-style logs waste time, tokens, and tool calls.

## Positioning

**One-liner**

TUI observability for AI coding agents: trace sessions, cost, tokens, tool failures, latency, anomalies, and health in one fast terminal dashboard.

**Problem**

AI coding agents behave like tiny build systems: they plan, call tools, retry, hang, and spend money. Most teams only see the final output, not the session health, token burn, tool failure rate, or whether the agent got stuck.

**Why now**

Agent usage is moving from experiments to daily engineering workflows. Developers need the same kind of local visibility they expect from build tools, test runners, and production telemetry.

## Launch Post

Title ideas:

- Show HN: agenttrace, a TUI observability dashboard for AI coding agents
- agenttrace: find where AI coding agents waste tokens, time, and tool calls
- I built a terminal dashboard for debugging AI coding agent sessions

Body:

I built agenttrace, a single-binary TUI for inspecting AI coding agent sessions locally.

It parses logs from Claude Code, Codex CLI, Gemini CLI, Aider, Cursor exports, Hermes Agent, OpenCode, OpenClaw, Kimi CLI, and Copilot-style traces, then shows:

- token and cost burn
- tool success/failure rate
- latency and hanging gaps
- anomaly detection
- per-session health score
- detail diagnostics and session diffs
- JSON output for dashboards
- CI health gates for average health, critical sessions, and tool failure rate

Install:

```bash
curl -sL https://raw.githubusercontent.com/luoyuctl/agenttrace/master/install.sh | sh
agenttrace
```

Or:

```bash
brew install luoyuctl/tap/agenttrace
```

No local agent logs yet?

```bash
agenttrace --demo
```

The pain point: when an agent gets stuck, retries a tool loop, or silently burns context, the output alone does not tell you what happened. agenttrace gives a quick local view before you dig through raw JSONL logs.

Repo: https://github.com/luoyuctl/agenttrace
Sample HTML report: https://luoyuctl.github.io/agenttrace/demo-report.html

## Short Posts

**X / Threads**

AI coding agents now need observability too.

I built agenttrace: a fast terminal dashboard for local agent sessions.

It shows token cost, latency, tool failures, anomalies, health score, details, diffs, JSON output, and CI health gates across Claude Code, Codex CLI, Gemini CLI, Aider, Cursor exports, Hermes, OpenCode, Kimi, and more.

https://github.com/luoyuctl/agenttrace

**Reddit / V2EX**

I made a TUI tool for people using AI coding agents daily. It scans local session logs and shows where agents waste time or money: hanging gaps, tool failures, retry loops, shallow reasoning, token/cost burn, and session health.

The goal is not another chat UI. It is closer to `htop`/`lazygit` for AI agent runs: fast local inspection, filtering, diagnostics, and exportable JSON.

Would love feedback from anyone using Claude Code, Codex CLI, Gemini CLI, Aider, Cursor, Hermes Agent, OpenCode, Kimi CLI, or similar tools.

Repo: https://github.com/luoyuctl/agenttrace

Feedback thread: https://github.com/luoyuctl/agenttrace/discussions/2
Sample report: https://luoyuctl.github.io/agenttrace/demo-report.html

## Target Channels

- Hacker News: Show HN
- Reddit: r/commandline, r/golang, r/LocalLLaMA, r/ClaudeAI, r/ChatGPTCoding
- V2EX: 分享创造 / 程序员
- X / Threads: AI engineering and developer tooling
- GitHub topics: `ai-agents`, `tui`, `observability`, `developer-tools`, `cost-tracking`, `aider`, `claude-code`, `codex-cli`
- Product Hunt: after a GIF demo and first external feedback

## Directory Submissions

Open PRs:

- awesome-tuis: https://github.com/rothgar/awesome-tuis/pull/658
- awesome-codex-cli: https://github.com/RoggeOhta/awesome-codex-cli/pull/23
- awesome-ai-coding-tools: https://github.com/ai-for-developers/awesome-ai-coding-tools/pull/288
- awesome-claude-code-toolkit: https://github.com/rohitg00/awesome-claude-code-toolkit/pull/361

Manual-only submission:

- awesome-claude-code: submit via the GitHub issue form, because the repo asks contributors not to create automated issues or PRs. Suggested category: Tooling / Usage Monitors.

## Demo Checklist

- Render `assets/agenttrace-demo.gif` with `scripts/record-demo.sh` when VHS is available.
- First screen should show the Overview dashboard.
- Include Session List filtering and command mode.
- Show Detail with primary issue and scroll percentage.
- Show Diagnostics for hanging/tool failures/context usage.
- Show Diff between two sessions.
- End with `agenttrace --overview -f json`.
- Show CI gate output with `agenttrace --overview --fail-under-health 80 --fail-on-critical`.
- For a reproducible recording, use `agenttrace --demo`.

See [demo-playbook.md](demo-playbook.md) for the recording script and storyline.

## Verification Before Sharing

```bash
go test ./...
go build -o /tmp/agenttrace ./cmd/agenttrace
/tmp/agenttrace --version
/tmp/agenttrace --demo --overview -f json
```

Install smoke:

```bash
tmp_home=$(mktemp -d)
AGENTTRACE_INSTALL_DIR="$tmp_home/bin" HOME="$tmp_home" sh install.sh
"$tmp_home/bin/agenttrace" --version
rm -rf "$tmp_home"
```
