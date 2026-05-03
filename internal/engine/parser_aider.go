package engine

import (
	"fmt"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
	"time"
)

const aiderHistoryFile = ".aider.chat.history.md"

var (
	aiderStartRE      = regexp.MustCompile(`^# aider chat started at ([0-9]{4}-[0-9]{2}-[0-9]{2} [0-9]{2}:[0-9]{2}:[0-9]{2})`)
	aiderTokensRE     = regexp.MustCompile(`(?i)Tokens:\s*([0-9.,]+k?)\s+sent(?:,\s*([0-9.,]+k?)\s+cache write)?(?:,\s*([0-9.,]+k?)\s+cache hit)?,\s*([0-9.,]+k?)\s+received`)
	aiderModelFlagRE  = regexp.MustCompile(`(?:^|\s)(?:--model|-m)\s+([^\s]+)`)
	aiderModelLabelRE = regexp.MustCompile(`(?i)\bmodel[:=]\s*([A-Za-z0-9._:/@-]+)`)
)

func isAiderHistoryFile(path, content string) bool {
	if filepath.Base(path) == aiderHistoryFile {
		return true
	}
	trimmed := strings.TrimSpace(content)
	if strings.HasPrefix(trimmed, "{") || strings.HasPrefix(trimmed, "[") {
		return false
	}
	return strings.Contains(trimmed, "# aider chat started at") &&
		strings.Contains(trimmed, "#### ")
}

func parseAiderChatHistory(raw string) ([]Event, error) {
	var events []Event
	var role string
	var lines []string
	startTS := ""
	model := "unknown"
	usage := map[string]int{}

	flush := func() {
		if role == "" {
			lines = nil
			return
		}
		content := strings.TrimSpace(strings.Join(lines, "\n"))
		if content != "" {
			events = append(events, Event{
				Role:       role,
				Content:    content,
				Timestamp:  startTS,
				ModelUsed:  model,
				SourceTool: "aider",
			})
		}
		role = ""
		lines = nil
	}

	for _, rawLine := range strings.Split(raw, "\n") {
		line := strings.TrimRight(rawLine, "\r")
		if match := aiderStartRE.FindStringSubmatch(line); len(match) == 2 {
			flush()
			startTS = aiderTime(match[1])
			continue
		}
		if strings.HasPrefix(line, "#### ") {
			text := strings.TrimSpace(strings.TrimPrefix(line, "#### "))
			if role != "user" {
				flush()
				role = "user"
			}
			lines = append(lines, text)
			continue
		}
		if strings.HasPrefix(line, ">") {
			flush()
			text := strings.TrimSpace(strings.TrimPrefix(line, ">"))
			if text == "" {
				continue
			}
			if inferred := inferAiderModel(text); inferred != "" {
				model = inferred
			}
			mergeAiderUsage(usage, text)
			continue
		}
		if strings.TrimSpace(line) == "" {
			if role != "" && len(lines) > 0 {
				lines = append(lines, "")
			}
			continue
		}
		if role != "assistant" {
			flush()
			role = "assistant"
		}
		lines = append(lines, line)
	}
	flush()

	if len(usage) > 0 || model != "unknown" || startTS != "" {
		meta := Event{Role: "meta", Timestamp: startTS, ModelUsed: model, SourceTool: "aider"}
		if len(usage) > 0 {
			meta.Usage = usage
		}
		events = append([]Event{meta}, events...)
	}
	if len(events) == 0 {
		return nil, fmt.Errorf("aider chat history: no parseable events")
	}
	return events, nil
}

func aiderTime(value string) string {
	t, err := time.ParseInLocation("2006-01-02 15:04:05", value, time.Local)
	if err != nil {
		return ""
	}
	return t.Format(time.RFC3339)
}

func inferAiderModel(text string) string {
	for _, re := range []*regexp.Regexp{aiderModelFlagRE, aiderModelLabelRE} {
		if match := re.FindStringSubmatch(text); len(match) == 2 {
			return strings.Trim(match[1], "`'\"")
		}
	}
	return ""
}

func mergeAiderUsage(usage map[string]int, text string) {
	match := aiderTokensRE.FindStringSubmatch(text)
	if len(match) != 5 {
		return
	}
	usage["input_tokens"] = parseAiderTokenCount(match[1])
	usage["cache_creation_input_tokens"] = parseAiderTokenCount(match[2])
	usage["cache_read_input_tokens"] = parseAiderTokenCount(match[3])
	usage["output_tokens"] = parseAiderTokenCount(match[4])
}

func parseAiderTokenCount(value string) int {
	value = strings.TrimSpace(strings.ReplaceAll(value, ",", ""))
	if value == "" {
		return 0
	}
	multiplier := 1.0
	if strings.HasSuffix(strings.ToLower(value), "k") {
		multiplier = 1000
		value = value[:len(value)-1]
	}
	n, err := strconv.ParseFloat(value, 64)
	if err != nil {
		return 0
	}
	return int(n * multiplier)
}
