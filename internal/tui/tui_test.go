package tui

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/luoyuctl/agenttrace/internal/engine"
	"github.com/luoyuctl/agenttrace/internal/i18n"
)

func sampleModelForTest() Model {
	sessions := []engine.Session{
		{
			Name:   "session_alpha",
			Health: 92,
			Metrics: engine.Metrics{
				AssistantTurns: 8, ToolCallsTotal: 12, ToolCallsOK: 11, ToolCallsFail: 1,
				TokensInput: 120000, TokensOutput: 34000, TokensCacheR: 60000,
				CostEstimated: 0.42, SourceTool: "codex_cli", ModelUsed: "gpt-5.1",
				GapsSec: []float64{0.2, 0.5, 1.2, 2.4},
			},
		},
		{
			Name:   "session_beta_with_a_long_name",
			Health: 64,
			Metrics: engine.Metrics{
				AssistantTurns: 15, ToolCallsTotal: 30, ToolCallsOK: 23, ToolCallsFail: 7,
				TokensInput: 260000, TokensOutput: 91000, TokensCacheR: 110000,
				CostEstimated: 1.25, SourceTool: "claude_code", ModelUsed: "claude-sonnet-4",
				GapsSec: []float64{1.0, 3.5, 12.0, 22.0},
			},
			Anomalies: []engine.Anomaly{{Type: "tool_failures", Severity: engine.SeverityHigh, Emoji: "!"}},
		},
		{
			Name:   "gamma",
			Health: 38,
			Metrics: engine.Metrics{
				AssistantTurns: 4, ToolCallsTotal: 5, ToolCallsOK: 2, ToolCallsFail: 3,
				TokensInput: 70000, TokensOutput: 22000,
				CostEstimated: 0.18, SourceTool: "gemini_cli", ModelUsed: "gemini-2.5-pro",
				GapsSec: []float64{4.0, 40.0},
			},
			Anomalies: []engine.Anomaly{{Type: "hanging", Severity: engine.SeverityHigh, Emoji: "!"}},
		},
	}
	m := Model{
		view:        viewOverview,
		sessions:    sessions,
		overview:    engine.ComputeOverview(sessions),
		aggStats:    engine.ComputeAggregateStats(sessions),
		costSummary: engine.ComputeCostSummary(sessions),
		tableReady:  true,
	}
	base := New("__missing_test_sessions__")
	m.table = base.table
	m.refreshTable()
	m.rebuildFilteredIndices()
	return m
}

func resizeForTest(t *testing.T, m Model, width, height int) Model {
	t.Helper()
	next, _ := m.Update(tea.WindowSizeMsg{Width: width, Height: height})
	got, ok := next.(Model)
	if !ok {
		t.Fatalf("unexpected model type %T", next)
	}
	return got
}

func pressForTest(t *testing.T, m Model, key string) Model {
	t.Helper()
	next, _ := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune(key)})
	if key == "tab" {
		next, _ = m.Update(tea.KeyMsg{Type: tea.KeyTab})
	}
	got, ok := next.(Model)
	if !ok {
		t.Fatalf("unexpected model type %T", next)
	}
	return got
}

func maxRenderedWidth(s string) int {
	maxW := 0
	for _, line := range strings.Split(s, "\n") {
		if w := lipgloss.Width(line); w > maxW {
			maxW = w
		}
	}
	return maxW
}

func widestLine(s string) string {
	var widest string
	maxW := 0
	for _, line := range strings.Split(s, "\n") {
		if w := lipgloss.Width(line); w > maxW {
			maxW = w
			widest = line
		}
	}
	return widest
}

func renderedHeight(s string) int {
	if s == "" {
		return 0
	}
	return len(strings.Split(s, "\n"))
}

func TestViewsRenderWithinTerminalWidth(t *testing.T) {
	for _, width := range []int{40, 52, 60, 72, 80, 100, 140} {
		m := resizeForTest(t, sampleModelForTest(), width, 36)
		views := []view{viewOverview, viewList, viewDetail, viewDiagnostics, viewDiff}
		for _, v := range views {
			m.view = v
			if v == viewDetail || v == viewDiagnostics {
				m.openDetail()
			}
			if v == viewDiff {
				m.diffResult = engine.DiffSessions(m.sessions[0], m.sessions[1])
			}
			rendered := m.View()
			if rendered == "" {
				t.Fatalf("empty render for width=%d view=%d", width, v)
			}
			if got := maxRenderedWidth(rendered); got > width {
				t.Fatalf("render too wide: width=%d view=%d got=%d line=%q", width, v, got, widestLine(rendered))
			}
		}
	}
}

func TestFilterSortKeepsTableColumnShape(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewList
	m.filterMode = "health"
	m.filterValue = "good"
	m.applyFilter()
	m.sortBy = "cost"
	m.sortDesc = true
	m.sortAndRefresh()
	for i, row := range m.table.Rows() {
		if len(row) != len(m.table.Columns()) {
			t.Fatalf("row %d has %d cells, columns=%d", i, len(row), len(m.table.Columns()))
		}
	}
}

func TestTextFilterKeepsRowsAndSelectionInSync(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewList
	m.filterText = "beta"
	m.rebuildFilteredView()

	if got := len(m.table.Rows()); got != 1 {
		t.Fatalf("expected one visible row, got %d", got)
	}
	idx := m.findSessionIndex()
	if idx < 0 || m.sessions[idx].Name != "session_beta_with_a_long_name" {
		t.Fatalf("selection mapped to wrong session: idx=%d", idx)
	}
}

func TestEmptyTextFilterDoesNotRestoreAllRows(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewList
	m.filterText = "no-such-session"
	m.rebuildFilteredView()

	if got := len(m.table.Rows()); got != 0 {
		t.Fatalf("expected no visible rows, got %d", got)
	}
	if got := m.findSessionIndex(); got != -1 {
		t.Fatalf("expected no selected session, got %d", got)
	}
}

func TestCombinedFiltersUseSingleIndexSet(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewList
	m.filterMode = "health"
	m.filterValue = "good"
	m.filterText = "beta"
	m.rebuildFilteredView()

	if got := len(m.table.Rows()); got != 0 {
		t.Fatalf("expected health+text filter to produce no rows, got %d", got)
	}
}

func TestFilterInputAllowsQCharacter(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewList
	next, _ := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune("/")})
	m = next.(Model)
	next, cmd := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune("q")})
	m = next.(Model)

	if cmd != nil {
		t.Fatalf("typing q in filter should not quit")
	}
	if !m.filterActive || m.filterInput != "q" {
		t.Fatalf("expected q to be typed into filter, active=%v input=%q", m.filterActive, m.filterInput)
	}
}

func TestTabIntoDiffPreparesComparison(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewDiagnostics
	m.rebuildFilteredView()

	next, _ := m.Update(tea.KeyMsg{Type: tea.KeyTab})
	m = next.(Model)

	if m.view != viewDiff {
		t.Fatalf("expected diff view, got %d", m.view)
	}
	if len(m.diffResult.Entries) == 0 {
		t.Fatalf("expected prepared diff entries")
	}
}

func TestCompactListKeepsTokensAndHealthReadable(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 80, 30)
	m.view = viewList

	if got := len(m.table.Columns()); got != 5 {
		t.Fatalf("expected compact columns, got %d", got)
	}
	row := m.table.Rows()[0]
	if row[3] != "154.0K" {
		t.Fatalf("expected full compact token value, got %q", row[3])
	}
	if row[4] != "92%" {
		t.Fatalf("expected health value, got %q", row[4])
	}
}

func TestWideListKeepsFullOperationalColumns(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 160, 36)
	m.view = viewList
	if got := len(m.table.Columns()); got != 9 {
		t.Fatalf("expected full columns on wide screens, got %d", got)
	}
	row := m.table.Rows()[0]
	if row[2] != "8" || row[3] != "12" || row[4] != "92" || row[5] != "1" {
		t.Fatalf("expected turns/tools/success/fail columns, got row=%v", row)
	}
}

func TestLanguageSwitchKeepsFilteredRows(t *testing.T) {
	prev := i18n.Current
	i18n.SetLang(i18n.EN)
	t.Cleanup(func() { i18n.SetLang(prev) })

	m := resizeForTest(t, sampleModelForTest(), 80, 30)
	m.view = viewList
	m.filterText = "beta"
	m.rebuildFilteredView()

	next, _ := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune("l")})
	m = next.(Model)

	if got := len(m.table.Rows()); got != 1 {
		t.Fatalf("expected language switch to preserve filtered rows, got %d", got)
	}
	if idx := m.findSessionIndex(); idx < 0 || m.sessions[idx].Name != "session_beta_with_a_long_name" {
		t.Fatalf("language switch changed selection mapping: idx=%d", idx)
	}
}

func TestChineseListUsesTranslatedLabels(t *testing.T) {
	prev := i18n.Current
	i18n.SetLang(i18n.ZH)
	t.Cleanup(func() { i18n.SetLang(prev) })

	m := resizeForTest(t, sampleModelForTest(), 120, 30)
	m.view = viewList
	m.runCommand("anomalies")

	rendered := m.View()
	for _, want := range []string{"会话", "筛选", "异常"} {
		if !strings.Contains(rendered, want) {
			t.Fatalf("expected translated label %q in list view", want)
		}
	}
}

func TestChineseTUITranslatesRuntimeLabels(t *testing.T) {
	prev := i18n.Current
	i18n.SetLang(i18n.ZH)
	t.Cleanup(func() { i18n.SetLang(prev) })

	m := resizeForTest(t, sampleModelForTest(), 140, 36)
	m.view = viewOverview
	overview := m.View()
	for _, unwanted := range []string{"sessions", "Needs attention"} {
		if strings.Contains(overview, unwanted) {
			t.Fatalf("overview leaked English label %q:\n%s", unwanted, overview)
		}
	}
	if !strings.Contains(overview, "会话") || !strings.Contains(overview, "需要关注") {
		t.Fatalf("overview missing Chinese runtime labels:\n%s", overview)
	}

	m.view = viewList
	m.table.SetCursor(1)
	list := m.View()
	if strings.Contains(list, "tool failures") {
		t.Fatalf("list leaked English anomaly label:\n%s", list)
	}
	if !strings.Contains(list, "工具失败") {
		t.Fatalf("list missing translated anomaly label:\n%s", list)
	}
}

func TestChineseDiffAndDiagnosticsTranslateComputedLabels(t *testing.T) {
	prev := i18n.Current
	i18n.SetLang(i18n.ZH)
	t.Cleanup(func() { i18n.SetLang(prev) })

	if got := diffFieldLabel("success_rate"); got != "成功率" {
		t.Fatalf("expected translated diff field, got %q", got)
	}
	if got := sortFieldLabel("health"); got != "健康" {
		t.Fatalf("expected translated sort field, got %q", got)
	}
	if got := anomalyTypeLabel("shallow_thinking"); got != "浅层思考" {
		t.Fatalf("expected translated anomaly type, got %q", got)
	}
	if got := riskLabel("high"); got != "高" {
		t.Fatalf("expected translated risk label, got %q", got)
	}
}

func TestWideListRendersStableSingleTable(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 160, 36)
	m.view = viewList
	rendered := m.View()
	for _, stale := range []string{"SESSION INSPECTOR", "Enter detail · d diff · w diagnostics"} {
		if strings.Contains(rendered, stale) {
			t.Fatalf("list should not render stale split preview fragment %q", stale)
		}
	}
	if !strings.Contains(rendered, "session_alpha") || !strings.Contains(rendered, "issue") {
		t.Fatalf("expected stable list table with selected summary")
	}
	if got := maxRenderedWidth(rendered); got > 160 {
		t.Fatalf("wide list render too wide: got=%d line=%q", got, widestLine(rendered))
	}
}

func TestListFrameClearsFullTerminalWidth(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 180, 40)
	m.view = viewList

	for i, line := range strings.Split(m.View(), "\n") {
		if got := lipgloss.Width(line); got != 180 {
			t.Fatalf("line %d should fill terminal width, got=%d line=%q", i, got, line)
		}
	}
}

func TestListViewFitsTerminalHeight(t *testing.T) {
	for _, height := range []int{24, 30, 62} {
		m := resizeForTest(t, sampleModelForTest(), 160, height)
		m.view = viewList
		rendered := m.View()
		if got := renderedHeight(rendered); got > height {
			t.Fatalf("list render too tall: height=%d got=%d", height, got)
		}
	}
}

func TestFooterRemainsVisibleWhenOverviewIsTall(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 80, 24)
	m.view = viewOverview

	rendered := m.View()
	if !strings.Contains(rendered, "Overview") || !strings.Contains(rendered, "$: top cost") {
		t.Fatalf("expected footer help to remain visible in clipped overview:\n%s", rendered)
	}
	if got := renderedHeight(rendered); got != 24 {
		t.Fatalf("expected render to fill terminal height, got %d", got)
	}
}

func TestCommandModeFiltersAndSorts(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewList
	m.runCommand("cost >0.5")

	if got := len(m.table.Rows()); got != 1 {
		t.Fatalf("expected one high-cost row, got %d", got)
	}
	idx := m.findSessionIndex()
	if idx < 0 || m.sessions[idx].Name != "session_beta_with_a_long_name" {
		t.Fatalf("expected beta after cost command, idx=%d", idx)
	}

	m.runCommand("sort health asc")
	if m.sortBy != "health" || m.sortDesc {
		t.Fatalf("sort command not applied: sortBy=%q desc=%v", m.sortBy, m.sortDesc)
	}
}

func TestCommandModeAcceptsSpacedNumericExpressions(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewList

	m.runCommand("cost > 0.5")
	if got := len(m.table.Rows()); got != 1 {
		t.Fatalf("expected spaced cost command to match one row, got %d", got)
	}
	if idx := m.findSessionIndex(); idx < 0 || m.sessions[idx].Name != "session_beta_with_a_long_name" {
		t.Fatalf("spaced cost command selected wrong session: idx=%d", idx)
	}

	m.runCommand("clear")
	m.runCommand("health >= 80")
	if got := len(m.table.Rows()); got != 1 {
		t.Fatalf("expected spaced health command to match one row, got %d", got)
	}
	if idx := m.findSessionIndex(); idx < 0 || m.sessions[idx].Name != "session_alpha" {
		t.Fatalf("spaced health command selected wrong session: idx=%d", idx)
	}

	m.runCommand("clear")
	m.runCommand("health CRITICAL")
	if got := len(m.table.Rows()); got != 1 {
		t.Fatalf("expected normalized critical command to match one row, got %d", got)
	}
	if idx := m.findSessionIndex(); idx < 0 || m.sessions[idx].Name != "gamma" {
		t.Fatalf("critical command selected wrong session: idx=%d", idx)
	}
}

func TestSortCommandSupportsSource(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewList
	m.runCommand("sort source asc")

	if m.sortBy != "source" || m.sortDesc {
		t.Fatalf("source sort command not applied: sortBy=%q desc=%v", m.sortBy, m.sortDesc)
	}
	if got := len(m.table.Rows()); got != len(m.sessions) {
		t.Fatalf("source sort lost rows: got %d", got)
	}
}

func TestCommandClearResetsAdvancedFilters(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewList
	m.runCommand("model claude")
	m.runCommand("anomalies")
	m.runCommand("clear")

	if m.hasAnyFilter() {
		t.Fatalf("expected all filters cleared: %s", m.filterLabel())
	}
	if got := len(m.table.Rows()); got != len(m.sessions) {
		t.Fatalf("expected all rows after clear, got %d", got)
	}
}

func TestFilterEscClearsExistingFilters(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewList
	m.filterText = "beta"
	m.rebuildFilteredView()

	next, _ := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune("/")})
	m = next.(Model)
	next, _ = m.Update(tea.KeyMsg{Type: tea.KeyEsc})
	m = next.(Model)

	if m.hasAnyFilter() || m.filterActive || m.filterInput != "" {
		t.Fatalf("expected Esc to clear filter state: active=%v input=%q label=%s", m.filterActive, m.filterInput, m.filterLabel())
	}
	if got := len(m.table.Rows()); got != len(m.sessions) {
		t.Fatalf("expected all rows after Esc clear, got %d", got)
	}
}

func TestLongCommandAndFilterBarsFitTerminalWidth(t *testing.T) {
	long := strings.Repeat("agenttrace-", 30)
	tests := []struct {
		name  string
		setup func(*Model)
	}{
		{
			name: "applied filter",
			setup: func(m *Model) {
				m.filterText = long
				m.rebuildFilteredView()
			},
		},
		{
			name: "filter input",
			setup: func(m *Model) {
				m.filterActive = true
				m.filterInput = long
			},
		},
		{
			name: "command input",
			setup: func(m *Model) {
				m.commandActive = true
				m.commandInput = long
			},
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			m := resizeForTest(t, sampleModelForTest(), 80, 30)
			m.view = viewList
			tt.setup(&m)
			rendered := m.View()
			if got := maxRenderedWidth(rendered); got > 80 {
				t.Fatalf("render too wide: got=%d line=%q", got, widestLine(rendered))
			}
		})
	}
}

func TestOverviewShortcutsOpenWorkbenchLists(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewOverview

	next, _ := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune("$")})
	m = next.(Model)

	if m.view != viewList || m.sortBy != "cost" || !m.sortDesc {
		t.Fatalf("top cost shortcut failed: view=%d sort=%q desc=%v", m.view, m.sortBy, m.sortDesc)
	}

	m.view = viewOverview
	next, _ = m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune("!")})
	m = next.(Model)
	if m.view != viewList || m.filterHealth != "crit" {
		t.Fatalf("critical shortcut failed: view=%d filterHealth=%q", m.view, m.filterHealth)
	}
}

func TestDetailRendersDiagnosticSummary(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 36)
	m.view = viewList
	m.table.SetCursor(1)
	m.openDetail()
	m.view = viewDetail

	rendered := m.View()
	if !strings.Contains(rendered, i18n.T("insight_primary_issue")) {
		t.Fatalf("expected diagnostic summary, got %q", rendered)
	}
}

func TestDiffRendersWinnerInsight(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 36)
	m.diffResult = engine.DiffSessions(m.sessions[0], m.sessions[1])
	m.view = viewDiff

	rendered := m.View()
	if !strings.Contains(rendered, "Winner:") {
		t.Fatalf("expected diff winner insight")
	}
}

func TestWideDiffUsesFullComparisonLayout(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 160, 36)
	m.diffResult = engine.DiffSessions(m.sessions[0], m.sessions[1])
	m.view = viewDiff

	rendered := m.View()
	for _, want := range []string{"COMPARISON", "A  session_alpha", "B  session_beta"} {
		if !strings.Contains(rendered, want) {
			t.Fatalf("expected %q in wide diff layout", want)
		}
	}
	if got := maxRenderedWidth(rendered); got > 160 {
		t.Fatalf("wide diff render too wide: got=%d line=%q", got, widestLine(rendered))
	}
}

func TestAppendSessionKeepsCacheMetadata(t *testing.T) {
	m := resizeForTest(t, New("__missing_test_sessions__"), 100, 30)
	m.loading = false
	m.sessionCache = engine.SessionCache{Entries: map[string]engine.CacheEntry{
		"/tmp/session.jsonl": {ModTime: 123, Size: 456},
	}}

	s := engine.Session{Name: "cached", Path: "/tmp/session.jsonl", Health: 90}
	m.appendSession(s, false)

	entry := m.sessionCache.Entries[s.Path]
	if entry.ModTime != 123 || entry.Size != 456 {
		t.Fatalf("cache metadata was overwritten: %+v", entry)
	}
}

func TestStartReloadHydratesCachedSessionsWithoutQueue(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "cached.jsonl")
	if err := os.WriteFile(path, []byte("{}\n"), 0644); err != nil {
		t.Fatal(err)
	}
	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}

	m := resizeForTest(t, New(dir), 100, 30)
	m.sessionCache = engine.SessionCache{Entries: map[string]engine.CacheEntry{
		path: {
			ModTime: info.ModTime().UnixNano(),
			Size:    info.Size(),
			Session: engine.Session{Name: "cached", Path: path, Health: 91},
		},
	}, Dirs: map[string]engine.DirCacheEntry{}}

	cmd := m.startReload()
	if cmd != nil {
		t.Fatalf("expected no load command when all sessions are cached")
	}
	if m.loading {
		t.Fatalf("expected loading to finish immediately")
	}
	if len(m.sessions) != 1 || m.sessions[0].Name != "cached" {
		t.Fatalf("cached sessions not hydrated: %+v", m.sessions)
	}
	if m.loadedFromCache != 1 || len(m.loadQueue) != 0 {
		t.Fatalf("bad cache load state: fromCache=%d queue=%d", m.loadedFromCache, len(m.loadQueue))
	}
}

func TestStartLoadMessageKeepsReloadedModelState(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "cached.jsonl")
	if err := os.WriteFile(path, []byte("{}\n"), 0644); err != nil {
		t.Fatal(err)
	}
	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}

	m := resizeForTest(t, New(dir), 100, 30)
	m.sessionCache = engine.SessionCache{Entries: map[string]engine.CacheEntry{
		path: {
			ModTime: info.ModTime().UnixNano(),
			Size:    info.Size(),
			Session: engine.Session{Name: "cached", Path: path, Health: 91},
		},
	}, Dirs: map[string]engine.DirCacheEntry{}}

	next, _ := m.Update(startLoadMsg{})
	got := next.(Model)
	if got.loadProgress != 1 || got.loadedFromCache != 1 || got.loadTotal != 1 || got.loading {
		t.Fatalf("startLoadMsg lost reloaded state: progress=%d fromCache=%d total=%d loading=%v",
			got.loadProgress, got.loadedFromCache, got.loadTotal, got.loading)
	}
}

func TestStartReloadDoesNotHydrateStaleCachedSessions(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "stale.jsonl")
	if err := os.WriteFile(path, []byte("{}\n"), 0644); err != nil {
		t.Fatal(err)
	}
	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}

	m := resizeForTest(t, New(dir), 100, 30)
	m.sessionCache = engine.SessionCache{Entries: map[string]engine.CacheEntry{
		path: {
			ModTime: info.ModTime().Add(-time.Hour).UnixNano(),
			Size:    info.Size(),
			Session: engine.Session{Name: "stale", Path: path, Health: 91},
		},
	}, Dirs: map[string]engine.DirCacheEntry{}}

	cmd := m.startReload()
	if cmd == nil {
		t.Fatalf("expected stale file to be queued for parsing")
	}
	if len(m.sessions) != 0 || m.loadedFromCache != 0 || len(m.loadQueue) != 1 {
		t.Fatalf("stale cache should not hydrate: sessions=%d fromCache=%d queue=%d", len(m.sessions), m.loadedFromCache, len(m.loadQueue))
	}
}

func TestDetailViewFitsTerminalHeight(t *testing.T) {
	for _, height := range []int{24, 30, 36} {
		m := resizeForTest(t, sampleModelForTest(), 80, height)
		m.view = viewList
		m.openDetail()
		m.view = viewDetail
		rendered := m.View()
		if got := renderedHeight(rendered); got > height {
			t.Fatalf("detail render too tall: height=%d got=%d", height, got)
		}
	}
}

func TestEmptyModelRendersAllViews(t *testing.T) {
	m := resizeForTest(t, New("__missing_test_sessions__"), 80, 24)
	for _, v := range []view{viewOverview, viewList, viewDetail, viewDiagnostics, viewDiff} {
		m.view = v
		if got := m.View(); got == "" {
			t.Fatalf("empty render for view=%d", v)
		}
	}
}

func TestLoadingRenderWithinTerminalWidth(t *testing.T) {
	for _, width := range []int{40, 52, 60, 80} {
		m := resizeForTest(t, New("__missing_test_sessions__"), width, 24)
		m.loading = true
		m.loadProgress = 3
		m.loadTotal = 10
		rendered := m.View()
		if got := maxRenderedWidth(rendered); got > width {
			t.Fatalf("loading render too wide: width=%d got=%d line=%q", width, got, widestLine(rendered))
		}
	}
}

func TestChineseViewsRenderWithinTerminalWidth(t *testing.T) {
	prev := i18n.Current
	i18n.SetLang(i18n.ZH)
	t.Cleanup(func() { i18n.SetLang(prev) })

	for _, width := range []int{40, 52, 60, 80} {
		m := resizeForTest(t, sampleModelForTest(), width, 36)
		m.lang = i18n.ZH
		m.refreshColumns()
		for _, v := range []view{viewOverview, viewList, viewDetail, viewDiagnostics, viewDiff} {
			m.view = v
			if v == viewDetail || v == viewDiagnostics {
				m.openDetail()
			}
			if v == viewDiff {
				m.diffResult = engine.DiffSessions(m.sessions[0], m.sessions[1])
			}
			rendered := m.View()
			if got := maxRenderedWidth(rendered); got > width {
				t.Fatalf("zh render too wide: width=%d view=%d got=%d line=%q", width, v, got, widestLine(rendered))
			}
		}
	}
}
