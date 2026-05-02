package engine

import (
	"encoding/json"
	"fmt"
	"html"
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
			wf("  %s [%s] %s: %s", a.Emoji, strings.ToUpper(a.Severity), a.Type, a.Detail)
		}
	} else {
		w("  " + i18n.T("no_anomalies"))
	}
	w("")

	// Loop Cost
	if m.LoopGroups > 0 {
		w("🔄 LOOP COST")
		w(sub)
		wf("  Tool Loop Cost:    $%9.4f  (%d groups)", m.LoopCostEst, m.LoopGroups)
		wf("  Retry Events:       %d", m.LoopRetryEvents)
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
	type groupItem struct {
		Name     string  `json:"name"`
		Sessions int     `json:"sessions"`
		Cost     float64 `json:"cost"`
	}
	type recentSession struct {
		Name       string  `json:"name"`
		SourceTool string  `json:"source_tool"`
		Model      string  `json:"model"`
		Turns      int     `json:"turns"`
		Tools      int     `json:"tools"`
		Tokens     int     `json:"tokens"`
		Cost       float64 `json:"cost"`
		Health     int     `json:"health"`
		Anomalies  int     `json:"anomalies"`
	}

	totalTokens := 0
	totalTools := 0
	failedTools := 0
	totalHealth := 0
	for _, s := range sessions {
		totalTokens += s.Metrics.TokensInput + s.Metrics.TokensOutput + s.Metrics.TokensCacheW + s.Metrics.TokensCacheR
		totalTools += s.Metrics.ToolCallsOK + s.Metrics.ToolCallsFail
		failedTools += s.Metrics.ToolCallsFail
		totalHealth += s.Health
	}
	avgHealth := 0.0
	if len(sessions) > 0 {
		avgHealth = round4(float64(totalHealth) / float64(len(sessions)))
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
			return models[i].Sessions > models[j].Sessions
		}
		return models[i].Cost > models[j].Cost
	})

	recentCap := len(sessions)
	if recentCap > 10 {
		recentCap = 10
	}
	recent := make([]recentSession, 0, recentCap)
	for i, s := range sessions {
		if i >= 10 {
			break
		}
		recent = append(recent, recentSession{
			Name:       s.Name,
			SourceTool: s.Metrics.SourceTool,
			Model:      s.Metrics.ModelUsed,
			Turns:      s.Metrics.AssistantTurns,
			Tools:      s.Metrics.ToolCallsOK + s.Metrics.ToolCallsFail,
			Tokens:     s.Metrics.TokensInput + s.Metrics.TokensOutput + s.Metrics.TokensCacheW + s.Metrics.TokensCacheR,
			Cost:       round4(s.Metrics.CostEstimated),
			Health:     s.Health,
			Anomalies:  len(s.Anomalies),
		})
	}
	anomalies := ov.AnomaliesTop
	if anomalies == nil {
		anomalies = []AnomalyTop{}
	}

	payload := map[string]interface{}{
		"version": Version,
		"summary": map[string]interface{}{
			"total_sessions": ov.TotalSessions,
			"healthy":        ov.Healthy,
			"warning":        ov.Warning,
			"critical":       ov.Critical,
			"avg_health":     avgHealth,
			"total_cost":     round4(ov.TotalCost),
			"total_tokens":   totalTokens,
			"tool_calls":     totalTools,
			"tool_failures":  failedTools,
			"tool_fail_rate": toolFailRate,
		},
		"by_agent":        agents,
		"by_model":        models,
		"recent_sessions": recent,
		"anomalies":       anomalies,
	}
	out, _ := json.MarshalIndent(payload, "", "  ")
	return string(out)
}

// ReportOverviewMarkdown generates a human-readable Markdown overview for PR comments and CI artifacts.
func ReportOverviewMarkdown(ov Overview, sessions []Session) string {
	summary := overviewReportSummary(sessions)

	var b strings.Builder
	fmt.Fprintf(&b, "# agenttrace overview\n\n")
	fmt.Fprintf(&b, "| Metric | Value |\n|---|---:|\n")
	fmt.Fprintf(&b, "| Sessions | %d |\n", ov.TotalSessions)
	fmt.Fprintf(&b, "| Healthy / Warning / Critical | %d / %d / %d |\n", ov.Healthy, ov.Warning, ov.Critical)
	fmt.Fprintf(&b, "| Average health | %.1f |\n", summary.AvgHealth)
	fmt.Fprintf(&b, "| Total cost | $%.2f |\n", ov.TotalCost)
	fmt.Fprintf(&b, "| Total tokens | %d |\n", summary.TotalTokens)
	fmt.Fprintf(&b, "| Tool failures | %d / %d (%.1f%%) |\n\n", summary.FailedTools, summary.TotalTools, summary.ToolFailRate)

	fmt.Fprintf(&b, "## By agent\n\n")
	fmt.Fprintf(&b, "| Agent | Sessions | Cost |\n|---|---:|---:|\n")
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

	fmt.Fprintf(&b, "\n## Recent sessions\n\n")
	fmt.Fprintf(&b, "| Session | Source | Model | Health | Cost | Anomalies |\n|---|---|---|---:|---:|---:|\n")
	limit := len(sessions)
	if limit > 10 {
		limit = 10
	}
	for i := 0; i < limit; i++ {
		s := sessions[i]
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

	fmt.Fprintf(&b, "\n## Recent anomalies\n\n")
	if len(ov.AnomaliesTop) == 0 {
		fmt.Fprintf(&b, "No anomalies detected.\n")
		return b.String()
	}
	fmt.Fprintf(&b, "| Session | Type | Age |\n|---|---|---|\n")
	anomLimit := len(ov.AnomaliesTop)
	if anomLimit > 10 {
		anomLimit = 10
	}
	for i := 0; i < anomLimit; i++ {
		a := ov.AnomaliesTop[i]
		fmt.Fprintf(&b, "| %s | %s | %s |\n", markdownCell(a.Session), markdownCell(a.Type), markdownCell(a.Age))
	}
	return b.String()
}

// ReportOverviewHTML generates a self-contained HTML report for CI artifacts and sharing.
func ReportOverviewHTML(ov Overview, sessions []Session) string {
	summary := overviewReportSummary(sessions)
	agents := sortedAgents(ov.ByAgent)
	models := sortedModels(ov.ByModel)

	var b strings.Builder
	w := func(s string) { b.WriteString(s + "\n") }
	w(`<!doctype html>`)
	w(`<html lang="en">`)
	w(`<head>`)
	w(`<meta charset="utf-8">`)
	w(`<meta name="viewport" content="width=device-width, initial-scale=1">`)
	w(`<title>agenttrace overview</title>`)
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
	w(`<div><div class="brand">agenttrace</div><h1>AI agent session overview</h1><p>Static report generated from local coding-agent traces.</p></div>`)
	w(fmt.Sprintf(`<div class="meta">v%s<br>%d sessions<br><code>agenttrace --overview -f html</code></div>`, html.EscapeString(Version), ov.TotalSessions))
	w(`</header>`)
	w(`<div class="grid" aria-label="summary metrics">`)
	w(fmt.Sprintf(`<div class="metric"><span>Sessions</span><strong>%d</strong><p>%d healthy / %d warning / %d critical</p></div>`, ov.TotalSessions, ov.Healthy, ov.Warning, ov.Critical))
	w(fmt.Sprintf(`<div class="metric"><span>Average health</span><strong>%s</strong><p>Fleet quality score</p></div>`, html.EscapeString(fmt.Sprintf("%.1f", summary.AvgHealth))))
	w(fmt.Sprintf(`<div class="metric"><span>Total cost</span><strong>$%.2f</strong><p>Estimated session cost</p></div>`, ov.TotalCost))
	w(fmt.Sprintf(`<div class="metric %s"><span>Tool failures</span><strong>%d/%d</strong><p>%.1f%% failure rate</p></div>`, html.EscapeString(failureClass(summary.ToolFailRate)), summary.FailedTools, summary.TotalTools, summary.ToolFailRate))
	w(`</div>`)

	w(`<section><h2>Recent sessions</h2><table><thead><tr><th>Session</th><th>Source</th><th>Model</th><th class="num">Tokens</th><th class="num">Cost</th><th class="num">Health</th><th class="num">Anomalies</th></tr></thead><tbody>`)
	limit := minReportInt(len(sessions), 20)
	for i := 0; i < limit; i++ {
		s := sessions[i]
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

	w(`<section><h2>By agent</h2><table><thead><tr><th>Agent</th><th class="num">Sessions</th><th class="num">Cost</th></tr></thead><tbody>`)
	for _, a := range agents {
		display := a.k
		if d, ok := ToolDisplayNames[a.k]; ok {
			display = d
		}
		w(fmt.Sprintf(`<tr><td>%s</td><td class="num">%d</td><td class="num">$%.2f</td></tr>`, html.EscapeString(display), a.v.Sessions, a.v.Cost))
	}
	w(`</tbody></table></section>`)

	w(`<section><h2>By model</h2><table><thead><tr><th>Model</th><th class="num">Sessions</th><th class="num">Cost</th></tr></thead><tbody>`)
	modelLimit := minReportInt(len(models), 12)
	for i := 0; i < modelLimit; i++ {
		mdl := models[i]
		w(fmt.Sprintf(`<tr><td>%s</td><td class="num">%d</td><td class="num">$%.2f</td></tr>`, html.EscapeString(mdl.k), mdl.v.Sessions, mdl.v.Cost))
	}
	w(`</tbody></table></section>`)

	w(`<section><h2>Recent anomalies</h2>`)
	if len(ov.AnomaliesTop) == 0 {
		w(`<p>No anomalies detected.</p>`)
	} else {
		w(`<table><thead><tr><th>Session</th><th>Type</th><th>Age</th></tr></thead><tbody>`)
		anomLimit := minReportInt(len(ov.AnomaliesTop), 20)
		for i := 0; i < anomLimit; i++ {
			a := ov.AnomaliesTop[i]
			w(fmt.Sprintf(`<tr><td>%s</td><td>%s</td><td>%s</td></tr>`, html.EscapeString(a.Session), html.EscapeString(a.Type), html.EscapeString(a.Age)))
		}
		w(`</tbody></table>`)
	}
	w(`</section>`)
	w(`</main>`)
	w(`</body>`)
	w(`</html>`)
	return b.String()
}

type overviewSummary struct {
	TotalTokens  int
	TotalTools   int
	FailedTools  int
	AvgHealth    float64
	ToolFailRate float64
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

func markdownCell(value string) string {
	value = strings.ReplaceAll(value, "|", "\\|")
	value = strings.ReplaceAll(value, "\n", "<br>")
	return value
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
	sep := strings.Repeat(i18n.T("separator_double"), 70)
	var b strings.Builder
	w := func(s string) { b.WriteString(s + "\n") }
	wf := func(f string, args ...interface{}) { b.WriteString(fmt.Sprintf(f, args...) + "\n") }

	w(sep)
	w(fmt.Sprintf("  AGENTTRACE v%s — "+i18n.T("overview_title")+"  (%d "+i18n.T("sessions_label")+")", Version, ov.TotalSessions))
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
		wf("    %-30s %4d sessions  $%7.2f", display, a.v.Sessions, a.v.Cost)
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
		wf("    %-25s %4d sessions  $%7.2f", mdl.k, mdl.v.Sessions, mdl.v.Cost)
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
			name := a.Session
			if len(name) > 30 {
				name = name[:30]
			}
			wf("    ⚠️  %-30s %s", name, a.Type)
		}
	}
	w("")
	w(sep)
	return b.String()
}
