package engine

import (
	"encoding/json"
	"strings"
	"testing"

	"github.com/luoyuctl/agenttrace/internal/i18n"
)

func TestSearchSessionsMatchesMetadataOnly(t *testing.T) {
	prev := i18n.Current
	i18n.SetLang(i18n.EN)
	t.Cleanup(func() { i18n.SetLang(prev) })

	sessions := []Session{
		{
			Name: "billing-export",
			Path: "/tmp/billing.jsonl",
			CWD:  "/repo/billing",
			Metrics: Metrics{
				SourceTool:       "codex_cli",
				ModelUsed:        "gpt-5.1",
				TokensInput:      100,
				TokensOutput:     20,
				ToolCallsOK:      1,
				ToolUsage:        map[string]int{"go_test": 1},
				ToolArgUsage:     map[string]int{"go test ./internal/billing": 1},
				FileUsage:        map[string]int{"internal/billing/export.go": 1},
				ToolAuthority:    map[string]int{ToolAuthorityTestOrBuild: 1},
				HighestAuthority: ToolAuthorityTestOrBuild,
				CostEstimated:    0.01,
			},
			Anomalies: []Anomaly{{Type: "tool_failures", Detail: "go_test failed once"}},
		},
		{
			Name: "hidden-content",
			Metrics: Metrics{
				SourceTool: "claude_code",
				ModelUsed:  "claude-sonnet-4",
				ToolUsage:  map[string]int{},
				FileUsage:  map[string]int{},
			},
			Anomalies: []Anomaly{{Type: "latency", Detail: "ordinary evidence"}},
		},
	}

	results := SearchSessions(sessions, "billing", 10)
	if len(results) != 1 || results[0].Name != "billing-export" {
		t.Fatalf("expected billing metadata result, got %+v", results)
	}
	if !containsSearchMatch(results[0].Matches, "file: internal/billing/export.go") {
		t.Fatalf("expected file evidence, got %+v", results[0].Matches)
	}
	if got := SearchSessions(sessions, "./internal/billing", 10); len(got) != 1 || !containsSearchMatch(got[0].Matches, "tool argument: go test ./internal/billing") {
		t.Fatalf("expected tool argument evidence, got %+v", got)
	}

	if got := SearchSessions(sessions, "secret prompt phrase", 10); len(got) != 0 {
		t.Fatalf("search should not inspect prompt/content text, got %+v", got)
	}
}

func TestReportSearchJSONAndText(t *testing.T) {
	prev := i18n.Current
	i18n.SetLang(i18n.EN)
	t.Cleanup(func() { i18n.SetLang(prev) })

	results := []SearchResult{{
		Name:       "billing-export",
		Path:       "/tmp/billing.jsonl",
		CWD:        "/repo/billing",
		SourceTool: "codex_cli",
		Model:      "gpt-5.1",
		Health:     88,
		Cost:       0.0123,
		Tokens:     120,
		Matches:    []string{"file: internal/billing/export.go"},
	}}

	var payload struct {
		Version string         `json:"version"`
		Count   int            `json:"count"`
		Results []SearchResult `json:"results"`
	}
	if err := json.Unmarshal([]byte(ReportSearchJSON(results)), &payload); err != nil {
		t.Fatalf("invalid search json: %v", err)
	}
	if payload.Version != Version || payload.Count != 1 || payload.Results[0].CWD != "/repo/billing" {
		t.Fatalf("bad search json payload: %+v", payload)
	}

	text := ReportSearchText(results, "billing")
	for _, want := range []string{"Search results", "billing-export", "codex_cli", "file: internal/billing/export.go"} {
		if !strings.Contains(text, want) {
			t.Fatalf("search text missing %q:\n%s", want, text)
		}
	}
}

func containsSearchMatch(items []string, want string) bool {
	for _, item := range items {
		if item == want {
			return true
		}
	}
	return false
}
