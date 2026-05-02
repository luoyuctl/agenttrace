package main

import (
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
