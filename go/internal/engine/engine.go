// Package engine provides the core analysis engine for agenttrace.
// Ported from Python agenttrace v3 with identical logic.
package engine

import (
	"encoding/json"
	"fmt"
	"math"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"
)

const Version = "4.0.0" // Go rewrite

// ── Pricing (USD per 1M tokens) ──

type Price struct {
	Input  float64
	Output float64
	CW     float64 // cache write
	CR     float64 // cache read
}

var Pricing = map[string]Price{
	"claude-opus-4":   {15.00, 75.00, 18.75, 1.50},
	"claude-opus-4.5": {15.00, 75.00, 18.75, 1.50},
	"claude-sonnet-4": {3.00, 15.00, 3.75, 0.30},
	"claude-sonnet-4.5": {3.00, 15.00, 3.75, 0.30},
	"claude-haiku-3.5":  {0.80, 4.00, 1.00, 0.08},
	"claude-haiku-4":    {0.80, 4.00, 1.00, 0.08},
	"gemini-2.5-pro":    {1.25, 10.00, 0, 0},
	"gemini-2.5-flash":  {0.15, 0.60, 0, 0},
	"gpt-4.1":           {2.00, 8.00, 0, 0},
	"gpt-4.1-mini":      {0.40, 1.60, 0, 0},
	"gpt-4.1-nano":      {0.10, 0.40, 0, 0},
	"deepseek-v3":       {0.27, 1.10, 0.07, 0.014},
	"deepseek-r1":       {0.55, 2.19, 0.14, 0.028},
	"default":           {3.00, 15.00, 0, 0},
}

// ── Event (normalized) ──

type Event struct {
	Role      string `json:"role"`
	Content   string `json:"content,omitempty"`
	Timestamp string `json:"timestamp,omitempty"`
	// Claude thinking
	Reasoning string `json:"reasoning,omitempty"`
	Redacted  bool   `json:"redacted,omitempty"`
	// Tool calls
	ToolCall  *ToolCall   `json:"tool_call,omitempty"`
	ToolCalls []ToolCall  `json:"tool_calls,omitempty"`
	// Tool result
	ToolCallID string `json:"tool_call_id,omitempty"`
	IsError    bool   `json:"is_error,omitempty"`
	// Meta
	Usage     map[string]int `json:"usage,omitempty"`
	ModelUsed string         `json:"model,omitempty"`
}

type ToolCall struct {
	ID   string `json:"id"`
	Name string `json:"name"`
	// Function wrapper (OpenAI style)
	Function *struct {
		Name string `json:"name"`
	} `json:"function,omitempty"`
}

// ── Metrics ──

type Metrics struct {
	EventsTotal     int
	UserMessages    int
	AssistantTurns  int
	ToolResults     int
	ToolCallsTotal  int
	ToolCallsOK     int
	ToolCallsFail   int
	ToolUsage       map[string]int
	ReasoningBlocks int
	ReasoningChars  int
	ReasoningLens   []int
	ReasoningRedact int
	TokensInput     int
	TokensOutput    int
	TokensCacheW    int
	TokensCacheR    int
	Timestamps      []time.Time
	GapsSec         []float64
	ModelUsed       string
	SessionStart    string
	SessionEnd      string
	DurationSec     float64
	CostEstimated   float64
}

// ── Anomaly ──

type Anomaly struct {
	Type     string `json:"type"`
	Severity string `json:"severity"`
	Emoji    string `json:"emoji"`
	Detail   string `json:"detail"`
}

// ── Session ──

type Session struct {
	Name     string
	Metrics  Metrics
	Anomalies []Anomaly
	Health   int
}

// ── Format Detection ──

func DetectFormat(path string) string {
	data, err := os.ReadFile(path)
	if err != nil {
		return "unknown"
	}
	content := strings.TrimSpace(string(data))
	if content == "" {
		return "unknown"
	}

	// Try first line as JSON
	firstLine := strings.SplitN(content, "\n", 2)[0]
	var parsed interface{}
	if err := json.Unmarshal([]byte(firstLine), &parsed); err != nil {
		return "unknown"
	}

	switch v := parsed.(type) {
	case map[string]interface{}:
		if role, ok := v["role"].(string); ok && role == "session_meta" {
			return "hermes"
		}
		_, hasMsgs := v["messages"]
		_, hasModel := v["model"]
		if hasMsgs && hasModel {
			return "claude_code"
		}
		_, hasCand := v["candidates"]
		_, hasCont := v["contents"]
		if hasCand || hasCont {
			return "gemini"
		}
		// JSONL with role/timestamp → hermes
		role, hasRole := v["role"].(string)
		_, hasTS := v["timestamp"]
		if hasRole && hasTS {
			switch role {
			case "user", "assistant", "tool":
				return "hermes"
			}
		}
	case []interface{}:
		for _, item := range v[:min(3, len(v))] {
			if m, ok := item.(map[string]interface{}); ok {
				if _, ok := m["role"]; ok {
					return "claude_code"
				}
			}
		}
	}
	return "unknown"
}

func Parse(path string) ([]Event, string, error) {
	fmt := DetectFormat(path)
	events, err := parseByFormat(path, fmt)
	return events, fmt, err
}

func parseByFormat(path string, fmt string) ([]Event, error) {
	switch fmt {
	case "hermes":
		return parseHermes(path)
	case "claude_code":
		return parseClaude(path)
	case "gemini":
		return parseGemini(path)
	default:
		return parseGeneric(path)
	}
}

func parseHermes(path string) ([]Event, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	var events []Event
	for _, line := range strings.Split(string(data), "\n") {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}
		var ev Event
		if err := json.Unmarshal([]byte(line), &ev); err != nil {
			continue
		}
		events = append(events, ev)
	}
	return events, nil
}

func parseClaude(path string) ([]Event, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	var raw interface{}
	if err := json.Unmarshal(data, &raw); err != nil {
		return nil, err
	}

	var events []Event
	var messages []interface{}
	model := "unknown"

	switch v := raw.(type) {
	case []interface{}:
		messages = v
	case map[string]interface{}:
		if msgs, ok := v["messages"].([]interface{}); ok {
			messages = msgs
		}
		if m, ok := v["model"].(string); ok {
			model = m
		}
		if usage, ok := v["usage"]; ok {
			events = append(events, Event{
				Role:      "meta",
				ModelUsed: model,
			})
			// Store usage in a side map (handled in Analyze)
			ub, _ := json.Marshal(usage)
			var um map[string]int
			json.Unmarshal(ub, &um)
			events[len(events)-1].Usage = um
		}
	}

	for _, msg := range messages {
		m, ok := msg.(map[string]interface{})
		if !ok {
			continue
		}
		role, _ := m["role"].(string)
		content := m["content"]

		switch c := content.(type) {
		case string:
			events = append(events, Event{Role: role, Content: c})
		case []interface{}:
			for _, block := range c {
				b, ok := block.(map[string]interface{})
				if !ok {
					continue
				}
				t, _ := b["type"].(string)
				switch t {
				case "text":
					text, _ := b["text"].(string)
					events = append(events, Event{Role: role, Content: text})
				case "thinking":
					think, _ := b["thinking"].(string)
					redacted, _ := b["redacted"].(bool)
					events = append(events, Event{
						Role:      "assistant",
						Reasoning: think,
						Redacted:  redacted,
					})
				case "tool_use":
					id, _ := b["id"].(string)
					name, _ := b["name"].(string)
					events = append(events, Event{
						Role: "assistant",
						ToolCall: &ToolCall{ID: id, Name: name},
					})
				case "tool_result":
					tid, _ := b["tool_use_id"].(string)
					isErr, _ := b["is_error"].(bool)
					ct, _ := b["content"].(string)
					events = append(events, Event{
						Role:       "tool",
						ToolCallID:  tid,
						Content:    ct,
						IsError:    isErr,
					})
				}
			}
		default:
			// Other content types → skip
		}
	}
	return events, nil
}

func parseGemini(path string) ([]Event, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	raw := strings.TrimSpace(string(data))

	var events []Event

	// Try single JSON
	var doc map[string]interface{}
	if err := json.Unmarshal([]byte(raw), &doc); err == nil {
		if contents, ok := doc["contents"].([]interface{}); ok {
			for _, item := range contents {
				m, ok := item.(map[string]interface{})
				if !ok {
					continue
				}
				role, _ := m["role"].(string)
				parts, _ := m["parts"].([]interface{})
				for _, part := range parts {
					p, ok := part.(map[string]interface{})
					if !ok {
						continue
					}
					text, _ := p["text"].(string)
					if text != "" {
						events = append(events, Event{Role: role, Content: text})
					}
				}
			}
			return events, nil
		}
	}

	// Fallback: JSONL
	for _, line := range strings.Split(raw, "\n") {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}
		var ev Event
		if err := json.Unmarshal([]byte(line), &ev); err != nil {
			continue
		}
		events = append(events, ev)
	}
	return events, nil
}

func parseGeneric(path string) ([]Event, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	raw := strings.TrimSpace(string(data))

	var events []Event
	// JSONL first
	for _, line := range strings.Split(raw, "\n") {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}
		var ev Event
		if err := json.Unmarshal([]byte(line), &ev); err != nil {
			continue
		}
		events = append(events, ev)
	}

	if len(events) > 0 {
		return events, nil
	}

	// Single JSON blob
	var arr []Event
	if err := json.Unmarshal([]byte(raw), &arr); err == nil {
		return arr, nil
	}
	return events, nil
}

// ── Analysis ──

func Analyze(events []Event, model string) Metrics {
	m := Metrics{
		ModelUsed: model,
		ToolUsage: make(map[string]int),
	}

	pricing := Pricing[model]
	if _, ok := Pricing[model]; !ok {
		pricing = Pricing["default"]
	}

	for _, ev := range events {
		// Timestamp
		ts := parseTS(ev.Timestamp)
		if !ts.IsZero() {
			m.Timestamps = append(m.Timestamps, ts)
		}

		switch ev.Role {
		case "session_meta", "meta":
			if ev.Usage != nil {
				m.TokensInput += ev.Usage["input_tokens"]
				m.TokensOutput += ev.Usage["output_tokens"]
				m.TokensCacheW += ev.Usage["cache_creation_input_tokens"]
				m.TokensCacheR += ev.Usage["cache_read_input_tokens"]
			}
			continue

		case "user":
			m.UserMessages++
			if ev.Content != "" {
				m.TokensInput += max(1, len(ev.Content)/4)
			}

		case "assistant":
			m.AssistantTurns++

			// Reasoning
			reasoning := ev.Reasoning
			if reasoning == "" {
				// Check for reasoning_content in raw (handled during parse)
				reasoning = ev.Content // some formats put reasoning in content
			}
			if reasoning != "" {
				m.ReasoningBlocks++
				rc := len(reasoning)
				m.ReasoningChars += rc
				m.ReasoningLens = append(m.ReasoningLens, rc)
				if ev.Redacted {
					m.ReasoningRedact++
				}
				m.TokensOutput += max(1, rc/4)
			}

			// Text content
			if ev.Content != "" {
				m.TokensOutput += max(1, len(ev.Content)/4)
			}

			// Tool calls
			tcs := ev.ToolCalls
			if ev.ToolCall != nil {
				tcs = []ToolCall{*ev.ToolCall}
			}
			m.ToolCallsTotal += len(tcs)
			for _, tc := range tcs {
				name := tc.Name
				if tc.Function != nil && tc.Function.Name != "" {
					name = tc.Function.Name
				}
				if name == "" {
					name = "unknown"
				}
				m.ToolUsage[name]++
			}

		case "tool":
			m.ToolResults++
			isErr := ev.IsError
			if !isErr && ev.Content != "" {
				// Try parsing JSON for error signals
				var r map[string]interface{}
				if err := json.Unmarshal([]byte(ev.Content), &r); err == nil {
					if s, ok := r["success"]; ok && s == false {
						isErr = true
					}
					if e, ok := r["error"]; ok && e != nil {
						isErr = true
					}
				}
			}
			if isErr {
				m.ToolCallsFail++
			} else {
				m.ToolCallsOK++
			}
		}
	}

	// Post-processing
	m.EventsTotal = len(events)

	if len(m.Timestamps) > 0 {
		sort.Slice(m.Timestamps, func(i, j int) bool {
			return m.Timestamps[i].Before(m.Timestamps[j])
		})
		m.SessionStart = m.Timestamps[0].Format(time.RFC3339)
		m.SessionEnd = m.Timestamps[len(m.Timestamps)-1].Format(time.RFC3339)
		m.DurationSec = m.Timestamps[len(m.Timestamps)-1].Sub(m.Timestamps[0]).Seconds()
	}

	for i := 1; i < len(m.Timestamps); i++ {
		gap := m.Timestamps[i].Sub(m.Timestamps[i-1]).Seconds()
		if gap > 0 {
			m.GapsSec = append(m.GapsSec, gap)
		}
	}

	// Cost estimation
	m.CostEstimated = round4(
		float64(m.TokensInput)/1e6*pricing.Input +
			float64(m.TokensOutput)/1e6*pricing.Output +
			float64(m.TokensCacheW)/1e6*pricing.CW +
			float64(m.TokensCacheR)/1e6*pricing.CR,
	)

	return m
}

// ── Anomaly Detection ──

func DetectAnomalies(m Metrics) []Anomaly {
	var a []Anomaly

	if len(m.GapsSec) > 0 {
		sl := make([]float64, len(m.GapsSec))
		copy(sl, m.GapsSec)
		sort.Float64s(sl)

		var longGaps int
		maxGap := sl[len(sl)-1]
		hasSuperLong := false
		for _, g := range sl {
			if g > 60 {
				longGaps++
			}
			if g > 300 {
				hasSuperLong = true
			}
		}

		if hasSuperLong {
			a = append(a, Anomaly{
				Type: "hanging", Severity: "high", Emoji: "🔴",
				Detail: fmt.Sprintf("%d gap(s) >60s, max=%.0fs", longGaps, maxGap),
			})
		} else if longGaps > 0 {
			a = append(a, Anomaly{
				Type: "hanging", Severity: "medium", Emoji: "🟡",
				Detail: fmt.Sprintf("%d gap(s) >60s, max=%.0fs", longGaps, maxGap),
			})
		} else if percentile(sl, 0.95) > 30 {
			a = append(a, Anomaly{
				Type: "latency", Severity: "low", Emoji: "🟢",
				Detail: fmt.Sprintf("p95 latency = %.1fs", percentile(sl, 0.95)),
			})
		}
	}

	totalTools := m.ToolCallsOK + m.ToolCallsFail
	if totalTools > 0 {
		failRate := float64(m.ToolCallsFail) / float64(totalTools)
		if failRate > 0.30 {
			a = append(a, Anomaly{
				Type: "tool_failures", Severity: "high", Emoji: "🔴",
				Detail: fmt.Sprintf("%d/%d failed (%.0f%%)", m.ToolCallsFail, totalTools, failRate*100),
			})
		} else if failRate > 0.15 {
			a = append(a, Anomaly{
				Type: "tool_failures", Severity: "medium", Emoji: "🟡",
				Detail: fmt.Sprintf("%d/%d failed (%.0f%%)", m.ToolCallsFail, totalTools, failRate*100),
			})
		}
	}

	if len(m.ReasoningLens) > 0 {
		avgReason := float64(m.ReasoningChars) / float64(m.ReasoningBlocks)
		if avgReason < 200 {
			a = append(a, Anomaly{
				Type: "shallow_thinking", Severity: "high", Emoji: "🔴",
				Detail: fmt.Sprintf("avg reasoning = %.0f chars (very shallow)", avgReason),
			})
		} else if avgReason < 500 {
			a = append(a, Anomaly{
				Type: "shallow_thinking", Severity: "medium", Emoji: "🟡",
				Detail: fmt.Sprintf("avg reasoning = %.0f chars", avgReason),
			})
		}
	}

	if m.ReasoningRedact > 0 {
		a = append(a, Anomaly{
			Type: "redaction", Severity: "medium", Emoji: "🟡",
			Detail: fmt.Sprintf("%d block(s) redacted", m.ReasoningRedact),
		})
	}

	if m.ToolCallsTotal == 0 && m.AssistantTurns > 2 {
		a = append(a, Anomaly{
			Type: "no_tools", Severity: "low", Emoji: "🟢",
			Detail: "no tool calls — chat-only session",
		})
	}

	return a
}

// ── Health Score ──

func HealthScore(m Metrics, anoms []Anomaly) int {
	score := 100
	penalties := map[string]int{"high": 30, "medium": 12, "low": 4}
	for _, a := range anoms {
		score -= penalties[a.Severity]
	}
	if score < 0 {
		score = 0
	}
	if score > 100 {
		score = 100
	}
	return score
}

// ── Session helpers ──

func LoadSession(path string) (*Session, error) {
	name := strings.TrimSuffix(filepath.Base(path), filepath.Ext(path))
	events, _, err := Parse(path)
	if err != nil {
		return nil, err
	}
	if len(events) == 0 {
		// Try with format force
		events, err = parseByFormat(path, "unknown")
		if err != nil || len(events) == 0 {
			return nil, fmt.Errorf("no parseable events")
		}
	}
	m := Analyze(events, "default")
	a := DetectAnomalies(m)
	h := HealthScore(m, a)
	return &Session{Name: name, Metrics: m, Anomalies: a, Health: h}, nil
}

func LoadAll(dir string) []Session {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil
	}
	var sessions []Session
	for _, e := range entries {
		if e.IsDir() || !strings.HasSuffix(e.Name(), ".jsonl") {
			continue
		}
		path := filepath.Join(dir, e.Name())
		s, err := LoadSession(path)
		if err != nil {
			continue
		}
		sessions = append(sessions, *s)
	}
	// Sort by name desc (newest first)
	sort.Slice(sessions, func(i, j int) bool {
		return sessions[i].Name > sessions[j].Name
	})
	return sessions
}

func FindJSONLFiles(dir string) []string {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil
	}
	var files []string
	for _, e := range entries {
		if !e.IsDir() && strings.HasSuffix(e.Name(), ".jsonl") {
			files = append(files, filepath.Join(dir, e.Name()))
		}
	}
	sort.Sort(sort.Reverse(sort.StringSlice(files)))
	return files
}

// ── Utilities ──

func parseTS(raw string) time.Time {
	if raw == "" {
		return time.Time{}
	}
	s := strings.ReplaceAll(raw, "Z", "+00:00")
	t, err := time.Parse(time.RFC3339, s)
	if err != nil {
		// Try other formats
		t, err = time.Parse("2006-01-02T15:04:05", s)
		if err != nil {
			return time.Time{}
		}
		return t.UTC()
	}
	return t
}

func percentile(sorted []float64, pct float64) float64 {
	if len(sorted) == 0 {
		return 0
	}
	idx := int(float64(len(sorted)) * pct)
	if idx >= len(sorted) {
		idx = len(sorted) - 1
	}
	if idx < 0 {
		idx = 0
	}
	return sorted[idx]
}

func safeCalc(lst []float64, fn func([]float64) float64) float64 {
	if len(lst) == 0 {
		return 0
	}
	return fn(lst)
}

func round4(v float64) float64 {
	return math.Round(v*10000) / 10000
}

func fmtDuration(sec float64) string {
	if sec < 60 {
		return fmt.Sprintf("%.0fs", sec)
	}
	if sec < 3600 {
		return fmt.Sprintf("%.1fm", sec/60)
	}
	h := int(sec / 3600)
	m := int(int(sec)%3600) / 60
	return fmt.Sprintf("%dh %dm", h, m)
}

func HealthEmoji(h int) string {
	if h >= 80 {
		return "🟢"
	}
	if h >= 50 {
		return "🟡"
	}
	return "🔴"
}

func HealthBar(h int) string {
	blocks := h / 5
	empty := 20 - blocks
	return "[" + strings.Repeat("█", blocks) + strings.Repeat("░", empty) + "]"
}

func SuccessRate(ok, total int) string {
	if total == 0 {
		return "N/A"
	}
	return fmt.Sprintf("%.0f%%", float64(ok)/float64(total)*100)
}
