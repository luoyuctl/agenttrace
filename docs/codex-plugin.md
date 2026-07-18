# Codex Plugin Session Audit

agenttrace includes a Codex plugin manifest and skill so Codex can use the installed `agenttrace` binary as a local session audit tool.

Use it when a Codex workflow needs quick evidence for questions that raw agent output usually hides:

- Which local sessions used the most tokens or estimated cost?
- Did tool failures, retries, or long gaps make a run unhealthy?
- Is there JSON, Markdown, or HTML evidence worth attaching to a PR, issue, or CI artifact?
- Does this session have Detailed evidence, or only Aggregate/Limited metrics?

Plugin files:

- `.codex-plugin/plugin.json`
- `skills/agenttrace-session-audit/SKILL.md`

The same audit path also works directly from a terminal:

```bash
agenttrace --doctor
agenttrace --overview -f json
agenttrace --overview --fail-under-health 80 --fail-on-critical --max-tool-fail-rate 15
```

This keeps the workflow local-first: session files stay on the developer machine unless the user explicitly exports and shares a report.
The plugin must not present Aggregate or Limited sessions as complete traces;
Tool Steps contain metadata only and exclude conversation and tool payload bodies.
