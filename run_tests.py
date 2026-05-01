#!/usr/bin/env python3
"""agenttrace v3 test runner — no external deps. 70+ tests."""
import sys, os, json, tempfile, subprocess
from collections import Counter

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import agenttrace as at

PASS = FAIL = 0
FAILURES = []

def _jl(events):
    fd, p = tempfile.mkstemp(suffix=".jsonl", text=True)
    with os.fdopen(fd, "w") as f:
        for ev in events:
            f.write(json.dumps(ev) + "\n")
    return p

def _rm(p):
    if p and os.path.exists(p):
        try: os.unlink(p)
        except: pass

def T(name, check):
    global PASS, FAIL, FAILURES
    try:
        ok = check() if callable(check) else bool(check)
        if ok:
            PASS += 1; print(f"  ✅ {name}")
        else:
            FAIL += 1; FAILURES.append(f"{name}: falsy"); print(f"  ❌ {name}")
    except Exception as e:
        FAIL += 1; FAILURES.append(f"{name}: {e}"); print(f"  ❌ {name} — {e}")

# ═══ Fixtures ═══
# Clean session (tight timestamps, no anomalies)
CLEAN = [
    {"role": "session_meta", "timestamp": "2025-06-01T12:00:00Z", "usage": {"input_tokens": 100, "output_tokens": 200}},
    {"role": "user", "content": "hello", "timestamp": "2025-06-01T12:00:01Z"},
    {"role": "assistant", "content": "hi there", "reasoning_content": "Let me think carefully about this request. The user wants a simple greeting. This is a straightforward interaction so I should respond warmly and concisely. No need for complex reasoning here, just be friendly and direct. The context suggests a first interaction so I will keep it light and welcoming to set a positive tone for the conversation going forward. I should also consider whether to ask a follow-up question but since this is just a hello, a simple greeting back is the most appropriate response.", "timestamp": "2025-06-01T12:00:02Z"},
    {"role": "tool", "content": "ok", "timestamp": "2025-06-01T12:00:03Z"},
]
# Failure-heavy session
FAIL_S = [
    {"role": "session_meta", "timestamp": "2025-06-01T12:00:00Z"},
    {"role": "user", "content": "x", "timestamp": "2025-06-01T12:00:01Z"},
    {"role": "assistant", "content": "", "tool_calls": [{"function": {"name": "a"}}], "timestamp": "2025-06-01T12:00:02Z"},
    {"role": "tool", "content": '{"error":"boom"}', "is_error": True, "timestamp": "2025-06-01T12:00:03Z"},
    {"role": "assistant", "content": "", "tool_calls": [{"function": {"name": "b"}}], "timestamp": "2025-06-01T12:00:04Z"},
    {"role": "tool", "content": '{"success":false}', "timestamp": "2025-06-01T12:00:05Z"},
    {"role": "assistant", "content": "", "tool_calls": [{"function": {"name": "c"}}], "timestamp": "2025-06-01T12:00:06Z"},
    {"role": "tool", "content": "ok", "timestamp": "2025-06-01T12:00:07Z"},
]

# ─────── FORMAT DETECTION ───────
print("Format Detection")
print("─" * 40)
p = _jl(CLEAN)
T("detect hermes", lambda: at.detect_format(p) == "hermes"); _rm(p)
T("detect nonexistent", lambda: at.detect_format("/nonexistent_xyz") == "unknown")

p = tempfile.mkstemp(suffix=".json")[1]
json.dump({"model":"x","messages":[{"role":"user","content":[{"type":"text","text":"hi"}]}]}, open(p,"w"))
T("detect claude_code", lambda: at.detect_format(p) == "claude_code"); _rm(p)

p = tempfile.mkstemp(suffix=".json")[1]
json.dump({"contents":[{"role":"user","parts":[{"text":"hi"}]}]}, open(p,"w"))
T("detect gemini", lambda: at.detect_format(p) == "gemini"); _rm(p)

# ─────── PARSE ───────
print("\nParse")
print("─" * 40)
p = _jl(CLEAN); ev = at.parse(p)
T("parse len=4", lambda: len(ev) == 4)
T("parse first=session_meta", lambda: ev[0]["role"] == "session_meta"); _rm(p)

# ─────── ANALYZE — Basics ───────
print("\nAnalyze — Basics")
m = at.analyze(CLEAN)
T("user_messages=1",     lambda: m["user_messages"] == 1)
T("assistant_turns=1",   lambda: m["assistant_turns"] == 1)
T("tool_results=1",      lambda: m["tool_results"] == 1)
T("reasoning_blocks=1",  lambda: m["reasoning_blocks"] == 1)
T("reasoning_chars>0",   lambda: m["reasoning_total_chars"] > 0)
T("cost>=0",             lambda: m["cost_estimated"] >= 0)
T("duration>0",          lambda: m["session_duration_s"] > 0)
T("start not None",      lambda: m["session_start"] is not None)
T("end not None",        lambda: m["session_end"] is not None)

# ─────── ANALYZE — Dual Reasoning ───────
DR = [
    {"role": "session_meta", "timestamp": "2025-06-01T12:00:00Z"},
    {"role": "assistant", "content": "x", "reasoning": "r1", "timestamp": "2025-06-01T12:00:01Z"},
    {"role": "assistant", "content": "y", "reasoning_content": "r2", "timestamp": "2025-06-01T12:00:02Z"},
]
T("dual reasoning=2", lambda: at.analyze(DR)["reasoning_blocks"] == 2)

# ─────── ANALYZE — Tool Calls ───────
TC = [
    {"role": "session_meta", "timestamp": "2025-06-01T12:00:00Z"},
    {"role": "user", "content": "do", "timestamp": "2025-06-01T12:00:01Z"},
    {"role": "assistant", "content": "", "tool_calls": [
        {"function": {"name": "read"}}, {"function": {"name": "write"}}
    ], "timestamp": "2025-06-01T12:00:02Z"},
    {"role": "tool", "content": "a", "timestamp": "2025-06-01T12:00:03Z"},
    {"role": "tool", "content": "b", "timestamp": "2025-06-01T12:00:04Z"},
]
mtc = at.analyze(TC)
T("tc_total=2", lambda: mtc["tool_calls_total"] == 2)
T("tc_read=1",  lambda: mtc["tool_usage"]["read"] == 1)

# ─────── ANALYZE — Claude-style tool_call ───────
CC = [
    {"role": "session_meta", "timestamp": "2025-06-01T12:00:00Z"},
    {"role": "assistant", "content": "", "tool_call": {"name": "browser"}, "timestamp": "2025-06-01T12:00:01Z"},
]
T("claude tc=1", lambda: at.analyze(CC)["tool_calls_total"] == 1)

# ─────── ANALYZE — Failures ───────
mf = at.analyze(FAIL_S)
T("fail=2", lambda: mf["tool_calls_fail"] == 2)
T("ok=1",   lambda: mf["tool_calls_ok"] == 1)

# ─────── ANALYZE — Redacted ───────
RR = [
    {"role": "session_meta", "timestamp": "2025-06-01T12:00:00Z"},
    {"role": "user", "content": "x", "timestamp": "2025-06-01T12:00:01Z"},
    {"role": "assistant", "content": "y", "reasoning": "secret", "redacted": True, "timestamp": "2025-06-01T12:00:02Z"},
]
T("redacted=1", lambda: at.analyze(RR)["reasoning_redacted"] == 1)

# ─────── ANALYZE — Cache Tokens ───────
CT = [
    {"role": "session_meta", "timestamp": "2025-06-01T12:00:00Z",
     "usage": {"input_tokens": 100, "output_tokens": 200, "cache_creation_input_tokens": 50, "cache_read_input_tokens": 75}},
]
mct = at.analyze(CT)
T("cache_write=50", lambda: mct["tokens_cache_write"] == 50)
T("cache_read=75",  lambda: mct["tokens_cache_read"] == 75)

# ─────── ANALYZE — Empty ───────
T("empty users=0", lambda: at.analyze([])["user_messages"] == 0)

# ─────── ANALYZE — Model flag ───────
T("model gemini",    lambda: at.analyze(CLEAN, model="gemini-2.5-flash")["model_used"] == "gemini-2.5-flash")
T("model unknown ok", lambda: at.analyze(CLEAN, model="nonexistent")["cost_estimated"] >= 0)

# ─────── ANOMALIES — Clean ───────
a_clean = at.anomalies(m)
T("clean=0 anomalies", lambda: len(a_clean) == 0)  # CLEAN uses 1s gaps

# ─────── ANOMALIES — Various ───────
af = at.anomalies(mf)
T("anom tool_failures", lambda: any(x["type"] == "tool_failures" for x in af))
ar = at.anomalies(at.analyze(RR))
T("anom redaction", lambda: any(x["type"] == "redaction" for x in ar))

SS = [
    {"role": "session_meta", "timestamp": "2025-06-01T12:00:00Z"},
    {"role": "user", "content": "x", "timestamp": "2025-06-01T12:00:01Z"},
    {"role": "assistant", "content": "y", "reasoning": "ok", "timestamp": "2025-06-01T12:00:02Z"},
]
T("anom shallow", lambda: any(x["type"] == "shallow_thinking" for x in at.anomalies(at.analyze(SS))))

CH = [
    {"role": "session_meta", "timestamp": "2025-06-01T12:00:00Z"},
    {"role": "user", "content": "a", "timestamp": "2025-06-01T12:00:01Z"},
    {"role": "assistant", "content": "b", "timestamp": "2025-06-01T12:00:02Z"},
    {"role": "user", "content": "c", "timestamp": "2025-06-01T12:00:03Z"},
    {"role": "assistant", "content": "d", "timestamp": "2025-06-01T12:00:04Z"},
    {"role": "user", "content": "e", "timestamp": "2025-06-01T12:00:05Z"},
    {"role": "assistant", "content": "f", "timestamp": "2025-06-01T12:00:06Z"},
]
T("anom no_tools", lambda: any(x["type"] == "no_tools" for x in at.anomalies(at.analyze(CH))))

LG = [
    {"role": "session_meta", "timestamp": "2025-06-01T12:00:00Z"},
    {"role": "user", "content": "x", "timestamp": "2025-06-01T12:00:00Z"},
    {"role": "assistant", "content": "y", "timestamp": "2025-06-01T12:05:01Z"},
]
T("anom hanging", lambda: any(x["type"] == "hanging" for x in at.anomalies(at.analyze(LG))))

# ─────── HEALTH SCORE ───────
print("\nHealth Score")
T("perfect=100",    lambda: at.health_score(m, a_clean) == 100)
T("degraded<100",   lambda: at.health_score(mf, af) < 100)
T("clamped_zero",   lambda: at.health_score({}, [{"severity": "high"}] * 4) == 0)

# ─────── REPORTERS ───────
print("\nReporters — Text")
h100 = 100
r1 = at.report_text(m, a_clean, h100)
T("text TOKEN COST",  lambda: "TOKEN COST" in r1)
T("text ACTIVITY",    lambda: "ACTIVITY" in r1)
T("text LATENCY",     lambda: "LATENCY" in r1)
T("text HEALTH",      lambda: "HEALTH SCORE" in r1)
T("text version",     lambda: at.__version__ in r1)

meg = {"cost_estimated": 0.01, "model_used": "default", "user_messages": 1, "assistant_turns": 1,
       "tool_calls_total": 0, "tool_calls_ok": 0, "tool_calls_fail": 0, "tool_usage": Counter(),
       "tokens_input": 100, "tokens_output": 200, "tokens_cache_write": 0, "tokens_cache_read": 0,
       "reasoning_blocks": 0, "reasoning_total_chars": 0, "reasoning_lens": [], "reasoning_redacted": 0,
       "gaps_seconds": [], "session_duration_s": 0, "timestamps": []}
T("text no gap msg", lambda: "(no gap data)" in at.report_text(meg, [], 100))

print("\nReporters — JSON")
rj = at.report_json(m, a_clean, h100)
jd = json.loads(rj)
T("json version",     lambda: jd["version"] == at.__version__)
T("json health int",  lambda: isinstance(jd["health_score"], int))
T("json anomalies[]", lambda: isinstance(jd["anomalies"], list))
T("json tokens.total", lambda: "total" in jd["tokens"])

print("\nReporters — Compare")
rc = at.report_compare([("s1", m, a_clean, h100), ("s2", mf, af, 50)])
T("compare s1", lambda: "s1" in rc)
T("compare s2", lambda: "s2" in rc)

# ─────── UTILS ───────
print("\nUtils")
T("parse_ts Z",       lambda: at._parse_ts("2025-06-15T12:30:00Z") is not None)
T("parse_ts None",    lambda: at._parse_ts(None) is None)
T("parse_ts empty",   lambda: at._parse_ts("") is None)
T("safe_calc empty",  lambda: at._safe_calc([], sum) == 0.0)
T("safe_calc norm",   lambda: at._safe_calc([1, 2, 3], sum) == 6.0)
# _percentile: for 10 items, p50 → idx=floor(10*0.50)=5 → sorted[5]=6
T("p50",              lambda: at._percentile([1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 0.50) == 6)
# p95 → idx=floor(10*0.95)=9 → sorted[9]=10
T("p95",              lambda: at._percentile([1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 0.95) == 10)
T("pct empty",        lambda: at._percentile([], 0.5) == 0.0)
T("fmt_dur s",        lambda: "s" in at._fmt_duration(45))
T("fmt_dur m",        lambda: "m" in at._fmt_duration(120))
T("fmt_dur h",        lambda: "h" in at._fmt_duration(7200))

# ─────── PRICING ───────
print("\nPricing")
for k, v in at.PRICING.items():
    if k == "default":
        continue
    T(f"pricing {k}", lambda v=v: all(f in v for f in ("input", "output", "cw", "cr")))

# ─────── CLI INTEGRATION ───────
print("\nCLI Integration")
p = _jl(CLEAN)
res = subprocess.run([sys.executable, "-m", "agenttrace", p], capture_output=True, text=True, timeout=15)
T("cli exit 0",  lambda: res.returncode == 0)
T("cli HEALTH",  lambda: "HEALTH SCORE" in res.stdout)
_rm(p)

p = _jl(CLEAN)
res = subprocess.run([sys.executable, "-m", "agenttrace", "-f", "json", p], capture_output=True, text=True, timeout=15)
jd2 = json.loads(res.stdout)
T("cli json health_score", lambda: "health_score" in jd2)
T("cli json tokens",       lambda: jd2["tokens"]["total"] > 0)
_rm(p)

res = subprocess.run([sys.executable, "-m", "agenttrace", "--help"], capture_output=True, text=True, timeout=15)
T("cli help --latest",      lambda: "--latest" in res.stdout)
T("cli help --list-models", lambda: "--list-models" in res.stdout)

res = subprocess.run([sys.executable, "-m", "agenttrace", "--list-models"], capture_output=True, text=True, timeout=15)
T("cli list-models", lambda: "claude-sonnet" in res.stdout)

res = subprocess.run([sys.executable, "-m", "agenttrace", "/no/such/file.jsonl"], capture_output=True, text=True, timeout=15)
T("cli missing non-zero", lambda: res.returncode != 0)

p = _jl(CLEAN)
fd, op = tempfile.mkstemp(suffix=".txt"); os.close(fd)
res = subprocess.run([sys.executable, "-m", "agenttrace", "-o", op, p], capture_output=True, text=True, timeout=15)
T("cli -o saves", lambda: "HEALTH" in open(op).read())
_rm(p); _rm(op)

p = _jl(CLEAN)
res = subprocess.run([sys.executable, "-m", "agenttrace", "-m", "gemini-2.5-pro", p], capture_output=True, text=True, timeout=15)
T("cli -m flag", lambda: "gemini-2.5-pro" in res.stdout)
_rm(p)

# ─────── COMPARE + LATEST ───────
p1 = _jl(CLEAN)
p2 = _jl(FAIL_S)
os.makedirs("/tmp/at_test_sessions", exist_ok=True)
os.rename(p1, "/tmp/at_test_sessions/s1.jsonl")
os.rename(p2, "/tmp/at_test_sessions/s2.jsonl")
res = subprocess.run([sys.executable, "-m", "agenttrace", "--latest", "-d", "/tmp/at_test_sessions"], capture_output=True, text=True, timeout=15)
T("cli latest ok", lambda: res.returncode == 0)
res = subprocess.run([sys.executable, "-m", "agenttrace", "--compare", "-d", "/tmp/at_test_sessions"], capture_output=True, text=True, timeout=15)
T("cli compare both", lambda: "s1" in res.stdout and "s2" in res.stdout)
_rm("/tmp/at_test_sessions/s1.jsonl"); _rm("/tmp/at_test_sessions/s2.jsonl")

# ─────── REAL SESSION ───────
real = "/home/tenbox/.hermes/sessions/20260501_123658_f92feb.jsonl"
if os.path.exists(real):
    print("\nReal Session Integration")
    res = subprocess.run([sys.executable, "-m", "agenttrace", real], capture_output=True, text=True, timeout=15)
    T("real exit 0",    lambda: res.returncode == 0)
    T("real HEALTH",    lambda: "HEALTH SCORE" in res.stdout)
    res2 = subprocess.run([sys.executable, "-m", "agenttrace", "-f", "json", real], capture_output=True, text=True, timeout=15)
    jd3 = json.loads(res2.stdout)
    T("real tokens>0",  lambda: jd3["tokens"]["total"] > 0)
    T("real health int", lambda: isinstance(jd3["health_score"], int))
    T("real has model", lambda: len(jd3["model_used"]) > 0)
else:
    print("\nReal Session — SKIP (file not found)")

# ═════════════════════════════════════════════════════════════
print()
print("=" * 60)
total = PASS + FAIL
print(f"  RESULTS: {PASS}/{total} passed, {FAIL} failed")
print("=" * 60)
if FAILURES:
    for f in FAILURES:
        print(f"  ❌ {f}")
    sys.exit(1)
else:
    print("  ✅ ALL TESTS PASSED")
    sys.exit(0)
