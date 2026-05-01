#!/usr/bin/env python3
"""
agenttrace test suite — covers parsing, analysis, anomaly detection,
health scoring, reporting, and CLI integration.
Run with: pytest -v test_agenttrace.py
"""

import json
import os
import sys
import tempfile
from collections import Counter
from pathlib import Path

# Ensure we can import the module under test
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import agenttrace as at


# ═══════════════════════════════════════════════════════════
# TEST FIXTURES (as functions, for pytest-free runs too)
# ═══════════════════════════════════════════════════════════

HERMES_MINIMAL = [
    {"role": "session_meta", "timestamp": "2025-01-01T00:00:00Z", "usage": {"input_tokens": 100, "output_tokens": 200}},
    {"role": "user", "content": "hello", "timestamp": "2025-01-01T00:01:00Z"},
    {"role": "assistant", "content": "Hi there!", "reasoning_content": "thinking...", "timestamp": "2025-01-01T00:02:00Z"},
    {"role": "tool", "content": '{"result": "ok"}', "timestamp": "2025-01-01T00:03:00Z"},
]

HERMES_WITH_FAILURES = [
    {"role": "session_meta", "timestamp": "2025-01-01T00:00:00Z"},
    {"role": "user", "content": "do stuff", "timestamp": "2025-01-01T00:01:00Z"},
    {"role": "assistant", "content": "ok", "tool_calls": [{"function": {"name": "foo"}}], "timestamp": "2025-01-01T00:02:00Z"},
    {"role": "tool", "content": '{"error": "something broke"}', "is_error": True, "timestamp": "2025-01-01T00:03:00Z"},
    {"role": "assistant", "content": "retry", "tool_calls": [{"function": {"name": "bar"}}], "timestamp": "2025-01-01T00:04:00Z"},
    {"role": "tool", "content": '{"success": false}', "timestamp": "2025-01-01T00:05:00Z"},
    {"role": "assistant", "content": "done", "tool_calls": [{"function": {"name": "baz"}}], "timestamp": "2025-01-01T00:06:00Z"},
    {"role": "tool", "content": '{"result": "good"}', "timestamp": "2025-01-01T00:07:00Z"},
]

HERMES_SHALLOW = [
    {"role": "session_meta", "timestamp": "2025-01-01T00:00:00Z"},
    {"role": "user", "content": "x", "timestamp": "2025-01-01T00:01:00Z"},
    {"role": "assistant", "content": "y", "reasoning_content": "ok", "timestamp": "2025-01-01T00:02:00Z"},
]

HERMES_NO_REASONING = [
    {"role": "session_meta", "timestamp": "2025-01-01T00:00:00Z"},
    {"role": "user", "content": "q", "timestamp": "2025-01-01T00:01:00Z"},
    {"role": "assistant", "content": "a", "timestamp": "2025-01-01T00:02:00Z"},
]

HERMES_CHAT_ONLY = [
    {"role": "session_meta", "timestamp": "2025-01-01T00:00:00Z"},
    {"role": "user", "content": "hi", "timestamp": "2025-01-01T00:01:00Z"},
    {"role": "assistant", "content": "hello", "timestamp": "2025-01-01T00:02:00Z"},
    {"role": "user", "content": "how?", "timestamp": "2025-01-01T00:03:00Z"},
    {"role": "assistant", "content": "fine", "timestamp": "2025-01-01T00:04:00Z"},
    {"role": "user", "content": "bye", "timestamp": "2025-01-01T00:05:00Z"},
    {"role": "assistant", "content": "bye", "timestamp": "2025-01-01T00:06:00Z"},
]

HERMES_REDACTED = [
    {"role": "session_meta", "timestamp": "2025-01-01T00:00:00Z"},
    {"role": "user", "content": "test", "timestamp": "2025-01-01T00:01:00Z"},
    {"role": "assistant", "content": "ok", "reasoning": "secret thing", "redacted": True, "timestamp": "2025-01-01T00:02:00Z"},
]

HERMES_CLAUDE_STYLE_TOOL = [
    {"role": "session_meta", "timestamp": "2025-01-01T00:00:00Z"},
    {"role": "user", "content": "do", "timestamp": "2025-01-01T00:01:00Z"},
    {"role": "assistant", "content": "", "tool_call": {"id": "a1", "name": "read_file", "input": {"path": "x.py"}}, "timestamp": "2025-01-01T00:02:00Z"},
    {"role": "tool", "content": "line 1", "tool_call_id": "a1", "timestamp": "2025-01-01T00:03:00Z"},
]

HERMES_ALTERNATIVE_REASONING = [
    {"role": "session_meta", "timestamp": "2025-01-01T00:00:00Z"},
    {"role": "user", "content": "x", "timestamp": "2025-01-01T00:01:00Z"},
    {"role": "assistant", "content": "y", "reasoning": "via reasoning field", "timestamp": "2025-01-01T00:02:00Z"},
    {"role": "assistant", "content": "z", "reasoning_content": "via reasoning_content field", "timestamp": "2025-01-01T00:03:00Z"},
]

HERMES_LONG_GAP = [
    {"role": "session_meta", "timestamp": "2025-01-01T00:00:00Z"},
    {"role": "user", "content": "x", "timestamp": "2025-01-01T00:00:00Z"},
    {"role": "assistant", "content": "y", "timestamp": "2025-01-01T00:05:01Z"},  # 301s gap
]

CLAUDE_CODE_MINI = {
    "model": "claude-sonnet-4",
    "messages": [
        {"role": "user", "content": [{"type": "text", "text": "hello"}]},
        {"role": "assistant", "content": [
            {"type": "thinking", "thinking": "I should say hi", "redacted": False},
            {"type": "text", "text": "Hi!"},
        ]},
    ],
}

GEMINI_MINI = {
    "contents": [
        {"role": "user", "parts": [{"text": "hello"}]},
        {"role": "model", "parts": [{"text": "Hi there"}]},
    ],
}


def _write_jsonl(events, suffix=".jsonl"):
    """Write events to a temp JSONL file, return path."""
    fd, path = tempfile.mkstemp(suffix=suffix, text=True)
    with os.fdopen(fd, "w", encoding="utf-8") as f:
        for ev in events:
            f.write(json.dumps(ev) + "\n")
    return path


def _write_json(data, suffix=".json"):
    """Write dict to a temp JSON file, return path."""
    fd, path = tempfile.mkstemp(suffix=suffix, text=True)
    with os.fdopen(fd, "w", encoding="utf-8") as f:
        json.dump(data, f)
    return path


def _cleanup(path):
    """Remove a temp file."""
    if path and os.path.exists(path):
        os.unlink(path)


# ═══════════════════════════════════════════════════════════
# TESTS — format detection
# ═══════════════════════════════════════════════════════════

class TestDetectFormat:

    def test_hermes_detection(self):
        p = _write_jsonl(HERMES_MINIMAL)
        try:
            assert at.detect_format(p) == "hermes"
        finally:
            _cleanup(p)

    def test_claude_code_detection(self):
        p = _write_json(CLAUDE_CODE_MINI)
        try:
            assert at.detect_format(p) == "claude_code"
        finally:
            _cleanup(p)

    def test_gemini_detection(self):
        p = _write_json(GEMINI_MINI)
        try:
            assert at.detect_format(p) == "gemini"
        finally:
            _cleanup(p)

    def test_empty_file(self):
        fd, p = tempfile.mkstemp(suffix=".jsonl", text=True)
        os.close(fd)
        try:
            assert at.detect_format(p) == "unknown"
        finally:
            _cleanup(p)

    def test_garbage_file(self):
        p = _write_jsonl([{"notrole": 1}])
        try:
            assert at.detect_format(p) in ("unknown", "hermes")
        finally:
            _cleanup(p)


# ═══════════════════════════════════════════════════════════
# TESTS — parsing
# ═══════════════════════════════════════════════════════════

class TestParse:

    def test_hermes_parse_event_count(self):
        p = _write_jsonl(HERMES_MINIMAL)
        try:
            events = at.parse(p, fmt="hermes")
            assert len(events) == 4
        finally:
            _cleanup(p)

    def test_hermes_parse_auto_detect(self):
        p = _write_jsonl(HERMES_MINIMAL)
        try:
            events = at.parse(p)
            assert len(events) == 4
        finally:
            _cleanup(p)

    def test_claude_parse(self):
        p = _write_json(CLAUDE_CODE_MINI)
        try:
            events = at.parse(p, fmt="claude_code")
            assert len(events) >= 2
            roles = {e["role"] for e in events}
            assert "user" in roles
            assert "assistant" in roles
        finally:
            _cleanup(p)

    def test_gemini_parse(self):
        p = _write_json(GEMINI_MINI)
        try:
            events = at.parse(p, fmt="gemini")
            assert len(events) == 2
        finally:
            _cleanup(p)

    def test_generic_jsonl(self):
        p = _write_jsonl([{"a": 1}, {"b": 2}])
        try:
            events = at.parse(p, fmt="unknown")
            assert len(events) == 2
        finally:
            _cleanup(p)


# ═══════════════════════════════════════════════════════════
# TESTS — analysis engine
# ═══════════════════════════════════════════════════════════

class TestAnalyze:

    def test_basic_counts(self):
        m = at.analyze(HERMES_MINIMAL)
        assert m["user_messages"] == 1
        assert m["assistant_turns"] == 1
        assert m["tool_results"] == 1

    def test_token_estimation(self):
        m = at.analyze(HERMES_MINIMAL)
        assert m["tokens_input"] > 0
        assert m["tokens_output"] > 0

    def test_tool_calls_detected(self):
        m = at.analyze(HERMES_WITH_FAILURES)
        assert m["tool_calls_total"] == 3

    def test_tool_failure_detection(self):
        m = at.analyze(HERMES_WITH_FAILURES)
        assert m["tool_calls_fail"] == 2
        assert m["tool_calls_ok"] == 1

    def test_tool_usage_counter(self):
        m = at.analyze(HERMES_WITH_FAILURES)
        assert m["tool_usage"]["foo"] == 1
        assert m["tool_usage"]["bar"] == 1

    def test_reasoning_extraction(self):
        m = at.analyze(HERMES_MINIMAL)
        assert m["reasoning_blocks"] == 1
        assert m["reasoning_total_chars"] > 0

    def test_reasoning_both_fields(self):
        """Should handle both 'reasoning' and 'reasoning_content'."""
        m = at.analyze(HERMES_ALTERNATIVE_REASONING)
        assert m["reasoning_blocks"] == 2

    def test_no_reasoning(self):
        m = at.analyze(HERMES_NO_REASONING)
        assert m["reasoning_blocks"] == 0
        assert m["reasoning_total_chars"] == 0

    def test_claude_tool_call_parsing(self):
        m = at.analyze(HERMES_CLAUDE_STYLE_TOOL)
        assert m["tool_calls_total"] == 1
        assert m["tool_results"] == 1

    def test_cost_estimation(self):
        m = at.analyze(HERMES_MINIMAL)
        assert m["cost_estimated"] >= 0

    def test_cost_with_cache(self):
        events = [
            {"role": "session_meta", "usage": {"input_tokens": 1000, "output_tokens": 500, "cache_creation_input_tokens": 200, "cache_read_input_tokens": 300}},
        ]
        m = at.analyze(events)
        assert m["tokens_cache_write"] == 200
        assert m["tokens_cache_read"] == 300

    def test_timestamps_extracted(self):
        m = at.analyze(HERMES_MINIMAL)
        assert len(m["timestamps"]) == 3  # session_meta excluded
        assert m["session_start"] is not None
        assert m["session_end"] is not None
        assert m["session_duration_s"] > 0

    def test_empty_events(self):
        m = at.analyze([])
        assert m["events_total"] == 0
        assert m["user_messages"] == 0

    def test_model_passed(self):
        m = at.analyze(HERMES_MINIMAL, model="gemini-2.5-flash")
        assert m["model_used"] == "gemini-2.5-flash"

    def test_redacted_reasoning(self):
        m = at.analyze(HERMES_REDACTED)
        assert m["reasoning_redacted"] == 1


# ═══════════════════════════════════════════════════════════
# TESTS — anomaly detection
# ═══════════════════════════════════════════════════════════

class TestAnomalies:

    def test_no_anomalies_on_clean_session(self):
        m = at.analyze(HERMES_MINIMAL)
        anoms = at.anomalies(m)
        assert len(anoms) == 0

    def test_tool_failure_anomaly(self):
        m = at.analyze(HERMES_WITH_FAILURES)
        anoms = at.anomalies(m)
        types = {a["type"] for a in anoms}
        assert "tool_failures" in types
        tf = [a for a in anoms if a["type"] == "tool_failures"][0]
        assert tf["severity"] in ("high", "medium")

    def test_shallow_thinking_anomaly(self):
        m = at.analyze(HERMES_SHALLOW)
        anoms = at.anomalies(m)
        assert any(a["type"] == "shallow_thinking" for a in anoms)

    def test_chat_only_anomaly(self):
        m = at.analyze(HERMES_CHAT_ONLY)
        anoms = at.anomalies(m)
        assert any(a["type"] == "no_tools" for a in anoms)

    def test_long_gap_anomaly(self):
        m = at.analyze(HERMES_LONG_GAP)
        anoms = at.anomalies(m)
        assert any(a["type"] == "hanging" for a in anoms)

    def test_redaction_anomaly(self):
        m = at.analyze(HERMES_REDACTED)
        anoms = at.anomalies(m)
        assert any(a["type"] == "redaction" for a in anoms)


# ═══════════════════════════════════════════════════════════
# TESTS — health score
# ═══════════════════════════════════════════════════════════

class TestHealthScore:

    def test_perfect_health(self):
        m = at.analyze(HERMES_MINIMAL)
        anoms = at.anomalies(m)
        h = at.health_score(m, anoms)
        assert h == 100

    def test_degraded_health(self):
        m = at.analyze(HERMES_WITH_FAILURES)
        anoms = at.anomalies(m)
        h = at.health_score(m, anoms)
        assert h < 100

    def test_health_clamped_to_zero(self):
        h = at.health_score({}, [{"severity": "high"}, {"severity": "high"}, {"severity": "high"}, {"severity": "high"}])
        assert h == 0


# ═══════════════════════════════════════════════════════════
# TESTS — reporters
# ═══════════════════════════════════════════════════════════

class TestReporters:

    def test_text_report_contains_sections(self):
        m = at.analyze(HERMES_MINIMAL)
        anoms = at.anomalies(m)
        h = at.health_score(m, anoms)
        r = at.report_text(m, anoms, h)
        assert "TOKEN COST" in r
        assert "ACTIVITY" in r
        assert "LATENCY" in r
        assert "HEALTH SCORE" in r
        assert str(m["cost_estimated"]) in r

    def test_text_report_handles_no_gaps(self):
        m = {
            "cost_estimated": 0.01,
            "model_used": "default",
            "user_messages": 1,
            "assistant_turns": 1,
            "tool_calls_total": 0,
            "tool_calls_ok": 0,
            "tool_calls_fail": 0,
            "tool_usage": Counter(),
            "tokens_input": 100,
            "tokens_output": 200,
            "tokens_cache_write": 0,
            "tokens_cache_read": 0,
            "reasoning_blocks": 0,
            "reasoning_total_chars": 0,
            "reasoning_redacted": 0,
            "gaps_seconds": [],
            "session_duration_s": 0,
        }
        r = at.report_text(m, [], 100)
        assert "(no gap data)" in r

    def test_json_report_valid(self):
        m = at.analyze(HERMES_MINIMAL)
        anoms = at.anomalies(m)
        h = at.health_score(m, anoms)
        j = at.report_json(m, anoms, h)
        data = json.loads(j)
        assert data["version"] == at.__version__
        assert isinstance(data["tokens"]["total"], int)
        assert isinstance(data["anomalies"], list)
        assert isinstance(data["health_score"], int)

    def test_compare_report(self):
        m1 = at.analyze(HERMES_MINIMAL)
        a1 = at.anomalies(m1)
        h1 = at.health_score(m1, a1)

        m2 = at.analyze(HERMES_WITH_FAILURES)
        a2 = at.anomalies(m2)
        h2 = at.health_score(m2, a2)

        sessions = [("sess1", m1, a1, h1), ("sess2", m2, a2, h2)]
        r = at.report_compare(sessions)
        assert "sess1" in r
        assert "sess2" in r
        assert "Health" in r


# ═══════════════════════════════════════════════════════════
# TESTS — pricing & model detection
# ═══════════════════════════════════════════════════════════

class TestPricing:

    def test_all_models_have_pricing(self):
        for model, rates in at.PRICING.items():
            if model == "default":
                continue
            assert "input" in rates
            assert "output" in rates
            assert rates["input"] > 0
            assert rates["output"] > 0

    def test_default_exists(self):
        assert "default" in at.PRICING

    def test_unknown_model_uses_default(self):
        m = at.analyze(HERMES_MINIMAL, model="nonexistent-model")
        # Should not crash — uses default pricing
        assert m["cost_estimated"] >= 0


# ═══════════════════════════════════════════════════════════
# TESTS — utility functions
# ═══════════════════════════════════════════════════════════

class TestUtils:

    def test_parse_ts_iso(self):
        dt = at._parse_ts("2025-06-15T12:30:00Z")
        assert dt is not None
        assert dt.month == 6

    def test_parse_ts_none(self):
        assert at._parse_ts(None) is None
        assert at._parse_ts("") is None

    def test_parse_ts_no_tz(self):
        dt = at._parse_ts("2025-06-15T12:30:00")
        assert dt is not None

    def test_safe_calc(self):
        assert at._safe_calc([], sum) == 0.0
        assert at._safe_calc([1, 2, 3], sum) == 6.0

    def test_percentile(self):
        vals = sorted([1, 2, 3, 4, 5, 6, 7, 8, 9, 10])
        assert at._percentile(vals, 0.50) == 5
        assert at._percentile(vals, 0.95) == 9
        assert at._percentile(vals, 0.0) == 1

    def test_percentile_empty(self):
        assert at._percentile([], 0.50) == 0.0

    def test_fmt_duration(self):
        assert "s" in at._fmt_duration(45)
        assert "m" in at._fmt_duration(120)
        assert "h" in at._fmt_duration(7200)


# ═══════════════════════════════════════════════════════════
# TESTS — CLI integration (integration tests)
# ═══════════════════════════════════════════════════════════

class TestCLI:

    def test_cli_text_output(self):
        """Run agenttrace as a subprocess on a temp file."""
        import subprocess
        p = _write_jsonl(HERMES_MINIMAL)
        try:
            result = subprocess.run(
                [sys.executable, "agenttrace.py", p],
                capture_output=True, text=True, timeout=10,
            )
            assert result.returncode == 0
            assert "HEALTH SCORE" in result.stdout
        finally:
            _cleanup(p)

    def test_cli_json_output(self):
        import subprocess
        p = _write_jsonl(HERMES_MINIMAL)
        try:
            result = subprocess.run(
                [sys.executable, "agenttrace.py", "-f", "json", p],
                capture_output=True, text=True, timeout=10,
            )
            assert result.returncode == 0
            data = json.loads(result.stdout)
            assert "health_score" in data
        finally:
            _cleanup(p)

    def test_cli_help(self):
        import subprocess
        result = subprocess.run(
            [sys.executable, "agenttrace.py", "--help"],
            capture_output=True, text=True, timeout=10,
        )
        assert result.returncode == 0
        assert "--latest" in result.stdout

    def test_cli_list_models(self):
        import subprocess
        result = subprocess.run(
            [sys.executable, "agenttrace.py", "--list-models"],
            capture_output=True, text=True, timeout=10,
        )
        assert result.returncode == 0
        assert "claude-sonnet" in result.stdout

    def test_cli_nonexistent_file(self):
        import subprocess
        result = subprocess.run(
            [sys.executable, "agenttrace.py", "/no/such/file.jsonl"],
            capture_output=True, text=True, timeout=10,
        )
        assert result.returncode != 0

    def test_cli_output_to_file(self):
        import subprocess
        fd, outpath = tempfile.mkstemp(suffix=".txt", text=True)
        os.close(fd)
        p = _write_jsonl(HERMES_MINIMAL)
        try:
            result = subprocess.run(
                [sys.executable, "agenttrace.py", "-o", outpath, p],
                capture_output=True, text=True, timeout=10,
            )
            assert result.returncode == 0
            with open(outpath) as f:
                assert "HEALTH" in f.read()
        finally:
            _cleanup(p)
            _cleanup(outpath)

    def test_cli_model_flag(self):
        import subprocess
        p = _write_jsonl(HERMES_MINIMAL)
        try:
            result = subprocess.run(
                [sys.executable, "agenttrace.py", "-m", "gemini-2.5-pro", p],
                capture_output=True, text=True, timeout=10,
            )
            assert result.returncode == 0
            assert "gemini-2.5-pro" in result.stdout
        finally:
            _cleanup(p)


# ═══════════════════════════════════════════════════════════
# RUNNER
# ═══════════════════════════════════════════════════════════

if __name__ == "__main__":
    import subprocess
    # Delegate to pytest if available
    result = subprocess.run(
        [sys.executable, "-m", "pytest", "-v", "--tb=short", __file__],
        timeout=60,
    )
    sys.exit(result.returncode)
