package engine

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestParseGeminiCurrentChat(t *testing.T) {
	path := filepath.Join("..", "..", "testdata", "gemini-current-chat.json")
	if got := DetectFormat(path).Format; got != "gemini_cli" {
		t.Fatalf("gemini chat format: %s", got)
	}
	session, err := LoadSession(path)
	if err != nil {
		t.Fatal(err)
	}
	m := session.Metrics
	if m.SourceTool != "gemini_cli" || m.ModelUsed != "gemini-2.5-pro" {
		t.Fatalf("bad gemini source/model: %+v", m)
	}
	if m.UserMessages != 1 || m.AssistantTurns < 1 {
		t.Fatalf("bad gemini role counts: %+v", m)
	}
	if m.ToolCallsTotal != 1 || m.ToolCallsOK != 1 || m.ToolUsage["read_file"] != 1 {
		t.Fatalf("bad gemini tool metrics: %+v", m)
	}
	if m.TokensInput != 120 || m.TokensOutput != 45 || m.TokensCacheR != 8 {
		t.Fatalf("bad gemini usage: %+v", m)
	}
}

func TestParseGeminiCheckpoint(t *testing.T) {
	path := filepath.Join("..", "..", "testdata", "gemini-checkpoint.json")
	if got := DetectFormat(path).Format; got != "gemini_cli" {
		t.Fatalf("gemini checkpoint format: %s", got)
	}
	events, err := Parse(path)
	if err != nil {
		t.Fatal(err)
	}
	var foundReasoning bool
	for _, ev := range events {
		if strings.Contains(ev.Reasoning, "restore the previous context") {
			foundReasoning = true
		}
	}
	if !foundReasoning {
		t.Fatalf("expected gemini checkpoint reasoning: %+v", events)
	}
	m := Analyze(events, "gemini-2.5-flash")
	if m.SourceTool != "gemini_cli" || m.TokensInput != 80 || m.TokensOutput != 20 {
		t.Fatalf("bad gemini checkpoint metrics: %+v", m)
	}
}

func TestParseGeminiMalformedWrapper(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "gemini-broken.json")
	raw := `{"modelVersion":"gemini-2.5-pro","history":[{"role":"model","parts":[{"functionCall":{}}]}]}`
	if err := os.WriteFile(path, []byte(raw), 0644); err != nil {
		t.Fatal(err)
	}
	if got := DetectFormat(path).Format; got != "gemini_cli" {
		t.Fatalf("gemini malformed format: %s", got)
	}
	if _, err := Parse(path); err == nil {
		t.Fatal("expected malformed gemini wrapper to fail")
	}
}

func TestFindSessionFilesIncludesGeminiTmpChatsAndCheckpoints(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	tmpRoot := filepath.Join(home, ".gemini", "tmp", "repo")
	chatDir := filepath.Join(tmpRoot, "chats")
	checkpointDir := filepath.Join(tmpRoot, "checkpoints")
	shadowGitDir := filepath.Join(tmpRoot, ".git")
	for _, dir := range []string{chatDir, checkpointDir, shadowGitDir} {
		if err := os.MkdirAll(dir, 0755); err != nil {
			t.Fatal(err)
		}
	}
	chatPath := filepath.Join(chatDir, "chat.json")
	checkpointPath := filepath.Join(checkpointDir, "checkpoint.json")
	ignoredPath := filepath.Join(shadowGitDir, "ignored.json")
	raw := `{"contents":[{"role":"user","parts":[{"text":"hi"}]}]}`
	for _, path := range []string{chatPath, checkpointPath, ignoredPath} {
		if err := os.WriteFile(path, []byte(raw), 0644); err != nil {
			t.Fatal(err)
		}
	}

	files := FindSessionFiles("")
	if !containsPath(files, chatPath) || !containsPath(files, checkpointPath) {
		t.Fatalf("expected gemini tmp chat/checkpoint files, got %v", files)
	}
	if containsPath(files, ignoredPath) {
		t.Fatalf("expected shadow git file to be skipped, got %v", files)
	}

	cache := SessionCache{Entries: map[string]CacheEntry{}, Dirs: map[string]DirCacheEntry{}}
	files = FindSessionFilesCached("", cache)
	if !containsPath(files, chatPath) || !containsPath(files, checkpointPath) {
		t.Fatalf("expected cached gemini tmp chat/checkpoint files, got %v", files)
	}
	if containsPath(files, ignoredPath) {
		t.Fatalf("expected cached shadow git file to be skipped, got %v", files)
	}
}

func containsPath(paths []string, want string) bool {
	for _, path := range paths {
		if path == want {
			return true
		}
	}
	return false
}
