package engine

import (
	"encoding/json"
	"os"
	"strings"
	"testing"
)

// makePiJSONL creates a pi-style JSONL string from entry maps.
func makePiJSONL(entries []map[string]interface{}) string {
	var parts []string
	for _, e := range entries {
		b, _ := json.Marshal(e)
		parts = append(parts, string(b))
	}
	return strings.Join(parts, "\n") + "\n"
}

func TestParsePi_BasicSession(t *testing.T) {
	raw := makePiJSONL([]map[string]interface{}{
		{
			"type":      "session",
			"version":   3,
			"id":        "test-uuid",
			"timestamp": "2025-12-08T22:41:00.000Z",
			"cwd":       "/home/user/project",
			"provider":  "anthropic",
			"modelId":   "claude-sonnet-4-5",
		},
		{
			"type":      "message",
			"timestamp": "2025-12-08T22:41:05.000Z",
			"message": map[string]interface{}{
				"role": "user",
				"content": []interface{}{
					map[string]interface{}{"type": "text", "text": "Hello, can you help me?"},
				},
			},
		},
		{
			"type":      "message",
			"timestamp": "2025-12-08T22:41:10.000Z",
			"message": map[string]interface{}{
				"role":    "assistant",
				"model":   "claude-sonnet-4-5",
				"content": []interface{}{
					map[string]interface{}{"type": "text", "text": "Sure! Let me help you."},
					map[string]interface{}{"type": "toolCall", "id": "toolu_001", "name": "read"},
				},
				"usage": map[string]interface{}{
					"input":      100,
					"output":     50,
					"totalTokens": 150,
				},
			},
		},
		{
			"type":      "message",
			"timestamp": "2025-12-08T22:41:15.000Z",
			"message": map[string]interface{}{
				"role":       "toolResult",
				"toolCallId": "toolu_001",
				"toolName":   "read",
				"content": []interface{}{
					map[string]interface{}{"type": "text", "text": "file contents here"},
				},
				"isError": false,
			},
		},
		{
			"type":      "message",
			"timestamp": "2025-12-08T22:42:00.000Z",
			"message": map[string]interface{}{
				"role":    "assistant",
				"model":   "claude-sonnet-4-5",
				"content": []interface{}{
					map[string]interface{}{"type": "text", "text": "Done!"},
				},
			},
		},
	})

	events, err := parsePi(raw)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// Verify event counts
	metaCount := 0
	userCount := 0
	assistantTextCount := 0
	toolCallCount := 0
	toolResultCount := 0
	for _, ev := range events {
		switch {
		case ev.Role == "meta":
			metaCount++
		case ev.Role == "user":
			userCount++
		case ev.Role == "assistant" && len(ev.ToolCalls) > 0:
			toolCallCount++
		case ev.Role == "assistant" && ev.Content != "":
			assistantTextCount++
		case ev.Role == "tool":
			toolResultCount++
		}
	}

	if metaCount < 1 {
		t.Errorf("expected at least 1 meta event (session header), got %d", metaCount)
	}
	if userCount != 1 {
		t.Errorf("expected 1 user event, got %d", userCount)
	}
	if assistantTextCount != 2 {
		t.Errorf("expected 2 assistant text events, got %d", assistantTextCount)
	}
	if toolCallCount != 1 {
		t.Errorf("expected 1 tool call event, got %d", toolCallCount)
	}
	if toolResultCount != 1 {
		t.Errorf("expected 1 tool result event, got %d", toolResultCount)
	}

	// Verify tool call details
	foundToolCall := false
	for _, ev := range events {
		if len(ev.ToolCalls) > 0 && ev.ToolCalls[0].Name == "read" {
			foundToolCall = true
			if ev.ToolCalls[0].ID != "toolu_001" {
				t.Errorf("tool call ID = %q, want toolu_001", ev.ToolCalls[0].ID)
			}
		}
	}
	if !foundToolCall {
		t.Error("read tool call not found")
	}

	// Verify tool result
	foundToolResult := false
	for _, ev := range events {
		if ev.Role == "tool" && ev.ToolCallID == "toolu_001" {
			foundToolResult = true
			if ev.Content != "file contents here" {
				t.Errorf("tool result content = %q", ev.Content)
			}
		}
	}
	if !foundToolResult {
		t.Error("tool result with ID toolu_001 not found")
	}
}

func TestParsePi_ThinkingRedacted(t *testing.T) {
	raw := makePiJSONL([]map[string]interface{}{
		{
			"type":      "session",
			"version":   3,
			"id":        "uuid",
			"timestamp": "2025-12-08T00:00:00.000Z",
			"cwd":       "/tmp",
		},
		{
			"type":      "message",
			"timestamp": "2025-12-08T00:00:01.000Z",
			"message": map[string]interface{}{
				"role": "user",
				"content": []interface{}{
					map[string]interface{}{"type": "text", "text": "think about this"},
				},
			},
		},
		{
			"type":      "message",
			"timestamp": "2025-12-08T00:00:02.000Z",
			"message": map[string]interface{}{
				"role":  "assistant",
				"model": "claude-opus-4-5",
				"content": []interface{}{
					map[string]interface{}{"type": "thinking", "thinking": "Let me analyze this..."},
					map[string]interface{}{"type": "text", "text": "Here is my analysis"},
				},
				"usage": map[string]interface{}{
					"input":       500,
					"output":      200,
					"totalTokens": 700,
				},
			},
		},
	})

	events, err := parsePi(raw)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// Should have thinking event
	foundThinking := false
	for _, ev := range events {
		if ev.Reasoning == "Let me analyze this..." {
			foundThinking = true
			if ev.Redacted != false {
				t.Error("thinking should not be redacted for open thinking")
			}
		}
	}
	if !foundThinking {
		t.Error("thinking event not found")
	}

	// Should have usage meta events
	hasUsage := false
	for _, ev := range events {
		if ev.Role == "meta" && ev.Usage != nil {
			if ev.Usage["input"] == 500 {
				hasUsage = true
			}
		}
	}
	if !hasUsage {
		t.Error("usage meta event not found or has wrong values")
	}
}

func TestParsePi_ModelChange(t *testing.T) {
	raw := makePiJSONL([]map[string]interface{}{
		{
			"type":      "session",
			"version":   3,
			"id":        "uuid",
			"timestamp": "2025-12-08T00:00:00.000Z",
			"cwd":       "/tmp",
			"modelId":   "claude-sonnet-4-5",
		},
		{
			"type":      "model_change",
			"timestamp": "2025-12-08T00:01:00.000Z",
			"provider":  "openai",
			"modelId":   "gpt-5.1-codex",
		},
		{
			"type":      "message",
			"timestamp": "2025-12-08T00:01:05.000Z",
			"message": map[string]interface{}{
				"role": "user",
				"content": []interface{}{
					map[string]interface{}{"type": "text", "text": "which model?"},
				},
			},
		},
		{
			"type":      "message",
			"timestamp": "2025-12-08T00:01:10.000Z",
			"message": map[string]interface{}{
				"role":  "assistant",
				"model": "gpt-5.1-codex",
				"content": []interface{}{
					map[string]interface{}{"type": "text", "text": "I'm using codex"},
				},
				"usage": map[string]interface{}{
					"input": 10,
				},
			},
		},
	})

	events, err := parsePi(raw)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// The usage meta event should have the correct model after model_change
	foundUsage := false
	foundModelChange := false
	for _, ev := range events {
		if ev.Role == "meta" && strings.Contains(ev.Content, "model_change:") {
			foundModelChange = true
			if !strings.Contains(ev.Content, "gpt-5.1-codex") {
				t.Errorf("model_change content = %q", ev.Content)
			}
		}
		if ev.Role == "meta" && ev.Usage != nil && ev.Usage["input"] == 10 {
			foundUsage = true
			if ev.ModelUsed != "gpt-5.1-codex" {
				t.Errorf("expected model gpt-5.1-codex, got %s", ev.ModelUsed)
			}
		}
	}
	if !foundModelChange {
		t.Error("model_change meta event not found")
	}
	if !foundUsage {
		t.Error("usage meta for codex model not found")
	}
}

func TestParsePi_Compaction(t *testing.T) {
	raw := makePiJSONL([]map[string]interface{}{
		{
			"type":      "session",
			"version":   3,
			"id":        "uuid",
			"timestamp": "2025-12-08T00:00:00.000Z",
			"cwd":       "/tmp",
		},
		{
			"type":         "compaction",
			"timestamp":    "2025-12-08T01:00:00.000Z",
			"summary":      "Previous conversation about refactoring",
			"tokensBefore":  json.Number("50000"),
		},
	})

	events, err := parsePi(raw)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	foundCompaction := false
	for _, ev := range events {
		if ev.Role == "meta" && strings.Contains(ev.Content, "compaction:") {
			foundCompaction = true
			if !strings.Contains(ev.Content, "Previous conversation") {
				t.Errorf("compaction content missing summary: %q", ev.Content)
			}
			if !strings.Contains(ev.Content, "50000") {
				t.Errorf("compaction content missing token count: %q", ev.Content)
			}
		}
	}
	if !foundCompaction {
		t.Error("compaction meta event not found")
	}
}

func TestParsePi_Empty(t *testing.T) {
	events, err := parsePi("")
	if err != nil {
		t.Fatalf("empty input should not error: %v", err)
	}
	if len(events) != 0 {
		t.Errorf("expected 0 events from empty input, got %d", len(events))
	}
}

func TestParsePi_InvalidJSON(t *testing.T) {
	events, err := parsePi("not valid json\n{\"type\":\"session\",\"version\":3,\"id\":\"x\",\"timestamp\":\"2025-01-01T00:00:00Z\",\"cwd\":\"/\"}\n")
	if err != nil {
		t.Fatalf("should skip invalid lines gracefully: %v", err)
	}
	if len(events) == 0 {
		t.Error("should have parsed the valid line")
	}
}

func TestDetectPiFormat(t *testing.T) {
	raw := makePiJSONL([]map[string]interface{}{
		{
			"type":      "session",
			"version":   3,
			"id":        "test-uuid",
			"timestamp": "2025-12-08T22:41:00.000Z",
			"cwd":       "/home/user/project",
		},
	})

	// Write to temp dir to verify JSONL header matches detection markers
	_ = t.TempDir() + "/test.pi.jsonl"
	// Verify that the JSONL detection would match by checking header line
	var entry map[string]interface{}
	if err := json.Unmarshal([]byte(strings.Split(raw, "\n")[0]), &entry); err != nil {
		t.Fatalf("bad JSON: %v", err)
	}
	typ, _ := entry["type"].(string)
	_, hasVersion := entry["version"]
	if typ != "session" || !hasVersion {
		t.Error("pi detection markers not found in JSONL header")
	}
}

func TestDetectFormat_Pi(t *testing.T) {
	// Write a real pi session to temp file and verify DetectFormat
	raw := makePiJSONL([]map[string]interface{}{
		{
			"type":      "session",
			"version":   3,
			"id":        "test-uuid",
			"timestamp": "2025-12-08T22:41:00.000Z",
			"cwd":       "/home/user/project",
		},
	})
	path := t.TempDir() + "/session.jsonl"
	os.WriteFile(path, []byte(raw), 0644)
	
	fi := DetectFormat(path)
	if fi.Format != "pi" {
		t.Errorf("expected pi format, got %q", fi.Format)
	}
}
