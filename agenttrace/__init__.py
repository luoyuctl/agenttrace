#!/usr/bin/env python3
"""
agenttrace - AI Agent Session Performance Analyzer
Usage:
    agenttrace <session.jsonl> [session.json]    Analyze a session
    agenttrace --dir <directory>                  Analyze all sessions in directory
    agenttrace --latest                           Analyze latest session
"""

import json
import os
import sys
import argparse
from datetime import datetime
from collections import Counter


def parse_jsonl(path):
    events = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                events.append(json.loads(line))
            except json.JSONDecodeError:
                continue
    return events


def parse_session_json(path):
    with open(path) as f:
        return json.load(f)


def analyze_events(events):
    metrics = {
        "total_events": len(events),
        "user_messages": 0,
        "assistant_turns": 0,
        "tool_results": 0,
        "tool_calls_total": 0,
        "tool_call_success": 0,
        "tool_call_fail": 0,
        "tool_names": Counter(),
        "reasoning_blocks": [],
        "timestamps": [],
        "turn_latencies": [],
    }

    pending_tool_calls = {}

    for ev in events:
        role = ev.get("role", "")
        ts_str = ev.get("timestamp", "")
        ts = None
        if ts_str:
            try:
                ts = datetime.fromisoformat(ts_str)
                metrics["timestamps"].append(ts)
            except ValueError:
                pass

        if role == "user":
            metrics["user_messages"] += 1

        elif role == "assistant":
            metrics["assistant_turns"] += 1
            reasoning = ev.get("reasoning_content", ev.get("reasoning", ""))
            if reasoning:
                metrics["reasoning_blocks"].append(len(reasoning))

            tool_calls = ev.get("tool_calls", [])
            metrics["tool_calls_total"] += len(tool_calls)

            for tc in tool_calls:
                fn = tc.get("function", {})
                name = fn.get("name", "unknown")
                metrics["tool_names"][name] += 1
                call_id = tc.get("call_id", tc.get("id", ""))
                if call_id and ts:
                    pending_tool_calls[call_id] = ts

        elif role == "tool":
            metrics["tool_results"] += 1
            call_id = ev.get("tool_call_id", "")
            content = ev.get("content", "")

            try:
                result = json.loads(content) if isinstance(content, str) else content
                if isinstance(result, dict):
                    if result.get("success") == False or result.get("error"):
                        metrics["tool_call_fail"] += 1
                    else:
                        metrics["tool_call_success"] += 1
                else:
                    metrics["tool_call_success"] += 1
            except (json.JSONDecodeError, TypeError):
                if content and "error" not in str(content).lower()[:200]:
                    metrics["tool_call_success"] += 1
                else:
                    metrics["tool_call_fail"] += 1

            if call_id in pending_tool_calls and ts:
                latency = (ts - pending_tool_calls[call_id]).total_seconds()
                if latency >= 0:
                    pass  # We'll use turn_latencies for gaps

    if len(metrics["timestamps"]) >= 2:
        for i in range(1, len(metrics["timestamps"])):
            gap = (metrics["timestamps"][i] - metrics["timestamps"][i - 1]).total_seconds()
            if gap > 0:
                metrics["turn_latencies"].append(gap)

    return metrics


def detect_anomalies(metrics):
    anomalies = []
    latencies = metrics.get("turn_latencies", [])

    if latencies:
        sorted_l = sorted(latencies)
        p95_idx = int(len(sorted_l) * 0.95)
        p95 = sorted_l[p95_idx if p95_idx < len(sorted_l) else -1]

        long_gaps = [(i, gap) for i, gap in enumerate(latencies) if gap > 60]
        if long_gaps:
            anomalies.append({
                "type": "hanging",
                "severity": "high" if any(g > 300 for _, g in long_gaps) else "medium",
                "detail": f"{len(long_gaps)} gaps > 60s. Max: {max(g for _, g in long_gaps):.0f}s",
            })
        elif p95 > 30:
            anomalies.append({
                "type": "slow_response",
                "severity": "low",
                "detail": f"P95 gap: {p95:.1f}s",
            })

    total = metrics["tool_call_success"] + metrics["tool_call_fail"]
    if total > 0:
        fail_rate = metrics["tool_call_fail"] / total
        if fail_rate > 0.2:
            anomalies.append({
                "type": "tool_failures",
                "severity": "high",
                "detail": f"{metrics['tool_call_fail']}/{total} failed ({fail_rate:.0%})",
            })

    reasoning_blocks = metrics.get("reasoning_blocks", [])
    if reasoning_blocks:
        avg_r = sum(reasoning_blocks) / len(reasoning_blocks)
        if avg_r < 500:
            anomalies.append({
                "type": "shallow_thinking",
                "severity": "medium",
                "detail": f"Avg reasoning: {avg_r:.0f} chars",
            })

    return anomalies


def generate_report(session_info, metrics, anomalies, fmt="text"):
    if fmt == "json":
        return json.dumps({
            "session": session_info,
            "metrics": {
                k: (dict(v) if isinstance(v, Counter) else v)
                for k, v in metrics.items()
                if k != "timestamps"
            },
            "anomalies": anomalies,
        }, indent=2, default=str)

    # Text format
    lines = ["=" * 60, "  AGENTTRACE - AI Agent Session Performance Report", "=" * 60, ""]
    sid = session_info.get("session_id", "N/A")
    model = session_info.get("model", "N/A")
    plat = session_info.get("platform", "N/A")
    start = session_info.get("session_start", "")
    end = session_info.get("last_updated", "")

    lines.append("📋 SESSION INFO")
    lines.append("-" * 40)
    lines.append(f"  Session:  {sid}")
    lines.append(f"  Model:    {model}")
    lines.append(f"  Platform: {plat}")
    if start:
        lines.append(f"  Started:  {start}")
    if end:
        lines.append(f"  Ended:    {end}")
        if start:
            try:
                dur = datetime.fromisoformat(end) - datetime.fromisoformat(start)
                lines.append(f"  Duration: {dur}")
            except ValueError:
                pass
    lines.append("")

    lines.append("📊 ACTIVITY")
    lines.append("-" * 40)
    lines.append(f"  Messages:    {metrics['user_messages']}")
    lines.append(f"  Turns:       {metrics['assistant_turns']}")
    lines.append(f"  Tool calls:  {metrics['tool_calls_total']}")
    total_tc = metrics["tool_call_success"] + metrics["tool_call_fail"]
    if total_tc > 0:
        sr = metrics["tool_call_success"] / total_tc * 100
        lines.append(f"  Success:     {sr:.0f}% ({metrics['tool_call_success']}/{total_tc})")
    lines.append("")

    latencies = metrics.get("turn_latencies", [])
    if latencies:
        lines.append("⏱️  LATENCY (gaps)")
        lines.append("-" * 40)
        sl = sorted(latencies)
        lines.append(f"  p50:  {sl[len(sl)//2]:.1f}s")
        p95i = min(int(len(sl) * 0.95), len(sl) - 1)
        lines.append(f"  p95:  {sl[p95i]:.1f}s")
        lines.append(f"  max:  {sl[-1]:.1f}s")
        lines.append("")

    tn = metrics.get("tool_names", Counter())
    if tn:
        lines.append("🔧 TOP TOOLS")
        lines.append("-" * 40)
        for name, n in tn.most_common(8):
            lines.append(f"  {name:<35s} {n:>4d}")
        lines.append("")

    rb = metrics.get("reasoning_blocks", [])
    if rb:
        lines.append("🧠 THINKING")
        lines.append("-" * 40)
        avg = sum(rb) / len(rb)
        lines.append(f"  Blocks:  {len(rb)}")
        lines.append(f"  Avg:     {avg:.0f} chars")
        lines.append(f"  Total:   {sum(rb):,} chars")
        lines.append("")

    if anomalies:
        lines.append("🚨 ANOMALIES")
        lines.append("-" * 40)
        for a in anomalies:
            icon = {"high": "🔴", "medium": "🟡", "low": "🟢"}.get(a["severity"], "⚪")
            lines.append(f"  {icon} [{a['severity'].upper()}] {a['type']}: {a['detail']}")
        lines.append("")

    score = 100
    for a in anomalies:
        deduct = {"high": 25, "medium": 10, "low": 5}
        score -= deduct.get(a["severity"], 0)
    score = max(0, min(100, score))

    lines.append("💯 HEALTH")
    lines.append("-" * 40)
    color = "🟢" if score >= 80 else ("🟡" if score >= 60 else "🔴")
    bar = "█" * (score // 5) + "░" * (20 - score // 5)
    lines.append(f"  {color}  {score}/100  [{bar}]")
    lines.append("=" * 60)
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description="AI Agent Session Performance Analyzer")
    parser.add_argument("jsonl", nargs="?", help="Path to session JSONL file")
    parser.add_argument("--json", "-j", help="Path to session JSON metadata file")
    parser.add_argument("--dir", "-d", help="Analyze all sessions in directory")
    parser.add_argument("--latest", "-l", action="store_true", help="Analyze latest session")
    parser.add_argument("--format", "-f", choices=["text", "json"], default="text")
    parser.add_argument("--output", "-o", help="Save report to file")
    args = parser.parse_args()

    sessions_dir = os.path.expanduser("~/.hermes/sessions")

    if args.latest:
        jsonls = sorted(
            [f for f in os.listdir(sessions_dir) if f.endswith(".jsonl")],
            reverse=True,
        )
        if not jsonls:
            print("No sessions found.", file=sys.stderr)
            sys.exit(1)
        args.jsonl = os.path.join(sessions_dir, jsonls[0])
        json_name = jsonls[0].replace(".jsonl", ".json")
        sj = os.path.join(sessions_dir, f"session_{json_name}")
        if os.path.exists(sj):
            args.json = sj

    if args.dir:
        jsonls = sorted(
            [f for f in os.listdir(args.dir) if f.endswith(".jsonl")],
            reverse=True,
        )
        for jl in jsonls[:5]:  # Top 5
            path = os.path.join(args.dir, jl)
            events = parse_jsonl(path)
            metrics = analyze_events(events)
            anomalies = detect_anomalies(metrics)
            info = {"session_id": jl.replace(".jsonl", "")}
            report = generate_report(info, metrics, anomalies, args.format)
            print(report)
            print()
        return

    if not args.jsonl:
        parser.print_help()
        sys.exit(1)

    if not os.path.exists(args.jsonl):
        print(f"File not found: {args.jsonl}", file=sys.stderr)
        sys.exit(1)

    events = parse_jsonl(args.jsonl)
    session_info = {}
    if args.json and os.path.exists(args.json):
        session_info = parse_session_json(args.json)
    else:
        session_info = {"session_id": os.path.basename(args.jsonl).replace(".jsonl", "")}

    metrics = analyze_events(events)
    anomalies = detect_anomalies(metrics)
    report = generate_report(session_info, metrics, anomalies, args.format)

    if args.output:
        os.makedirs(os.path.dirname(args.output) or ".", exist_ok=True)
        with open(args.output, "w") as f:
            f.write(report)
        print(f"Saved: {args.output}", file=sys.stderr)

    print(report)


if __name__ == "__main__":
    main()
