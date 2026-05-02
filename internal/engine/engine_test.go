package engine

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/luoyuctl/agentwaste/internal/i18n"
)

func mustJSON(v interface{}) string {
	b, _ := json.Marshal(v)
	return string(b)
}

func makeJSONL(lines []interface{}) string {
	var parts []string
	for _, l := range lines {
		parts = append(parts, mustJSON(l))
	}
	return strings.Join(parts, "\n") + "\n"
}

func TestParseHermesJSONL(t *testing.T) {
	raw := makeJSONL([]interface{}{
		map[string]interface{}{"role": "session_meta", "session_id": "s1"},
		map[string]interface{}{"role": "user", "content": "hello", "timestamp": "2026-01-01T00:00:00Z"},
		map[string]interface{}{"role": "assistant", "content": "hi", "timestamp": "2026-01-01T00:00:01Z", "tool_calls": []interface{}{map[string]interface{}{"id": "tc1", "name": "read_file"}}},
		map[string]interface{}{"role": "tool", "content": "file content", "tool_call_id": "tc1", "timestamp": "2026-01-01T00:00:02Z"},
	})
	events, err := parseHermesJSONL(raw)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(events) != 4 {
		t.Fatalf("expected 4 events, got %d", len(events))
	}
	if events[2].ToolCalls[0].Name != "read_file" {
		t.Errorf("tool call name: got %q", events[2].ToolCalls[0].Name)
	}
	if events[3].ToolCallID != "tc1" {
		t.Errorf("ToolCallID: got %q", events[3].ToolCallID)
	}
}

func TestParseHermesJSONL_Empty(t *testing.T) {
	events, err := parseHermesJSONL("")
	if err != nil {
		t.Fatalf("empty input should not error: %v", err)
	}
	if len(events) != 0 {
		t.Errorf("expected 0 events from empty input, got %d", len(events))
	}
}

func TestParseHermesJSON(t *testing.T) {
	doc := map[string]interface{}{
		"model": "claude-sonnet-4",
		"usage": map[string]interface{}{"input_tokens": 100, "output_tokens": 200},
		"messages": []interface{}{
			map[string]interface{}{"role": "user", "content": "hello", "timestamp": "2026-01-01T00:00:00Z"},
			map[string]interface{}{"role": "assistant", "content": "", "timestamp": "2026-01-01T00:00:01Z", "tool_calls": []interface{}{map[string]interface{}{"id": "tc1", "function": map[string]interface{}{"name": "read_file"}}}},
			map[string]interface{}{"role": "tool", "content": "ok", "tool_call_id": "tc1", "timestamp": "2026-01-01T00:00:02Z", "is_error": false},
		},
	}
	events, err := parseHermesJSON(doc)
	if err != nil {
		t.Fatalf("error: %v", err)
	}
	// Bug1: no duplicate tool events
	toolCount := 0
	for _, ev := range events {
		if ev.Role == "tool" {
			toolCount++
		}
	}
	if toolCount != 1 {
		t.Errorf("Bug1: tool dupe, got %d tool events", toolCount)
	}
	// Bug2: usage meta
	hasMeta := false
	for _, ev := range events {
		if ev.Role == "meta" && ev.Usage != nil {
			hasMeta = true
			if ev.Usage["input_tokens"] != 100 {
				t.Errorf("Bug2: wrong input tokens: %v", ev.Usage)
			}
		}
	}
	if !hasMeta {
		t.Error("Bug2: missing meta")
	}
	// Bug3: timestamps
	for _, ev := range events {
		if ev.Role != "meta" && ev.Timestamp == "" {
			t.Errorf("Bug3: role=%q missing ts", ev.Role)
		}
	}
}

func TestParseClaudeCode_ToolResultArray(t *testing.T) {
	doc := map[string]interface{}{
		"model": "claude",
		"messages": []interface{}{
			map[string]interface{}{"role": "user", "content": []interface{}{map[string]interface{}{"type": "text", "text": "do"}}},
			map[string]interface{}{"role": "assistant", "content": []interface{}{map[string]interface{}{"type": "tool_use", "id": "tu1", "name": "bash"}}},
			map[string]interface{}{"role": "user", "content": []interface{}{map[string]interface{}{"type": "tool_result", "tool_use_id": "tu1", "is_error": false, "content": []interface{}{map[string]interface{}{"type": "text", "text": "success"}}}}},
		},
	}
	events, _ := parseClaudeCode(doc, nil)
	found := false
	for _, ev := range events {
		if ev.Role == "tool" && ev.ToolCallID == "tu1" && ev.Content == "success" {
			found = true
		}
	}
	if !found {
		t.Error("Bug6: tool_result array content not extracted")
	}
}

func TestParseGeneric_Invalid(t *testing.T) {
	_, err := parseGeneric("not json at all")
	if err == nil {
		t.Error("Bug7: expected error for invalid input")
	}
}

func TestDetectFormat_GeminiAndOpenClaw(t *testing.T) {
	dir := t.TempDir()
	geminiPath := filepath.Join(dir, "gemini.json")
	geminiDoc := map[string]interface{}{
		"modelVersion": "gemini-2.5-pro",
		"contents": []interface{}{
			map[string]interface{}{"role": "user", "parts": []interface{}{map[string]interface{}{"text": "hi"}}},
		},
	}
	if err := os.WriteFile(geminiPath, []byte(mustJSON(geminiDoc)), 0644); err != nil {
		t.Fatal(err)
	}
	if got := DetectFormat(geminiPath).Format; got != "gemini_cli" {
		t.Fatalf("gemini format: %s", got)
	}

	openClawPath := filepath.Join(dir, "openclaw.json")
	openClawDoc := map[string]interface{}{
		"provider": "openclaw",
		"model":    "claude-sonnet-4",
		"messages": []interface{}{
			map[string]interface{}{"role": "assistant", "content": "ok"},
		},
	}
	if err := os.WriteFile(openClawPath, []byte(mustJSON(openClawDoc)), 0644); err != nil {
		t.Fatal(err)
	}
	if got := DetectFormat(openClawPath).Format; got != "openclaw" {
		t.Fatalf("openclaw format: %s", got)
	}
}

func TestDetectLargeParams_UsesToolArguments(t *testing.T) {
	arg := strings.Repeat("x", 10001)
	events := []Event{{
		Role:      "assistant",
		ToolCalls: []ToolCall{{Name: "write_file", Args: arg}},
	}}
	large := DetectLargeParams(events)
	if len(large) != 1 {
		t.Fatalf("expected one large arg call, got %d", len(large))
	}
	if large[0].ToolName != "write_file" || large[0].ParamSize != len(arg) {
		t.Fatalf("bad large param result: %+v", large[0])
	}
}

func TestFindSessionFilesAutoDiscoverySortsByModTime(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	oldDir := filepath.Join(home, ".codex", "sessions")
	newDir := filepath.Join(home, ".gemini", "sessions")
	if err := os.MkdirAll(oldDir, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(newDir, 0755); err != nil {
		t.Fatal(err)
	}
	oldPath := filepath.Join(oldDir, "z-old.jsonl")
	newPath := filepath.Join(newDir, "a-new.jsonl")
	if err := os.WriteFile(oldPath, []byte("{}\n"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(newPath, []byte("{}\n"), 0644); err != nil {
		t.Fatal(err)
	}
	oldTime := time.Now().Add(-time.Hour)
	newTime := time.Now()
	if err := os.Chtimes(oldPath, oldTime, oldTime); err != nil {
		t.Fatal(err)
	}
	if err := os.Chtimes(newPath, newTime, newTime); err != nil {
		t.Fatal(err)
	}
	files := FindSessionFiles("")
	if len(files) != 2 {
		t.Fatalf("files: %v", files)
	}
	if files[0] != newPath {
		t.Fatalf("expected newest first, got %v", files)
	}
}

func TestFindSessionFilesCachedUsesDirIndexAndFindsNewFiles(t *testing.T) {
	dir := t.TempDir()
	first := filepath.Join(dir, "first.jsonl")
	if err := os.WriteFile(first, []byte("{}\n"), 0644); err != nil {
		t.Fatal(err)
	}
	firstTime := time.Now().Add(-time.Hour)
	if err := os.Chtimes(first, firstTime, firstTime); err != nil {
		t.Fatal(err)
	}

	cache := SessionCache{
		Entries: map[string]CacheEntry{},
		Dirs:    map[string]DirCacheEntry{},
	}
	files := FindSessionFilesCached(dir, cache)
	if len(files) != 1 || files[0] != first {
		t.Fatalf("first scan files: %v", files)
	}
	if _, ok := cache.Dirs[dir]; !ok {
		t.Fatalf("expected directory listing cached")
	}

	second := filepath.Join(dir, "second.jsonl")
	if err := os.WriteFile(second, []byte("{}\n"), 0644); err != nil {
		t.Fatal(err)
	}
	secondTime := time.Now()
	if err := os.Chtimes(second, secondTime, secondTime); err != nil {
		t.Fatal(err)
	}
	if err := os.Chtimes(dir, secondTime, secondTime); err != nil {
		t.Fatal(err)
	}

	files = FindSessionFilesCached(dir, cache)
	if len(files) != 2 {
		t.Fatalf("second scan files: %v", files)
	}
	if files[0] != second {
		t.Fatalf("expected newest file first, got %v", files)
	}
}

func TestLoadSessionCacheSkipsOnlyBadEntries(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	cachePath := sessionCachePath()
	if err := os.MkdirAll(filepath.Dir(cachePath), 0755); err != nil {
		t.Fatal(err)
	}
	payload := `{
		"entries": {
			"/tmp/good.jsonl": {"mod_time": 1, "size": 2, "session": {"Name": "good", "Health": 90}},
			"/tmp/bad.jsonl": {"mod_time": "bad"}
		},
		"dirs": {
			"/tmp": {"mod_time": 3, "files": ["/tmp/good.jsonl"], "dirs": []},
			"/bad": {"mod_time": "bad"}
		}
	}`
	if err := os.WriteFile(cachePath, []byte(payload), 0644); err != nil {
		t.Fatal(err)
	}

	cache := LoadSessionCache()
	if len(cache.Entries) != 1 || cache.Entries["/tmp/good.jsonl"].Session.Name != "good" {
		t.Fatalf("expected only good entry loaded: %+v", cache.Entries)
	}
	if len(cache.Dirs) != 1 || len(cache.Dirs["/tmp"].Files) != 1 {
		t.Fatalf("expected only good dir loaded: %+v", cache.Dirs)
	}
}

func TestAnalyze(t *testing.T) {
	events := []Event{
		{Role: "user", Content: "hello", SourceTool: "h", Timestamp: "2026-01-01T00:00:00Z"},
		{Role: "assistant", Content: "hi", SourceTool: "h", Timestamp: "2026-01-01T00:00:01Z"},
		{Role: "assistant", ToolCalls: []ToolCall{{Name: "read_file", ID: "t1"}}, SourceTool: "h", Timestamp: "2026-01-01T00:00:02Z"},
		{Role: "tool", Content: "ok", ToolCallID: "t1", SourceTool: "h", Timestamp: "2026-01-01T00:00:03Z"},
	}
	m := Analyze(events, "default")
	if m.UserMessages != 1 {
		t.Errorf("UserMessages: %d", m.UserMessages)
	}
	if m.AssistantTurns != 2 {
		t.Errorf("AssistantTurns: %d", m.AssistantTurns)
	}
	if m.ToolCallsTotal != 1 {
		t.Errorf("ToolCallsTotal: %d", m.ToolCallsTotal)
	}
	if m.ToolCallsOK != 1 {
		t.Errorf("ToolCallsOK: %d", m.ToolCallsOK)
	}
	if m.CostEstimated < 0 {
		t.Error("cost should be >=0")
	}
}

func TestAnalyze_ToolFailures(t *testing.T) {
	events := []Event{
		{Role: "user", Content: "x", SourceTool: "c"},
		{Role: "assistant", ToolCalls: []ToolCall{{Name: "bad", ID: "t1"}}, SourceTool: "c"},
		{Role: "tool", Content: `{"error":"fail"}`, ToolCallID: "t1", IsError: true, SourceTool: "c"},
	}
	m := Analyze(events, "default")
	if m.ToolCallsOK != 0 {
		t.Errorf("ToolCallsOK: %d", m.ToolCallsOK)
	}
	if m.ToolCallsFail != 1 {
		t.Errorf("ToolCallsFail: %d", m.ToolCallsFail)
	}
}

func TestDetectAnomalies_Perfect(t *testing.T) {
	m := Metrics{UserMessages: 5, AssistantTurns: 5, ToolCallsTotal: 20, ToolCallsOK: 20, ReasoningBlocks: 5, ReasoningChars: 4000}
	anoms := DetectAnomalies(m)
	if len(anoms) != 0 {
		t.Errorf("expected 0, got %d", len(anoms))
	}
}

func TestDetectAnomalies_Hanging(t *testing.T) {
	m := Metrics{UserMessages: 5, AssistantTurns: 5, ToolCallsTotal: 10, ToolCallsOK: 10, ReasoningBlocks: 5, ReasoningChars: 4000, GapsSec: []float64{10, 15, 350}}
	anoms := DetectAnomalies(m)
	for _, a := range anoms {
		if a.Type == "hanging" {
			return
		}
	}
	t.Error("no hanging anomaly for gap>300s")
}

func TestDetectAnomalies_ToolFail(t *testing.T) {
	m := Metrics{UserMessages: 5, AssistantTurns: 5, ToolCallsTotal: 100, ToolCallsOK: 60, ToolCallsFail: 40, ReasoningBlocks: 5, ReasoningChars: 4000}
	anoms := DetectAnomalies(m)
	for _, a := range anoms {
		if a.Type == "tool_failures" {
			return
		}
	}
	t.Error("no tool_failures anomaly for 40% fail")
}

func TestDetectAnomalies_Shallow(t *testing.T) {
	m := Metrics{UserMessages: 5, AssistantTurns: 5, ToolCallsTotal: 10, ToolCallsOK: 10, ReasoningBlocks: 3, ReasoningChars: 300, ReasoningLens: []int{100, 100, 100}}
	anoms := DetectAnomalies(m)
	for _, a := range anoms {
		if a.Type == "shallow_thinking" {
			return
		}
	}
	t.Error("no shallow_thinking anomaly")
}

func TestHealthScore_Perfect(t *testing.T) {
	s := HealthScore(Metrics{}, nil)
	if s != 100 {
		t.Errorf("expected 100, got %d", s)
	}
}

func TestHealthScore_WithAnomalies(t *testing.T) {
	anoms := []Anomaly{{Severity: SeverityHigh}, {Severity: SeverityMedium}}
	s := HealthScore(Metrics{}, anoms)
	if s != 58 {
		t.Errorf("expected 58, got %d", s)
	}
}

func TestHealthScore_Floor(t *testing.T) {
	anoms := []Anomaly{{Severity: SeverityHigh}, {Severity: SeverityHigh}, {Severity: SeverityHigh}, {Severity: SeverityHigh}}
	s := HealthScore(Metrics{}, anoms)
	if s != 0 {
		t.Errorf("expected 0, got %d", s)
	}
}

func TestDiffSessions(t *testing.T) {
	a := Session{Name: "A", Health: 90, Metrics: Metrics{AssistantTurns: 10, ToolCallsTotal: 20, ToolCallsOK: 18, ToolCallsFail: 2, CostEstimated: 0.05, ModelUsed: "opus", DurationSec: 60}}
	b := Session{Name: "B", Health: 72, Metrics: Metrics{AssistantTurns: 24, ToolCallsTotal: 40, ToolCallsOK: 32, ToolCallsFail: 8, CostEstimated: 0.12, ModelUsed: "haiku", DurationSec: 180}, Anomalies: []Anomaly{{Type: "tool_failures"}}}
	diff := DiffSessions(a, b)
	if diff.SessionA != "A" || diff.SessionB != "B" {
		t.Error("wrong session names")
	}
	if len(diff.Entries) < 7 {
		t.Errorf("entries: %d", len(diff.Entries))
	}
}

func TestGenerateFixes_Hanging(t *testing.T) {
	m := Metrics{GapsSec: []float64{350}}
	anoms := []Anomaly{{Type: "hanging", Severity: SeverityHigh}}
	i18n.Current = i18n.ZH
	fixes := GenerateFixes(m, anoms)
	i18n.Current = i18n.EN
	if len(fixes) == 0 {
		t.Fatal("no fixes")
	}
	if fixes[0].Category != "hanging" {
		t.Errorf("category: %q", fixes[0].Category)
	}
}

func TestGenerateFixes_English(t *testing.T) {
	m := Metrics{GapsSec: []float64{350}}
	anoms := []Anomaly{{Type: "hanging", Severity: SeverityHigh}}
	fixes := GenerateFixes(m, anoms)
	if len(fixes) == 0 {
		t.Fatal("no fixes")
	}
	if fixes[0].Category != "hanging" {
		t.Errorf("category: %q", fixes[0].Category)
	}
}

func TestAnalyzeHealthTrend_Stable(t *testing.T) {
	sessions := []Session{{Name: "s1", Health: 85}, {Name: "s2", Health: 83}, {Name: "s3", Health: 86}, {Name: "s4", Health: 84}}
	if AnalyzeHealthTrend(sessions).Regressing {
		t.Error("should not regress")
	}
}

func TestAnalyzeHealthTrend_Declining(t *testing.T) {
	sessions := []Session{{Name: "s1", Health: 90}, {Name: "s2", Health: 70}, {Name: "s3", Health: 50}}
	if !AnalyzeHealthTrend(sessions).Regressing {
		t.Error("should regress")
	}
}

func TestValidateToolPatterns_DeadLoop(t *testing.T) {
	events := make([]Event, 6)
	for i := range events {
		events[i] = Event{Role: "assistant", ToolCalls: []ToolCall{{Name: "read_file"}}}
	}
	warnings := ValidateToolPatterns(events)
	for _, w := range warnings {
		if w.Pattern == "dead_loop" {
			return
		}
	}
	t.Error("no dead_loop warning for 6 consecutive same tool calls")
}

func TestValidateToolPatterns_NoIssues(t *testing.T) {
	events := []Event{{Role: "user", Content: "hi"}, {Role: "assistant", Content: "hey"}}
	if len(ValidateToolPatterns(events)) != 0 {
		t.Error("expected 0 warnings")
	}
}

func TestPredictCostAnomaly_Normal(t *testing.T) {
	sessions := []Session{
		{Metrics: Metrics{AssistantTurns: 10, CostEstimated: 0.05}},
		{Metrics: Metrics{AssistantTurns: 10, CostEstimated: 0.06}},
	}
	current := Session{Metrics: Metrics{AssistantTurns: 10, CostEstimated: 0.07}}
	if PredictCostAnomaly(sessions, current).Triggered {
		t.Error("should not trigger")
	}
}

func TestPredictCostAnomaly_Critical(t *testing.T) {
	sessions := []Session{
		{Metrics: Metrics{AssistantTurns: 10, CostEstimated: 0.05}},
	}
	current := Session{Metrics: Metrics{AssistantTurns: 10, CostEstimated: 0.25}}
	alert := PredictCostAnomaly(sessions, current)
	if !alert.Triggered || alert.Level != "critical" {
		t.Errorf("level: %s, triggered: %v", alert.Level, alert.Triggered)
	}
}
