package engine

import (
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/luoyuctl/agenttrace/internal/i18n"
)

func mustJSON(v interface{}) string {
	b, _ := json.Marshal(v)
	return string(b)
}

func makeJSONL(lines []interface{}) string {
	var parts []string
	for _, l := range lines {
		parts = append(parts, mustJSON(l))
	}
	return strings.Join(parts, "\n") + "\n"
}

func TestParseHermesJSONL(t *testing.T) {
	raw := makeJSONL([]interface{}{
		map[string]interface{}{"role": "session_meta", "session_id": "s1"},
		map[string]interface{}{"role": "user", "content": "hello", "timestamp": "2026-01-01T00:00:00Z"},
		map[string]interface{}{"role": "assistant", "content": "hi", "timestamp": "2026-01-01T00:00:01Z", "tool_calls": []interface{}{map[string]interface{}{"id": "tc1", "name": "read_file"}}},
		map[string]interface{}{"role": "tool", "content": "file content", "tool_call_id": "tc1", "timestamp": "2026-01-01T00:00:02Z"},
	})
	events, err := parseHermesJSONL(raw)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(events) != 4 {
		t.Fatalf("expected 4 events, got %d", len(events))
	}
	if events[2].ToolCalls[0].Name != "read_file" {
		t.Errorf("tool call name: got %q", events[2].ToolCalls[0].Name)
	}
	if events[3].ToolCallID != "tc1" {
		t.Errorf("ToolCallID: got %q", events[3].ToolCallID)
	}
}

func TestParseHermesJSONL_Empty(t *testing.T) {
	events, err := parseHermesJSONL("")
	if err != nil {
		t.Fatalf("empty input should not error: %v", err)
	}
	if len(events) != 0 {
		t.Errorf("expected 0 events from empty input, got %d", len(events))
	}
}

func TestParseHermesJSON(t *testing.T) {
	doc := map[string]interface{}{
		"model": "claude-sonnet-4",
		"usage": map[string]interface{}{"input_tokens": 100, "output_tokens": 200},
		"messages": []interface{}{
			map[string]interface{}{"role": "user", "content": "hello", "timestamp": "2026-01-01T00:00:00Z"},
			map[string]interface{}{"role": "assistant", "content": "", "timestamp": "2026-01-01T00:00:01Z", "tool_calls": []interface{}{map[string]interface{}{"id": "tc1", "function": map[string]interface{}{"name": "read_file"}}}},
			map[string]interface{}{"role": "tool", "content": "ok", "tool_call_id": "tc1", "timestamp": "2026-01-01T00:00:02Z", "is_error": false},
		},
	}
	events, err := parseHermesJSON(doc)
	if err != nil {
		t.Fatalf("error: %v", err)
	}
	// Bug1: no duplicate tool events
	toolCount := 0
	for _, ev := range events {
		if ev.Role == "tool" {
			toolCount++
		}
	}
	if toolCount != 1 {
		t.Errorf("Bug1: tool dupe, got %d tool events", toolCount)
	}
	// Bug2: usage meta
	hasMeta := false
	for _, ev := range events {
		if ev.Role == "meta" && ev.Usage != nil {
			hasMeta = true
			if ev.Usage["input_tokens"] != 100 {
				t.Errorf("Bug2: wrong input tokens: %v", ev.Usage)
			}
		}
	}
	if !hasMeta {
		t.Error("Bug2: missing meta")
	}
	// Bug3: timestamps
	for _, ev := range events {
		if ev.Role != "meta" && ev.Timestamp == "" {
			t.Errorf("Bug3: role=%q missing ts", ev.Role)
		}
	}
}

func TestParseClaudeCode_ToolResultArray(t *testing.T) {
	doc := map[string]interface{}{
		"model": "claude",
		"messages": []interface{}{
			map[string]interface{}{"role": "user", "content": []interface{}{map[string]interface{}{"type": "text", "text": "do"}}},
			map[string]interface{}{"role": "assistant", "content": []interface{}{map[string]interface{}{"type": "tool_use", "id": "tu1", "name": "bash"}}},
			map[string]interface{}{"role": "user", "content": []interface{}{map[string]interface{}{"type": "tool_result", "tool_use_id": "tu1", "is_error": false, "content": []interface{}{map[string]interface{}{"type": "text", "text": "success"}}}}},
		},
	}
	events, _ := parseClaudeCode(doc, nil)
	found := false
	for _, ev := range events {
		if ev.Role == "tool" && ev.ToolCallID == "tu1" && ev.Content == "success" {
			found = true
		}
	}
	if !found {
		t.Error("Bug6: tool_result array content not extracted")
	}
}

func TestParseGeneric_Invalid(t *testing.T) {
	_, err := parseGeneric("not json at all")
	if err == nil {
		t.Error("Bug7: expected error for invalid input")
	}
}

func TestDetectFormat_GeminiAndOpenClaw(t *testing.T) {
	dir := t.TempDir()
	geminiPath := filepath.Join(dir, "gemini.json")
	geminiDoc := map[string]interface{}{
		"modelVersion": "gemini-2.5-pro",
		"contents": []interface{}{
			map[string]interface{}{"role": "user", "parts": []interface{}{map[string]interface{}{"text": "hi"}}},
		},
	}
	if err := os.WriteFile(geminiPath, []byte(mustJSON(geminiDoc)), 0644); err != nil {
		t.Fatal(err)
	}
	if got := DetectFormat(geminiPath).Format; got != "gemini_cli" {
		t.Fatalf("gemini format: %s", got)
	}

	openClawPath := filepath.Join(dir, "openclaw.json")
	openClawDoc := map[string]interface{}{
		"provider": "openclaw",
		"model":    "claude-sonnet-4",
		"messages": []interface{}{
			map[string]interface{}{"role": "assistant", "content": "ok"},
		},
	}
	if err := os.WriteFile(openClawPath, []byte(mustJSON(openClawDoc)), 0644); err != nil {
		t.Fatal(err)
	}
	if got := DetectFormat(openClawPath).Format; got != "openclaw" {
		t.Fatalf("openclaw format: %s", got)
	}
}

func TestParseAiderChatHistory(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, ".aider.chat.history.md")
	raw := `
# aider chat started at 2026-05-02 10:00:00

> aider --model gpt-4.1

#### Add a CI health gate

Implemented the gate with focused tests.

> Tokens: 1.2k sent, 300 cache write, 400 cache hit, 345 received. Cost: $0.004 message, $0.004 session.

#### Run tests

All tests pass.
`
	if err := os.WriteFile(path, []byte(raw), 0644); err != nil {
		t.Fatal(err)
	}
	if got := DetectFormat(path).Format; got != "aider_chat_history" {
		t.Fatalf("aider format: %s", got)
	}
	events, err := Parse(path)
	if err != nil {
		t.Fatal(err)
	}
	m := Analyze(events, "gpt-4.1")
	if m.SourceTool != "aider" || m.UserMessages != 2 || m.AssistantTurns != 2 {
		t.Fatalf("bad aider metrics: %+v", m)
	}
	if m.ModelUsed != "gpt-4.1" || m.TokensInput != 1200 || m.TokensCacheW != 300 || m.TokensCacheR != 400 || m.TokensOutput != 345 {
		t.Fatalf("bad aider usage/model: %+v", m)
	}
}

func TestParseOhMyPiSessionJSONL(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "session.jsonl")
	raw := makeJSONL([]interface{}{
		map[string]interface{}{
			"type":      "session",
			"version":   3,
			"id":        "1f9d2a6b9c0d1234",
			"timestamp": "2026-02-16T10:20:30.000Z",
			"cwd":       "/work/pi",
		},
		map[string]interface{}{
			"type":      "message",
			"id":        "u1",
			"parentId":  nil,
			"timestamp": "2026-02-16T10:21:00.000Z",
			"message": map[string]interface{}{
				"role":      "user",
				"content":   []interface{}{map[string]interface{}{"type": "text", "text": "Inspect the failing test"}},
				"timestamp": float64(1771237260000),
			},
		},
		map[string]interface{}{
			"type":      "message",
			"id":        "a1",
			"parentId":  "u1",
			"timestamp": "2026-02-16T10:21:10.000Z",
			"message": map[string]interface{}{
				"role":     "assistant",
				"provider": "anthropic",
				"model":    "claude-sonnet-4-5",
				"content": []interface{}{
					map[string]interface{}{"type": "thinking", "thinking": "Need to inspect logs first."},
					map[string]interface{}{"type": "text", "text": "I will inspect the failure."},
					map[string]interface{}{"type": "toolCall", "id": "tc1", "name": "read", "arguments": map[string]interface{}{"path": "go.mod"}},
				},
				"usage": map[string]interface{}{
					"input":      100,
					"output":     20,
					"cacheRead":  7,
					"cacheWrite": 3,
				},
				"timestamp": float64(1771237270000),
			},
		},
		map[string]interface{}{
			"type":      "message",
			"id":        "t1",
			"parentId":  "a1",
			"timestamp": "2026-02-16T10:21:12.000Z",
			"message": map[string]interface{}{
				"role":       "toolResult",
				"toolCallId": "tc1",
				"toolName":   "read",
				"content":    []interface{}{map[string]interface{}{"type": "text", "text": "module github.com/luoyuctl/agenttrace"}},
				"isError":    false,
				"timestamp":  float64(1771237272000),
			},
		},
	})
	if err := os.WriteFile(path, []byte(raw), 0644); err != nil {
		t.Fatal(err)
	}

	if got := DetectFormat(path).Format; got != "oh_my_pi" {
		t.Fatalf("oh-my-pi format: %s", got)
	}
	events, err := Parse(path)
	if err != nil {
		t.Fatal(err)
	}
	m := Analyze(events, "claude-sonnet-4-5")
	if m.SourceTool != "oh_my_pi" || m.UserMessages != 1 || m.AssistantTurns != 1 || m.ToolCallsTotal != 1 || m.ToolCallsOK != 1 {
		t.Fatalf("bad oh-my-pi metrics: %+v", m)
	}
	if m.ModelUsed != "claude-sonnet-4-5" || m.TokensInput != 100 || m.TokensOutput != 20 || m.TokensCacheR != 7 || m.TokensCacheW != 3 {
		t.Fatalf("bad oh-my-pi model/usage: %+v", m)
	}
	if m.ReasoningBlocks != 1 || m.ToolUsage["read"] != 1 {
		t.Fatalf("bad oh-my-pi reasoning/tool usage: %+v", m)
	}
}

func TestParseOhMyPiSessionJSONL_InvalidHeader(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "broken.jsonl")
	raw := makeJSONL([]interface{}{
		map[string]interface{}{"type": "session", "version": 3, "cwd": "/work/pi"},
		map[string]interface{}{"type": "message", "message": map[string]interface{}{"role": "user", "content": "hello"}},
	})
	if err := os.WriteFile(path, []byte(raw), 0644); err != nil {
		t.Fatal(err)
	}

	if got := DetectFormat(path).Format; got != "oh_my_pi" {
		t.Fatalf("expected malformed oh-my-pi session to be detected, got %s", got)
	}
	if _, err := Parse(path); err == nil {
		t.Fatalf("expected invalid oh-my-pi header to fail")
	}
}

func TestParseQwenCodeStreamJSONL(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "qwen-stream.jsonl")
	raw := makeJSONL([]interface{}{
		map[string]interface{}{
			"type":       "system",
			"subtype":    "session_start",
			"uuid":       "sys-1",
			"session_id": "session-1",
			"model":      "qwen3-coder-plus",
			"timestamp":  "2026-05-03T10:00:00Z",
		},
		map[string]interface{}{
			"type":       "assistant",
			"uuid":       "assistant-1",
			"session_id": "session-1",
			"timestamp":  "2026-05-03T10:00:02Z",
			"message": map[string]interface{}{
				"id":    "msg-1",
				"type":  "message",
				"role":  "assistant",
				"model": "qwen3-coder-plus",
				"content": []interface{}{
					map[string]interface{}{"type": "reasoning", "text": "Need to inspect package files."},
					map[string]interface{}{"type": "text", "text": "I'll inspect the package."},
					map[string]interface{}{"type": "tool_use", "id": "tool-1", "name": "read_file", "input": map[string]interface{}{"path": "package.json"}},
				},
				"usage": map[string]interface{}{
					"input_tokens":                120,
					"output_tokens":               45,
					"cache_read_input_tokens":     10,
					"cache_creation_input_tokens": 5,
				},
			},
		},
		map[string]interface{}{
			"type":       "user",
			"uuid":       "user-1",
			"session_id": "session-1",
			"timestamp":  "2026-05-03T10:00:03Z",
			"message": map[string]interface{}{
				"role":    "user",
				"content": "Please continue after inspecting it.",
			},
		},
		map[string]interface{}{
			"type":       "user",
			"uuid":       "tool-result-1",
			"session_id": "session-1",
			"timestamp":  "2026-05-03T10:00:04Z",
			"message": map[string]interface{}{
				"role": "user",
				"content": []interface{}{
					map[string]interface{}{"type": "tool_result", "tool_use_id": "tool-1", "content": "package metadata", "is_error": false},
				},
			},
		},
		map[string]interface{}{
			"type":        "result",
			"subtype":     "success",
			"uuid":        "result-1",
			"session_id":  "session-1",
			"is_error":    false,
			"duration_ms": 1234,
			"result":      "I'll inspect the package.",
			"usage": map[string]interface{}{
				"input_tokens":  120,
				"output_tokens": 45,
			},
		},
	})
	if err := os.WriteFile(path, []byte(raw), 0644); err != nil {
		t.Fatal(err)
	}

	if got := DetectFormat(path).Format; got != "qwen_code" {
		t.Fatalf("qwen format: %s", got)
	}
	events, err := Parse(path)
	if err != nil {
		t.Fatal(err)
	}
	m := Analyze(events, "qwen3-coder-plus")
	if m.SourceTool != "qwen_code" || m.UserMessages != 1 || m.AssistantTurns != 1 || m.ToolCallsTotal != 1 || m.ToolCallsOK != 1 {
		t.Fatalf("bad qwen metrics: %+v", m)
	}
	if m.ModelUsed != "qwen3-coder-plus" || m.TokensInput != 120 || m.TokensOutput != 45 || m.TokensCacheR != 10 || m.TokensCacheW != 5 {
		t.Fatalf("bad qwen usage/model: %+v", m)
	}
	if m.ReasoningBlocks != 1 || m.ToolUsage["read_file"] != 1 {
		t.Fatalf("bad qwen reasoning/tool usage: %+v", m)
	}
}

func TestParseQwenCodeJSONOutputArray(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "qwen-output.json")
	raw := []interface{}{
		map[string]interface{}{
			"type":       "system",
			"subtype":    "session_start",
			"uuid":       "sys-1",
			"session_id": "session-1",
			"model":      "qwen3-coder-plus",
		},
		map[string]interface{}{
			"type":       "result",
			"subtype":    "success",
			"uuid":       "result-1",
			"session_id": "session-1",
			"is_error":   false,
			"result":     "The capital of France is Paris.",
			"stats": map[string]interface{}{
				"models": map[string]interface{}{
					"qwen3-coder-plus": map[string]interface{}{
						"tokens": map[string]interface{}{"input": 20, "output": 7},
					},
				},
			},
		},
	}
	if err := os.WriteFile(path, []byte(mustJSON(raw)), 0644); err != nil {
		t.Fatal(err)
	}

	if got := DetectFormat(path).Format; got != "qwen_code" {
		t.Fatalf("qwen array format: %s", got)
	}
	events, err := Parse(path)
	if err != nil {
		t.Fatal(err)
	}
	m := Analyze(events, "qwen3-coder-plus")
	if m.SourceTool != "qwen_code" || m.AssistantTurns != 1 || m.TokensInput != 20 || m.TokensOutput != 7 {
		t.Fatalf("bad qwen json metrics: %+v", m)
	}
}

func TestParseQwenCodeJSONObjectOutput(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "qwen-result.json")
	raw := map[string]interface{}{
		"response": "Done.",
		"stats": map[string]interface{}{
			"models": map[string]interface{}{
				"qwen3-coder-plus": map[string]interface{}{
					"tokens": map[string]interface{}{"input": 31, "output": 9, "cacheRead": 4},
				},
			},
		},
	}
	if err := os.WriteFile(path, []byte(mustJSON(raw)), 0644); err != nil {
		t.Fatal(err)
	}

	if got := DetectFormat(path).Format; got != "qwen_code" {
		t.Fatalf("qwen object format: %s", got)
	}
	events, err := Parse(path)
	if err != nil {
		t.Fatal(err)
	}
	m := Analyze(events, "qwen3-coder-plus")
	if m.SourceTool != "qwen_code" || m.AssistantTurns != 1 || m.TokensInput != 31 || m.TokensOutput != 9 || m.TokensCacheR != 4 {
		t.Fatalf("bad qwen object metrics: %+v", m)
	}
}

func TestParseQwenCodeStreamJSONL_NoMessages(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "empty-qwen.jsonl")
	raw := makeJSONL([]interface{}{
		map[string]interface{}{
			"type":       "system",
			"subtype":    "session_start",
			"uuid":       "sys-1",
			"session_id": "session-1",
			"model":      "qwen3-coder-plus",
		},
	})
	if err := os.WriteFile(path, []byte(raw), 0644); err != nil {
		t.Fatal(err)
	}

	if got := DetectFormat(path).Format; got != "qwen_code" {
		t.Fatalf("qwen empty format: %s", got)
	}
	if _, err := Parse(path); err == nil {
		t.Fatalf("expected qwen stream without assistant/result content to fail")
	}
}

func TestFindSessionFilesIncludesQwenProjectChats(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	chatDir := filepath.Join(home, ".qwen", "projects", "repo", "chats")
	if err := os.MkdirAll(chatDir, 0755); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(chatDir, "chat.jsonl")
	if err := os.WriteFile(path, []byte("{}\n"), 0644); err != nil {
		t.Fatal(err)
	}

	files := FindSessionFiles("")
	if len(files) != 1 || files[0] != path {
		t.Fatalf("expected qwen chat file, got %v", files)
	}
	cache := SessionCache{Entries: map[string]CacheEntry{}, Dirs: map[string]DirCacheEntry{}}
	files = FindSessionFilesCached("", cache)
	if len(files) != 1 || files[0] != path {
		t.Fatalf("expected cached qwen chat file, got %v", files)
	}
}

func TestFindSessionFilesIncludesAiderHistory(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, ".aider.chat.history.md")
	if err := os.WriteFile(path, []byte("# aider chat started at 2026-05-02 10:00:00\n\n#### hi\n\nhello\n"), 0644); err != nil {
		t.Fatal(err)
	}
	files := FindSessionFiles(dir)
	if len(files) != 1 || files[0] != path {
		t.Fatalf("expected aider history file, got %v", files)
	}
	cache := SessionCache{Entries: map[string]CacheEntry{}, Dirs: map[string]DirCacheEntry{}}
	files = FindSessionFilesCached(dir, cache)
	if len(files) != 1 || files[0] != path {
		t.Fatalf("expected cached aider history file, got %v", files)
	}
}

func TestParseCursorSQLiteJSONExport(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "cursor-export.json")
	raw := map[string]interface{}{
		"aiService.prompts": []interface{}{
			map[string]interface{}{"text": "Refactor the auth flow", "commandType": 1},
		},
		"aiService.generations": []interface{}{
			map[string]interface{}{
				"unixMs":          float64(1710000000000),
				"generationUUID":  "gen-1",
				"type":            "composer",
				"textDescription": "Updated the login component and tests",
			},
		},
		"composer.composerData": map[string]interface{}{
			"allComposers": []interface{}{
				map[string]interface{}{
					"composerId":    "cmp-1",
					"name":          "Auth cleanup",
					"subtitle":      "2 files changed",
					"lastUpdatedAt": float64(1710000060000),
				},
			},
		},
	}
	if err := os.WriteFile(path, []byte(mustJSON(raw)), 0644); err != nil {
		t.Fatal(err)
	}
	if got := DetectFormat(path).Format; got != "cursor" {
		t.Fatalf("cursor format: %s", got)
	}
	events, err := Parse(path)
	if err != nil {
		t.Fatal(err)
	}
	m := Analyze(events, "default")
	if m.SourceTool != "cursor" || m.UserMessages != 1 || m.AssistantTurns != 2 {
		t.Fatalf("bad cursor metrics: %+v", m)
	}
	if m.SessionStart == "" || m.SessionEnd == "" {
		t.Fatalf("expected cursor timestamps, got %+v", m)
	}
}

func TestParseCursorGenerationArray(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "cursor-generations.json")
	raw := []interface{}{
		map[string]interface{}{
			"unixMs":          float64(1710000000000),
			"generationUUID":  "gen-1",
			"type":            "chat",
			"textDescription": "Explained the failing test",
		},
	}
	if err := os.WriteFile(path, []byte(mustJSON(raw)), 0644); err != nil {
		t.Fatal(err)
	}
	if got := DetectFormat(path).Format; got != "cursor" {
		t.Fatalf("cursor array format: %s", got)
	}
	events, err := Parse(path)
	if err != nil {
		t.Fatal(err)
	}
	if len(events) != 1 || events[0].SourceTool != "cursor" || events[0].Role != "assistant" {
		t.Fatalf("bad cursor array events: %+v", events)
	}
}

func TestDetectLargeParams_UsesToolArguments(t *testing.T) {
	arg := strings.Repeat("x", 10001)
	events := []Event{{
		Role:      "assistant",
		ToolCalls: []ToolCall{{Name: "write_file", Args: arg}},
	}}
	large := DetectLargeParams(events)
	if len(large) != 1 {
		t.Fatalf("expected one large arg call, got %d", len(large))
	}
	if large[0].ToolName != "write_file" || large[0].ParamSize != len(arg) {
		t.Fatalf("bad large param result: %+v", large[0])
	}
}

func TestFindSessionFilesAutoDiscoverySortsByModTime(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	oldDir := filepath.Join(home, ".codex", "sessions")
	newDir := filepath.Join(home, ".gemini", "sessions")
	if err := os.MkdirAll(oldDir, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(newDir, 0755); err != nil {
		t.Fatal(err)
	}
	oldPath := filepath.Join(oldDir, "z-old.jsonl")
	newPath := filepath.Join(newDir, "a-new.jsonl")
	if err := os.WriteFile(oldPath, []byte("{}\n"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(newPath, []byte("{}\n"), 0644); err != nil {
		t.Fatal(err)
	}
	oldTime := time.Now().Add(-time.Hour)
	newTime := time.Now()
	if err := os.Chtimes(oldPath, oldTime, oldTime); err != nil {
		t.Fatal(err)
	}
	if err := os.Chtimes(newPath, newTime, newTime); err != nil {
		t.Fatal(err)
	}
	files := FindSessionFiles("")
	if len(files) != 2 {
		t.Fatalf("files: %v", files)
	}
	if files[0] != newPath {
		t.Fatalf("expected newest first, got %v", files)
	}
}

func TestFindSessionFilesCachedUsesDirIndexAndFindsNewFiles(t *testing.T) {
	dir := t.TempDir()
	first := filepath.Join(dir, "first.jsonl")
	if err := os.WriteFile(first, []byte("{}\n"), 0644); err != nil {
		t.Fatal(err)
	}
	firstTime := time.Now().Add(-time.Hour)
	if err := os.Chtimes(first, firstTime, firstTime); err != nil {
		t.Fatal(err)
	}

	cache := SessionCache{
		Entries: map[string]CacheEntry{},
		Dirs:    map[string]DirCacheEntry{},
	}
	files := FindSessionFilesCached(dir, cache)
	if len(files) != 1 || files[0] != first {
		t.Fatalf("first scan files: %v", files)
	}
	if _, ok := cache.Dirs[dir]; !ok {
		t.Fatalf("expected directory listing cached")
	}

	second := filepath.Join(dir, "second.jsonl")
	if err := os.WriteFile(second, []byte("{}\n"), 0644); err != nil {
		t.Fatal(err)
	}
	secondTime := time.Now()
	if err := os.Chtimes(second, secondTime, secondTime); err != nil {
		t.Fatal(err)
	}
	if err := os.Chtimes(dir, secondTime, secondTime); err != nil {
		t.Fatal(err)
	}

	files = FindSessionFilesCached(dir, cache)
	if len(files) != 2 {
		t.Fatalf("second scan files: %v", files)
	}
	if files[0] != second {
		t.Fatalf("expected newest file first, got %v", files)
	}
}

func TestLoadSessionCacheSkipsOnlyBadEntries(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	cachePath := sessionCachePath()
	if err := os.MkdirAll(filepath.Dir(cachePath), 0755); err != nil {
		t.Fatal(err)
	}
	payload := `{
		"entries": {
			"/tmp/good.jsonl": {"mod_time": 1, "size": 2, "session": {"Name": "good", "Health": 90}},
			"/tmp/bad.jsonl": {"mod_time": "bad"}
		},
		"dirs": {
			"/tmp": {"mod_time": 3, "files": ["/tmp/good.jsonl"], "dirs": []},
			"/bad": {"mod_time": "bad"}
		}
	}`
	if err := os.WriteFile(cachePath, []byte(payload), 0644); err != nil {
		t.Fatal(err)
	}

	cache := LoadSessionCache()
	if len(cache.Entries) != 1 || cache.Entries["/tmp/good.jsonl"].Session.Name != "good" {
		t.Fatalf("expected only good entry loaded: %+v", cache.Entries)
	}
	if len(cache.Dirs) != 1 || len(cache.Dirs["/tmp"].Files) != 1 {
		t.Fatalf("expected only good dir loaded: %+v", cache.Dirs)
	}
}

func TestSessionCachePersistsAndClears(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("XDG_CACHE_HOME", filepath.Join(home, ".cache"))

	cachePath := SessionCachePath()
	if filepath.Base(cachePath) != "sessions.json" || filepath.Base(filepath.Dir(cachePath)) != "agenttrace" {
		t.Fatalf("unexpected cache path: %s", cachePath)
	}
	if err := ClearSessionCache(); err != nil {
		t.Fatalf("clearing a missing cache should be a no-op: %v", err)
	}

	want := SessionCache{
		Entries: map[string]CacheEntry{
			"/tmp/session.jsonl": {
				ModTime: 10,
				Size:    20,
				Session: Session{Name: "cached", Health: 88},
			},
		},
		Dirs: map[string]DirCacheEntry{
			"/tmp": {
				ModTime: 30,
				Files:   []string{"/tmp/session.jsonl"},
				Dirs:    []string{"/tmp/nested"},
			},
		},
	}
	if err := SaveSessionCache(want); err != nil {
		t.Fatalf("save cache: %v", err)
	}
	if _, err := os.Stat(cachePath); err != nil {
		t.Fatalf("expected cache file to exist: %v", err)
	}

	got := LoadSessionCache()
	if got.Entries["/tmp/session.jsonl"].Session.Name != "cached" {
		t.Fatalf("cache entry did not round-trip: %+v", got.Entries)
	}
	if len(got.Dirs["/tmp"].Files) != 1 || got.Dirs["/tmp"].Dirs[0] != "/tmp/nested" {
		t.Fatalf("directory cache did not round-trip: %+v", got.Dirs)
	}

	if err := ClearSessionCache(); err != nil {
		t.Fatalf("clear cache: %v", err)
	}
	if _, err := os.Stat(cachePath); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("expected cache file removed, stat err=%v", err)
	}
	if err := ClearSessionCache(); err != nil {
		t.Fatalf("clearing an already removed cache should be a no-op: %v", err)
	}
}

func TestCachedSessionRelocalizesLanguageDependentDiagnostics(t *testing.T) {
	prev := i18n.Current
	i18n.SetLang(i18n.ZH)
	t.Cleanup(func() { i18n.SetLang(prev) })

	dir := t.TempDir()
	path := filepath.Join(dir, "cached.jsonl")
	if err := os.WriteFile(path, []byte("{}\n"), 0644); err != nil {
		t.Fatal(err)
	}
	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}

	cache := SessionCache{Entries: map[string]CacheEntry{
		path: {
			ModTime: info.ModTime().UnixNano(),
			Size:    info.Size(),
			Session: Session{
				Name: "cached",
				Metrics: Metrics{
					GapsSec:        []float64{61, 302},
					AssistantTurns: 3,
				},
				Anomalies: []Anomaly{{Type: "hanging", Severity: SeverityHigh, Detail: "2 gap(s) >60s, max=302s"}},
				ContextUtil: ContextUtilization{
					RiskLevel:  "warning",
					Suggestion: "context filling up",
				},
				LargeParams:      []LargeParamCall{{ToolName: "terminal", ParamSize: 12000, Detail: "terminal arguments are 12000 chars"}},
				UnusedTools:      []UnusedToolInfo{{ToolName: "terminal", CallCount: 1, Detail: "tool 'terminal' called only 1x"}},
				LoopFingerprints: []LoopFingerprint{{ToolName: "terminal", Count: 3, Detail: "tool 'terminal' returned same result 3x"}},
				ToolWarnings:     []ToolWarning{{ToolName: "terminal", Pattern: "empty_args", Count: 1, Detail: "terminal called with empty arguments"}},
			},
		},
	}}

	got, ok := CachedSession(path, cache)
	if !ok {
		t.Fatalf("expected cached session")
	}
	for _, want := range []string{"最长", "上下文", "参数", "仅调用", "参数为空"} {
		joined := strings.Join([]string{
			got.Anomalies[0].Detail,
			got.ContextUtil.Suggestion,
			got.LargeParams[0].Detail,
			got.UnusedTools[0].Detail,
			got.ToolWarnings[0].Detail,
		}, "\n")
		if !strings.Contains(joined, want) {
			t.Fatalf("cached session missing localized text %q:\n%s", want, joined)
		}
	}
	if strings.Contains(got.Anomalies[0].Detail, "gap(s)") || strings.Contains(got.ContextUtil.Suggestion, "context") {
		t.Fatalf("cached session kept stale English diagnostics: %+v", got)
	}
}

func TestAnalyze(t *testing.T) {
	events := []Event{
		{Role: "user", Content: "hello", SourceTool: "h", Timestamp: "2026-01-01T00:00:00Z"},
		{Role: "assistant", Content: "hi", SourceTool: "h", Timestamp: "2026-01-01T00:00:01Z"},
		{Role: "assistant", ToolCalls: []ToolCall{{Name: "read_file", ID: "t1"}}, SourceTool: "h", Timestamp: "2026-01-01T00:00:02Z"},
		{Role: "tool", Content: "ok", ToolCallID: "t1", SourceTool: "h", Timestamp: "2026-01-01T00:00:03Z"},
	}
	m := Analyze(events, "default")
	if m.UserMessages != 1 {
		t.Errorf("UserMessages: %d", m.UserMessages)
	}
	if m.AssistantTurns != 2 {
		t.Errorf("AssistantTurns: %d", m.AssistantTurns)
	}
	if m.ToolCallsTotal != 1 {
		t.Errorf("ToolCallsTotal: %d", m.ToolCallsTotal)
	}
	if m.ToolCallsOK != 1 {
		t.Errorf("ToolCallsOK: %d", m.ToolCallsOK)
	}
	if m.CostEstimated < 0 {
		t.Error("cost should be >=0")
	}
}

func TestReportOverviewJSONIncludesOperationalSummary(t *testing.T) {
	sessions := []Session{
		{
			Name:   "good",
			Health: 90,
			Metrics: Metrics{
				SourceTool:    "claude_code_jsonl",
				ModelUsed:     "claude-sonnet-4",
				TokensInput:   100,
				TokensOutput:  50,
				ToolCallsOK:   9,
				ToolCallsFail: 1,
				CostEstimated: 0.25,
			},
		},
		{
			Name:      "bad",
			Health:    40,
			Anomalies: []Anomaly{{Type: "hanging", Severity: SeverityHigh}},
			Metrics: Metrics{
				SourceTool:    "codex_cli",
				ModelUsed:     "gpt-5.1",
				TokensInput:   200,
				TokensOutput:  100,
				ToolCallsFail: 2,
				CostEstimated: 0.75,
			},
		},
	}

	var payload struct {
		Version string `json:"version"`
		Summary struct {
			TotalSessions int     `json:"total_sessions"`
			Critical      int     `json:"critical"`
			TotalCost     float64 `json:"total_cost"`
			TotalTokens   int     `json:"total_tokens"`
			ToolFailRate  float64 `json:"tool_fail_rate"`
			HealthTrend   struct {
				Direction  string `json:"direction"`
				Regressing bool   `json:"regressing"`
				Message    string `json:"message"`
				Points     []struct {
					Name   string `json:"name"`
					Health int    `json:"health"`
				} `json:"points"`
			} `json:"health_trend"`
		} `json:"summary"`
		ByAgent []struct {
			Name     string  `json:"name"`
			Sessions int     `json:"sessions"`
			Cost     float64 `json:"cost"`
		} `json:"by_agent"`
		RecentSessions []struct {
			Name      string `json:"name"`
			Health    int    `json:"health"`
			Anomalies int    `json:"anomalies"`
		} `json:"recent_sessions"`
		Anomalies []AnomalyTop `json:"anomalies"`
	}
	if err := json.Unmarshal([]byte(ReportOverviewJSON(ComputeOverview(sessions), sessions)), &payload); err != nil {
		t.Fatalf("invalid overview json: %v", err)
	}
	if payload.Version != Version || payload.Summary.TotalSessions != 2 || payload.Summary.Critical != 1 {
		t.Fatalf("bad summary: %+v", payload.Summary)
	}
	if payload.Summary.TotalCost != 1 || payload.Summary.TotalTokens != 450 || payload.Summary.ToolFailRate != 25 {
		t.Fatalf("missing operational totals: %+v", payload.Summary)
	}
	if payload.Summary.HealthTrend.Direction == "" || payload.Summary.HealthTrend.Message == "" || len(payload.Summary.HealthTrend.Points) != 2 {
		t.Fatalf("missing health trend: %+v", payload.Summary.HealthTrend)
	}
	if len(payload.ByAgent) != 2 || len(payload.RecentSessions) != 2 || len(payload.Anomalies) != 1 {
		t.Fatalf("missing overview sections: agents=%d recent=%d anomalies=%d",
			len(payload.ByAgent), len(payload.RecentSessions), len(payload.Anomalies))
	}
}

func TestReportOverviewMarkdownIncludesCISummary(t *testing.T) {
	sessions := []Session{
		{
			Name:   "good|session",
			Health: 92,
			Metrics: Metrics{
				SourceTool:    "aider",
				ModelUsed:     "gpt-4.1",
				TokensInput:   1000,
				TokensOutput:  500,
				ToolCallsOK:   4,
				ToolCallsFail: 1,
				CostEstimated: 0.12,
			},
		},
		{
			Name:      "bad",
			Health:    30,
			Anomalies: []Anomaly{{Type: "hanging", Severity: SeverityHigh}},
			Metrics: Metrics{
				SourceTool:    "cursor",
				ModelUsed:     "default",
				TokensInput:   300,
				TokensOutput:  200,
				ToolCallsFail: 5,
				CostEstimated: 0.34,
			},
		},
	}
	out := ReportOverviewMarkdown(ComputeOverview(sessions), sessions)
	for _, want := range []string{
		"# agenttrace overview",
		"| Sessions | 2 |",
		"| Health Trend |",
		"| Tool failures | 6 / 10 (60.0%) |",
		"| Aider | 1 |",
		"| Cursor | 1 |",
		"good\\|session",
		"| bad | hanging |",
	} {
		if !strings.Contains(out, want) {
			t.Fatalf("markdown report missing %q:\n%s", want, out)
		}
	}
}

func TestReportOverviewMarkdownChineseLabels(t *testing.T) {
	prev := i18n.Current
	i18n.SetLang(i18n.ZH)
	t.Cleanup(func() { i18n.SetLang(prev) })

	sessions := []Session{{
		Name:      "bad",
		Health:    30,
		Anomalies: []Anomaly{{Type: "hanging", Severity: SeverityHigh}},
		Metrics: Metrics{
			SourceTool:    "cursor",
			ModelUsed:     "default",
			TokensInput:   300,
			TokensOutput:  200,
			ToolCallsFail: 5,
			CostEstimated: 0.34,
		},
	}}
	out := ReportOverviewMarkdown(ComputeOverview(sessions), sessions)
	for _, want := range []string{"# agenttrace 概览", "| 指标 | 值 |", "| 会话 | 1 |", "健康趋势", "## 最近会话", "挂起"} {
		if !strings.Contains(out, want) {
			t.Fatalf("Chinese markdown report missing %q:\n%s", want, out)
		}
	}
	for _, unwanted := range []string{"Metric", "Recent sessions", "Tool failures", "| bad | hanging |"} {
		if strings.Contains(out, unwanted) {
			t.Fatalf("Chinese markdown report leaked English label %q:\n%s", unwanted, out)
		}
	}
}

func TestReportTextChineseLabelsAnomalySeverityAndLoopCost(t *testing.T) {
	prev := i18n.Current
	i18n.SetLang(i18n.ZH)
	t.Cleanup(func() { i18n.SetLang(prev) })

	out := ReportText(Metrics{
		LoopGroups:      1,
		LoopCostEst:     0.25,
		LoopRetryEvents: 2,
	}, []Anomaly{{Type: "redaction", Severity: SeverityHigh, Emoji: "!"}}, 42)

	for _, want := range []string{"[严重] 思维脱敏", "循环成本", "工具循环成本", "重试事件"} {
		if !strings.Contains(out, want) {
			t.Fatalf("Chinese report text missing %q:\n%s", want, out)
		}
	}
	for _, unwanted := range []string{"[HIGH]", "redaction", "LOOP COST", "Tool Loop Cost", "Retry Events"} {
		if strings.Contains(out, unwanted) {
			t.Fatalf("Chinese report text leaked English label %q:\n%s", unwanted, out)
		}
	}
}

func TestReportOverviewHTMLIsShareableAndEscaped(t *testing.T) {
	sessions := []Session{
		{
			Name:   `good<script>`,
			Health: 92,
			Metrics: Metrics{
				SourceTool:    "aider",
				ModelUsed:     "gpt-4.1",
				TokensInput:   1000,
				TokensOutput:  500,
				ToolCallsOK:   4,
				ToolCallsFail: 1,
				CostEstimated: 0.12,
			},
		},
		{
			Name:      "bad",
			Health:    30,
			Anomalies: []Anomaly{{Type: "hanging", Severity: SeverityHigh}},
			Metrics: Metrics{
				SourceTool:    "cursor",
				ModelUsed:     "default",
				TokensInput:   300,
				TokensOutput:  200,
				ToolCallsFail: 5,
				CostEstimated: 0.34,
			},
		},
	}
	out := ReportOverviewHTML(ComputeOverview(sessions), sessions)
	for _, want := range []string{
		"<!doctype html>",
		"<title>agenttrace overview</title>",
		"AI agent session overview",
		"Tool failures",
		"Aider",
		"Cursor",
		"good&lt;script&gt;",
		"health-bad",
	} {
		if !strings.Contains(out, want) {
			t.Fatalf("html report missing %q:\n%s", want, out)
		}
	}
	if strings.Contains(out, "good<script>") {
		t.Fatalf("html report did not escape session name:\n%s", out)
	}
}

func TestReportOverviewHTMLChineseLabels(t *testing.T) {
	prev := i18n.Current
	i18n.SetLang(i18n.ZH)
	t.Cleanup(func() { i18n.SetLang(prev) })

	sessions := []Session{{
		Name:      "bad<script>",
		Health:    30,
		Anomalies: []Anomaly{{Type: "tool_failures", Severity: SeverityHigh}},
		Metrics: Metrics{
			SourceTool:    "cursor",
			ModelUsed:     "default",
			TokensInput:   300,
			TokensOutput:  200,
			ToolCallsFail: 5,
			CostEstimated: 0.34,
		},
	}}
	out := ReportOverviewHTML(ComputeOverview(sessions), sessions)
	for _, want := range []string{`<html lang="zh">`, "<title>agenttrace 概览</title>", "AI 代理会话概览", "工具失败", "最近会话", "bad&lt;script&gt;"} {
		if !strings.Contains(out, want) {
			t.Fatalf("Chinese html report missing %q:\n%s", want, out)
		}
	}
	for _, unwanted := range []string{"AI agent session overview", "Recent sessions", "Tool failures", "bad<script>"} {
		if strings.Contains(out, unwanted) {
			t.Fatalf("Chinese html report leaked English/raw label %q:\n%s", unwanted, out)
		}
	}
}

func TestAnalyze_ToolFailures(t *testing.T) {
	events := []Event{
		{Role: "user", Content: "x", SourceTool: "c"},
		{Role: "assistant", ToolCalls: []ToolCall{{Name: "bad", ID: "t1"}}, SourceTool: "c"},
		{Role: "tool", Content: `{"error":"fail"}`, ToolCallID: "t1", IsError: true, SourceTool: "c"},
	}
	m := Analyze(events, "default")
	if m.ToolCallsOK != 0 {
		t.Errorf("ToolCallsOK: %d", m.ToolCallsOK)
	}
	if m.ToolCallsFail != 1 {
		t.Errorf("ToolCallsFail: %d", m.ToolCallsFail)
	}
}

func TestDetectAnomalies_Perfect(t *testing.T) {
	m := Metrics{UserMessages: 5, AssistantTurns: 5, ToolCallsTotal: 20, ToolCallsOK: 20, ReasoningBlocks: 5, ReasoningChars: 4000}
	anoms := DetectAnomalies(m)
	if len(anoms) != 0 {
		t.Errorf("expected 0, got %d", len(anoms))
	}
}

func TestDetectAnomalies_Hanging(t *testing.T) {
	m := Metrics{UserMessages: 5, AssistantTurns: 5, ToolCallsTotal: 10, ToolCallsOK: 10, ReasoningBlocks: 5, ReasoningChars: 4000, GapsSec: []float64{10, 15, 350}}
	anoms := DetectAnomalies(m)
	for _, a := range anoms {
		if a.Type == "hanging" {
			return
		}
	}
	t.Error("no hanging anomaly for gap>300s")
}

func TestDetectAnomalies_ToolFail(t *testing.T) {
	m := Metrics{UserMessages: 5, AssistantTurns: 5, ToolCallsTotal: 100, ToolCallsOK: 60, ToolCallsFail: 40, ReasoningBlocks: 5, ReasoningChars: 4000}
	anoms := DetectAnomalies(m)
	for _, a := range anoms {
		if a.Type == "tool_failures" {
			return
		}
	}
	t.Error("no tool_failures anomaly for 40% fail")
}

func TestDetectAnomalies_Shallow(t *testing.T) {
	m := Metrics{UserMessages: 5, AssistantTurns: 5, ToolCallsTotal: 10, ToolCallsOK: 10, ReasoningBlocks: 3, ReasoningChars: 300, ReasoningLens: []int{100, 100, 100}}
	anoms := DetectAnomalies(m)
	for _, a := range anoms {
		if a.Type == "shallow_thinking" {
			return
		}
	}
	t.Error("no shallow_thinking anomaly")
}

func TestHealthScore_Perfect(t *testing.T) {
	s := HealthScore(Metrics{}, nil)
	if s != 100 {
		t.Errorf("expected 100, got %d", s)
	}
}

func TestHealthScore_WithAnomalies(t *testing.T) {
	anoms := []Anomaly{{Severity: SeverityHigh}, {Severity: SeverityMedium}}
	s := HealthScore(Metrics{}, anoms)
	if s != 58 {
		t.Errorf("expected 58, got %d", s)
	}
}

func TestHealthScore_Floor(t *testing.T) {
	anoms := []Anomaly{{Severity: SeverityHigh}, {Severity: SeverityHigh}, {Severity: SeverityHigh}, {Severity: SeverityHigh}}
	s := HealthScore(Metrics{}, anoms)
	if s != 0 {
		t.Errorf("expected 0, got %d", s)
	}
}

func TestDiffSessions(t *testing.T) {
	a := Session{Name: "A", Health: 90, Metrics: Metrics{AssistantTurns: 10, ToolCallsTotal: 20, ToolCallsOK: 18, ToolCallsFail: 2, CostEstimated: 0.05, ModelUsed: "opus", DurationSec: 60}}
	b := Session{Name: "B", Health: 72, Metrics: Metrics{AssistantTurns: 24, ToolCallsTotal: 40, ToolCallsOK: 32, ToolCallsFail: 8, CostEstimated: 0.12, ModelUsed: "haiku", DurationSec: 180}, Anomalies: []Anomaly{{Type: "tool_failures"}}}
	diff := DiffSessions(a, b)
	if diff.SessionA != "A" || diff.SessionB != "B" {
		t.Error("wrong session names")
	}
	if len(diff.Entries) < 7 {
		t.Errorf("entries: %d", len(diff.Entries))
	}
}

func TestGenerateFixes_Hanging(t *testing.T) {
	m := Metrics{GapsSec: []float64{350}}
	anoms := []Anomaly{{Type: "hanging", Severity: SeverityHigh}}
	i18n.Current = i18n.ZH
	fixes := GenerateFixes(m, anoms)
	i18n.Current = i18n.EN
	if len(fixes) == 0 {
		t.Fatal("no fixes")
	}
	if fixes[0].Category != "hanging" {
		t.Errorf("category: %q", fixes[0].Category)
	}
}

func TestGenerateFixes_English(t *testing.T) {
	m := Metrics{GapsSec: []float64{350}}
	anoms := []Anomaly{{Type: "hanging", Severity: SeverityHigh}}
	fixes := GenerateFixes(m, anoms)
	if len(fixes) == 0 {
		t.Fatal("no fixes")
	}
	if fixes[0].Category != "hanging" {
		t.Errorf("category: %q", fixes[0].Category)
	}
}

func TestAnalyzeHealthTrend_Stable(t *testing.T) {
	sessions := []Session{{Name: "s1", Health: 85}, {Name: "s2", Health: 83}, {Name: "s3", Health: 86}, {Name: "s4", Health: 84}}
	if AnalyzeHealthTrend(sessions).Regressing {
		t.Error("should not regress")
	}
}

func TestAnalyzeHealthTrend_Declining(t *testing.T) {
	sessions := []Session{{Name: "s1", Health: 90}, {Name: "s2", Health: 70}, {Name: "s3", Health: 50}}
	trend := AnalyzeHealthTrend(sessions)
	if !trend.Regressing {
		t.Error("should regress")
	}
	if strings.Contains(trend.Message, "%!") {
		t.Fatalf("trend message has bad formatting: %q", trend.Message)
	}
}

func TestAnalyzeHealthTrend_DownMessage(t *testing.T) {
	sessions := []Session{{Name: "s1", Health: 100}, {Name: "s2", Health: 10}, {Name: "s3", Health: 66}}
	trend := AnalyzeHealthTrend(sessions)
	if trend.Direction != "down" {
		t.Fatalf("expected down trend, got %+v", trend)
	}
	if !strings.Contains(trend.Message, "Declining") {
		t.Fatalf("down trend should not render as stable: %q", trend.Message)
	}
}

func TestAnalyzeHealthTrendUsesSessionTimeOrder(t *testing.T) {
	sessions := []Session{
		{Name: "new", Health: 50, Metrics: Metrics{SessionStart: "2026-01-03T00:00:00Z"}},
		{Name: "mid", Health: 70, Metrics: Metrics{SessionStart: "2026-01-02T00:00:00Z"}},
		{Name: "old", Health: 90, Metrics: Metrics{SessionStart: "2026-01-01T00:00:00Z"}},
	}
	trend := AnalyzeHealthTrend(sessions)
	if !trend.Regressing {
		t.Fatalf("newest-first sessions should still detect decline: %+v", trend)
	}
	if trend.Points[0].Name != "old" || trend.Points[2].Name != "new" {
		t.Fatalf("trend points should be old-to-new, got %+v", trend.Points)
	}
}

func TestValidateToolPatterns_DeadLoop(t *testing.T) {
	events := make([]Event, 6)
	for i := range events {
		events[i] = Event{Role: "assistant", ToolCalls: []ToolCall{{Name: "read_file"}}}
	}
	warnings := ValidateToolPatterns(events)
	for _, w := range warnings {
		if w.Pattern == "dead_loop" {
			return
		}
	}
	t.Error("no dead_loop warning for 6 consecutive same tool calls")
}

func TestValidateToolPatterns_NoIssues(t *testing.T) {
	events := []Event{{Role: "user", Content: "hi"}, {Role: "assistant", Content: "hey"}}
	if len(ValidateToolPatterns(events)) != 0 {
		t.Error("expected 0 warnings")
	}
}

func TestPredictCostAnomaly_Normal(t *testing.T) {
	sessions := []Session{
		{Metrics: Metrics{AssistantTurns: 10, CostEstimated: 0.05}},
		{Metrics: Metrics{AssistantTurns: 10, CostEstimated: 0.06}},
	}
	current := Session{Metrics: Metrics{AssistantTurns: 10, CostEstimated: 0.07}}
	if PredictCostAnomaly(sessions, current).Triggered {
		t.Error("should not trigger")
	}
}

func TestPredictCostAnomaly_Critical(t *testing.T) {
	sessions := []Session{
		{Metrics: Metrics{AssistantTurns: 10, CostEstimated: 0.05}},
	}
	current := Session{Metrics: Metrics{AssistantTurns: 10, CostEstimated: 0.25}}
	alert := PredictCostAnomaly(sessions, current)
	if !alert.Triggered || alert.Level != "critical" {
		t.Errorf("level: %s, triggered: %v", alert.Level, alert.Triggered)
	}
}
