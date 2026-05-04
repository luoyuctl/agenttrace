# Demo Playbook

Use this when sharing agenttrace on GitHub, Hacker News, Reddit, V2EX, X, or Product Hunt.

## Record the GIF

```bash
scripts/record-demo.sh
```

The script renders [docs/demo.tape](demo.tape) into `assets/agenttrace-demo.gif` with [VHS](https://github.com/charmbracelet/vhs). VHS also needs `ttyd` available on `PATH`.

## Storyline

1. Start with `agenttrace --demo` so viewers do not need local logs.
2. Press `!` to jump from overview to critical sessions.
3. Press `Enter` to open the selected session detail.
4. Press `w` to jump to diagnostics for loop/tool/context evidence.
5. End on Overview so the GIF loops back to the dashboard.

## Short Caption

agenttrace is a local TUI for AI coding agent session history. It shows what Claude Code, Codex CLI, Gemini CLI, Aider, Cursor exports, and similar tools spent across cost, tokens, and time, then helps diagnose why a task was slow.

## Verification Before Posting

```bash
go test ./...
go build -o /tmp/agenttrace ./cmd/agenttrace
/tmp/agenttrace --demo --overview -f json
```
