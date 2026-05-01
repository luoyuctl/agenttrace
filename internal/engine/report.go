package engine

import (
	"encoding/json"
	"fmt"
	"sort"
	"strings"

	"github.com/luoyuctl/agenttrace/internal/i18n"
)

// ReportText generates the formatted text report.
func ReportText(m Metrics, anoms []Anomaly, h int) string {
	totalTokens := m.TokensInput + m.TokensOutput + m.TokensCacheW + m.TokensCacheR
	totalTools := m.ToolCallsOK + m.ToolCallsFail
	successRate := SuccessRate(m.ToolCallsOK, totalTools)
	avgReason := 0.0
	if m.ReasoningBlocks > 0 {
		avgReason = float64(m.ReasoningChars) / float64(m.ReasoningBlocks)
	}

	gaps := make([]float64, len(m.GapsSec))
	copy(gaps, m.GapsSec)
	sort.Float64s(gaps)

	sep := strings.Repeat(i18n.T("separator_double"), 60)
	sub := strings.Repeat(i18n.T("separator_single"), 40)

	var b strings.Builder
	w := func(f string, args ...interface{}) { b.WriteString(fmt.Sprintf(f, args...) + "\n") }

	w(sep)
	w("  "+i18n.T("title"), Version)
	w(sep)
	w("")

	// Token Cost
	w(i18n.T("token_cost"))
	w(sub)
	w("  "+i18n.T("input"), m.TokensInput)
	w("  "+i18n.T("output"), m.TokensOutput)
	if m.TokensCacheW > 0 || m.TokensCacheR > 0 {
		w("  "+i18n.T("cache_write"), m.TokensCacheW)
		w("  "+i18n.T("cache_read"), m.TokensCacheR)
	}
	w("  ────────────────────────────────────")
	w("  "+i18n.T("total_tokens"), totalTokens)
	w("  "+i18n.T("est_cost"), m.CostEstimated, m.ModelUsed)
	w("")

	// Activity
	w(i18n.T("activity"))
	w(sub)
	w("  "+i18n.T("messages_label"), m.UserMessages, m.AssistantTurns)
	w("  "+i18n.T("tool_calls_label"), m.ToolCallsTotal)
	if totalTools > 0 {
		srEmoji := "🟢"
		rate := float64(m.ToolCallsOK) / float64(totalTools)
		if rate < 0.70 {
			srEmoji = "🔴"
		} else if rate < 0.85 {
			srEmoji = "🟡"
		}
		w("  "+i18n.T("success_label"), successRate, m.ToolCallsOK, totalTools, srEmoji)
	}
	w("")

	// Latency
	w(i18n.T("latency"))
	w(sub)
	if len(gaps) > 0 {
		w("  "+i18n.T("min_lat"), gaps[0])
		w("  "+i18n.T("median"), percentile(gaps, 0.50))
		w("  "+i18n.T("p95"), percentile(gaps, 0.95))
		w("  "+i18n.T("max_lat"), gaps[len(gaps)-1])
		sum := 0.0
		for _, g := range gaps {
			sum += g
		}
		w("  "+i18n.T("avg_lat"), sum/float64(len(gaps)))
	} else {
		w("  " + i18n.T("no_gap_data"))
	}
	w("  "+i18n.T("duration"), fmtDuration(m.DurationSec))
	w("")

	// Top Tools
	if len(m.ToolUsage) > 0 {
		w(i18n.T("top_tools"))
		w(sub)
		type kv struct {
			k string
			v int
		}
		var sorted []kv
		for k, v := range m.ToolUsage {
			sorted = append(sorted, kv{k, v})
		}
		sort.Slice(sorted, func(i, j int) bool { return sorted[i].v > sorted[j].v })
		for i, item := range sorted {
			if i >= 8 {
				break
			}
			w("  %-35s %4d", item.k, item.v)
		}
		w("")
	}

	// Thinking/COT
	w(i18n.T("thinking_cot"))
	w(sub)
	if m.ReasoningBlocks > 0 {
		qualityLbl := i18n.T("quality_deep")
		qEmoji := "🟢"
		if avgReason < 400 {
			qualityLbl = i18n.T("quality_shallow")
			qEmoji = "🔴"
		} else if avgReason < 800 {
			qualityLbl = i18n.T("quality_moderate")
			qEmoji = "🟡"
		}
		w("  "+i18n.T("blocks"), m.ReasoningBlocks)
		w("  "+i18n.T("avg_chars"), avgReason)
		w("  "+i18n.T("total_chars"), m.ReasoningChars)
		w("  "+i18n.T("quality_label"), qEmoji, qualityLbl)
		if m.ReasoningRedact > 0 {
			w("  "+i18n.T("redacted_blocks"), m.ReasoningRedact)
		}
	} else {
		w("  " + i18n.T("no_thinking_blocks"))
	}
	w("")

	// Anomalies
	w(i18n.T("anomalies"))
	w(sub)
	if len(anoms) > 0 {
		for _, a := range anoms {
			w("  %s [%s] %s: %s", a.Emoji, strings.ToUpper(a.Severity), a.Type, a.Detail)
		}
	} else {
		w("  " + i18n.T("no_anomalies"))
	}
	w("")

	// Health Score
	w(i18n.T("health_score"))
	w(sub)
	hBar := HealthBar(h)
	hEmoji := HealthEmoji(h)
	w("  %s  %d/100  %s", hEmoji, h, hBar)
	w("")
	w(sep)

	return b.String()
}

// ReportJSON generates the JSON report.
func ReportJSON(m Metrics, anoms []Anomaly, h int) string {
	totalTokens := m.TokensInput + m.TokensOutput + m.TokensCacheW + m.TokensCacheR
	totalTools := m.ToolCallsOK + m.ToolCallsFail
	avgReason := 0.0
	if m.ReasoningBlocks > 0 {
		avgReason = round4(float64(m.ReasoningChars) / float64(m.ReasoningBlocks))
	}

	gaps := make([]float64, len(m.GapsSec))
	copy(gaps, m.GapsSec)
	sort.Float64s(gaps)

	toolRate := 0.0
	if totalTools > 0 {
		toolRate = round4(float64(m.ToolCallsOK) / float64(totalTools) * 100)
	}

	top10 := make(map[string]int)
	type kv struct {
		k string
		v int
	}
	var sorted []kv
	for k, v := range m.ToolUsage {
		sorted = append(sorted, kv{k, v})
	}
	sort.Slice(sorted, func(i, j int) bool { return sorted[i].v > sorted[j].v })
	for i, item := range sorted {
		if i >= 10 {
			break
		}
		top10[item.k] = item.v
	}

	anomalyItems := make([]map[string]string, len(anoms))
	for i, a := range anoms {
		anomalyItems[i] = map[string]string{
			"type": a.Type, "severity": a.Severity, "detail": a.Detail,
		}
	}

	payload := map[string]interface{}{
		"version":    Version,
		"model_used": m.ModelUsed,
		"session": map[string]interface{}{
			"start":             m.SessionStart,
			"end":               m.SessionEnd,
			"duration_seconds":  m.DurationSec,
			"duration_human":    fmtDuration(m.DurationSec),
		},
		"tokens": map[string]int{
			"input":       m.TokensInput,
			"output":      m.TokensOutput,
			"cache_write": m.TokensCacheW,
			"cache_read":  m.TokensCacheR,
			"total":       totalTokens,
		},
		"cost": map[string]interface{}{
			"estimated": m.CostEstimated,
			"model":     m.ModelUsed,
		},
		"activity": map[string]interface{}{
			"user_messages":     m.UserMessages,
			"assistant_turns":   m.AssistantTurns,
			"tool_calls_total":  m.ToolCallsTotal,
			"tool_calls_ok":     m.ToolCallsOK,
			"tool_calls_fail":   m.ToolCallsFail,
			"tool_success_rate": toolRate,
		},
		"latency": map[string]float64{
			"min":    safeCalc(gaps, func(x []float64) float64 { return x[0] }),
			"median": safeCalc(gaps, func(x []float64) float64 { return percentile(x, 0.50) }),
			"p95":    safeCalc(gaps, func(x []float64) float64 { return percentile(x, 0.95) }),
			"max":    safeCalc(gaps, func(x []float64) float64 { return x[len(x)-1] }),
			"avg":    safeCalc(gaps, func(x []float64) float64 { s := 0.0; for _, v := range x { s += v }; return s / float64(len(x)) }),
		},
		"tools_top":   top10,
		"reasoning": map[string]interface{}{
			"blocks":      m.ReasoningBlocks,
			"total_chars": m.ReasoningChars,
			"avg_chars":   avgReason,
			"redacted":    m.ReasoningRedact,
		},
		"anomalies":    anomalyItems,
		"health_score": h,
	}

	out, _ := json.MarshalIndent(payload, "", "  ")
	return string(out)
}

// ReportCompare generates multi-session comparison text.
func ReportCompare(sessions []Session, model string) string {
	sep := strings.Repeat(i18n.T("separator_double"), 76)
	var b strings.Builder
	w := func(f string, args ...interface{}) { b.WriteString(fmt.Sprintf(f, args...) + "\n") }

	w(sep)
	w("  "+i18n.T("compare_title")+"  (model: %s)", model)
	w(sep)
	w("")
	header := fmt.Sprintf("  %-28s %4s %5s %5s %5s %9s %7s", i18n.T("session"), i18n.T("turns_header"), i18n.T("tools"), i18n.T("succ_pct"), i18n.T("succ_pct"), i18n.T("cost"), i18n.T("health"))
	w(header)
	w("  " + strings.Repeat(i18n.T("separator_single"), 70))

	for _, s := range sessions {
		m := s.Metrics
		totalTools := m.ToolCallsOK + m.ToolCallsFail
		sr := "N/A"
		if totalTools > 0 {
			sr = fmt.Sprintf("%.0f%%", float64(m.ToolCallsOK)/float64(totalTools)*100)
		}
		hEmoji := HealthEmoji(s.Health)
		name := s.Name
		if len(name) > 27 {
			name = name[:27]
		}
		w("  %-28s %4d %5d %5d %5s $%8.4f %s %4d/100",
			name, m.UserMessages, m.AssistantTurns, m.ToolCallsTotal,
			sr, m.CostEstimated, hEmoji, s.Health)
	}
	w(sep)
	return b.String()
}
