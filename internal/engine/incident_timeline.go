package engine

import (
	"fmt"
	"math"
	"sort"
	"strings"

	"github.com/luoyuctl/agenttrace/internal/i18n"
)

type IncidentTimelineItem struct {
	Kind     string `json:"kind"`
	Label    string `json:"label"`
	Detail   string `json:"detail"`
	Severity string `json:"severity"`
}

type IncidentTimelineSummary struct {
	Session string                 `json:"session"`
	Items   []IncidentTimelineItem `json:"items"`
}

func BuildIncidentTimeline(s Session) IncidentTimelineSummary {
	m := s.Metrics
	summary := IncidentTimelineSummary{Session: s.Name}
	add := func(kind, label, detail, severity string) {
		detail = strings.TrimSpace(detail)
		if detail == "" {
			return
		}
		summary.Items = append(summary.Items, IncidentTimelineItem{
			Kind: kind, Label: label, Detail: detail, Severity: severity,
		})
	}

	if m.AssistantTurns > 0 {
		detail := fmt.Sprintf(i18n.T("incident_milestone_detail"), m.AssistantTurns, FmtDuration(m.DurationSec))
		add("milestone", i18n.T("incident_milestone"), detail, SeverityLow)
	}

	if gap := maxIncidentGap(m.GapsSec); gap >= 30 {
		severity := SeverityLow
		if gap >= 300 {
			severity = SeverityHigh
		} else if gap >= 60 {
			severity = SeverityMedium
		}
		add("idle_gap", i18n.T("incident_idle_gap"), fmt.Sprintf(i18n.T("incident_idle_gap_detail"), gap), severity)
	}

	if fp, ok := topIncidentLoop(s.LoopFingerprints); ok {
		tool := incidentSafeName(fp.ToolName)
		if tool == "" {
			tool = i18n.T("unknown_tool")
		}
		add("failure_loop", i18n.T("incident_failure_loop"), fmt.Sprintf(i18n.T("incident_failure_loop_detail"), tool, fp.Count), incidentLoopSeverity(fp.Severity))
	} else if totalTools := m.ToolCallsOK + m.ToolCallsFail; totalTools > 0 && m.ToolCallsFail > 1 {
		failRate := float64(m.ToolCallsFail) / float64(totalTools) * 100
		severity := SeverityMedium
		if failRate >= 30 {
			severity = SeverityHigh
		}
		add("failure_loop", i18n.T("incident_failure_loop"), fmt.Sprintf(i18n.T("incident_failure_rate_detail"), m.ToolCallsFail, totalTools, failRate), severity)
	}

	if totalTools := m.ToolCallsOK + m.ToolCallsFail; totalTools > 0 && len(m.ToolUsage) > 0 {
		name, count := topIncidentTool(m.ToolUsage)
		if count > 0 {
			add("touched_surface", i18n.T("incident_touched_surface"), fmt.Sprintf(i18n.T("incident_touched_surface_detail"), len(m.ToolUsage), totalTools, incidentSafeName(name), count), SeverityLow)
		}
	}

	totalTokens := m.TokensInput + m.TokensOutput + m.TokensCacheW + m.TokensCacheR
	if s.LoopCost.TotalLoopCost > 0 && m.CostEstimated > 0 {
		loopCost := ClampLoopWaste(s.LoopCost.TotalLoopCost, m.CostEstimated)
		add("burn_divergence", i18n.T("incident_burn_divergence"), fmt.Sprintf(i18n.T("incident_burn_loop_detail"), loopCost, m.CostEstimated), SeverityHigh)
	} else if totalTokens > 0 && m.AssistantTurns > 0 {
		tokensPerTurn := totalTokens / m.AssistantTurns
		if tokensPerTurn >= 10000 {
			add("burn_divergence", i18n.T("incident_burn_divergence"), fmt.Sprintf(i18n.T("incident_burn_tokens_detail"), tokensPerTurn, m.AssistantTurns), SeverityMedium)
		}
	}

	return summary
}

func maxIncidentGap(gaps []float64) float64 {
	maxGap := 0.0
	for _, gap := range gaps {
		if math.IsNaN(gap) || math.IsInf(gap, 0) || gap < 0 {
			continue
		}
		if gap > maxGap {
			maxGap = gap
		}
	}
	return maxGap
}

func topIncidentLoop(items []LoopFingerprint) (LoopFingerprint, bool) {
	if len(items) == 0 {
		return LoopFingerprint{}, false
	}
	ordered := append([]LoopFingerprint(nil), items...)
	sort.SliceStable(ordered, func(i, j int) bool {
		if ordered[i].Count == ordered[j].Count {
			if ordered[i].ToolName == ordered[j].ToolName {
				return ordered[i].ResultHash < ordered[j].ResultHash
			}
			return ordered[i].ToolName < ordered[j].ToolName
		}
		return ordered[i].Count > ordered[j].Count
	})
	return ordered[0], true
}

func topIncidentTool(items map[string]int) (string, int) {
	type kv struct {
		name  string
		count int
	}
	ordered := make([]kv, 0, len(items))
	for name, count := range items {
		if count <= 0 {
			continue
		}
		ordered = append(ordered, kv{name: name, count: count})
	}
	if len(ordered) == 0 {
		return "", 0
	}
	sort.SliceStable(ordered, func(i, j int) bool {
		if ordered[i].count == ordered[j].count {
			return ordered[i].name < ordered[j].name
		}
		return ordered[i].count > ordered[j].count
	})
	return ordered[0].name, ordered[0].count
}

func incidentLoopSeverity(severity string) string {
	switch strings.ToLower(severity) {
	case SeverityHigh, "critical":
		return SeverityHigh
	case SeverityMedium, "warning":
		return SeverityMedium
	default:
		return SeverityLow
	}
}

func incidentSafeName(value string) string {
	value = strings.TrimSpace(value)
	if fields := strings.Fields(value); len(fields) > 0 {
		value = fields[0]
	}
	return truncateTextRunes(value, 48, "")
}
