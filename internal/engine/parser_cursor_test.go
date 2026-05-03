package engine

import (
	"os"
	"path/filepath"
	"testing"
)

func TestParseCursorSQLiteJSONStringValues(t *testing.T) {
	path := filepath.Join("..", "..", "testdata", "cursor-json-string-values.json")
	if got := DetectFormat(path).Format; got != "cursor" {
		t.Fatalf("cursor string value format: %s", got)
	}
	session, err := LoadSession(path)
	if err != nil {
		t.Fatal(err)
	}
	m := session.Metrics
	if m.SourceTool != "cursor" || m.UserMessages != 1 || m.AssistantTurns != 2 {
		t.Fatalf("bad cursor string value metrics: %+v", m)
	}
	if m.SessionStart == "" || m.SessionEnd == "" {
		t.Fatalf("expected cursor timestamps, got %+v", m)
	}
}

func TestParseCursorSQLiteJSONStringValuesMalformed(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "cursor-broken.json")
	raw := `{"aiService.prompts":"[{broken]","aiService.generations":"not json"}`
	if err := os.WriteFile(path, []byte(raw), 0644); err != nil {
		t.Fatal(err)
	}
	if got := DetectFormat(path).Format; got != "cursor" {
		t.Fatalf("cursor malformed format: %s", got)
	}
	if _, err := Parse(path); err == nil {
		t.Fatal("expected malformed cursor export to fail")
	}
}
