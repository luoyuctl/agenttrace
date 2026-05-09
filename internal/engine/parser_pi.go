package engine

import (
	"encoding/json"
	"fmt"
	"strings"
	"time"
)

const piSource = "pi"

// parsePi parses pi (earendil-works/pi) coding agent JSONL session files.
// pi stores sessions at ~/.pi/agent/sessions/--<path>--/<timestamp>_<uuid>.jsonl.
//
// Format (one JSON object per line):
//
//	{"type":"session","version":3,"id":"uuid","timestamp":"ISO","cwd":"/path",...}
//	{"type":"message","timestamp":"ISO","message":{AgentMessage}}
//	{"type":"model_change",...}
//	{"type":"compaction",...}
//	{"type":"branch_summary",...}
//	{"type":"thinking_level_change",...}
//	{"type":"session_info",...}
//	{"type":"custom_message",...}
//
// AgentMessage roles: user, assistant, developer, toolResult, bashExecution,
// custom, branchSummary, compactionSummary.
// Content blocks: text, thinking, redactedThinking, toolCall (camelCase), image.
//
// Key differences from Claude/Kimi:
//   - "toolCall" (camelCase) instead of "tool_use"
//   - "toolResult" role instead of "tool"
//   - Each assistant message contains full usage + cost data
//   - Timestamps can be dual: entry-level ISO string + message-level Unix ms number
//   - Entries have id/parentId tree structure (linearized for analysis)
func parsePi(raw string) ([]Event, error) {
	var events []Event
	currentModel := "unknown"

	for _, line := range strings.Split(raw, "\n") {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}

		var entry map[string]interface{}
		if err := json.Unmarshal([]byte(line), &entry); err != nil {
			continue
		}

		entryType, _ := entry["type"].(string)

		switch entryType {
		case "session":
			// Header: extract model info for subsequent messages
			if modelID, _ := entry["modelId"].(string); modelID != "" {
				currentModel = modelID
			}
			// Record session metadata as meta event
			cwd, _ := entry["cwd"].(string)
			sessionID, _ := entry["id"].(string)
			events = append(events, Event{
				Role:       "meta",
				Content:    fmt.Sprintf("session_id=%s cwd=%s", sessionID, cwd),
				ModelUsed:  currentModel,
				SourceTool: piSource,
				Timestamp:  piTimestamp(entry["timestamp"], ""),
			})

		case "message":
			msg, ok := entry["message"].(map[string]interface{})
			if !ok {
				continue
			}
			role, _ := msg["role"].(string)
			ts := piTimestamp(msg["timestamp"], piTimestamp(entry["timestamp"], ""))

			// Track model from message-level model field
			if nextModel := str(msg, "model"); nextModel != "" {
				currentModel = nextModel
			}

			// Extract usage from message (pi embeds costs in assistant messages)
			if usage := piUsage(msg["usage"]); len(usage) > 0 {
				events = append(events, Event{
					Role:       "meta",
					Timestamp:  ts,
					Usage:      usage,
					ModelUsed:  currentModel,
					SourceTool: piSource,
				})
			}

			switch role {
			case "user":
				text := concatContentText(msg["content"])
				if text != "" {
					events = append(events, Event{
						Role:       "user",
						Content:    text,
						Timestamp:  ts,
						SourceTool: piSource,
					})
				}
			case "developer":
				text := concatContentText(msg["content"])
				if text != "" {
					events = append(events, Event{
						Role:       "system",
						Content:    text,
						Timestamp:  ts,
						ModelUsed:  currentModel,
						SourceTool: piSource,
					})
				}
			case "assistant":
				contentBlocks, _ := msg["content"].([]interface{})
				for _, block := range contentBlocks {
					b, ok := block.(map[string]interface{})
					if !ok {
						continue
					}
					bt, _ := b["type"].(string)
					switch bt {
					case "text":
						text, _ := b["text"].(string)
						if text != "" {
							events = append(events, Event{
								Role:       "assistant",
								Content:    text,
								Timestamp:  ts,
								ModelUsed:  currentModel,
								SourceTool: piSource,
							})
						}
					case "thinking":
						think, _ := b["thinking"].(string)
						events = append(events, Event{
							Role:       "assistant",
							Timestamp:  ts,
							Reasoning:  think,
							Redacted:   false,
							ModelUsed:  currentModel,
							SourceTool: piSource,
						})
					case "redactedThinking":
						data, _ := b["data"].(string)
						events = append(events, Event{
							Role:       "assistant",
							Timestamp:  ts,
							Reasoning:  data,
							Redacted:   true,
							ModelUsed:  currentModel,
							SourceTool: piSource,
						})
					case "toolCall":
						id, _ := b["id"].(string)
						name, _ := b["name"].(string)
						events = append(events, Event{
							Role:      "assistant",
							Timestamp: ts,
							ToolCalls: []ToolCall{{ID: id, Name: name, Args: jsonish(b["arguments"])}},
							ModelUsed: currentModel,
							SourceTool: piSource,
						})
					case "image":
						events = append(events, Event{
							Role:       "assistant",
							Content:    "[image]",
							Timestamp:  ts,
							SourceTool: piSource,
						})
					}
				}
			case "toolResult":
				content := concatContentText(msg["content"])
				events = append(events, Event{
					Role:       "tool",
					Content:    content,
					Timestamp:  ts,
					ToolCallID: str(msg, "toolCallId"),
					IsError:    boolValue(msg["isError"]),
					ModelUsed:  currentModel,
					SourceTool: piSource,
				})
			case "bashExecution":
				command, _ := msg["command"].(string)
				output, _ := msg["output"].(string)
				exitCode, _ := msg["exitCode"]
				cancelled, _ := msg["cancelled"].(bool)
				isErr := cancelled || (exitCode != nil && exitCode != float64(0))
				content := fmt.Sprintf("$ %s\n%s", command, output)
				events = append(events, Event{
					Role:       "tool",
					Content:    content,
					Timestamp:  ts,
					IsError:    isErr,
					SourceTool: piSource,
				})
			case "compactionSummary":
				summary, _ := msg["summary"].(string)
				if summary != "" {
					events = append(events, Event{
						Role:       "meta",
						Content:    fmt.Sprintf("compaction_summary: %s", summary),
						Timestamp:  ts,
						SourceTool: piSource,
					})
				}
			case "branchSummary":
				summary, _ := msg["summary"].(string)
				if summary != "" {
					events = append(events, Event{
						Role:       "meta",
						Content:    fmt.Sprintf("branch_summary: %s", summary),
						Timestamp:  ts,
						SourceTool: piSource,
					})
				}
			}

		case "custom_message":
			content := concatContentText(entry["content"])
			if strings.TrimSpace(content) != "" {
				ts := piTimestamp(entry["timestamp"], "")
				events = append(events, Event{
					Role:       "user",
					Content:    content,
					Timestamp:  ts,
					ModelUsed:  currentModel,
					SourceTool: piSource,
				})
			}

		case "model_change":
			modelID, _ := entry["modelId"].(string)
			provider, _ := entry["provider"].(string)
			ts := piTimestamp(entry["timestamp"], "")
			if modelID != "" {
				currentModel = modelID
			}
			events = append(events, Event{
				Role:       "meta",
				Content:    fmt.Sprintf("model_change: %s/%s", provider, modelID),
				Timestamp:  ts,
				ModelUsed:  currentModel,
				SourceTool: piSource,
			})

		case "compaction":
			summary, _ := entry["summary"].(string)
			tokensBefore, _ := entry["tokensBefore"]
			content := fmt.Sprintf("compaction: %v tokens", tokensBefore)
			if summary != "" {
				content = fmt.Sprintf("compaction: %s (%v tokens)", summary, tokensBefore)
			}
			ts := piTimestamp(entry["timestamp"], "")
			events = append(events, Event{
				Role:       "meta",
				Content:    content,
				Timestamp:  ts,
				SourceTool: piSource,
			})

		case "branch_summary":
			summary, _ := entry["summary"].(string)
			fromID, _ := entry["fromId"].(string)
			info := ""
			if fromID != "" {
				info = fmt.Sprintf(" (from %s)", fromID)
			}
			ts := piTimestamp(entry["timestamp"], "")
			events = append(events, Event{
				Role:       "meta",
				Content:    fmt.Sprintf("branch_summary: %s%s", summary, info),
				Timestamp:  ts,
				SourceTool: piSource,
			})

		case "thinking_level_change":
			level, _ := entry["thinkingLevel"].(string)
			ts := piTimestamp(entry["timestamp"], "")
			events = append(events, Event{
				Role:       "meta",
				Content:    fmt.Sprintf("thinking_level: %s", level),
				Timestamp:  ts,
				SourceTool: piSource,
			})

		case "session_info":
			name, _ := entry["name"].(string)
			if name != "" {
				ts := piTimestamp(entry["timestamp"], "")
				events = append(events, Event{
					Role:       "meta",
					Content:    fmt.Sprintf("session_name: %s", name),
					Timestamp:  ts,
					SourceTool: piSource,
				})
			}
		}
	}

	return events, nil
}

// concatContentText joins text blocks from pi's content array into a single string.
// Handles both string content and content block arrays.
func concatContentText(raw interface{}) string {
	switch c := raw.(type) {
	case string:
		return c
	case []interface{}:
		var parts []string
		for _, item := range c {
			block, ok := item.(map[string]interface{})
			if !ok {
				continue
			}
			if str(block, "type") == "text" {
				if text := str(block, "text"); text != "" {
					parts = append(parts, text)
				}
			}
		}
		return strings.Join(parts, "\n")
	default:
		return jsonish(raw)
	}
}

// piUsage extracts usage data from pi's message.usage field.
// Converts numeric values but preserves original field names.
func piUsage(raw interface{}) map[string]int {
	m, ok := raw.(map[string]interface{})
	if !ok {
		return nil
	}
	usage := make(map[string]int)
	for k, v := range m {
		usage[k] = numberAsInt(v)
	}
	if len(usage) == 0 {
		return nil
	}
	// Check if any non-zero value exists
	for _, v := range usage {
		if v > 0 {
			return usage
		}
	}
	return nil
}

// piTimestamp extracts a string or Unix-ms timestamp from a value.
// pi may store timestamps as both ISO strings and Unix ms numbers.
// Uses RFC3339Nano for maximum precision when converting from Unix ms.
func piTimestamp(raw interface{}, fallback string) string {
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
