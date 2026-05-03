package engine

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestParseOpenCodeWrapperStillWorks(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "opencode-wrapper.json")
	raw := `{"provider":"opencode","model":"claude-sonnet-4","messages":[{"role":"user","content":"hi","timestamp":"2026-01-01T00:00:00Z"},{"role":"assistant","content":"hello","timestamp":"2026-01-01T00:00:01Z"}]}`
	if err := os.WriteFile(path, []byte(raw), 0644); err != nil {
		t.Fatal(err)
	}
	if got := DetectFormat(path).Format; got != "opencode" {
		t.Fatalf("opencode wrapper format: %s", got)
	}
	session, err := LoadSession(path)
	if err != nil {
		t.Fatal(err)
	}
	if session.Metrics.SourceTool != "opencode" || session.Metrics.UserMessages != 1 || session.Metrics.AssistantTurns != 1 {
		t.Fatalf("bad opencode wrapper metrics: %+v", session.Metrics)
	}
}

func TestParseOpenCodeStorageSession(t *testing.T) {
	path := filepath.Join("..", "..", "testdata", "opencode", "storage", "session", "project_alpha", "ses_abc.json")
	if got := DetectFormat(path).Format; got != "opencode" {
		t.Fatalf("opencode storage format: %s", got)
	}

	events, err := Parse(path)
	if err != nil {
		t.Fatal(err)
	}
	if len(events) < 4 {
		t.Fatalf("expected stitched opencode events, got %+v", events)
	}
	var foundUser, foundAssistantText, foundToolCall, foundToolResult bool
	for _, ev := range events {
		if ev.Role == "user" && strings.Contains(ev.Content, "parser status") {
			foundUser = true
		}
		if ev.Role == "assistant" && strings.Contains(ev.Content, "inspect the parser") {
			foundAssistantText = true
		}
		if ev.Role == "assistant" && len(ev.ToolCalls) == 1 && ev.ToolCalls[0].Name == "read" {
			foundToolCall = true
		}
		if ev.Role == "tool" && ev.ToolCallID == "call_read" && ev.Content == "engine.go parsed" {
			foundToolResult = true
		}
	}
	if !foundUser || !foundAssistantText || !foundToolCall || !foundToolResult {
		t.Fatalf("missing stitched opencode events: %+v", events)
	}

	session, err := LoadSession(path)
	if err != nil {
		t.Fatal(err)
	}
	m := session.Metrics
	if m.SourceTool != "opencode" || m.ModelUsed != "claude-sonnet-4" {
		t.Fatalf("bad opencode source/model: %+v", m)
	}
	if m.UserMessages != 1 || m.AssistantTurns < 1 {
		t.Fatalf("bad opencode role counts: %+v", m)
	}
	if m.ToolCallsTotal != 1 || m.ToolCallsOK != 1 || m.ToolUsage["read"] != 1 {
		t.Fatalf("bad opencode tool metrics: %+v", m)
	}
	if m.TokensInput != 42 || m.TokensOutput != 17 || m.TokensCacheR != 3 || m.TokensCacheW != 2 {
		t.Fatalf("bad opencode usage: %+v", m)
	}
}

func TestParseOpenCodeStorageSessionMissingMessages(t *testing.T) {
	dir := t.TempDir()
	sessionDir := filepath.Join(dir, "opencode", "storage", "session", "project_alpha")
	if err := os.MkdirAll(sessionDir, 0755); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(sessionDir, "ses_missing.json")
	raw := `{"id":"ses_missing","projectID":"project_alpha","time":{"created":1764750000000}}`
	if err := os.WriteFile(path, []byte(raw), 0644); err != nil {
		t.Fatal(err)
	}
	if got := DetectFormat(path).Format; got != "opencode" {
		t.Fatalf("opencode malformed format: %s", got)
	}
	if _, err := Parse(path); err == nil {
		t.Fatal("expected missing opencode messages to fail")
	}
}

func TestFindSessionFilesIncludesOpenCodeStorageSessions(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("XDG_DATA_HOME", "")
	t.Setenv("OPENCODE_DATA_DIR", "")
	storage := filepath.Join(home, ".local", "share", "opencode", "storage")
	sessionDir := filepath.Join(storage, "session", "project_alpha")
	messageDir := filepath.Join(storage, "message", "ses_abc")
	partDir := filepath.Join(storage, "part", "msg_user")
	for _, dir := range []string{sessionDir, messageDir, partDir} {
		if err := os.MkdirAll(dir, 0755); err != nil {
			t.Fatal(err)
		}
	}
	sessionPath := filepath.Join(sessionDir, "ses_abc.json")
	messagePath := filepath.Join(messageDir, "msg_user.json")
	partPath := filepath.Join(partDir, "part_text.json")
	raw := `{"id":"ses_abc","projectID":"project_alpha","time":{"created":1764750000000}}`
	for _, path := range []string{sessionPath, messagePath, partPath} {
		if err := os.WriteFile(path, []byte(raw), 0644); err != nil {
			t.Fatal(err)
		}
	}

	files := FindSessionFiles("")
	if !containsPath(files, sessionPath) {
		t.Fatalf("expected opencode session file, got %v", files)
	}
	if containsPath(files, messagePath) || containsPath(files, partPath) {
		t.Fatalf("expected only opencode session files, got %v", files)
	}

	files = FindSessionFiles(storage)
	if !containsPath(files, sessionPath) || containsPath(files, messagePath) || containsPath(files, partPath) {
		t.Fatalf("expected bounded custom opencode discovery, got %v", files)
	}

	cache := SessionCache{Entries: map[string]CacheEntry{}, Dirs: map[string]DirCacheEntry{}}
	files = FindSessionFilesCached("", cache)
	if !containsPath(files, sessionPath) {
		t.Fatalf("expected cached opencode session file, got %v", files)
	}
	if containsPath(files, messagePath) || containsPath(files, partPath) {
		t.Fatalf("expected cached discovery to skip opencode message/part files, got %v", files)
	}
}

func TestFindSessionFilesIncludesOpenCodeDataDir(t *testing.T) {
	storage := filepath.Join(t.TempDir(), "custom-opencode-storage")
	t.Setenv("OPENCODE_DATA_DIR", storage)
	sessionDir := filepath.Join(storage, "session", "project_alpha")
	messageDir := filepath.Join(storage, "message", "ses_abc")
	if err := os.MkdirAll(sessionDir, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(messageDir, 0755); err != nil {
		t.Fatal(err)
	}
	sessionPath := filepath.Join(sessionDir, "ses_abc.json")
	messagePath := filepath.Join(messageDir, "msg_user.json")
	raw := `{"id":"ses_abc","projectID":"project_alpha","time":{"created":1764750000000}}`
	for _, path := range []string{sessionPath, messagePath} {
		if err := os.WriteFile(path, []byte(raw), 0644); err != nil {
			t.Fatal(err)
		}
	}

	if got := DetectFormat(sessionPath).Format; got != "opencode" {
		t.Fatalf("opencode data dir format: %s", got)
	}
	files := FindSessionFiles("")
	if !containsPath(files, sessionPath) {
		t.Fatalf("expected OPENCODE_DATA_DIR session file, got %v", files)
	}
	if containsPath(files, messagePath) {
		t.Fatalf("expected OPENCODE_DATA_DIR discovery to skip message files, got %v", files)
	}
}
