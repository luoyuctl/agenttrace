package main

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/luoyuctl/agenttrace/internal/engine"
	"github.com/luoyuctl/agenttrace/internal/i18n"
)

func TestOverviewGateFlagsFailures(t *testing.T) {
	sessions := []engine.Session{
		{Health: 92, Metrics: engine.Metrics{ToolCallsOK: 9, ToolCallsFail: 1}},
		{Health: 45, Metrics: engine.Metrics{ToolCallsOK: 2, ToolCallsFail: 3}},
	}
	ov := engine.ComputeOverview(sessions)

	failures := evaluateOverviewGate(ov, sessions, overviewGate{
		FailUnderHealth: 80,
		FailOnCritical:  true,
		MaxToolFailRate: 20,
	})

	if len(failures) != 3 {
		t.Fatalf("expected three gate failures, got %d: %v", len(failures), failures)
	}
}

func TestOverviewGatePassesHealthySessions(t *testing.T) {
	sessions := []engine.Session{
		{Health: 95, Metrics: engine.Metrics{ToolCallsOK: 10}},
		{Health: 85, Metrics: engine.Metrics{ToolCallsOK: 8, ToolCallsFail: 1}},
	}
	ov := engine.ComputeOverview(sessions)

	failures := evaluateOverviewGate(ov, sessions, overviewGate{
		FailUnderHealth: 80,
		FailOnCritical:  true,
		MaxToolFailRate: 20,
	})

	if len(failures) != 0 {
		t.Fatalf("expected gate to pass, got %v", failures)
	}
}

func TestOverviewGateChineseMessage(t *testing.T) {
	prev := i18n.Current
	i18n.SetLang(i18n.ZH)
	t.Cleanup(func() { i18n.SetLang(prev) })

	sessions := []engine.Session{{Health: 40}}
	failures := evaluateOverviewGate(engine.ComputeOverview(sessions), sessions, overviewGate{FailUnderHealth: 80})
	if len(failures) != 1 || !strings.Contains(failures[0], "平均健康分") {
		t.Fatalf("expected Chinese gate message, got %v", failures)
	}
}

func TestOverviewGateEvidenceIncludesLocalFailureFacts(t *testing.T) {
	prev := i18n.Current
	i18n.SetLang(i18n.EN)
	t.Cleanup(func() { i18n.SetLang(prev) })

	sessions := []engine.Session{
		{
			Name:   "healthier",
			Path:   "/tmp/agenttrace/b-healthier.jsonl",
			Health: 70,
			Metrics: engine.Metrics{
				ToolCallsOK:   9,
				ToolCallsFail: 1,
			},
		},
		{
			Name:   "critical",
			Path:   "/tmp/agenttrace/a-critical.jsonl",
			Health: 20,
			Metrics: engine.Metrics{
				ToolCallsOK:   1,
				ToolCallsFail: 4,
			},
		},
	}

	got := renderOverviewGateEvidence(engine.ComputeOverview(sessions), sessions, overviewGateEvidenceOptions{
		OutputPath:  "agenttrace-overview.json",
		SessionsDir: "/tmp/agenttrace",
	})

	for _, want := range []string{
		"Local evidence:",
		"- avg health: 45.0",
		"- critical sessions: 1",
		"- tool fail rate: 33.3%",
		"- report: agenttrace-overview.json",
		"- lowest-health session: /tmp/agenttrace/a-critical.jsonl",
		"- inspect: `agenttrace -d /tmp/agenttrace --overview -f json`",
	} {
		if !strings.Contains(got, want) {
			t.Fatalf("gate evidence missing %q:\n%s", want, got)
		}
	}
}

func TestOverviewGateEvidenceUsesDemoInspectCommand(t *testing.T) {
	got := renderOverviewGateEvidence(engine.ComputeOverview([]engine.Session{{Health: 40}}), []engine.Session{{Health: 40}}, overviewGateEvidenceOptions{Demo: true})
	if !strings.Contains(got, "agenttrace --demo --overview -f json") {
		t.Fatalf("demo gate evidence should show a reusable demo command:\n%s", got)
	}
}

func TestTUILaunchErrorMessageAddsDemoTTYFallback(t *testing.T) {
	prev := i18n.Current
	i18n.SetLang(i18n.EN)
	t.Cleanup(func() { i18n.SetLang(prev) })

	msg := tuiLaunchErrorMessage(errors.New("could not open a new TTY: open /dev/tty: device not configured"), true)

	for _, want := range []string{
		"Error: could not open a new TTY",
		"agenttrace --demo --overview -f json",
		"agenttrace --demo --overview -f html -o agenttrace-overview.html",
	} {
		if !strings.Contains(msg, want) {
			t.Fatalf("TTY fallback message missing %q:\n%s", want, msg)
		}
	}
}

func TestTUILaunchErrorMessageSkipsFallbackOutsideDemo(t *testing.T) {
	msg := tuiLaunchErrorMessage(errors.New("open /dev/tty: device not configured"), false)
	if strings.Contains(msg, "--demo --overview") {
		t.Fatalf("non-demo TUI error should not include demo fallback:\n%s", msg)
	}
}

func TestLatestSessionFileUsesPreciseMtimeAndTieBreak(t *testing.T) {
	dir := t.TempDir()
	older := filepath.Join(dir, "01-old.jsonl")
	newer := filepath.Join(dir, "02-new.jsonl")
	tieWinner := filepath.Join(dir, "03-tie.jsonl")
	for _, path := range []string{older, newer, tieWinner} {
		if err := os.WriteFile(path, []byte("{}\n"), 0644); err != nil {
			t.Fatal(err)
		}
	}

	base := time.Unix(1700000000, 0)
	newest := base.Add(250 * time.Millisecond)
	for path, ts := range map[string]time.Time{
		older:     base,
		newer:     newest,
		tieWinner: newest,
	} {
		if err := os.Chtimes(path, ts, ts); err != nil {
			t.Fatal(err)
		}
	}

	got := latestSessionFile([]string{"/missing/session.jsonl", newer, older, tieWinner})
	if got != tieWinner {
		t.Fatalf("expected lexicographic tie winner %q, got %q", tieWinner, got)
	}
}

func TestCacheSuggestionUsesSinglePercent(t *testing.T) {
	got := i18n.T("cache_suggestion_none")
	if strings.Contains(got, "%%") || !strings.Contains(got, "90%") {
		t.Fatalf("cache suggestion should render a single percent sign: %q", got)
	}
}
