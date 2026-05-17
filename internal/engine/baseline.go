package engine

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
)

type BaselineThresholds struct {
	MaxDurationDeltaPct float64 `json:"max_duration_delta_pct"`
	MaxCostDeltaPct     float64 `json:"max_cost_delta_pct"`
	MaxTokenDeltaPct    float64 `json:"max_token_delta_pct"`
}

type BaselineComparison struct {
	BaselinePath               string             `json:"baseline_path"`
	Thresholds                 BaselineThresholds `json:"thresholds"`
	Current                    baselineSnapshot   `json:"current"`
	Baseline                   baselineSnapshot   `json:"baseline"`
	DurationDeltaPct           float64            `json:"duration_delta_pct"`
	CostDeltaPct               float64            `json:"cost_delta_pct"`
	TokenDeltaPct              float64            `json:"token_delta_pct"`
	SlowerThanBaseline         bool               `json:"slower_than_baseline"`
	CostAboveThreshold         bool               `json:"cost_above_threshold"`
	TokensAboveThreshold       bool               `json:"tokens_above_threshold"`
	NewFailureFamilies         []string           `json:"new_failure_families"`
	BroaderToolSurface         bool               `json:"broader_tool_surface"`
	NewTools                   []string           `json:"new_tools"`
	BroaderFileSurface         bool               `json:"broader_file_surface"`
	NewFiles                   []string           `json:"new_files"`
	NewToolAuthorityCategories []string           `json:"new_tool_authority_categories"`
	NewHighAuthorityToolUse    []string           `json:"new_high_authority_tool_use"`
}

type baselineSnapshot struct {
	DurationSeconds     float64  `json:"duration_seconds"`
	Cost                float64  `json:"cost"`
	Tokens              int      `json:"tokens"`
	FailureFamilies     []string `json:"failure_families"`
	Tools               []string `json:"tools"`
	Files               []string `json:"files"`
	AuthorityCategories []string `json:"authority_categories"`
	HighAuthorityTools  []string `json:"high_authority_tools"`
}

type overviewBaselineReport struct {
	Version string `json:"version"`
	Summary struct {
		TotalDurationSeconds float64 `json:"total_duration_seconds"`
		TotalCost            float64 `json:"total_cost"`
		TotalTokens          int     `json:"total_tokens"`
	} `json:"summary"`
	FailureFamilies []string `json:"failure_families"`
	Surfaces        struct {
		Tools               []string `json:"tools"`
		Files               []string `json:"files"`
		AuthorityCategories []string `json:"authority_categories"`
		HighAuthorityTools  []string `json:"high_authority_tools"`
	} `json:"surfaces"`
}

func AddBaselineComparison(currentReport string, baselinePath string, thresholds BaselineThresholds) (string, error) {
	if baselinePath == "" {
		return "", errors.New("missing baseline report path")
	}
	current, err := decodeOverviewBaselineReport([]byte(currentReport), "current report")
	if err != nil {
		return "", err
	}
	baselineData, err := os.ReadFile(baselinePath)
	if err != nil {
		if os.IsNotExist(err) {
			return "", fmt.Errorf("missing baseline report: %s", baselinePath)
		}
		return "", fmt.Errorf("read baseline report %s: %w", baselinePath, err)
	}
	baseline, err := decodeOverviewBaselineReport(baselineData, "baseline report")
	if err != nil {
		return "", err
	}
	if baseline.Version != Version {
		return "", fmt.Errorf("baseline report version %q is incompatible with current version %q", baseline.Version, Version)
	}

	comparison := compareOverviewBaseline(current, baseline, baselinePath, thresholds)
	var payload map[string]interface{}
	if err := json.Unmarshal([]byte(currentReport), &payload); err != nil {
		return "", fmt.Errorf("decode current report payload: %w", err)
	}
	payload["baseline_comparison"] = comparison
	out, err := json.MarshalIndent(payload, "", "  ")
	if err != nil {
		return "", fmt.Errorf("encode baseline comparison report: %w", err)
	}
	return string(out), nil
}

func decodeOverviewBaselineReport(data []byte, label string) (overviewBaselineReport, error) {
	var report overviewBaselineReport
	if err := json.Unmarshal(data, &report); err != nil {
		return report, fmt.Errorf("decode %s JSON: %w", label, err)
	}
	if report.Version == "" {
		return report, fmt.Errorf("%s is missing version", label)
	}
	if report.Summary.TotalTokens == 0 && report.Summary.TotalCost == 0 && report.Summary.TotalDurationSeconds == 0 {
		return report, fmt.Errorf("%s is missing overview summary totals", label)
	}
	return report, nil
}

func compareOverviewBaseline(current, baseline overviewBaselineReport, baselinePath string, thresholds BaselineThresholds) BaselineComparison {
	currentSnapshot := baselineSnapshotFromReport(current)
	baselineSnapshot := baselineSnapshotFromReport(baseline)
	durationDelta := deltaPct(currentSnapshot.DurationSeconds, baselineSnapshot.DurationSeconds)
	costDelta := deltaPct(currentSnapshot.Cost, baselineSnapshot.Cost)
	tokenDelta := deltaPct(float64(currentSnapshot.Tokens), float64(baselineSnapshot.Tokens))
	newTools := setDiff(currentSnapshot.Tools, baselineSnapshot.Tools)
	newFiles := setDiff(currentSnapshot.Files, baselineSnapshot.Files)
	return BaselineComparison{
		BaselinePath:               baselinePath,
		Thresholds:                 thresholds,
		Current:                    currentSnapshot,
		Baseline:                   baselineSnapshot,
		DurationDeltaPct:           durationDelta,
		CostDeltaPct:               costDelta,
		TokenDeltaPct:              tokenDelta,
		SlowerThanBaseline:         durationDelta > thresholds.MaxDurationDeltaPct,
		CostAboveThreshold:         costDelta > thresholds.MaxCostDeltaPct,
		TokensAboveThreshold:       tokenDelta > thresholds.MaxTokenDeltaPct,
		NewFailureFamilies:         setDiff(currentSnapshot.FailureFamilies, baselineSnapshot.FailureFamilies),
		BroaderToolSurface:         len(newTools) > 0,
		NewTools:                   newTools,
		BroaderFileSurface:         len(newFiles) > 0,
		NewFiles:                   newFiles,
		NewToolAuthorityCategories: setDiff(currentSnapshot.AuthorityCategories, baselineSnapshot.AuthorityCategories),
		NewHighAuthorityToolUse:    setDiff(currentSnapshot.HighAuthorityTools, baselineSnapshot.HighAuthorityTools),
	}
}

func baselineSnapshotFromReport(report overviewBaselineReport) baselineSnapshot {
	return baselineSnapshot{
		DurationSeconds:     round4(report.Summary.TotalDurationSeconds),
		Cost:                round4(report.Summary.TotalCost),
		Tokens:              report.Summary.TotalTokens,
		FailureFamilies:     sortedStringSet(report.FailureFamilies),
		Tools:               sortedStringSet(report.Surfaces.Tools),
		Files:               sortedStringSet(report.Surfaces.Files),
		AuthorityCategories: sortedStringSet(report.Surfaces.AuthorityCategories),
		HighAuthorityTools:  sortedStringSet(report.Surfaces.HighAuthorityTools),
	}
}

func deltaPct(current, baseline float64) float64 {
	if baseline == 0 {
		if current > 0 {
			return 100
		}
		return 0
	}
	return round4((current - baseline) / baseline * 100)
}

func setDiff(current, baseline []string) []string {
	seen := make(map[string]struct{}, len(baseline))
	for _, item := range baseline {
		seen[item] = struct{}{}
	}
	out := make([]string, 0)
	for _, item := range sortedStringSet(current) {
		if _, ok := seen[item]; !ok {
			out = append(out, item)
		}
	}
	return out
}

func sortedStringSet(items []string) []string {
	seen := make(map[string]struct{}, len(items))
	for _, item := range items {
		if item != "" {
			seen[item] = struct{}{}
		}
	}
	return sortedReportKeys(seen)
}
