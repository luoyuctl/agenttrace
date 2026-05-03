package engine

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"time"
)

const (
	clineSourceTool     = "cline"
	clineAPIHistoryFile = "api_conversation_history.json"
	clineUIMessagesFile = "ui_messages.json"
	clineMetadataFile   = "task_metadata.json"
	clineHistoryFile    = "taskHistory.json"
)

func isClineTaskDir(path string) bool {
	if path == "" {
		return false
	}
	info, err := os.Stat(path)
	if err != nil || !info.IsDir() {
		return false
	}
	return fileExists(filepath.Join(path, clineAPIHistoryFile)) ||
		fileExists(filepath.Join(path, clineUIMessagesFile))
}

func isClineTaskFile(path string) bool {
	switch filepath.Base(path) {
	case clineAPIHistoryFile, clineUIMessagesFile, clineMetadataFile:
		return true
	default:
		return false
	}
}

func parseClinePath(path string) ([]Event, error) {
	info, err := os.Stat(path)
	if err != nil {
		return nil, fmt.Errorf("cline: stat %s: %w", path, err)
	}
	if info.IsDir() {
		return parseClineTaskDir(path)
	}
	if isClineTaskFile(path) {
		return parseClineTaskDir(filepath.Dir(path))
	}
	return parseClineSingleFile(path)
}

func parseClineTaskDir(dir string) ([]Event, error) {
	metadata, err := readClineMetadata(dir)
	if err != nil {
		return nil, err
	}
	model := clineModel(metadata)

	var events []Event
	seen := make(map[string]bool)

	apiPath := filepath.Join(dir, clineAPIHistoryFile)
	if raw, ok, err := readClineJSON(apiPath); err != nil {
		return nil, err
	} else if ok {
		for _, ev := range parseClineAPIHistory(raw, model) {
			appendClineEvent(&events, seen, ev)
		}
	}

	uiPath := filepath.Join(dir, clineUIMessagesFile)
	if raw, ok, err := readClineJSON(uiPath); err != nil {
		return nil, err
	} else if ok {
		for _, ev := range parseClineUIMessages(raw, model) {
			appendClineEvent(&events, seen, ev)
		}
	}

	if len(events) == 0 {
		return nil, fmt.Errorf("cline: no parseable events in %s", dir)
	}
	applyClineMetadataTimestamps(events, metadata)
	return events, nil
}

func parseClineSingleFile(path string) ([]Event, error) {
	raw, ok, err := readClineJSON(path)
	if err != nil {
		return nil, err
	}
	if !ok {
		return nil, fmt.Errorf("cline: missing %s", path)
	}
	var events []Event
	seen := make(map[string]bool)
	for _, ev := range parseClineAPIHistory(raw, "unknown") {
		appendClineEvent(&events, seen, ev)
	}
	if len(events) == 0 {
		for _, ev := range parseClineUIMessages(raw, "unknown") {
			appendClineEvent(&events, seen, ev)
		}
	}
	if len(events) == 0 {
		return nil, fmt.Errorf("cline: no parseable events in %s", path)
	}
	return events, nil
}

func readClineMetadata(dir string) (map[string]interface{}, error) {
	metadata := make(map[string]interface{})
	if raw, ok, err := readClineJSON(filepath.Join(dir, clineMetadataFile)); err != nil {
		return nil, err
	} else if ok {
		if m, ok := raw.(map[string]interface{}); ok {
			for k, v := range m {
				metadata[k] = v
			}
		}
	}

	taskID := firstString(metadata, "taskId", "task_id", "id")
	for _, historyPath := range clineTaskHistoryCandidates(dir) {
		raw, ok, err := readClineJSON(historyPath)
		if err != nil {
			return nil, err
		}
		if !ok {
			continue
		}
		for k, v := range clineTaskHistoryEntry(raw, taskID) {
			if _, exists := metadata[k]; !exists {
				metadata[k] = v
			}
		}
	}

	return metadata, nil
}

func clineTaskHistoryCandidates(dir string) []string {
	return []string{
		filepath.Join(dir, clineHistoryFile),
		filepath.Join(filepath.Dir(dir), clineHistoryFile),
		filepath.Join(filepath.Dir(filepath.Dir(dir)), clineHistoryFile),
	}
}

func clineTaskHistoryEntry(raw interface{}, taskID string) map[string]interface{} {
	switch v := raw.(type) {
	case []interface{}:
		for _, item := range v {
			m, ok := item.(map[string]interface{})
			if !ok {
				continue
			}
			if taskID == "" || firstString(m, "taskId", "task_id", "id") == taskID {
				return m
			}
		}
	case map[string]interface{}:
		if tasks, ok := v["tasks"].([]interface{}); ok {
			return clineTaskHistoryEntry(tasks, taskID)
		}
		if taskID == "" || firstString(v, "taskId", "task_id", "id") == taskID {
			return v
		}
	}
	return nil
}

func readClineJSON(path string) (interface{}, bool, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, false, nil
		}
		return nil, false, fmt.Errorf("cline: read %s: %w", filepath.Base(path), err)
	}
	var raw interface{}
	if err := json.Unmarshal(data, &raw); err != nil {
		return nil, true, fmt.Errorf("cline: parse %s: %w", filepath.Base(path), err)
	}
	return raw, true, nil
}

func parseClineAPIHistory(raw interface{}, model string) []Event {
	var messages []interface{}
	switch v := raw.(type) {
	case []interface{}:
		messages = v
	case map[string]interface{}:
		for _, key := range []string{"messages", "conversation", "history"} {
			if arr, ok := v[key].([]interface{}); ok {
				messages = arr
				break
			}
		}
	}

	var events []Event
	for _, item := range messages {
		msg, ok := item.(map[string]interface{})
		if !ok {
			continue
		}
		role := clineRole(firstString(msg, "role", "speaker", "author"))
		ts := clineTimestamp(firstPresent(msg, "timestamp", "ts", "createdAt", "created_at"))
		msgEvents := clineContentEvents(role, ts, model, msg["content"])
		events = append(events, msgEvents...)
		if len(msgEvents) == 0 {
			text := firstString(msg, "text", "message")
			if role != "" && text != "" {
				events = append(events, Event{
					Role:       role,
					Content:    text,
					Timestamp:  ts,
					ModelUsed:  model,
					SourceTool: clineSourceTool,
				})
			}
		}
	}
	return events
}

func clineContentEvents(role, ts, model string, content interface{}) []Event {
	var events []Event
	switch v := content.(type) {
	case string:
		if role != "" && v != "" {
			events = append(events, Event{
				Role:       role,
				Content:    v,
				Timestamp:  ts,
				ModelUsed:  model,
				SourceTool: clineSourceTool,
			})
		}
	case []interface{}:
		for _, item := range v {
			block, ok := item.(map[string]interface{})
			if !ok {
				continue
			}
			typ := firstString(block, "type")
			switch typ {
			case "text":
				text := firstString(block, "text")
				if role != "" && text != "" {
					events = append(events, Event{
						Role:       role,
						Content:    text,
						Timestamp:  ts,
						ModelUsed:  model,
						SourceTool: clineSourceTool,
					})
				}
			case "thinking":
				events = append(events, Event{
					Role:       "assistant",
					Reasoning:  firstString(block, "thinking", "text"),
					Timestamp:  ts,
					ModelUsed:  model,
					SourceTool: clineSourceTool,
				})
			case "tool_use":
				events = append(events, Event{
					Role: "assistant",
					ToolCalls: []ToolCall{{
						ID:   firstString(block, "id", "tool_use_id"),
						Name: firstString(block, "name", "tool_name"),
						Args: jsonish(firstPresent(block, "input", "arguments")),
					}},
					Timestamp:  ts,
					ModelUsed:  model,
					SourceTool: clineSourceTool,
				})
			case "tool_result":
				isErr, _ := block["is_error"].(bool)
				events = append(events, Event{
					Role:       "tool",
					Content:    extractToolResultContent(block),
					Timestamp:  ts,
					ToolCallID: firstString(block, "tool_use_id", "tool_call_id", "id"),
					IsError:    isErr,
					ModelUsed:  model,
					SourceTool: clineSourceTool,
				})
			}
		}
	}
	return events
}

func parseClineUIMessages(raw interface{}, model string) []Event {
	var messages []interface{}
	switch v := raw.(type) {
	case []interface{}:
		messages = v
	case map[string]interface{}:
		if arr, ok := v["messages"].([]interface{}); ok {
			messages = arr
		}
	}

	var events []Event
	for _, item := range messages {
		msg, ok := item.(map[string]interface{})
		if !ok {
			continue
		}
		text := firstString(msg, "text", "content", "message")
		if text == "" {
			continue
		}
		ts := clineTimestamp(firstPresent(msg, "ts", "timestamp", "createdAt", "created_at"))
		kind := firstString(msg, "type")
		ask := firstString(msg, "ask")
		say := firstString(msg, "say")
		role := "assistant"
		if kind == "ask" || ask != "" {
			role = "user"
		}
		if say == "tool" || ask == "tool" {
			role = "assistant"
		}
		events = append(events, Event{
			Role:       role,
			Content:    text,
			Timestamp:  ts,
			ModelUsed:  model,
			SourceTool: clineSourceTool,
		})
	}
	return events
}

func appendClineEvent(events *[]Event, seen map[string]bool, ev Event) {
	if ev.SourceTool == "" {
		ev.SourceTool = clineSourceTool
	}
	if ev.ModelUsed == "" {
		ev.ModelUsed = "unknown"
	}
	if ev.Role == "" {
		return
	}
	key := clineEventKey(ev)
	if seen[key] {
		return
	}
	seen[key] = true
	*events = append(*events, ev)
}

func clineEventKey(ev Event) string {
	var toolParts []string
	for _, tc := range ev.ToolCalls {
		toolParts = append(toolParts, tc.ID+":"+tc.Name+":"+tc.Args)
	}
	return strings.Join([]string{
		ev.Role,
		ev.Content,
		ev.Timestamp,
		ev.ToolCallID,
		strings.Join(toolParts, ","),
	}, "\x00")
}

func applyClineMetadataTimestamps(events []Event, metadata map[string]interface{}) {
	if len(events) == 0 {
		return
	}
	hasTimestamp := false
	for _, ev := range events {
		if ev.Timestamp != "" {
			hasTimestamp = true
			break
		}
	}
	if hasTimestamp {
		return
	}
	start := clineTimestamp(firstPresent(metadata, "createdAt", "created_at", "ts"))
	end := clineTimestamp(firstPresent(metadata, "updatedAt", "updated_at", "lastUpdatedAt"))
	if start != "" {
		events[0].Timestamp = start
	}
	if end != "" {
		events[len(events)-1].Timestamp = end
	}
}

func clineModel(metadata map[string]interface{}) string {
	if model := firstString(metadata, "model", "modelId", "model_id", "apiModelId"); model != "" {
		return model
	}
	if cfg, ok := metadata["apiConfiguration"].(map[string]interface{}); ok {
		if model := firstString(cfg, "model", "modelId", "model_id", "apiModelId"); model != "" {
			return model
		}
	}
	return "unknown"
}

func clineRole(role string) string {
	switch strings.ToLower(role) {
	case "human":
		return "user"
	case "ai", "bot":
		return "assistant"
	default:
		return role
	}
}

func clineTimestamp(v interface{}) string {
	switch value := v.(type) {
	case string:
		if value == "" {
			return ""
		}
		if _, err := time.Parse(time.RFC3339Nano, strings.ReplaceAll(value, "Z", "+00:00")); err == nil {
			return value
		}
		if n, err := strconv.ParseInt(value, 10, 64); err == nil {
			return clineUnixTimestamp(n)
		}
		return value
	case float64:
		return clineUnixTimestamp(int64(value))
	case json.Number:
		n, _ := value.Int64()
		return clineUnixTimestamp(n)
	default:
		return ""
	}
}

func clineUnixTimestamp(n int64) string {
	switch {
	case n > 1_000_000_000_000:
		return time.UnixMilli(n).UTC().Format(time.RFC3339Nano)
	case n > 1_000_000_000:
		return time.Unix(n, 0).UTC().Format(time.RFC3339Nano)
	default:
		return ""
	}
}

func firstString(m map[string]interface{}, keys ...string) string {
	for _, key := range keys {
		if value, ok := m[key].(string); ok {
			return value
		}
	}
	return ""
}

func firstPresent(m map[string]interface{}, keys ...string) interface{} {
	for _, key := range keys {
		if value, ok := m[key]; ok {
			return value
		}
	}
	return nil
}

func fileExists(path string) bool {
	info, err := os.Stat(path)
	return err == nil && !info.IsDir()
}
