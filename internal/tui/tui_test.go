package tui

import (
	"fmt"
	"math"
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

func TestKeymapFitsDefaultTerminalAndShowsLanguageShortcut(t *testing.T) {
	prev := i18n.Current
	t.Cleanup(func() { i18n.SetLang(prev) })

	for _, tt := range []struct {
		lang i18n.Lang
		want string
	}{
		{lang: i18n.EN, want: "language"},
		{lang: i18n.ZH, want: "语言"},
	} {
		i18n.SetLang(tt.lang)
		m := resizeForTest(t, sampleModelForTest(), 80, 24)
		m.lang = tt.lang
		m.helpOpen = true

		rendered := m.View()
		if !strings.Contains(rendered, tt.want) {
			t.Fatalf("keymap should show language shortcut for %s:\n%s", tt.lang, rendered)
		}
		if got := maxRenderedWidth(rendered); got > 80 {
			t.Fatalf("keymap too wide for %s: got=%d line=%q", tt.lang, got, widestLine(rendered))
		}
		if got := renderedHeight(rendered); got > 24 {
			t.Fatalf("keymap too tall for %s: got=%d", tt.lang, got)
		}
	}
}

func TestLanguageSwitchWorksFromKeymap(t *testing.T) {
	prev := i18n.Current
	i18n.SetLang(i18n.EN)
	t.Cleanup(func() { i18n.SetLang(prev) })

	m := resizeForTest(t, sampleModelForTest(), 80, 24)
	m.lang = i18n.EN
	m.helpOpen = true

	next, _ := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune("l")})
	m = next.(Model)

	if m.lang != i18n.ZH || i18n.Current != i18n.ZH {
		t.Fatalf("expected keymap language switch to set Chinese, model=%s current=%s", m.lang, i18n.Current)
	}
	if !m.helpOpen {
		t.Fatalf("language switch should keep keymap open")
	}
	if rendered := m.View(); !strings.Contains(rendered, "语言") {
		t.Fatalf("expected keymap to rerender in Chinese:\n%s", rendered)
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

func TestTurnSortUsesDisplayedNonNegativeValue(t *testing.T) {
	m := sampleModelForTest()
	m.sessions[0].Metrics.AssistantTurns = -10
	m.sessions[1].Metrics.AssistantTurns = 0
	m.sessions[2].Metrics.AssistantTurns = 2
	m = resizeForTest(t, m, 100, 30)
	m.view = viewList
	m.sortBy = "turns"
	m.sortDesc = false

	m.sortAndRefresh()

	rows := m.table.Rows()
	if rows[0][0] != "session_alpha" || rows[1][0] != "session_beta_with_a_long_name" {
		t.Fatalf("negative turns should sort as displayed zero value, rows=%+v", rows)
	}
}

func TestSortByFailuresAndAnomalies(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 120, 30)
	m.view = viewList

	m.runCommand("sort errors desc")
	if m.sortBy != "failures" || !m.sortDesc {
		t.Fatalf("expected failures desc sort, got %q desc=%v", m.sortBy, m.sortDesc)
	}
	if got := m.table.Rows()[0][0]; got != "session_beta_with_a_long_name" {
		t.Fatalf("expected highest failure session first, got %q", got)
	}

	m.runCommand("top anom")
	if m.sortBy != "anomalies" || !m.sortDesc {
		t.Fatalf("expected anomalies desc sort, got %q desc=%v", m.sortBy, m.sortDesc)
	}
	if got := m.table.Rows()[0][0]; got != "session_beta_with_a_long_name" {
		t.Fatalf("expected anomalous session first, got %q", got)
	}
}

func TestListSortShortcutsCoverFailureAndAnomalyColumns(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 120, 30)
	m.view = viewList

	m = pressForTest(t, m, "e")
	if m.sortBy != "failures" || !m.sortDesc {
		t.Fatalf("expected e to sort failures desc, got %q desc=%v", m.sortBy, m.sortDesc)
	}

	m = pressForTest(t, m, "a")
	if m.sortBy != "anomalies" || !m.sortDesc {
		t.Fatalf("expected a to sort anomalies desc, got %q desc=%v", m.sortBy, m.sortDesc)
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

func TestFinishLoadingKeepsActiveSort(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.sortBy = "cost"
	m.sortDesc = true

	m.finishLoading()

	rows := m.table.Rows()
	if len(rows) == 0 || rows[0][0] != "session_beta_with_a_long_name" {
		t.Fatalf("finishLoading should keep cost sort row order, rows=%+v", rows)
	}
	if cols := m.table.Columns(); len(cols) < 7 || !strings.Contains(cols[6].Title, "▼") {
		t.Fatalf("cost sort indicator should remain visible, cols=%+v", cols)
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

func TestListEmptyFilterStateShowsRecoveryPath(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewList
	m.filterText = "no-such-session"
	m.rebuildFilteredView()

	rendered := m.View()
	for _, want := range []string{
		i18n.T("no_visible_sessions_title"),
		fmt.Sprintf(i18n.T("no_visible_sessions_active"), m.filterLabel()),
		i18n.T("no_visible_sessions_clear"),
	} {
		if !strings.Contains(rendered, want) {
			t.Fatalf("empty filter state missing %q:\n%s", want, rendered)
		}
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

func TestOverviewSearchShortcutOpensListFilter(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewOverview

	next, _ := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune("/")})
	m = next.(Model)

	if m.view != viewList || !m.filterActive || m.filterInput != "" {
		t.Fatalf("expected overview search to open list filter: view=%d active=%v input=%q", m.view, m.filterActive, m.filterInput)
	}

	next, _ = m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune("beta")})
	m = next.(Model)
	next, _ = m.Update(tea.KeyMsg{Type: tea.KeyEnter})
	m = next.(Model)

	if m.filterText != "beta" || len(m.table.Rows()) != 1 {
		t.Fatalf("overview search filter not applied: text=%q rows=%d", m.filterText, len(m.table.Rows()))
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

	if got := len(m.table.Columns()); got != 7 {
		t.Fatalf("expected compact columns, got %d", got)
	}
	row := m.table.Rows()[0]
	if row[2] != "1" || row[3] != "0" {
		t.Fatalf("expected compact triage signals, got row=%v", row)
	}
	if row[5] != "154.0K" {
		t.Fatalf("expected full compact token value, got %q", row[5])
	}
	if row[6] != "92%" {
		t.Fatalf("expected health value, got %q", row[6])
	}
}

func TestStandardWidthListKeepsHeadersAndHelpReadable(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 80, 24)
	m.view = viewList

	rendered := m.View()
	for _, want := range []string{"SESSION", "SOURCE", "FAIL", "ANOM", "TOKENS", "HEALTH", "? keys"} {
		if !strings.Contains(rendered, want) {
			t.Fatalf("standard-width list missing readable label %q:\n%s", want, rendered)
		}
	}
	for _, clipped := range []string{"HEAL…", "?: key…"} {
		if strings.Contains(rendered, clipped) {
			t.Fatalf("standard-width list should avoid clipped core labels %q:\n%s", clipped, rendered)
		}
	}
	if got := m.frameBodyWidth(); got < 68 {
		t.Fatalf("standard-width list should use most terminal columns, body width=%d", got)
	}
}

func TestCompactListShowsTriageSortIndicators(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 80, 30)
	m.view = viewList

	m.runCommand("sort failures desc")
	cols := m.table.Columns()
	if !strings.Contains(cols[2].Title, "▼") {
		t.Fatalf("compact failure column should show descending sort indicator, got %+v", cols)
	}

	m.runCommand("sort anomalies desc")
	cols = m.table.Columns()
	if !strings.Contains(cols[3].Title, "▼") {
		t.Fatalf("compact anomaly column should show descending sort indicator, got %+v", cols)
	}
}

func TestWideListKeepsFullOperationalColumns(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 160, 36)
	m.view = viewList
	if got := len(m.table.Columns()); got != 13 {
		t.Fatalf("expected full columns on wide screens, got %d", got)
	}
	row := m.table.Rows()[0]
	if row[3] != "8" || row[4] != "12" || row[5] != "92" || row[6] != "1" {
		t.Fatalf("expected turns/tools/success/fail columns, got row=%v", row)
	}
	if row[12] != "No major anomaly" {
		t.Fatalf("expected issue column, got row=%v", row)
	}
}

func TestUltraWideListAddsDiagnosticColumns(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 180, 36)
	m.view = viewList
	if got := len(m.table.Columns()); got != 13 {
		t.Fatalf("expected diagnostic columns on ultra-wide screens, got %d", got)
	}
	cols := m.table.Columns()
	if cols[2].Title != "MODEL" || cols[9].Title != "DURATION" || cols[10].Title != "ANOM" || cols[12].Title != "ISSUE" {
		t.Fatalf("unexpected ultra-wide column titles: %+v", cols)
	}
	row := m.table.Rows()[0]
	if row[2] != "gpt-5.1" || row[10] != "0" || row[12] != "No major anomaly" {
		t.Fatalf("expected model and anomaly count columns, got row=%v", row)
	}
}

func TestListAndDetailClampInvalidToolCounts(t *testing.T) {
	m := sampleModelForTest()
	m.sessions[0].Metrics.AssistantTurns = -2
	m.sessions[0].Metrics.ToolCallsTotal = -5
	m.sessions[0].Metrics.ToolCallsOK = -3
	m.sessions[0].Metrics.ToolCallsFail = -2
	m.sessions[0].Metrics.CostEstimated = math.Inf(1)
	m = resizeForTest(t, m, 160, 36)
	m.view = viewList

	row := m.table.Rows()[0]
	if row[3] != "0" || row[4] != "0" || row[5] != "N/A" {
		t.Fatalf("expected invalid tool counts to be clamped in list row, got row=%v", row)
	}

	m.openDetail()
	m.view = viewDetail
	rendered := m.View()
	if !strings.Contains(rendered, "TOOLS 0/0 N/A") || !strings.Contains(rendered, "0 turns") || !strings.Contains(rendered, "COST $0.0000") {
		t.Fatalf("detail should clamp invalid tool counts:\n%s", rendered)
	}
	if strings.Contains(rendered, "-200%") || strings.Contains(rendered, "Inf") || strings.Contains(rendered, "NaN") {
		t.Fatalf("detail should not render invalid metrics:\n%s", rendered)
	}
}

func TestDetailReportUsesSafeMetricValues(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 120, 80)
	m.sessions[0].Metrics.UserMessages = -4
	m.sessions[0].Metrics.AssistantTurns = -3
	m.sessions[0].Metrics.ToolCallsTotal = -8
	m.sessions[0].Metrics.ToolCallsOK = -6
	m.sessions[0].Metrics.ToolCallsFail = -2
	m.sessions[0].Metrics.TokensInput = -100
	m.sessions[0].Metrics.TokensOutput = -200
	m.sessions[0].Metrics.TokensCacheW = -300
	m.sessions[0].Metrics.TokensCacheR = -400
	m.sessions[0].Metrics.CostEstimated = math.NaN()
	m.sessions[0].Metrics.DurationSec = math.Inf(1)
	m.sessions[0].Metrics.GapsSec = []float64{-12, math.NaN(), math.Inf(1)}
	m.sessions[0].Metrics.ToolUsage = map[string]int{"bad_tool": -7}
	m.view = viewList
	m.openDetail()

	content := m.renderDetailViewportContent(m.sessions[0])

	for _, bad := range []string{"NaN", "Inf", "-100", "-200", "-300", "-400", "-12", "-7", "-6", "-4", "-3", "-2"} {
		if strings.Contains(content, bad) {
			t.Fatalf("detail report should sanitize invalid metric %q:\n%s", bad, content)
		}
	}
}

func TestViewsClampInvalidHealthScores(t *testing.T) {
	m := sampleModelForTest()
	m.sessions[0].Health = -20
	m.sessions[1].Health = 150
	m.overview = engine.ComputeOverview(m.sessions)
	m.aggStats = engine.ComputeAggregateStats(m.sessions)
	m = resizeForTest(t, m, 160, 36)
	m.view = viewList

	rows := m.table.Rows()
	if !strings.Contains(rows[0][11], "0%") || !strings.Contains(rows[1][11], "100%") {
		t.Fatalf("expected health scores to be clamped in list rows, rows=%+v", rows)
	}
	for i, row := range rows[:2] {
		if got, max := lipgloss.Width(row[11]), m.table.Columns()[11].Width; got > max {
			t.Fatalf("health cell %d too wide: got=%d max=%d row=%q", i, got, max, row[11])
		}
	}

	m.openDetail()
	m.view = viewDetail
	rendered := m.View()
	if !strings.Contains(rendered, "HEALTH 0/100") || strings.Contains(rendered, "-20%") {
		t.Fatalf("detail should clamp invalid health score:\n%s", rendered)
	}

	m.view = viewOverview
	rendered = m.View()
	if strings.Contains(rendered, "150%") || strings.Contains(rendered, "-20%") {
		t.Fatalf("overview should clamp invalid health scores:\n%s", rendered)
	}
}

func TestHealthCellShowsScanFriendlyBar(t *testing.T) {
	tests := []struct {
		health int
		width  int
		want   string
	}{
		{health: 92, width: 9, want: "92%"},
		{health: 64, width: 8, want: "64%"},
		{health: -20, width: 9, want: "0%"},
		{health: 150, width: 9, want: "100%"},
		{health: 42, width: 5, want: "42%"},
	}

	for _, tt := range tests {
		got := healthCell(tt.health, tt.width)
		if !strings.Contains(got, tt.want) {
			t.Fatalf("healthCell(%d, %d) missing %q: %q", tt.health, tt.width, tt.want, got)
		}
		if lipgloss.Width(got) > tt.width {
			t.Fatalf("healthCell(%d, %d) too wide: got=%d %q", tt.health, tt.width, lipgloss.Width(got), got)
		}
		if tt.width >= 7 && !strings.Contains(got, "█") && !strings.Contains(got, "░") {
			t.Fatalf("healthCell(%d, %d) should include a scan bar: %q", tt.health, tt.width, got)
		}
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

	m.runCommand("clear")
	m.runCommand("health good")
	rendered = m.View()
	if !strings.Contains(rendered, "健康=良好") {
		t.Fatalf("expected translated health filter label in list view:\n%s", rendered)
	}

	m.runCommand("clear")
	m.filterText = "不存在"
	m.rebuildFilteredView()
	rendered = m.View()
	for _, want := range []string{"没有匹配会话", "当前筛选", "按 Esc"} {
		if !strings.Contains(rendered, want) {
			t.Fatalf("expected translated empty filter state %q:\n%s", want, rendered)
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
	if got := healthFilterLabel("crit"); got != "严重" {
		t.Fatalf("expected translated health filter label, got %q", got)
	}
}

func TestWideListRendersStableSingleTable(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 220, 36)
	m.view = viewList
	rendered := m.View()
	for _, stale := range []string{"SESSION INSPECTOR", "Enter detail · d diff · w diagnostics", "Health 92%", "Cost $0.4200"} {
		if strings.Contains(rendered, stale) {
			t.Fatalf("list should not render stale split preview fragment %q", stale)
		}
	}
	for _, want := range []string{"SESSION", "SOURCE", "MODEL", "DURATION", "ANOM", "ISSUE", "session_alpha", "gpt-5.1"} {
		if !strings.Contains(rendered, want) {
			t.Fatalf("expected wide list to include %q:\n%s", want, rendered)
		}
	}
	if got := maxRenderedWidth(rendered); got > 220 {
		t.Fatalf("wide list render too wide: got=%d line=%q", got, widestLine(rendered))
	}
}

func TestListFrameClearsFullTerminalWidth(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 244, 40)
	m.view = viewList

	for i, line := range strings.Split(m.View(), "\n") {
		if got := lipgloss.Width(line); got != 244 {
			t.Fatalf("line %d should fill terminal width, got=%d line=%q", i, got, line)
		}
	}
}

func TestChineseWideListColumnsAndFilterLabels(t *testing.T) {
	prev := i18n.Current
	i18n.SetLang(i18n.ZH)
	t.Cleanup(func() { i18n.SetLang(prev) })

	m := resizeForTest(t, sampleModelForTest(), 220, 36)
	m.view = viewList
	m.runCommand("critical")

	rendered := m.View()
	for _, want := range []string{"模型", "时长", "异常", "健康=严重"} {
		if !strings.Contains(rendered, want) {
			t.Fatalf("wide Chinese list missing translated label %q:\n%s", want, rendered)
		}
	}
	for _, unwanted := range []string{"MODEL", "DURATION", "ANOM", "health=crit"} {
		if strings.Contains(rendered, unwanted) {
			t.Fatalf("wide Chinese list leaked English/internal label %q:\n%s", unwanted, rendered)
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
	if !strings.Contains(rendered, "Overview") || !strings.Contains(rendered, "$ top") {
		t.Fatalf("expected footer help to remain visible in clipped overview:\n%s", rendered)
	}
	if got := renderedHeight(rendered); got != 24 {
		t.Fatalf("expected render to fill terminal height, got %d", got)
	}
}

func TestCompactOverviewUsesScanFriendlySmallViewport(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 80, 24)
	m.view = viewOverview

	rendered := m.View()

	for _, want := range []string{"Next", "FOCUS", "RECENT", "TOKEN", "HEALTH"} {
		if !strings.Contains(rendered, want) {
			t.Fatalf("compact overview missing %q:\n%s", want, rendered)
		}
	}
	if strings.Contains(rendered, "TOKEN USAGE") {
		t.Fatalf("compact overview should avoid tall dashboard panels:\n%s", rendered)
	}
	if got := maxRenderedWidth(rendered); got > 80 {
		t.Fatalf("compact overview too wide: got=%d line=%q", got, widestLine(rendered))
	}
	if got := renderedHeight(rendered); got != 24 {
		t.Fatalf("compact overview should fill terminal height, got=%d", got)
	}
}

func TestDashboardHeroKeepsTitleSingleLineAtNarrowWidth(t *testing.T) {
	for _, lang := range []i18n.Lang{i18n.EN, i18n.ZH} {
		t.Run(string(lang), func(t *testing.T) {
			prev := i18n.Current
			i18n.SetLang(lang)
			t.Cleanup(func() { i18n.SetLang(prev) })

			m := sampleModelForTest()
			hero := m.renderDashboardHero(78)
			if got := renderedHeight(hero); got != 1 {
				t.Fatalf("wide compact hero should stay on one line, got %d:\n%s", got, hero)
			}
			if got := maxRenderedWidth(hero); got > 78 {
				t.Fatalf("hero too wide: got=%d line=%q", got, widestLine(hero))
			}
			if strings.Contains(hero, "…") {
				t.Fatalf("standard-width hero summary should avoid truncation:\n%s", hero)
			}
		})
	}
}

func TestDashboardTitleFallsBackInsteadOfWrapping(t *testing.T) {
	for _, width := range []int{4, 10, 20, 29} {
		title := renderDashboardTitle(width)
		if got := renderedHeight(title); got != 1 {
			t.Fatalf("title should not wrap at width=%d, got %d:\n%s", width, got, title)
		}
		if got := maxRenderedWidth(title); got > width {
			t.Fatalf("title too wide at width=%d: got=%d line=%q", width, got, widestLine(title))
		}
	}
}

func TestOverviewShowsActionHint(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 120, 36)
	m.view = viewOverview

	rendered := m.View()

	if !strings.Contains(rendered, "Next") || !strings.Contains(rendered, "press !") {
		t.Fatalf("overview should guide the next triage action:\n%s", rendered)
	}
	if got := maxRenderedWidth(rendered); got > 120 {
		t.Fatalf("overview action hint too wide: got=%d line=%q", got, widestLine(rendered))
	}
}

func TestChineseOverviewShowsTranslatedActionHint(t *testing.T) {
	prev := i18n.Current
	i18n.SetLang(i18n.ZH)
	t.Cleanup(func() { i18n.SetLang(prev) })

	m := resizeForTest(t, sampleModelForTest(), 120, 36)
	m.lang = i18n.ZH
	m.refreshColumns()
	m.view = viewOverview

	rendered := m.View()

	if !strings.Contains(rendered, "下一步") || !strings.Contains(rendered, "按 ! 查看") {
		t.Fatalf("Chinese overview should guide the next triage action:\n%s", rendered)
	}
	if strings.Contains(rendered, "Next") {
		t.Fatalf("Chinese overview leaked English action label:\n%s", rendered)
	}
}

func TestChineseCompactOverviewTranslatesRuntimeLabels(t *testing.T) {
	prev := i18n.Current
	i18n.SetLang(i18n.ZH)
	t.Cleanup(func() { i18n.SetLang(prev) })

	m := resizeForTest(t, sampleModelForTest(), 80, 24)
	m.view = viewOverview

	rendered := m.View()
	for _, want := range []string{"关注", "最近", "健康"} {
		if !strings.Contains(rendered, want) {
			t.Fatalf("compact overview missing translated label %q:\n%s", want, rendered)
		}
	}
	for _, unwanted := range []string{"FOCUS", "RECENT"} {
		if strings.Contains(rendered, unwanted) {
			t.Fatalf("compact overview leaked English label %q:\n%s", unwanted, rendered)
		}
	}
}

func TestDetailViewportUsesFrameBodyWidth(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 80, 24)
	m.view = viewList
	m.openDetail()

	if m.viewport.Width != m.frameBodyWidth() {
		t.Fatalf("detail viewport width=%d, want frame body width=%d", m.viewport.Width, m.frameBodyWidth())
	}
	if m.viewport.Height < 6 {
		t.Fatalf("detail viewport should leave usable space in 80x24, got height=%d", m.viewport.Height)
	}
	content := m.renderDetailViewportContent(m.sessions[0])
	for i, line := range strings.Split(content, "\n") {
		if got := lipgloss.Width(line); got > m.viewport.Width {
			t.Fatalf("detail content line %d exceeds viewport: got=%d want<=%d line=%q", i, got, m.viewport.Width, line)
		}
	}
}

func TestOverviewHandlesZeroTokenAgents(t *testing.T) {
	m := sampleModelForTest()
	for i := range m.sessions {
		m.sessions[i].Metrics.TokensInput = 0
		m.sessions[i].Metrics.TokensOutput = 0
		m.sessions[i].Metrics.TokensCacheR = 0
	}
	m.overview = engine.ComputeOverview(m.sessions)
	m.costSummary = engine.ComputeCostSummary(m.sessions)
	m = resizeForTest(t, m, 120, 36)
	m.view = viewOverview

	rendered := m.View()

	if got := maxRenderedWidth(rendered); got > 120 {
		t.Fatalf("zero-token overview too wide: got=%d line=%q", got, widestLine(rendered))
	}
}

func TestOverviewClampsInvalidChartValues(t *testing.T) {
	m := sampleModelForTest()
	for i := range m.sessions {
		m.sessions[i].Metrics.TokensInput = -1000
		m.sessions[i].Metrics.TokensOutput = -1000
		m.sessions[i].Metrics.TokensCacheR = -1000
		m.sessions[i].Metrics.GapsSec = []float64{-100, math.NaN(), math.Inf(1)}
		m.sessions[i].Metrics.CostEstimated = math.NaN()
	}
	m.sessions[0].Metrics.GapsSec = append(m.sessions[0].Metrics.GapsSec, 10)
	m.sessions[1].Metrics.CostEstimated = math.Inf(1)
	m.sessions[2].Metrics.CostEstimated = -1
	m.overview = engine.ComputeOverview(m.sessions)
	m.costSummary = engine.ComputeCostSummary(m.sessions)
	m = resizeForTest(t, m, 120, 36)
	m.view = viewOverview

	rendered := m.View()

	if got := maxRenderedWidth(rendered); got > 120 {
		t.Fatalf("invalid metric overview too wide: got=%d line=%q", got, widestLine(rendered))
	}
	if strings.Contains(rendered, "NaN") || strings.Contains(rendered, "Inf") {
		t.Fatalf("overview should not render invalid numbers:\n%s", rendered)
	}
	if strings.Contains(rendered, "-2.0K") {
		t.Fatalf("overview should clamp negative token display:\n%s", rendered)
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

func TestCostFilterUsesSafeDisplayValue(t *testing.T) {
	m := sampleModelForTest()
	m.sessions[0].Metrics.CostEstimated = math.Inf(1)
	m = resizeForTest(t, m, 100, 30)
	m.view = viewList

	m.runCommand("cost >0.5")

	for _, row := range m.table.Rows() {
		if row[0] == "session_alpha" {
			t.Fatalf("invalid infinite cost should not match high-cost filter, rows=%+v", m.table.Rows())
		}
	}
	if got := len(m.table.Rows()); got != 1 {
		t.Fatalf("expected only real high-cost row, got %d", got)
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

func TestSortCommandSupportsTriageFields(t *testing.T) {
	m := sampleModelForTest()
	m.sessions[0].Anomalies = append(m.sessions[0].Anomalies,
		engine.Anomaly{Type: "latency", Severity: engine.SeverityMedium},
		engine.Anomaly{Type: "redacted", Severity: engine.SeverityLow},
	)
	m = resizeForTest(t, m, 180, 30)
	m.view = viewList

	m.runCommand("sort anomalies desc")
	if m.sortBy != "anomalies" || !m.sortDesc {
		t.Fatalf("anomaly sort command not applied: sortBy=%q desc=%v", m.sortBy, m.sortDesc)
	}
	if got := m.table.Rows()[0][0]; got != "session_alpha" {
		t.Fatalf("anomaly sort should put most anomalous session first, got %q", got)
	}

	m.runCommand("top failures")
	if m.sortBy != "failures" || !m.sortDesc {
		t.Fatalf("failure top command not applied: sortBy=%q desc=%v", m.sortBy, m.sortDesc)
	}
	if got := m.table.Rows()[0][0]; got != "session_beta_with_a_long_name" {
		t.Fatalf("failure top should put most failed session first, got %q", got)
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

func TestTriageSortShowsColumnIndicators(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 180, 30)
	m.view = viewList
	m.runCommand("sort failures desc")

	cols := m.table.Columns()
	if len(cols) < 12 {
		t.Fatalf("expected wide columns, got %+v", cols)
	}
	if !strings.Contains(cols[6].Title, "▼") {
		t.Fatalf("failure sort column should show descending indicator, got %q", cols[6].Title)
	}

	m.runCommand("sort anomalies desc")
	cols = m.table.Columns()
	if !strings.Contains(cols[10].Title, "▼") {
		t.Fatalf("anomaly sort column should show descending indicator, got %q", cols[10].Title)
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

func TestCommandHelpShowsOutsideListView(t *testing.T) {
	for _, v := range []view{viewOverview, viewDetail, viewDiagnostics, viewDiff} {
		m := resizeForTest(t, sampleModelForTest(), 120, 30)
		m.view = v
		if v == viewDetail {
			m.openDetail()
		}
		if v == viewDiff {
			m.diffResult = engine.DiffSessions(m.sessions[0], m.sessions[1])
		}

		next, _ := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune(":")})
		m = next.(Model)

		rendered := m.View()
		if !strings.Contains(rendered, "Enter: run") {
			t.Fatalf("expected command help in view=%d, got:\n%s", v, rendered)
		}
	}
}

func TestQuestionMarkOpensAndClosesKeymap(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewOverview

	next, _ := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune("?")})
	m = next.(Model)

	rendered := m.View()
	if !m.helpOpen || !strings.Contains(rendered, "Keyboard Shortcuts") || !strings.Contains(rendered, "cache reloads") {
		t.Fatalf("expected keymap view, got:\n%s", rendered)
	}
	if got := maxRenderedWidth(rendered); got > 100 {
		t.Fatalf("keymap render too wide: got=%d line=%q", got, widestLine(rendered))
	}

	next, _ = m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune("?")})
	m = next.(Model)
	if m.helpOpen {
		t.Fatalf("second ? should close keymap")
	}
}

func TestChineseKeymapTranslatesLabels(t *testing.T) {
	prev := i18n.Current
	i18n.SetLang(i18n.ZH)
	t.Cleanup(func() { i18n.SetLang(prev) })

	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.lang = i18n.ZH
	m.refreshColumns()

	next, _ := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune("?")})
	m = next.(Model)

	rendered := m.View()
	for _, want := range []string{"快捷键", "筛选和排序", "重建缓存"} {
		if !strings.Contains(rendered, want) {
			t.Fatalf("Chinese keymap missing %q:\n%s", want, rendered)
		}
	}
	if strings.Contains(rendered, "Keyboard Shortcuts") {
		t.Fatalf("Chinese keymap leaked English title:\n%s", rendered)
	}
}

func TestKeymapAvoidsTruncatedLabelsAtStandardWidth(t *testing.T) {
	for _, lang := range []i18n.Lang{i18n.EN, i18n.ZH} {
		t.Run(string(lang), func(t *testing.T) {
			prev := i18n.Current
			i18n.SetLang(lang)
			t.Cleanup(func() { i18n.SetLang(prev) })

			m := resizeForTest(t, sampleModelForTest(), 80, 24)
			m.lang = lang
			m.refreshColumns()
			m.view = viewList
			m.helpOpen = true

			rendered := m.View()
			if strings.Contains(rendered, "…") {
				t.Fatalf("standard-width keymap should avoid truncated labels:\n%s", rendered)
			}
			want := "cost / critical"
			if lang == i18n.ZH {
				want = "费用 / 严重"
			}
			if !strings.Contains(rendered, want) {
				t.Fatalf("standard-width keymap should show cost and critical shortcut %q:\n%s", want, rendered)
			}
			if got := maxRenderedWidth(rendered); got > 80 {
				t.Fatalf("keymap too wide: got=%d line=%q", got, widestLine(rendered))
			}
			if got := renderedHeight(rendered); got != 24 {
				t.Fatalf("keymap should fill terminal height without clipping: got=%d", got)
			}
		})
	}
}

func TestKeymapFitsSmallTerminal(t *testing.T) {
	for _, width := range []int{32, 40, 60, 80} {
		m := resizeForTest(t, sampleModelForTest(), width, 18)
		m.view = viewList
		m.helpOpen = true
		rendered := m.View()
		if got := maxRenderedWidth(rendered); got > width {
			t.Fatalf("keymap too wide: width=%d got=%d line=%q", width, got, widestLine(rendered))
		}
		if got := renderedHeight(rendered); got != 18 {
			t.Fatalf("keymap should fit terminal height: width=%d got=%d", width, got)
		}
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
	if !strings.Contains(rendered, i18n.T("no_visible_sessions_title")) ||
		!strings.Contains(rendered, i18n.T("no_visible_sessions_clear")) {
		t.Fatalf("expected empty detail hint, got:\n%s", rendered)
	}
}

func TestDiagnosticsShowsNoVisibleSessionsHint(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 36)
	m.view = viewDiagnostics
	m.filterText = "no-such-session"
	m.rebuildFilteredView()

	rendered := m.View()
	if !strings.Contains(rendered, i18n.T("no_visible_sessions_title")) ||
		!strings.Contains(rendered, i18n.T("no_visible_sessions_clear")) {
		t.Fatalf("expected no visible sessions hint, got:\n%s", rendered)
	}
}

func TestDiagnosticsClampsInvalidContextUtilization(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 80, 30)
	m.sessions[0].ContextUtil.UtilizationPct = -25
	m.sessions[0].ContextUtil.AvailableForTask = -200
	m.sessions[0].ToolLatencies = []engine.ToolLatencyItem{{
		ToolName: "slow",
		Count:    -4,
		AvgSec:   math.NaN(),
		P95Sec:   math.Inf(1),
		MaxSec:   -10,
		Timeouts: -2,
		IsSlow:   true,
	}}
	m.sessions[0].LargeParams = []engine.LargeParamCall{{
		ToolName:  "write_file",
		ParamSize: -4096,
		Risk:      "high",
		Detail:    "bad arg",
	}}
	m.view = viewDiagnostics

	rendered := m.View()

	if got := maxRenderedWidth(rendered); got > 80 {
		t.Fatalf("diagnostics render too wide: got=%d line=%q", got, widestLine(rendered))
	}
	for _, bad := range []string{"NaN", "Inf", "-10", "-4", "-2", "-200", "-4.0"} {
		if strings.Contains(rendered, bad) {
			t.Fatalf("diagnostics should sanitize invalid metric %q:\n%s", bad, rendered)
		}
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

func TestDiffUsesSafeSessionValues(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 120, 36)
	m.view = viewList
	m.sessions[0].Health = -20
	m.sessions[0].Metrics.AssistantTurns = -5
	m.sessions[0].Metrics.ToolCallsTotal = -3
	m.sessions[0].Metrics.ToolCallsFail = -2
	m.sessions[0].Metrics.CostEstimated = math.Inf(1)
	m.sessions[0].Metrics.DurationSec = math.Inf(1)
	m.sessions[1].Health = 150
	m.sessions[1].Metrics.CostEstimated = math.NaN()

	if !m.prepareDiffForCursor() {
		t.Fatalf("expected diff to be prepared")
	}
	m.view = viewDiff
	rendered := m.View()

	for _, bad := range []string{"NaN", "Inf", "Health         -20", "Health         150", "Turns          -", "Tools          -", "Fail count     -"} {
		if strings.Contains(rendered, bad) {
			t.Fatalf("diff should sanitize invalid metric %q:\n%s", bad, rendered)
		}
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

func TestFooterShowsCacheStatusAfterLoad(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.view = viewList
	m.loadTotal = 3
	m.loadedFromCache = 2

	rendered := m.View()

	if !strings.Contains(rendered, "cache 2/3") {
		t.Fatalf("footer should expose cache hit status:\n%s", rendered)
	}
}

func TestChineseFooterShowsCacheStatus(t *testing.T) {
	prev := i18n.Current
	i18n.SetLang(i18n.ZH)
	t.Cleanup(func() { i18n.SetLang(prev) })

	m := resizeForTest(t, sampleModelForTest(), 100, 30)
	m.lang = i18n.ZH
	m.refreshColumns()
	m.view = viewList
	m.loadTotal = 3
	m.loadedFromCache = 5

	rendered := m.View()

	if !strings.Contains(rendered, "缓存 3/3") {
		t.Fatalf("footer should expose translated clamped cache status:\n%s", rendered)
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

func TestLoadingRenderClampsProgressPastTotal(t *testing.T) {
	m := resizeForTest(t, New("__missing_test_sessions__"), 80, 24)
	m.loading = true
	m.loadProgress = 3
	m.loadTotal = 2

	rendered := m.View()

	if !strings.Contains(rendered, "2/2") || !strings.Contains(rendered, "100%") {
		t.Fatalf("expected clamped loading progress, got:\n%s", rendered)
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

func TestOverviewRendersHealthTrend(t *testing.T) {
	m := resizeForTest(t, sampleModelForTest(), 140, 40)
	m.view = viewOverview
	rendered := m.View()
	if !strings.Contains(rendered, i18n.T("trend_title")) {
		t.Fatalf("expected health trend in overview:\n%s", rendered)
	}
	if got := maxRenderedWidth(rendered); got > 140 {
		t.Fatalf("overview render too wide: got=%d line=%q", got, widestLine(rendered))
	}
}
