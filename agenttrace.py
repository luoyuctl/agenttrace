#!/usr/bin/env python3
"""
agenttrace v2 — Multi-format AI Agent Session Analyzer
Supports: Hermes Agent, Claude Code, Gemini CLI
Features: performance metrics, token cost estimation, multi-session comparison
"""

import json, os, sys, argparse
from datetime import datetime
from collections import Counter, defaultdict

# ─── PRICING (USD per 1M tokens) ───
PRICING = {
    "claude-opus-4":     {"input": 15.00, "output": 75.00, "cw": 18.75, "cr": 1.50},
    "claude-sonnet-4":   {"input": 3.00,  "output": 15.00, "cw": 3.75,  "cr": 0.30},
    "claude-haiku-3.5":  {"input": 0.80,  "output": 4.00,  "cw": 1.00,  "cr": 0.08},
    "gemini-2.5-pro":    {"input": 1.25,  "output": 10.00, "cw": 0,     "cr": 0},
    "gemini-2.5-flash":  {"input": 0.15,  "output": 0.60,  "cw": 0,     "cr": 0},
    "gpt-4.1":          {"input": 2.00,  "output": 8.00,  "cw": 0,     "cr": 0},
    "default":           {"input": 3.00,  "output": 15.00, "cw": 0,     "cr": 0},
}

# ─── FORMAT DETECTION & PARSING ───

def detect_format(path):
    """Auto-detect session format from first line."""
    try:
        with open(path) as f:
            first = f.readline().strip()
        data = json.loads(first)
        if data.get("role") == "session_meta":
            return "hermes"
        # Claude: {"model": ..., "messages": [...]} or [{"role": ...}]
        if "messages" in data or ("model" in data and isinstance(data, dict)):
            return "claude_code"
        if "candidates" in data or "contents" in data:
            return "gemini"
    except:
        pass
    return "unknown"

def parse(path, fmt=None):
    """Auto-detect and parse a session file."""
    if fmt is None:
        fmt = detect_format(path)
    
    if fmt == "hermes":
        return _parse_hermes(path)
    elif fmt == "claude_code":
        return _parse_claude(path)
    elif fmt == "gemini":
        return _parse_gemini(path)
    else:
        # Generic: try line-by-line JSON
        return _parse_generic(path)

def _parse_hermes(path):
    events = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line: continue
            try:
                events.append(json.loads(line))
            except: continue
    return events

def _parse_claude(path):
    with open(path) as f:
        data = json.load(f)
    events = []
    messages = data if isinstance(data, list) else data.get("messages", [])
    model = "unknown" if isinstance(data, list) else data.get("model", "unknown")
    
    if not isinstance(data, list) and "usage" in data:
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
            if not isinstance(block, dict): continue
            t = block.get("type", "")
            if t == "text":
                events.append({"role": role, "content": block.get("text", "")})
            elif t == "thinking":
                events.append({"role": "assistant", "reasoning": block.get("thinking", ""),
                               "redacted": block.get("redacted", False)})
            elif t == "tool_use":
                events.append({"role": "assistant", "tool_call": {
                    "id": block.get("id", ""), "name": block.get("name", ""),
                    "input": block.get("input", {})}})
            elif t == "tool_result":
                events.append({"role": "tool", "tool_call_id": block.get("tool_use_id", ""),
                               "content": block.get("content", ""),
                               "is_error": block.get("is_error", False)})
    return events

def _parse_gemini(path):
    with open(path) as f:
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
    except: pass
    
    for line in raw.split("\n"):
        line = line.strip()
        if not line: continue
        try: events.append(json.loads(line))
        except: continue
    return events

def _parse_generic(path):
    with open(path) as f:
        raw = f.read().strip()
    events = []
    for line in raw.split("\n"):
        line = line.strip()
        if not line: continue
        try: events.append(json.loads(line))
        except: continue
    return events

# ─── ANALYSIS ENGINE ───

def analyze(events, model="default"):
    """Unified analysis across all formats."""
    m = {
        "events": len(events),
        "user_msgs": 0, "turns": 0, "tool_results": 0,
        "tool_total": 0, "tool_ok": 0, "tool_fail": 0,
        "tool_names": Counter(), "reasoning_lens": [],
        "timestamps": [], "gaps": [],
        "tokens": {"input": 0, "output": 0, "cache_write": 0, "cache_read": 0},
        "thinking_redacted": 0,
    }
    
    pending = {}
    
    for ev in events:
        role = ev.get("role", "")
        ts = _parse_ts(ev.get("timestamp", ""))
        if ts: m["timestamps"].append(ts)
        
        if role == "session_meta": continue
        
        # Tokens from meta
        if role == "meta":
            u = ev.get("usage", {})
            m["tokens"]["input"] += u.get("input_tokens", 0)
            m["tokens"]["output"] += u.get("output_tokens", 0)
            m["tokens"]["cache_write"] += u.get("cache_creation_input_tokens", 0)
            m["tokens"]["cache_read"] += u.get("cache_read_input_tokens", 0)
            continue
        
        if role == "user":
            m["user_msgs"] += 1
            # Estimate input tokens from content length (rough: 1 token ≈ 4 chars)
            content = ev.get("content", "")
            m["tokens"]["input"] += len(content) // 4 if isinstance(content, str) else 0
        
        elif role == "assistant":
            m["turns"] += 1
            reasoning = ev.get("reasoning", ev.get("reasoning_content", ""))
            if reasoning:
                m["reasoning_lens"].append(len(reasoning))
                if ev.get("redacted"):
                    m["thinking_redacted"] += 1
                # Estimate output tokens
                m["tokens"]["output"] += len(reasoning) // 4
            
            content = ev.get("content", "")
            if content:
                m["tokens"]["output"] += len(content) // 4 if isinstance(content, str) else 0
            
            # Tool calls
            tcs = ev.get("tool_calls", [])
            tc = ev.get("tool_call")
            if tc: tcs = [tc]
            
            m["tool_total"] += len(tcs)
            for tc in tcs:
                fn = tc.get("function", {})
                name = fn.get("name", tc.get("name", "unknown"))
                m["tool_names"][name] += 1
                cid = tc.get("call_id", tc.get("id", ""))
                if cid and ts: pending[cid] = ts
        
        elif role == "tool":
            m["tool_results"] += 1
            cid = ev.get("tool_call_id", "")
            content = ev.get("content", "")
            
            # Success/failure
            is_err = ev.get("is_error", False)
            if not is_err:
                try:
                    r = json.loads(content) if isinstance(content, str) else content
                    if isinstance(r, dict):
                        is_err = r.get("success") == False or bool(r.get("error"))
                except: pass
            
            if is_err: m["tool_fail"] += 1
            else: m["tool_ok"] += 1
    
    # Gaps
    if len(m["timestamps"]) >= 2:
        for i in range(1, len(m["timestamps"])):
            gap = (m["timestamps"][i] - m["timestamps"][i-1]).total_seconds()
            if gap > 0: m["gaps"].append(gap)
    
    # Cost
    p = PRICING.get(model, PRICING["default"])
    m["cost"] = (
        m["tokens"]["input"] / 1e6 * p["input"] +
        m["tokens"]["output"] / 1e6 * p["output"] +
        m["tokens"]["cache_write"] / 1e6 * p["cw"] +
        m["tokens"]["cache_read"] / 1e6 * p["cr"]
    )
    m["model"] = model
    
    return m

def _parse_ts(s):
    if not s: return None
    try: return datetime.fromisoformat(s)
    except: return None

# ─── ANOMALY DETECTION ───

def anomalies(m):
    a = []
    if m["gaps"]:
        sl = sorted(m["gaps"])
        p95 = sl[min(int(len(sl)*0.95), len(sl)-1)]
        long = [g for g in m["gaps"] if g > 60]
        if long:
            a.append({"type": "hanging", "sev": "high" if max(long)>300 else "medium",
                       "detail": f"{len(long)} gaps >60s, max={max(long):.0f}s"})
        elif p95 > 30:
            a.append({"type": "slow", "sev": "low", "detail": f"p95={p95:.1f}s"})
    
    total = m["tool_ok"] + m["tool_fail"]
    if total and m["tool_fail"]/total > 0.2:
        a.append({"type": "tool_failures", "sev": "high",
                   "detail": f"{m['tool_fail']}/{total} failed ({m['tool_fail']/total:.0%})"})
    
    if m["reasoning_lens"]:
        avg = sum(m["reasoning_lens"])/len(m["reasoning_lens"])
        if avg < 500:
            a.append({"type": "shallow", "sev": "medium", "detail": f"avg={avg:.0f} chars"})
    
    if m["thinking_redacted"]:
        a.append({"type": "redacted", "sev": "medium",
                   "detail": f"{m['thinking_redacted']} blocks redacted"})
    
    return a

# ─── HEALTH SCORE ───

def health(m, anoms):
    score = 100
    for a in anoms:
        score -= {"high": 25, "medium": 10, "low": 5}.get(a["sev"], 0)
    return max(0, min(100, score))

# ─── REPORT GENERATION ───

def report(m, anoms, fmt="text"):
    if fmt == "json":
        return json.dumps({
            "metrics": {k: (dict(v) if isinstance(v, Counter) else v)
                        for k, v in m.items() if k != "timestamps"},
            "anomalies": anoms,
        }, indent=2, default=str)
    
    lines = ["=" * 60, "  AGENTTRACE v2 — AI Agent Session Performance Report", "=" * 60, ""]
    
    # Cost
    lines.append("💰 TOKEN COST")
    lines.append("-" * 40)
    lines.append(f"  Input:         {m['tokens']['input']:>10,} tokens")
    lines.append(f"  Output:        {m['tokens']['output']:>10,} tokens")
    lines.append(f"  Cache write:   {m['tokens']['cache_write']:>10,} tokens")
    lines.append(f"  Cache read:    {m['tokens']['cache_read']:>10,} tokens")
    total_t = sum(m["tokens"].values())
    lines.append(f"  ─────────────────────────────")
    lines.append(f"  Total tokens:  {total_t:>10,}")
    lines.append(f"  Est. cost:     ${m['cost']:>10.4f}")
    lines.append("")
    
    # Activity
    lines.append("📊 ACTIVITY")
    lines.append("-" * 40)
    lines.append(f"  Messages:  {m['user_msgs']}")
    lines.append(f"  Turns:     {m['turns']}")
    lines.append(f"  Tool calls:{m['tool_total']}")
    total = m["tool_ok"] + m["tool_fail"]
    if total:
        lines.append(f"  Success:   {m['tool_ok']/total*100:.0f}% ({m['tool_ok']}/{total})")
    lines.append("")
    
    # Latency
    if m["gaps"]:
        sl = sorted(m["gaps"])
        lines.append("⏱️  LATENCY")
        lines.append("-" * 40)
        lines.append(f"  p50:  {sl[len(sl)//2]:.1f}s")
        p95i = min(int(len(sl)*0.95), len(sl)-1)
        lines.append(f"  p95:  {sl[p95i]:.1f}s")
        lines.append(f"  max:  {sl[-1]:.1f}s")
        lines.append("")
    
    # Tools
    if m["tool_names"]:
        lines.append("🔧 TOP TOOLS")
        lines.append("-" * 40)
        for n, c in m["tool_names"].most_common(8):
            lines.append(f"  {n:<35s} {c:>4d}")
        lines.append("")
    
    # Thinking
    if m["reasoning_lens"]:
        lines.append("🧠 THINKING")
        lines.append("-" * 40)
        avg = sum(m["reasoning_lens"]) / len(m["reasoning_lens"])
        lines.append(f"  Blocks: {len(m['reasoning_lens'])}")
        lines.append(f"  Avg:    {avg:.0f} chars")
        lines.append(f"  Total:  {sum(m['reasoning_lens']):,}")
        if m["thinking_redacted"]:
            lines.append(f"  ⚠️  {m['thinking_redacted']} blocks REDACTED")
        lines.append("")
    
    # Anomalies
    if anoms:
        lines.append("🚨 ANOMALIES")
        lines.append("-" * 40)
        for a in anoms:
            i = {"high": "🔴", "medium": "🟡", "low": "🟢"}.get(a["sev"], "⚪")
            lines.append(f"  {i} [{a['sev'].upper()}] {a['type']}: {a['detail']}")
        lines.append("")
    
    # Health
    h = health(m, anoms)
    lines.append("💯 HEALTH")
    lines.append("-" * 40)
    c = "🟢" if h >= 80 else ("🟡" if h >= 60 else "🔴")
    bar = "█" * (h // 5) + "░" * (20 - h // 5)
    lines.append(f"  {c}  {h}/100  [{bar}]")
    lines.append("=" * 60)
    
    return "\n".join(lines)

def compare(sessions):
    """Multi-session comparison."""
    lines = ["=" * 65, "  AGENTTRACE — Multi-Session Comparison", "=" * 65, ""]
    lines.append(f"{'Session':<24s} {'Turns':>6s} {'Tools':>6s} {'Succ':>6s} {'Cost':>8s} {'Health':>7s}")
    lines.append("-" * 65)
    
    for name, m, anoms in sessions:
        total = m["tool_ok"] + m["tool_fail"]
        sr = f"{m['tool_ok']/total*100:.0f}%" if total else "N/A"
        h = health(m, anoms)
        lines.append(
            f"{name[:23]:<24s} {m['turns']:>6d} {m['tool_total']:>6d} "
            f"{sr:>6s} ${m['cost']:>7.4f} {h:>6d}/100"
        )
    
    lines.append("=" * 65)
    return "\n".join(lines)

# ─── CLI ───

def main():
    p = argparse.ArgumentParser(description="agenttrace v2 — Multi-format AI Agent Session Analyzer")
    p.add_argument("path", nargs="?", help="Session file path")
    p.add_argument("--format", "-f", choices=["text", "json"], default="text")
    p.add_argument("--dir", "-d", help="Analyze all sessions in directory")
    p.add_argument("--compare", "-c", action="store_true", help="Compare all sessions")
    p.add_argument("--model", "-m", default="default", help="Model for cost estimation")
    p.add_argument("--output", "-o", help="Save report to file")
    p.add_argument("--list-models", action="store_true", help="List known models")
    args = p.parse_args()
    
    if args.list_models:
        print("Known models for pricing:")
        for k, v in PRICING.items():
            print(f"  {k:<22s}  in=${v['input']}/M  out=${v['output']}/M")
        return
    
    sd = os.path.expanduser("~/.hermes/sessions")
    
    if args.dir:
        sd = args.dir
    
    if args.compare:
        if not args.path:
            jsonls = sorted([f for f in os.listdir(sd) if f.endswith('.jsonl')], reverse=True)[:10]
        else:
            jsonls = sorted([f for f in os.listdir(args.path) if f.endswith('.jsonl')], reverse=True)[:10]
            sd = args.path
        
        sessions = []
        for jl in jsonls:
            path = os.path.join(sd, jl)
            events = parse(path)
            m = analyze(events, args.model)
            anoms = anomalies(m)
            sessions.append((jl.replace(".jsonl", ""), m, anoms))
        
        out = compare(sessions)
        if args.output:
            with open(args.output, 'w') as f: f.write(out)
        print(out)
        return
    
    if not args.path:
        # Default: latest session
        jsonls = sorted([f for f in os.listdir(sd) if f.endswith('.jsonl')], reverse=True)
        if not jsonls:
            print("No sessions found.", file=sys.stderr)
            sys.exit(1)
        args.path = os.path.join(sd, jsonls[0])
    
    if not os.path.exists(args.path):
        print(f"Not found: {args.path}", file=sys.stderr)
        sys.exit(1)
    
    events = parse(args.path)
    m = analyze(events, args.model)
    anoms = anomalies(m)
    out = report(m, anoms, args.format)
    
    if args.output:
        os.makedirs(os.path.dirname(args.output) or ".", exist_ok=True)
        with open(args.output, 'w') as f: f.write(out)
        print(f"Saved: {args.output}", file=sys.stderr)
    
    print(out)

if __name__ == "__main__":
    main()
