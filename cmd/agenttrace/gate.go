package main

import (
	"fmt"
	"sort"
	"strings"

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

type overviewGateEvidenceOptions struct {
	OutputPath  string
	SessionsDir string
	Demo        bool
}

func renderOverviewGateEvidence(ov engine.Overview, sessions []engine.Session, opts overviewGateEvidenceOptions) string {
	if len(sessions) == 0 {
		return ""
	}

	var b strings.Builder
	fmt.Fprint(&b, i18n.T("gate_evidence_title"))
	fmt.Fprintf(&b, i18n.T("gate_evidence_avg_health"), averageHealth(sessions))
	fmt.Fprintf(&b, i18n.T("gate_evidence_critical"), ov.Critical)
	fmt.Fprintf(&b, i18n.T("gate_evidence_tool_fail_rate"), toolFailRate(sessions))

	if opts.OutputPath != "" {
		fmt.Fprintf(&b, i18n.T("gate_evidence_report"), opts.OutputPath)
	}
	if s, ok := lowestHealthSession(sessions); ok && s.Path != "" {
		fmt.Fprintf(&b, i18n.T("gate_evidence_session"), s.Path)
	}
	fmt.Fprintf(&b, i18n.T("gate_evidence_next"), overviewReportCommand(opts))
	return b.String()
}

func lowestHealthSession(sessions []engine.Session) (engine.Session, bool) {
	if len(sessions) == 0 {
		return engine.Session{}, false
	}
	ordered := append([]engine.Session(nil), sessions...)
	sort.SliceStable(ordered, func(i, j int) bool {
		if ordered[i].Health != ordered[j].Health {
			return ordered[i].Health < ordered[j].Health
		}
		if ordered[i].Path != ordered[j].Path {
			return ordered[i].Path < ordered[j].Path
		}
		return ordered[i].Name < ordered[j].Name
	})
	return ordered[0], true
}

func overviewReportCommand(opts overviewGateEvidenceOptions) string {
	var parts []string
	if opts.Demo {
		parts = append(parts, "--demo")
	} else if opts.SessionsDir != "" {
		parts = append(parts, "-d", shellArg(opts.SessionsDir))
	}
	parts = append(parts, "--overview", "-f", "json")
	return "agenttrace " + strings.Join(parts, " ")
}

func shellArg(s string) string {
	if s == "" {
		return `""`
	}
	if !strings.ContainsAny(s, " \t\n'\"\\$`") {
		return s
	}
	return "'" + strings.ReplaceAll(s, "'", `'\''`) + "'"
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
