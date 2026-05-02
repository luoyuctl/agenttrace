package tui

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
	"unicode/utf8"

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
	for _, width := range []int{32, 40, 52, 60, 72, 80, 100, 140} {
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

func TestViewsFitVerySmallTerminalBounds(t *testing.T) {
	for _, width := range []int{1, 2, 3, 5, 8, 13, 21, 31, 39} {
		for _, height := range []int{1, 2, 3, 5, 8, 12} {
			m := resizeForTest(t, sampleModelForTest(), width, height)
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
					t.Fatalf("tiny render too wide: width=%d height=%d view=%d got=%d line=%q", width, height, v, got, widestLine(rendered))
				}
				if got := renderedHeight(rendered); got > height {
					t.Fatalf("tiny render too tall: width=%d height=%d view=%d got=%d", width, height, v, got)
				}
			}
		}
	}
}

func TestAppHeaderDoesNotWrapStatusBadge(t *testing.T) {
	for _, width := range []int{72, 80, 96, 100} {
		m := resizeForTest(t, sampleModelForTest(), width, 24)
		header := m.renderAppHeader()
		lines := strings.Split(header, "\n")
		if got := len(lines); got != 2 {
			t.Fatalf("expected two header lines at width=%d, got %d:\n%s", width, got, header)
		}
		for i, line := range lines {
			if got := lipgloss.Width(line); got > width {
				t.Fatalf("header line %d too wide at width=%d: got=%d line=%q", i, width, got, line)
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

func TestSortPreservesSelectedSession(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewList
	m.table.SetCursor(1)
	if idx := m.findSessionIndex(); idx < 0 || m.sessions[idx].Name != "session_beta_with_a_long_name" {
		t.Fatalf("test setup selected wrong session: idx=%d", idx)
	}

	m.sortBy = "name"
	m.sortDesc = false
	m.sortAndRefresh()

	idx := m.findSessionIndex()
	if idx < 0 || m.sessions[idx].Name != "session_beta_with_a_long_name" {
		t.Fatalf("sort should preserve selected session, idx=%d", idx)
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

func TestTextFilterPreservesVisibleSelection(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewList
	m.table.SetCursor(1)

	m.runCommand("session")

	if got := len(m.table.Rows()); got != 2 {
		t.Fatalf("expected two matching rows, got %d", got)
	}
	if idx := m.findSessionIndex(); idx < 0 || m.sessions[idx].Name != "session_beta_with_a_long_name" {
		t.Fatalf("filter should preserve visible selected session, idx=%d", idx)
	}
}

func TestFinishLoadingKeepsFilteredTableRows(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewList
	m.filterText = "beta"
	m.finishLoading()

	if got := len(m.filteredIndices); got != 1 {
		t.Fatalf("expected one filtered index after finishLoading, got %d", got)
	}
	if got := len(m.table.Rows()); got != 1 {
		t.Fatalf("finishLoading should keep table rows filtered, got %d", got)
	}
	if idx := m.findSessionIndex(); idx < 0 || m.sessions[idx].Name != "session_beta_with_a_long_name" {
		t.Fatalf("filtered table selected wrong session after finishLoading: idx=%d", idx)
	}
}

func TestEmptyTextFilterDoesNotRestoreAllRows(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewList
	m.filterText = "no-such-session"
	m.rebuildFilteredView()
	m.table.SetCursor(0)

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

func TestFilterBackspaceRemovesWholeRune(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewList
	next, _ := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune("/")})
	m = next.(Model)
	next, _ = m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune("测试")})
	m = next.(Model)
	next, _ = m.Update(tea.KeyMsg{Type: tea.KeyBackspace})
	m = next.(Model)

	if m.filterInput != "测" {
		t.Fatalf("expected backspace to remove one rune, got %q", m.filterInput)
	}
	if !utf8.ValidString(m.filterInput) {
		t.Fatalf("filter input became invalid UTF-8: %q", m.filterInput)
	}
}

func TestCommandBackspaceRemovesWholeRune(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	next, _ := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune(":")})
	m = next.(Model)
	next, _ = m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune("费用")})
	m = next.(Model)
	next, _ = m.Update(tea.KeyMsg{Type: tea.KeyBackspace})
	m = next.(Model)

	if m.commandInput != "费" {
		t.Fatalf("expected backspace to remove one rune, got %q", m.commandInput)
	}
	if !utf8.ValidString(m.commandInput) {
		t.Fatalf("command input became invalid UTF-8: %q", m.commandInput)
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

func TestLanguageSwitchRefreshesDetailViewport(t *testing.T) {
	prev := i18n.Current
	i18n.SetLang(i18n.EN)
	t.Cleanup(func() { i18n.SetLang(prev) })

	m := resizeForTest(t, sampleModelForTest(), 120, 80)
	m.lang = i18n.EN
	m.view = viewList
	m.openDetail()
	m.view = viewDetail
	if !strings.Contains(m.View(), "Money wasted") {
		t.Fatalf("expected English detail content before language switch")
	}

	next, _ := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune("l")})
	m = next.(Model)

	rendered := m.View()
	if !strings.Contains(rendered, "烧掉的钱") {
		t.Fatalf("expected detail viewport to rerender in Chinese:\n%s", rendered)
	}
	if strings.Contains(rendered, "Money wasted") {
		t.Fatalf("detail viewport kept stale English content:\n%s", rendered)
	}
}

func TestDetailRefreshUpdatesSessionDiagnostics(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 120, 80)
	m.sessions[1].LoopResultData = engine.LoopResult{
		HasLoop:  true,
		LoopType: "tool_loop",
		Turns:    7,
		LoopCost: 0.25,
	}
	m.view = viewList
	m.table.SetCursor(0)
	m.openDetail()
	if m.loopResult.HasLoop {
		t.Fatalf("test setup should start with a non-loop session")
	}

	m.table.SetCursor(1)
	m.refreshDetailViewport()

	if !m.loopResult.HasLoop || m.loopResult.LoopType != "tool_loop" {
		t.Fatalf("detail refresh kept stale diagnostics: %+v", m.loopResult)
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

func TestHealthCommandRejectsUnknownFilter(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewList
	m.runCommand("health good")

	m.runCommand("health banana")

	if m.filterHealth != "good" {
		t.Fatalf("invalid health command should keep previous filter, got %q", m.filterHealth)
	}
	if got := len(m.table.Rows()); got != 1 {
		t.Fatalf("invalid health command should keep previous rows, got %d", got)
	}
	if m.commandFeedback != i18n.T("cmd_usage_health") {
		t.Fatalf("expected health usage feedback, got %q", m.commandFeedback)
	}
}

func TestNumericCommandsRejectNonFiniteValues(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewList
	m.runCommand("cost >0.5")

	m.runCommand("cost NaN")
	if m.filterCostOp != ">" || m.filterCostValue != 0.5 {
		t.Fatalf("invalid cost value should keep previous filter: op=%q value=%v", m.filterCostOp, m.filterCostValue)
	}
	if m.commandFeedback != i18n.T("cmd_cost_expect") {
		t.Fatalf("expected cost usage feedback, got %q", m.commandFeedback)
	}

	m.runCommand("health good")
	m.runCommand("health Inf")
	if m.filterHealth != "good" {
		t.Fatalf("invalid health value should keep previous filter, got %q", m.filterHealth)
	}
	if m.commandFeedback != i18n.T("cmd_usage_health") {
		t.Fatalf("expected health usage feedback, got %q", m.commandFeedback)
	}
}

func TestNumericCommandsRejectTrailingTokens(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewList
	m.runCommand("cost >0.5")

	m.runCommand("cost > 0.2 extra")
	if m.filterCostOp != ">" || m.filterCostValue != 0.5 {
		t.Fatalf("invalid cost command should keep previous filter: op=%q value=%v", m.filterCostOp, m.filterCostValue)
	}
	if m.commandFeedback != i18n.T("cmd_cost_expect") {
		t.Fatalf("expected cost usage feedback, got %q", m.commandFeedback)
	}

	m.runCommand("health good")
	m.runCommand("health > 80 extra")
	if m.filterHealth != "good" {
		t.Fatalf("invalid health command should keep previous filter, got %q", m.filterHealth)
	}
	if m.commandFeedback != i18n.T("cmd_usage_health") {
		t.Fatalf("expected health usage feedback, got %q", m.commandFeedback)
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

func TestSourceSortShowsColumnIndicator(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewList
	m.runCommand("sort source asc")

	cols := m.table.Columns()
	if len(cols) < 2 {
		t.Fatalf("expected source column, got %+v", cols)
	}
	if !strings.Contains(cols[1].Title, "▲") {
		t.Fatalf("source sort column should show ascending indicator, got %q", cols[1].Title)
	}
}

func TestSortCommandRejectsUnknownDirection(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewList
	m.sortBy = "name"
	m.sortDesc = false

	m.runCommand("sort cost sideways")

	if m.sortBy != "name" || m.sortDesc {
		t.Fatalf("invalid sort direction should not change sort: sortBy=%q desc=%v", m.sortBy, m.sortDesc)
	}
	if !strings.Contains(m.commandFeedback, "sideways") {
		t.Fatalf("expected feedback to mention invalid direction, got %q", m.commandFeedback)
	}
}

func TestSortAndTopCommandsRejectTrailingTokens(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewList
	m.sortBy = "name"
	m.sortDesc = false

	m.runCommand("sort cost asc extra")
	if m.sortBy != "name" || m.sortDesc {
		t.Fatalf("invalid sort command should keep previous sort: sortBy=%q desc=%v", m.sortBy, m.sortDesc)
	}
	if m.commandFeedback != i18n.T("cmd_usage_sort") {
		t.Fatalf("expected sort usage feedback, got %q", m.commandFeedback)
	}

	m.runCommand("top cost extra")
	if m.sortBy != "name" || m.sortDesc {
		t.Fatalf("invalid top command should keep previous sort: sortBy=%q desc=%v", m.sortBy, m.sortDesc)
	}
	if m.commandFeedback != i18n.T("cmd_usage_top") {
		t.Fatalf("expected top usage feedback, got %q", m.commandFeedback)
	}
}

func TestTopCommandRejectsNameField(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewList
	m.sortBy = "cost"
	m.sortDesc = true

	m.runCommand("top name")

	if m.sortBy != "cost" || !m.sortDesc {
		t.Fatalf("invalid top field should keep previous sort: sortBy=%q desc=%v", m.sortBy, m.sortDesc)
	}
	if m.commandFeedback != i18n.T("cmd_usage_top") {
		t.Fatalf("expected top usage feedback, got %q", m.commandFeedback)
	}
}

func TestSourceShortcutRecoversFromUnknownSourceFilter(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewList
	m.runCommand("source does-not-exist")
	if got := len(m.table.Rows()); got != 0 {
		t.Fatalf("expected unknown source filter to hide all rows, got %d", got)
	}

	next, _ := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune("s")})
	m = next.(Model)

	if m.filterSource == "does-not-exist" {
		t.Fatalf("source shortcut should not stay stuck on unknown source")
	}
	if got := len(m.table.Rows()); got == 0 {
		t.Fatalf("expected source shortcut to recover visible rows")
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

func TestClearCommandFromEmptyDetailReturnsToList(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewList
	m.filterText = "no-such-session"
	m.rebuildFilteredView()
	m.view = viewDetail
	m.detailReady = false

	m.runCommand("clear")

	if m.view != viewList {
		t.Fatalf("clear command should return to list from empty detail, got view=%d", m.view)
	}
	if m.detailReady {
		t.Fatalf("empty detail viewport should stay cleared")
	}
	if m.hasAnyFilter() {
		t.Fatalf("expected clear to remove filters, got %s", m.filterLabel())
	}
	if got := len(m.table.Rows()); got != len(m.sessions) {
		t.Fatalf("expected all rows after clear, got %d", got)
	}
}

func TestZeroArgCommandsRejectTrailingTokens(t *testing.T) {
	tests := []struct {
		command  string
		feedback string
		assert   func(*testing.T, Model)
	}{
		{
			command:  "clear now",
			feedback: i18n.T("cmd_usage_clear"),
			assert: func(t *testing.T, m Model) {
				if !m.hasAnyFilter() {
					t.Fatalf("invalid clear command should keep existing filters")
				}
			},
		},
		{
			command:  "help me",
			feedback: i18n.T("cmd_usage_help"),
			assert:   func(t *testing.T, m Model) {},
		},
		{
			command:  "anomalies now",
			feedback: i18n.T("cmd_usage_anomalies"),
			assert: func(t *testing.T, m Model) {
				if m.filterAnomaly {
					t.Fatalf("invalid anomalies command should not enable anomaly filter")
				}
			},
		},
		{
			command:  "critical now",
			feedback: i18n.T("cmd_usage_critical"),
			assert: func(t *testing.T, m Model) {
				if m.filterHealth == "crit" {
					t.Fatalf("invalid critical command should not enable critical filter")
				}
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.command, func(t *testing.T) {
			m := resizeForTest(t, sampleModelForTest(), 100, 30)
			m.view = viewList
			m.filterText = "beta"
			m.rebuildFilteredView()

			m.runCommand(tt.command)

			if m.commandFeedback != tt.feedback {
				t.Fatalf("expected feedback %q, got %q", tt.feedback, m.commandFeedback)
			}
			tt.assert(t, m)
		})
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

func TestOpeningDetailWithNoVisibleRowsClearsStaleViewport(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 36)
	m.view = viewList
	m.table.SetCursor(1)
	m.openDetail()
	if !m.detailReady {
		t.Fatalf("expected initial detail viewport to be ready")
	}

	m.view = viewList
	m.filterText = "no-such-session"
	m.rebuildFilteredView()
	next, _ := m.Update(tea.KeyMsg{Type: tea.KeyTab})
	m = next.(Model)

	if m.view != viewDetail {
		t.Fatalf("expected Tab from list to enter detail view, got %d", m.view)
	}
	if m.detailReady {
		t.Fatalf("expected stale detail viewport to be cleared")
	}
	rendered := m.View()
	if !strings.Contains(rendered, strings.TrimSpace(i18n.T("no_visible_sessions_hint"))) {
		t.Fatalf("expected empty detail hint, got:\n%s", rendered)
	}
}

func TestDiagnosticsShowsNoVisibleSessionsHint(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 36)
	m.view = viewDiagnostics
	m.filterText = "no-such-session"
	m.rebuildFilteredView()

	rendered := m.View()
	if !strings.Contains(rendered, strings.TrimSpace(i18n.T("no_visible_sessions_hint"))) {
		t.Fatalf("expected no visible sessions hint, got:\n%s", rendered)
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

func TestListShortcutOpensDiagnostics(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewList
	m.table.SetCursor(1)

	next, _ := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune("w")})
	m = next.(Model)

	if m.view != viewDiagnostics {
		t.Fatalf("expected list w shortcut to open diagnostics, got view=%d", m.view)
	}
	if idx := m.findSessionIndex(); idx < 0 || m.sessions[idx].Name != "session_beta_with_a_long_name" {
		t.Fatalf("diagnostics shortcut changed selected session: idx=%d", idx)
	}
}

func TestListDiffShortcutWorksAtLastRow(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewList
	m.table.SetCursor(len(m.filteredIndices) - 1)

	next, _ := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune("d")})
	m = next.(Model)

	if m.view != viewDiff {
		t.Fatalf("expected list d shortcut at last row to open diff, got view=%d", m.view)
	}
	if len(m.diffResult.Entries) == 0 {
		t.Fatalf("expected diff entries for last-row neighbor comparison")
	}
}

func TestDetailDiffShortcutUsesFilteredNeighbor(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewList
	m.filterSource = "cli"
	m.rebuildFilteredView()
	if got := len(m.filteredIndices); got != 2 {
		t.Fatalf("expected setup to keep alpha and gamma, got %d", got)
	}
	m.table.SetCursor(1)
	m.openDetail()
	m.view = viewDetail

	next, _ := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune("d")})
	m = next.(Model)

	if m.view != viewDiff {
		t.Fatalf("expected detail d shortcut to open diff, got view=%d", m.view)
	}
	if m.diffResult.SessionA != "session_alpha" || m.diffResult.SessionB != "gamma" {
		t.Fatalf("detail diff should use filtered neighbors, got %q -> %q", m.diffResult.SessionA, m.diffResult.SessionB)
	}
}

func TestDiffEmptyStateExplainsSingleVisibleSession(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewDiff
	m.filterText = "beta"
	m.rebuildFilteredView()
	m.diffResult = engine.SessionDiff{}

	rendered := m.View()
	if !strings.Contains(rendered, strings.TrimSpace(i18n.T("diff_need_two"))) {
		t.Fatalf("expected single-visible diff hint, got:\n%s", rendered)
	}
	if strings.Contains(rendered, strings.TrimSpace(i18n.T("diff_select_neighbor"))) {
		t.Fatalf("diff empty state should not ask for a neighbor when only one row is visible:\n%s", rendered)
	}
}

func TestDiffShortcutsOpenEmptyStateWhenSingleVisibleSession(t *testing.T) {
	for _, tt := range []struct {
		name string
		view view
		key  tea.KeyMsg
	}{
		{name: "list d", view: viewList, key: tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune("d")}},
		{name: "detail d", view: viewDetail, key: tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune("d")}},
		{name: "numeric 4", view: viewList, key: tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune("4")}},
		{name: "diagnostics tab", view: viewDiagnostics, key: tea.KeyMsg{Type: tea.KeyTab}},
	} {
		t.Run(tt.name, func(t *testing.T) {
			m := resizeForTest(t, sampleModelForTest(), 100, 30)
			m.view = viewList
			m.filterText = "beta"
			m.rebuildFilteredView()
			m.view = tt.view
			if tt.view == viewDetail {
				m.openDetail()
			}

			next, _ := m.Update(tt.key)
			m = next.(Model)

			if m.view != viewDiff {
				t.Fatalf("expected diff empty state view, got %d", m.view)
			}
			rendered := m.View()
			if !strings.Contains(rendered, strings.TrimSpace(i18n.T("diff_need_two"))) {
				t.Fatalf("expected diff empty state hint, got:\n%s", rendered)
			}
		})
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

func TestAppendSessionKeepsFilteredRowsInSync(t *testing.T) {
	m := resizeForTest(t, New("__missing_test_sessions__"), 100, 30)
	m.loading = true
	m.filterText = "match"

	m.appendSession(engine.Session{Name: "skip", Health: 90}, false)
	if got := len(m.table.Rows()); got != 0 {
		t.Fatalf("non-matching appended session should stay hidden, got %d rows", got)
	}

	m.appendSession(engine.Session{Name: "match", Health: 90}, false)
	if got := len(m.filteredIndices); got != 1 {
		t.Fatalf("expected one filtered index after matching append, got %d", got)
	}
	if got := len(m.table.Rows()); got != 1 {
		t.Fatalf("expected one visible row after matching append, got %d", got)
	}
	if idx := m.findSessionIndex(); idx < 0 || m.sessions[idx].Name != "match" {
		t.Fatalf("appended filtered row selected wrong session: idx=%d", idx)
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
	for _, width := range []int{32, 40, 52, 60, 80} {
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

	for _, width := range []int{32, 40, 52, 60, 80} {
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
