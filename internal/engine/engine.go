// Package engine provides the core analysis engine for agenttrace v4.
// Pure Go. Supports 5 agent formats: Hermes Agent, Claude Code, Codex CLI, Gemini CLI, OpenCode.
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

const Version = "4.0.0"

// ── Pricing (USD per 1M tokens) ──

type Price struct {
	Input  float64
	Output float64
	CW     float64
	CR     float64
}

var Pricing = map[string]Price{
	"claude-opus-4":     {15.00, 75.00, 18.75, 1.50},
	"claude-opus-4.5":   {15.00, 75.00, 18.75, 1.50},
	"claude-sonnet-4":   {3.00, 15.00, 3.75, 0.30},
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

var ToolDisplayNames = map[string]string{
	"hermes_jsonl": "Hermes Agent (JSONL)",
	"hermes_json":  "Hermes Agent (.json)",
	"claude_code":  "Claude Code",
	"codex_cli":    "Codex CLI",
	"gemini_cli":   "Gemini CLI",
	"opencode":     "OpenCode",
	"generic":      "Generic JSON/JSONL",
}

// ── Normalized Event ──

type Event struct {
	Role        string
	Content     string
	Timestamp   string
	Reasoning   string
	Redacted    bool
	ToolCalls   []ToolCall
	ToolCallID  string
	IsError     bool
	Usage       map[string]int
	ModelUsed   string
	SourceTool  string // which tool produced this session
}

type ToolCall struct {
	ID   string
	Name string
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

	// JSONL: check first few lines
	firstLine := strings.SplitN(content, "\n", 2)[0]
	var firstLineObj map[string]interface{}
	if err := json.Unmarshal([]byte(firstLine), &firstLineObj); err == nil {
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

	msgs, _ := doc["messages"].([]interface{})
	for _, raw := range msgs {
		msg, ok := raw.(map[string]interface{})
		if !ok {
			continue
		}

		role, _ := msg["role"].(string)

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

		ev := Event{
			Role:       role,
			Content:    content,
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

		events = append(events, Event{
			Role:       "tool",
			Content:    content,
			ToolCallID: str(msg, "tool_call_id"),
			IsError:    isErr,
			SourceTool: "hermes_json",
		})
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

		content := msg["content"]
		switch c := content.(type) {
		case string:
			events = append(events, Event{
				Role: role, Content: c,
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
						Role: role, Content: text,
						SourceTool: "claude_code",
					})
				case "thinking":
					think, _ := b["thinking"].(string)
					redacted, _ := b["redacted"].(bool)
					events = append(events, Event{
						Role:       "assistant",
						Reasoning:  think,
						Redacted:   redacted,
						SourceTool: "claude_code",
					})
				case "tool_use":
					id, _ := b["id"].(string)
					name, _ := b["name"].(string)
					events = append(events, Event{
						Role: "assistant",
						ToolCalls: []ToolCall{{ID: id, Name: name}},
						SourceTool: "claude_code",
					})
				case "tool_result":
					tid, _ := b["tool_use_id"].(string)
					isErr, _ := b["is_error"].(bool)
					ct, _ := b["content"].(string)
					events = append(events, Event{
						Role:       "tool",
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

		// Codex CLI also stores assistant message per tool_use (Anthropic-format hybrid)
		if role == "assistant" && len(toolCalls) == 0 {
			// Check for Anthropic-style tool_use blocks in content
			if cArr, ok := msg["content"].([]interface{}); ok {
				for _, blk := range cArr {
					if b, ok := blk.(map[string]interface{}); ok {
						if tp, _ := b["type"].(string); tp == "tool_use" {
							toolCalls = append(toolCalls, ToolCall{
								ID: str(b, "id"), Name: str(b, "name"),
							})
						}
					}
				}
			}
		}

		ev := Event{
			Role:       role,
			Content:    content,
			Reasoning:  reasoning,
			ToolCalls:  toolCalls,
			ModelUsed:  model,
			SourceTool: "codex_cli",
		}

		// Tool results
		if role == "tool" {
			ev.ToolCallID = str(msg, "tool_call_id")
			ev.IsError, _ = msg["is_error"].(bool)
		}

		events = append(events, ev)
	}

	return events, nil
}

// ── Gemini CLI ──

func parseGeminiCLI(doc map[string]interface{}, raw string) ([]Event, error) {
	var events []Event

	// Single JSON blob with contents
	if doc != nil {
		if contents, ok := doc["contents"].([]interface{}); ok {
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
							Role: role, Content: text,
							SourceTool: "gemini_cli",
						})
					}
					// Function calls
					if fc, ok := p["functionCall"]; ok {
						fcJSON, _ := json.Marshal(fc)
						events = append(events, Event{
							Role:    role,
							Content: string(fcJSON),
							SourceTool: "gemini_cli",
						})
					}
					if fr, ok := p["functionResponse"]; ok {
						frJSON, _ := json.Marshal(fr)
						events = append(events, Event{
							Role:    "tool",
							Content: string(frJSON),
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
							Role: role, Content: text,
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
								Role: role, Content: text,
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

		content := msg["content"]
		switch c := content.(type) {
		case string:
			events = append(events, Event{
				Role: role, Content: c,
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
						Role: role, Content: text,
						SourceTool: "opencode",
					})
				case "thinking":
					think, _ := b["thinking"].(string)
					redacted, _ := b["redacted"].(bool)
					events = append(events, Event{
						Role:      "assistant",
						Reasoning: think,
						Redacted:  redacted,
						SourceTool: "opencode",
					})
				case "tool_use":
					id, _ := b["id"].(string)
					name, _ := b["name"].(string)
					events = append(events, Event{
						Role: "assistant",
						ToolCalls: []ToolCall{{ID: id, Name: name}},
						SourceTool: "opencode",
					})
				case "tool_result":
					tid, _ := b["tool_use_id"].(string)
					isErr, _ := b["is_error"].(bool)
					ct, _ := b["content"].(string)
					events = append(events, Event{
						Role:       "tool",
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
					Role: role, ToolCalls: tcList,
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
		content, _ := msg["content"].(string)
		isErr, _ := msg["is_error"].(bool)

		events = append(events, Event{
			Role:       "tool",
			Content:    content,
			ToolCallID: str(msg, "tool_call_id"),
			IsError:    isErr,
			SourceTool: "opencode",
		})
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

	return events, nil
}

// ═══════════════════════════════════════════════════════════════
// ANALYSIS ENGINE
// ═══════════════════════════════════════════════════════════════

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
			if ev.Reasoning != "" {
				m.ReasoningBlocks++
				rc := len(ev.Reasoning)
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

// ═══════════════════════════════════════════════════════════════
// HEALTH SCORE
// ═══════════════════════════════════════════════════════════════

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
	return &Session{Name: name, Path: path, Metrics: m, Anomalies: a, Health: h}, nil
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
		return sessions[i].Name > sessions[j].Name
	})
	return sessions
}

func FindSessionFiles(dir string) []string {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil
	}
	var files []string
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
		files = append(files, filepath.Join(dir, name))
	}
	sort.Sort(sort.Reverse(sort.StringSlice(files)))
	return files
}

// ═══════════════════════════════════════════════════════════════
// UTILITIES
// ═══════════════════════════════════════════════════════════════

func str(m map[string]interface{}, key string) string {
	if v, ok := m[key].(string); ok {
		return v
	}
	return ""
}

func parseTS(raw string) time.Time {
	if raw == "" {
		return time.Time{}
	}
	s := strings.ReplaceAll(raw, "Z", "+00:00")
	if t, err := time.Parse(time.RFC3339, s); err == nil {
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
