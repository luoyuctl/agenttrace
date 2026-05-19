# agenttrace roadmap

agenttrace is focused on two jobs:

1. Review AI coding agent history across cost, tokens, elapsed time, and tool authority.
2. Diagnose why an agent task ran slowly or regressed.

This roadmap keeps the project pointed at local post-run evidence instead of
becoming a generic observability dashboard.

## Strategic directions

- Broaden reliable local parser coverage for active coding-agent session formats.
- Keep local-first privacy and public asset hygiene explicit in screenshots,
  reports, and launch materials.
- Make first-screen TUI triage faster for cost, tokens, tool failures, latency,
  anomalies, health, and incident evidence.
- Strengthen shareable evidence through terminal text, JSON, Markdown, and HTML
  reports, local baseline comparison, and repeatable CI gates.
- Keep tool authority categories conservative, deterministic, and clearly framed
  as report evidence rather than policy enforcement.
- Keep doctor, install paths, release artifacts, Homebrew, and site surfaces
  consistent with the current project state.
- Improve discoverability around local coding-agent session observability,
  distinct from generic tracing SDKs.
- Route ecosystem feedback into parser, quality, growth, product, or radar lanes
  with clear ownership.
- Track adjacent surfaces such as local session search, multi-agent dashboards,
  token attribution, persistent memory, and upstream log fidelity before
  committing implementation.

## Non-goals

Non-goals: hosted prompt storage, billing-grade invoice reconciliation, replacing
agent chat UIs, live tracing while a model is streaming, security enforcement,
release promises, package-publish promises, and internal platform targets.
