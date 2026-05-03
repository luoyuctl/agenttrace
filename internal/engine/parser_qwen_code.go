package engine

import (
	"encoding/json"
	"fmt"
	"strings"
)

const qwenCodeSource = "qwen_code"

func isQwenCodeEvent(obj map[string]interface{}) bool {
	typ, _ := obj["type"].(string)
	if typ != "system" && typ != "user" && typ != "assistant" && typ != "result" && typ != "stream_event" {
		return false
	}
	if _, ok := obj["session_id"]; ok {
		return true
	}
	if _, ok := obj["uuid"]; ok {
		_, hasMessage := obj["message"]
		_, hasResult := obj["result"]
		_, hasSubtype := obj["subtype"]
		return hasMessage || hasResult || hasSubtype
	}
	return false
}

func isQwenCodeJSONOutput(obj map[string]interface{}) bool {
	if _, hasResponse := obj["response"]; !hasResponse {
		if _, hasError := obj["error"]; !hasError {
			return false
		}
	}
	_, hasStats := obj["stats"]
	_, hasUsage := obj["usage"]
	return hasStats || hasUsage
}

func isQwenCodeArray(arr []interface{}) bool {
	for _, item := range arr {
		obj, ok := item.(map[string]interface{})
		if ok && isQwenCodeEvent(obj) {
			return true
		}
	}
	return false
}

func parseQwenCode(doc map[string]interface{}, arr []interface{}, raw string) ([]Event, error) {
	var objs []map[string]interface{}
	if len(arr) > 0 {
		for _, item := range arr {
			if obj, ok := item.(map[string]interface{}); ok {
				objs = append(objs, obj)
			}
		}
	} else if doc != nil {
		if isQwenCodeJSONOutput(doc) && !isQwenCodeEvent(doc) {
			return parseQwenCodeJSONOutput(doc)
		}
		objs = append(objs, doc)
	} else {
		for _, line := range strings.Split(raw, "\n") {
			line = strings.TrimSpace(line)
			if line == "" {
				continue
			}
			var obj map[string]interface{}
			if err := json.Unmarshal([]byte(line), &obj); err != nil {
				continue
			}
			objs = append(objs, obj)
		}
	}

	var metaEvents []Event
	var events []Event
	model := "unknown"
	hasAssistant := false
	hasUsage := false

	for _, obj := range objs {
		if !isQwenCodeEvent(obj) {
			continue
		}
		typ, _ := obj["type"].(string)
		ts := str(obj, "timestamp")
		switch typ {
		case "system":
			if nextModel := str(obj, "model"); nextModel != "" {
				model = nextModel
			}
			metaEvents = append(metaEvents, Event{
				Role:       "meta",
				Timestamp:  ts,
				ModelUsed:  model,
				SourceTool: qwenCodeSource,
			})
		case "user":
			msg, ok := obj["message"].(map[string]interface{})
			if !ok {
				continue
			}
			for _, ev := range qwenMessageEvents(msg, ts, &model) {
				events = append(events, ev)
			}
		case "assistant":
			msg, ok := obj["message"].(map[string]interface{})
			if !ok {
				continue
			}
			for _, ev := range qwenMessageEvents(msg, ts, &model) {
				if ev.Role == "meta" {
					metaEvents = append(metaEvents, ev)
					if len(ev.Usage) > 0 {
						hasUsage = true
					}
				} else {
					if ev.Role == "assistant" {
						hasAssistant = true
					}
					events = append(events, ev)
				}
			}
		case "result":
			if usage := qwenUsage(obj["usage"]); len(usage) > 0 && !hasUsage {
				metaEvents = append(metaEvents, Event{
					Role:       "meta",
					Timestamp:  ts,
					Usage:      usage,
					ModelUsed:  model,
					SourceTool: qwenCodeSource,
				})
				hasUsage = true
			}
			if usage := qwenStatsUsage(obj["stats"]); len(usage) > 0 && !hasUsage {
				metaEvents = append(metaEvents, Event{
					Role:       "meta",
					Timestamp:  ts,
					Usage:      usage,
					ModelUsed:  model,
					SourceTool: qwenCodeSource,
				})
				hasUsage = true
			}
			if usage := qwenModelUsage(obj["modelUsage"]); len(usage) > 0 && !hasUsage {
				metaEvents = append(metaEvents, Event{
					Role:       "meta",
					Timestamp:  ts,
					Usage:      usage,
					ModelUsed:  model,
					SourceTool: qwenCodeSource,
				})
				hasUsage = true
			}
			if !hasAssistant {
				content := strings.TrimSpace(str(obj, "result"))
				if content != "" {
					events = append(events, Event{
						Role:       "assistant",
						Content:    content,
						Timestamp:  ts,
						ModelUsed:  model,
						SourceTool: qwenCodeSource,
					})
					hasAssistant = true
				}
			}
		}
	}

	if len(events) == 0 {
		return nil, fmt.Errorf("qwen_code: no parseable events")
	}
	return append(metaEvents, events...), nil
}

func parseQwenCodeJSONOutput(obj map[string]interface{}) ([]Event, error) {
	var events []Event
	model := "unknown"
	if usage := qwenUsage(obj["usage"]); len(usage) > 0 {
		events = append(events, Event{Role: "meta", Usage: usage, ModelUsed: model, SourceTool: qwenCodeSource})
	} else if usage := qwenStatsUsage(obj["stats"]); len(usage) > 0 {
		events = append(events, Event{Role: "meta", Usage: usage, ModelUsed: model, SourceTool: qwenCodeSource})
	}
	if content := strings.TrimSpace(str(obj, "response")); content != "" {
		events = append(events, Event{Role: "assistant", Content: content, ModelUsed: model, SourceTool: qwenCodeSource})
	}
	if len(events) == 0 {
		return nil, fmt.Errorf("qwen_code: no parseable events")
	}
	return events, nil
}

func qwenMessageEvents(msg map[string]interface{}, fallbackTS string, model *string) []Event {
	if nextModel := str(msg, "model"); nextModel != "" {
		*model = nextModel
	}
	ts := str(msg, "timestamp")
	if ts == "" {
		ts = fallbackTS
	}

	var events []Event
	if usage := qwenUsage(msg["usage"]); len(usage) > 0 {
		events = append(events, Event{
			Role:       "meta",
			Timestamp:  ts,
			Usage:      usage,
			ModelUsed:  *model,
			SourceTool: qwenCodeSource,
		})
	}

	content, reasoning, redacted, toolCalls := qwenContent(msg["content"])
	toolResults := qwenToolResultEvents(msg["content"], ts, *model)
	role := str(msg, "role")
	if role == "" {
		role = "assistant"
	}
	switch role {
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
				SourceTool: qwenCodeSource,
			})
		}
	case "user":
		if content != "" {
			events = append(events, Event{
				Role:       "user",
				Content:    content,
				Timestamp:  ts,
				ModelUsed:  *model,
				SourceTool: qwenCodeSource,
			})
		}
		events = append(events, toolResults...)
	case "tool", "tool_result", "toolResult":
		events = append(events, Event{
			Role:       "tool",
			Content:    content,
			Timestamp:  ts,
			ToolCallID: firstNonEmpty(str(msg, "tool_call_id"), str(msg, "toolCallId")),
			IsError:    boolValue(msg["is_error"]) || boolValue(msg["isError"]),
			ModelUsed:  *model,
			SourceTool: qwenCodeSource,
		})
	}
	return events
}

func qwenContent(raw interface{}) (string, string, bool, []ToolCall) {
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
			case "thinking", "reasoning":
				if thinking := firstNonEmpty(str(block, "thinking"), str(block, "text")); thinking != "" {
					reasoningParts = append(reasoningParts, thinking)
				}
			case "redacted_thinking", "redactedThinking":
				redacted = true
				if data := firstNonEmpty(str(block, "data"), str(block, "text")); data != "" {
					reasoningParts = append(reasoningParts, data)
				}
			case "tool_use", "toolCall", "function_call":
				id := firstNonEmpty(str(block, "id"), str(block, "tool_call_id"), str(block, "call_id"))
				name := str(block, "name")
				if fn, ok := block["function"].(map[string]interface{}); ok {
					name = firstNonEmpty(name, str(fn, "name"))
				}
				args := firstNonEmpty(jsonish(block["input"]), jsonish(block["arguments"]))
				if id != "" || name != "" {
					toolCalls = append(toolCalls, ToolCall{ID: id, Name: name, Args: args})
				}
			case "tool_result", "toolResult":
				continue
			case "image":
				textParts = append(textParts, "[image]")
			}
		}
		return strings.Join(textParts, "\n"), strings.Join(reasoningParts, "\n"), redacted, toolCalls
	default:
		return jsonish(raw), "", false, nil
	}
}

func qwenToolResultEvents(raw interface{}, ts string, model string) []Event {
	blocks, ok := raw.([]interface{})
	if !ok {
		return nil
	}
	var events []Event
	for _, item := range blocks {
		block, ok := item.(map[string]interface{})
		if !ok {
			continue
		}
		tp := str(block, "type")
		if tp != "tool_result" && tp != "toolResult" {
			continue
		}
		events = append(events, Event{
			Role:       "tool",
			Content:    extractToolResultContent(block),
			Timestamp:  ts,
			ToolCallID: firstNonEmpty(str(block, "tool_use_id"), str(block, "toolCallId")),
			IsError:    boolValue(block["is_error"]) || boolValue(block["isError"]),
			ModelUsed:  model,
			SourceTool: qwenCodeSource,
		})
	}
	return events
}

func qwenUsage(raw interface{}) map[string]int {
	m, ok := raw.(map[string]interface{})
	if !ok {
		return nil
	}
	usage := map[string]int{
		"input_tokens": numberAsInt(m["input_tokens"]) +
			numberAsInt(m["prompt_tokens"]) +
			numberAsInt(m["input"]) +
			numberAsInt(m["promptTokenCount"]),
		"output_tokens": numberAsInt(m["output_tokens"]) +
			numberAsInt(m["completion_tokens"]) +
			numberAsInt(m["output"]) +
			numberAsInt(m["candidatesTokenCount"]),
		"cache_read_input_tokens": numberAsInt(m["cache_read_input_tokens"]) +
			numberAsInt(m["cacheRead"]) +
			numberAsInt(m["cached_tokens"]),
		"cache_creation_input_tokens": numberAsInt(m["cache_creation_input_tokens"]) +
			numberAsInt(m["cacheWrite"]),
	}
	for _, v := range usage {
		if v > 0 {
			return usage
		}
	}
	return nil
}

func qwenStatsUsage(raw interface{}) map[string]int {
	stats, ok := raw.(map[string]interface{})
	if !ok {
		return nil
	}
	models, ok := stats["models"].(map[string]interface{})
	if !ok {
		return nil
	}
	usage := map[string]int{}
	for _, rawModel := range models {
		modelStats, ok := rawModel.(map[string]interface{})
		if !ok {
			continue
		}
		tokens, ok := modelStats["tokens"].(map[string]interface{})
		if !ok {
			continue
		}
		usage["input_tokens"] += numberAsInt(tokens["input"]) + numberAsInt(tokens["input_tokens"])
		usage["output_tokens"] += numberAsInt(tokens["output"]) + numberAsInt(tokens["output_tokens"])
		usage["cache_read_input_tokens"] += numberAsInt(tokens["cacheRead"]) + numberAsInt(tokens["cache_read_input_tokens"])
		usage["cache_creation_input_tokens"] += numberAsInt(tokens["cacheWrite"]) + numberAsInt(tokens["cache_creation_input_tokens"])
	}
	for _, v := range usage {
		if v > 0 {
			return usage
		}
	}
	return nil
}

func qwenModelUsage(raw interface{}) map[string]int {
	modelUsage, ok := raw.(map[string]interface{})
	if !ok {
		return nil
	}
	usage := map[string]int{}
	for _, rawModel := range modelUsage {
		modelStats, ok := rawModel.(map[string]interface{})
		if !ok {
			continue
		}
		usage["input_tokens"] += numberAsInt(modelStats["inputTokens"]) + numberAsInt(modelStats["input_tokens"])
		usage["output_tokens"] += numberAsInt(modelStats["outputTokens"]) + numberAsInt(modelStats["output_tokens"])
		usage["cache_read_input_tokens"] += numberAsInt(modelStats["cacheReadInputTokens"]) + numberAsInt(modelStats["cache_read_input_tokens"])
		usage["cache_creation_input_tokens"] += numberAsInt(modelStats["cacheCreationInputTokens"]) + numberAsInt(modelStats["cache_creation_input_tokens"])
	}
	for _, v := range usage {
		if v > 0 {
			return usage
		}
	}
	return nil
}

func firstNonEmpty(values ...string) string {
	for _, value := range values {
		if value != "" {
			return value
		}
	}
	return ""
}
