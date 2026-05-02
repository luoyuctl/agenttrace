# Community Pain Points

These are the product angles agenttrace should keep optimizing for.

## Repeated Themes

- Agent runs need step-level visibility, not just final output.
- Token cost is hard to control without seeing prompts, cache usage, retries, and context bloat.
- Tool latency and tool failures are often the real bottleneck behind a slow agent.
- Retry loops and format loops quietly burn time and money.
- Teams want CI gates and regression checks, not only passive dashboards.
- Developers using several coding agents need comparable local metrics across tools.
- TUI users expect token/cost/status information to stay visible while navigating, not hidden behind a report-only command.
- Power users type command filters naturally (`cost > 0.5`, `health >= 80`) and expect long inputs to never corrupt terminal layout.

## Product Implications

- Keep startup fast and local-first.
- Keep parser coverage broad: Claude Code, Codex CLI, Gemini CLI, Aider, Cursor exports, Hermes, OpenCode, OpenClaw, Kimi, Copilot-style logs.
- Make the first TUI screen answer: cost, health, errors, latency, and recent bad sessions.
- Keep JSON output and CI gates first-class.
- Prioritize diagnostics that produce an action, not just a chart.
- Treat command mode and filtering as first-class TUI workflows: permissive parsing, visible feedback, clear reset behavior, and width-safe rendering.
- Borrow the best open-source TUI pattern: dashboard first, detail drill-down second, persistent status context throughout.

## Source Notes

- TrueFoundry highlights step-level traces, tool latency, and whether a delay comes from LLM thinking or slow tools: https://www.truefoundry.com/blog/ai-agent-observability-tools
- Galileo frames cost debugging around token counters, decision paths, tool call frequency, context size, latency, and retry loops: https://www.galileo.ai/blog/ai-agent-cost-optimization-observability
- Braintrust separates monitoring, observability, and debugging, and emphasizes CI/CD quality gates and regression prevention: https://www.braintrust.dev/articles/best-ai-agent-debugging-tools-2026
- LangChain notes that agent failures often look healthy in traditional metrics while output quality still regresses: https://www.langchain.com/articles/ai-observability
- Cursor community threads point to local workspace SQLite history as the practical source for chat/composer exports: https://forum.cursor.com/t/chat-history-folder/7653
- OpenCode users explicitly ask for token usage in the TUI status bar, `/usage` commands, and toggleable detail views: https://github.com/anomalyco/opencode/issues/13003
- Claude Code users ask for a richer TUI with context/token sidebars and cleaner layouts inspired by OpenCode: https://github.com/anthropics/claude-code/issues/52518
- claude-monitor positions dashboard, detail, agent tree, analytics, task board, history, and Codex monitoring as separate TUI views: https://github.com/fakirAyoub/claude-monitor
- CodeBurn validates the multi-agent cost-observability angle across Claude Code, Codex, Cursor, Gemini, OpenCode, Kiro, Roo, KiloCode, Qwen, and related tools: https://github.com/getagentseal/codeburn
- Microsoft's Project Telescope reinforces local-first agent observability across sessions, turns, tools, shell, files, cost, retries, context, and human-in-the-loop events: https://github.com/microsoft/project-telescope
