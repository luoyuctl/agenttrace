package engine

import (
	"os"
	"path/filepath"
	"testing"
)

func TestParseCursorComposerMessages(t *testing.T) {
	path := filepath.Join("..", "..", "testdata", "cursor-composer-messages.json")
	if got := DetectFormat(path).Format; got != "cursor" {
		t.Fatalf("cursor composer message format: %s", got)
	}
	session, err := LoadSession(path)
	if err != nil {
		t.Fatal(err)
	}
	m := session.Metrics
	if m.SourceTool != "cursor" || m.UserMessages != 1 || m.AssistantTurns != 1 {
		t.Fatalf("bad cursor composer metrics: %+v", m)
	}
	if m.SessionStart != "2026-05-03T10:00:00Z" || m.SessionEnd != "2026-05-03T10:01:00Z" {
		t.Fatalf("bad cursor composer timestamps: %+v", m)
	}
}

func TestParseCursorComposerMessagesMalformed(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "cursor-broken-composer.json")
	raw := `{"composer.composerData":{"allComposers":[{"messages":[{"role":"assistant"}]}]}}`
	if err := os.WriteFile(path, []byte(raw), 0644); err != nil {
		t.Fatal(err)
	}
	if got := DetectFormat(path).Format; got != "cursor" {
		t.Fatalf("cursor malformed composer format: %s", got)
	}
	if _, err := Parse(path); err == nil {
		t.Fatal("expected malformed cursor composer export to fail")
	}
}
