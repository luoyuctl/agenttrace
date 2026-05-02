# agenttrace

TUI observability for AI coding agent sessions. It helps you inspect local agent runs across cost, token usage, latency, tool failures, anomalies, health score, and CI quality gates.

This npm package is a thin installer. On `postinstall`, it downloads the platform-specific `agenttrace` binary from the latest GitHub release.

## Install

```bash
npm install -g agenttrace
agenttrace --version
```

Other supported installs:

```bash
brew install luoyuctl/tap/agenttrace
curl -fsSL https://raw.githubusercontent.com/luoyuctl/agenttrace/master/install.sh | sh
```

## Usage

```bash
# Open the TUI
agenttrace

# Try built-in sample sessions
agenttrace --demo

# Diagnose local session discovery and cache status
agenttrace --doctor

# JSON overview for automation
agenttrace --overview -f json

# CI health gate
agenttrace --overview --fail-under-health 80 --fail-on-critical --max-tool-fail-rate 15
```

## Supported Sources

agenttrace auto-detects local sessions from Claude Code, Codex CLI, Gemini CLI, OpenCode, OpenClaw, Copilot CLI, Kimi CLI, Hermes Agent, and Aider chat history.

For Aider repositories, run:

```bash
agenttrace -d /path/to/repo
```

The parser looks for `.aider.chat.history.md`.

## Links

- GitHub: https://github.com/luoyuctl/agenttrace
- Releases: https://github.com/luoyuctl/agenttrace/releases
- CI integration: https://github.com/luoyuctl/agenttrace/blob/master/docs/ci-integration.md
