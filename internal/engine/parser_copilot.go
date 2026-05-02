package engine

import (
	"encoding/json"
	"strings"
	"time"
)

// parseCopilotCLI parses Copilot CLI OTel (OpenTelemetry) JSONL session files.
// Copilot CLI stores sessions at ~/.copilot/otel/*.jsonl, one OTel span per line.
//
// Key OTel fields:
//   - "traceId", "spanId", "parentSpanId" — trace identifiers
//   - "name" — span name like "chat.completion", "tool.call", "tool.result"
//   - "attributes" — array of {key, value} pairs including:
//   - "gen_ai.request.model" → model name
//   - "gen_ai.usage.input_tokens" → input tokens
//   - "gen_ai.usage.output_tokens" → output tokens
//   - "startTimeUnixNano", "endTimeUnixNano" → nanosecond timestamps
func parseCopilotCLI(raw string) ([]Event, error) {
	var events []Event
	model := "unknown"

	for _, line := range strings.Split(raw, "\n") {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}

		var span map[string]interface{}
		if err := json.Unmarshal([]byte(line), &span); err != nil {
			continue
		}

		// Extract span name → determines role
		name, _ := span["name"].(string)

		// Extract model from attributes
		if attrs, ok := span["attributes"].([]interface{}); ok {
			for _, attr := range attrs {
				if am, ok := attr.(map[string]interface{}); ok {
					key, _ := am["key"].(string)
					if key == "gen_ai.request.model" {
						if v, ok := am["value"].(map[string]interface{}); ok {
							if sv, _ := v["stringValue"].(string); sv != "" {
								model = sv
							}
						}
					}
				}
			}
		}

		// Extract timestamp (nanoseconds → RFC3339)
		ts := ""
		if startNs, ok := toFloat64(span["startTimeUnixNano"]); ok && startNs > 0 {
			ts = time.Unix(0, int64(startNs)).UTC().Format(time.RFC3339Nano)
		}

		// Extract usage from attributes
		usage := make(map[string]int)
		if attrs, ok := span["attributes"].([]interface{}); ok {
			for _, attr := range attrs {
				if am, ok := attr.(map[string]interface{}); ok {
					key, _ := am["key"].(string)
					switch key {
					case "gen_ai.usage.input_tokens":
						usage["input_tokens"] = extractIntValue(am)
					case "gen_ai.usage.output_tokens":
						usage["output_tokens"] = extractIntValue(am)
					case "gen_ai.usage.cache_creation_input_tokens":
						usage["cache_creation_input_tokens"] = extractIntValue(am)
					case "gen_ai.usage.cache_read_input_tokens":
						usage["cache_read_input_tokens"] = extractIntValue(am)
					}
				}
			}
		}

		// Map span name to event
		switch name {
		case "chat.completion":
			content := extractSpanContent(span)
			if content != "" {
				events = append(events, Event{
					Role:       "assistant",
					Content:    content,
					Timestamp:  ts,
					ModelUsed:  model,
					SourceTool: "copilot_cli",
				})
			}
			// If usage data was extracted, add as meta
			if len(usage) > 0 {
				events = append(events, Event{
					Role:       "meta",
					Usage:      usage,
					ModelUsed:  model,
					SourceTool: "copilot_cli",
				})
			}

		case "tool.call":
			toolName := extractStringAttribute(span, "tool.name")
			toolCallID := extractStringAttribute(span, "tool.call.id")
			if toolCallID == "" {
				toolCallID = spanID(span)
			}
			events = append(events, Event{
				Role: "assistant",
				ToolCalls: []ToolCall{{
					ID:   toolCallID,
					Name: toolName,
				}},
				Timestamp:  ts,
				ModelUsed:  model,
				SourceTool: "copilot_cli",
			})

		case "tool.result":
			toolCallID := extractStringAttribute(span, "tool.call.id")
			if toolCallID == "" {
				toolCallID = parentSpanID(span)
			}
			content := extractSpanContent(span)
			isErr := extractBoolAttribute(span, "tool.result.is_error")
			events = append(events, Event{
				Role:       "tool",
				Content:    content,
				Timestamp:  ts,
				ToolCallID: toolCallID,
				IsError:    isErr,
				ModelUsed:  model,
				SourceTool: "copilot_cli",
			})

		default:
			// Generic span: treat content as assistant output
			content := extractSpanContent(span)
			if content != "" {
				events = append(events, Event{
					Role:       "assistant",
					Content:    content,
					Timestamp:  ts,
					ModelUsed:  model,
					SourceTool: "copilot_cli",
				})
			}
		}
	}

	return events, nil
}

// ── Copilot OTel helpers ──

func extractSpanContent(span map[string]interface{}) string {
	// Try direct content fields
	for _, key := range []string{"content", "text", "message", "body"} {
		if v, ok := span[key].(string); ok && v != "" {
			return v
		}
	}
	// Try attributes for content
	return extractStringAttribute(span, "content")
}

func extractStringAttribute(span map[string]interface{}, targetKey string) string {
	attrs, ok := span["attributes"].([]interface{})
	if !ok {
		return ""
	}
	for _, attr := range attrs {
		am, ok := attr.(map[string]interface{})
		if !ok {
			continue
		}
		key, _ := am["key"].(string)
		if key == targetKey {
			if v, ok := am["value"].(map[string]interface{}); ok {
				if sv, _ := v["stringValue"].(string); sv != "" {
					return sv
				}
			}
		}
	}
	return ""
}

func extractBoolAttribute(span map[string]interface{}, targetKey string) bool {
	attrs, ok := span["attributes"].([]interface{})
	if !ok {
		return false
	}
	for _, attr := range attrs {
		am, ok := attr.(map[string]interface{})
		if !ok {
			continue
		}
		key, _ := am["key"].(string)
		if key == targetKey {
			if v, ok := am["value"].(map[string]interface{}); ok {
				if bv, _ := v["boolValue"].(bool); bv {
					return true
				}
			}
		}
	}
	return false
}

func extractIntValue(attr map[string]interface{}) int {
	if v, ok := attr["value"].(map[string]interface{}); ok {
		if iv, ok := v["intValue"]; ok {
			switch n := iv.(type) {
			case float64:
				return int(n)
			case int:
				return n
			case json.Number:
				val, _ := n.Int64()
				return int(val)
			}
		}
		if sv, _ := v["stringValue"].(string); sv != "" {
			var num json.Number = json.Number(sv)
			val, err := num.Int64()
			if err == nil {
				return int(val)
			}
		}
	}
	return 0
}

func spanID(span map[string]interface{}) string {
	if id, ok := span["spanId"].(string); ok {
		return id
	}
	return ""
}

func parentSpanID(span map[string]interface{}) string {
	if id, ok := span["parentSpanId"].(string); ok {
		return id
	}
	return ""
}

func toFloat64(v interface{}) (float64, bool) {
	switch n := v.(type) {
	case float64:
		return n, true
	case json.Number:
		f, err := n.Float64()
		return f, err == nil
	}
	return 0, false
}
