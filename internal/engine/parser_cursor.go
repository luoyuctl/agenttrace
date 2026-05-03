package engine

import (
	"encoding/json"
	"fmt"
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
		composerDoc, ok := cursorObject(v)
		if !ok {
			return
		}
		for _, item := range cursorArray(composerDoc["allComposers"]) {
			m, ok := item.(map[string]interface{})
			if !ok {
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
				Timestamp:  cursorUnixMS(cursorFirst(m["lastUpdatedAt"], m["createdAt"])),
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
	if raw, ok := v.(string); ok && raw != "" {
		var arr []interface{}
		if err := json.Unmarshal([]byte(raw), &arr); err == nil {
			return arr
		}
	}
	return nil
}

func cursorObject(v interface{}) (map[string]interface{}, bool) {
	if obj, ok := v.(map[string]interface{}); ok {
		return obj, true
	}
	if raw, ok := v.(string); ok && raw != "" {
		var obj map[string]interface{}
		if err := json.Unmarshal([]byte(raw), &obj); err == nil {
			return obj, true
		}
	}
	return nil, false
}

func cursorFirst(values ...interface{}) interface{} {
	for _, v := range values {
		if v != nil {
			return v
		}
	}
	return nil
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
