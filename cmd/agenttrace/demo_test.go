package main

import (
	"testing"

	"github.com/luoyuctl/agenttrace/internal/engine"
)

func TestWriteDemoSessionsProducesUsableData(t *testing.T) {
	dir, cleanup, err := writeDemoSessions()
	if err != nil {
		t.Fatal(err)
	}
	defer cleanup()

	files := engine.FindSessionFiles(dir)
	if len(files) != 3 {
		t.Fatalf("expected 3 demo sessions, got %d", len(files))
	}
	var sessions []engine.Session
	for _, file := range files {
		s, err := engine.LoadSession(file)
		if err != nil {
			t.Fatalf("demo session should load: %v", err)
		}
		sessions = append(sessions, *s)
	}
	ov := engine.ComputeOverview(sessions)
	if ov.TotalSessions != 3 || len(ov.AnomaliesTop) == 0 {
		t.Fatalf("demo overview should show multiple sessions and anomalies: %+v", ov)
	}
}
