package tui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/luoyuctl/agenttrace/internal/engine"
	"github.com/luoyuctl/agenttrace/internal/i18n"
)

type sessionInsight struct {
	Issue      string
	Impact     string
	Evidence   string
	NextAction string
	Confidence string
	Color      lipgloss.Color
}

func buildSessionInsight(s engine.Session, fixes []engine.FixSuggestion, alert engine.CostAlert) sessionInsight {
	met := s.Metrics
	totalTools := met.ToolCallsOK + met.ToolCallsFail
	success := 100.0
	if totalTools > 0 {
		success = float64(met.ToolCallsOK) / float64(totalTools) * 100
	}

	ins := sessionInsight{
		Issue:      i18n.T("insight_no_major_waste"),
		Impact:     fmt.Sprintf(i18n.T("insight_impact_fmt"), met.CostEstimated, met.AssistantTurns),
		Evidence:   fmt.Sprintf(i18n.T("insight_evidence_fmt"), len(s.Anomalies), success),
		NextAction: i18n.T("insight_default_next"),
		Confidence: i18n.T("insight_medium"),
		Color:      lipgloss.Color("42"),
	}

	if len(s.Anomalies) > 0 {
		a := s.Anomalies[0]
		ins.Issue = anomalyTypeLabel(a.Type)
		ins.Evidence = a.Detail
		ins.Confidence = i18n.T("insight_high")
		ins.Color = lipgloss.Color("196")
		if a.Severity == engine.SeverityMedium {
			ins.Color = lipgloss.Color("220")
		}
	}
	if s.LoopCost.TotalLoopCost > 0 {
		ins.Issue = i18n.T("insight_loop_cost")
		ins.Impact = fmt.Sprintf(i18n.T("insight_loop_impact"), s.LoopCost.TotalLoopCost, met.CostEstimated)
		ins.Evidence = fmt.Sprintf(i18n.T("insight_loop_evidence"), s.LoopCost.RetryEvents, s.LoopCost.LoopGroups)
		ins.Confidence = i18n.T("insight_high")
		ins.Color = lipgloss.Color("196")
	}
	if alert.Triggered {
		ins.Issue = i18n.T("insight_cost_anomaly")
		ins.Impact = alert.Message
		ins.Confidence = i18n.T("insight_high")
		ins.Color = lipgloss.Color("196")
	}
	if len(fixes) > 0 {
		ins.NextAction = fixes[0].Action
	}
	return ins
}

func (m Model) renderDiagnosticSummary() string {
	idx := m.findSessionIndex()
	if idx < 0 || idx >= len(m.sessions) {
		return ""
	}
	s := m.sessions[idx]
	ins := buildSessionInsight(s, m.fixSuggestions, m.costAlert)
	cardW := minInt(maxInt(20, m.contentWidth()-6), 140)
	bodyW := maxInt(8, cardW-4)
	lines := []string{
		lipgloss.NewStyle().Bold(true).Foreground(ins.Color).Render(i18n.T("insight_primary_issue")) + truncate(ins.Issue, bodyW-15),
		dimStyle.Render(i18n.T("insight_impact")) + truncate(ins.Impact, bodyW-8),
		dimStyle.Render(i18n.T("insight_evidence")) + truncate(ins.Evidence, bodyW-10),
		dimStyle.Render(i18n.T("insight_next")) + truncate(ins.NextAction, bodyW-6),
		dimStyle.Render(i18n.T("insight_confidence")) + ins.Confidence,
	}
	style := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(ins.Color).
		Padding(0, 1)
	return styleForOuterWidth(style, cardW).Render(lipgloss.JoinVertical(lipgloss.Left, lines...))
}

func buildDiffInsight(dr engine.SessionDiff) (winner string, explanation string) {
	scoreA, scoreB := 0, 0
	var drivers []string
	for _, e := range dr.Entries {
		switch e.Better {
		case "A":
			scoreA++
			if len(drivers) < 3 {
				drivers = append(drivers, fmt.Sprintf(i18n.T("diff_favors"), diffFieldLabel(e.Field), "A"))
			}
		case "B":
			scoreB++
			if len(drivers) < 3 {
				drivers = append(drivers, fmt.Sprintf(i18n.T("diff_favors"), diffFieldLabel(e.Field), "B"))
			}
		}
	}
	switch {
	case scoreA > scoreB:
		winner = "A"
	case scoreB > scoreA:
		winner = "B"
	default:
		winner = i18n.T("diff_tie")
	}
	if len(drivers) == 0 {
		explanation = i18n.T("diff_similar")
	} else {
		explanation = strings.Join(drivers, ", ") + "."
	}
	return winner, explanation
}
