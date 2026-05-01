# agenttrace 🔍

**AI Agent Session Performance Analyzer** — find hanging, token waste, thinking redaction, and quality regressions in your AI coding sessions.

```
$ agenttrace --latest
============================================================
  AGENTTRACE v2 — AI Agent Session Performance Report
============================================================

💰 TOKEN COST
  Input:         1,342 tokens    Output:        3,947 tokens
  Est. cost:     $0.0632

📊 ACTIVITY:  2 msgs | 42 turns | 70 tool calls | 91% success
⏱️  LATENCY: p50=457.9s | p95=457.9s | max=457.9s
🔧 TOP TOOLS: browser_navigate=31, terminal=14, todo=9
🧠 THINKING: 20 blocks | avg=392 chars | ⚠️ shallow detected
🚨 ANOMALIES: 🟡 shallow thinking, 🔴 high tool failures
💯 HEALTH: 🟢 90/100
```

## Why?

AI coding agents (Claude Code, Gemini CLI, Codex CLI, Hermes Agent) produce session logs — but nobody analyzes them systematically. Developers waste tokens, miss quality regressions, and can't compare tools objectively.

**agenttrace** gives you the dashboard your AI agent needs.

## Features

- 🔍 **Multi-format** — Hermes Agent, Claude Code, Gemini CLI (auto-detect)
- 💰 **Token cost estimation** — 7 models priced (Opus $75/M output → Flash $0.60/M)
- 🚨 **4 anomaly types** — hanging, high failure rate, shallow thinking, redaction
- 📊 **Multi-session comparison** — compare across sessions/tools
- 💯 **Health score** — 0-100 composite score
- 🏃 **Zero dependencies** — pure Python 3.9+, no pip install needed
- 🤖 **Machine-readable** — JSON output for CI/CD integration

## Quick Start

```bash
# Clone and run
git clone https://github.com/user/agenttrace.git
cd agenttrace
python3 agenttrace.py --latest

# Or with model pricing
python3 agenttrace.py -m claude-sonnet-4 session.jsonl
python3 agenttrace.py --compare
python3 agenttrace.py -f json session.jsonl  # JSON output
```

## Supported Formats

| Format | Detection | Token Cost | Anomalies |
|---|---|---|---|
| Hermes Agent (JSONL) | ✅ auto | ✅ estimated | ✅ |
| Claude Code (Messages API) | ✅ auto | ✅ from metadata | ✅ |
| Gemini CLI | ✅ auto | ✅ estimated | ✅ |

## Anomaly Types

| Type | Trigger | Severity |
|---|---|---|
| Hanging | Event gaps > 60s | 🔴/🟡 |
| Tool Failures | Failure rate > 20% | 🔴 |
| Shallow Thinking | Avg < 500 chars | 🟡 |
| Thinking Redaction | Redacted blocks found | 🟡 |

## Roadmap

- [ ] Web dashboard
- [ ] GitHub Action for CI integration
- [ ] Historical trends
- [ ] Cost forecasting
- [ ] VS Code extension

## License

MIT
