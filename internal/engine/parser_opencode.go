package engine

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"time"
)

type openCodeRecord struct {
	path string
	doc  map[string]interface{}
}

func parseOpenCode(path string, doc map[string]interface{}) ([]Event, error) {
	if isOpenCodeStorageSessionDoc(path, doc) {
		return parseOpenCodeStorageSession(path, doc)
	}
	return parseAnthropicMessageWrapper(doc, "opencode")
}

func openCodeKnownSessionDirs(home string) []KnownSessionDir {
	var dirs []KnownSessionDir
	seen := make(map[string]bool)
	add := func(name, path string) {
		if path == "" || seen[path] {
			return
		}
		seen[path] = true
		dirs = append(dirs, KnownSessionDir{Name: name, Path: path})
	}

	if dataDir := os.Getenv("OPENCODE_DATA_DIR"); dataDir != "" {
		add("OpenCode", dataDir)
	}
	if dataHome := os.Getenv("XDG_DATA_HOME"); dataHome != "" {
		add("OpenCode", filepath.Join(dataHome, "opencode", "storage"))
	} else {
		add("OpenCode", filepath.Join(home, ".local", "share", "opencode", "storage"))
	}
	add("OpenCode macOS", filepath.Join(home, "Library", "Application Support", "opencode", "storage"))
	return dirs
}

func isOpenCodeStoragePath(path string) bool {
	_, ok := openCodeStorageRel(path)
	return ok
}

func isOpenCodeStorageRoot(path string) bool {
	rel, ok := openCodeStorageRel(path)
	return ok && rel == ""
}

func isOpenCodeStorageSessionRoot(path string) bool {
	rel, ok := openCodeStorageRel(path)
	return ok && rel == "session"
}

func isOpenCodeStorageSkippedDir(path string) bool {
	rel, ok := openCodeStorageRel(path)
	if !ok || rel == "" {
		return false
	}
	parts := strings.Split(rel, "/")
	if parts[0] != "session" {
		return true
	}
	return len(parts) > 2
}

func isOpenCodeStorageSessionFile(path string) bool {
	rel, ok := openCodeStorageRel(path)
	if !ok {
		return false
	}
	parts := strings.Split(rel, "/")
	return len(parts) == 3 && parts[0] == "session" && strings.HasSuffix(parts[2], ".json")
}

func isOpenCodeStorageSessionDoc(path string, doc map[string]interface{}) bool {
	if doc == nil || !isOpenCodeStorageSessionFile(path) {
		return false
	}
	return str(doc, "id") != "" && str(doc, "projectID") != ""
}

func openCodeStorageRel(path string) (string, bool) {
	for _, root := range openCodeEnvStorageRoots() {
		if rel, ok := openCodeRelFromRoot(root, path); ok {
			return rel, true
		}
	}

	clean := filepath.ToSlash(filepath.Clean(path))
	marker := "/opencode/storage"
	idx := strings.LastIndex(clean, marker)
	if idx < 0 {
		return "", false
	}
	start := idx + len(marker)
	if len(clean) == start {
		return "", true
	}
	if clean[start] != '/' {
		return "", false
	}
	return strings.Trim(clean[start+1:], "/"), true
}

func openCodeEnvStorageRoots() []string {
	if root := os.Getenv("OPENCODE_DATA_DIR"); root != "" {
		return []string{root}
	}
	return nil
}

func openCodeRelFromRoot(root, path string) (string, bool) {
	absRoot, err := filepath.Abs(root)
	if err != nil {
		return "", false
	}
	absPath, err := filepath.Abs(path)
	if err != nil {
		return "", false
	}
	rel, err := filepath.Rel(absRoot, absPath)
	if err != nil || rel == ".." || strings.HasPrefix(rel, ".."+string(os.PathSeparator)) || filepath.IsAbs(rel) {
		return "", false
	}
	if rel == "." {
		return "", true
	}
	return filepath.ToSlash(rel), true
}

func parseOpenCodeStorageSession(path string, session map[string]interface{}) ([]Event, error) {
	sessionID := str(session, "id")
	if sessionID == "" {
		return nil, fmt.Errorf("opencode: missing session id")
	}
	storageRoot, ok := openCodeStorageRootFromSessionFile(path)
	if !ok {
		return nil, fmt.Errorf("opencode: unsupported storage path %s", path)
	}

	messages, err := readOpenCodeRecords(filepath.Join(storageRoot, "message", sessionID))
	if err != nil || len(messages) == 0 {
		return nil, fmt.Errorf("opencode: no messages found for session %s", sessionID)
	}
	sortOpenCodeRecords(messages)

	model := openCodeSessionModel(session)
	usage := make(map[string]int)
	var body []Event
	for _, msg := range messages {
		if msgModel := openCodeMessageModel(msg.doc); msgModel != "" && model == "unknown" {
			model = msgModel
		}
		messageHadUsage := addOpenCodeTokens(usage, msg.doc["tokens"])
		events, partUsage := parseOpenCodeMessage(storageRoot, msg.doc, model, messageHadUsage)
		addUsage(usage, partUsage)
		body = append(body, events...)
	}
	if len(body) == 0 {
		return nil, fmt.Errorf("opencode: no parseable events for session %s", sessionID)
	}

	var events []Event
	if model != "unknown" || usageHasValues(usage) {
		ev := Event{Role: "meta", ModelUsed: model, SourceTool: "opencode"}
		if usageHasValues(usage) {
			ev.Usage = usage
		}
		events = append(events, ev)
	}
	return append(events, body...), nil
}

func openCodeStorageRootFromSessionFile(path string) (string, bool) {
	if !isOpenCodeStorageSessionFile(path) {
		return "", false
	}
	return filepath.Dir(filepath.Dir(filepath.Dir(path))), true
}

func readOpenCodeRecords(dir string) ([]openCodeRecord, error) {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil, err
	}
	var records []openCodeRecord
	for _, entry := range entries {
		if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".json") {
			continue
		}
		path := filepath.Join(dir, entry.Name())
		data, err := os.ReadFile(path)
		if err != nil {
			continue
		}
		var doc map[string]interface{}
		if err := json.Unmarshal(data, &doc); err != nil {
			continue
		}
		records = append(records, openCodeRecord{path: path, doc: doc})
	}
	return records, nil
}

func sortOpenCodeRecords(records []openCodeRecord) {
	sort.SliceStable(records, func(i, j int) bool {
		ti := openCodeRecordTime(records[i].doc)
		tj := openCodeRecordTime(records[j].doc)
		if !ti.IsZero() && !tj.IsZero() && !ti.Equal(tj) {
			return ti.Before(tj)
		}
		return records[i].path < records[j].path
	})
}

func parseOpenCodeMessage(storageRoot string, msg map[string]interface{}, model string, messageHadUsage bool) ([]Event, map[string]int) {
	role := str(msg, "role")
	ts := openCodeTimeFromMap(msg["time"], "created", "start")
	msgID := str(msg, "id")
	parts, _ := readOpenCodeRecords(filepath.Join(storageRoot, "part", msgID))
	sortOpenCodeRecords(parts)

	var events []Event
	partUsage := make(map[string]int)
	for _, record := range parts {
		part := record.doc
		partType := str(part, "type")
		partTS := openCodePartTimestamp(part, ts)
		switch partType {
		case "text":
			text := str(part, "text")
			if text != "" && (role == "user" || role == "assistant") {
				events = append(events, Event{
					Role: role, Content: text, Timestamp: partTS,
					ModelUsed: model, SourceTool: "opencode",
				})
			}
		case "reasoning":
			text := str(part, "text")
			if text != "" {
				events = append(events, Event{
					Role: "assistant", Reasoning: text, Timestamp: partTS,
					ModelUsed: model, SourceTool: "opencode",
				})
			}
		case "tool":
			events = append(events, openCodeToolEvents(part, partTS, model)...)
		case "step-finish":
			if !messageHadUsage {
				addOpenCodeTokens(partUsage, part["tokens"])
			}
		}
	}
	if len(parts) == 0 {
		if text := str(msg, "content"); text != "" && (role == "user" || role == "assistant") {
			events = append(events, Event{
				Role: role, Content: text, Timestamp: ts,
				ModelUsed: model, SourceTool: "opencode",
			})
		}
	}
	return events, partUsage
}

func openCodeToolEvents(part map[string]interface{}, ts, model string) []Event {
	state, _ := part["state"].(map[string]interface{})
	status := str(state, "status")
	callID := str(part, "callID")
	if callID == "" {
		callID = str(part, "id")
	}
	name := str(part, "tool")
	if name == "" {
		name = str(part, "name")
	}
	input := jsonish(state["input"])
	callTS := openCodeTimeFromMap(state["time"], "start")
	if callTS == "" {
		callTS = ts
	}

	events := []Event{{
		Role: "assistant", Timestamp: callTS,
		ToolCalls: []ToolCall{{ID: callID, Name: name, Args: input}},
		ModelUsed: model, SourceTool: "opencode",
	}}

	output := jsonish(state["output"])
	isErr := status == "error"
	if output == "" {
		output = jsonish(state["error"])
		isErr = isErr || output != ""
	}
	if output != "" {
		resultTS := openCodeTimeFromMap(state["time"], "end")
		if resultTS == "" {
			resultTS = callTS
		}
		events = append(events, Event{
			Role: "tool", Content: output, Timestamp: resultTS,
			ToolCallID: callID, IsError: isErr, SourceTool: "opencode",
		})
	}
	return events
}

func openCodePartTimestamp(part map[string]interface{}, fallback string) string {
	if ts := openCodeTimeFromMap(part["time"], "start", "created"); ts != "" {
		return ts
	}
	if state, ok := part["state"].(map[string]interface{}); ok {
		if ts := openCodeTimeFromMap(state["time"], "start", "created"); ts != "" {
			return ts
		}
	}
	return fallback
}

func openCodeRecordTime(doc map[string]interface{}) time.Time {
	if t := openCodeTimeValue(doc["time"], "start", "created"); !t.IsZero() {
		return t
	}
	if state, ok := doc["state"].(map[string]interface{}); ok {
		return openCodeTimeValue(state["time"], "start", "created")
	}
	return time.Time{}
}

func openCodeTimeFromMap(raw interface{}, keys ...string) string {
	t := openCodeTimeValue(raw, keys...)
	if t.IsZero() {
		return ""
	}
	return t.UTC().Format(time.RFC3339Nano)
}

func openCodeTimeValue(raw interface{}, keys ...string) time.Time {
	m, ok := raw.(map[string]interface{})
	if !ok {
		return openCodeParseTime(raw)
	}
	for _, key := range keys {
		if t := openCodeParseTime(m[key]); !t.IsZero() {
			return t
		}
	}
	return time.Time{}
}

func openCodeParseTime(raw interface{}) time.Time {
	switch v := raw.(type) {
	case float64:
		return openCodeUnixTime(v)
	case int64:
		return openCodeUnixTime(float64(v))
	case int:
		return openCodeUnixTime(float64(v))
	case json.Number:
		f, _ := v.Float64()
		return openCodeUnixTime(f)
	case string:
		if t := parseTS(v); !t.IsZero() {
			return t
		}
		f, err := strconv.ParseFloat(v, 64)
		if err == nil {
			return openCodeUnixTime(f)
		}
	}
	return time.Time{}
}

func openCodeUnixTime(v float64) time.Time {
	if v <= 0 {
		return time.Time{}
	}
	if v > 1e12 {
		return time.UnixMilli(int64(v))
	}
	return time.Unix(int64(v), 0)
}

func openCodeSessionModel(session map[string]interface{}) string {
	if model, ok := session["model"].(map[string]interface{}); ok {
		if id := str(model, "id"); id != "" {
			return id
		}
		if id := str(model, "modelID"); id != "" {
			return id
		}
	}
	return "unknown"
}

func openCodeMessageModel(msg map[string]interface{}) string {
	if model := str(msg, "modelID"); model != "" {
		return model
	}
	if model := str(msg, "model"); model != "" {
		return model
	}
	return ""
}

func addOpenCodeTokens(usage map[string]int, raw interface{}) bool {
	tokens, ok := raw.(map[string]interface{})
	if !ok {
		return false
	}
	addUsageValue(usage, "input_tokens", tokens["input"])
	addUsageValue(usage, "output_tokens", tokens["output"])
	if cache, ok := tokens["cache"].(map[string]interface{}); ok {
		addUsageValue(usage, "cache_read_input_tokens", cache["read"])
		addUsageValue(usage, "cache_creation_input_tokens", cache["write"])
	}
	return true
}

func addUsage(dst, src map[string]int) {
	for k, v := range src {
		dst[k] += v
	}
}

func addUsageValue(usage map[string]int, key string, raw interface{}) {
	if n := numberAsInt(raw); n > 0 {
		usage[key] += n
	}
}

func usageHasValues(usage map[string]int) bool {
	for _, v := range usage {
		if v > 0 {
			return true
		}
	}
	return false
}
