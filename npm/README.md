# agenttrace

TUI observability for AI coding agent sessions. It helps you inspect local agent runs across cost, token usage, latency, tool failures, anomalies, health score, and CI quality gates.

This directory contains the npm wrapper for agenttrace. The public npm package has not been published yet, so `npm install -g agenttrace` will return a registry 404 until the first publish.

## Install

Use one of the supported install methods for now:

```bash
brew install luoyuctl/tap/agenttrace
agenttrace --version
```

```bash
curl -fsSL https://raw.githubusercontent.com/luoyuctl/agenttrace/master/install.sh | sh
agenttrace --version
```

After the package is published, the npm install path will be:

```bash
npm install -g agenttrace
agenttrace --version
```

## Maintainer Checks

From this directory:

```bash
node --check install.js
node --check run.js
npm pack --dry-run
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
