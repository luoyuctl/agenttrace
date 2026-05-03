# agenttrace Launch Kit

agenttrace is a terminal observability dashboard for AI coding agent sessions. It helps developers see where Claude Code, Codex CLI, Gemini CLI, Qwen Code, Aider, Cursor exports, Hermes Agent, OpenCode, OpenClaw, Kimi CLI, and Copilot-style logs waste time, tokens, and tool calls.

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

It parses logs from Claude Code, Codex CLI, Gemini CLI, Qwen Code, Aider, Cursor exports, Hermes Agent, OpenCode, OpenClaw, Kimi CLI, and Copilot-style traces, then shows:

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

It shows token cost, latency, tool failures, anomalies, health score, details, diffs, JSON output, and CI health gates across Claude Code, Codex CLI, Gemini CLI, Qwen Code, Aider, Cursor exports, Hermes, OpenCode, Kimi, and more.

https://github.com/luoyuctl/agenttrace

**Reddit / V2EX**

I made a TUI tool for people using AI coding agents daily. It scans local session logs and shows where agents waste time or money: hanging gaps, tool failures, retry loops, shallow reasoning, token/cost burn, and session health.

The goal is not another chat UI. It is closer to `htop`/`lazygit` for AI agent runs: fast local inspection, filtering, diagnostics, and exportable JSON.

Would love feedback from anyone using Claude Code, Codex CLI, Gemini CLI, Qwen Code, Aider, Cursor, Hermes Agent, OpenCode, Kimi CLI, or similar tools.

Repo: https://github.com/luoyuctl/agenttrace

Feedback thread: https://github.com/luoyuctl/agenttrace/discussions/2
Sample report: https://luoyuctl.github.io/agenttrace/demo-report.html

## Channel-Specific Drafts

Use these as starting points. Edit for the community, then post manually.

**Hacker News**

Title:

```text
Show HN: agenttrace, local TUI observability for AI coding agent sessions
```

Body:

```text
I built agenttrace to answer a question I kept running into with coding agents: what actually happened during the run?

It scans local session logs and turns them into a terminal dashboard for token/cost usage, tool failures, latency gaps, anomalies, session health, diffs, and JSON/Markdown/HTML reports. It is local-first, so prompts and raw logs do not need to leave your machine.

It supports Claude Code, Codex CLI, Gemini CLI, Qwen Code, Aider, Cursor exports, Hermes, OpenCode, Kimi, and Copilot-style logs.

Try it without local logs:

agenttrace --demo

Repo: https://github.com/luoyuctl/agenttrace
Sample report: https://luoyuctl.github.io/agenttrace/demo-report.html
```

**Reddit**

```text
I made a local TUI for debugging AI coding agent sessions.

The problem it tries to solve: when an agent hangs, retries tools, burns a lot of tokens, or produces a weird final answer, the final output does not show the operational story.

agenttrace scans local session logs and shows token/cost burn, tool success/failure rate, latency gaps, anomalies, session health, diffs, and exportable JSON/Markdown/HTML reports. It also has CI health gates if you want to track agent runs in PRs or nightly jobs.

It is not a hosted tracing backend and does not require uploading prompts or raw logs.

Repo: https://github.com/luoyuctl/agenttrace
Demo: agenttrace --demo
```

**V2EX**

```text
做了一个本地优先的 AI coding agent 会话观测 TUI：agenttrace。

它不是聊天客户端，也不是云端 tracing 服务。主要解决一个问题：agent 跑完之后，只看最终输出很难知道中间有没有卡住、重复调用工具、消耗大量 token、或者健康度明显下降。

目前可以扫本地 Claude Code、Codex CLI、Gemini CLI、Qwen Code、Aider、Cursor 导出、Hermes、OpenCode、Kimi 等会话日志，展示 token/cost、tool failure、latency gap、anomaly、health score，也能导出 JSON/Markdown/HTML 给 CI 或 PR 用。

没本地日志也可以先跑 demo：

agenttrace --demo

Repo: https://github.com/luoyuctl/agenttrace
```

**X / Threads**

```text
AI coding agents need local observability too.

agenttrace scans local session logs for token/cost burn, tool failures, latency gaps, anomalies, health, reports, and CI gates.

Try: agenttrace --demo

https://github.com/luoyuctl/agenttrace
```

## Target Channels

- Hacker News: Show HN
- Reddit: r/commandline, r/golang, r/LocalLLaMA, r/ClaudeAI, r/ChatGPTCoding
- V2EX: 分享创造 / 程序员
- X / Threads: AI engineering and developer tooling
- GitHub topics: `ai-agents`, `tui`, `observability`, `developer-tools`, `cost-tracking`, `aider`, `claude-code`, `codex-cli`
- Product Hunt: after a GIF demo and first external feedback

## Directory Submissions

Open PRs:

- awesome-codex-cli: https://github.com/RoggeOhta/awesome-codex-cli/pull/23
- awesome-ai-coding-tools: https://github.com/ai-for-developers/awesome-ai-coding-tools/pull/288
- awesome-vibe-coding: https://github.com/ai-for-developers/awesome-vibe-coding/pull/56
- awesome-ai-coding: https://github.com/wsxiaoys/awesome-ai-coding/pull/97
- filipecalegario/awesome-vibe-coding: https://github.com/filipecalegario/awesome-vibe-coding/pull/171
- awesome-coding-ai: https://github.com/ohong/awesome-coding-ai/pull/6
- awesome-claude-code-toolkit: https://github.com/rohitg00/awesome-claude-code-toolkit/pull/361
- jqueryscript/awesome-claude-code: https://github.com/jqueryscript/awesome-claude-code/pull/252
- awesome-go-cli: https://github.com/mantcz/awesome-go-cli/pull/4
- awesome-observability: https://github.com/adriannovegil/awesome-observability/pull/63
- awesome-agents: https://github.com/kyrolabs/awesome-agents/pull/437
- awesome-ai-devtools: https://github.com/jamesmurdza/awesome-ai-devtools/pull/492
- awesome_ai_agents: https://github.com/jim-schwoebel/awesome_ai_agents/pull/250
- awesome-ai-agents-2026: https://github.com/caramaschiHG/awesome-ai-agents-2026/pull/207
- Awesome-AI-Agents: https://github.com/Jenqyang/Awesome-AI-Agents/pull/204
- Awesome-LLMOps: https://github.com/InftyAI/Awesome-LLMOps/pull/420
- awesome-ai: https://github.com/hemanthgk10/awesome-ai/pull/7
- awesome-terminals-ai: https://github.com/BNLNPPS/awesome-terminals-ai/pull/6
- awesome-llmops: https://github.com/KennethanCeyer/awesome-llmops/pull/10
- awesome-harness-engineering: https://github.com/ai-boost/awesome-harness-engineering/pull/14
- Scottcjn/awesome-agents: https://github.com/Scottcjn/awesome-agents/pull/12
- awesome-cli-apps-in-a-csv: https://github.com/toolleeo/awesome-cli-apps-in-a-csv/pull/255
- awesome-cli-apps-in-a-csv follow-up: https://github.com/toolleeo/awesome-cli-apps-in-a-csv/pull/256
- awesome-agent-clis: https://github.com/ComposioHQ/awesome-agent-clis/pull/8
- awesome-code-agents: https://github.com/sorrycc/awesome-code-agents/pull/20
- awesome-code-agents follow-up: https://github.com/sorrycc/awesome-code-agents/pull/22
- tensorchord/Awesome-LLMOps: https://github.com/tensorchord/Awesome-LLMOps/pull/444
- awesome-agent-cortex: https://github.com/0xNyk/awesome-agent-cortex/pull/20
- ARUNAGIRINATHAN-K/awesome-ai-agents: https://github.com/ARUNAGIRINATHAN-K/awesome-ai-agents/pull/27
- awesome-cli-tui-software: https://github.com/lgaggini/awesome-cli-tui-software/pull/3
- awesome-tuis follow-up: https://github.com/rothgar/awesome-tuis/pull/659
- LangGPT/awesome-claude-code: https://github.com/LangGPT/awesome-claude-code/pull/58
- awesome-agent-harness: https://github.com/Picrew/awesome-agent-harness/pull/5
- awesome-AI-driven-development: https://github.com/eltociear/awesome-AI-driven-development/pull/48
- command-line-tools: https://github.com/linsa-io/command-line-tools/pull/35
- awesome-cli-coding-agents: https://github.com/bradAGI/awesome-cli-coding-agents/pull/73
- awesome-opencode: https://github.com/awesome-opencode/awesome-opencode/pull/334
- awesome-opensource-ai: https://github.com/alvinreal/awesome-opensource-ai/pull/418
- awesome-llm-skills: https://github.com/Prat011/awesome-llm-skills/pull/116
- awesome-codex-plugins: https://github.com/hashgraph-online/awesome-codex-plugins/pull/65
- awesome-copilot-agents: https://github.com/Code-and-Sorts/awesome-copilot-agents/pull/53
- github/awesome-copilot: https://github.com/github/awesome-copilot/pull/1595
- awesome-agent-skills: https://github.com/heilcheng/awesome-agent-skills/pull/216
- awesome-ai-eval: https://github.com/Vvkmnn/awesome-ai-eval/pull/10
- awesome-ai-dev-tools: https://github.com/PierrunoYT/awesome-ai-dev-tools/pull/20
- awesome-devtools: https://github.com/devtoolsd/awesome-devtools/pull/213
- awesome-ai-sdks: https://github.com/e2b-dev/awesome-ai-sdks/pull/175
- awesome-agents follow-up: https://github.com/kyrolabs/awesome-agents/pull/440
- awesome_ai_agents follow-up: https://github.com/jim-schwoebel/awesome_ai_agents/pull/254
- awesome-ai-devtools follow-up: https://github.com/jamesmurdza/awesome-ai-devtools/pull/495
- awesome-observability follow-up: https://github.com/adriannovegil/awesome-observability/pull/64
- awesome-utils-dev: https://github.com/pegaltier/awesome-utils-dev/pull/29

Merged listings:

- awesome-ChatGPT-repositories: https://github.com/taishi-i/awesome-ChatGPT-repositories/pull/130
- GetBindu/awesome-claude-code-and-skills: https://github.com/GetBindu/awesome-claude-code-and-skills/pull/21
- awesome-gemini-cli: https://github.com/Piebald-AI/awesome-gemini-cli/pull/47
- awesome-mac: https://github.com/jaywcjlove/awesome-mac/pull/2026
- awesome-skills: https://github.com/gmh5225/awesome-skills/pull/14
- charm-in-the-wild: https://github.com/charm-and-friends/charm-in-the-wild/pull/88

Manual-only submission:

- hesreallyhim/awesome-claude-code: submit via the GitHub issue form, because the repo asks contributors not to create automated issues or PRs. Suggested category: Tooling / Usage Monitors.
- e2b-dev/awesome-ai-agents: submit through the Google Form linked from the README; the repo asks for product submissions through the form instead of direct README edits.
- awesome-claude-skills: skip automated PRs unless submitted manually by a human; its contribution guide asks that PRs are not AI-assisted and generally expects social proof.
- awesome-go: defer until the project is older and has the required quality links; contribution checks expect repository maturity, pkg.go.dev, Go Report Card, and coverage evidence.
- awesome-cli-apps: PR https://github.com/agarrharr/awesome-cli-apps/pull/1032 was closed without maintainer feedback. Revisit after more external adoption or a clearer category fit.
- awesome-tuis: likely blocked until the repo is at least 6 months old; its PR template requires repos to be at least 6 months old, PR #658 was closed after reviewer feedback, and follow-up PR #659 is open.
- Terminal Trove: submit through https://terminaltrove.com/post/ after confirming the author contact email. Suggested categories: `macos`, `linux`, `windows`, `monitoring`, `observability`, `tui`, `json`, `ai`, `cli`, `debugging`, `cross-platform`. Preview PNG: `https://luoyuctl.github.io/agenttrace/assets/tui-preview.png`; GIF: `https://luoyuctl.github.io/agenttrace/assets/agenttrace-demo.gif`.
- Terminal Apps: submitted suggestion issue https://github.com/scmmishra/terminal-apps.dev/issues/55. Name: `agenttrace`; GitHub URL: `https://github.com/luoyuctl/agenttrace`.
- awesome-ai-coding-techniques: submitted technique suggestion https://github.com/inmve/awesome-ai-coding-techniques/issues/37. Suggested technique: inspect AI agent session traces after a run.
- InftyAI/Awesome-LLMOps: closed duplicate PR https://github.com/InftyAI/Awesome-LLMOps/pull/418 in favor of workflow-generated PR https://github.com/InftyAI/Awesome-LLMOps/pull/420.

Terminal Trove draft:

- Name: `agenttrace`
- URL: `github.com/luoyuctl/agenttrace`
- Tagline: `Local-first TUI observability for AI coding agent sessions.`
- Description: `agenttrace parses local Claude Code, Codex CLI, Gemini CLI, Qwen Code, Aider, Cursor export, Hermes, OpenCode, Kimi, and Copilot-style logs into a fast terminal dashboard for session health, cost, token usage, latency, tool failures, anomalies, diffs, and CI evidence.`
- Standout features: `Overview, session list, detail, diagnostics, and diff views; incremental local cache; JSON, Markdown, and self-contained HTML reports with CI gates for health and tool failure rate.`
- Who it is for: `Developers using AI coding agents who need to find expensive, stuck, slow, or low-quality sessions without uploading private logs to a hosted observability service.`
- Primary language: `go`
- License: `mit`
- Install:
  - macOS/Linux: `curl -sL https://raw.githubusercontent.com/luoyuctl/agenttrace/master/install.sh | sh`
  - Homebrew: `brew install luoyuctl/tap/agenttrace`
  - Go install: `go install github.com/luoyuctl/agenttrace/cmd/agenttrace@latest`
  - Windows PowerShell: `iwr -useb https://raw.githubusercontent.com/luoyuctl/agenttrace/master/install.ps1 | iex`

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
