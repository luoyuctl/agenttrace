package main

import (
	"errors"
	"strings"
	"testing"

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
