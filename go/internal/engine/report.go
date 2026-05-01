package engine

import (
	"encoding/json"
	"fmt"
	"sort"
	"strings"
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

	sep := strings.Repeat("━", 60)
	sub := strings.Repeat("─", 40)

	var b strings.Builder
	w := func(f string, args ...interface{}) { b.WriteString(fmt.Sprintf(f, args...) + "\n") }

	w(sep)
	w("  AGENTTRACE v%s — AI Agent Session Performance Report", Version)
	w(sep)
	w("")

	// Token Cost
	w("💰 TOKEN COST")
	w(sub)
	w("  Input:        %10d  tokens", m.TokensInput)
	w("  Output:       %10d  tokens", m.TokensOutput)
	if m.TokensCacheW > 0 || m.TokensCacheR > 0 {
		w("  Cache write:  %10d  tokens", m.TokensCacheW)
		w("  Cache read:   %10d  tokens", m.TokensCacheR)
	}
	w("  ────────────────────────────────────")
	w("  Total tokens: %10d", totalTokens)
	w("  Est. cost:    $%11.4f  (model: %s)", m.CostEstimated, m.ModelUsed)
	w("")

	// Activity
	w("📊 ACTIVITY")
	w(sub)
	w("  Messages:    %d user  |  %d turns", m.UserMessages, m.AssistantTurns)
	w("  Tool calls:  %d", m.ToolCallsTotal)
	if totalTools > 0 {
		srEmoji := "🟢"
		rate := float64(m.ToolCallsOK) / float64(totalTools)
		if rate < 0.70 {
			srEmoji = "🔴"
		} else if rate < 0.85 {
			srEmoji = "🟡"
		}
		w("  Success:     %s (%d/%d) %s", successRate, m.ToolCallsOK, totalTools, srEmoji)
	}
	w("")

	// Latency
	w("⏱️  LATENCY")
	w(sub)
	if len(gaps) > 0 {
		w("  min:     %.1fs", gaps[0])
		w("  median:  %.1fs", percentile(gaps, 0.50))
		w("  p95:     %.1fs", percentile(gaps, 0.95))
		w("  max:     %.1fs", gaps[len(gaps)-1])
		sum := 0.0
		for _, g := range gaps {
			sum += g
		}
		w("  avg:     %.1fs", sum/float64(len(gaps)))
	} else {
		w("  (no gap data)")
	}
	w("  Duration: %s", fmtDuration(m.DurationSec))
	w("")

	// Top Tools
	if len(m.ToolUsage) > 0 {
		w("🔧 TOP TOOLS")
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
	w("🧠 THINKING / COT")
	w(sub)
	if m.ReasoningBlocks > 0 {
		qualityLbl := "deep"
		qEmoji := "🟢"
		if avgReason < 400 {
			qualityLbl = "shallow"
			qEmoji = "🔴"
		} else if avgReason < 800 {
			qualityLbl = "moderate"
			qEmoji = "🟡"
		}
		w("  Blocks: %d", m.ReasoningBlocks)
		w("  Avg:    %.0f chars", avgReason)
		w("  Total:  %d chars", m.ReasoningChars)
		w("  Quality: %s %s", qEmoji, qualityLbl)
		if m.ReasoningRedact > 0 {
			w("  ⚠️  %d blocks REDACTED", m.ReasoningRedact)
		}
	} else {
		w("  (no thinking blocks)")
	}
	w("")

	// Anomalies
	w("🚨 ANOMALIES")
	w(sub)
	if len(anoms) > 0 {
		for _, a := range anoms {
			w("  %s [%s] %s: %s", a.Emoji, strings.ToUpper(a.Severity), a.Type, a.Detail)
		}
	} else {
		w("  ✅ No anomalies detected")
	}
	w("")

	// Health Score
	w("💯 HEALTH SCORE")
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
	sep := strings.Repeat("━", 76)
	var b strings.Builder
	w := func(f string, args ...interface{}) { b.WriteString(fmt.Sprintf(f, args...) + "\n") }

	w(sep)
	w("  AGENTTRACE — Multi-Session Comparison  (model: %s)", model)
	w(sep)
	w("")
	header := fmt.Sprintf("  %-28s %4s %5s %5s %5s %9s %7s", "Session", "Msgs", "Turns", "Tools", "Succ", "Cost", "Health")
	w(header)
	w("  " + strings.Repeat("─", 70))

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
