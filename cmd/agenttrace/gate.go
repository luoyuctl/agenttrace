package main

import (
	"fmt"

	"github.com/luoyuctl/agenttrace/internal/engine"
	"github.com/luoyuctl/agenttrace/internal/i18n"
)

type overviewGate struct {
	FailUnderHealth int
	FailOnCritical  bool
	MaxToolFailRate float64
}

func (g overviewGate) enabled() bool {
	return g.FailUnderHealth > 0 || g.FailOnCritical || g.MaxToolFailRate >= 0
}

func evaluateOverviewGate(ov engine.Overview, sessions []engine.Session, gate overviewGate) []string {
	if !gate.enabled() {
		return nil
	}

	avgHealth := averageHealth(sessions)
	toolFailRate := toolFailRate(sessions)
	var failures []string
	if gate.FailUnderHealth > 0 && avgHealth < float64(gate.FailUnderHealth) {
		failures = append(failures, fmt.Sprintf(i18n.T("gate_avg_health_failed"), avgHealth, gate.FailUnderHealth))
	}
	if gate.FailOnCritical && ov.Critical > 0 {
		failures = append(failures, fmt.Sprintf(i18n.T("gate_critical_failed"), ov.Critical))
	}
	if gate.MaxToolFailRate >= 0 && toolFailRate > gate.MaxToolFailRate {
		failures = append(failures, fmt.Sprintf(i18n.T("gate_tool_fail_rate_failed"), toolFailRate, gate.MaxToolFailRate))
	}
	return failures
}

func averageHealth(sessions []engine.Session) float64 {
	if len(sessions) == 0 {
		return 0
	}
	total := 0
	for _, s := range sessions {
		total += s.Health
	}
	return float64(total) / float64(len(sessions))
}

func toolFailRate(sessions []engine.Session) float64 {
	total := 0
	failed := 0
	for _, s := range sessions {
		total += s.Metrics.ToolCallsOK + s.Metrics.ToolCallsFail
		failed += s.Metrics.ToolCallsFail
	}
	if total == 0 {
		return 0
	}
	return float64(failed) / float64(total) * 100
}
