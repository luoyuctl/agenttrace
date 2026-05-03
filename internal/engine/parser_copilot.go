package engine

import (
	"encoding/json"
	"fmt"
	"strconv"
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
		if sv := extractStringAttribute(span, "gen_ai.request.model"); sv != "" {
			model = sv
		}

		// Extract timestamp (nanoseconds → RFC3339)
		ts := ""
		if startNs, ok := toFloat64(span["startTimeUnixNano"]); ok && startNs > 0 {
			ts = time.Unix(0, int64(startNs)).UTC().Format(time.RFC3339Nano)
		}

		// Extract usage from attributes
		usage := make(map[string]int)
		if n := extractIntAttribute(span, "gen_ai.usage.input_tokens"); n > 0 {
			usage["input_tokens"] = n
		}
		if n := extractIntAttribute(span, "gen_ai.usage.output_tokens"); n > 0 {
			usage["output_tokens"] = n
		}
		if n := extractIntAttribute(span, "gen_ai.usage.cache_creation_input_tokens"); n > 0 {
			usage["cache_creation_input_tokens"] = n
		}
		if n := extractIntAttribute(span, "gen_ai.usage.cache_read_input_tokens"); n > 0 {
			usage["cache_read_input_tokens"] = n
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

	if len(events) == 0 {
		return nil, fmt.Errorf("copilot_cli: no valid events found")
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
	for _, key := range []string{"content", "message", "body", "gen_ai.response.text"} {
		if content := extractStringAttribute(span, key); content != "" {
			return content
		}
	}
	return ""
}

func extractStringAttribute(span map[string]interface{}, targetKey string) string {
	return copilotStringValue(copilotAttributeValue(span, targetKey))
}

func extractBoolAttribute(span map[string]interface{}, targetKey string) bool {
	return copilotBoolValue(copilotAttributeValue(span, targetKey))
}

func extractIntAttribute(span map[string]interface{}, targetKey string) int {
	return copilotIntValue(copilotAttributeValue(span, targetKey))
}

func copilotAttributeValue(span map[string]interface{}, targetKey string) interface{} {
	switch attrs := span["attributes"].(type) {
	case []interface{}:
		for _, attr := range attrs {
			am, ok := attr.(map[string]interface{})
			if !ok {
				continue
			}
			key, _ := am["key"].(string)
			if key == targetKey {
				return am["value"]
			}
		}
	case map[string]interface{}:
		return attrs[targetKey]
	}
	return nil
}

func copilotStringValue(v interface{}) string {
	switch value := v.(type) {
	case string:
		return value
	case map[string]interface{}:
		if sv, _ := value["stringValue"].(string); sv != "" {
			return sv
		}
	}
	return ""
}

func copilotBoolValue(v interface{}) bool {
	switch value := v.(type) {
	case bool:
		return value
	case map[string]interface{}:
		if bv, _ := value["boolValue"].(bool); bv {
			return true
		}
	}
	return false
}

func copilotIntValue(v interface{}) int {
	switch value := v.(type) {
	case float64:
		return int(value)
	case int:
		return value
	case json.Number:
		val, _ := value.Int64()
		return int(val)
	case string:
		val, err := strconv.ParseInt(value, 10, 64)
		if err == nil {
			return int(val)
		}
	case map[string]interface{}:
		if iv, ok := value["intValue"]; ok {
			return copilotIntValue(iv)
		}
		if sv, ok := value["stringValue"]; ok {
			return copilotIntValue(sv)
		}
		if dv, ok := value["doubleValue"]; ok {
			return copilotIntValue(dv)
		}
	}
	return 0
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
	case string:
		f, err := strconv.ParseFloat(n, 64)
		return f, err == nil
	}
	return 0, false
}
