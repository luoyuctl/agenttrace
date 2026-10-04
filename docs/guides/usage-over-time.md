# Usage over time

AgentTrace can roll local usage up by calendar period or into 5-hour blocks. Everything runs locally; all cost values are estimates.

## Daily, weekly, and monthly

```bash
agenttrace --daily --range 30d
agenttrace --weekly --tz utc -f json
agenttrace --monthly --tz +08:00
```

- Each usage record is bucketed by its own timestamp, so a session that crosses midnight is split across days.
- Weeks are ISO weeks and are labelled by their Monday.
- `--tz` accepts `local` (default), `utc`, or a fixed offset such as `+08:00`, `-0530`, or `+8`. IANA names such as `Asia/Shanghai` are not supported.
- `--limit` (default 20) caps the rows in text output and the buckets/blocks in JSON, newest first; the text `TOTAL` row always covers the full range. Pass a larger `--limit` (e.g. `--limit 1000`) to get every bucket.
- Sessions without per-turn usage (text estimates, aggregate SQLite sources) are counted once at their start time.

## 5-hour blocks

```bash
agenttrace --blocks
agenttrace --blocks --token-limit 50000000 --cost-limit 100 -f json
```

- A block starts at the top of the hour of its first record and lasts five hours. The next record after the block ends, or after a five-hour gap, opens a new block.
- `TOK/MIN` and `COST/HR` are measured from the block start (to now for the active block), with a 15-minute minimum window so a single early record does not inflate the rate.
- For the active block, the report projects tokens and cost to the block end at the current rate.
- `--token-limit` and `--cost-limit` add used and projected percentages of your own per-block budget.

Blocks are estimated from local logs. Provider rate limits are enforced server-side, can depend on factors that are not in local logs, and may differ from these numbers. JSON output marks this with `"estimated": true`.
