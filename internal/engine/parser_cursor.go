package engine

import (
	"fmt"
	"strings"
	"time"
)

const cursorSource = "cursor"

func isCursorExport(doc map[string]interface{}) bool {
	for _, key := range []string{
		"composer.composerData",
		"composerData",
		"aiService.generations",
		"aiService.prompts",
		"allComposers",
	} {
		if _, ok := doc[key]; ok {
			return true
		}
	}
	return false
}

func isCursorGenerationArray(arr []interface{}) bool {
	for _, item := range arr {
		m, ok := item.(map[string]interface{})
		if !ok {
			continue
		}
		if _, hasID := m["generationUUID"]; hasID {
			if _, hasTS := m["unixMs"]; hasTS {
				return true
			}
		}
	}
	return false
}

func isCursorPromptArray(arr []interface{}) bool {
	for _, item := range arr {
		m, ok := item.(map[string]interface{})
		if !ok {
			continue
		}
		if _, hasText := m["text"]; hasText {
			if _, hasType := m["commandType"]; hasType {
				return true
			}
		}
	}
	return false
}

func parseCursorExport(doc map[string]interface{}, arr []interface{}) ([]Event, error) {
	var events []Event

	addPrompts := func(v interface{}) {
		for _, item := range cursorArray(v) {
			m, ok := item.(map[string]interface{})
			if !ok {
				continue
			}
			text := str(m, "text")
			if text == "" {
				continue
			}
			events = append(events, Event{Role: "user", Content: text, SourceTool: cursorSource})
		}
	}

	addGenerations := func(v interface{}) {
		for _, item := range cursorArray(v) {
			m, ok := item.(map[string]interface{})
			if !ok {
				continue
			}
			content := str(m, "textDescription")
			if content == "" {
				content = str(m, "description")
			}
			if content == "" {
				content = str(m, "text")
			}
			if content == "" {
				content = str(m, "type")
			}
			if content == "" {
				continue
			}
			events = append(events, Event{
				Role:       "assistant",
				Content:    content,
				Timestamp:  cursorUnixMS(m["unixMs"]),
				SourceTool: cursorSource,
			})
		}
	}

	addComposers := func(v interface{}) {
		composerDoc, ok := v.(map[string]interface{})
		if !ok {
			return
		}
		for _, item := range cursorArray(composerDoc["allComposers"]) {
			m, ok := item.(map[string]interface{})
			if !ok {
				continue
			}
			fallbackTS := cursorTimestamp(cursorFirst(m["lastUpdatedAt"], m["createdAt"]))
			if msgEvents := cursorComposerMessageEvents(m, fallbackTS); len(msgEvents) > 0 {
				events = append(events, msgEvents...)
				continue
			}
			content := str(m, "name")
			subtitle := str(m, "subtitle")
			if subtitle != "" && subtitle != content {
				if content != "" {
					content += "\n"
				}
				content += subtitle
			}
			if content == "" {
				content = str(m, "type")
			}
			if content == "" {
				continue
			}
			events = append(events, Event{
				Role:       "assistant",
				Content:    content,
				Timestamp:  fallbackTS,
				SourceTool: cursorSource,
			})
		}
	}

	if doc != nil {
		addPrompts(cursorFirst(doc["aiService.prompts"], doc["prompts"]))
		addGenerations(cursorFirst(doc["aiService.generations"], doc["generations"]))
		addComposers(cursorFirst(doc["composer.composerData"], doc["composerData"], doc))
	} else {
		if isCursorPromptArray(arr) {
			addPrompts(arr)
		}
		if isCursorGenerationArray(arr) {
			addGenerations(arr)
		}
	}

	if len(events) == 0 {
		return nil, fmt.Errorf("cursor export: no parseable events")
	}
	return events, nil
}

func cursorArray(v interface{}) []interface{} {
	if arr, ok := v.([]interface{}); ok {
		return arr
	}
	return nil
}

func cursorComposerMessageEvents(composer map[string]interface{}, fallbackTS string) []Event {
	messages := cursorComposerMessages(composer)
	var events []Event
	for _, item := range messages {
		msg, ok := item.(map[string]interface{})
		if !ok {
			continue
		}
		role := cursorRole(str(msg, "role"))
		if role == "" {
			role = cursorRole(str(msg, "speaker"))
		}
		if role == "" {
			role = cursorRole(str(msg, "type"))
		}
		content := cursorText(cursorFirst(msg["text"], msg["content"], msg["message"], msg["markdown"], msg["rawText"]))
		if role == "" || content == "" {
			continue
		}
		ts := cursorTimestamp(cursorFirst(msg["unixMs"], msg["timestamp"], msg["createdAt"], msg["lastUpdatedAt"]))
		if ts == "" {
			ts = fallbackTS
		}
		events = append(events, Event{
			Role:       role,
			Content:    content,
			Timestamp:  ts,
			SourceTool: cursorSource,
		})
	}
	return events
}

func cursorComposerMessages(composer map[string]interface{}) []interface{} {
	for _, key := range []string{"messages", "conversationMessages"} {
		if arr := cursorArray(composer[key]); len(arr) > 0 {
			return arr
		}
	}
	if arr := cursorArray(composer["conversation"]); len(arr) > 0 {
		return arr
	}
	if conv, ok := composer["conversation"].(map[string]interface{}); ok {
		for _, key := range []string{"messages", "conversationMessages"} {
			if arr := cursorArray(conv[key]); len(arr) > 0 {
				return arr
			}
		}
	}
	return nil
}

func cursorRole(role string) string {
	switch strings.ToLower(role) {
	case "user", "human":
		return "user"
	case "assistant", "ai", "bot":
		return "assistant"
	default:
		return ""
	}
}

func cursorText(v interface{}) string {
	switch value := v.(type) {
	case string:
		return value
	case map[string]interface{}:
		return str(value, "text")
	case []interface{}:
		var parts []string
		for _, item := range value {
			if text := cursorText(item); text != "" {
				parts = append(parts, text)
			}
		}
		return strings.Join(parts, "\n")
	default:
		return ""
	}
}

func cursorFirst(values ...interface{}) interface{} {
	for _, v := range values {
		if v != nil {
			return v
		}
	}
	return nil
}

func cursorTimestamp(v interface{}) string {
	if ts, ok := v.(string); ok {
		return ts
	}
	return cursorUnixMS(v)
}

func cursorUnixMS(v interface{}) string {
	var ms int64
	switch n := v.(type) {
	case float64:
		ms = int64(n)
	case int64:
		ms = n
	case int:
		ms = int64(n)
	default:
		return ""
	}
	if ms <= 0 {
		return ""
	}
	return time.UnixMilli(ms).Format(time.RFC3339)
}
