package engine

import (
	"encoding/json"
	"fmt"
	"strings"
	"time"
)

// parsePi parses pi coding agent JSONL session files.
// pi stores sessions at ~/.pi/agent/sessions/--<path>--/<timestamp>_<uuid>.jsonl.
//
// Format (one JSON object per line):
//
//	{"type":"session","version":3,"id":"uuid","timestamp":"ISO","cwd":"/path",...}
//	{"type":"message","timestamp":"ISO","message":{AgentMessage}}
//	{"type":"model_change",...}
//	{"type":"compaction",...}
//	{"type":"branch_summary",...}
//
// AgentMessage roles: user, assistant, toolResult, bashExecution, custom,
// branchSummary, compactionSummary.
// Content blocks: text, thinking, toolCall (camelCase), image.
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
				SourceTool: "pi",
				Timestamp:  strOrUnixMs(entry, "timestamp"),
			})

		case "message":
			msg, ok := entry["message"].(map[string]interface{})
			if !ok {
				continue
			}
			role, _ := msg["role"].(string)
			ts := strOrUnixMs(entry, "timestamp")

			switch role {
			case "user":
				contentBlocks, _ := msg["content"].([]interface{})
				text := concatTextBlocks(contentBlocks)
				if text != "" {
					events = append(events, Event{
						Role:       "user",
						Content:    text,
						Timestamp:  ts,
						SourceTool: "pi",
					})
				}

			case "assistant":
				contentBlocks, _ := msg["content"].([]interface{})
				model := str(msg, "model")
				if model == "" {
					model = currentModel
				}
				if model != "" {
					currentModel = model
				}

				// Extract usage from assistant message (pi embeds costs here)
				if u, ok := msg["usage"]; ok {
					ev := Event{Role: "meta", ModelUsed: currentModel, SourceTool: "pi", Timestamp: ts}
					ub, _ := json.Marshal(u)
					json.Unmarshal(ub, &ev.Usage)
					events = append(events, ev)
				}

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
								SourceTool: "pi",
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
							SourceTool: "pi",
						})
					case "toolCall":
						id, _ := b["id"].(string)
						name, _ := b["name"].(string)
						events = append(events, Event{
							Role:       "assistant",
							Timestamp:  ts,
							ToolCalls:  []ToolCall{{ID: id, Name: name}},
							ModelUsed:  currentModel,
							SourceTool: "pi",
						})
					case "image":
						events = append(events, Event{
							Role:       role,
							Content:    "[image]",
							Timestamp:  ts,
							SourceTool: "pi",
						})
					}
				}

			case "toolResult":
				contentBlocks, _ := msg["content"].([]interface{})
				text := concatTextBlocks(contentBlocks)
				toolCallID := str(msg, "toolCallId")
				isErr, _ := msg["isError"].(bool)
				events = append(events, Event{
					Role:       "tool",
					Content:    text,
					Timestamp:  ts,
					ToolCallID: toolCallID,
					IsError:    isErr,
					SourceTool: "pi",
				})

			case "bashExecution":
				// Bash execution: record as tool event with command + output
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
					SourceTool: "pi",
				})

			case "compactionSummary":
				summary, _ := msg["summary"].(string)
				if summary != "" {
					events = append(events, Event{
						Role:       "meta",
						Content:    fmt.Sprintf("compaction_summary: %s", summary),
						Timestamp:  ts,
						SourceTool: "pi",
					})
				}

			case "branchSummary":
				summary, _ := msg["summary"].(string)
				if summary != "" {
					events = append(events, Event{
						Role:       "meta",
						Content:    fmt.Sprintf("branch_summary: %s", summary),
						Timestamp:  ts,
						SourceTool: "pi",
					})
				}
			}

		case "model_change":
			modelID, _ := entry["modelId"].(string)
			provider, _ := entry["provider"].(string)
			if modelID != "" {
				currentModel = modelID
			}
			events = append(events, Event{
				Role:       "meta",
				Content:    fmt.Sprintf("model_change: %s/%s", provider, modelID),
				Timestamp:  strOrUnixMs(entry, "timestamp"),
				ModelUsed:  currentModel,
				SourceTool: "pi",
			})

		case "compaction":
			summary, _ := entry["summary"].(string)
			tokensBefore, _ := entry["tokensBefore"]
			content := fmt.Sprintf("compaction: %v tokens", tokensBefore)
			if summary != "" {
				content = fmt.Sprintf("compaction: %s (%v tokens)", summary, tokensBefore)
			}
			events = append(events, Event{
				Role:       "meta",
				Content:    content,
				Timestamp:  strOrUnixMs(entry, "timestamp"),
				SourceTool: "pi",
			})

		case "branch_summary":
			summary, _ := entry["summary"].(string)
			fromID, _ := entry["fromId"].(string)
			info := ""
			if fromID != "" {
				info = fmt.Sprintf(" (from %s)", fromID)
			}
			events = append(events, Event{
				Role:       "meta",
				Content:    fmt.Sprintf("branch_summary: %s%s", summary, info),
				Timestamp:  strOrUnixMs(entry, "timestamp"),
				SourceTool: "pi",
			})

		case "thinking_level_change":
			level, _ := entry["thinkingLevel"].(string)
			events = append(events, Event{
				Role:       "meta",
				Content:    fmt.Sprintf("thinking_level: %s", level),
				Timestamp:  strOrUnixMs(entry, "timestamp"),
				SourceTool: "pi",
			})

		case "session_info":
			name, _ := entry["name"].(string)
			if name != "" {
				events = append(events, Event{
					Role:       "meta",
					Content:    fmt.Sprintf("session_name: %s", name),
					Timestamp:  strOrUnixMs(entry, "timestamp"),
					SourceTool: "pi",
				})
			}
		}
	}

	return events, nil
}

// concatTextBlocks joins text content blocks into a single string.
func concatTextBlocks(blocks []interface{}) string {
	var parts []string
	for _, block := range blocks {
		b, ok := block.(map[string]interface{})
		if !ok {
			continue
		}
		if bt, _ := b["type"].(string); bt == "text" {
			if text, _ := b["text"].(string); text != "" {
				parts = append(parts, text)
			}
		}
	}
	return strings.Join(parts, "\n")
}

// strOrUnixMs extracts a string or Unix-ms timestamp from a map field.
// pi may store timestamps as both ISO strings and Unix ms numbers.
func strOrUnixMs(m map[string]interface{}, key string) string {
	if v, ok := m[key].(string); ok {
		return v
	}
	if v, ok := m[key].(float64); ok {
		return time.UnixMilli(int64(v)).UTC().Format(time.RFC3339)
	}
	return ""
}
