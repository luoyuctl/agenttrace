# Demo Playbook

Use this when sharing agenttrace on GitHub, Hacker News, Reddit, V2EX, X, or Product Hunt.

## Record the GIF

```bash
scripts/record-demo.sh
```

The script renders [docs/demo.tape](demo.tape) into `assets/agenttrace-demo.gif` with [VHS](https://github.com/charmbracelet/vhs).

## Storyline

1. Start with `agenttrace --demo` so viewers do not need local logs.
2. Open Session List and show sorting/filtering.
3. Run `:health <80` to show command mode.
4. Open Detail to show the primary issue and cost/health summary.
5. Jump to Diagnostics for loop/tool/context evidence.
6. End on Diff or Overview to show this is more than a log viewer.

## Short Caption

agenttrace is a local TUI observability dashboard for AI coding agents. It shows where Claude Code, Codex CLI, Gemini CLI, Aider, Cursor exports, and similar tools waste tokens, time, and tool calls.

## Verification Before Posting

```bash
go test ./...
go build -o /tmp/agenttrace ./cmd/agenttrace
/tmp/agenttrace --demo --overview -f json
```
