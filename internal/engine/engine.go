// Package engine provides the core analysis engine for agentwaste.
// Pure Go. Supports 6 agent formats: Hermes Agent, Claude Code, Codex CLI, Gemini CLI, OpenCode, OpenClaw.
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

	"github.com/luoyuctl/agentwaste/internal/i18n"
)

const Version = "0.1.1"

// ── Pricing (USD per 1M tokens) ──

type Price struct {
	Input  float64
	Output float64
	CW     float64
	CR     float64
}

var ToolDisplayNames = map[string]string{
	"hermes_jsonl": "Hermes Agent (JSONL)",
	"hermes_json":  "Hermes Agent (.json)",
	"claude_code":  "Claude Code",
	"codex_cli":    "Codex CLI",
	"gemini_cli":   "Gemini CLI",
	"opencode":     "OpenCode",
	"openclaw":     "OpenClaw",
	"generic":      "Generic JSON/JSONL",
}

// ── Normalized Event ──

type Event struct {
	Role        string
	Content     string
	Timestamp   string
	Reasoning   string
	Redacted    bool
	ToolCalls   []ToolCall `json:"tool_calls"`
	ToolCallID  string     `json:"tool_call_id"`
	IsError     bool       `json:"is_error"`
	Usage       map[string]int
	ModelUsed   string
	SourceTool  string // which tool produced this session
}

type ToolCall struct {
	ID   string `json:"id"`
	Name string `json:"name"`
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
	SourceTool      string
	SessionStart    string
	SessionEnd      string
	DurationSec     float64
	CostEstimated   float64
	LoopRetryEvents int     `json:"-"`
	LoopGroups      int     `json:"-"`
	LoopCostEst     float64 `json:"-"`
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
	Name      string
	Path      string
	Metrics   Metrics
	Anomalies []Anomaly
	Health    int
	LoopCost  LoopCost

	// v0.2: community-driven diagnostics (pre-computed in LoadSession)
	LoopFingerprints   []LoopFingerprint
	ToolLatencies      []ToolLatencyItem
	ContextUtil        ContextUtilization
	LargeParams        []LargeParamCall
	UnusedTools        []UnusedToolInfo
	StuckPatternsExtra []StuckPattern
	LoopResultData     LoopResult
	ToolWarnings       []ToolWarning
}

// ── Loop Cost Analysis ──

type LoopCost struct {
	RetryCost       float64 `json:"retry_cost"`
	ToolLoopCost    float64 `json:"tool_loop_cost"`
	FormatRetryCost float64 `json:"format_retry_cost"`
	TotalLoopCost   float64 `json:"total_loop_cost"`
	RetryEvents     int     `json:"retry_events"`
	LoopGroups      int     `json:"loop_groups"`
}

// ── Global Overview ──

// AgentOverview holds per-agent aggregate stats.
type AgentOverview struct {
	Sessions int
	Cost     float64
}

// ModelOverview holds per-model aggregate stats.
type ModelOverview struct {
	Sessions int
	Cost     float64
}

// AnomalyTop is a lightweight anomaly reference for the overview.
type AnomalyTop struct {
	Session string
	Type    string
	Age     string // human-readable relative time
}

// Overview aggregates all sessions for the dashboard.
type Overview struct {
	TotalSessions int
	Healthy       int
	Warning       int
	Critical      int
	TotalCost     float64
	AnomaliesTop  []AnomalyTop
	ByAgent       map[string]AgentOverview
	ByModel       map[string]ModelOverview
}

// ComputeOverview builds the global dashboard overview from loaded sessions.
func ComputeOverview(sessions []Session) Overview {
	ov := Overview{
		ByAgent: make(map[string]AgentOverview),
		ByModel: make(map[string]ModelOverview),
	}
	for _, s := range sessions {
		ov.TotalSessions++
		ov.TotalCost += s.Metrics.CostEstimated
		switch {
		case s.Health >= 80:
			ov.Healthy++
		case s.Health >= 50:
			ov.Warning++
		default:
			ov.Critical++
		}
		// By agent
		agent := s.Metrics.SourceTool
		ao := ov.ByAgent[agent]
		ao.Sessions++
		ao.Cost += s.Metrics.CostEstimated
		ov.ByAgent[agent] = ao
		// By model
		model := s.Metrics.ModelUsed
		if model == "" {
			model = "unknown"
		}
		mo := ov.ByModel[model]
		mo.Sessions++
		mo.Cost += s.Metrics.CostEstimated
		ov.ByModel[model] = mo
		// Anomalies
		for _, a := range s.Anomalies {
			ov.AnomaliesTop = append(ov.AnomaliesTop, AnomalyTop{
				Session: s.Name,
				Type:    a.Type,
				Age:     "now",
			})
		}
	}
	// Sort anomalies by severity-ish order (high → medium → low)
	sort.Slice(ov.AnomaliesTop, func(i, j int) bool {
		severityOrder := map[string]int{"high": 0, "medium": 1, "low": 2}
		ai := severityOrder[ov.AnomaliesTop[i].Type]
		aj := severityOrder[ov.AnomaliesTop[j].Type]
		return ai < aj
	})
	return ov
}

// ── Aggregate Stats (for btop-style health panel) ──

// AggregateStats holds aggregate metrics for the health panel.
type AggregateStats struct {
	TotalSessions int
	Healthy       int
	Warning       int
	Critical      int
	AvgHealth     float64
	TotalCost     float64
}

// ComputeAggregateStats computes AggregateStats from sessions.
func ComputeAggregateStats(sessions []Session) AggregateStats {
	var as AggregateStats
	as.TotalSessions = len(sessions)
	if len(sessions) == 0 {
		return as
	}
	sumHealth := 0
	sumCost := 0.0
	for _, s := range sessions {
		sumHealth += s.Health
		sumCost += s.Metrics.CostEstimated
		switch {
		case s.Health >= 80:
			as.Healthy++
		case s.Health >= 50:
			as.Warning++
		default:
			as.Critical++
		}
	}
	as.TotalCost = sumCost
	as.AvgHealth = float64(sumHealth) / float64(len(sessions))
	return as
}

// ── Filter Sessions ──

// FilterSessions filters sessions by a text query (case-insensitive match on Name and SourceTool).
func FilterSessions(sessions []Session, query string) []Session {
	if query == "" {
		return sessions
	}
	q := strings.ToLower(query)
	var filtered []Session
	for _, s := range sessions {
		if strings.Contains(strings.ToLower(s.Name), q) ||
			strings.Contains(strings.ToLower(s.Metrics.SourceTool), q) {
			filtered = append(filtered, s)
		}
	}
	return filtered
}

// ── Loop Analysis ──

// LoopResult is the result of analyzing events for loops.
type LoopResult struct {
	HasLoop   bool
	LoopType  string // "retry", "tool_loop", etc.
	Turns     int
	LoopCost  float64
}

// AnalyzeLoops detects redundant/looping behavior in events.
// It looks for patterns of repeated tool calls with the same name.
func AnalyzeLoops(events []Event) LoopResult {
	var lr LoopResult
	if len(events) == 0 {
		return lr
	}
	// Count consecutive repeated tool calls
	seen := make(map[string]int) // toolName → consecutive count
	maxRepeat := 0
	repeatName := ""
	lastTool := ""
	consecutive := 0
	for _, ev := range events {
		if len(ev.ToolCalls) > 0 {
			for _, tc := range ev.ToolCalls {
				key := tc.Name
				seen[key]++
				if key == lastTool {
					consecutive++
				} else {
					if consecutive > maxRepeat {
						maxRepeat = consecutive
						repeatName = lastTool
					}
					consecutive = 1
				}
				lastTool = key
			}
		}
	}
	if consecutive > maxRepeat {
		maxRepeat = consecutive
		repeatName = lastTool
	}

	if maxRepeat >= 3 {
		lr.HasLoop = true
		lr.LoopType = "tool_loop"
		if repeatName != "" {
			lr.LoopType = repeatName + "_loop"
		}
		lr.Turns = maxRepeat
		// Estimate loop cost: each turn is roughly cost of 1 assistant turn
		assistantTurns := 0
		for _, ev := range events {
			if ev.Role == "assistant" {
				assistantTurns++
			}
		}
		if assistantTurns > 0 {
			avgCost := 0.015 // rough estimate per turn
			lr.LoopCost = float64(lr.Turns) * avgCost
		}
	}
	return lr
}

// ═══════════════════════════════════════════════════════════════
// FORMAT DETECTION
// ═══════════════════════════════════════════════════════════════

// FormatInfo holds detected format + the parsed top-level data (to avoid re-reading).
type FormatInfo struct {
	Format string
	Raw    []byte   // first ~8KB for JSONL, full content for single JSON
	Doc    map[string]interface{} // parsed top-level JSON if single blob
	Arr    []interface{}          // parsed top-level JSON array
}

func DetectFormat(path string) FormatInfo {
	fi := FormatInfo{Format: "unknown"}

	// Read first 64KB for detection
	data, err := os.ReadFile(path)
	if err != nil {
		return fi
	}
	if len(data) > 64*1024 {
		data = data[:64*1024]
	}
	content := strings.TrimSpace(string(data))
	if content == "" {
		return fi
	}

	// Try as single JSON blob first
	if content[0] == '{' || content[0] == '[' {
		var doc map[string]interface{}
		if err := json.Unmarshal(data, &doc); err == nil {
			fi.Doc = doc
			fi.Raw = data
			fi.Format = detectSingleJSON(doc)
			return fi
		}
		var arr []interface{}
		if err := json.Unmarshal(data, &arr); err == nil {
			fi.Arr = arr
			fi.Raw = data
			fi.Format = detectJSONArray(arr)
		}
	}

	// JSONL: check first few valid lines (skip empty and comments)
	var firstLineObj map[string]interface{}
	for _, line := range strings.Split(content, "\n") {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		if err := json.Unmarshal([]byte(line), &firstLineObj); err == nil {
			break
		}
	}
	if firstLineObj != nil {
		fi.Raw = data

		// Hermes JSONL: role=session_meta or role with timestamp
		if role, _ := firstLineObj["role"].(string); role == "session_meta" {
			fi.Format = "hermes_jsonl"
			return fi
		}
		if role, _ := firstLineObj["role"].(string); (role == "user" || role == "assistant" || role == "tool") {
			if _, hasTS := firstLineObj["timestamp"]; hasTS {
				fi.Format = "hermes_jsonl"
				return fi
			}
		}
		// Gemini JSONL
		if _, hasCand := firstLineObj["candidates"]; hasCand {
			fi.Format = "gemini_cli"
			return fi
		}
		if _, hasCont := firstLineObj["contents"]; hasCont {
			fi.Format = "gemini_cli"
			return fi
		}
		// Generic JSONL with role field → try parse as generic
		if _, hasRole := firstLineObj["role"]; hasRole {
			fi.Format = "generic"
			return fi
		}
	}

	return fi
}

func detectSingleJSON(doc map[string]interface{}) string {
	// OpenClaw: provider="openclaw" distinguishes from other formats
	if provider, _ := doc["provider"].(string); provider == "openclaw" {
		return "openclaw"
	}

	// Hermes .json: session_id + messages + model + platform, NO "usage" at top level
	_, hasSessID := doc["session_id"]
	_, hasMsgs := doc["messages"]
	_, hasModel := doc["model"]
	_, hasPlatform := doc["platform"]
	_, hasUsage := doc["usage"]

	if hasSessID && hasMsgs && hasModel && hasPlatform {
		return "hermes_json"
	}

	// Claude Code: messages + model, messages contain Anthropic content blocks
	if hasMsgs && hasModel {
		if hasUsage {
			return "claude_code"
		}
		// Check if messages use Anthropic block format (content is array of {"type":"..."})
		if msgs, ok := doc["messages"].([]interface{}); ok {
			for _, m := range msgs {
				if msg, ok := m.(map[string]interface{}); ok {
					if content, ok := msg["content"]; ok {
						if _, isArr := content.([]interface{}); isArr {
							return "claude_code"
						}
					}
				}
			}
		}
		// OpenCode: has messages + model + provider or additional metadata but no usage
		if _, hasProvider := doc["provider"]; hasProvider {
			return "opencode"
		}
		if _, hasSession := doc["session"]; hasSession {
			return "opencode"
		}
		// Hermes JSON without platform field (backward compat)
		if hasSessID {
			return "hermes_json"
		}
		// Default to Codex CLI for OpenAI-style messages
		return "codex_cli"
	}

	// Codex CLI: messages + model + created/id (OpenAI API style)
	_, hasCreated := doc["created"]
	_, hasID := doc["id"]
	if hasMsgs && (hasCreated || hasID) {
		return "codex_cli"
	}

	// Gemini CLI: contents + candidates
	_, hasContents := doc["contents"]
	_, hasCandidates := doc["candidates"]
	if hasContents || hasCandidates {
		return "gemini_cli"
	}

	// Default with messages → try Claude Code block format
	if hasMsgs {
		return "codex_cli"
	}

	return "unknown"
}

func detectJSONArray(arr []interface{}) string {
	for _, item := range arr {
		if m, ok := item.(map[string]interface{}); ok {
			if _, hasRole := m["role"]; hasRole {
				// Check content format
				if content, ok := m["content"]; ok {
					if _, isArr := content.([]interface{}); isArr {
						return "claude_code"
					}
				}
				return "codex_cli"
			}
		}
	}
	return "generic"
}

// ═══════════════════════════════════════════════════════════════
// PARSERS
// ═══════════════════════════════════════════════════════════════

func Parse(path string) ([]Event, error) {
	fi := DetectFormat(path)
	switch fi.Format {
	case "hermes_jsonl":
		return parseHermesJSONL(string(fi.Raw))
	case "hermes_json":
		return parseHermesJSON(fi.Doc)
	case "claude_code":
		return parseClaudeCode(fi.Doc, fi.Arr)
	case "codex_cli":
		return parseCodexCLI(fi.Doc, fi.Arr, string(fi.Raw))
	case "gemini_cli":
		return parseGeminiCLI(fi.Doc, string(fi.Raw))
	case "opencode":
		return parseOpenCode(fi.Doc)
	case "openclaw":
		return parseOpenClaw(fi.Doc)
	default:
		return parseGeneric(string(fi.Raw))
	}
}

// ── Hermes Agent JSONL ──

func parseHermesJSONL(raw string) ([]Event, error) {
	var events []Event
	for _, line := range strings.Split(raw, "\n") {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}
		var ev Event
		if err := json.Unmarshal([]byte(line), &ev); err != nil {
			continue
		}
		ev.SourceTool = "hermes_jsonl"
		events = append(events, ev)
	}
	return events, nil
}

// ── Hermes Agent .json (message array in dict) ──

func parseHermesJSON(doc map[string]interface{}) ([]Event, error) {
	var events []Event
	model := ""
	if m, ok := doc["model"].(string); ok {
		model = m
	}

	// Extract session-level timestamps (messages lack per-message timestamps)
	sessionStart, _ := doc["session_start"].(string)
	sessionEnd, _ := doc["last_updated"].(string)

	// Bug 2 fix: extract top-level usage as meta event
	if usage, ok := doc["usage"]; ok {
		ev := Event{Role: "meta", ModelUsed: model, SourceTool: "hermes_json"}
		ub, _ := json.Marshal(usage)
		json.Unmarshal(ub, &ev.Usage)
		events = append(events, ev)
	}

	msgs, _ := doc["messages"].([]interface{})
	for _, raw := range msgs {
		msg, ok := raw.(map[string]interface{})
		if !ok {
			continue
		}

		role, _ := msg["role"].(string)

		// Bug 1 fix: skip tool messages in first pass; handled in second pass
		if role == "tool" {
			continue
		}

		// Reasonings
		reasoning, _ := msg["reasoning"].(string)
		if reasoning == "" {
			reasoning, _ = msg["reasoning_content"].(string)
		}
		redacted, _ := msg["redacted"].(bool)

		// Tool calls (OpenAI function-call style)
		var toolCalls []ToolCall
		if tcs, ok := msg["tool_calls"].([]interface{}); ok {
			for _, tc := range tcs {
				if tcm, ok := tc.(map[string]interface{}); ok {
					tc := ToolCall{ID: str(tcm, "id")}
					if fn, ok := tcm["function"].(map[string]interface{}); ok {
						tc.Name = str(fn, "name")
					}
					toolCalls = append(toolCalls, tc)
				}
			}
		}

		// Content
		content, _ := msg["content"].(string)

		// Bug 3 fix: extract timestamp
		ts, _ := msg["timestamp"].(string)

		ev := Event{
			Role:       role,
			Content:    content,
			Timestamp:  ts,
			Reasoning:  reasoning,
			Redacted:   redacted,
			ToolCalls:  toolCalls,
			ModelUsed:  model,
			SourceTool: "hermes_json",
		}
		events = append(events, ev)
	}

	// Tool results are embedded in messages with role=tool
	for _, raw := range msgs {
		msg, ok := raw.(map[string]interface{})
		if !ok {
			continue
		}
		role, _ := msg["role"].(string)
		if role != "tool" {
			continue
		}
		content, _ := msg["content"].(string)
		isErr, _ := msg["is_error"].(bool)
		ts, _ := msg["timestamp"].(string)

		events = append(events, Event{
			Role:       "tool",
			Content:    content,
			Timestamp:  ts,
			ToolCallID: str(msg, "tool_call_id"),
			IsError:    isErr,
			SourceTool: "hermes_json",
		})
	}

	// Inject session timestamps if messages lack them
	if sessionStart != "" || sessionEnd != "" {
		hasTimestamps := false
		for _, ev := range events {
			if ev.Timestamp != "" {
				hasTimestamps = true
				break
			}
		}
		if !hasTimestamps && len(events) > 0 {
			for i := range events {
				if events[i].Role != "meta" && events[i].Role != "session_meta" {
					if sessionStart != "" {
						events[i].Timestamp = sessionStart
					}
					break
				}
			}
			for i := len(events) - 1; i >= 0; i-- {
				if events[i].Role != "meta" && events[i].Role != "session_meta" {
					if sessionEnd != "" {
						events[i].Timestamp = sessionEnd
					}
					break
				}
			}
		}
	}

	return events, nil
}

// ── Claude Code (Anthropic API content blocks) ──

func parseClaudeCode(doc map[string]interface{}, arr []interface{}) ([]Event, error) {
	var events []Event

	// Resolve messages and model
	var messages []interface{}
	model := "unknown"

	if arr != nil {
		messages = arr
	} else if doc != nil {
		if msgs, ok := doc["messages"].([]interface{}); ok {
			messages = msgs
		}
		if m, ok := doc["model"].(string); ok {
			model = m
		}
		// Top-level usage → meta event
		if usage, ok := doc["usage"]; ok {
			ev := Event{Role: "meta", ModelUsed: model, SourceTool: "claude_code"}
			ub, _ := json.Marshal(usage)
			json.Unmarshal(ub, &ev.Usage)
			events = append(events, ev)
		}
	}

	for _, raw := range messages {
		msg, ok := raw.(map[string]interface{})
		if !ok {
			continue
		}
		role, _ := msg["role"].(string)
		// Bug 3 fix: extract timestamp
		ts, _ := msg["timestamp"].(string)

		content := msg["content"]
		switch c := content.(type) {
		case string:
			events = append(events, Event{
				Role: role, Content: c, Timestamp: ts,
				SourceTool: "claude_code",
			})
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
					events = append(events, Event{
						Role: role, Content: text, Timestamp: ts,
						SourceTool: "claude_code",
					})
				case "thinking":
					think, _ := b["thinking"].(string)
					redacted, _ := b["redacted"].(bool)
					events = append(events, Event{
						Role:       "assistant",
						Timestamp:  ts,
						Reasoning:  think,
						Redacted:   redacted,
						SourceTool: "claude_code",
					})
				case "tool_use":
					id, _ := b["id"].(string)
					name, _ := b["name"].(string)
					events = append(events, Event{
						Role: "assistant", Timestamp: ts,
						ToolCalls: []ToolCall{{ID: id, Name: name}},
						SourceTool: "claude_code",
					})
				case "tool_result":
					tid, _ := b["tool_use_id"].(string)
					isErr, _ := b["is_error"].(bool)
					ct := extractToolResultContent(b)
					events = append(events, Event{
						Role:       "tool",
						Timestamp:  ts,
						ToolCallID: tid,
						Content:    ct,
						IsError:    isErr,
						SourceTool: "claude_code",
					})
				}
			}
		}
	}

	return events, nil
}

// ── Codex CLI (OpenAI API format) ──

func parseCodexCLI(doc map[string]interface{}, arr []interface{}, raw string) ([]Event, error) {
	_ = raw // Bug 8 fix: suppress unused warning; kept for API compatibility
	var events []Event

	var messages []interface{}
	model := "unknown"

	if arr != nil {
		messages = arr
	} else if doc != nil {
		if msgs, ok := doc["messages"].([]interface{}); ok {
			messages = msgs
		}
		if m, ok := doc["model"].(string); ok {
			model = m
		}
		// Usage at top
		if usage, ok := doc["usage"]; ok {
			ev := Event{Role: "meta", ModelUsed: model, SourceTool: "codex_cli"}
			ub, _ := json.Marshal(usage)
			json.Unmarshal(ub, &ev.Usage)
			events = append(events, ev)
		}
	}

	for _, raw := range messages {
		msg, ok := raw.(map[string]interface{})
		if !ok {
			continue
		}
		role, _ := msg["role"].(string)

		// Bug 3 fix: extract timestamp
		ts, _ := msg["timestamp"].(string)

		// OpenAI reasoning (o1/o3 models)
		reasoning, _ := msg["reasoning_content"].(string)
		if reasoning == "" {
			reasoning, _ = msg["reasoning"].(string)
		}

		// Content
		content, _ := msg["content"].(string)

		// Tool calls (OpenAI function-call format)
		var toolCalls []ToolCall
		if tcs, ok := msg["tool_calls"].([]interface{}); ok {
			for _, tc := range tcs {
				if tcm, ok := tc.(map[string]interface{}); ok {
					tc := ToolCall{ID: str(tcm, "id")}
					if fn, ok := tcm["function"].(map[string]interface{}); ok {
						tc.Name = str(fn, "name")
					}
					toolCalls = append(toolCalls, tc)
				}
			}
		}

		// Bug 4 fix: Codex CLI also stores assistant message with Anthropic blocks;
		// extract tool_use AND text blocks from content array.
		if role == "assistant" && len(toolCalls) == 0 {
			if cArr, ok := msg["content"].([]interface{}); ok {
				for _, blk := range cArr {
					if b, ok := blk.(map[string]interface{}); ok {
						tp, _ := b["type"].(string)
						switch tp {
						case "tool_use":
							toolCalls = append(toolCalls, ToolCall{
								ID: str(b, "id"), Name: str(b, "name"),
							})
						case "text":
							if text, _ := b["text"].(string); text != "" {
								// If content was empty from string assertion, use text block
								if content == "" {
									content = text
								} else {
									content += "\n" + text
								}
							}
						}
					}
				}
			}
		}

		ev := Event{
			Role:       role,
			Content:    content,
			Timestamp:  ts,
			Reasoning:  reasoning,
			ToolCalls:  toolCalls,
			ModelUsed:  model,
			SourceTool: "codex_cli",
		}

		// Tool results
		if role == "tool" {
			ev.ToolCallID = str(msg, "tool_call_id")
			ev.IsError, _ = msg["is_error"].(bool)
			// Bug 11 fix: tool content may be array (multi-modal)
			if ev.Content == "" {
				ev.Content = extractToolResultContent(msg)
			}
		}

		events = append(events, ev)
	}

	return events, nil
}

// ── Gemini CLI ──

func parseGeminiCLI(doc map[string]interface{}, raw string) ([]Event, error) {
	var events []Event

	// Bug 5 fix: extract model and usage metadata from Gemini responses
	model := "unknown"

	// Single JSON blob with contents
	if doc != nil {
		if mv, ok := doc["modelVersion"].(string); ok && mv != "" {
			model = mv
		}
		// Extract usage metadata
		if um, ok := doc["usageMetadata"]; ok {
			ev := Event{Role: "meta", ModelUsed: model, SourceTool: "gemini_cli"}
			ub, _ := json.Marshal(um)
			json.Unmarshal(ub, &ev.Usage)
			events = append(events, ev)
		}
		// Also check top-level timestamp
		if ts, _ := doc["timestamp"].(string); ts != "" {
			// Will be used below
			_ = ts
		}
		if contents, ok := doc["contents"].([]interface{}); ok {
			for _, item := range contents {
				cItem, ok := item.(map[string]interface{})
				if !ok {
					continue
				}
				role, _ := cItem["role"].(string)
				// Bug 3 fix: extract timestamp from content item
				ts, _ := cItem["timestamp"].(string)
				parts, _ := cItem["parts"].([]interface{})
				for _, part := range parts {
					p, ok := part.(map[string]interface{})
					if !ok {
						continue
					}
					text, _ := p["text"].(string)
					if text != "" {
						events = append(events, Event{
							Role: role, Content: text, Timestamp: ts,
							SourceTool: "gemini_cli",
						})
					}
					// Function calls
					if fc, ok := p["functionCall"]; ok {
						fcJSON, _ := json.Marshal(fc)
						events = append(events, Event{
							Role:    role,
							Content: string(fcJSON), Timestamp: ts,
							SourceTool: "gemini_cli",
						})
					}
					if fr, ok := p["functionResponse"]; ok {
						frJSON, _ := json.Marshal(fr)
						events = append(events, Event{
							Role:    "tool",
							Content: string(frJSON), Timestamp: ts,
							SourceTool: "gemini_cli",
						})
					}
				}
			}
			return events, nil
		}
	}

	// JSONL: each line is a Gemini response object
	for _, line := range strings.Split(raw, "\n") {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}
		var obj map[string]interface{}
		if err := json.Unmarshal([]byte(line), &obj); err != nil {
			continue
		}

		// Bug 5 fix: extract model and usage from JSONL lines
		lineModel := model
		if mv, ok := obj["modelVersion"].(string); ok && mv != "" {
			lineModel = mv
		}
		if um, ok := obj["usageMetadata"]; ok {
			ev := Event{Role: "meta", ModelUsed: lineModel, SourceTool: "gemini_cli"}
			ub, _ := json.Marshal(um)
			json.Unmarshal(ub, &ev.Usage)
			events = append(events, ev)
		}
		// Bug 3 fix: extract timestamp
		ts, _ := obj["timestamp"].(string)

		if contents, ok := obj["contents"].([]interface{}); ok {
			for _, item := range contents {
				cItem, ok := item.(map[string]interface{})
				if !ok {
					continue
				}
				role, _ := cItem["role"].(string)
				parts, _ := cItem["parts"].([]interface{})
				for _, part := range parts {
					p, ok := part.(map[string]interface{})
					if !ok {
						continue
					}
					text, _ := p["text"].(string)
					if text != "" {
						events = append(events, Event{
							Role: role, Content: text, Timestamp: ts,
							SourceTool: "gemini_cli",
						})
					}
				}
			}
		}
		// Candidates format
		if candidates, ok := obj["candidates"].([]interface{}); ok {
			for _, cand := range candidates {
				c, ok := cand.(map[string]interface{})
				if !ok {
					continue
				}
				if cont, ok := c["content"].(map[string]interface{}); ok {
					role, _ := cont["role"].(string)
					parts, _ := cont["parts"].([]interface{})
					for _, part := range parts {
						p, ok := part.(map[string]interface{})
						if !ok {
							continue
						}
						text, _ := p["text"].(string)
						if text != "" {
							events = append(events, Event{
								Role: role, Content: text, Timestamp: ts,
								SourceTool: "gemini_cli",
							})
						}
					}
				}
			}
		}
	}

	return events, nil
}

// ── OpenCode (Claude-compatible with session wrapper) ──

func parseOpenCode(doc map[string]interface{}) ([]Event, error) {
	// OpenCode stores sessions similar to Claude Code but with extra metadata
	// May wrap in {"session": {...}, "messages": [...], "provider": "anthropic"}
	var events []Event
	var messages []interface{}
	model := "unknown"

	// Unwrap session wrapper
	target := doc
	if sess, ok := doc["session"].(map[string]interface{}); ok {
		target = sess
	}

	if msgs, ok := target["messages"].([]interface{}); ok {
		messages = msgs
	}
	if m, ok := target["model"].(string); ok {
		model = m
	}
	if model == "unknown" {
		if m, ok := doc["model"].(string); ok {
			model = m
		}
	}

	// Usage at top
	if usage, ok := target["usage"]; ok {
		ev := Event{Role: "meta", ModelUsed: model, SourceTool: "opencode"}
		ub, _ := json.Marshal(usage)
		json.Unmarshal(ub, &ev.Usage)
		events = append(events, ev)
	}

	// Parse messages (Anthropic block format or OpenAI format)
	for _, raw := range messages {
		msg, ok := raw.(map[string]interface{})
		if !ok {
			continue
		}
		role, _ := msg["role"].(string)
		// Bug 3 fix: extract timestamp
		ts, _ := msg["timestamp"].(string)

		content := msg["content"]
		switch c := content.(type) {
		case string:
			events = append(events, Event{
				Role: role, Content: c, Timestamp: ts,
				SourceTool: "opencode",
			})
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
					events = append(events, Event{
						Role: role, Content: text, Timestamp: ts,
						SourceTool: "opencode",
					})
				case "thinking":
					think, _ := b["thinking"].(string)
					redacted, _ := b["redacted"].(bool)
					events = append(events, Event{
						Role:      "assistant",
						Timestamp: ts,
						Reasoning: think,
						Redacted:  redacted,
						SourceTool: "opencode",
					})
				case "tool_use":
					id, _ := b["id"].(string)
					name, _ := b["name"].(string)
					events = append(events, Event{
						Role: "assistant", Timestamp: ts,
						ToolCalls: []ToolCall{{ID: id, Name: name}},
						SourceTool: "opencode",
					})
				case "tool_result":
					tid, _ := b["tool_use_id"].(string)
					isErr, _ := b["is_error"].(bool)
					ct := extractToolResultContent(b)
					events = append(events, Event{
						Role:       "tool",
						Timestamp:  ts,
						ToolCallID: tid,
						Content:    ct,
						IsError:    isErr,
						SourceTool: "opencode",
					})
				}
			}
		default:
			// OpenAI-style tool_calls
			if tcs, ok := msg["tool_calls"].([]interface{}); ok {
				var tcList []ToolCall
				for _, tc := range tcs {
					if tcm, ok := tc.(map[string]interface{}); ok {
						tcItem := ToolCall{ID: str(tcm, "id")}
						if fn, ok := tcm["function"].(map[string]interface{}); ok {
							tcItem.Name = str(fn, "name")
						}
						tcList = append(tcList, tcItem)
					}
				}
				events = append(events, Event{
					Role: role, ToolCalls: tcList, Timestamp: ts,
					SourceTool: "opencode",
				})
			}
		}
	}

	// Tool results may also be in messages
	for _, raw := range messages {
		msg, ok := raw.(map[string]interface{})
		if !ok {
			continue
		}
		role, _ := msg["role"].(string)
		if role != "tool" {
			continue
		}
		// Bug 11 fix: tool content may be array
		content := extractToolResultContent(msg)
		if content == "" {
			content, _ = msg["content"].(string)
		}
		isErr, _ := msg["is_error"].(bool)
		ts, _ := msg["timestamp"].(string)

		events = append(events, Event{
			Role:       "tool",
			Content:    content,
			Timestamp:  ts,
			ToolCallID: str(msg, "tool_call_id"),
			IsError:    isErr,
			SourceTool: "opencode",
		})
	}

	return events, nil
}

// ── OpenClaw ──

func parseOpenClaw(doc map[string]interface{}) ([]Event, error) {
	// OpenClaw format: session_id + messages + model + platform, provider="openclaw"
	var events []Event
	var messages []interface{}
	model := "unknown"

	if msgs, ok := doc["messages"].([]interface{}); ok {
		messages = msgs
	}
	if m, ok := doc["model"].(string); ok {
		model = m
	}

	// Parse messages
	for _, raw := range messages {
		msg, ok := raw.(map[string]interface{})
		if !ok {
			continue
		}
		role, _ := msg["role"].(string)
		// Bug 3 fix: extract timestamp
		ts, _ := msg["timestamp"].(string)

		// Content can be string or array of blocks (Anthropic-style)
		content := msg["content"]
		switch c := content.(type) {
		case string:
			events = append(events, Event{
				Role:       role,
				Content:    c,
				Timestamp:  ts,
				SourceTool: "openclaw",
			})
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
					events = append(events, Event{
						Role:       role,
						Content:    text,
						Timestamp:  ts,
						SourceTool: "openclaw",
					})
				case "thinking":
					think, _ := b["thinking"].(string)
					redacted, _ := b["redacted"].(bool)
					events = append(events, Event{
						Role:       "assistant",
						Timestamp:  ts,
						Reasoning:  think,
						Redacted:   redacted,
						SourceTool: "openclaw",
					})
				case "tool_use":
					id, _ := b["id"].(string)
					name, _ := b["name"].(string)
					events = append(events, Event{
						Role: "assistant", Timestamp: ts,
						ToolCalls: []ToolCall{{ID: id, Name: name}},
						SourceTool: "openclaw",
					})
				case "tool_result":
					tid, _ := b["tool_use_id"].(string)
					isErr, _ := b["is_error"].(bool)
					ct := extractToolResultContent(b)
					events = append(events, Event{
						Role:       "tool",
						Timestamp:  ts,
						ToolCallID: tid,
						Content:    ct,
						IsError:    isErr,
						SourceTool: "openclaw",
					})
				}
			}
		default:
			// OpenAI-style tool_calls
			if tcs, ok := msg["tool_calls"].([]interface{}); ok {
				var tcList []ToolCall
				for _, tc := range tcs {
					if tcm, ok := tc.(map[string]interface{}); ok {
						tcItem := ToolCall{ID: str(tcm, "id")}
						if fn, ok := tcm["function"].(map[string]interface{}); ok {
							tcItem.Name = str(fn, "name")
						}
						tcList = append(tcList, tcItem)
					}
				}
				events = append(events, Event{
					Role:       role,
					Timestamp:  ts,
					ToolCalls:  tcList,
					SourceTool: "openclaw",
				})
			}
		}
	}

	// Handle tool role messages (OpenAI-style flat format)
	for _, raw := range messages {
		msg, ok := raw.(map[string]interface{})
		if !ok {
			continue
		}
		role, _ := msg["role"].(string)
		if role != "tool" {
			continue
		}
		// Bug 11 fix: tool content may be array
		content := extractToolResultContent(msg)
		if content == "" {
			content, _ = msg["content"].(string)
		}
		isErr, _ := msg["is_error"].(bool)
		ts, _ := msg["timestamp"].(string)

		events = append(events, Event{
			Role:       "tool",
			Content:    content,
			Timestamp:  ts,
			ToolCallID: str(msg, "tool_call_id"),
			IsError:    isErr,
			SourceTool: "openclaw",
		})
	}

	// Add model info as meta event
	if model != "unknown" {
		events = append([]Event{{
			Role:       "meta",
			ModelUsed:  model,
			SourceTool: "openclaw",
		}}, events...)
	}

	return events, nil
}

// ── Generic fallback ──

func parseGeneric(raw string) ([]Event, error) {
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
		ev.SourceTool = "generic"
		events = append(events, ev)
	}
	if len(events) > 0 {
		return events, nil
	}

	// Single JSON array
	var arr []Event
	if err := json.Unmarshal([]byte(raw), &arr); err == nil {
		for i := range arr {
			arr[i].SourceTool = "generic"
		}
		return arr, nil
	}

	// Bug 7 fix: return error instead of silently returning empty events
	return nil, fmt.Errorf("generic: unable to parse as JSONL or JSON array")
}

// ═══════════════════════════════════════════════════════════════
// ANALYSIS ENGINE
// ═══════════════════════════════════════════════════════════════

func Analyze(events []Event, model string) Metrics {
	m := Metrics{
		ModelUsed: model,
		ToolUsage: make(map[string]int),
	}

	pricing := LookupPrice(model)
	hasMetaUsage := false

	for _, ev := range events {
		// Track source tool from first non-meta event
		if m.SourceTool == "" && ev.SourceTool != "" && ev.Role != "meta" && ev.Role != "session_meta" {
			m.SourceTool = ev.SourceTool
		}

		// Timestamp
		if ts := parseTS(ev.Timestamp); !ts.IsZero() {
			m.Timestamps = append(m.Timestamps, ts)
		}

		switch ev.Role {
		case "session_meta", "meta":
			if ev.Usage != nil {
				m.TokensInput += ev.Usage["input_tokens"]
				m.TokensOutput += ev.Usage["output_tokens"]
				m.TokensCacheW += ev.Usage["cache_creation_input_tokens"]
				m.TokensCacheR += ev.Usage["cache_read_input_tokens"]
				hasMetaUsage = true
			}
			continue

		case "user":
			m.UserMessages++
			if ev.Content != "" && !hasMetaUsage {
				m.TokensInput += max(1, len(ev.Content)/4)
			}

		case "assistant":
			m.AssistantTurns++

			// Reasoning
			if ev.Reasoning != "" {
				m.ReasoningBlocks++
				rc := len(ev.Reasoning)
				m.ReasoningChars += rc
				m.ReasoningLens = append(m.ReasoningLens, rc)
				if ev.Redacted {
					m.ReasoningRedact++
				}
				if !hasMetaUsage {
					m.TokensOutput += max(1, rc/4)
				}
			}

			// Text content
			if ev.Content != "" && !hasMetaUsage {
				m.TokensOutput += max(1, len(ev.Content)/4)
			}

			// Tool calls
			m.ToolCallsTotal += len(ev.ToolCalls)
			for _, tc := range ev.ToolCalls {
				name := tc.Name
				if name == "" {
					name = "unknown"
				}
				m.ToolUsage[name]++
			}

		case "tool":
			m.ToolResults++
			isErr := ev.IsError
			if !isErr && ev.Content != "" {
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

	// Normalize: ToolCallsOK must not exceed ToolCallsTotal
	// (some parsers may count tool results differently from requests)
	if m.ToolCallsOK > m.ToolCallsTotal-m.ToolCallsFail {
		m.ToolCallsOK = max(0, m.ToolCallsTotal-m.ToolCallsFail)
	}

	m.CostEstimated = round4(
		float64(m.TokensInput)/1e6*pricing.Input +
			float64(m.TokensOutput)/1e6*pricing.Output +
			float64(m.TokensCacheW)/1e6*pricing.CW +
			float64(m.TokensCacheR)/1e6*pricing.CR,
	)

	return m
}

// ═══════════════════════════════════════════════════════════════
// ANOMALY DETECTION
// ═══════════════════════════════════════════════════════════════

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
				Type: "hanging", Severity: i18n.T("severity_high"), Emoji: "🔴",
				Detail: fmt.Sprintf(i18n.T("anomaly_hanging_detail"), longGaps, maxGap),
			})
		} else if longGaps > 0 {
			a = append(a, Anomaly{
				Type: "hanging", Severity: i18n.T("severity_medium"), Emoji: "🟡",
				Detail: fmt.Sprintf(i18n.T("anomaly_hanging_detail"), longGaps, maxGap),
			})
		} else if percentile(sl, 0.95) > 30 {
			a = append(a, Anomaly{
				Type: "latency", Severity: i18n.T("severity_low"), Emoji: "🟢",
				Detail: fmt.Sprintf(i18n.T("anomaly_latency_detail"), percentile(sl, 0.95)),
			})
		}
	}

	totalTools := m.ToolCallsOK + m.ToolCallsFail
	if totalTools > 0 {
		failRate := float64(m.ToolCallsFail) / float64(totalTools)
		if failRate > 0.30 {
			a = append(a, Anomaly{
				Type: "tool_failures", Severity: i18n.T("severity_high"), Emoji: "🔴",
				Detail: fmt.Sprintf(i18n.T("anomaly_tool_fail_detail"), m.ToolCallsFail, totalTools, failRate*100),
			})
		} else if failRate > 0.15 {
			a = append(a, Anomaly{
				Type: "tool_failures", Severity: i18n.T("severity_medium"), Emoji: "🟡",
				Detail: fmt.Sprintf(i18n.T("anomaly_tool_fail_detail"), m.ToolCallsFail, totalTools, failRate*100),
			})
		}
	}

	if len(m.ReasoningLens) > 0 {
		avgReason := float64(m.ReasoningChars) / float64(m.ReasoningBlocks)
		if avgReason < 200 {
			a = append(a, Anomaly{
				Type: "shallow_thinking", Severity: i18n.T("severity_high"), Emoji: "🔴",
				Detail: fmt.Sprintf(i18n.T("anomaly_shallow_detail"), avgReason),
			})
		} else if avgReason < 500 {
			a = append(a, Anomaly{
				Type: "shallow_thinking", Severity: i18n.T("severity_medium"), Emoji: "🟡",
				Detail: fmt.Sprintf(i18n.T("anomaly_shallow_medium_detail"), avgReason),
			})
		}
	}

	if m.ReasoningRedact > 0 {
		a = append(a, Anomaly{
			Type: "redaction", Severity: i18n.T("severity_medium"), Emoji: "🟡",
			Detail: fmt.Sprintf(i18n.T("anomaly_redaction_detail"), m.ReasoningRedact),
		})
	}

	if m.ToolCallsTotal == 0 && m.AssistantTurns > 2 {
		a = append(a, Anomaly{
			Type: "no_tools", Severity: i18n.T("severity_low"), Emoji: "🟢",
			Detail: i18n.T("anomaly_no_tools_detail"),
		})
	}

	return a
}

// ═══════════════════════════════════════════════════════════════
// HEALTH SCORE
// ═══════════════════════════════════════════════════════════════

func HealthScore(m Metrics, anoms []Anomaly) int {
	score := 100
	for _, a := range anoms {
		sev := a.Severity
		switch {
		case sev == i18n.T("severity_high"):
			score -= 30
		case sev == i18n.T("severity_medium"):
			score -= 12
		case sev == i18n.T("severity_low"):
			score -= 4
		}
	}
	if score < 0 {
		score = 0
	}
	if score > 100 {
		score = 100
	}
	return score
}

// ═══════════════════════════════════════════════════════════════
// SESSION LOADING
// ═══════════════════════════════════════════════════════════════

func LoadSession(path string) (*Session, error) {
	name := strings.TrimSuffix(filepath.Base(path), filepath.Ext(path))
	events, err := Parse(path)
	if err != nil {
		return nil, err
	}
	if len(events) == 0 {
		return nil, fmt.Errorf("no parseable events in %s", path)
	}

	// Determine model from events
	model := "default"
	for _, ev := range events {
		if ev.ModelUsed != "" && ev.ModelUsed != "unknown" {
			model = ev.ModelUsed
			break
		}
	}

	m := Analyze(events, model)
	a := DetectAnomalies(m)
	h := HealthScore(m, a)
	loopResult := AnalyzeLoops(events)

	// v0.2: community-driven diagnostics
	loops := DetectFingerprintLoops(events)
	latencies := AnalyzeToolLatency(events)
	ctxUtil := AnalyzeContextUtilization(events, 0) // MCP tool count from user config
	largeParams := DetectLargeParams(events)
	unusedTools := DetectUnusedTools(events)
	stuckExtra := DetectStuckPatternsEnhanced(events)

	toolWarnings := ValidateToolPatterns(events)

	return &Session{
		Name: name, Path: path,
		Metrics:           m,
		Anomalies:         a,
		Health:            h,
		LoopCost:          LoopCost{TotalLoopCost: loopResult.LoopCost},
		LoopFingerprints:  loops,
		ToolLatencies:     latencies,
		ContextUtil:       ctxUtil,
		LargeParams:       largeParams,
		UnusedTools:       unusedTools,
		StuckPatternsExtra: stuckExtra,
		LoopResultData:    loopResult,
		ToolWarnings:      toolWarnings,
	}, nil
}

func LoadAll(dir string) []Session {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil
	}
	var sessions []Session
	for _, e := range entries {
		if e.IsDir() {
			continue
		}
		name := e.Name()
		if !strings.HasSuffix(name, ".jsonl") && !strings.HasSuffix(name, ".json") {
			continue
		}
		// Skip non-session files
		if strings.HasPrefix(name, "request_dump_") || name == "sessions.json" {
			continue
		}
		path := filepath.Join(dir, name)
		s, err := LoadSession(path)
		if err != nil {
			continue
		}
		sessions = append(sessions, *s)
	}
	sort.Slice(sessions, func(i, j int) bool {
		return sessions[i].Metrics.SessionStart > sessions[j].Metrics.SessionStart
	})
	return sessions
}

func FindSessionFiles(dir string) []string {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil
	}
	type entryInfo struct {
		path string
		t    time.Time
	}
	var items []entryInfo
	for _, e := range entries {
		if e.IsDir() {
			continue
		}
		name := e.Name()
		if !strings.HasSuffix(name, ".jsonl") && !strings.HasSuffix(name, ".json") {
			continue
		}
		if strings.HasPrefix(name, "request_dump_") || name == "sessions.json" {
			continue
		}
		info, err := e.Info()
		mt := time.Time{}
		if err == nil {
			mt = info.ModTime()
		}
		items = append(items, entryInfo{path: filepath.Join(dir, name), t: mt})
	}
	sort.Slice(items, func(i, j int) bool {
		return items[i].t.After(items[j].t)
	})
	files := make([]string, len(items))
	for i, it := range items {
		files[i] = it.path
	}
	return files
}

// ═══════════════════════════════════════════════════════════════
// UTILITIES
// ═══════════════════════════════════════════════════════════════

func str(m map[string]interface{}, key string) string {
	if m == nil {
		return ""
	}
	if v, ok := m[key].(string); ok {
		return v
	}
	return ""
}

// extractToolResultContent handles tool_result content that may be string, array of blocks, or other types.
func extractToolResultContent(b map[string]interface{}) string {
	switch v := b["content"].(type) {
	case string:
		return v
	case []interface{}:
		// Take the text from the first text block
		for _, item := range v {
			if m, ok := item.(map[string]interface{}); ok {
				if t, _ := m["type"].(string); t == "text" {
					if ct, _ := m["text"].(string); ct != "" {
						return ct
					}
				}
			}
		}
		// Fallback: try to serialize
		if jb, err := json.Marshal(v); err == nil {
			return string(jb)
		}
		return ""
	default:
		if v != nil {
			if jb, err := json.Marshal(v); err == nil {
				return string(jb)
			}
		}
		return ""
	}
}

func parseTS(raw string) time.Time {
	if raw == "" {
		return time.Time{}
	}
	s := strings.ReplaceAll(raw, "Z", "+00:00")
	if t, err := time.Parse(time.RFC3339Nano, s); err == nil {
		return t
	}
	if t, err := time.Parse("2006-01-02T15:04:05", s); err == nil {
		return t.UTC()
	}
	return time.Time{}
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

func FmtDuration(sec float64) string {
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

// ═══════════════════════════════════════════════════════════════
// 1. 智能修复建议 (Tier 0)
// ═══════════════════════════════════════════════════════════════

// FixSuggestion 修复建议
type FixSuggestion struct {
	Title       string // 标题 e.g. "添加工具超时"
	Description string // 描述 e.g. "检测到 %d 个间隔 >60s"
	Action      string // 可操作的建议 e.g. "为长时间运行的工具添加 timeout 参数"
	Severity    string // "high"/"medium"/"low"
	Category    string // "hanging"/"tool_failure"/"thinking"/"redaction"/"no_tools"
}

// GenerateFixes 根据异常和指标生成智能修复建议列表。
// lang 参数: "zh" 返回中文建议, "en" 返回英文建议。
func GenerateFixes(m Metrics, anomalies []Anomaly, lang string) []FixSuggestion {
	var fixes []FixSuggestion
	totalTools := m.ToolCallsOK + m.ToolCallsFail
	var failRate float64
	if totalTools > 0 {
		failRate = float64(m.ToolCallsFail) / float64(totalTools)
	}

	for _, a := range anomalies {
		switch a.Type {
		case "hanging":
			if lang == "zh" {
				fixes = append(fixes, FixSuggestion{
					Title:       "添加工具超时保护",
					Description: fmt.Sprintf("检测到 %d 个间隔 >60s, 最长=%.0fs", len(m.GapsSec), maxGap(m.GapsSec)),
					Action:      "为工具调用添加 timeout 并限制重试次数",
					Severity:    a.Severity,
					Category:    "hanging",
				})
			} else {
				fixes = append(fixes, FixSuggestion{
					Title:       "Add Tool Timeout Protection",
					Description: fmt.Sprintf("Detected %d gaps >60s, max=%.0fs", len(m.GapsSec), maxGap(m.GapsSec)),
					Action:      "Add timeout to tool calls and limit retry attempts",
					Severity:    a.Severity,
					Category:    "hanging",
				})
			}

		case "tool_failures":
			if failRate > 0.30 {
				if lang == "zh" {
					fixes = append(fixes, FixSuggestion{
						Title:       "检查工具 Schema",
						Description: fmt.Sprintf("工具失败率 %.0f%% (%d/%d)", failRate*100, m.ToolCallsFail, totalTools),
						Action:      "验证工具参数格式，确保 LLM 传入正确类型的参数",
						Severity:    a.Severity,
						Category:    "tool_failure",
					})
				} else {
					fixes = append(fixes, FixSuggestion{
						Title:       "Check Tool Schema",
						Description: fmt.Sprintf("Tool failure rate %.0f%% (%d/%d)", failRate*100, m.ToolCallsFail, totalTools),
						Action:      "Validate tool parameter formats, ensure LLM passes correct argument types",
						Severity:    a.Severity,
						Category:    "tool_failure",
					})
				}
			} else if failRate > 0.15 {
				if lang == "zh" {
					fixes = append(fixes, FixSuggestion{
						Title:       "优化工具描述",
						Description: fmt.Sprintf("工具失败率 %.0f%% (%d/%d)", failRate*100, m.ToolCallsFail, totalTools),
						Action:      "在 tool description 中提供更精确的参数示例和约束",
						Severity:    a.Severity,
						Category:    "tool_failure",
					})
				} else {
					fixes = append(fixes, FixSuggestion{
						Title:       "Improve Tool Descriptions",
						Description: fmt.Sprintf("Tool failure rate %.0f%% (%d/%d)", failRate*100, m.ToolCallsFail, totalTools),
						Action:      "Provide more precise parameter examples and constraints in tool descriptions",
						Severity:    a.Severity,
						Category:    "tool_failure",
					})
				}
			}

		case "shallow_thinking":
			if lang == "zh" {
				fixes = append(fixes, FixSuggestion{
					Title:       "增加推理深度",
					Description: fmt.Sprintf("平均推理仅 %.0f 字符", safeAvgReason(m)),
					Action:      "在 system prompt 中添加 '请一步步思考' 或增加 max_tokens",
					Severity:    a.Severity,
					Category:    "thinking",
				})
			} else {
				fixes = append(fixes, FixSuggestion{
					Title:       "Increase Reasoning Depth",
					Description: fmt.Sprintf("Average reasoning only %.0f chars", safeAvgReason(m)),
					Action:      "Add 'Think step by step' to system prompt or increase max_tokens",
					Severity:    a.Severity,
					Category:    "thinking",
				})
			}

		case "redaction":
			if lang == "zh" {
				fixes = append(fixes, FixSuggestion{
					Title:       "检查脱敏配置",
					Description: fmt.Sprintf("发现 %d 个思维块被脱敏", m.ReasoningRedact),
					Action:      "推理内容被脱敏，检查 hermes config 中的 redact 设置",
					Severity:    a.Severity,
					Category:    "redaction",
				})
			} else {
				fixes = append(fixes, FixSuggestion{
					Title:       "Check Redaction Config",
					Description: fmt.Sprintf("Found %d redacted thinking blocks", m.ReasoningRedact),
					Action:      "Reasoning content is redacted, check the redact setting in hermes config",
					Severity:    a.Severity,
					Category:    "redaction",
				})
			}

		case "no_tools":
			if lang == "zh" {
				fixes = append(fixes, FixSuggestion{
					Title:       "启用工具调用",
					Description: fmt.Sprintf("共 %d 轮对话，零工具调用", m.AssistantTurns),
					Action:      "当前为纯对话模式，考虑为 agent 配置工具以提升效率",
					Severity:    a.Severity,
					Category:    "no_tools",
				})
			} else {
				fixes = append(fixes, FixSuggestion{
					Title:       "Enable Tool Calling",
					Description: fmt.Sprintf("%d turns with zero tool calls", m.AssistantTurns),
					Action:      "Currently in chat-only mode, consider configuring tools for the agent",
					Severity:    a.Severity,
					Category:    "no_tools",
				})
			}
		}
	}

	return fixes
}

// maxGap 返回 gaps 列表中的最大值
func maxGap(gaps []float64) float64 {
	if len(gaps) == 0 {
		return 0
	}
	m := gaps[0]
	for _, g := range gaps[1:] {
		if g > m {
			m = g
		}
	}
	return m
}

// safeAvgReason 安全计算平均推理字符数
func safeAvgReason(m Metrics) float64 {
	if m.ReasoningBlocks == 0 {
		return 0
	}
	return float64(m.ReasoningChars) / float64(m.ReasoningBlocks)
}

// ═══════════════════════════════════════════════════════════════
// 2. Session Diff (Tier 0)
// ═══════════════════════════════════════════════════════════════

// DiffEntry 单条差异
type DiffEntry struct {
	Field  string // 字段名
	ValueA string // 会话A的值
	ValueB string // 会话B的值
	Delta  string // 变化方向 "↑"/"↓"/"→"
	Better string // "A"/"B"/"same" 哪个更好
}

// SessionDiff 会话差异报告
type SessionDiff struct {
	SessionA string
	SessionB string
	Entries  []DiffEntry
	Summary  string // 一句话总结
}

// DiffSessions 逐字段对比两个会话并生成差异报告。
func DiffSessions(a, b Session) SessionDiff {
	diff := SessionDiff{
		SessionA: a.Name,
		SessionB: b.Name,
	}

	// turns (少更好, 假设少=高效但需结合成功率)
	diff.Entries = append(diff.Entries, compareInt("turns", a.Metrics.AssistantTurns, b.Metrics.AssistantTurns, "lower"))

	// tools (多可能好=充分利用工具)
	diff.Entries = append(diff.Entries, compareInt("tools", a.Metrics.ToolCallsTotal, b.Metrics.ToolCallsTotal, "higher"))

	// success_rate (高更好)
	srA := successRateVal(a.Metrics)
	srB := successRateVal(b.Metrics)
	diff.Entries = append(diff.Entries, compareFloat("success_rate", srA, srB, "higher"))

	// fail_count (少更好)
	diff.Entries = append(diff.Entries, compareInt("fail_count", a.Metrics.ToolCallsFail, b.Metrics.ToolCallsFail, "lower"))

	// cost (低更好)
	diff.Entries = append(diff.Entries, compareFloat("cost", a.Metrics.CostEstimated, b.Metrics.CostEstimated, "lower"))

	// health (高更好)
	diff.Entries = append(diff.Entries, compareInt("health", a.Health, b.Health, "higher"))

	// duration (短更好但需看产出)
	diff.Entries = append(diff.Entries, compareFloat("duration", a.Metrics.DurationSec, b.Metrics.DurationSec, "lower"))

	// model (不同=降级/升级)
	diff.Entries = append(diff.Entries, DiffEntry{
		Field:  "model",
		ValueA: a.Metrics.ModelUsed,
		ValueB: b.Metrics.ModelUsed,
		Delta:  deltaStr(a.Metrics.ModelUsed, b.Metrics.ModelUsed),
		Better: "same",
	})

	// anomaly_count (少更好)
	diff.Entries = append(diff.Entries, compareInt("anomaly_count", len(a.Anomalies), len(b.Anomalies), "lower"))

	// 构建 summary
	diff.Summary = buildDiffSummary(a, b)

	return diff
}

// compareInt 比较整数字段并返回 DiffEntry
func compareInt(field string, va, vb int, prefer string) DiffEntry {
	entry := DiffEntry{
		Field:  field,
		ValueA: fmt.Sprintf("%d", va),
		ValueB: fmt.Sprintf("%d", vb),
	}
	if va > vb {
		entry.Delta = "↓"
		if prefer == "lower" {
			entry.Better = "B"
		} else {
			entry.Better = "A"
		}
	} else if va < vb {
		entry.Delta = "↑"
		if prefer == "lower" {
			entry.Better = "A"
		} else {
			entry.Better = "B"
		}
	} else {
		entry.Delta = "→"
		entry.Better = "same"
	}
	return entry
}

// compareFloat 比较浮点字段并返回 DiffEntry
func compareFloat(field string, va, vb float64, prefer string) DiffEntry {
	entry := DiffEntry{
		Field:  field,
		ValueA: fmt.Sprintf("%.4f", va),
		ValueB: fmt.Sprintf("%.4f", vb),
	}
	if va > vb {
		entry.Delta = "↓"
		if prefer == "lower" {
			entry.Better = "B"
		} else {
			entry.Better = "A"
		}
	} else if va < vb {
		entry.Delta = "↑"
		if prefer == "lower" {
			entry.Better = "A"
		} else {
			entry.Better = "B"
		}
	} else {
		entry.Delta = "→"
		entry.Better = "same"
	}
	return entry
}

// deltaStr 比较字符串并返回变化方向字符串
func deltaStr(a, b string) string {
	if a != b {
		return a + " → " + b
	}
	return a
}

// successRateVal 计算工具调用成功率
func successRateVal(m Metrics) float64 {
	total := m.ToolCallsOK + m.ToolCallsFail
	if total == 0 {
		return 0
	}
	return float64(m.ToolCallsOK) / float64(total) * 100
}

// buildDiffSummary 构建人类可读的差异总结
func buildDiffSummary(a, b Session) string {
	healthDelta := b.Health - a.Health
	costDelta := b.Metrics.CostEstimated - a.Metrics.CostEstimated

	parts := []string{}
	parts = append(parts, fmt.Sprintf("Session %s vs %s", b.Name, a.Name))

	if healthDelta > 0 {
		parts = append(parts, fmt.Sprintf("health +%d", healthDelta))
	} else if healthDelta < 0 {
		parts = append(parts, fmt.Sprintf("health %d", healthDelta))
	} else {
		parts = append(parts, "health unchanged")
	}

	if costDelta > 0.0001 {
		parts = append(parts, fmt.Sprintf("cost +$%.4f", costDelta))
	} else if costDelta < -0.0001 {
		parts = append(parts, fmt.Sprintf("cost -$%.4f", -costDelta))
	}

	return strings.Join(parts, ", ")
}

// ═══════════════════════════════════════════════════════════════
// 3. 成本异常预测 (Tier 0)
// ═══════════════════════════════════════════════════════════════

// CostAlert 成本预警
type CostAlert struct {
	Triggered bool
	Level     string  // "critical"/"warning"/"info"
	Message   string  // 预警消息
	Current   float64 // 当前值
	Baseline  float64 // 基线值
	Ratio     float64 // 比值
}

// PredictCostAnomaly 预测当前会话是否存在成本异常。
// sessions: 历史会话列表（用于计算基线），current: 当前会话。
func PredictCostAnomaly(sessions []Session, current Session) CostAlert {
	if len(sessions) == 0 {
		return CostAlert{Triggered: false, Level: "info", Message: "无历史数据用于比较"}
	}

	// 计算所有 session 的平均 cost/turn
	var totalCostTurn float64
	var totalLoopTurn float64
	var count int
	for _, s := range sessions {
		if s.Metrics.AssistantTurns > 0 {
			totalCostTurn += s.Metrics.CostEstimated / float64(s.Metrics.AssistantTurns)
			totalLoopTurn += s.LoopCost.TotalLoopCost / float64(s.Metrics.AssistantTurns)
			count++
		}
	}
	if count == 0 {
		return CostAlert{Triggered: false, Level: "info", Message: "无有效历史数据"}
	}

	avgCostTurn := totalCostTurn / float64(count)

	// 当前 session 的 cost/turn
	var curCostTurn float64
	if current.Metrics.AssistantTurns > 0 {
		curCostTurn = current.Metrics.CostEstimated / float64(current.Metrics.AssistantTurns)
	}

	var curLoopTurn float64
	if current.Metrics.AssistantTurns > 0 {
		curLoopTurn = current.LoopCost.TotalLoopCost / float64(current.Metrics.AssistantTurns)
	}

	alert := CostAlert{
		Current:  curCostTurn,
		Baseline: avgCostTurn,
	}

	if avgCostTurn > 0 {
		alert.Ratio = curCostTurn / avgCostTurn
	}

	// 判断预警级别
	if curCostTurn > avgCostTurn*3 {
		alert.Triggered = true
		alert.Level = "critical"
		alert.Message = fmt.Sprintf("本会话单轮成本($%.4f)是平均($%.4f)的%.1f倍",
			curCostTurn, avgCostTurn, alert.Ratio)
	} else if curCostTurn > avgCostTurn*2 {
		alert.Triggered = true
		alert.Level = "warning"
		alert.Message = fmt.Sprintf("本会话单轮成本($%.4f)是平均($%.4f)的%.1f倍",
			curCostTurn, avgCostTurn, alert.Ratio)
	} else if current.Metrics.CostEstimated > 0 && current.LoopCost.TotalLoopCost/current.Metrics.CostEstimated > 0.50 {
		alert.Triggered = true
		alert.Level = "critical"
		alert.Message = fmt.Sprintf("循环成本($%.4f)占总成本($%.4f)的%.0f%%",
			current.LoopCost.TotalLoopCost, current.Metrics.CostEstimated,
			current.LoopCost.TotalLoopCost/current.Metrics.CostEstimated*100)
	} else if current.Metrics.CostEstimated > 0 && current.LoopCost.TotalLoopCost/current.Metrics.CostEstimated > 0.30 {
		alert.Triggered = true
		alert.Level = "warning"
		alert.Message = fmt.Sprintf("循环成本($%.4f)占总成本($%.4f)的%.0f%%",
			current.LoopCost.TotalLoopCost, current.Metrics.CostEstimated,
			current.LoopCost.TotalLoopCost/current.Metrics.CostEstimated*100)
	} else {
		alert.Triggered = false
		alert.Level = "info"
		alert.Message = fmt.Sprintf("单轮成本 $%.4f，在正常范围内 (平均 $%.4f)", curCostTurn, avgCostTurn)
	}

	// 同时检查 loop_cost 占比
	_ = curLoopTurn

	return alert
}

// ═══════════════════════════════════════════════════════════════
// 4. 健康趋势 + 退化检测 (Tier 1)
// ═══════════════════════════════════════════════════════════════

// TrendPoint 趋势数据点
type TrendPoint struct {
	Name   string
	Health int
	Cost   float64
}

// HealthTrend 健康趋势分析
type HealthTrend struct {
	Points     []TrendPoint // 最近10个session的趋势点
	Direction  string       // "up"/"down"/"stable"
	AvgHealth  float64
	Regressing bool   // 是否在退化
	Message    string // 趋势描述
}

// AnalyzeHealthTrend 分析最近 10 个 session 的健康趋势。
// 使用 3 点移动平均平滑数据，检测退化信号。
func AnalyzeHealthTrend(sessions []Session) HealthTrend {
	trend := HealthTrend{}

	if len(sessions) == 0 {
		trend.Message = "无可用会话数据"
		return trend
	}

	// 取最近 10 个 session
	n := len(sessions)
	if n > 10 {
		n = 10
	}
	recent := sessions[:n]

	// 构建趋势点
	for _, s := range recent {
		trend.Points = append(trend.Points, TrendPoint{
			Name:   s.Name,
			Health: s.Health,
			Cost:   s.Metrics.CostEstimated,
		})
	}

	// 3 点移动平均
	smoothed := make([]float64, len(trend.Points))
	for i := range trend.Points {
		sum := 0
		cnt := 0
		for j := i - 1; j <= i+1; j++ {
			if j >= 0 && j < len(trend.Points) {
				sum += trend.Points[j].Health
				cnt++
			}
		}
		smoothed[i] = float64(sum) / float64(cnt)
	}

	// 计算平均健康分
	sumHealth := 0.0
	for _, p := range trend.Points {
		sumHealth += float64(p.Health)
	}
	trend.AvgHealth = sumHealth / float64(len(trend.Points))

	// 判断方向
	if len(smoothed) >= 2 {
		first := smoothed[0]
		last := smoothed[len(smoothed)-1]
		diff := last - first
		if diff > 5 {
			trend.Direction = "up"
		} else if diff < -5 {
			trend.Direction = "down"
		} else {
			trend.Direction = "stable"
		}
	} else {
		trend.Direction = "stable"
	}

	// 检测退化: 最近 3 个点持续下降 + 最后 1 个 < 平均
	if len(smoothed) >= 3 {
		last3 := smoothed[len(smoothed)-3:]
		descending := true
		for i := 1; i < len(last3); i++ {
			if last3[i] >= last3[i-1] {
				descending = false
				break
			}
		}
		if descending && last3[len(last3)-1] < trend.AvgHealth {
			trend.Regressing = true
		}
	}

	// 构建 Message
	if trend.Regressing {
		last3Vals := []string{}
		startIdx := len(trend.Points) - 3
		if startIdx < 0 {
			startIdx = 0
		}
		for i := startIdx; i < len(trend.Points); i++ {
			last3Vals = append(last3Vals, fmt.Sprintf("%d", trend.Points[i].Health))
		}
		trend.Message = fmt.Sprintf("健康分持续下降: %s", strings.Join(last3Vals, "→"))
	} else {
		trend.Message = fmt.Sprintf("健康分稳定在 %.0f", trend.AvgHealth)
	}

	return trend
}

// ═══════════════════════════════════════════════════════════════
// 5. Tool Call 模式校验 (Tier 1)
// ═══════════════════════════════════════════════════════════════

// ToolWarning 工具使用警告
type ToolWarning struct {
	ToolName string
	Pattern  string // "dead_loop"/"empty_args"/"fail_retry_chain"/"redundant"
	Count    int    // 出现次数
	Detail   string // 详细描述
	Severity string
}

// ValidateToolPatterns 分析事件流中的工具调用模式，检测异常使用模式。
func ValidateToolPatterns(events []Event) []ToolWarning {
	var warnings []ToolWarning

	// dead_loop: 同一 tool 连续调用 5+ 次（不管参数）
	type consecKey struct {
		name string
	}
	var lastTool string
	consecutive := 0
	for _, ev := range events {
		if ev.Role == "assistant" && len(ev.ToolCalls) > 0 {
			for _, tc := range ev.ToolCalls {
				if tc.Name == lastTool {
					consecutive++
				} else {
					if consecutive >= 5 {
						warnings = append(warnings, ToolWarning{
							ToolName: lastTool,
							Pattern:  "dead_loop",
							Count:    consecutive,
							Detail:   fmt.Sprintf("工具 '%s' 连续调用 %d 次，可能存在死循环", lastTool, consecutive),
							Severity: "high",
						})
					}
					consecutive = 1
					lastTool = tc.Name
				}
			}
		}
	}
	// 检查最后一段
	if consecutive >= 5 && lastTool != "" {
		warnings = append(warnings, ToolWarning{
			ToolName: lastTool,
			Pattern:  "dead_loop",
			Count:    consecutive,
			Detail:   fmt.Sprintf("工具 '%s' 连续调用 %d 次，可能存在死循环", lastTool, consecutive),
			Severity: "high",
		})
	}

	// empty_args: tool call 时 content 为空或 "{}"（近似检测）
	emptyCounts := make(map[string]int)
	for _, ev := range events {
		if ev.Role == "assistant" && len(ev.ToolCalls) > 0 {
			if ev.Content == "" || ev.Content == "{}" {
				for _, tc := range ev.ToolCalls {
					emptyCounts[tc.Name]++
				}
			}
		}
	}
	for name, count := range emptyCounts {
		if count > 0 {
			warnings = append(warnings, ToolWarning{
				ToolName: name,
				Pattern:  "empty_args",
				Count:    count,
				Detail:   fmt.Sprintf("工具 '%s' 有 %d 次调用参数为空", name, count),
				Severity: "medium",
			})
		}
	}

	// fail_retry_chain: 失败后立即重试同一 tool 3+ 次
	type failRetry struct {
		lastFail string
		chain    int
		started  bool
	}
	var fr failRetry
	for _, ev := range events {
		if ev.Role == "tool" && ev.IsError {
			// 尝试找到关联的 tool call（通过 ToolCallID 或最近的 tool call）
			fr.lastFail = ""
			fr.chain = 0
			fr.started = true
		} else if ev.Role == "assistant" && len(ev.ToolCalls) > 0 && fr.started {
			for _, tc := range ev.ToolCalls {
				if tc.Name == fr.lastFail || fr.lastFail == "" {
					fr.lastFail = tc.Name
					fr.chain++
				} else {
					if fr.chain >= 3 {
						warnings = append(warnings, ToolWarning{
							ToolName: fr.lastFail,
							Pattern:  "fail_retry_chain",
							Count:    fr.chain,
							Detail:   fmt.Sprintf("工具 '%s' 失败后连续重试 %d 次", fr.lastFail, fr.chain),
							Severity: "high",
						})
					}
					fr.lastFail = tc.Name
					fr.chain = 1
				}
			}
		}
	}
	if fr.chain >= 3 && fr.lastFail != "" {
		warnings = append(warnings, ToolWarning{
			ToolName: fr.lastFail,
			Pattern:  "fail_retry_chain",
			Count:    fr.chain,
			Detail:   fmt.Sprintf("工具 '%s' 失败后连续重试 %d 次", fr.lastFail, fr.chain),
			Severity: "high",
		})
	}

	// redundant: 在不同轮次调用同一 tool 多次（如 read_file 同一文件多次）
	toolCallTurns := make(map[string][]int) // toolName → turn indices
	turnIdx := 0
	for _, ev := range events {
		if ev.Role == "user" {
			turnIdx++
		}
		if ev.Role == "assistant" && len(ev.ToolCalls) > 0 {
			for _, tc := range ev.ToolCalls {
				toolCallTurns[tc.Name] = append(toolCallTurns[tc.Name], turnIdx)
			}
		}
	}
	for name, turns := range toolCallTurns {
		if len(turns) >= 4 {
			// 检查是否在多个不同轮次中调用
			uniqueTurns := make(map[int]bool)
			for _, t := range turns {
				uniqueTurns[t] = true
			}
			if len(uniqueTurns) >= 3 {
				warnings = append(warnings, ToolWarning{
					ToolName: name,
					Pattern:  "redundant",
					Count:    len(turns),
					Detail:   fmt.Sprintf("工具 '%s' 在 %d 个轮次中被调用 %d 次，可能存在冗余调用", name, len(uniqueTurns), len(turns)),
					Severity: "low",
				})
			}
		}
	}

	return warnings
}

// ═══════════════════════════════════════════════════════════════
// 6. Prompt 变更影响分析 (Tier 1)
// ═══════════════════════════════════════════════════════════════

// PromptChange 一次 prompt 变更
type PromptChange struct {
	SessionName string
	Before      string  // 变更前摘要
	After       string  // 变更后摘要
	Impact      string  // 影响描述
	HealthDelta int
	CostDelta   float64
}

// PromptImpact 变更影响分析
type PromptImpact struct {
	Changes    []PromptChange
	Trend      string // "improving"/"worsening"/"mixed"
	Suggestion string
}

// AnalyzePromptImpact 分析同名 agent 的连续 session，评估 prompt 变更的影响。
// 名称前缀相同的 session 视为同一 agent 的多次运行。
func AnalyzePromptImpact(sessions []Session) PromptImpact {
	impact := PromptImpact{}

	if len(sessions) < 2 {
		impact.Trend = "stable"
		impact.Suggestion = "需要至少 2 个同名 session 才能分析趋势"
		return impact
	}

	// 按名称前缀分组（使用名称中的公共前缀，去掉数字/日期后缀）
	groups := groupByPrefix(sessions)

	// 分析每组内相邻 session 的变化
	var allDeltas []int
	for _, group := range groups {
		if len(group) < 2 {
			continue
		}
		// 按 SessionStart 排序以保证时间顺序
		sort.Slice(group, func(i, j int) bool {
			return group[i].Metrics.SessionStart < group[j].Metrics.SessionStart
		})

		for i := 1; i < len(group); i++ {
			prev := group[i-1]
			curr := group[i]
			healthDelta := curr.Health - prev.Health
			costDelta := curr.Metrics.CostEstimated - prev.Metrics.CostEstimated

			allDeltas = append(allDeltas, healthDelta)

			ch := PromptChange{
				SessionName: curr.Name,
				Before:      fmt.Sprintf("health=%d cost=$%.4f", prev.Health, prev.Metrics.CostEstimated),
				After:       fmt.Sprintf("health=%d cost=$%.4f", curr.Health, curr.Metrics.CostEstimated),
				HealthDelta: healthDelta,
				CostDelta:   costDelta,
			}

			if healthDelta > 0 && costDelta <= 0 {
				ch.Impact = "positive"
			} else if healthDelta < 0 && costDelta >= 0 {
				ch.Impact = "negative"
			} else if healthDelta > 0 {
				ch.Impact = "positive (higher cost)"
			} else if healthDelta < 0 {
				ch.Impact = "negative (lower cost)"
			} else {
				ch.Impact = "neutral"
			}

			impact.Changes = append(impact.Changes, ch)
		}
	}

	// 判断整体趋势
	if len(allDeltas) == 0 {
		impact.Trend = "stable"
		impact.Suggestion = "无足够数据判断趋势"
		return impact
	}

	improving := 0
	worsening := 0
	for _, d := range allDeltas {
		if d > 0 {
			improving++
		} else if d < 0 {
			worsening++
		}
	}

	if improving > worsening && worsening == 0 {
		impact.Trend = "improving"
		impact.Suggestion = "健康分稳步提升，prompt 优化方向正确，建议继续保持"
	} else if worsening > improving && improving == 0 {
		impact.Trend = "worsening"
		impact.Suggestion = "健康分持续下降，建议回滚最近的 prompt 变更并重新评估"
	} else {
		impact.Trend = "mixed"
		impact.Suggestion = "健康分波动较大，建议逐一排查每次变更的影响，找出正负因素"
	}

	return impact
}

// groupByPrefix 按名称的公共前缀对 sessions 分组。
// 提取规则：去掉尾部的数字、日期、UUID 之类后缀。
func groupByPrefix(sessions []Session) map[string][]Session {
	groups := make(map[string][]Session)
	for _, s := range sessions {
		prefix := extractPrefix(s.Name)
		groups[prefix] = append(groups[prefix], s)
	}
	return groups
}

// extractPrefix 提取 session 名称的公共前缀（去掉数字/日期后缀）。
func extractPrefix(name string) string {
	// 去掉常见的后缀模式：数字、日期、短横线、下划线 + 数字等
	name = strings.TrimRight(name, "0123456789-_")
	// 去掉尾部 .json/.jsonl 残留
	name = strings.TrimSuffix(name, ".json")
	name = strings.TrimSuffix(name, ".jsonl")
	// 如果去后缀后为空，返回原名
	if name == "" {
		return name
	}
	// 再去一次尾部数字
	name = strings.TrimRight(name, "0123456789-_")
	if name == "" {
		return name
	}
	return name
}



// ═══════════════════════════════════════════════════
// WASTE ANALYSIS (April 2026)
// ═══════════════════════════════════════════════════

type CacheEfficiency struct {
	CacheWriteTokens int
	CacheReadTokens  int
	TotalInputTokens int
	HitRate          float64
	WastedTokens     int
	WastedCost       float64
	Rating           string
	Suggestion       string
}

func AnalyzeCacheEfficiency(m Metrics) CacheEfficiency {
	ce := CacheEfficiency{
		CacheWriteTokens: m.TokensCacheW,
		CacheReadTokens:  m.TokensCacheR,
		TotalInputTokens: m.TokensInput,
	}
	if m.TokensInput > 0 {
		ce.HitRate = float64(m.TokensCacheR) / float64(m.TokensInput) * 100
	}
	// Input tokens that could have been cached but weren't
	ce.WastedTokens = m.TokensInput - m.TokensCacheR
	if ce.WastedTokens < 0 {
		ce.WastedTokens = 0
	}
	pricing := LookupPrice(m.ModelUsed)
	ce.WastedCost = round4(float64(ce.WastedTokens) / 1e6 * pricing.Input)
	switch {
	case ce.HitRate >= 80:
		ce.Rating = "excellent"
		ce.Suggestion = "cache utilization excellent - keep current prompt structure"
	case ce.HitRate >= 40:
		ce.Rating = "good"
		ce.Suggestion = "moderate cache hit - place static system instructions at prompt prefix"
	case m.TokensCacheW > 0:
		ce.Rating = "poor"
		ce.Suggestion = "low cache hit rate - enable prompt caching with static prefix content"
	default:
		ce.Rating = "none"
		ce.Suggestion = "caching not enabled - enable Anthropic prompt caching to save up to 90% on input cost"
	}
	return ce
}


// ToolBloatItem tracks per-tool usage metrics.
type ToolBloatItem struct {
	ToolName    string
	CallCount   int
	TotalCost   float64
	IsRedundant bool
}
type ToolBloatAnalysis struct {
	ToolsPerTurn   float64
	TotalToolCost  float64
	RedundantCalls int
	BloatScore     int
	BloatLevel     string
	Suggestion     string
	TopBloat       []ToolBloatItem
}
func AnalyzeToolBloat(m Metrics) ToolBloatAnalysis {
	tba := ToolBloatAnalysis{}
	if m.AssistantTurns > 0 {
		tba.ToolsPerTurn = float64(m.ToolCallsTotal) / float64(m.AssistantTurns)
	}
	avgCostPerTurn := 0.0
	if m.AssistantTurns > 0 && m.CostEstimated > 0 {
		avgCostPerTurn = m.CostEstimated / float64(m.AssistantTurns)
	}
	tba.TotalToolCost = avgCostPerTurn * float64(m.ToolCallsTotal)

	switch {
	case tba.ToolsPerTurn > 5:
		tba.BloatScore = 90; tba.BloatLevel = "severe"
		tba.Suggestion = "severe tool bloat: limit max tool calls per turn or split into smaller tasks"
	case tba.ToolsPerTurn > 3:
		tba.BloatScore = 65; tba.BloatLevel = "high"
		tba.Suggestion = "too many tool calls: check if simple tasks use over-complex agent orchestration"
	case tba.ToolsPerTurn > 1.5:
		tba.BloatScore = 35; tba.BloatLevel = "medium"
		tba.Suggestion = "moderate tool usage: watch for unnecessary tool call patterns"
	default:
		tba.BloatScore = 10; tba.BloatLevel = "low"
		tba.Suggestion = "tool usage is lean"
	}

	type kv struct { k string; v int }
	var sorted []kv
	for k, v := range m.ToolUsage {
		sorted = append(sorted, kv{k, v})
	}
	sort.Slice(sorted, func(i, j int) bool { return sorted[i].v > sorted[j].v })
	for i, item := range sorted {
		if i >= 5 { break }
		isRedundant := item.v > m.AssistantTurns && m.AssistantTurns > 0
		if isRedundant { tba.RedundantCalls += item.v - m.AssistantTurns }
		tba.TopBloat = append(tba.TopBloat, ToolBloatItem{
			ToolName: item.k, CallCount: item.v,
			TotalCost: avgCostPerTurn * float64(item.v),
			IsRedundant: isRedundant,
		})
	}
	return tba
}

type StuckPattern struct {
	Pattern     string
	Description string
	Severity    string
}

func DetectStuckPatterns(events []Event) []StuckPattern {
	var p []StuckPattern
	emptyStreak := 0
	for _, ev := range events {
		if ev.Role == "assistant" && ev.Content == "" && len(ev.ToolCalls) == 0 {
			emptyStreak++
		} else {
			emptyStreak = 0
		}
	}
	if emptyStreak >= 3 {
		p = append(p, StuckPattern{
			Pattern:     "empty_response",
			Severity:    "critical",
			Description: fmt.Sprintf("%d consecutive empty assistant responses — agent likely stuck", emptyStreak),
		})
	}
	return p
}

type WasteReport struct {
	Cache       CacheEfficiency
	Bloat       ToolBloatAnalysis
	Stuck       []StuckPattern
	LoopCost    float64
	LoopPercent float64
	WasteScore  int
	WasteLevel  string
	TotalWasted float64
	Summary     string
	TopActions  []string
}

func ComputeWasteReport(m Metrics, events []Event, loopResult LoopResult) WasteReport {
	wr := WasteReport{}
	wr.Cache = AnalyzeCacheEfficiency(m)
	wr.Bloat = AnalyzeToolBloat(m)
	wr.Stuck = DetectStuckPatterns(events)
	wr.LoopCost = loopResult.LoopCost
	if m.CostEstimated > 0 {
		wr.LoopPercent = wr.LoopCost / m.CostEstimated * 100
	}
	wr.TotalWasted = wr.Cache.WastedCost + wr.LoopCost
	if wr.Bloat.BloatScore > 50 {
		wr.TotalWasted += m.CostEstimated * 0.15
	}

	score := 0.0
	switch wr.Cache.Rating {
	case "none": score += 20
	case "poor": score += 15
	case "good": score += 5
	}
	score += float64(wr.Bloat.BloatScore) * 0.25
	score += wr.LoopPercent * 0.6
	if score > 30 { score = 30 }
	stuckScore := float64(len(wr.Stuck)) * 7.0
	for _, s := range wr.Stuck {
		if s.Severity == "critical" { stuckScore += 5 }
	}
	if stuckScore > 20 { stuckScore = 20 }
	score += stuckScore
	if m.TokensCacheR > 0 && m.TokensInput > 0 {
		if float64(m.TokensCacheR)/float64(m.TokensInput) < 0.3 { score += 6 }
	}
	wr.WasteScore = int(score)
	if wr.WasteScore > 100 { wr.WasteScore = 100 }
	if wr.WasteScore < 0 { wr.WasteScore = 0 }

	switch {
	case wr.WasteScore >= 70: wr.WasteLevel = "red"
	case wr.WasteScore >= 40: wr.WasteLevel = "orange"
	case wr.WasteScore >= 15: wr.WasteLevel = "yellow"
	default: wr.WasteLevel = "green"
	}

	switch wr.WasteLevel {
	case "green": wr.Summary = "efficient session - no significant waste"
	case "yellow": wr.Summary = fmt.Sprintf("minor waste - cache %.0f%% hit, room for optimization", wr.Cache.HitRate)
	case "orange": wr.Summary = fmt.Sprintf("wasting $%.2f: loops %.0f%%, tools %.1f/turn", wr.TotalWasted, wr.LoopPercent, wr.Bloat.ToolsPerTurn)
	case "red": wr.Summary = fmt.Sprintf("severe waste $%.2f: loops %.0f%%, %d stuck, no cache", wr.TotalWasted, wr.LoopPercent, len(wr.Stuck))
	}

	if wr.Cache.Rating == "none" || wr.Cache.Rating == "poor" {
		wr.TopActions = append(wr.TopActions, wr.Cache.Suggestion)
	}
	if wr.Bloat.BloatLevel == "severe" || wr.Bloat.BloatLevel == "high" {
		if len(wr.Bloat.TopBloat) > 0 {
			wr.TopActions = append(wr.TopActions,
				fmt.Sprintf("top tool %q called %dx - reduce or batch", wr.Bloat.TopBloat[0].ToolName, wr.Bloat.TopBloat[0].CallCount))
		} else { wr.TopActions = append(wr.TopActions, wr.Bloat.Suggestion) }
	}
	if wr.LoopPercent > 20 {
		wr.TopActions = append(wr.TopActions,
			fmt.Sprintf("loop waste $%.2f (%.0f%%) - add max retries limit", wr.LoopCost, wr.LoopPercent))
	}
	if len(wr.TopActions) > 3 { wr.TopActions = wr.TopActions[:3] }
	if len(wr.TopActions) == 0 { wr.TopActions = []string{"session running optimally"} }
	return wr
}

func WasteReportText(wr WasteReport) string {
	var b strings.Builder
	w := func(f string, args ...interface{}) { b.WriteString(fmt.Sprintf(f, args...) + "\n") }
	sep := strings.Repeat("━", 60)
	w(sep)
	w("  AGENTWASTE v%s - Waste Analysis", Version)
	w(sep)
	w("")
	w("  Score: %d/100 (%s)", wr.WasteScore, levelEmoji(wr.WasteLevel)+" "+wr.WasteLevel)
	w("  Wasted: $%.4f", wr.TotalWasted)
	w("  %s", wr.Summary)
	w("")
	w("  -- Cache --")
	w("  %s (%.0f%% hit, %d read / %d input)", wr.Cache.Rating, wr.Cache.HitRate, wr.Cache.CacheReadTokens, wr.Cache.TotalInputTokens)
	if wr.Cache.WastedCost > 0 { w("  Cache waste: $%.4f", wr.Cache.WastedCost) }
	w("  Suggestion: %s", wr.Cache.Suggestion)
	w("")
	w("  -- Tool Bloat --")
	w("  %s (%.1f tools/turn)", wr.Bloat.BloatLevel, wr.Bloat.ToolsPerTurn)
	for _, tb := range wr.Bloat.TopBloat {
		m := ""; if tb.IsRedundant { m = " *redundant" }
		w("    %-25s %3dx $%.3f%s", tb.ToolName, tb.CallCount, tb.TotalCost, m)
	}
	w("")
	w("  -- Stuck --")
	if len(wr.Stuck) == 0 { w("  none") } else {
		for _, s := range wr.Stuck { w("  [%s] %s", s.Severity, s.Description) }
	}
	w("")
	w("  -- Actions --")
	for i, a := range wr.TopActions { w("  %d. %s", i+1, a) }
	w("")
	w(sep)
	return b.String()
}

func levelEmoji(level string) string {
	switch level {
	case "green": return "🟢"
	case "yellow": return "🟡"
	case "orange": return "🟠"
	case "red": return "🔴"
	}
	return ""
}

// ═══════════════════════════════════════════════════════════════
// v0.2 COMMUNITY-DRIVEN DIAGNOSTICS
// ═══════════════════════════════════════════════════════════════

// ── 1. Fingerprint Loop Detection ──

type LoopFingerprint struct {
	ToolName   string
	ResultHash string
	Count      int
	FirstIndex int
	LastIndex  int
	Severity   string
	Detail     string
}

func DetectFingerprintLoops(events []Event) []LoopFingerprint {
	type fpKey struct {
		tool string
		hash string
	}
	var fingerprints []LoopFingerprint

	// Collect consecutive tool-result pairs
	type pair struct {
		tool   string
		rhash  string
		idx    int
	}
	var pairs []pair
	for i, ev := range events {
		if ev.Role == "assistant" && len(ev.ToolCalls) > 0 {
			for _, tc := range ev.ToolCalls {
				// Find matching tool result
				resultHash := ""
				for j := i + 1; j < len(events) && j < i+5; j++ {
					if events[j].Role == "tool" && events[j].ToolCallID == tc.ID {
						content := events[j].Content
						if len(content) > 200 {
							content = content[:200]
						}
						resultHash = fmt.Sprintf("%x", hashString(content))
						break
					}
				}
				pairs = append(pairs, pair{tc.Name, resultHash, i})
			}
		}
	}

	// Detect consecutive identical fingerprints
	if len(pairs) < 3 {
		return nil
	}

	currentFP := fpKey{pairs[0].tool, pairs[0].rhash}
	startIdx := pairs[0].idx
	count := 1

	for i := 1; i < len(pairs); i++ {
		thisFP := fpKey{pairs[i].tool, pairs[i].rhash}
		if thisFP == currentFP && currentFP.hash != "" {
			count++
		} else {
			if count >= 3 {
				sev := "high"
				if count >= 5 { sev = "critical" }
				fingerprints = append(fingerprints, LoopFingerprint{
					ToolName: currentFP.tool, ResultHash: currentFP.hash,
					Count: count, FirstIndex: startIdx, LastIndex: pairs[i-1].idx,
					Severity: sev,
					Detail:   fmt.Sprintf("tool '%s' returned same result %dx consecutively — no progress", currentFP.tool, count),
				})
			}
			currentFP = thisFP
			startIdx = pairs[i].idx
			count = 1
		}
	}
	if count >= 3 && currentFP.hash != "" {
		sev := "high"
		if count >= 5 { sev = "critical" }
		fingerprints = append(fingerprints, LoopFingerprint{
			ToolName: currentFP.tool, ResultHash: currentFP.hash,
			Count: count, FirstIndex: startIdx, LastIndex: pairs[len(pairs)-1].idx,
			Severity: sev,
			Detail:   fmt.Sprintf("tool '%s' returned same result %dx consecutively — no progress", currentFP.tool, count),
		})
	}

	return fingerprints
}

// hashString is a simple djb2 hash for fingerprinting.
func hashString(s string) uint32 {
	var h uint32 = 5381
	for _, c := range []byte(s) {
		h = ((h << 5) + h) + uint32(c)
	}
	return h
}

// ── 2. Per-Tool Latency Ranking ──

type ToolLatencyItem struct {
	ToolName string
	Count    int
	AvgSec   float64
	P95Sec   float64
	MaxSec   float64
	MinSec   float64
	Timeouts int
	IsSlow   bool // p95 > 30s
}

func AnalyzeToolLatency(events []Event) []ToolLatencyItem {
	type latRec struct {
		durations []float64
		timeouts  int
	}
	toolLats := make(map[string]*latRec)

	// Collect latencies: time from tool_use event to its tool_result
	for i, ev := range events {
		if ev.Role == "assistant" && len(ev.ToolCalls) > 0 && ev.Timestamp != "" {
			tsStart := parseTS(ev.Timestamp)
			if tsStart.IsZero() { continue }
			for _, tc := range ev.ToolCalls {
				// Find matching tool result
				for j := i + 1; j < len(events); j++ {
					if events[j].Role == "tool" && events[j].ToolCallID == tc.ID {
						tsEnd := parseTS(events[j].Timestamp)
						if !tsEnd.IsZero() {
							dur := tsEnd.Sub(tsStart).Seconds()
							if dur >= 0 && dur < 3600 { // filter outliers >1h
								rec := toolLats[tc.Name]
								if rec == nil {
									rec = &latRec{}
									toolLats[tc.Name] = rec
								}
								rec.durations = append(rec.durations, dur)
							}
						}
						break
					}
				}
				// If no matching tool result found, count as timeout
				found := false
				for j := i + 1; j < len(events); j++ {
					if events[j].Role == "tool" && events[j].ToolCallID == tc.ID {
						found = true; break
					}
				}
				if !found {
					rec := toolLats[tc.Name]
					if rec == nil { rec = &latRec{}; toolLats[tc.Name] = rec }
					rec.timeouts++
				}
			}
		}
	}

	var items []ToolLatencyItem
	for name, rec := range toolLats {
		if len(rec.durations) == 0 && rec.timeouts == 0 { continue }
		sort.Float64s(rec.durations)
		item := ToolLatencyItem{
			ToolName: name,
			Count:    len(rec.durations) + rec.timeouts,
			Timeouts: rec.timeouts,
		}
		if len(rec.durations) > 0 {
			sum := 0.0
			for _, d := range rec.durations { sum += d }
			item.AvgSec = sum / float64(len(rec.durations))
			item.MaxSec = rec.durations[len(rec.durations)-1]
			item.MinSec = rec.durations[0]
			item.P95Sec = percentile(rec.durations, 0.95)
		}
		item.IsSlow = item.P95Sec > 30 || item.MaxSec > 60 || item.Timeouts > 0
		items = append(items, item)
	}

	sort.Slice(items, func(i, j int) bool {
		// Sort slowest first
		return items[i].MaxSec > items[j].MaxSec
	})
	return items
}

// ── 3. Context Window Utilization ──

type ContextUtilization struct {
	EstimatedTotal    int
	ToolDefinitions   int
	ConversationHist  int
	SystemPrompt      int
	AvailableForTask  int
	UtilizationPct    float64
	RiskLevel         string // "critical"/"warning"/"good"
	Suggestion        string
}

func AnalyzeContextUtilization(events []Event, mcpToolCount int) ContextUtilization {
	cu := ContextUtilization{
		EstimatedTotal: 200000, // default for Claude
		SystemPrompt:   12000,  // typical system prompt
	}
	if mcpToolCount == 0 {
		mcpToolCount = 8 // typical default
	}
	// Estimate tool definitions: ~300 tokens per tool on average
	cu.ToolDefinitions = mcpToolCount * 300

	// Estimate conversation history from actual events
	totalContent := 0
	for _, ev := range events {
		totalContent += len(ev.Content)
		totalContent += len(ev.Reasoning)
	}
	cu.ConversationHist = totalContent / 2 // rough char→token

	cu.AvailableForTask = cu.EstimatedTotal - cu.ToolDefinitions - cu.ConversationHist - cu.SystemPrompt
	if cu.AvailableForTask < 0 { cu.AvailableForTask = 0 }

	if cu.EstimatedTotal > 0 {
		cu.UtilizationPct = float64(cu.ToolDefinitions+cu.ConversationHist+cu.SystemPrompt) / float64(cu.EstimatedTotal) * 100
	}

	switch {
	case cu.AvailableForTask < 20000:
		cu.RiskLevel = "critical"
		cu.Suggestion = "context nearly full — reduce MCP tools or compact conversation immediately"
	case cu.AvailableForTask < 50000:
		cu.RiskLevel = "warning"
		cu.Suggestion = "context filling up — consider compacting or removing unused tools"
	default:
		cu.RiskLevel = "good"
		cu.Suggestion = "plenty of context headroom"
	}
	return cu
}

// ── 4. Large Parameter Detection ──

type LargeParamCall struct {
	ToolName  string
	ParamSize int
	Timestamp string
	Risk      string // "high" (>50KB), "medium" (>10KB)
	Detail    string
}

func DetectLargeParams(events []Event) []LargeParamCall {
	var large []LargeParamCall
	for _, ev := range events {
		if ev.Role == "assistant" && len(ev.Content) > 0 {
			size := len(ev.Content)
			var risk string
			if size > 50000 {
				risk = "high"
			} else if size > 10000 {
				risk = "medium"
			} else {
				continue
			}
			toolNames := ""
			for i, tc := range ev.ToolCalls {
				if i > 0 { toolNames += "," }
				toolNames += tc.Name
			}
			if toolNames == "" { toolNames = "text_response" }
			large = append(large, LargeParamCall{
				ToolName: toolNames, ParamSize: size, Timestamp: ev.Timestamp,
				Risk: risk,
				Detail: fmt.Sprintf("%s call with %d chars — may cause timeout or hang", toolNames, size),
			})
		}
	}
	return large
}

// ── 5. Unused / Rare Tool Detection ──

type UnusedToolInfo struct {
	ToolName  string
	CallCount int
	Level     string // "unused"/"rare"/"normal"
	Detail    string
}

func DetectUnusedTools(events []Event) []UnusedToolInfo {
	usage := make(map[string]int)
	for _, ev := range events {
		for _, tc := range ev.ToolCalls {
			if tc.Name != "" { usage[tc.Name]++ }
		}
	}
	var info []UnusedToolInfo
	for name, count := range usage {
		level := "normal"
		detail := ""
		if count == 0 {
			level = "unused"
			detail = fmt.Sprintf("tool '%s' registered but never called — wasting context window", name)
		} else if count <= 2 {
			level = "rare"
			detail = fmt.Sprintf("tool '%s' called only %dx — consider removing to free context", name, count)
		}
		if level != "normal" {
			info = append(info, UnusedToolInfo{ToolName: name, CallCount: count, Level: level, Detail: detail})
		}
	}
	sort.Slice(info, func(i, j int) bool { return info[i].CallCount < info[j].CallCount })
	return info
}

// ── 6. Enhanced Stuck Detection (community patterns) ──

func DetectStuckPatternsEnhanced(events []Event) []StuckPattern {
	patterns := DetectStuckPatterns(events)

	// Pattern: repeated identical assistant responses (Ralph Wiggum drift)
	type contentKey struct {
		role    string
		content string
	}
	contentStreak := make(map[contentKey]int)
	for _, ev := range events {
		if ev.Content != "" && len(ev.Content) > 50 {
			ck := contentKey{ev.Role, ev.Content[:50]}
			contentStreak[ck]++
		}
	}
	for ck, cnt := range contentStreak {
		if cnt >= 4 && ck.role == "assistant" {
			patterns = append(patterns, StuckPattern{
				Pattern:     "repeated_response",
				Description: fmt.Sprintf("assistant repeated same response %dx (drift pattern)", cnt),
				Severity:    "warning",
			})
		}
	}

	// Pattern: tool call without corresponding result (zombie tool calls)
	hasResult := make(map[string]bool)
	for _, ev := range events {
		if ev.Role == "tool" && ev.ToolCallID != "" { hasResult[ev.ToolCallID] = true }
	}
	zombieCount := 0
	for _, ev := range events {
		if ev.Role == "assistant" {
			for _, tc := range ev.ToolCalls {
				if tc.ID != "" && !hasResult[tc.ID] { zombieCount++ }
			}
		}
	}
	if zombieCount > 0 {
		patterns = append(patterns, StuckPattern{
			Pattern:     "zombie_tool_calls",
			Description: fmt.Sprintf("%d tool call(s) with no response — may indicate hang/timeout", zombieCount),
			Severity:    "warning",
		})
	}

	return patterns
}

// ── 7. Cumulative Cost Summary (cross-session) ──

type CostSummary struct {
	TotalSessions   int
	TotalCost       float64
	TotalTokensIn   int
	TotalTokensOut  int
	TotalCacheRead  int
	TotalCacheWrite int
	AvgCostPerTurn  float64
	CostliestModel  string
	CostliestCost   float64
}

func ComputeCostSummary(sessions []Session) CostSummary {
	var cs CostSummary
	cs.TotalSessions = len(sessions)
	modelCosts := make(map[string]float64)
	totalTurns := 0

	for _, s := range sessions {
		m := s.Metrics
		cs.TotalCost += m.CostEstimated
		cs.TotalTokensIn += m.TokensInput
		cs.TotalTokensOut += m.TokensOutput
		cs.TotalCacheRead += m.TokensCacheR
		cs.TotalCacheWrite += m.TokensCacheW
		totalTurns += m.AssistantTurns

		modelCosts[m.ModelUsed] += m.CostEstimated
	}

	if totalTurns > 0 {
		cs.AvgCostPerTurn = cs.TotalCost / float64(totalTurns)
	}
	for model, cost := range modelCosts {
		if cost > cs.CostliestCost {
			cs.CostliestCost = cost
			cs.CostliestModel = model
		}
	}
	return cs
}
