package engine

import (
	"path/filepath"
	"testing"
)

func TestAiderDetectionDoesNotClaimJSONLWithEmbeddedHistory(t *testing.T) {
	path := filepath.Join("..", "..", "testdata", "codex-rollout-with-aider-text.jsonl")
	if got := DetectFormat(path).Format; got != "codex_rollout" {
		t.Fatalf("embedded aider text JSONL format: %s", got)
	}
	session, err := LoadSession(path)
	if err != nil {
		t.Fatal(err)
	}
	m := session.Metrics
	if m.SourceTool != "codex_cli" || m.ModelUsed != "openai" || m.UserMessages != 1 || m.AssistantTurns != 1 {
		t.Fatalf("bad codex rollout metrics: %+v", m)
	}
}

func TestParseAiderChatHistoryMalformed(t *testing.T) {
	if _, err := parseAiderChatHistory(""); err == nil {
		t.Fatal("expected malformed aider history to fail")
	}
}
