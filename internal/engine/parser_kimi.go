package engine

import (
	"encoding/json"
	"fmt"
)

// parseKimiCLI parses Kimi CLI JSON session files.
// Kimi CLI stores sessions at ~/.kimi/sessions/*.json.
//
// Format is similar to Claude Code but with these key differences:
//   - "messages" array with "role" (user/assistant/tool) and "content"
//   - "model" field at top level
//   - No top-level "usage" field (usage may be embedded in metadata)
//   - Tool calls in Anthropic content block format within assistant messages
//     (blocks of type "text", "tool_use", "tool_result")
func parseKimiCLI(doc map[string]interface{}) ([]Event, error) {
	var events []Event
	var messages []interface{}
	model := "unknown"

	if msgs, ok := doc["messages"].([]interface{}); ok {
		messages = msgs
	}
	if m, ok := doc["model"].(string); ok {
		model = m
	}

	// Check for top-level usage/metadata (Kimi may store usage differently)
	if usage, ok := doc["usage"]; ok {
		ev := Event{Role: "meta", ModelUsed: model, SourceTool: "kimi_cli"}
		ub, _ := json.Marshal(usage)
		json.Unmarshal(ub, &ev.Usage)
		events = append(events, ev)
	}

	// Also check for usage in metadata
	if meta, ok := doc["metadata"].(map[string]interface{}); ok {
		if usage, ok := meta["usage"]; ok {
			ev := Event{Role: "meta", ModelUsed: model, SourceTool: "kimi_cli"}
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
		ts, _ := msg["timestamp"].(string)

		content := msg["content"]
		switch c := content.(type) {
		case string:
			events = append(events, Event{
				Role:       role,
				Content:    c,
				Timestamp:  ts,
				SourceTool: "kimi_cli",
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
						SourceTool: "kimi_cli",
					})
				case "thinking":
					think, _ := b["thinking"].(string)
					redacted, _ := b["redacted"].(bool)
					events = append(events, Event{
						Role:       "assistant",
						Timestamp:  ts,
						Reasoning:  think,
						Redacted:   redacted,
						SourceTool: "kimi_cli",
					})
				case "tool_use":
					id, _ := b["id"].(string)
					name, _ := b["name"].(string)
					args := jsonish(b["input"])
					if args == "" {
						args = jsonish(b["arguments"])
					}
					// Kimi may use "function" sub-object for tool name
					if name == "" {
						if fn, ok := b["function"].(map[string]interface{}); ok {
							name, _ = fn["name"].(string)
							if args == "" {
								args = jsonish(fn["arguments"])
							}
						}
					}
					events = append(events, Event{
						Role: "assistant",
						ToolCalls: []ToolCall{{
							ID:   id,
							Name: name,
							Args: args,
						}},
						Timestamp:  ts,
						SourceTool: "kimi_cli",
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
						SourceTool: "kimi_cli",
					})
				case "image":
					// Image blocks: record as assistant with placeholder
					events = append(events, Event{
						Role:       role,
						Content:    "[image]",
						Timestamp:  ts,
						SourceTool: "kimi_cli",
					})
				}
			}
		default:
			// Check for OpenAI-style tool_calls in assistant messages
			if tcs, ok := msg["tool_calls"].([]interface{}); ok {
				var tcList []ToolCall
				for _, tc := range tcs {
					if tcm, ok := tc.(map[string]interface{}); ok {
						tcItem := ToolCall{ID: str(tcm, "id")}
						if fn, ok := tcm["function"].(map[string]interface{}); ok {
							tcItem.Name = str(fn, "name")
							tcItem.Args = jsonish(fn["arguments"])
						}
						tcList = append(tcList, tcItem)
					}
				}
				events = append(events, Event{
					Role:       role,
					ToolCalls:  tcList,
					Timestamp:  ts,
					SourceTool: "kimi_cli",
				})
			}
		}
	}

	// Tool results may also appear as top-level messages with role="tool"
	for _, raw := range messages {
		msg, ok := raw.(map[string]interface{})
		if !ok {
			continue
		}
		role, _ := msg["role"].(string)
		if role != "tool" {
			continue
		}
		// Skip if already handled as content block above
		content := msg["content"]
		if cArr, ok := content.([]interface{}); ok {
			// Already processed in content block loop
			_ = cArr
			continue
		}

		// String content for tool role
		ct := extractToolResultContent(msg)
		if ct == "" {
			if s, ok := content.(string); ok {
				ct = s
			}
		}
		isErr, _ := msg["is_error"].(bool)
		ts, _ := msg["timestamp"].(string)

		events = append(events, Event{
			Role:       "tool",
			Content:    ct,
			Timestamp:  ts,
			ToolCallID: str(msg, "tool_call_id"),
			IsError:    isErr,
			SourceTool: "kimi_cli",
		})
	}

	if len(events) == 0 {
		return nil, fmt.Errorf("kimi_cli: no parseable events")
	}
	return events, nil
}
