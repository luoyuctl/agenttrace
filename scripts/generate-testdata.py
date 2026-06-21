#!/usr/bin/env python3
import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "testdata" / "generated"


def line(*items):
    return "\n".join(json.dumps(item, ensure_ascii=False, separators=(",", ":")) for item in items) + "\n"


FILES = {
    "detailed-tool-steps.jsonl": line(
        {"role":"user","content":"inspect","timestamp":"2025-01-01T00:00:00Z","ModelUsed":"gpt-5","SourceTool":"generic"},
        {"role":"assistant","content":"checking","timestamp":"2025-01-01T00:00:01Z","tool_calls":[{"id":"ok-1","name":"Read","args":{"path":"a.rs"}},{"id":"bad-1","name":"Shell","args":{"cmd":"false"}},{"id":"missing-1","name":"Search","args":{"query":"x"}}],"ModelUsed":"gpt-5","SourceTool":"generic"},
        {"role":"tool","content":"ok","tool_call_id":"ok-1","timestamp":"2025-01-01T00:00:02Z","SourceTool":"generic"},
        {"role":"tool","content":"failed","tool_call_id":"bad-1","timestamp":"2025-01-01T00:00:04Z","is_error":True,"SourceTool":"generic"},
    ),
    "aggregate-session.sql": "\n".join([
        "CREATE TABLE sessions (id TEXT, model TEXT, started_at REAL, ended_at REAL, message_count INTEGER, tool_call_count INTEGER, input_tokens INTEGER, output_tokens INTEGER, cache_read_tokens INTEGER, cache_write_tokens INTEGER, cwd TEXT);",
        "CREATE TABLE messages (session_id TEXT, role TEXT);",
        "INSERT INTO sessions VALUES ('aggregate','gpt-5',1735689600,1735689660,2,1,100,20,0,0,'/tmp/project');",
        "INSERT INTO sessions VALUES ('limited',NULL,0,0,0,0,0,0,0,0,'');",
        "INSERT INTO messages VALUES ('aggregate','user');",
        "INSERT INTO messages VALUES ('aggregate','assistant');",
        "",
    ]),
    "tool-step-redaction.jsonl": line(
        {"role":"assistant","content":"SHOULD_NOT_LEAK_CONTENT","timestamp":"2025-01-02T00:00:00Z","tool_calls":[{"id":"secret-1","name":"Shell","args":{"token":"SHOULD_NOT_LEAK_ARGS"}}],"ModelUsed":"gpt-5","SourceTool":"generic"},
        {"role":"tool","content":"SHOULD_NOT_LEAK_RESULT","tool_call_id":"secret-1","timestamp":"2025-01-02T00:00:01Z","SourceTool":"generic"},
    ),
    "workbuddy.jsonl": line(
        {"type":"message","role":"user","content":[{"type":"input_text","text":"inspect"}],"timestamp":1735689600000,"sessionId":"s1","cwd":"/tmp/project","providerData":{"agent":"cli"}},
        {"type":"function_call","name":"Read","callId":"c1","arguments":"{\"path\":\"a.rs\"}","timestamp":1735689601000,"sessionId":"s1","cwd":"/tmp/project","providerData":{"model":"glm-5.2","agent":"cli"}},
        {"type":"function_call_result","name":"Read","callId":"c1","status":"completed","output":{"type":"text","text":"ok"},"timestamp":1735689602000,"sessionId":"s1","cwd":"/tmp/project","providerData":{"model":"glm-5.2","agent":"cli"}},
    ),
    "antigravity.jsonl": line(
        {"step_index":0,"source":"USER_EXPLICIT","type":"USER_INPUT","status":"DONE","created_at":"2025-01-03T00:00:00Z","content":"inspect"},
        {"step_index":1,"source":"MODEL","type":"PLANNER_RESPONSE","status":"DONE","created_at":"2025-01-03T00:00:01Z","thinking":"check","tool_calls":[{"name":"view_file","args":{"AbsolutePath":"a.rs"}}]},
        {"step_index":2,"source":"MODEL","type":"VIEW_FILE","status":"DONE","created_at":"2025-01-03T00:00:02Z","content":"ok"},
    ),
    "copilot-session.jsonl": line(
        {"type":"session.start","timestamp":"2025-01-04T00:00:00Z","data":{"context":{"cwd":"/tmp/copilot"}}},
        {"type":"user.message","timestamp":"2025-01-04T00:00:01Z","data":{"content":"inspect"}},
        {"type":"tool.execution_start","timestamp":"2025-01-04T00:00:02Z","data":{"toolName":"Read","toolCallId":"c1","arguments":{"path":"a.rs"}}},
        {"type":"tool.execution_complete","timestamp":"2025-01-04T00:00:03Z","data":{"toolCallId":"c1","success":True}},
        {"type":"session.shutdown","timestamp":"2025-01-04T00:00:04Z","data":{"modelMetrics":{"gpt-5":{"usage":{"inputTokens":100,"outputTokens":20}}}}},
    ),
    "kimi-wire.jsonl": line(
        {"type":"metadata","protocol_version":"2"},
        {"timestamp":1736035200.0,"message":{"type":"TurnBegin","payload":{"user_input":"inspect"}}},
        {"timestamp":1736035201.0,"message":{"type":"ToolCall","payload":{"id":"c1","name":"Read","arguments":{"path":"a.rs"}}}},
        {"timestamp":1736035202.0,"message":{"type":"ToolResult","payload":{"id":"c1","result":"ok"}}},
    ),
    "openclaw-wrapper.json": json.dumps({"provider":"openclaw","model":"claude-sonnet-4","usage":{"input_tokens":10,"output_tokens":2},"messages":[{"role":"user","content":"inspect"},{"role":"assistant","content":[{"type":"tool_use","id":"c1","name":"Read","input":{"path":"a.rs"}}]},{"role":"tool","tool_call_id":"c1","content":"ok"}]}, separators=(",", ":")) + "\n",
    "pi-session.jsonl": line(
        {"type":"session","version":3,"id":"pi-1","timestamp":"2025-01-06T00:00:00Z","cwd":"/tmp/pi"},
        {"type":"message","timestamp":"2025-01-06T00:00:01Z","message":{"role":"user","content":[{"type":"text","text":"inspect"}]}},
        {"type":"message","timestamp":"2025-01-06T00:00:02Z","message":{"role":"assistant","content":[{"type":"text","text":"done"}],"model":"gpt-5"}},
    ),
    "qwen-stream.jsonl": line(
        {"type":"system","uuid":"q1","session_id":"qwen-1","cwd":"/tmp/qwen","model":"qwen3-coder"},
        {"type":"user","uuid":"q2","session_id":"qwen-1","message":{"role":"user","content":"inspect"},"timestamp":"2025-01-07T00:00:00Z"},
        {"type":"assistant","uuid":"q3","session_id":"qwen-1","message":{"role":"assistant","content":"done"},"timestamp":"2025-01-07T00:00:01Z"},
    ),
}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    stale = []
    for name, content in FILES.items():
        path = OUT / name
        if args.check:
            if not path.exists() or path.read_text() != content:
                stale.append(str(path.relative_to(ROOT)))
        else:
            OUT.mkdir(parents=True, exist_ok=True)
            path.write_text(content)
    if stale:
        raise SystemExit("generated testdata is stale: " + ", ".join(stale))


if __name__ == "__main__":
    main()
