package main

import (
	"database/sql"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/luoyuctl/agenttrace/internal/engine"
	"github.com/luoyuctl/agenttrace/internal/i18n"

	_ "modernc.org/sqlite"
)

func TestDoctorReportWithDemoSessions(t *testing.T) {
	dir, cleanup, err := writeDemoSessions()
	if err != nil {
		t.Fatal(err)
	}
	defer cleanup()

	out, err := renderDoctorReport(dir, true, "json")
	if err != nil {
		t.Fatal(err)
	}
	var report doctorReport
	if err := json.Unmarshal([]byte(out), &report); err != nil {
		t.Fatal(err)
	}
	if report.Version != engine.Version || report.Sessions != 3 || report.SessionFiles != 3 || len(report.Directories) != 1 {
		t.Fatalf("unexpected doctor report: %+v", report)
	}
	if !report.Directories[0].Exists || report.Directories[0].Files != 3 {
		t.Fatalf("bad directory diagnosis: %+v", report.Directories[0])
	}
	if len(report.Recommendations) != 2 || report.Recommendations[1] != i18n.T("doctor_next_demo_cache") {
		t.Fatalf("demo doctor should explain temporary cache behavior: %+v", report.Recommendations)
	}
}

func TestDoctorReportUsesReportableSQLiteBackedSources(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("XDG_CONFIG_HOME", filepath.Join(home, ".config"))
	t.Setenv("XDG_CACHE_HOME", filepath.Join(home, ".cache"))

	sessionsDir := filepath.Join(home, ".hermes", "sessions")
	if err := os.MkdirAll(sessionsDir, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(sessionsDir, "legacy.jsonl"), []byte("{}\n"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := writeDoctorHermesStateDBForTest(filepath.Join(home, ".hermes", "state.db")); err != nil {
		t.Fatal(err)
	}

	report := buildDoctorReport("", false)
	if report.Sessions != 1 || report.SessionFiles != 0 {
		t.Fatalf("doctor should split sessions from reportable files, got sessions=%d files=%d", report.Sessions, report.SessionFiles)
	}

	var hermesFileRow, hermesDBRow *doctorDirReport
	for i := range report.Directories {
		switch report.Directories[i].Name {
		case "Hermes Agent":
			hermesFileRow = &report.Directories[i]
		case "Hermes Agent (DB)":
			hermesDBRow = &report.Directories[i]
		}
	}
	if hermesFileRow == nil || hermesFileRow.Files != 0 {
		t.Fatalf("sqlite-backed legacy file dir should be skipped, got %+v", hermesFileRow)
	}
	if hermesDBRow == nil || !hermesDBRow.Exists || hermesDBRow.Files != 1 {
		t.Fatalf("sqlite-backed source should be shown, got %+v", hermesDBRow)
	}
}

func TestDoctorReportChineseText(t *testing.T) {
	prev := i18n.Current
	i18n.SetLang(i18n.ZH)
	t.Cleanup(func() { i18n.SetLang(prev) })

	out, err := renderDoctorReport(t.TempDir(), false, "text")
	if err != nil {
		t.Fatal(err)
	}
	for _, want := range []string{"AGENTTRACE 诊断", "会话", "建议"} {
		if !strings.Contains(out, want) {
			t.Fatalf("doctor output missing %q:\n%s", want, out)
		}
	}
}

func writeDoctorHermesStateDBForTest(path string) error {
	if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
		return err
	}
	db, err := sql.Open("sqlite", path)
	if err != nil {
		return err
	}
	defer db.Close()
	if _, err := db.Exec(`create table sessions (
		id text primary key,
		model text,
		started_at real,
		ended_at real,
		message_count integer,
		tool_call_count integer,
		input_tokens integer,
		output_tokens integer,
		cache_read_tokens integer,
		cache_write_tokens integer
	)`); err != nil {
		return err
	}
	if _, err := db.Exec(`create table messages (session_id text, role text)`); err != nil {
		return err
	}
	if _, err := db.Exec(`insert into sessions values ('db-session', 'gpt-5.1', 1760000000, 1760000060, 2, 1, 1000, 200, 50, 25)`); err != nil {
		return err
	}
	_, err = db.Exec(`insert into messages values ('db-session', 'user'), ('db-session', 'assistant')`)
	return err
}
