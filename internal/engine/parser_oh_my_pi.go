package engine

import (
	"encoding/json"
	"fmt"
	"strings"
	"time"
)

const ohMyPiSource = "oh_my_pi"

func isOhMyPiSessionHeader(obj map[string]interface{}) bool {
	if typ, _ := obj["type"].(string); typ != "session" {
		return false
	}
	_, hasVersion := obj["version"]
	_, hasCWD := obj["cwd"]
	_, hasTitleSource := obj["titleSource"]
	_, hasParent := obj["parentSession"]
	return hasVersion || hasCWD || hasTitleSource || hasParent
}

func parseOhMyPiSessionJSONL(raw string) ([]Event, error) {
	var metaEvents []Event
	var events []Event
	model := "unknown"
	seenHeader := false

	for _, line := range strings.Split(raw, "\n") {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}

		var obj map[string]interface{}
		if err := json.Unmarshal([]byte(line), &obj); err != nil {
			continue
		}

		typ, _ := obj["type"].(string)
		if !seenHeader {
			if typ != "session" {
				return nil, fmt.Errorf("oh_my_pi: missing session header")
			}
			if str(obj, "id") == "" {
				return nil, fmt.Errorf("oh_my_pi: invalid session header")
			}
			seenHeader = true
			continue
		}

		ts := str(obj, "timestamp")
		switch typ {
		case "message":
			msg, ok := obj["message"].(map[string]interface{})
			if !ok {
				continue
			}
			for _, ev := range ohMyPiMessageEvents(msg, ts, &model) {
				if ev.Role == "meta" {
					metaEvents = append(metaEvents, ev)
				} else {
					events = append(events, ev)
				}
			}
		case "custom_message":
			content, _, _, _ := ohMyPiContent(obj["content"])
			if strings.TrimSpace(content) != "" {
				events = append(events, Event{
					Role:       "user",
					Content:    content,
					Timestamp:  ts,
					ModelUsed:  model,
					SourceTool: ohMyPiSource,
				})
			}
		case "model_change":
			if nextModel := str(obj, "model"); nextModel != "" {
				model = nextModel
				metaEvents = append(metaEvents, Event{
					Role:       "meta",
					Timestamp:  ts,
					ModelUsed:  model,
					SourceTool: ohMyPiSource,
				})
			}
		case "branch_summary", "compaction":
			content := strings.TrimSpace(str(obj, "summary"))
			if content != "" {
				events = append(events, Event{
					Role:       "assistant",
					Content:    content,
					Timestamp:  ts,
					ModelUsed:  model,
					SourceTool: ohMyPiSource,
				})
			}
		}
	}

	if !seenHeader {
		return nil, fmt.Errorf("oh_my_pi: missing session header")
	}
	if len(events) == 0 {
		return nil, fmt.Errorf("oh_my_pi: no parseable events")
	}
	return append(metaEvents, events...), nil
}

func ohMyPiMessageEvents(msg map[string]interface{}, entryTS string, model *string) []Event {
	role := str(msg, "role")
	ts := ohMyPiTimestamp(msg["timestamp"], entryTS)

	if nextModel := str(msg, "model"); nextModel != "" {
		*model = nextModel
	}

	var events []Event
	if usage := ohMyPiUsage(msg["usage"]); len(usage) > 0 {
		events = append(events, Event{
			Role:       "meta",
			Timestamp:  ts,
			Usage:      usage,
			ModelUsed:  *model,
			SourceTool: ohMyPiSource,
		})
	}

	content, reasoning, redacted, toolCalls := ohMyPiContent(msg["content"])
	switch role {
	case "user":
		if content != "" {
			events = append(events, Event{
				Role:       "user",
				Content:    content,
				Timestamp:  ts,
				ModelUsed:  *model,
				SourceTool: ohMyPiSource,
			})
		}
	case "developer":
		if content != "" {
			events = append(events, Event{
				Role:       "system",
				Content:    content,
				Timestamp:  ts,
				ModelUsed:  *model,
				SourceTool: ohMyPiSource,
			})
		}
	case "assistant":
		if content != "" || reasoning != "" || len(toolCalls) > 0 {
			events = append(events, Event{
				Role:       "assistant",
				Content:    content,
				Reasoning:  reasoning,
				Redacted:   redacted,
				ToolCalls:  toolCalls,
				Timestamp:  ts,
				ModelUsed:  *model,
				SourceTool: ohMyPiSource,
			})
		}
	case "toolResult":
		events = append(events, Event{
			Role:       "tool",
			Content:    content,
			Timestamp:  ts,
			ToolCallID: str(msg, "toolCallId"),
			IsError:    boolValue(msg["isError"]),
			ModelUsed:  *model,
			SourceTool: ohMyPiSource,
		})
	}
	return events
}

func ohMyPiContent(raw interface{}) (string, string, bool, []ToolCall) {
	switch c := raw.(type) {
	case string:
		return c, "", false, nil
	case []interface{}:
		var textParts []string
		var reasoningParts []string
		var toolCalls []ToolCall
		redacted := false
		for _, item := range c {
			block, ok := item.(map[string]interface{})
			if !ok {
				continue
			}
			switch str(block, "type") {
			case "text":
				if text := str(block, "text"); text != "" {
					textParts = append(textParts, text)
				}
			case "thinking":
				if thinking := str(block, "thinking"); thinking != "" {
					reasoningParts = append(reasoningParts, thinking)
				}
			case "redactedThinking":
				redacted = true
				if data := str(block, "data"); data != "" {
					reasoningParts = append(reasoningParts, data)
				}
			case "toolCall":
				id := str(block, "id")
				name := str(block, "name")
				if id != "" || name != "" {
					toolCalls = append(toolCalls, ToolCall{
						ID:   id,
						Name: name,
						Args: jsonish(block["arguments"]),
					})
				}
			case "image":
				textParts = append(textParts, "[image]")
			}
		}
		return strings.Join(textParts, "\n"), strings.Join(reasoningParts, "\n"), redacted, toolCalls
	default:
		return jsonish(raw), "", false, nil
	}
}

func ohMyPiUsage(raw interface{}) map[string]int {
	m, ok := raw.(map[string]interface{})
	if !ok {
		return nil
	}
	usage := map[string]int{
		"input_tokens":                numberAsInt(m["input"]) + numberAsInt(m["input_tokens"]),
		"output_tokens":               numberAsInt(m["output"]) + numberAsInt(m["output_tokens"]),
		"cache_read_input_tokens":     numberAsInt(m["cacheRead"]) + numberAsInt(m["cache_read_input_tokens"]),
		"cache_creation_input_tokens": numberAsInt(m["cacheWrite"]) + numberAsInt(m["cache_creation_input_tokens"]),
	}
	for _, v := range usage {
		if v > 0 {
			return usage
		}
	}
	return nil
}

func ohMyPiTimestamp(raw interface{}, fallback string) string {
	if ms, ok := numberAsInt64(raw); ok && ms > 0 {
		return time.UnixMilli(ms).UTC().Format(time.RFC3339Nano)
	}
	if s, ok := raw.(string); ok && s != "" {
		return s
	}
	return fallback
}

func numberAsInt64(v interface{}) (int64, bool) {
	switch n := v.(type) {
	case int:
		return int64(n), true
	case int64:
		return n, true
	case float64:
		return int64(n), true
	case json.Number:
		i, err := n.Int64()
		return i, err == nil
	default:
		return 0, false
	}
}

func boolValue(v interface{}) bool {
	b, _ := v.(bool)
	return b
}
