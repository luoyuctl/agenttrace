package engine

import (
	"encoding/json"
	"fmt"
	"sort"
	"strings"

	"github.com/luoyuctl/agenttrace/internal/i18n"
)

type SearchResult struct {
	Name       string   `json:"name"`
	Path       string   `json:"path"`
	CWD        string   `json:"cwd,omitempty"`
	SourceTool string   `json:"source_tool"`
	Model      string   `json:"model"`
	Health     int      `json:"health"`
	Cost       float64  `json:"cost"`
	Tokens     int      `json:"tokens"`
	Matches    []string `json:"matches"`
}

func SearchSessions(sessions []Session, query string, limit int) []SearchResult {
	query = strings.ToLower(strings.TrimSpace(query))
	if query == "" {
		return nil
	}
	if limit <= 0 {
		limit = 20
	}

	results := make([]SearchResult, 0)
	for _, s := range canonicalOverviewSessions(sessions) {
		matches := searchSessionEvidence(s, query)
		if len(matches) == 0 {
			continue
		}
		results = append(results, SearchResult{
			Name:       s.Name,
			Path:       s.Path,
			CWD:        s.CWD,
			SourceTool: s.Metrics.SourceTool,
			Model:      s.Metrics.ModelUsed,
			Health:     s.Health,
			Cost:       round4(s.Metrics.CostEstimated),
			Tokens:     sessionSearchTokens(s),
			Matches:    matches,
		})
		if len(results) >= limit {
			break
		}
	}
	return results
}

func ReportSearchJSON(results []SearchResult) string {
	if results == nil {
		results = []SearchResult{}
	}
	out, _ := json.MarshalIndent(map[string]interface{}{
		"version": Version,
		"count":   len(results),
		"results": results,
	}, "", "  ")
	return string(out)
}

func ReportSearchText(results []SearchResult, query string) string {
	var b strings.Builder
	fmt.Fprintf(&b, "%s: %q (%d)\n", i18n.T("search_results_title"), query, len(results))
	if len(results) == 0 {
		fmt.Fprintf(&b, "%s\n", i18n.T("search_no_results"))
		return b.String()
	}
	for _, r := range results {
		fmt.Fprintf(&b, "\n%s  %s  %s  %s  $%.4f  %d %s\n",
			r.Name,
			r.SourceTool,
			r.Model,
			fmt.Sprintf(i18n.T("search_health"), r.Health),
			r.Cost,
			r.Tokens,
			i18n.T("tokens"))
		if r.CWD != "" {
			fmt.Fprintf(&b, "  %s: %s\n", i18n.T("search_cwd"), r.CWD)
		}
		if r.Path != "" {
			fmt.Fprintf(&b, "  %s: %s\n", i18n.T("search_path"), r.Path)
		}
		for _, match := range r.Matches {
			fmt.Fprintf(&b, "  - %s\n", match)
		}
	}
	return b.String()
}

func searchSessionEvidence(s Session, query string) []string {
	seen := make(map[string]struct{})
	matches := make([]string, 0)
	add := func(label, value string) {
		value = strings.TrimSpace(value)
		if value == "" || !strings.Contains(strings.ToLower(value), query) {
			return
		}
		item := label + ": " + value
		if _, ok := seen[item]; ok {
			return
		}
		seen[item] = struct{}{}
		matches = append(matches, item)
	}

	add(i18n.T("search_match_name"), s.Name)
	add(i18n.T("search_match_path"), s.Path)
	add(i18n.T("search_match_cwd"), s.CWD)
	add(i18n.T("search_match_source"), s.Metrics.SourceTool)
	add(i18n.T("search_match_model"), s.Metrics.ModelUsed)
	add(i18n.T("search_match_authority"), highestAuthorityForMetrics(s.Metrics))
	add(i18n.T("search_match_cost_driver"), possibleCostDriverNote(s))

	for _, key := range sortedSearchIntMapKeys(s.Metrics.ToolUsage) {
		add(i18n.T("search_match_tool"), key)
	}
	for _, key := range sortedSearchIntMapKeys(s.Metrics.ToolArgUsage) {
		add(i18n.T("search_match_tool_arg"), key)
	}
	for _, key := range sortedSearchIntMapKeys(s.Metrics.FileUsage) {
		add(i18n.T("search_match_file"), key)
	}
	for _, a := range s.Anomalies {
		add(i18n.T("search_match_anomaly"), a.Type)
		add(i18n.T("search_match_anomaly"), reportAnomalyTypeLabel(a.Type))
		add(i18n.T("search_match_anomaly"), a.Detail)
	}
	for _, lp := range s.LargeParams {
		add(i18n.T("search_match_large_param"), lp.ToolName)
		add(i18n.T("search_match_large_param"), lp.Detail)
	}
	for _, tw := range s.ToolWarnings {
		add(i18n.T("search_match_tool_warning"), tw.ToolName)
		add(i18n.T("search_match_tool_warning"), tw.Pattern)
		add(i18n.T("search_match_tool_warning"), tw.Detail)
	}
	if len(matches) > 8 {
		matches = matches[:8]
	}
	return matches
}

func sortedSearchIntMapKeys(items map[string]int) []string {
	keys := make([]string, 0, len(items))
	for key, count := range items {
		if key != "" && count > 0 {
			keys = append(keys, key)
		}
	}
	sort.Strings(keys)
	return keys
}

func sessionSearchTokens(s Session) int {
	return s.Metrics.TokensInput + s.Metrics.TokensOutput + s.Metrics.TokensCacheW + s.Metrics.TokensCacheR
}
