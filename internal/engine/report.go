package engine

import (
	"encoding/json"
	"fmt"
	"html"
	"sort"
	"strings"
	"unicode/utf8"

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
	w := func(s string) { b.WriteString(s + "\n") }
	wf := func(f string, args ...interface{}) { b.WriteString(fmt.Sprintf(f, args...) + "\n") }

	w(sep)
	w(fmt.Sprintf("  "+i18n.T("title"), Version))
	w(sep)
	w("")

	// Token Cost
	w(i18n.T("waste_cost"))
	w(sub)
	wf("  "+i18n.T("input"), m.TokensInput)
	wf("  "+i18n.T("output"), m.TokensOutput)
	if m.TokensCacheW > 0 || m.TokensCacheR > 0 {
		wf("  "+i18n.T("cache_write"), m.TokensCacheW)
		wf("  "+i18n.T("cache_read"), m.TokensCacheR)
	}
	w("  ────────────────────────────────────")
	wf("  "+i18n.T("total_tokens"), totalTokens)
	wf("  "+i18n.T("est_cost"), m.CostEstimated, m.ModelUsed)
	w("")

	// Activity
	w(i18n.T("activity"))
	w(sub)
	wf("  "+i18n.T("messages_label"), m.UserMessages, m.AssistantTurns)
	wf("  "+i18n.T("tool_calls_label"), m.ToolCallsTotal)
	if totalTools > 0 {
		srEmoji := "🟢"
		rate := float64(m.ToolCallsOK) / float64(totalTools)
		if rate < 0.70 {
			srEmoji = "🔴"
		} else if rate < 0.85 {
			srEmoji = "🟡"
		}
		wf("  "+i18n.T("success_label"), successRate, m.ToolCallsOK, totalTools, srEmoji)
	}
	w("")

	// Latency
	w(i18n.T("latency"))
	w(sub)
	if len(gaps) > 0 {
		wf("  "+i18n.T("min_lat"), gaps[0])
		wf("  "+i18n.T("median"), percentile(gaps, 0.50))
		wf("  "+i18n.T("p95"), percentile(gaps, 0.95))
		wf("  "+i18n.T("max_lat"), gaps[len(gaps)-1])
		sum := 0.0
		for _, g := range gaps {
			sum += g
		}
		wf("  "+i18n.T("avg_lat"), sum/float64(len(gaps)))
	} else {
		w("  " + i18n.T("no_gap_data"))
	}
	wf("  "+i18n.T("duration"), FmtDuration(m.DurationSec))
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
			wf("  %-35s %4d", item.k, item.v)
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
		wf("  "+i18n.T("blocks"), m.ReasoningBlocks)
		wf("  "+i18n.T("avg_chars"), avgReason)
		wf("  "+i18n.T("total_chars"), m.ReasoningChars)
		wf("  "+i18n.T("quality_label"), qEmoji, qualityLbl)
		if m.ReasoningRedact > 0 {
			wf("  "+i18n.T("redacted_blocks"), m.ReasoningRedact)
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
			wf("  %s [%s] %s: %s", a.Emoji, reportSeverityLabel(a.Severity), reportAnomalyTypeLabel(a.Type), a.Detail)
		}
	} else {
		w("  " + i18n.T("no_anomalies"))
	}
	w("")

	// Loop Cost
	if m.LoopGroups > 0 {
		w("🔄 " + i18n.T("loop_section_title"))
		w(sub)
		wf("  "+i18n.T("loop_tool_loop_cost"), m.LoopCostEst, m.LoopGroups)
		wf("  "+i18n.T("loop_retry_events"), m.LoopRetryEvents)
		w("")
	}

	// Health Score
	w(i18n.T("health_score"))
	w(sub)
	hBar := HealthBar(h)
	hEmoji := HealthEmoji(h)
	wf("  %s  %d/100  %s", hEmoji, h, hBar)
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
		"version":     Version,
		"model_used":  m.ModelUsed,
		"source_tool": m.SourceTool,
		"session": map[string]interface{}{
			"start":            m.SessionStart,
			"end":              m.SessionEnd,
			"duration_seconds": m.DurationSec,
			"duration_human":   FmtDuration(m.DurationSec),
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
		"tool_authority": map[string]interface{}{
			"highest": highestAuthorityForMetrics(m),
			"counts":  copyReportIntMap(m.ToolAuthority),
		},
		"latency": map[string]float64{
			"min":    safeCalc(gaps, func(x []float64) float64 { return x[0] }),
			"median": safeCalc(gaps, func(x []float64) float64 { return percentile(x, 0.50) }),
			"p95":    safeCalc(gaps, func(x []float64) float64 { return percentile(x, 0.95) }),
			"max":    safeCalc(gaps, func(x []float64) float64 { return x[len(x)-1] }),
			"avg": safeCalc(gaps, func(x []float64) float64 {
				s := 0.0
				for _, v := range x {
					s += v
				}
				return s / float64(len(x))
			}),
		},
		"tools_top": top10,
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
	w := func(s string) { b.WriteString(s + "\n") }

	w(sep)
	ww := func(s string) { b.WriteString(s + "\n") }
	ww(fmt.Sprintf("  "+i18n.T("compare_title")+"  ("+i18n.T("model_label")+")", model))
	ww(sep)
	ww("")
	header := fmt.Sprintf("  %-28s %4s %5s %5s %5s %9s %7s",
		i18n.T("session"), i18n.T("turns_header"), i18n.T("tools"),
		i18n.T("succ_pct"), i18n.T("fail"), i18n.T("cost"), i18n.T("health"))
	ww(header)
	ww("  " + strings.Repeat(i18n.T("separator_single"), 70))

	for _, s := range sessions {
		m := s.Metrics
		totalTools := m.ToolCallsOK + m.ToolCallsFail
		sr := i18n.T("not_available")
		if totalTools > 0 {
			sr = fmt.Sprintf("%.0f%%", float64(m.ToolCallsOK)/float64(totalTools)*100)
		}
		failStr := fmt.Sprintf("%d", m.ToolCallsFail)
		hEmoji := HealthEmoji(s.Health)
		healthStr := fmt.Sprintf("%s %d/100", hEmoji, s.Health)
		name := s.Name
		if len(name) > 27 {
			name = name[:27]
		}
		ww(fmt.Sprintf("  %-28s %4d %5d %5s %5s $%8.4f %s",
			name, m.AssistantTurns, m.ToolCallsTotal,
			sr, failStr, m.CostEstimated, healthStr))
	}
	w(sep)
	return b.String()
}

// ReportCompareJSON generates multi-session comparison JSON.
func ReportCompareJSON(sessions []Session, model string) string {
	type item struct {
		Name    string `json:"name"`
		Metrics struct {
			Turns       int     `json:"turns"`
			Tools       int     `json:"tools"`
			SuccessRate string  `json:"success_rate"`
			Fail        int     `json:"fail"`
			Cost        float64 `json:"cost"`
		} `json:"metrics"`
		Health int `json:"health"`
	}

	var items []item
	for _, s := range sessions {
		m := s.Metrics
		totalTools := m.ToolCallsOK + m.ToolCallsFail
		sr := i18n.T("not_available")
		if totalTools > 0 {
			sr = fmt.Sprintf("%.0f%%", float64(m.ToolCallsOK)/float64(totalTools)*100)
		}
		it := item{Name: s.Name, Health: s.Health}
		it.Metrics.Turns = m.AssistantTurns
		it.Metrics.Tools = m.ToolCallsTotal
		it.Metrics.SuccessRate = sr
		it.Metrics.Fail = m.ToolCallsFail
		it.Metrics.Cost = m.CostEstimated
		items = append(items, it)
	}

	out, _ := json.MarshalIndent(items, "", "  ")
	return string(out)
}

// ReportOverviewJSON generates machine-readable global overview data.
func ReportOverviewJSON(ov Overview, sessions []Session) string {
	orderedSessions := canonicalOverviewSessions(sessions)
	type groupItem struct {
		Name     string  `json:"name"`
		Sessions int     `json:"sessions"`
		Cost     float64 `json:"cost"`
	}
	type recentSession struct {
		Name               string  `json:"name"`
		SourceTool         string  `json:"source_tool"`
		Model              string  `json:"model"`
		CWD                string  `json:"cwd,omitempty"`
		Turns              int     `json:"turns"`
		Tools              int     `json:"tools"`
		Tokens             int     `json:"tokens"`
		Cost               float64 `json:"cost"`
		Health             int     `json:"health"`
		Anomalies          int     `json:"anomalies"`
		Authority          string  `json:"highest_tool_authority"`
		PossibleCostDriver string  `json:"possible_cost_driver,omitempty"`
	}
	type trendPoint struct {
		Name   string  `json:"name"`
		Health int     `json:"health"`
		Cost   float64 `json:"cost"`
	}

	totalTokens := 0
	totalTools := 0
	failedTools := 0
	totalDuration := 0.0
	totalHealth := 0
	toolSurface := make(map[string]struct{})
	fileSurface := make(map[string]struct{})
	failureFamilies := make(map[string]struct{})
	authorityCounts := make(map[string]int)
	highestAuthority := ""
	for _, s := range orderedSessions {
		totalTokens += s.Metrics.TokensInput + s.Metrics.TokensOutput + s.Metrics.TokensCacheW + s.Metrics.TokensCacheR
		totalTools += s.Metrics.ToolCallsOK + s.Metrics.ToolCallsFail
		failedTools += s.Metrics.ToolCallsFail
		totalDuration += s.Metrics.DurationSec
		totalHealth += s.Health
		for tool := range s.Metrics.ToolUsage {
			toolSurface[tool] = struct{}{}
		}
		for file := range s.Metrics.FileUsage {
			fileSurface[file] = struct{}{}
		}
		for _, anomaly := range s.Anomalies {
			if anomaly.Type != "" {
				failureFamilies[anomaly.Type] = struct{}{}
			}
		}
		for authority, count := range s.Metrics.ToolAuthority {
			if count > 0 {
				authorityCounts[authority] += count
				highestAuthority = HigherToolAuthority(highestAuthority, authority)
			}
		}
		highestAuthority = HigherToolAuthority(highestAuthority, s.Metrics.HighestAuthority)
	}
	avgHealth := 0.0
	if len(orderedSessions) > 0 {
		avgHealth = round4(float64(totalHealth) / float64(len(orderedSessions)))
	}
	toolFailRate := 0.0
	if totalTools > 0 {
		toolFailRate = round4(float64(failedTools) / float64(totalTools) * 100)
	}

	agents := make([]groupItem, 0, len(ov.ByAgent))
	for k, v := range ov.ByAgent {
		name := k
		if display, ok := ToolDisplayNames[k]; ok {
			name = display
		}
		agents = append(agents, groupItem{Name: name, Sessions: v.Sessions, Cost: round4(v.Cost)})
	}
	sort.Slice(agents, func(i, j int) bool {
		if agents[i].Sessions == agents[j].Sessions {
			if agents[i].Cost == agents[j].Cost {
				return agents[i].Name < agents[j].Name
			}
			return agents[i].Cost > agents[j].Cost
		}
		return agents[i].Sessions > agents[j].Sessions
	})

	models := make([]groupItem, 0, len(ov.ByModel))
	for k, v := range ov.ByModel {
		models = append(models, groupItem{Name: k, Sessions: v.Sessions, Cost: round4(v.Cost)})
	}
	sort.Slice(models, func(i, j int) bool {
		if models[i].Cost == models[j].Cost {
			if models[i].Sessions == models[j].Sessions {
				return models[i].Name < models[j].Name
			}
			return models[i].Sessions > models[j].Sessions
		}
		return models[i].Cost > models[j].Cost
	})

	recentCap := len(orderedSessions)
	if recentCap > 10 {
		recentCap = 10
	}
	recent := make([]recentSession, 0, recentCap)
	for i, s := range orderedSessions {
		if i >= 10 {
			break
		}
		recent = append(recent, recentSession{
			Name:               s.Name,
			SourceTool:         s.Metrics.SourceTool,
			Model:              s.Metrics.ModelUsed,
			CWD:                s.CWD,
			Turns:              s.Metrics.AssistantTurns,
			Tools:              s.Metrics.ToolCallsOK + s.Metrics.ToolCallsFail,
			Tokens:             s.Metrics.TokensInput + s.Metrics.TokensOutput + s.Metrics.TokensCacheW + s.Metrics.TokensCacheR,
			Cost:               round4(s.Metrics.CostEstimated),
			Health:             s.Health,
			Anomalies:          len(s.Anomalies),
			Authority:          highestAuthorityForMetrics(s.Metrics),
			PossibleCostDriver: possibleCostDriverNote(s),
		})
	}
	anomalies := ov.AnomaliesTop
	if anomalies == nil {
		anomalies = []AnomalyTop{}
	}
	anomalyLimit := minReportInt(len(anomalies), 50)
	anomaliesReturned := anomalies[:anomalyLimit]
	trend := AnalyzeHealthTrend(orderedSessions)
	points := make([]trendPoint, 0, len(trend.Points))
	for _, p := range trend.Points {
		points = append(points, trendPoint{Name: p.Name, Health: p.Health, Cost: round4(p.Cost)})
	}
	incidentTimelines := overviewIncidentTimelines(orderedSessions, 10)

	payload := map[string]interface{}{
		"version": Version,
		"summary": map[string]interface{}{
			"total_sessions":         ov.TotalSessions,
			"healthy":                ov.Healthy,
			"warning":                ov.Warning,
			"critical":               ov.Critical,
			"avg_health":             avgHealth,
			"total_cost":             round4(ov.TotalCost),
			"total_duration_seconds": round4(totalDuration),
			"total_tokens":           totalTokens,
			"tool_calls":             totalTools,
			"tool_failures":          failedTools,
			"tool_fail_rate":         toolFailRate,
			"anomalies_total":        len(anomalies),
			"anomalies_returned":     len(anomaliesReturned),
			"anomalies_truncated":    len(anomaliesReturned) < len(anomalies),
			"health_trend": map[string]interface{}{
				"direction":  trend.Direction,
				"regressing": trend.Regressing,
				"avg_health": round4(trend.AvgHealth),
				"message":    trend.Message,
				"points":     points,
			},
			"tool_authority": map[string]interface{}{
				"highest": highestAuthority,
				"counts":  authorityCounts,
			},
		},
		"failure_families": sortedReportKeys(failureFamilies),
		"surfaces": map[string]interface{}{
			"tools":                sortedReportKeys(toolSurface),
			"files":                sortedReportKeys(fileSurface),
			"authority_categories": sortedReportIntKeys(authorityCounts),
			"high_authority_tools": highAuthorityTools(sortedReportKeys(toolSurface)),
		},
		"by_agent":           agents,
		"by_model":           models,
		"recent_sessions":    recent,
		"incident_timelines": incidentTimelines,
		"anomalies":          anomaliesReturned,
	}
	out, _ := json.MarshalIndent(payload, "", "  ")
	return string(out)
}

// ReportOverviewMarkdown generates a human-readable Markdown overview for PR comments and CI artifacts.
func ReportOverviewMarkdown(ov Overview, sessions []Session) string {
	orderedSessions := canonicalOverviewSessions(sessions)
	summary := overviewReportSummary(orderedSessions)
	authority := overviewAuthoritySummary(orderedSessions)
	trend := AnalyzeHealthTrend(orderedSessions)

	var b strings.Builder
	fmt.Fprintf(&b, "# %s\n\n", i18n.T("report_md_title"))
	fmt.Fprintf(&b, "| %s | %s |\n|---|---:|\n", i18n.T("report_metric"), i18n.T("report_value"))
	fmt.Fprintf(&b, "| %s | %d |\n", i18n.T("report_sessions"), ov.TotalSessions)
	fmt.Fprintf(&b, "| %s | %d / %d / %d |\n", i18n.T("report_health_breakdown"), ov.Healthy, ov.Warning, ov.Critical)
	fmt.Fprintf(&b, "| %s | %.1f |\n", i18n.T("report_avg_health"), summary.AvgHealth)
	fmt.Fprintf(&b, "| %s | %s |\n", i18n.T("trend_title"), markdownCell(trend.Message))
	fmt.Fprintf(&b, "| %s | $%.2f |\n", i18n.T("total_cost"), ov.TotalCost)
	fmt.Fprintf(&b, "| %s | %d |\n", i18n.T("report_total_tokens"), summary.TotalTokens)
	fmt.Fprintf(&b, "| %s | %d / %d (%.1f%%) |\n\n", i18n.T("report_tool_failures"), summary.FailedTools, summary.TotalTools, summary.ToolFailRate)

	if authority.HasData {
		fmt.Fprintf(&b, "## %s\n\n", i18n.T("report_tool_authority"))
		fmt.Fprintf(&b, "| %s | %s |\n|---|---:|\n", i18n.T("report_metric"), i18n.T("report_value"))
		if authority.Highest != "" {
			fmt.Fprintf(&b, "| %s | `%s` |\n", i18n.T("report_highest_authority"), markdownInlineCode(authority.Highest))
		}
		if len(authority.HighTools) > 0 {
			fmt.Fprintf(&b, "| %s | %s |\n", i18n.T("report_high_authority_tools"), reportMarkdownCodeList(authority.HighTools))
		}
		if len(authority.Counts) > 0 {
			fmt.Fprintf(&b, "\n### %s\n\n", i18n.T("report_authority_category_counts"))
			fmt.Fprintf(&b, "| %s | %s |\n|---|---:|\n", i18n.T("report_authority_category"), i18n.T("report_count"))
			for _, item := range authority.Counts {
				fmt.Fprintf(&b, "| `%s` | %d |\n", markdownInlineCode(item.Category), item.Count)
			}
			fmt.Fprintln(&b)
		}
	}

	if notes := overviewCostDriverNotes(orderedSessions, 6); len(notes) > 0 {
		fmt.Fprintf(&b, "## %s\n\n", i18n.T("report_possible_cost_drivers"))
		for _, note := range notes {
			fmt.Fprintf(&b, "- **%s**: %s\n", markdownCell(note.Session), markdownCell(note.Note))
		}
		fmt.Fprintln(&b)
	}

	fmt.Fprintf(&b, "## %s\n\n", i18n.T("incident_timeline_title"))
	timelines := overviewIncidentTimelines(orderedSessions, 6)
	if len(timelines) == 0 {
		fmt.Fprintf(&b, "%s\n\n", i18n.T("incident_timeline_no_evidence"))
	} else {
		fmt.Fprintf(&b, "| %s | %s | %s | %s |\n|---|---|---|---|\n",
			i18n.T("report_session"), i18n.T("incident_timeline_signal"), i18n.T("incident_timeline_evidence"), i18n.T("incident_timeline_severity"))
		for _, timeline := range timelines {
			for _, item := range timeline.Items {
				fmt.Fprintf(&b, "| %s | %s | %s | %s |\n",
					markdownCell(timeline.Session),
					markdownCell(item.Label),
					markdownCell(item.Detail),
					markdownCell(reportSeverityLabel(item.Severity)))
			}
		}
		fmt.Fprintln(&b)
	}

	fmt.Fprintf(&b, "## %s\n\n", i18n.T("report_by_agent"))
	fmt.Fprintf(&b, "| %s | %s | %s |\n|---|---:|---:|\n", i18n.T("report_agent"), i18n.T("report_sessions"), i18n.T("report_cost"))
	type akv struct {
		k string
		v AgentOverview
	}
	var agents []akv
	for k, v := range ov.ByAgent {
		agents = append(agents, akv{k, v})
	}
	sort.Slice(agents, func(i, j int) bool {
		if agents[i].v.Sessions == agents[j].v.Sessions {
			if agents[i].v.Cost == agents[j].v.Cost {
				return agents[i].k < agents[j].k
			}
			return agents[i].v.Cost > agents[j].v.Cost
		}
		return agents[i].v.Sessions > agents[j].v.Sessions
	})
	for _, a := range agents {
		display := a.k
		if d, ok := ToolDisplayNames[a.k]; ok {
			display = d
		}
		fmt.Fprintf(&b, "| %s | %d | $%.2f |\n", markdownCell(display), a.v.Sessions, a.v.Cost)
	}

	fmt.Fprintf(&b, "\n## %s\n\n", i18n.T("report_recent_sessions"))
	fmt.Fprintf(&b, "| %s | %s | %s | %s | %s | %s |\n|---|---|---|---:|---:|---:|\n",
		i18n.T("report_session"), i18n.T("report_source"), i18n.T("report_model"), i18n.T("report_health"), i18n.T("report_cost"), i18n.T("report_anomalies"))
	limit := len(orderedSessions)
	if limit > 10 {
		limit = 10
	}
	for i := 0; i < limit; i++ {
		s := orderedSessions[i]
		source := s.Metrics.SourceTool
		if d, ok := ToolDisplayNames[source]; ok {
			source = d
		}
		fmt.Fprintf(&b, "| %s | %s | %s | %d | $%.4f | %d |\n",
			markdownCell(s.Name),
			markdownCell(source),
			markdownCell(s.Metrics.ModelUsed),
			s.Health,
			s.Metrics.CostEstimated,
			len(s.Anomalies))
	}

	fmt.Fprintf(&b, "\n## %s\n\n", i18n.T("report_recent_anomalies"))
	if len(ov.AnomaliesTop) == 0 {
		fmt.Fprintf(&b, "%s\n", i18n.T("report_no_anomalies"))
		return b.String()
	}
	fmt.Fprintf(&b, "| %s | %s | %s |\n|---|---|---|\n", i18n.T("report_session"), i18n.T("report_type"), i18n.T("report_age"))
	anomLimit := len(ov.AnomaliesTop)
	if anomLimit > 10 {
		anomLimit = 10
	}
	for i := 0; i < anomLimit; i++ {
		a := ov.AnomaliesTop[i]
		fmt.Fprintf(&b, "| %s | %s | %s |\n", markdownCell(a.Session), markdownCell(reportAnomalyTypeLabel(a.Type)), markdownCell(a.Age))
	}
	return b.String()
}

// ReportOverviewHTML generates a self-contained HTML report for CI artifacts and sharing.
func ReportOverviewHTML(ov Overview, sessions []Session) string {
	orderedSessions := canonicalOverviewSessions(sessions)
	summary := overviewReportSummary(orderedSessions)
	authority := overviewAuthoritySummary(orderedSessions)
	trend := AnalyzeHealthTrend(orderedSessions)
	agents := sortedAgents(ov.ByAgent)
	models := sortedModels(ov.ByModel)

	var b strings.Builder
	w := func(s string) { b.WriteString(s + "\n") }
	w(`<!doctype html>`)
	w(fmt.Sprintf(`<html lang="%s">`, html.EscapeString(reportLangCode())))
	w(`<head>`)
	w(`<meta charset="utf-8">`)
	w(`<meta name="viewport" content="width=device-width, initial-scale=1">`)
	w(fmt.Sprintf(`<title>%s</title>`, html.EscapeString(i18n.T("report_html_title"))))
	w(`<link rel="icon" href="data:,">`)
	w(`<style>`)
	w(`:root{color-scheme:dark;--bg:#07090b;--panel:#101419;--line:#273039;--text:#f4f0dd;--muted:#a9a391;--green:#54ff00;--cyan:#00d8ff;--amber:#ffb000;--red:#ff4a4a}`)
	w(`*{box-sizing:border-box}body{margin:0;background:linear-gradient(180deg,#0b0f12,#050607);color:var(--text);font:15px/1.55 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace}`)
	w(`main{max-width:1180px;margin:0 auto;padding:32px 18px 48px}header{display:flex;justify-content:space-between;gap:24px;align-items:flex-start;border-bottom:1px solid var(--line);padding-bottom:24px;margin-bottom:24px}`)
	w(`h1{font-size:clamp(42px,7vw,88px);line-height:.9;margin:0;letter-spacing:0}h2{margin:0 0 14px;font-size:20px;color:var(--cyan)}p{margin:10px 0 0;color:var(--muted)}`)
	w(`.brand{color:var(--green);font-weight:800}.meta{text-align:right;color:var(--muted)}.grid{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:1px;background:var(--line);border:1px solid var(--line);margin:24px 0}.metric{background:var(--panel);padding:18px;min-height:120px}.metric span{display:block;color:var(--muted);font-size:12px;text-transform:uppercase}.metric strong{display:block;margin-top:12px;font-size:30px;color:var(--green)}.warn strong{color:var(--amber)}.bad strong{color:var(--red)}`)
	w(`section{border:1px solid var(--line);background:rgba(16,20,25,.78);padding:20px;margin-top:20px}table{width:100%;border-collapse:collapse}th,td{padding:10px;border-bottom:1px solid var(--line);text-align:left;vertical-align:top}th{color:var(--muted);font-size:12px;text-transform:uppercase}td.num,th.num{text-align:right}.health-good{color:var(--green)}.health-warn{color:var(--amber)}.health-bad{color:var(--red)}code{color:var(--cyan)}@media(max-width:760px){header{display:block}.meta{text-align:left;margin-top:16px}.grid{grid-template-columns:1fr}table{font-size:13px}}`)
	w(`</style>`)
	w(`</head>`)
	w(`<body>`)
	w(`<main>`)
	w(`<header>`)
	w(fmt.Sprintf(`<div><div class="brand">agenttrace</div><h1>%s</h1><p>%s</p></div>`, html.EscapeString(i18n.T("report_html_h1")), html.EscapeString(i18n.T("report_html_subtitle"))))
	w(fmt.Sprintf(`<div class="meta">v%s<br>%d %s<br><code>agenttrace --overview -f html</code></div>`, html.EscapeString(Version), ov.TotalSessions, html.EscapeString(i18n.T("report_sessions"))))
	w(`</header>`)
	w(`<div class="grid" aria-label="summary metrics">`)
	w(fmt.Sprintf(`<div class="metric"><span>%s</span><strong>%d</strong><p>%d %s / %d %s / %d %s</p></div>`, html.EscapeString(i18n.T("report_sessions")), ov.TotalSessions, ov.Healthy, html.EscapeString(i18n.T("overview_healthy")), ov.Warning, html.EscapeString(i18n.T("overview_warning")), ov.Critical, html.EscapeString(i18n.T("overview_critical"))))
	w(fmt.Sprintf(`<div class="metric"><span>%s</span><strong>%d</strong><p>%s</p></div>`, html.EscapeString(i18n.T("report_total_tokens")), summary.TotalTokens, html.EscapeString(i18n.T("metric_live"))))
	w(fmt.Sprintf(`<div class="metric"><span>%s</span><strong>%s</strong><p>%s</p></div>`, html.EscapeString(i18n.T("report_avg_health")), html.EscapeString(fmt.Sprintf("%.1f", summary.AvgHealth)), html.EscapeString(i18n.T("report_fleet_quality"))))
	w(fmt.Sprintf(`<div class="metric"><span>%s</span><strong>$%.2f</strong><p>%s</p></div>`, html.EscapeString(i18n.T("total_cost")), ov.TotalCost, html.EscapeString(i18n.T("report_estimated_cost"))))
	w(fmt.Sprintf(`<div class="metric %s"><span>%s</span><strong>%d/%d</strong><p>%s</p></div>`, html.EscapeString(failureClass(summary.ToolFailRate)), html.EscapeString(i18n.T("report_tool_failures")), summary.FailedTools, summary.TotalTools, html.EscapeString(fmt.Sprintf(i18n.T("report_failure_rate"), summary.ToolFailRate))))
	w(`</div>`)
	if authority.HasData {
		w(fmt.Sprintf(`<section><h2>%s</h2>`, html.EscapeString(i18n.T("report_tool_authority"))))
		if authority.Highest != "" {
			w(fmt.Sprintf(`<p><strong>%s</strong>: <code>%s</code></p>`, html.EscapeString(i18n.T("report_highest_authority")), html.EscapeString(authority.Highest)))
		}
		if len(authority.Counts) > 0 {
			w(fmt.Sprintf(`<table><caption>%s</caption><thead><tr><th>%s</th><th class="num">%s</th></tr></thead><tbody>`,
				html.EscapeString(i18n.T("report_authority_category_counts")), html.EscapeString(i18n.T("report_authority_category")), html.EscapeString(i18n.T("report_count"))))
			for _, item := range authority.Counts {
				w(fmt.Sprintf(`<tr><td><code>%s</code></td><td class="num">%d</td></tr>`, html.EscapeString(item.Category), item.Count))
			}
			w(`</tbody></table>`)
		}
		if len(authority.HighTools) > 0 {
			w(fmt.Sprintf(`<p><strong>%s</strong>: %s</p>`, html.EscapeString(i18n.T("report_high_authority_tools")), reportHTMLCodeList(authority.HighTools)))
		}
		w(`</section>`)
	}
	if notes := overviewCostDriverNotes(orderedSessions, 8); len(notes) > 0 {
		w(fmt.Sprintf(`<section><h2>%s</h2><table><thead><tr><th>%s</th><th>%s</th></tr></thead><tbody>`,
			html.EscapeString(i18n.T("report_possible_cost_drivers")), html.EscapeString(i18n.T("report_session")), html.EscapeString(i18n.T("report_evidence"))))
		for _, note := range notes {
			w(fmt.Sprintf(`<tr><td>%s</td><td>%s</td></tr>`, html.EscapeString(note.Session), html.EscapeString(note.Note)))
		}
		w(`</tbody></table></section>`)
	}
	if len(orderedSessions) > 1 {
		w(fmt.Sprintf(`<section><h2>%s</h2><p>%s</p></section>`, html.EscapeString(i18n.T("trend_title")), html.EscapeString(trend.Message)))
	}

	w(fmt.Sprintf(`<section><h2>%s</h2>`, html.EscapeString(i18n.T("incident_timeline_title"))))
	timelines := overviewIncidentTimelines(orderedSessions, 8)
	if len(timelines) == 0 {
		w(fmt.Sprintf(`<p>%s</p>`, html.EscapeString(i18n.T("incident_timeline_no_evidence"))))
	} else {
		w(fmt.Sprintf(`<table><thead><tr><th>%s</th><th>%s</th><th>%s</th><th>%s</th></tr></thead><tbody>`,
			html.EscapeString(i18n.T("report_session")), html.EscapeString(i18n.T("incident_timeline_signal")), html.EscapeString(i18n.T("incident_timeline_evidence")), html.EscapeString(i18n.T("incident_timeline_severity"))))
		for _, timeline := range timelines {
			for _, item := range timeline.Items {
				w(fmt.Sprintf(`<tr><td>%s</td><td>%s</td><td>%s</td><td>%s</td></tr>`,
					html.EscapeString(timeline.Session),
					html.EscapeString(item.Label),
					html.EscapeString(item.Detail),
					html.EscapeString(reportSeverityLabel(item.Severity))))
			}
		}
		w(`</tbody></table>`)
	}
	w(`</section>`)

	w(fmt.Sprintf(`<section><h2>%s</h2><table><thead><tr><th>%s</th><th>%s</th><th>%s</th><th class="num">%s</th><th class="num">%s</th><th class="num">%s</th><th class="num">%s</th></tr></thead><tbody>`,
		html.EscapeString(i18n.T("report_recent_sessions")), html.EscapeString(i18n.T("report_session")), html.EscapeString(i18n.T("report_source")), html.EscapeString(i18n.T("report_model")), html.EscapeString(i18n.T("report_total_tokens")), html.EscapeString(i18n.T("report_cost")), html.EscapeString(i18n.T("report_health")), html.EscapeString(i18n.T("report_anomalies"))))
	limit := minReportInt(len(orderedSessions), 20)
	for i := 0; i < limit; i++ {
		s := orderedSessions[i]
		source := s.Metrics.SourceTool
		if d, ok := ToolDisplayNames[source]; ok {
			source = d
		}
		tokens := s.Metrics.TokensInput + s.Metrics.TokensOutput + s.Metrics.TokensCacheW + s.Metrics.TokensCacheR
		w(fmt.Sprintf(`<tr><td>%s</td><td>%s</td><td>%s</td><td class="num">%d</td><td class="num">$%.4f</td><td class="num %s">%d</td><td class="num">%d</td></tr>`,
			html.EscapeString(s.Name),
			html.EscapeString(source),
			html.EscapeString(s.Metrics.ModelUsed),
			tokens,
			s.Metrics.CostEstimated,
			html.EscapeString(healthClass(s.Health)),
			s.Health,
			len(s.Anomalies)))
	}
	w(`</tbody></table></section>`)

	w(fmt.Sprintf(`<section><h2>%s</h2><table><thead><tr><th>%s</th><th class="num">%s</th><th class="num">%s</th></tr></thead><tbody>`,
		html.EscapeString(i18n.T("report_by_agent")), html.EscapeString(i18n.T("report_agent")), html.EscapeString(i18n.T("report_sessions")), html.EscapeString(i18n.T("report_cost"))))
	for _, a := range agents {
		display := a.k
		if d, ok := ToolDisplayNames[a.k]; ok {
			display = d
		}
		w(fmt.Sprintf(`<tr><td>%s</td><td class="num">%d</td><td class="num">$%.2f</td></tr>`, html.EscapeString(display), a.v.Sessions, a.v.Cost))
	}
	w(`</tbody></table></section>`)

	w(fmt.Sprintf(`<section><h2>%s</h2><table><thead><tr><th>%s</th><th class="num">%s</th><th class="num">%s</th></tr></thead><tbody>`,
		html.EscapeString(i18n.T("report_by_model")), html.EscapeString(i18n.T("report_model")), html.EscapeString(i18n.T("report_sessions")), html.EscapeString(i18n.T("report_cost"))))
	modelLimit := minReportInt(len(models), 12)
	for i := 0; i < modelLimit; i++ {
		mdl := models[i]
		w(fmt.Sprintf(`<tr><td>%s</td><td class="num">%d</td><td class="num">$%.2f</td></tr>`, html.EscapeString(mdl.k), mdl.v.Sessions, mdl.v.Cost))
	}
	w(`</tbody></table></section>`)

	w(fmt.Sprintf(`<section><h2>%s</h2>`, html.EscapeString(i18n.T("report_recent_anomalies"))))
	if len(ov.AnomaliesTop) == 0 {
		w(fmt.Sprintf(`<p>%s</p>`, html.EscapeString(i18n.T("report_no_anomalies"))))
	} else {
		w(fmt.Sprintf(`<table><thead><tr><th>%s</th><th>%s</th><th>%s</th></tr></thead><tbody>`,
			html.EscapeString(i18n.T("report_session")), html.EscapeString(i18n.T("report_type")), html.EscapeString(i18n.T("report_age"))))
		anomLimit := minReportInt(len(ov.AnomaliesTop), 20)
		for i := 0; i < anomLimit; i++ {
			a := ov.AnomaliesTop[i]
			w(fmt.Sprintf(`<tr><td>%s</td><td>%s</td><td>%s</td></tr>`, html.EscapeString(a.Session), html.EscapeString(reportAnomalyTypeLabel(a.Type)), html.EscapeString(a.Age)))
		}
		w(`</tbody></table>`)
	}
	w(`</section>`)
	w(`</main>`)
	w(`</body>`)
	w(`</html>`)
	return b.String()
}

func reportLangCode() string {
	if i18n.Current == i18n.ZH {
		return "zh"
	}
	return "en"
}

func reportAnomalyTypeLabel(kind string) string {
	key := "anomaly_type_" + kind
	if translated := i18n.T(key); translated != key {
		return translated
	}
	return strings.ReplaceAll(kind, "_", " ")
}

func reportSeverityLabel(severity string) string {
	switch strings.ToLower(severity) {
	case SeverityHigh:
		return i18n.T("severity_high")
	case SeverityMedium:
		return i18n.T("severity_medium")
	case SeverityLow:
		return i18n.T("severity_low")
	default:
		return strings.ToUpper(severity)
	}
}

type overviewSummary struct {
	TotalTokens  int
	TotalTools   int
	FailedTools  int
	AvgHealth    float64
	ToolFailRate float64
}

type authorityCount struct {
	Category string
	Count    int
}

type overviewAuthority struct {
	Highest   string
	Counts    []authorityCount
	HighTools []string
	HasData   bool
}

type costDriverNote struct {
	Session string
	Note    string
}

func overviewCostDriverNotes(sessions []Session, limit int) []costDriverNote {
	if limit <= 0 {
		return nil
	}
	notes := make([]costDriverNote, 0, minReportInt(len(sessions), limit))
	for _, s := range sessions {
		note := possibleCostDriverNote(s)
		if note == "" {
			continue
		}
		notes = append(notes, costDriverNote{Session: s.Name, Note: note})
		if len(notes) >= limit {
			break
		}
	}
	return notes
}

func possibleCostDriverNote(s Session) string {
	if s.ContextUtil.RiskLevel == "critical" || s.ContextUtil.RiskLevel == "warning" {
		return fmt.Sprintf(i18n.T("cost_driver_context_pressure"), s.ContextUtil.RiskLevel, s.ContextUtil.UtilizationPct)
	}
	if len(s.LargeParams) > 0 {
		return fmt.Sprintf(i18n.T("cost_driver_large_params"), len(s.LargeParams))
	}
	if s.LoopCost.TotalLoopCost > 0 || s.LoopResultData.HasLoop || s.Metrics.LoopRetryEvents > 0 || len(s.LoopFingerprints) > 0 {
		return i18n.T("cost_driver_retry_loop")
	}
	totalTools := s.Metrics.ToolCallsOK + s.Metrics.ToolCallsFail
	if totalTools > 0 {
		failRate := float64(s.Metrics.ToolCallsFail) / float64(totalTools) * 100
		if failRate >= 25 {
			return fmt.Sprintf(i18n.T("cost_driver_tool_failures"), s.Metrics.ToolCallsFail, totalTools, failRate)
		}
	}
	if tokensPerAssistantTurn(s) >= 50000 {
		return fmt.Sprintf(i18n.T("cost_driver_high_tokens_per_turn"), tokensPerAssistantTurn(s), s.Metrics.AssistantTurns)
	}
	return ""
}

func tokensPerAssistantTurn(s Session) int {
	if s.Metrics.AssistantTurns <= 0 {
		return 0
	}
	totalTokens := s.Metrics.TokensInput + s.Metrics.TokensOutput + s.Metrics.TokensCacheW + s.Metrics.TokensCacheR
	if totalTokens <= 0 {
		return 0
	}
	return totalTokens / s.Metrics.AssistantTurns
}

func overviewReportSummary(sessions []Session) overviewSummary {
	var summary overviewSummary
	totalHealth := 0
	for _, s := range sessions {
		summary.TotalTokens += s.Metrics.TokensInput + s.Metrics.TokensOutput + s.Metrics.TokensCacheW + s.Metrics.TokensCacheR
		summary.TotalTools += s.Metrics.ToolCallsOK + s.Metrics.ToolCallsFail
		summary.FailedTools += s.Metrics.ToolCallsFail
		totalHealth += s.Health
	}
	if len(sessions) > 0 {
		summary.AvgHealth = round4(float64(totalHealth) / float64(len(sessions)))
	}
	if summary.TotalTools > 0 {
		summary.ToolFailRate = round4(float64(summary.FailedTools) / float64(summary.TotalTools) * 100)
	}
	return summary
}

func overviewAuthoritySummary(sessions []Session) overviewAuthority {
	counts := make(map[string]int)
	toolSurface := make(map[string]struct{})
	highest := ""
	for _, s := range sessions {
		for tool := range s.Metrics.ToolUsage {
			toolSurface[tool] = struct{}{}
		}
		for category, count := range s.Metrics.ToolAuthority {
			if count > 0 {
				counts[category] += count
				highest = HigherToolAuthority(highest, category)
			}
		}
		highest = HigherToolAuthority(highest, s.Metrics.HighestAuthority)
	}
	keys := sortedReportIntKeys(counts)
	items := make([]authorityCount, 0, len(keys))
	for _, key := range keys {
		items = append(items, authorityCount{Category: key, Count: counts[key]})
	}
	highTools := highAuthorityTools(sortedReportKeys(toolSurface))
	return overviewAuthority{
		Highest:   highest,
		Counts:    items,
		HighTools: highTools,
		HasData:   highest != "" || len(items) > 0 || len(highTools) > 0,
	}
}

func canonicalOverviewSessions(sessions []Session) []Session {
	ordered := append([]Session(nil), sessions...)
	sort.SliceStable(ordered, func(i, j int) bool {
		ti := parseTS(ordered[i].Metrics.SessionStart)
		tj := parseTS(ordered[j].Metrics.SessionStart)
		if !ti.IsZero() || !tj.IsZero() {
			if ti.IsZero() != tj.IsZero() {
				return !ti.IsZero()
			}
			if !ti.Equal(tj) {
				return ti.After(tj)
			}
		}
		if ordered[i].Name != ordered[j].Name {
			return ordered[i].Name < ordered[j].Name
		}
		return ordered[i].Path < ordered[j].Path
	})
	return ordered
}

type agentKV struct {
	k string
	v AgentOverview
}

func sortedAgents(items map[string]AgentOverview) []agentKV {
	agents := make([]agentKV, 0, len(items))
	for k, v := range items {
		agents = append(agents, agentKV{k, v})
	}
	sort.Slice(agents, func(i, j int) bool {
		if agents[i].v.Sessions == agents[j].v.Sessions {
			if agents[i].v.Cost == agents[j].v.Cost {
				return agents[i].k < agents[j].k
			}
			return agents[i].v.Cost > agents[j].v.Cost
		}
		return agents[i].v.Sessions > agents[j].v.Sessions
	})
	return agents
}

type modelKV struct {
	k string
	v ModelOverview
}

func sortedModels(items map[string]ModelOverview) []modelKV {
	models := make([]modelKV, 0, len(items))
	for k, v := range items {
		models = append(models, modelKV{k, v})
	}
	sort.Slice(models, func(i, j int) bool {
		if models[i].v.Cost == models[j].v.Cost {
			if models[i].v.Sessions == models[j].v.Sessions {
				return models[i].k < models[j].k
			}
			return models[i].v.Sessions > models[j].v.Sessions
		}
		return models[i].v.Cost > models[j].v.Cost
	})
	return models
}

func healthClass(health int) string {
	switch {
	case health >= 80:
		return "health-good"
	case health >= 50:
		return "health-warn"
	default:
		return "health-bad"
	}
}

func failureClass(rate float64) string {
	if rate >= 25 {
		return "bad"
	}
	if rate >= 10 {
		return "warn"
	}
	return ""
}

func minReportInt(a, b int) int {
	if a < b {
		return a
	}
	return b
}

func sortedReportKeys(items map[string]struct{}) []string {
	out := make([]string, 0, len(items))
	for item := range items {
		if item != "" {
			out = append(out, item)
		}
	}
	sort.Strings(out)
	return out
}

func sortedReportIntKeys(items map[string]int) []string {
	out := make([]string, 0, len(items))
	for item, count := range items {
		if item != "" && count > 0 {
			out = append(out, item)
		}
	}
	sort.Strings(out)
	return out
}

func copyReportIntMap(items map[string]int) map[string]int {
	out := make(map[string]int, len(items))
	for item, count := range items {
		if item != "" && count > 0 {
			out[item] = count
		}
	}
	return out
}

func highestAuthorityForMetrics(m Metrics) string {
	highest := m.HighestAuthority
	for authority, count := range m.ToolAuthority {
		if count > 0 {
			highest = HigherToolAuthority(highest, authority)
		}
	}
	return highest
}

func highAuthorityTools(tools []string) []string {
	out := make([]string, 0)
	for _, tool := range tools {
		if IsHighAuthorityCategory(ClassifyToolAuthority(ToolCall{Name: tool})) {
			out = append(out, tool)
		}
	}
	return out
}

func reportHTMLCodeList(values []string) string {
	parts := make([]string, 0, len(values))
	for _, value := range values {
		if value != "" {
			parts = append(parts, fmt.Sprintf(`<code>%s</code>`, html.EscapeString(value)))
		}
	}
	return strings.Join(parts, ", ")
}

func reportMarkdownCodeList(values []string) string {
	parts := make([]string, 0, len(values))
	for _, value := range values {
		if value != "" {
			parts = append(parts, "`"+markdownInlineCode(value)+"`")
		}
	}
	return strings.Join(parts, ", ")
}

func markdownInlineCode(value string) string {
	value = strings.ReplaceAll(value, "`", "'")
	value = strings.ReplaceAll(value, "|", "\\|")
	value = strings.ReplaceAll(value, "\n", "<br>")
	return value
}

func markdownCell(value string) string {
	value = strings.ReplaceAll(value, "|", "\\|")
	value = strings.ReplaceAll(value, "\n", "<br>")
	return value
}

func overviewIncidentTimelines(sessions []Session, limit int) []IncidentTimelineSummary {
	if limit <= 0 {
		return []IncidentTimelineSummary{}
	}
	items := make([]IncidentTimelineSummary, 0, minReportInt(len(sessions), limit))
	for _, s := range sessions {
		timeline := BuildIncidentTimeline(s)
		if len(timeline.Items) == 0 {
			continue
		}
		items = append(items, timeline)
		if len(items) >= limit {
			break
		}
	}
	if items == nil {
		return []IncidentTimelineSummary{}
	}
	return items
}

// LoopCostSection generates the loop cost breakdown section for text reports.
func LoopCostSection(lc LoopCost) string {
	var b strings.Builder
	w := func(s string) { b.WriteString(s + "\n") }
	wf := func(f string, args ...interface{}) { b.WriteString(fmt.Sprintf(f, args...) + "\n") }
	sub := strings.Repeat(i18n.T("separator_single"), 40)

	w("🔄 " + i18n.T("loop_section_title"))
	w(sub)
	wf("  "+i18n.T("loop_tool_loop_cost"), lc.ToolLoopCost, lc.LoopGroups)
	wf("  "+i18n.T("loop_retry_cost"), lc.RetryCost, lc.RetryEvents)
	wf("  "+i18n.T("loop_format_retry_cost"), lc.FormatRetryCost)
	w("  ─────────────────────────────")
	wf("  "+i18n.T("loop_total_waste"), lc.TotalLoopCost)
	w("")
	return b.String()
}

// ReportOverview generates the CLI overview dashboard text.
func ReportOverview(ov Overview, sessions []Session) string {
	orderedSessions := canonicalOverviewSessions(sessions)
	authority := overviewAuthoritySummary(orderedSessions)
	sep := strings.Repeat(i18n.T("separator_double"), 70)
	var b strings.Builder
	w := func(s string) { b.WriteString(s + "\n") }
	wf := func(f string, args ...interface{}) { b.WriteString(fmt.Sprintf(f, args...) + "\n") }

	w(sep)
	w(fmt.Sprintf("  AGENTTRACE v%s — "+i18n.T("overview_title")+"  (%d "+i18n.T("report_sessions")+")", Version, ov.TotalSessions))
	w(sep)
	w("")

	// Stats summary
	healthyPct, warnPct, critPct := 0, 0, 0
	if ov.TotalSessions > 0 {
		healthyPct = ov.Healthy * 100 / ov.TotalSessions
		warnPct = ov.Warning * 100 / ov.TotalSessions
		critPct = ov.Critical * 100 / ov.TotalSessions
	}
	wf("  "+i18n.T("overview_total")+":     %d", ov.TotalSessions)
	wf("  🟢 "+i18n.T("overview_healthy")+":   %d (%d%%)", ov.Healthy, healthyPct)
	wf("  🟡 "+i18n.T("overview_warning")+":   %d (%d%%)", ov.Warning, warnPct)
	wf("  🔴 "+i18n.T("overview_critical")+":   %d (%d%%)", ov.Critical, critPct)
	wf("  💰 "+i18n.T("total_cost")+":      $%.2f", ov.TotalCost)
	w("")

	timelines := overviewIncidentTimelines(orderedSessions, 3)
	if len(timelines) > 0 {
		w("  ── " + i18n.T("incident_timeline_title") + " ──")
		rendered := 0
		for _, timeline := range timelines {
			for _, item := range timeline.Items {
				wf("    %-30s %s: %s", textCell(timeline.Session, 30), item.Label, textCell(item.Detail, textIncidentDetailLimit(item.Label)))
				rendered++
				if rendered >= 5 {
					break
				}
			}
			if rendered >= 5 {
				break
			}
		}
		w("")
	}

	if authority.HasData {
		w("  ── " + i18n.T("report_tool_authority") + " ──")
		if authority.Highest != "" {
			wf("    %s: %s", i18n.T("report_highest_authority"), authority.Highest)
		}
		if len(authority.Counts) > 0 {
			for _, line := range textWrappedKeyValues(i18n.T("report_authority_category_counts"), textAuthorityCountValues(authority.Counts), 96) {
				wf("    %s", line)
			}
		}
		if len(authority.HighTools) > 0 {
			for _, line := range textWrappedKeyValues(i18n.T("report_high_authority_tools"), textToolValues(authority.HighTools), 96) {
				wf("    %s", line)
			}
		}
		w("")
	}

	if notes := overviewCostDriverNotes(orderedSessions, 3); len(notes) > 0 {
		w("  ── " + i18n.T("report_possible_cost_drivers") + " ──")
		for _, note := range notes {
			wf("    %-30s %s", textCell(note.Session, 30), textCell(note.Note, 80))
		}
		w("")
	}

	// By agent
	w("  ── " + i18n.T("overview_agents") + " ──")
	type akv struct {
		k string
		v AgentOverview
	}
	var agents []akv
	for k, v := range ov.ByAgent {
		agents = append(agents, akv{k, v})
	}
	sort.Slice(agents, func(i, j int) bool { return agents[i].v.Sessions > agents[j].v.Sessions })
	for _, a := range agents {
		display := a.k
		if d, ok := ToolDisplayNames[a.k]; ok {
			display = d
		}
		wf("    %-30s %4d %s  $%7.2f", display, a.v.Sessions, i18n.T("report_sessions"), a.v.Cost)
	}
	w("")

	// By model
	w("  ── " + i18n.T("overview_models") + " ──")
	type mkv struct {
		k string
		v ModelOverview
	}
	var models []mkv
	for k, v := range ov.ByModel {
		models = append(models, mkv{k, v})
	}
	sort.Slice(models, func(i, j int) bool { return models[i].v.Cost > models[j].v.Cost })
	for i, mdl := range models {
		if i >= 8 {
			break
		}
		wf("    %-25s %4d %s  $%7.2f", mdl.k, mdl.v.Sessions, i18n.T("report_sessions"), mdl.v.Cost)
	}
	w("")

	// Anomalies
	w("  ── " + i18n.T("overview_recent_anomalies") + " ──")
	if len(ov.AnomaliesTop) == 0 {
		w("    " + i18n.T("overview_no_anomalies"))
	} else {
		limit := len(ov.AnomaliesTop)
		if limit > 8 {
			limit = 8
		}
		for i := 0; i < limit; i++ {
			a := ov.AnomaliesTop[i]
			wf("    ⚠️  %-30s %s", textCell(a.Session, 30), reportAnomalyTypeLabel(a.Type))
		}
	}
	w("")
	w(sep)
	return b.String()
}

func textAuthorityCountValues(items []authorityCount) []string {
	parts := make([]string, 0, len(items))
	for _, item := range items {
		parts = append(parts, fmt.Sprintf("%s=%d", item.Category, item.Count))
	}
	return parts
}

func textToolValues(items []string) []string {
	parts := make([]string, 0, len(items))
	for _, item := range items {
		parts = append(parts, textCell(item, 40))
	}
	return parts
}

func textIncidentDetailLimit(label string) int {
	const lineLimit = 96
	const indentWidth = 4
	const sessionWidth = 30
	const separatorsWidth = 3

	limit := lineLimit - indentWidth - sessionWidth - separatorsWidth - utf8.RuneCountInString(label)
	if limit < 24 {
		return 24
	}
	return limit
}

func textWrappedKeyValues(label string, values []string, limit int) []string {
	if len(values) == 0 {
		return []string{label + ":"}
	}
	prefix := label + ": "
	continuation := strings.Repeat(" ", utf8.RuneCountInString(label)+2)
	lines := make([]string, 0, 1)
	current := prefix
	for _, value := range values {
		separator := ""
		if current != prefix && current != continuation {
			separator = ", "
		}
		next := separator + value
		if utf8.RuneCountInString(current)+utf8.RuneCountInString(next) > limit && current != prefix && current != continuation {
			lines = append(lines, current)
			current = continuation + value
			continue
		}
		if utf8.RuneCountInString(current)+utf8.RuneCountInString(next) > limit {
			valueLimit := limit - utf8.RuneCountInString(current)
			if valueLimit < 4 {
				valueLimit = 4
			}
			next = textCell(value, valueLimit)
		}
		current += next
	}
	lines = append(lines, current)
	return lines
}

func textCell(value string, limit int) string {
	value = strings.Join(strings.Fields(value), " ")
	if limit > 3 {
		return truncateTextRunes(value, limit, "...")
	}
	return value
}

func truncateTextRunes(value string, limit int, suffix string) string {
	if limit <= 0 {
		return ""
	}
	if utf8.RuneCountInString(value) <= limit {
		return value
	}
	cut := limit
	if suffixRunes := utf8.RuneCountInString(suffix); suffix != "" && suffixRunes < limit {
		cut = limit - suffixRunes
	} else {
		suffix = ""
	}
	runes := []rune(value)
	return string(runes[:cut]) + suffix
}
