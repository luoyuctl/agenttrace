package engine

import (
	"os"
	"path/filepath"
	"testing"
)

func TestParseCopilotCLIAttributesMap(t *testing.T) {
	path := filepath.Join("..", "..", "testdata", "copilot-attrs-map.jsonl")
	if got := DetectFormat(path).Format; got != "copilot_cli" {
		t.Fatalf("copilot attributes map format: %s", got)
	}
	session, err := LoadSession(path)
	if err != nil {
		t.Fatal(err)
	}
	m := session.Metrics
	if m.SourceTool != "copilot_cli" || m.ModelUsed != "gpt-4.1" {
		t.Fatalf("bad copilot source/model: %+v", m)
	}
	if m.TokensInput != 120 || m.TokensOutput < 40 {
		t.Fatalf("bad copilot usage: %+v", m)
	}
	if m.ToolCallsTotal != 1 || m.ToolCallsOK != 1 || m.ToolUsage["read_file"] != 1 {
		t.Fatalf("bad copilot tool metrics: %+v", m)
	}
	if m.SessionStart == "" || m.SessionEnd == "" {
		t.Fatalf("expected copilot timestamps, got %+v", m)
	}
}

func TestParseCopilotCLIMalformed(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "copilot-broken.jsonl")
	raw := "{\"traceId\":\"trace-1\",\"spanId\":\"span-empty\",\"name\":\"chat.completion\",\"attributes\":{\"gen_ai.request.model\":\"gpt-4.1\"}}\n{}\n"
	if err := os.WriteFile(path, []byte(raw), 0644); err != nil {
		t.Fatal(err)
	}
	if got := DetectFormat(path).Format; got != "copilot_cli" {
		t.Fatalf("copilot malformed format: %s", got)
	}
	if _, err := Parse(path); err == nil {
		t.Fatal("expected malformed copilot log to fail")
	}
}
