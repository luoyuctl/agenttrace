package engine

import (
	"database/sql"
	"encoding/json"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"time"

	_ "modernc.org/sqlite"
)

type roleCounts struct {
	user      int
	assistant int
	tool      int
}

type sqliteSessionAgg struct {
	id              string
	title           string
	model           string
	startUnix       float64
	endUnix         float64
	events          int
	userMessages    int
	assistantTurns  int
	toolResults     int
	toolCallsTotal  int
	toolCallsOK     int
	toolCallsFail   int
	inputTokens     int
	outputTokens    int
	cacheReadTokens int
	cacheWriteToken int
	messageTokens   int
	usageCost       float64
	usageCostSet    bool
	sourceTool      string
	path            string
}

func loadSQLiteBackedSessions() []Session {
	home, _ := os.UserHomeDir()
	if home == "" {
		return nil
	}
	var sessions []Session
	sessions = append(sessions, loadHermesSQLiteSessions(hermesStateDBPath(home))...)
	sessions = append(sessions, loadOpenCodeSQLiteSessions(openCodeDBPath(home))...)
	return sessions
}

// LoadSQLiteBackedSessions 返回以本地 SQLite 为权威来源的会话。
func LoadSQLiteBackedSessions() []Session {
	return loadSQLiteBackedSessions()
}

func skipSQLiteBackedFileDir(dir string) bool {
	home, _ := os.UserHomeDir()
	if home == "" {
		return false
	}
	clean := filepath.Clean(dir)
	if sqliteFileExists(hermesStateDBPath(home)) && clean == filepath.Clean(filepath.Join(home, ".hermes", "sessions")) {
		return true
	}
	if sqliteFileExists(openCodeDBPath(home)) && isOpenCodeStorageRoot(clean) {
		return true
	}
	return false
}

func hermesStateDBPath(home string) string {
	return filepath.Join(home, ".hermes", "state.db")
}

func openCodeDBPath(home string) string {
	return filepath.Join(home, ".local", "share", "opencode", "opencode.db")
}

func sqliteFileExists(path string) bool {
	info, err := os.Stat(path)
	return err == nil && !info.IsDir()
}

func openSQLiteReadOnly(path string) (*sql.DB, error) {
	u := url.URL{Scheme: "file", Path: path}
	q := u.Query()
	q.Set("mode", "ro")
	u.RawQuery = q.Encode()
	return sql.Open("sqlite", u.String())
}

func loadHermesSQLiteSessions(path string) []Session {
	if !sqliteFileExists(path) {
		return nil
	}
	db, err := openSQLiteReadOnly(path)
	if err != nil {
		return nil
	}
	defer db.Close()

	roles := sqliteRoleCounts(db, "messages", "session_id", "role")
	rows, err := db.Query(`select id, model, started_at, ended_at, message_count, tool_call_count, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens from sessions`)
	if err != nil {
		return nil
	}
	defer rows.Close()

	var sessions []Session
	for rows.Next() {
		var id string
		var model sql.NullString
		var started sql.NullFloat64
		var ended sql.NullFloat64
		var messageCount, toolCalls, input, output, cacheRead, cacheWrite sql.NullInt64
		if err := rows.Scan(&id, &model, &started, &ended, &messageCount, &toolCalls, &input, &output, &cacheRead, &cacheWrite); err != nil {
			continue
		}
		agg := sqliteSessionAgg{
			id:              id,
			model:           nullString(model, "default"),
			startUnix:       nullFloat(started),
			endUnix:         nullFloat(ended),
			events:          int(nullInt(messageCount)),
			toolCallsTotal:  int(nullInt(toolCalls)),
			toolCallsOK:     int(nullInt(toolCalls)),
			inputTokens:     int(nullInt(input)),
			outputTokens:    int(nullInt(output)),
			cacheReadTokens: int(nullInt(cacheRead)),
			cacheWriteToken: int(nullInt(cacheWrite)),
			sourceTool:      "hermes_db",
			path:            path,
		}
		if rc, ok := roles[id]; ok {
			agg.userMessages = rc.user
			agg.assistantTurns = rc.assistant
			agg.toolResults = rc.tool
		}
		sessions = append(sessions, sessionFromSQLiteAgg(agg))
	}
	return sessions
}

func loadOpenCodeSQLiteSessions(path string) []Session {
	if !sqliteFileExists(path) {
		return nil
	}
	db, err := openSQLiteReadOnly(path)
	if err != nil {
		return nil
	}
	defer db.Close()

	aggs := openCodeSQLiteSessionRows(db, path)
	if len(aggs) == 0 {
		return nil
	}
	addOpenCodeSQLiteMessages(db, aggs)
	addOpenCodeSQLiteParts(db, aggs)

	sessions := make([]Session, 0, len(aggs))
	for _, agg := range aggs {
		if agg.model == "" {
			agg.model = "default"
		}
		if agg.events == 0 {
			agg.events = agg.userMessages + agg.assistantTurns + agg.toolCallsTotal
		}
		sessions = append(sessions, sessionFromSQLiteAgg(*agg))
	}
	return sessions
}

func openCodeSQLiteSessionRows(db *sql.DB, path string) map[string]*sqliteSessionAgg {
	rows, err := db.Query(`select id, title, time_created, time_updated from session`)
	if err != nil {
		return nil
	}
	defer rows.Close()

	out := make(map[string]*sqliteSessionAgg)
	for rows.Next() {
		var id string
		var title sql.NullString
		var created, updated sql.NullInt64
		if err := rows.Scan(&id, &title, &created, &updated); err != nil {
			continue
		}
		out[id] = &sqliteSessionAgg{
			id:         id,
			title:      nullString(title, ""),
			startUnix:  float64(nullInt(created)) / 1000,
			endUnix:    float64(nullInt(updated)) / 1000,
			sourceTool: "opencode_db",
			path:       path,
		}
	}
	return out
}

func addOpenCodeSQLiteMessages(db *sql.DB, aggs map[string]*sqliteSessionAgg) {
	rows, err := db.Query(`select session_id, data from message`)
	if err != nil {
		return
	}
	defer rows.Close()

	for rows.Next() {
		var sessionID, raw string
		if err := rows.Scan(&sessionID, &raw); err != nil {
			continue
		}
		agg := aggs[sessionID]
		if agg == nil {
			continue
		}
		var doc map[string]interface{}
		if err := json.Unmarshal([]byte(raw), &doc); err != nil {
			continue
		}
		agg.events++
		switch str(doc, "role") {
		case "user":
			agg.userMessages++
		case "assistant":
			agg.assistantTurns++
		case "tool":
			agg.toolResults++
		}
		if model := openCodeSQLiteMessageModel(doc); model != "" {
			agg.model = model
		}
		if addOpenCodeSQLiteMessageTokens(agg, doc) {
			agg.messageTokens++
		}
	}
}

func addOpenCodeSQLiteParts(db *sql.DB, aggs map[string]*sqliteSessionAgg) {
	rows, err := db.Query(`select session_id, data from part`)
	if err != nil {
		return
	}
	defer rows.Close()

	for rows.Next() {
		var sessionID, raw string
		if err := rows.Scan(&sessionID, &raw); err != nil {
			continue
		}
		agg := aggs[sessionID]
		if agg == nil {
			continue
		}
		var doc map[string]interface{}
		if err := json.Unmarshal([]byte(raw), &doc); err != nil {
			continue
		}
		switch str(doc, "type") {
		case "step-finish":
			if agg.messageTokens == 0 {
				addOpenCodeStepFinishTokens(agg, doc)
			}
		case "tool":
			agg.toolCallsTotal++
			if openCodeToolFailed(doc) {
				agg.toolCallsFail++
			} else {
				agg.toolCallsOK++
			}
		}
	}
}

func addOpenCodeSQLiteMessageTokens(agg *sqliteSessionAgg, doc map[string]interface{}) bool {
	tokens, ok := doc["tokens"].(map[string]interface{})
	if !ok {
		return false
	}
	input, output, cacheRead, cacheWrite := addOpenCodeTokensFromMap(agg, tokens)
	model := openCodeSQLiteMessageModel(doc)
	if model == "" {
		model = agg.model
	}
	if model != "" {
		agg.usageCost += tokenCostRaw(input, output, cacheWrite, cacheRead, model)
		agg.usageCostSet = true
	}
	return true
}

func addOpenCodeStepFinishTokens(agg *sqliteSessionAgg, doc map[string]interface{}) {
	tokens, ok := doc["tokens"].(map[string]interface{})
	if !ok {
		return
	}
	input, output, cacheRead, cacheWrite := addOpenCodeTokensFromMap(agg, tokens)
	if agg.model != "" {
		agg.usageCost += tokenCostRaw(input, output, cacheWrite, cacheRead, agg.model)
		agg.usageCostSet = true
	}
}

func addOpenCodeTokensFromMap(agg *sqliteSessionAgg, tokens map[string]interface{}) (int, int, int, int) {
	cache, _ := tokens["cache"].(map[string]interface{})
	input := numberAsInt(tokens["input"])
	output := numberAsInt(tokens["output"]) + numberAsInt(tokens["reasoning"])
	cacheRead := numberAsInt(cache["read"])
	cacheWrite := numberAsInt(cache["write"])
	agg.inputTokens += input
	agg.outputTokens += output
	agg.cacheReadTokens += cacheRead
	agg.cacheWriteToken += cacheWrite
	return input, output, cacheRead, cacheWrite
}

func openCodeToolFailed(doc map[string]interface{}) bool {
	state, _ := doc["state"].(map[string]interface{})
	status := strings.ToLower(str(state, "status"))
	return status == "error" || status == "failed" || status == "cancelled"
}

func openCodeSQLiteMessageModel(doc map[string]interface{}) string {
	if model := str(doc, "modelID"); model != "" {
		return model
	}
	if nested, ok := doc["model"].(map[string]interface{}); ok {
		if model := str(nested, "modelID"); model != "" {
			return model
		}
	}
	return ""
}

func sqliteRoleCounts(db *sql.DB, table, sessionColumn, roleColumn string) map[string]roleCounts {
	rows, err := db.Query(`select ` + sessionColumn + `, ` + roleColumn + `, count(*) from ` + table + ` group by ` + sessionColumn + `, ` + roleColumn)
	if err != nil {
		return nil
	}
	defer rows.Close()

	out := make(map[string]roleCounts)
	for rows.Next() {
		var sessionID, role string
		var count int
		if err := rows.Scan(&sessionID, &role, &count); err != nil {
			continue
		}
		rc := out[sessionID]
		switch role {
		case "user":
			rc.user = count
		case "assistant":
			rc.assistant = count
		case "tool":
			rc.tool = count
		}
		out[sessionID] = rc
	}
	return out
}

func sessionFromSQLiteAgg(agg sqliteSessionAgg) Session {
	model := agg.model
	if model == "" {
		model = "default"
	}
	costEstimated := estimateTokenCost(agg.inputTokens, agg.outputTokens, agg.cacheWriteToken, agg.cacheReadTokens, model)
	if agg.usageCostSet {
		costEstimated = round4(agg.usageCost)
	}
	m := Metrics{
		EventsTotal:    agg.events,
		UserMessages:   agg.userMessages,
		AssistantTurns: agg.assistantTurns,
		ToolResults:    agg.toolResults,
		ToolCallsTotal: agg.toolCallsTotal,
		ToolCallsOK:    agg.toolCallsOK,
		ToolCallsFail:  agg.toolCallsFail,
		ToolUsage:      make(map[string]int),
		TokensInput:    agg.inputTokens,
		TokensOutput:   agg.outputTokens,
		TokensCacheW:   agg.cacheWriteToken,
		TokensCacheR:   agg.cacheReadTokens,
		ModelUsed:      model,
		SourceTool:     agg.sourceTool,
		SessionStart:   unixSecondsRFC3339(agg.startUnix),
		SessionEnd:     unixSecondsRFC3339(agg.endUnix),
		CostEstimated:  costEstimated,
	}
	if agg.endUnix > agg.startUnix {
		m.DurationSec = agg.endUnix - agg.startUnix
	}
	anomalies := DetectAnomalies(m)
	name := agg.id
	if agg.title != "" {
		name = agg.title
	}
	return Session{
		Name:      name,
		Path:      agg.path,
		Metrics:   m,
		Anomalies: anomalies,
		Health:    HealthScore(m, anomalies),
	}
}

func estimateTokenCost(input, output, cacheWrite, cacheRead int, model string) float64 {
	return round4(tokenCostRaw(input, output, cacheWrite, cacheRead, model))
}

func tokenCostRaw(input, output, cacheWrite, cacheRead int, model string) float64 {
	pricing := LookupPrice(model)
	return float64(input)/1e6*pricing.Input +
		float64(output)/1e6*pricing.Output +
		float64(cacheWrite)/1e6*pricing.CW +
		float64(cacheRead)/1e6*pricing.CR
}

func unixSecondsRFC3339(v float64) string {
	if v <= 0 {
		return ""
	}
	sec := int64(v)
	nsec := int64((v - float64(sec)) * 1e9)
	return time.Unix(sec, nsec).UTC().Format(time.RFC3339)
}

func nullString(v sql.NullString, fallback string) string {
	if v.Valid && v.String != "" {
		return v.String
	}
	return fallback
}

func nullInt(v sql.NullInt64) int64 {
	if v.Valid {
		return v.Int64
	}
	return 0
}

func nullFloat(v sql.NullFloat64) float64 {
	if v.Valid {
		return v.Float64
	}
	return 0
}
