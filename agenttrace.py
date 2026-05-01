#!/usr/bin/env python3
"""
agenttrace v3 — Multi-format AI Agent Session Performance Analyzer.
Supports: Hermes Agent, Claude Code, Gemini CLI, generic JSONL.
Features: token cost, anomaly detection, health score, multi-session comparison.

Usage:
    python3 agenttrace.py --latest          # analyze latest session
    python3 agenttrace.py session.jsonl     # analyze a specific session
    python3 agenttrace.py --compare         # compare all sessions
    python3 agenttrace.py -f json --latest # JSON output
"""

import json
import os
import sys
import argparse
from datetime import datetime, timezone, timedelta
from collections import Counter, defaultdict
from pathlib import Path

__version__ = "3.0.0"

# ═══════════════════════════════════════════════════════════════
# PRICING — USD per 1M tokens (input, output, cache_write, cache_read)
# ═══════════════════════════════════════════════════════════════

PRICING = {
    "claude-opus-4":       {"input": 15.00, "output": 75.00, "cw": 18.75, "cr": 1.50},
    "claude-opus-4.5":     {"input": 15.00, "output": 75.00, "cw": 18.75, "cr": 1.50},
    "claude-sonnet-4":     {"input":  3.00, "output": 15.00, "cw":  3.75, "cr": 0.30},
    "claude-sonnet-4.5":   {"input":  3.00, "output": 15.00, "cw":  3.75, "cr": 0.30},
    "claude-haiku-3.5":    {"input":  0.80, "output":  4.00, "cw":  1.00, "cr": 0.08},
    "claude-haiku-4":      {"input":  0.80, "output":  4.00, "cw":  1.00, "cr": 0.08},
    "gemini-2.5-pro":      {"input":  1.25, "output": 10.00, "cw":  0.00, "cr": 0.00},
    "gemini-2.5-flash":    {"input":  0.15, "output":  0.60, "cw":  0.00, "cr": 0.00},
    "gpt-4.1":             {"input":  2.00, "output":  8.00, "cw":  0.00, "cr": 0.00},
    "gpt-4.1-mini":        {"input":  0.40, "output":  1.60, "cw":  0.00, "cr": 0.00},
    "gpt-4.1-nano":        {"input":  0.10, "output":  0.40, "cw":  0.00, "cr": 0.00},
    "deepseek-v3":         {"input":  0.27, "output":  1.10, "cw":  0.07, "cr": 0.014},
    "deepseek-r1":         {"input":  0.55, "output":  2.19, "cw":  0.14, "cr": 0.028},
    "default":             {"input":  3.00, "output": 15.00, "cw":  0.00, "cr": 0.00},
}

# ═══════════════════════════════════════════════════════════════
# UTILITY
# ═══════════════════════════════════════════════════════════════

def _parse_ts(raw):
    """Parse timestamp from string or None. Handles ISO8601 with/without tz."""
    if not raw:
        return None
    try:
        s = str(raw).replace("Z", "+00:00")
        dt = datetime.fromisoformat(s)
        if dt.tzinfo is None:
            dt = dt.replace(tzinfo=timezone.utc)
        return dt
    except (ValueError, TypeError):
        return None


def _safe_calc(lst, fn):
    """Safely apply fn to a non-empty list, return 0 on empty."""
    return fn(lst) if lst else 0.0


def _percentile(sorted_vals, pct):
    """p-th percentile. pct in [0, 1]."""
    if not sorted_vals:
        return 0.0
    n = len(sorted_vals)
    idx = int(n * pct)
    idx = max(0, min(idx, n - 1))
    return sorted_vals[idx]

# ═══════════════════════════════════════════════════════════════
# FORMAT DETECTION
# ═══════════════════════════════════════════════════════════════

def detect_format(path):
    """Auto-detect session format from first line."""
    try:
        with open(path, encoding="utf-8", errors="replace") as f:
            first = f.readline().strip()
        if not first:
            return "unknown"
        data = json.loads(first)
        if data.get("role") == "session_meta":
            return "hermes"
        if isinstance(data, dict):
            if "messages" in data and "model" in data:
                return "claude_code"
            if "candidates" in data or "contents" in data:
                return "gemini"
        # Single JSON blob with messages array
        if isinstance(data, list):
            for item in data[:3]:
                if isinstance(item, dict) and item.get("role"):
                    return "claude_code"
        # JSONL with role/timestamp fields → hermes
        if data.get("role") in ("user", "assistant", "tool") and data.get("timestamp"):
            return "hermes"
    except (json.JSONDecodeError, IOError):
        pass
    return "unknown"


def parse(path, fmt=None):
    """Parse a session file, returning list of normalized event dicts."""
    if fmt is None:
        fmt = detect_format(path)

    parsers = {
        "hermes":     _parse_hermes,
        "claude_code": _parse_claude,
        "gemini":     _parse_gemini,
    }
    parser = parsers.get(fmt, _parse_generic)
    return parser(path)


def _parse_hermes(path):
    events = []
    with open(path, encoding="utf-8", errors="replace") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                events.append(json.loads(line))
            except json.JSONDecodeError:
                continue
    return events


def _parse_claude(path):
    with open(path, encoding="utf-8", errors="replace") as f:
        data = json.load(f)

    events = []
    messages = data if isinstance(data, list) else data.get("messages", [])
    model = data if isinstance(data, str) else data.get("model", "unknown") if isinstance(data, dict) else "unknown"

    if isinstance(data, dict) and "usage" in data:
        events.append({"role": "meta", "usage": data["usage"], "model": model})

    for msg in messages:
        role = msg.get("role", "")
        content = msg.get("content", [])
        if isinstance(content, str):
            events.append({"role": role, "content": content})
            continue
        if not isinstance(content, list):
            content = [content]
        for block in content:
            if not isinstance(block, dict):
                continue
            t = block.get("type", "")
            if t == "text":
                events.append({"role": role, "content": block.get("text", "")})
            elif t == "thinking":
                events.append({
                    "role": "assistant",
                    "reasoning": block.get("thinking", ""),
                    "redacted": block.get("redacted", False)
                })
            elif t == "tool_use":
                events.append({
                    "role": "assistant",
                    "tool_call": {
                        "id": block.get("id", ""),
                        "name": block.get("name", ""),
                        "input": block.get("input", {})
                    }
                })
            elif t == "tool_result":
                events.append({
                    "role": "tool",
                    "tool_call_id": block.get("tool_use_id", ""),
                    "content": block.get("content", ""),
                    "is_error": block.get("is_error", False)
                })
    return events


def _parse_gemini(path):
    with open(path, encoding="utf-8", errors="replace") as f:
        raw = f.read().strip()

    events = []
    try:
        data = json.loads(raw)
        if isinstance(data, dict) and "contents" in data:
            for item in data.get("contents", []):
                role = item.get("role", "")
                for part in item.get("parts", []):
                    text = part.get("text", "")
                    if text:
                        events.append({"role": role, "content": text})
            return events
    except json.JSONDecodeError:
        pass

    for line in raw.split("\n"):
        line = line.strip()
        if not line:
            continue
        try:
            events.append(json.loads(line))
        except json.JSONDecodeError:
            continue
    return events


def _parse_generic(path):
    """Try JSONL first, then single JSON blob."""
    with open(path, encoding="utf-8", errors="replace") as f:
        raw = f.read().strip()

    events = []
    for line in raw.split("\n"):
        line = line.strip()
        if not line:
            continue
        try:
            events.append(json.loads(line))
        except json.JSONDecodeError:
            continue

    if not events:
        try:
            data = json.loads(raw)
            if isinstance(data, list):
                events = data
        except json.JSONDecodeError:
            pass

    return events

# ═══════════════════════════════════════════════════════════════
# ANALYSIS ENGINE
# ═══════════════════════════════════════════════════════════════

def analyze(events, model="default"):
    """Run unified analysis on normalized events. Returns metrics dict."""

    m = {
        # Raw counts
        "events_total":        len(events),
        "user_messages":       0,
        "assistant_turns":     0,
        "tool_results":        0,

        # Tool stats
        "tool_calls_total":    0,
        "tool_calls_ok":       0,
        "tool_calls_fail":     0,
        "tool_usage":          Counter(),

        # Reasoning
        "reasoning_blocks":    0,
        "reasoning_total_chars": 0,
        "reasoning_lens":      [],     # list of ints
        "reasoning_redacted":  0,

        # Tokens
        "tokens_input":        0,
        "tokens_output":       0,
        "tokens_cache_write":  0,
        "tokens_cache_read":   0,

        # Timing
        "timestamps":           [],    # list of parsed datetimes
        "gaps_seconds":         [],    # list of inter-event gaps (float)

        # Meta
        "model_used":          model,
        "session_start":       None,
        "session_end":         None,
        "session_duration_s":  0.0,
    }

    # First pass — collect raw data
    for ev in events:
        role = ev.get("role", "")

        # Timestamp
        ts_raw = ev.get("timestamp", "")
        ts = _parse_ts(ts_raw)
        if ts:
            m["timestamps"].append(ts)

        # Session meta line (skip counting)
        if role in ("session_meta", "meta"):
            u = ev.get("usage", {})
            if u:
                m["tokens_input"] += u.get("input_tokens", 0) or 0
                m["tokens_output"] += u.get("output_tokens", 0) or 0
                m["tokens_cache_write"] += u.get("cache_creation_input_tokens", 0) or 0
                m["tokens_cache_read"] += u.get("cache_read_input_tokens", 0) or 0
            continue

        # ── User messages ──
        if role == "user":
            m["user_messages"] += 1
            content = ev.get("content", "")
            if isinstance(content, str):
                m["tokens_input"] += max(1, len(content) // 4)

        # ── Assistant turns ──
        elif role == "assistant":
            m["assistant_turns"] += 1

            # Reasoning content (Claude: "thinking", Hermes: "reasoning_content" or "reasoning")
            reasoning = ev.get("reasoning") or ev.get("reasoning_content") or ""
            if reasoning and isinstance(reasoning, str):
                m["reasoning_blocks"] += 1
                rc = len(reasoning)
                m["reasoning_total_chars"] += rc
                m["reasoning_lens"].append(rc)
                if ev.get("redacted"):
                    m["reasoning_redacted"] += 1
                # Rough token estimate: ~4 chars per token
                m["tokens_output"] += max(1, rc // 4)

            # Text content
            content = ev.get("content", "")
            if isinstance(content, str) and content:
                m["tokens_output"] += max(1, len(content) // 4)

            # Tool calls
            tcs = ev.get("tool_calls", [])
            tc = ev.get("tool_call")
            if tc:
                tcs = [tc]
            m["tool_calls_total"] += len(tcs)
            for tc in tcs:
                fn = tc.get("function", {})
                name = fn.get("name", tc.get("name", "unknown"))
                m["tool_usage"][name] += 1

        # ── Tool results ──
        elif role == "tool":
            m["tool_results"] += 1
            content = ev.get("content", "")

            # Detect failure
            is_err = ev.get("is_error", False)

            # Try parsing JSON content for success/error signals
            if not is_err and isinstance(content, str):
                try:
                    r = json.loads(content)
                    if isinstance(r, dict):
                        is_err = (
                            r.get("success") is False
                            or bool(r.get("error"))
                        )
                except (json.JSONDecodeError, TypeError):
                    pass

            if is_err:
                m["tool_calls_fail"] += 1
            else:
                m["tool_calls_ok"] += 1

        # ── Unknown role → skip ──
        else:
            continue

    # ═══ Post-processing ═══

    # Session timing
    if m["timestamps"]:
        m["session_start"] = min(m["timestamps"]).isoformat()
        m["session_end"]   = max(m["timestamps"]).isoformat()
        m["session_duration_s"] = (max(m["timestamps"]) - min(m["timestamps"])).total_seconds()

    # Inter-event gaps (only positive)
    if len(m["timestamps"]) >= 2:
        for i in range(1, len(m["timestamps"])):
            gap = (m["timestamps"][i] - m["timestamps"][i - 1]).total_seconds()
            if gap > 0:
                m["gaps_seconds"].append(gap)

    # Cost estimation
    p = PRICING.get(model, PRICING["default"])
    m["cost_estimated"] = round(
        m["tokens_input"] / 1e6 * p["input"]
        + m["tokens_output"] / 1e6 * p["output"]
        + m["tokens_cache_write"] / 1e6 * p["cw"]
        + m["tokens_cache_read"] / 1e6 * p["cr"],
        4,
    )

    return m


# ═══════════════════════════════════════════════════════════════
# ANOMALY DETECTION
# ═══════════════════════════════════════════════════════════════

def anomalies(m):
    """Detect anomalies from metrics dict. Returns list of anomaly dicts."""
    a = []

    # Hanging / long latency
    if m["gaps_seconds"]:
        sl = sorted(m["gaps_seconds"])
        long_gaps = [g for g in sl if g > 60]
        if any(g > 300 for g in sl):
            a.append({
                "type": "hanging",
                "severity": "high",
                "emoji": "🔴",
                "detail": f"{len(long_gaps)} gap(s) >60s, max={max(sl):.0f}s",
            })
        elif long_gaps:
            a.append({
                "type": "hanging",
                "severity": "medium",
                "emoji": "🟡",
                "detail": f"{len(long_gaps)} gap(s) >60s, max={max(sl):.0f}s",
            })
        elif _percentile(sl, 0.95) > 30:
            a.append({
                "type": "latency",
                "severity": "low",
                "emoji": "🟢",
                "detail": f"p95 latency = {_percentile(sl, 0.95):.1f}s",
            })

    # Tool failure rate
    total_tools = m["tool_calls_ok"] + m["tool_calls_fail"]
    if total_tools > 0:
        fail_rate = m["tool_calls_fail"] / total_tools
        if fail_rate > 0.30:
            a.append({
                "type": "tool_failures",
                "severity": "high",
                "emoji": "🔴",
                "detail": f"{m['tool_calls_fail']}/{total_tools} failed ({fail_rate:.0%})",
            })
        elif fail_rate > 0.15:
            a.append({
                "type": "tool_failures",
                "severity": "medium",
                "emoji": "🟡",
                "detail": f"{m['tool_calls_fail']}/{total_tools} failed ({fail_rate:.0%})",
            })

    # Shallow thinking
    if m["reasoning_lens"]:
        avg_reason = m["reasoning_total_chars"] / m["reasoning_blocks"]
        if avg_reason < 200:
            a.append({
                "type": "shallow_thinking",
                "severity": "high",
                "emoji": "🔴",
                "detail": f"avg reasoning = {avg_reason:.0f} chars (very shallow)",
            })
        elif avg_reason < 500:
            a.append({
                "type": "shallow_thinking",
                "severity": "medium",
                "emoji": "🟡",
                "detail": f"avg reasoning = {avg_reason:.0f} chars",
            })

    # Redacted thinking
    if m["reasoning_redacted"]:
        a.append({
            "type": "redaction",
            "severity": "medium",
            "emoji": "🟡",
            "detail": f"{m['reasoning_redacted']} block(s) redacted",
        })

    # Zero tool calls (purely chat session)
    if m["tool_calls_total"] == 0 and m["assistant_turns"] > 2:
        a.append({
            "type": "no_tools",
            "severity": "low",
            "emoji": "🟢",
            "detail": "no tool calls — chat-only session",
        })

    return a


# ═══════════════════════════════════════════════════════════════
# HEALTH SCORE
# ═══════════════════════════════════════════════════════════════

def health_score(m, anoms):
    """Calculate 0-100 health score."""
    score = 100
    penalties = {"high": 30, "medium": 12, "low": 4}
    for a in anoms:
        score -= penalties.get(a["severity"], 0)
    return max(0, min(100, score))


# ═══════════════════════════════════════════════════════════════
# REPORTERS
# ═══════════════════════════════════════════════════════════════

def _fmt_duration(seconds):
    """Format seconds as human-readable string."""
    if seconds < 60:
        return f"{seconds:.0f}s"
    elif seconds < 3600:
        return f"{seconds/60:.1f}m"
    else:
        h = int(seconds // 3600)
        m = int((seconds % 3600) // 60)
        return f"{h}h {m}m"


def _health_bar(score):
    """ASCII health bar [████░░]"""
    blocks = score // 5
    return "[" + "█" * blocks + "░" * (20 - blocks) + "]"


def _health_emoji(score):
    if score >= 80:
        return "🟢"
    elif score >= 50:
        return "🟡"
    return "🔴"


def report_text(m, anoms, h):
    """Generate formatted text report."""

    # ── Compute derived values ──
    total_tokens = m["tokens_input"] + m["tokens_output"] + m["tokens_cache_write"] + m["tokens_cache_read"]
    total_tools = m["tool_calls_ok"] + m["tool_calls_fail"]
    success_rate = f"{m['tool_calls_ok'] / total_tools * 100:.0f}%" if total_tools else "N/A"
    avg_reason = m["reasoning_total_chars"] / m["reasoning_blocks"] if m["reasoning_blocks"] else 0

    gaps = sorted(m["gaps_seconds"]) if m["gaps_seconds"] else []

    lines = []
    sep  = "━" * 60
    sub  = "─" * 40

    lines.append(sep)
    lines.append(f"  AGENTTRACE v{__version__} — AI Agent Session Performance Report")
    lines.append(sep)
    lines.append("")

    # ── Token Cost ──
    lines.append("💰 TOKEN COST")
    lines.append(sub)
    lines.append(f"  Input:        {m['tokens_input']:>10,}  tokens")
    lines.append(f"  Output:       {m['tokens_output']:>10,}  tokens")
    if m["tokens_cache_write"] or m["tokens_cache_read"]:
        lines.append(f"  Cache write:  {m['tokens_cache_write']:>10,}  tokens")
        lines.append(f"  Cache read:   {m['tokens_cache_read']:>10,}  tokens")
    lines.append(f"  ────────────────────────────────────")
    lines.append(f"  Total tokens: {total_tokens:>10,}")
    lines.append(f"  Est. cost:    ${m['cost_estimated']:>11.4f}  (model: {m['model_used']})")
    lines.append("")

    # ── Activity ──
    lines.append("📊 ACTIVITY")
    lines.append(sub)
    lines.append(f"  Messages:    {m['user_messages']} user  |  {m['assistant_turns']} turns")
    lines.append(f"  Tool calls:  {m['tool_calls_total']}")
    if total_tools:
        sr_emoji = "🟢" if m["tool_calls_ok"] / total_tools >= 0.85 else ("🟡" if m["tool_calls_ok"] / total_tools >= 0.70 else "🔴")
        lines.append(f"  Success:     {success_rate} ({m['tool_calls_ok']}/{total_tools}) {sr_emoji}")
    lines.append("")

    # ── Latency ──
    lines.append("⏱️  LATENCY")
    lines.append(sub)
    if gaps:
        lines.append(f"  min:     {min(gaps):.1f}s")
        lines.append(f"  median:  {_percentile(gaps, 0.50):.1f}s")
        lines.append(f"  p95:     {_percentile(gaps, 0.95):.1f}s")
        lines.append(f"  max:     {max(gaps):.1f}s")
        lines.append(f"  avg:     {sum(gaps)/len(gaps):.1f}s")
    else:
        lines.append("  (no gap data)")
    lines.append(f"  Duration: {_fmt_duration(m['session_duration_s'])}")
    lines.append("")

    # ── Top Tools ──
    if m["tool_usage"]:
        lines.append("🔧 TOP TOOLS")
        lines.append(sub)
        for name, count in m["tool_usage"].most_common(8):
            lines.append(f"  {name:<35s} {count:>4d}")
        lines.append("")

    # ── Thinking/COT ──
    lines.append("🧠 THINKING / COT")
    lines.append(sub)
    if m["reasoning_blocks"]:
        quality_lbl = "deep" if avg_reason >= 800 else ("moderate" if avg_reason >= 400 else "shallow")
        q_emoji = "🟢" if avg_reason >= 800 else ("🟡" if avg_reason >= 400 else "🔴")
        lines.append(f"  Blocks: {m['reasoning_blocks']}")
        lines.append(f"  Avg:    {avg_reason:.0f} chars")
        lines.append(f"  Total:  {m['reasoning_total_chars']:,} chars")
        lines.append(f"  Quality: {q_emoji} {quality_lbl}")
        if m["reasoning_redacted"]:
            lines.append(f"  ⚠️  {m['reasoning_redacted']} blocks REDACTED")
    else:
        lines.append("  (no thinking blocks)")
    lines.append("")

    # ── Anomalies ──
    lines.append("🚨 ANOMALIES")
    lines.append(sub)
    if anoms:
        for a in anoms:
            lines.append(f"  {a['emoji']} [{a['severity'].upper()}] {a['type']}: {a['detail']}")
    else:
        lines.append("  ✅ No anomalies detected")
    lines.append("")

    # ── Health ──
    lines.append("💯 HEALTH SCORE")
    lines.append(sub)
    h_bar = _health_bar(h)
    h_emoji = _health_emoji(h)
    lines.append(f"  {h_emoji}  {h}/100  {h_bar}")
    lines.append("")
    lines.append(sep)

    return "\n".join(lines)


def report_json(m, anoms, h):
    """Generate JSON report."""
    total_tokens = m["tokens_input"] + m["tokens_output"] + m["tokens_cache_write"] + m["tokens_cache_read"]
    total_tools = m["tool_calls_ok"] + m["tool_calls_fail"]
    avg_reason = round(m["reasoning_total_chars"] / m["reasoning_blocks"], 1) if m["reasoning_blocks"] else 0
    gaps = sorted(m["gaps_seconds"]) if m["gaps_seconds"] else []
    session_start = min(m["timestamps"]).isoformat() if m["timestamps"] else None
    session_end   = max(m["timestamps"]).isoformat() if m["timestamps"] else None

    payload = {
        "version": __version__,
        "model_used": m["model_used"],
        "session": {
            "start": session_start,
            "end": session_end,
            "duration_seconds": m["session_duration_s"],
            "duration_human": _fmt_duration(m["session_duration_s"]),
        },
        "tokens": {
            "input":  m["tokens_input"],
            "output": m["tokens_output"],
            "cache_write": m["tokens_cache_write"],
            "cache_read":  m["tokens_cache_read"],
            "total":  total_tokens,
        },
        "cost": {
            "estimated": m["cost_estimated"],
            "model": m["model_used"],
        },
        "activity": {
            "user_messages":      m["user_messages"],
            "assistant_turns":    m["assistant_turns"],
            "tool_calls_total":   m["tool_calls_total"],
            "tool_calls_ok":      m["tool_calls_ok"],
            "tool_calls_fail":    m["tool_calls_fail"],
            "tool_success_rate":  round(m["tool_calls_ok"] / total_tools * 100, 1) if total_tools else 0,
        },
        "latency": {
            "min":    _safe_calc(gaps, min),
            "median": _safe_calc(gaps, lambda x: _percentile(x, 0.50)),
            "p95":    _safe_calc(gaps, lambda x: _percentile(x, 0.95)),
            "max":    _safe_calc(gaps, max),
            "avg":    _safe_calc(gaps, lambda x: sum(x) / len(x)),
        },
        "tools_top": dict(m["tool_usage"].most_common(10)),
        "reasoning": {
            "blocks":       m["reasoning_blocks"],
            "total_chars":  m["reasoning_total_chars"],
            "avg_chars":    avg_reason,
            "redacted":     m["reasoning_redacted"],
        },
        "anomalies": [
            {
                "type":     a["type"],
                "severity": a["severity"],
                "detail":   a["detail"],
            }
            for a in anoms
        ],
        "health_score": h,
    }
    return json.dumps(payload, indent=2, ensure_ascii=False)


def report_compare(sessions, model="default"):
    """Generate multi-session comparison (text table)."""
    lines = []
    sep = "━" * 76
    lines.append(sep)
    lines.append(f"  AGENTTRACE — Multi-Session Comparison  (model: {model})")
    lines.append(sep)
    lines.append("")
    header = f"  {'Session':<28s} {'Msgs':>4s} {'Turns':>5s} {'Tools':>5s} {'Succ':>5s} {'Cost':>9s} {'Health':>7s}"
    lines.append(header)
    lines.append("  " + "─" * 70)

    for name, m, anoms, h in sessions:
        total_tools = m["tool_calls_ok"] + m["tool_calls_fail"]
        sr = f"{m['tool_calls_ok'] / total_tools * 100:.0f}%" if total_tools else "N/A"
        h_emoji = _health_emoji(h)
        lines.append(
            f"  {name[:27]:<28s} "
            f"{m['user_messages']:>4d} "
            f"{m['assistant_turns']:>5d} "
            f"{m['tool_calls_total']:>5d} "
            f"{sr:>5s} "
            f"${m['cost_estimated']:>8.4f} "
            f"{h_emoji} {h:>4d}/100"
        )
    lines.append(sep)
    return "\n".join(lines)


# ═══════════════════════════════════════════════════════════════
# CLI
# ═══════════════════════════════════════════════════════════════

def _find_jsonl_files(directory):
    """Find JSONL files in directory, sorted by name (desc) for latest-first."""
    dd = Path(directory)
    if not dd.is_dir():
        return []
    return sorted(
        [str(p) for p in dd.glob("*.jsonl")],
        reverse=True,
    )


def _find_latest(directory):
    """Find the chronologically latest JSONL file in directory."""
    jsonls = _find_jsonl_files(directory)
    if not jsonls:
        raise SystemExit(f"No .jsonl session files found in {directory}")
    # Sort by mtime
    jsonls.sort(key=lambda p: os.path.getmtime(p), reverse=True)
    return jsonls[0]


def _ensure_output_dir(path):
    """Ensure parent directory of output path exists."""
    parent = os.path.dirname(os.path.abspath(path))
    if parent:
        os.makedirs(parent, exist_ok=True)


def main():
    parser = argparse.ArgumentParser(
        description=f"agenttrace v{__version__} — AI Agent Session Performance Analyzer",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  agenttrace                           launch interactive TUI dashboard (default)
  agenttrace --latest                  analyze latest session
  agenttrace session.jsonl             analyze a specific session
  agenttrace -f json session.jsonl     JSON output
  agenttrace --compare                 compare all sessions
  agenttrace --dir /path/to/sessions   analyze specific directory
  agenttrace -m claude-sonnet-4 --latest   with model pricing
  agenttrace --list-models             show known models
""",
    )
    parser.add_argument(
        "path", nargs="?", help="Session file path (JSONL or JSON)"
    )
    parser.add_argument(
        "-f", "--format", choices=["text", "json"], default="text",
        help="Output format (default: text)",
    )
    parser.add_argument(
        "-d", "--dir", help="Directory containing session JSONL files",
    )
    parser.add_argument(
        "-c", "--compare", action="store_true",
        help="Compare all sessions in the directory",
    )
    parser.add_argument(
        "-m", "--model", default="default",
        help="Model for token cost estimation (default: claude-sonnet-4 pricing)",
    )
    parser.add_argument(
        "-o", "--output", help="Save report to file",
    )
    parser.add_argument(
        "--latest", action="store_true",
        help="Analyze the latest (most recently modified) session",
    )
    parser.add_argument(
        "--list-models", action="store_true",
        help="List available models with pricing",
    )

    args = parser.parse_args()

    # ── Default: no explicit action → launch TUI ──
    has_explicit_action = (
        args.path is not None
        or args.latest
        or args.compare
        or args.list_models
    )
    if not has_explicit_action:
        sessions_dir = args.dir or os.path.expanduser("~/.hermes/sessions")
        from agenttrace.tui import main as tui_main
        import sys as _sys
        _sys.argv = [_sys.argv[0], "-d", sessions_dir]
        tui_main()
        return

    # ── List models ──
    if args.list_models:
        print(f"agenttrace v{__version__} — Supported Models")
        print("=" * 58)
        print(f"  {'Model':<22s} {'Input $/M':>10s} {'Output $/M':>10s}")
        print("  " + "-" * 44)
        for k, v in PRICING.items():
            if k == "default":
                continue
            print(f"  {k:<22s} ${v['input']:>8.2f}  ${v['output']:>8.2f}")
        print()
        print(f"  default = claude-sonnet-4 (${PRICING['default']['input']:.2f}/${PRICING['default']['output']:.2f})")
        return

    # ── Determine session directory ──
    sessions_dir = args.dir or os.path.expanduser("~/.hermes/sessions")

    # ── Resolve path ──
    target_path = args.path

    if args.latest:
        target_path = _find_latest(sessions_dir)
        if not target_path:
            print("No session files found.", file=sys.stderr)
            sys.exit(1)

    if args.compare:
        # Compare mode — analyze all sessions in directory
        jsonls = _find_jsonl_files(sessions_dir)
        if len(jsonls) == 0:
            print(f"No .jsonl files found in {sessions_dir}", file=sys.stderr)
            sys.exit(1)
        if len(jsonls) > 15:
            jsonls = jsonls[:15]  # Cap at 15 for readability

        sessions = []
        for jl in jsonls:
            events = parse(jl)
            m = analyze(events, args.model)
            anoms = anomalies(m)
            h = health_score(m, anoms)
            # Use the basename without extension as label
            label = os.path.basename(jl).replace(".jsonl", "")
            # Truncate long labels
            if len(label) > 27:
                label = label[:24] + "..."
            sessions.append((label, m, anoms, h))

        out = report_compare(sessions, args.model)

        if args.output:
            _ensure_output_dir(args.output)
            with open(args.output, "w", encoding="utf-8") as f:
                f.write(out + "\n")
            print(f"Saved: {args.output}", file=sys.stderr)

        print(out)
        return

    # ── Single-session mode ──
    if not target_path:
        # Default to latest in session dir
        target_path = _find_latest(sessions_dir)

    if not os.path.exists(target_path):
        print(f"File not found: {target_path}", file=sys.stderr)
        sys.exit(1)

    events = parse(target_path)
    if not events:
        print(f"No parseable events in: {target_path}", file=sys.stderr)
        sys.exit(1)

    m = analyze(events, args.model)
    anoms = anomalies(m)
    h = health_score(m, anoms)

    if args.format == "json":
        out = report_json(m, anoms, h)
    else:
        out = report_text(m, anoms, h)

    if args.output:
        _ensure_output_dir(args.output)
        with open(args.output, "w", encoding="utf-8") as f:
            f.write(out + "\n")
        print(f"Saved: {args.output}", file=sys.stderr)

    print(out)


if __name__ == "__main__":
    main()
