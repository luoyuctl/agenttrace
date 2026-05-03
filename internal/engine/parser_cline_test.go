package engine

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestParseClineTaskHistory(t *testing.T) {
	path := filepath.Join("..", "..", "testdata", "cline-task")
	if got := DetectFormat(path).Format; got != "cline" {
		t.Fatalf("cline task format: %s", got)
	}

	events, err := Parse(path)
	if err != nil {
		t.Fatal(err)
	}

	var foundTool, foundUI bool
	for _, ev := range events {
		if strings.Contains(ev.Content, "Task finished from Cline UI") {
			foundUI = true
		}
		for _, tc := range ev.ToolCalls {
			if tc.Name == "read_file" && strings.Contains(tc.Args, "go.mod") {
				foundTool = true
			}
		}
	}
	if !foundTool {
		t.Fatalf("expected Cline tool call in events: %+v", events)
	}
	if !foundUI {
		t.Fatalf("expected Cline UI message in events: %+v", events)
	}

	session, err := LoadSession(path)
	if err != nil {
		t.Fatal(err)
	}
	m := session.Metrics
	if m.SourceTool != "cline" || m.ModelUsed != "claude-3-5-sonnet-latest" {
		t.Fatalf("bad cline source/model: %+v", m)
	}
	if m.UserMessages != 1 || m.AssistantTurns < 3 {
		t.Fatalf("bad cline role counts: %+v", m)
	}
	if m.ToolCallsTotal != 1 || m.ToolCallsOK != 1 || m.ToolUsage["read_file"] != 1 {
		t.Fatalf("bad cline tool metrics: %+v", m)
	}
	if m.SessionStart == "" || m.SessionEnd == "" {
		t.Fatalf("expected cline timestamps, got %+v", m)
	}
}

func TestParseClineTaskMissingOptionalMetadata(t *testing.T) {
	dir := t.TempDir()
	raw := `[
		{"role":"user","content":"hello"},
		{"role":"assistant","content":[{"type":"text","text":"hi"}]}
	]`
	if err := os.WriteFile(filepath.Join(dir, clineAPIHistoryFile), []byte(raw), 0644); err != nil {
		t.Fatal(err)
	}

	events, err := Parse(dir)
	if err != nil {
		t.Fatal(err)
	}
	if len(events) != 2 {
		t.Fatalf("events: %+v", events)
	}
}

func TestParseClineTaskMalformedJSON(t *testing.T) {
	path := filepath.Join("..", "..", "testdata", "cline-broken-task")
	if got := DetectFormat(path).Format; got != "cline" {
		t.Fatalf("cline malformed format: %s", got)
	}
	_, err := Parse(path)
	if err == nil {
		t.Fatal("expected malformed cline task to fail")
	}
	if !strings.Contains(err.Error(), clineAPIHistoryFile) {
		t.Fatalf("expected file-specific cline error, got %v", err)
	}
}

func TestFindSessionFilesIncludesClineTaskDirs(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	configDir, err := os.UserConfigDir()
	if err != nil {
		t.Fatal(err)
	}
	taskDir := filepath.Join(configDir, "Code", "User", "globalStorage", "saoudrizwan.claude-dev", "tasks", "task-123")
	if err := os.MkdirAll(taskDir, 0755); err != nil {
		t.Fatal(err)
	}
	raw := `[{"role":"user","content":"hello"},{"role":"assistant","content":"hi"}]`
	if err := os.WriteFile(filepath.Join(taskDir, clineAPIHistoryFile), []byte(raw), 0644); err != nil {
		t.Fatal(err)
	}

	files := FindSessionFiles("")
	if len(files) != 1 || files[0] != taskDir {
		t.Fatalf("expected cline task dir, got %v", files)
	}
	files = FindSessionFiles(taskDir)
	if len(files) != 1 || files[0] != taskDir {
		t.Fatalf("expected direct cline task dir, got %v", files)
	}

	cache := SessionCache{Entries: map[string]CacheEntry{}, Dirs: map[string]DirCacheEntry{}}
	files = FindSessionFilesCached("", cache)
	if len(files) != 1 || files[0] != taskDir {
		t.Fatalf("expected cached cline task dir, got %v", files)
	}
}
