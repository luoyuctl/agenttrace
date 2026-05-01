#!/usr/bin/env python3
"""
agenttrace v3 — Full Test Suite
Tests: parsing, analysis, anomaly detection, health score, reporting, CLI
Pure stdlib — no pip install needed.
"""

import json
import os
import sys
import tempfile
import unittest
from io import StringIO

# Add parent to path so we can import agenttrace
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
import agenttrace as at


# ═══════════════════════════════════════════════════════════════
# TEST FIXTURES
# ═══════════════════════════════════════════════════════════════

HERMES_JSONL = """\
{"role": "user", "content": "Hello, write a script", "timestamp": "2026-05-01T10:00:00"}
{"role": "assistant", "content": "", "reasoning_content": "I need to write a Python script that does X", "tool_calls": [{"id": "call_1", "type": "function", "function": {"name": "terminal", "arguments": "{\\"command\\": \\"echo hello\\"}"}}], "timestamp": "2026-05-01T10:00:15"}
{"role": "tool", "content": "hello\\n", "tool_call_id": "call_1", "timestamp": "2026-05-01T10:00:16"}
{"role": "assistant", "content": "Here is your script:", "reasoning_content": "Script is ready", "timestamp": "2026-05-01T10:00:18"}
{"role": "user", "content": "Add error handling", "timestamp": "2026-05-01T10:01:00"}
{"role": "assistant", "content": "", "reasoning_content": "Adding try/except blocks", "tool_calls": [{"id": "call_2", "type": "function", "function": {"name": "write_file", "arguments": "{\\"path\\": \\"/tmp/test.py\\", \\"content\\": \\"print(1)\\"}"}}], "timestamp": "2026-05-01T10:01:20"}
{"role": "tool", "content": "file written", "tool_call_id": "call_2", "timestamp": "2026-05-01T10:01:21"}
"""

HERMES_JSONL_WITH_ERRORS = """\
{"role": "user", "content": "Delete prod", "timestamp": "2026-05-01T10:00:00"}
{"role": "assistant", "content": "", "reasoning_content": "ok", "tool_calls": [{"id": "call_1", "type": "function", "function": {"name": "terminal", "arguments": "{\\"command\\": \\"rm -rf /\\"}"}}], "timestamp": "2026-05-01T10:00:05"}
{"role": "tool", "content": "{\\"exit_code\\": 1, \\"error\\": \\"Permission denied\\"}", "tool_call_id": "call_1", "timestamp": "2026-05-01T10:00:06"}
{"role": "assistant", "content": "", "reasoning_content": "hmm", "tool_calls": [{"id": "call_2", "type": "function", "function": {"name": "terminal", "arguments": "{\\"command\\": \\"sudo rm -rf /\\"}"}}], "timestamp": "2026-05-01T10:00:10"}
{"role": "tool", "content": "command failed", "tool_call_id": "call_2", "timestamp": "2026-05-01T10:00:11", "is_error": true}
"""

HERMES_JSONL_WITH_HANG = """\
{"role": "user", "content": "Do X", "timestamp": "2026-05-01T10:00:00"}
{"role": "assistant", "content": "", "reasoning_content": "Let me think about this carefully and deeply", "timestamp": "2026-05-01T10:00:10"}
{"role": "assistant", "content": "I will now execute", "tool_calls": [{"id": "call_1", "type": "function", "function": {"name": "read_file", "arguments": "{\\"path\\": \\"/tmp/x\\"}"}}], "reasoning_content": "Still thinking about the best approach", "timestamp": "2026-05-01T10:05:00"}
{"role": "tool", "content": "file contents", "tool_call_id": "call_1", "timestamp": "2026-05-01T10:05:01"}
"""

CLAUDE_CODE_JSON = """\
{
  "model": "claude-sonnet-4-20250514",
  "usage": {
    "input_tokens": 1200,
    "output_tokens": 800,
    "cache_read_input_tokens": 300
  },
  "messages": [
    {"role": "user", "content": [{"type": "text", "text": "Write a function"}]},
    {"role": "assistant", "content": [
      {"type": "thinking", "thinking": "I should write a function that does...", "redacted": false},
      {"type": "text", "text": "Here is the function:\\n```python\\ndef foo(): pass```"},
      {"type": "tool_use", "id": "toolu_1", "name": "write_file", "input": {"path": "/tmp/foo.py", "content": "def foo(): pass"}}
    ]},
    {"role": "tool", "content": "File written successfully", "tool_use_id": "toolu_1", "is_error": false}
  ]
}"""

GEMINI_JSON = """\
{
  "contents": [
    {"role": "user", "parts": [{"text": "Explain AI"}]},
    {"role": "model", "parts": [{"text": "AI stands for Artificial Intelligence."}]},
    {"role": "user", "parts": [{"text": "More details"}]},
    {"role": "model", "parts": [{"text": "AI is a broad field encompassing machine learning, neural networks, and more."}]}
  ]
}"""


# ═══════════════════════════════════════════════════════════════
# TESTS: Format Detection
# ═══════════════════════════════════════════════════════════════

class TestFormatDetection(unittest.TestCase):

    def test_detect_hermes(self):
        path = _write_temp(HERMES_JSONL)
        self.assertEqual(at.detect_format(path), "hermes")

    def test_detect_claude(self):
        path = _write_temp(CLAUDE_CODE_JSON)
        self.assertEqual(at.detect_format(path), "claude_code")

    def test_detect_gemini(self):
        path = _write_temp(GEMINI_JSON)
        self.assertEqual(at.detect_format(path), "gemini")

    def test_detect_empty_file(self):
        path = _write_temp("")
        self.assertEqual(at.detect_format(path), "unknown")

    def test_detect_nonexistent_file(self):
        self.assertEqual(at.detect_format("/nonexistent/file.jsonl"), "unknown")


# ═══════════════════════════════════════════════════════════════
# TESTS: Parsing
# ═══════════════════════════════════════════════════════════════

class TestParsing(unittest.TestCase):

    def test_parse_hermes(self):
        path = _write_temp(HERMES_JSONL)
        events = at.parse(path, "hermes")
        self.assertEqual(len(events), 7)
        roles = [e["role"] for e in events]
        self.assertEqual(roles, ["user", "assistant", "tool", "assistant", "user", "assistant", "tool"])

    def test_parse_hermes_auto(self):
        path = _write_temp(HERMES_JSONL)
        events = at.parse(path)  # auto-detect
        self.assertEqual(len(events), 7)

    def test_parse_claude(self):
        path = _write_temp(CLAUDE_CODE_JSON)
        events = at.parse(path, "claude_code")
        self.assertGreater(len(events), 0)
        # Should have a meta event
        meta = [e for e in events if e["role"] == "meta"]
        self.assertEqual(len(meta), 1)
        self.assertEqual(meta[0]["usage"]["input_tokens"], 1200)

    def test_parse_claude_auto(self):
        path = _write_temp(CLAUDE_CODE_JSON)
        events = at.parse(path)
        self.assertGreater(len(events), 0)

    def test_parse_gemini(self):
        path = _write_temp(GEMINI_JSON)
        events = at.parse(path, "gemini")
        self.assertEqual(len(events), 4)

    def test_parse_generic(self):
        path = _write_temp(HERMES_JSONL)
        events = at.parse(path, "generic")  # fallback path
        self.assertEqual(len(events), 7)

    def test_parse_unknown(self):
        path = _write_temp("not json at all\njust text\n")
        events = at.parse(path, "unknown")
        self.assertEqual(len(events), 0)


# ═══════════════════════════════════════════════════════════════
# TESTS: Analysis Engine
# ═══════════════════════════════════════════════════════════════

class TestAnalysis(unittest.TestCase):

    def test_analyze_hermes_basic(self):
        events = at.parse(_write_temp(HERMES_JSONL), "hermes")
        m = at.analyze(events)
        self.assertEqual(m["user_msgs"], 2)
        self.assertEqual(m["turns"], 3)
        self.assertEqual(m["tool_total"], 2)
        self.assertEqual(m["tool_ok"], 2)
        self.assertEqual(m["tool_fail"], 0)

    def test_analyze_detects_tool_errors(self):
        events = at.parse(_write_temp(HERMES_JSONL_WITH_ERRORS), "hermes")
        m = at.analyze(events)
        self.assertEqual(m["tool_total"], 2)
        self.assertGreater(m["tool_fail"], 0, "Should detect at least one tool failure")

    def test_analyze_reasoning_stats(self):
        events = at.parse(_write_temp(HERMES_JSONL), "hermes")
        m = at.analyze(events)
        self.assertGreater(len(m["reasoning_lens"]), 0)

    def test_analyze_gaps(self):
        events = at.parse(_write_temp(HERMES_JSONL_WITH_HANG), "hermes")
        m = at.analyze(events)
        self.assertGreater(len(m["gaps"]), 0)
        self.assertGreater(max(m["gaps"]), 60, "Should detect hang > 60s")

    def test_analyze_tokens_estimated(self):
        events = at.parse(_write_temp(HERMES_JSONL), "hermes")
        m = at.analyze(events)
        self.assertGreater(m["tokens"]["input"], 0)
        self.assertGreater(m["tokens"]["output"], 0)

    def test_cost_calculation(self):
        events = at.parse(_write_temp(HERMES_JSONL), "hermes")
        m = at.analyze(events, "gpt-4o-mini")
        self.assertGreater(m["cost"], 0)
        self.assertLess(m["cost"], 1.0, "Should be cheap for a short session")

    def test_cost_claude_opus(self):
        events = at.parse(_write_temp(HERMES_JSONL), "hermes")
        m = at.analyze(events, "claude-opus-4")
        # Opus is expensive — cost should be higher than default
        m2 = at.analyze(events, "gpt-4o-mini")
        self.assertGreater(m["cost"], m2["cost"])

    def test_default_model_cost(self):
        events = at.parse(_write_temp(HERMES_JSONL), "hermes")
        m = at.analyze(events)
        self.assertIsInstance(m["cost"], float)

    def test_claude_meta_usage(self):
        events = at.parse(_write_temp(CLAUDE_CODE_JSON), "claude_code")
        m = at.analyze(events)
        self.assertEqual(m["tokens"]["input"], 1200)
        self.assertEqual(m["tokens"]["output"], 800)

    def test_empty_events(self):
        m = at.analyze([])
        self.assertEqual(m["user_msgs"], 0)
        self.assertEqual(m["cost"], 0.0)


# ═══════════════════════════════════════════════════════════════
# TESTS: Tool Error Detection
# ═══════════════════════════════════════════════════════════════

class TestToolErrorDetection(unittest.TestCase):

    def test_json_exit_code(self):
        self.assertTrue(at._check_tool_error('{"exit_code": 1}'))
        self.assertFalse(at._check_tool_error('{"exit_code": 0}'))

    def test_json_success_false(self):
        self.assertTrue(at._check_tool_error('{"success": false}'))
        self.assertFalse(at._check_tool_error('{"success": true}'))

    def test_json_error_field(self):
        self.assertTrue(at._check_tool_error('{"error": "something broke"}'))
        self.assertFalse(at._check_tool_error('{"result": "ok"}'))

    def test_dict_exit_code(self):
        self.assertTrue(at._check_tool_error({"exit_code": 1}))
        self.assertFalse(at._check_tool_error({"exit_code": 0}))

    def test_dict_error_field(self):
        self.assertTrue(at._check_tool_error({"error": "bad"}))
        self.assertTrue(at._check_tool_error({"is_error": True}))

    def test_plain_text_error(self):
        self.assertTrue(at._check_tool_error("Error: something failed"))
        self.assertTrue(at._check_tool_error("Traceback (most recent call last)"))

    def test_plain_text_ok(self):
        self.assertFalse(at._check_tool_error("All good"))
        self.assertFalse(at._check_tool_error("[todo] updated task list"))

    def test_non_string_non_dict(self):
        self.assertFalse(at._check_tool_error(42))
        self.assertFalse(at._check_tool_error(None))


# ═══════════════════════════════════════════════════════════════
# TESTS: Anomaly Detection
# ═══════════════════════════════════════════════════════════════

class TestAnomalyDetection(unittest.TestCase):

    def test_no_anomalies_clean_session(self):
        m = _clean_metrics()
        anoms = at.anomalies(m)
        self.assertEqual(len(anoms), 0)

    def test_detects_hanging(self):
        m = _clean_metrics()
        m["gaps"] = [5.0, 12.0, 120.0, 3.0]  # one >60s
        anoms = at.anomalies(m)
        hanging = [a for a in anoms if a["type"] == "hanging"]
        self.assertEqual(len(hanging), 1)

    def test_detects_critical_hanging(self):
        m = _clean_metrics()
        m["gaps"] = [5.0, 600.0]  # gap >300s
        anoms = at.anomalies(m)
        hanging = [a for a in anoms if a["type"] == "hanging"]
        self.assertEqual(len(hanging), 1)
        self.assertEqual(hanging[0]["sev"], "high")

    def test_detects_tool_failures(self):
        m = _clean_metrics()
        m["tool_ok"] = 2
        m["tool_fail"] = 3  # 60% failure
        anoms = at.anomalies(m)
        fail = [a for a in anoms if a["type"] == "tool_failures"]
        self.assertEqual(len(fail), 1)
        self.assertEqual(fail[0]["sev"], "high")

    def test_medium_tool_failures(self):
        m = _clean_metrics()
        m["tool_ok"] = 7
        m["tool_fail"] = 3  # 30%
        anoms = at.anomalies(m)
        fail = [a for a in anoms if a["type"] == "tool_failures"]
        self.assertEqual(fail[0]["sev"], "medium")

    def test_detects_very_shallow_thinking(self):
        m = _clean_metrics()
        m["reasoning_lens"] = [80, 120, 90, 150]  # avg ~110
        anoms = at.anomalies(m)
        shallow = [a for a in anoms if a["type"] == "shallow_thinking"]
        self.assertEqual(len(shallow), 1)
        self.assertEqual(shallow[0]["sev"], "high")

    def test_detects_shallow_thinking(self):
        m = _clean_metrics()
        m["reasoning_lens"] = [300, 400, 350, 450]  # avg ~375
        anoms = at.anomalies(m)
        shallow = [a for a in anoms if a["type"] == "shallow_thinking"]
        self.assertEqual(len(shallow), 1)
        self.assertEqual(shallow[0]["sev"], "medium")

    def test_detects_thinking_redaction(self):
        m = _clean_metrics()
        m["thinking_redacted"] = 2
        anoms = at.anomalies(m)
        redacted = [a for a in anoms if a["type"] == "thinking_redacted"]
        self.assertEqual(len(redacted), 1)

    def test_slow_latency_warning(self):
        m = _clean_metrics()
        m["gaps"] = [40.0, 45.0, 50.0, 35.0]  # p95 > 30
        anoms = at.anomalies(m)
        slow = [a for a in anoms if a["type"] == "slow"]
        self.assertEqual(len(slow), 1)


# ═══════════════════════════════════════════════════════════════
# TESTS: Health Score
# ═══════════════════════════════════════════════════════════════

class TestHealthScore(unittest.TestCase):

    def test_perfect_health(self):
        m = _clean_metrics()
        anoms = at.anomalies(m)
        self.assertEqual(at.health_score(m, anoms), 100)

    def test_health_with_high_severity(self):
        m = _clean_metrics()
        anoms = [{"type": "hanging", "sev": "high", "detail": "test"}]
        self.assertEqual(at.health_score(m, anoms), 75)

    def test_health_multiple_anomalies(self):
        m = _clean_metrics()
        anoms = [
            {"type": "hanging", "sev": "high", "detail": "test"},
            {"type": "shallow_thinking", "sev": "medium", "detail": "test"},
            {"type": "tool_failures", "sev": "low", "detail": "test"},
        ]
        self.assertEqual(at.health_score(m, anoms), 60)

    def test_health_floor_zero(self):
        m = _clean_metrics()
        anoms = [{"type": "x", "sev": "high", "detail": "t"} for _ in range(10)]
        self.assertEqual(at.health_score(m, anoms), 0)

    def test_health_ceiling_100(self):
        m = _clean_metrics()
        self.assertEqual(at.health_score(m, []), 100)


# ═══════════════════════════════════════════════════════════════
# TESTS: Report Generation
# ═══════════════════════════════════════════════════════════════

class TestReports(unittest.TestCase):

    def test_report_text_has_sections(self):
        events = at.parse(_write_temp(HERMES_JSONL), "hermes")
        m = at.analyze(events)
        anoms = at.anomalies(m)
        r = at.report_text(m, anoms)
        self.assertIn("AGENTTRACE v3", r)
        self.assertIn("TOKEN COST", r)
        self.assertIn("ACTIVITY", r)
        self.assertIn("HEALTH", r)

    def test_report_text_with_model(self):
        events = at.parse(_write_temp(HERMES_JSONL), "hermes")
        m = at.analyze(events, "claude-opus-4")
        anoms = at.anomalies(m)
        r = at.report_text(m, anoms)
        self.assertIn("claude-opus-4", r)

    def test_report_text_no_anomalies_shows_ok(self):
        m = _clean_metrics()
        anoms = []
        r = at.report_text(m, anoms)
        self.assertIn("No anomalies detected", r)

    def test_report_json(self):
        events = at.parse(_write_temp(HERMES_JSONL), "hermes")
        m = at.analyze(events)
        anoms = at.anomalies(m)
        r = at.report_json(m, anoms)
        data = json.loads(r)
        self.assertEqual(data["agenttrace_version"], "3.0.0")
        self.assertIn("metrics", data)
        self.assertIn("anomalies", data)

    def test_report_json_valid_output(self):
        events = at.parse(_write_temp(HERMES_JSONL), "hermes")
        m = at.analyze(events)
        anoms = at.anomalies(m)
        r = at.report_json(m, anoms)
        # Must be valid JSON
        json.loads(r)

    def test_compare_report(self):
        sessions = [
            ("session_a", _clean_metrics(user_msgs=3, turns=10, tools=15), []),
            ("session_b", _clean_metrics(user_msgs=5, turns=25, tools=40), []),
        ]
        r = at.compare_report(sessions)
        self.assertIn("Multi-Session Comparison", r)
        self.assertIn("session_a", r)
        self.assertIn("session_b", r)


# ═══════════════════════════════════════════════════════════════
# TESTS: Utility Functions
# ═══════════════════════════════════════════════════════════════

class TestUtilities(unittest.TestCase):

    def test_parse_ts_iso(self):
        ts = at._parse_ts("2026-05-01T10:00:00")
        self.assertIsNotNone(ts)
        self.assertEqual(ts.year, 2026)

    def test_parse_ts_with_z(self):
        ts = at._parse_ts("2026-05-01T10:00:00Z")
        self.assertIsNotNone(ts)

    def test_parse_ts_empty(self):
        self.assertIsNone(at._parse_ts(""))
        self.assertIsNone(at._parse_ts(None))

    def test_parse_ts_invalid(self):
        self.assertIsNone(at._parse_ts("not a timestamp"))

    def test_estimate_tokens(self):
        self.assertEqual(at._estimate_tokens(""), 0)
        self.assertEqual(at._estimate_tokens("1234"), 1)
        self.assertEqual(at._estimate_tokens("a" * 400), 100)
        self.assertEqual(at._estimate_tokens(None), 0)

    def test_percentile(self):
        self.assertEqual(at._pct([1.0, 2.0, 3.0, 4.0, 5.0], 50), 3.0)
        self.assertEqual(at._pct([1.0, 2.0, 3.0, 4.0, 5.0], 100), 5.0)
        self.assertEqual(at._pct([1.0, 2.0, 3.0, 4.0, 5.0], 0), 1.0)

    def test_p50_p95_p99(self):
        vals = list(range(1, 101))
        self.assertEqual(at._p50(vals), 50.0)
        self.assertEqual(at._p95(vals), 95.0)
        self.assertEqual(at._p99(vals), 99.0)

    def test_percentile_empty(self):
        self.assertEqual(at._pct([], 50), 0)
        self.assertEqual(at._p50([]), 0)
        self.assertEqual(at._p95([]), 0)


# ═══════════════════════════════════════════════════════════════
# TESTS: CLI Argument Parsing
# ═══════════════════════════════════════════════════════════════

class TestCLI(unittest.TestCase):

    def test_cli_version(self):
        args = _parse_args(["--version"])
        self.assertTrue(args.version)

    def test_cli_list_models(self):
        args = _parse_args(["--list-models"])
        self.assertTrue(args.list_models)

    def test_cli_latest(self):
        args = _parse_args(["--latest"])
        self.assertTrue(args.latest)

    def test_cli_compare(self):
        args = _parse_args(["--compare"])
        self.assertTrue(args.compare)

    def test_cli_all(self):
        args = _parse_args(["--all"])
        self.assertTrue(args.all)

    def test_cli_format_json(self):
        args = _parse_args(["--format", "json"])
        self.assertEqual(args.format, "json")

    def test_cli_model(self):
        args = _parse_args(["--model", "gpt-4o"])
        self.assertEqual(args.model, "gpt-4o")

    def test_cli_output(self):
        args = _parse_args(["--output", "/tmp/test.txt"])
        self.assertEqual(args.output, "/tmp/test.txt")

    def test_cli_limit(self):
        args = _parse_args(["--limit", "5"])
        self.assertEqual(args.limit, 5)

    def test_cli_with_path(self):
        args = _parse_args(["/some/file.jsonl"])
        self.assertEqual(args.path, "/some/file.jsonl")

    def test_cli_combined(self):
        args = _parse_args(["--latest", "-f", "json", "-m", "claude-sonnet-4", "-o", "/tmp/x.json"])
        self.assertTrue(args.latest)
        self.assertEqual(args.format, "json")
        self.assertEqual(args.model, "claude-sonnet-4")
        self.assertEqual(args.output, "/tmp/x.json")


# ═══════════════════════════════════════════════════════════════
# TESTS: Pricing Data Integrity
# ═══════════════════════════════════════════════════════════════

class TestPricing(unittest.TestCase):

    def test_all_models_have_required_fields(self):
        for model, p in at.PRICING.items():
            self.assertIn("input", p, f"{model} missing input")
            self.assertIn("output", p, f"{model} missing output")
            self.assertIn("cache_write", p, f"{model} missing cache_write")
            self.assertIn("cache_read", p, f"{model} missing cache_read")

    def test_default_exists(self):
        self.assertIn("default", at.PRICING)

    def test_prices_are_positive(self):
        for model, p in at.PRICING.items():
            self.assertGreaterEqual(p["input"], 0, f"{model} input < 0")
            self.assertGreaterEqual(p["output"], 0, f"{model} output < 0")


# ═══════════════════════════════════════════════════════════════
# INTEGRATION TESTS (use real session data if available)
# ═══════════════════════════════════════════════════════════════

class TestIntegrationWithRealData(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        cls.sessions_dir = os.path.expanduser("~/.hermes/sessions")
        cls.has_real_data = os.path.isdir(cls.sessions_dir)

    def test_list_jsonl_files(self):
        if not self.has_real_data:
            self.skipTest("No real session data")
        files = at._list_jsonl_files(self.sessions_dir)
        self.assertGreater(len(files), 0)

    def test_parse_all_real_sessions(self):
        if not self.has_real_data:
            self.skipTest("No real session data")
        files = at._list_jsonl_files(self.sessions_dir)
        for fp in files:
            events = at.parse(fp)
            self.assertIsInstance(events, list, f"parse({fp}) should return list")
            if events:
                m = at.analyze(events)
                self.assertIsInstance(m["cost"], float)
                self.assertIsInstance(m["user_msgs"], int)
                self.assertIsInstance(m["turns"], int)

    def test_no_exceptions_on_all_real_data(self):
        if not self.has_real_data:
            self.skipTest("No real session data")
        files = at._list_jsonl_files(self.sessions_dir)
        for fp in files:
            try:
                events = at.parse(fp)
                m = at.analyze(events)
                anoms = at.anomalies(m)
                _ = at.report_text(m, anoms)
                _ = at.report_json(m, anoms)
            except Exception as e:
                self.fail(f"Failed on {fp}: {e}")


# ═══════════════════════════════════════════════════════════════
# HELPERS
# ═══════════════════════════════════════════════════════════════

def _write_temp(content):
    """Write content to a temp file and return path."""
    fd, path = tempfile.mkstemp(suffix=".jsonl" if "\n" in content else ".json")
    with os.fdopen(fd, "w") as f:
        f.write(content)
    return path


def _clean_metrics(user_msgs=2, turns=5, tools=10):
    """Create minimal clean metrics dict for testing."""
    return {
        "events": 20,
        "user_msgs": user_msgs,
        "turns": turns,
        "tool_results": tools,
        "tool_total": tools,
        "tool_ok": tools,
        "tool_fail": 0,
        "tool_names": Counter({"terminal": tools}),
        "reasoning_lens": [800, 1200, 900, 1100, 950],
        "timestamps": [],
        "gaps": [5.0, 8.0, 12.0, 3.0, 6.0],
        "tokens": {"input": 2000, "output": 5000, "cache_write": 0, "cache_read": 0},
        "thinking_redacted": 0,
        "cost": 0.09,
        "model": "default",
    }


def _parse_args(argv):
    """Parse CLI args for testing."""
    p = argparse.ArgumentParser()
    p.add_argument("path", nargs="?")
    p.add_argument("--latest", "-l", action="store_true")
    p.add_argument("--format", "-f", choices=["text", "json"])
    p.add_argument("--dir", "-d")
    p.add_argument("--compare", "-c", action="store_true")
    p.add_argument("--all", "-a", action="store_true")
    p.add_argument("--model", "-m", default="default")
    p.add_argument("--output", "-o")
    p.add_argument("--list-models", action="store_true")
    p.add_argument("--version", "-v", action="store_true")
    p.add_argument("--limit", "-n", type=int, default=50)
    return p.parse_args(argv)


# argparse import for CLI test helper
import argparse


if __name__ == "__main__":
    # Run with verbose output
    unittest.main(verbosity=2)
