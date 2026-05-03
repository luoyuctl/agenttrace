package engine

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestCodexRolloutTokenCountsUseTurnContextModel(t *testing.T) {
	raw := strings.Join([]string{
		`{"timestamp":"2026-05-03T10:00:00Z","type":"session_meta","payload":{"model_provider":"openai"}}`,
		`{"timestamp":"2026-05-03T10:00:01Z","type":"turn_context","payload":{"model":"gpt-5.4"}}`,
		`{"timestamp":"2026-05-03T10:00:02Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":1000,"cached_input_tokens":400,"output_tokens":100,"reasoning_output_tokens":20,"total_tokens":1100},"last_token_usage":{"input_tokens":1000,"cached_input_tokens":400,"output_tokens":100,"reasoning_output_tokens":20,"total_tokens":1100}}}}`,
		`{"timestamp":"2026-05-03T10:00:03Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":1000,"cached_input_tokens":400,"output_tokens":100,"reasoning_output_tokens":20,"total_tokens":1100},"last_token_usage":{"input_tokens":1000,"cached_input_tokens":400,"output_tokens":100,"reasoning_output_tokens":20,"total_tokens":1100}}}}`,
		`{"timestamp":"2026-05-03T10:00:04Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":1700,"cached_input_tokens":900,"output_tokens":160,"reasoning_output_tokens":30,"total_tokens":1860},"last_token_usage":{"input_tokens":700,"cached_input_tokens":500,"output_tokens":60,"reasoning_output_tokens":10,"total_tokens":760}}}}`,
		`{"timestamp":"2026-05-03T10:00:05Z","type":"response_item","payload":{"type":"function_call","call_id":"call_1","name":"shell","arguments":"{}"}}`,
		`{"timestamp":"2026-05-03T10:00:06Z","type":"response_item","payload":{"type":"function_call_output","call_id":"call_1","output":"ok"}}`,
	}, "\n")

	path := filepath.Join(t.TempDir(), "rollout.jsonl")
	if err := os.WriteFile(path, []byte(raw), 0644); err != nil {
		t.Fatal(err)
	}

	session, err := LoadSession(path)
	if err != nil {
		t.Fatal(err)
	}
	m := session.Metrics
	if m.ModelUsed != "gpt-5.4" || m.SourceTool != "codex_cli" {
		t.Fatalf("bad codex identity: %+v", m)
	}
	if m.TokensInput != 800 || m.TokensCacheR != 900 || m.TokensOutput != 190 {
		t.Fatalf("bad codex usage: input=%d cache_read=%d output=%d", m.TokensInput, m.TokensCacheR, m.TokensOutput)
	}
	if m.ToolCallsTotal != 1 || m.ToolCallsOK != 1 {
		t.Fatalf("bad codex tool counts: %+v", m)
	}
}
