// Package tui provides the Bubble Tea interactive terminal UI for agenttrace.
// btop-style modern dashboard with four views: Overview, Session List, Detail, Diagnostics, Diff.
package tui

import (
	"fmt"
	"os"
	"sort"
	"strings"
	"unicode/utf8"

	"github.com/charmbracelet/bubbles/table"
	"github.com/charmbracelet/bubbles/viewport"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/charmbracelet/x/ansi"
	"github.com/luoyuctl/agenttrace/internal/engine"
	"github.com/luoyuctl/agenttrace/internal/i18n"
)

// ── Styles ──

var (
	baseStyle = lipgloss.NewStyle().
			BorderStyle(lipgloss.RoundedBorder()).
			BorderForeground(lipgloss.Color("238")).
			Padding(1, 2)

	titleStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("82")).
			Bold(true).
			Padding(0, 1)

	statusStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("243"))

	cyanStyle   = lipgloss.NewStyle().Foreground(lipgloss.Color("39"))
	greenStyle  = lipgloss.NewStyle().Foreground(lipgloss.Color("42"))
	yellowStyle = lipgloss.NewStyle().Foreground(lipgloss.Color("220"))
	orangeStyle = lipgloss.NewStyle().Foreground(lipgloss.Color("208"))
	redStyle    = lipgloss.NewStyle().Foreground(lipgloss.Color("196"))
	dimStyle    = lipgloss.NewStyle().Foreground(lipgloss.Color("245"))
	boldStyle   = lipgloss.NewStyle().Bold(true)

	helpStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("245")).
			Background(lipgloss.Color("235")).
			Border(lipgloss.NormalBorder(), true, false, false, false).
			BorderForeground(lipgloss.Color("238")).
			Padding(0, 1)

	// Health score background colors
	healthGoodStyle = lipgloss.NewStyle().Background(lipgloss.Color("34")).Foreground(lipgloss.Color("15"))
	healthWarnStyle = lipgloss.NewStyle().Background(lipgloss.Color("178")).Foreground(lipgloss.Color("16"))
	healthBadStyle  = lipgloss.NewStyle().Background(lipgloss.Color("160")).Foreground(lipgloss.Color("15"))

	dashPanelStyle = lipgloss.NewStyle().
			BorderStyle(lipgloss.RoundedBorder()).
			BorderForeground(lipgloss.Color("238")).
			Padding(1, 2)
	dashTitleStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("245")).
			Bold(true)
	brandStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("82")).
			Bold(true)
)

// ── Model ──

type view int

const (
	viewOverview view = iota
	viewList
	viewDetail
	viewDiagnostics // diagnostics dashboard
	viewDiff        // diff two sessions side-by-side
)

// loadProgressMsg carries one loaded (or cached) session during progressive loading.
type loadProgressMsg struct {
	session   engine.Session
	fromCache bool
	index     int
	total     int
	err       error // non-nil if this file failed to parse
	done      bool  // true when all files have been processed
}

// startLoadMsg triggers the initial load in Update() (so state changes are on the real model).
type startLoadMsg struct{}

type Model struct {
	view     view
	sessions []engine.Session
	dir      string
	lang     i18n.Lang

	// Progressive loading state
	loading         bool
	loadProgress    int
	loadTotal       int
	loadedFromCache int
	loadQueue       []string
	sessionCache    engine.SessionCache
	unsavedNewCount int

	// Overview data
	overview engine.Overview

	// Filter state
	filterText      string
	filterMode      string // "", "health", "source"
	filterValue     string // e.g. "good", "hermes_jsonl"
	filteredIndices []int  // maps table row → sessions index
	filterHealth    string // good, warn, crit, <80, >=90
	filterSource    string
	filterModel     string
	filterCostOp    string
	filterCostValue float64
	filterAnomaly   bool

	// 命令栏状态
	commandActive   bool
	commandInput    string
	commandFeedback string

	// Sort state
	sortBy   string // "health", "cost", "turns", "name", "source"
	sortDesc bool   // descending if true

	// 文本筛选输入
	filterInput  string // 用户输入的筛选文本
	filterActive bool   // 是否处于文本筛选模式

	// 全局聚合统计
	aggStats engine.AggregateStats

	// List view
	table      table.Model
	tableReady bool

	// Detail view
	viewport    viewport.Model
	detailReady bool
	loopResult  engine.LoopResult // 循环检测结果

	// Diff view
	diffResult     engine.SessionDiff
	fixSuggestions []engine.FixSuggestion
	toolWarnings   []engine.ToolWarning
	costAlert      engine.CostAlert
	wasteReport    engine.WasteReport
	healthTrend    engine.HealthTrend
	costSummary    engine.CostSummary
	stuckPatterns  []engine.StuckPattern

	// Dimensions
	width  int
	height int
}

func New(dir string) Model {
	columns := []table.Column{
		{Title: i18n.T("session"), Width: 20},
		{Title: i18n.T("source_tool"), Width: 12},
		{Title: i18n.T("turns_header"), Width: 5},
		{Title: i18n.T("tools"), Width: 5},
		{Title: i18n.T("succ_pct"), Width: 5},
		{Title: i18n.T("fail"), Width: 5},
		{Title: i18n.T("cost"), Width: 8},
		{Title: i18n.T("tokens"), Width: 7},
		{Title: i18n.T("health"), Width: 9},
	}

	t := table.New(
		table.WithColumns(columns),
		table.WithRows(nil),
		table.WithFocused(true),
		table.WithHeight(20),
	)
	s := table.DefaultStyles()
	s.Header = s.Header.
		BorderStyle(lipgloss.NormalBorder()).
		BorderForeground(lipgloss.Color("240")).
		BorderBottom(true).
		Bold(true).
		Foreground(lipgloss.Color("39"))
	s.Selected = s.Selected.
		Foreground(lipgloss.Color("15")).
		Background(lipgloss.Color("238")).
		Bold(false)
	t.SetStyles(s)

	return Model{
		view:         viewOverview,
		sessions:     nil,
		dir:          dir,
		lang:         i18n.Current,
		table:        t,
		tableReady:   true,
		sessionCache: engine.LoadSessionCache(),
		loading:      true,
	}
}

func (m Model) Init() tea.Cmd {
	return func() tea.Msg { return startLoadMsg{} }
}

// ── Progressive Loading ──────────────────────────────────────────

const cacheSaveInterval = 10

// startReload begins a progressive (cached) load of all session files.
func (m *Model) startReload() tea.Cmd {
	m.sessions = nil
	m.view = viewOverview
	m.detailReady = false
	files := engine.FindSessionFilesCached(m.dir, m.sessionCache)
	m.loadQueue = nil
	m.loadProgress = 0
	m.loadTotal = len(files)
	m.loadedFromCache = 0
	m.unsavedNewCount = 0
	m.loading = true

	for _, path := range files {
		if s, ok := engine.CachedSession(path, m.sessionCache); ok {
			m.sessions = append(m.sessions, s)
			m.loadProgress++
			m.loadedFromCache++
			continue
		}
		m.loadQueue = append(m.loadQueue, path)
	}

	if m.loadTotal == 0 || len(m.loadQueue) == 0 {
		m.finishLoading()
		return nil
	}
	return loadNextCmd(m.loadQueue, m.sessionCache, 0)
}

// startForceReload clears the cache and does a fresh progressive load.
func (m *Model) startForceReload() tea.Cmd {
	engine.ClearSessionCache()
	m.sessionCache = engine.SessionCache{
		Entries: make(map[string]engine.CacheEntry),
		Dirs:    make(map[string]engine.DirCacheEntry),
	}
	return m.startReload()
}

// appendSession 追加一条加载结果，并同步当前筛选后的列表。
func (m *Model) appendSession(s engine.Session, fromCache bool) {
	m.sessions = append(m.sessions, s)
	if !fromCache {
		m.unsavedNewCount++
	}
	m.loadProgress++
	if fromCache {
		m.loadedFromCache++
	}

	m.rebuildFilteredView()

	if m.unsavedNewCount >= cacheSaveInterval {
		engine.SaveSessionCache(m.sessionCache)
		m.unsavedNewCount = 0
	}
}

func (m *Model) finishLoading() {
	m.loading = false
	sort.SliceStable(m.sessions, func(i, j int) bool {
		return m.sessions[i].Metrics.SessionStart > m.sessions[j].Metrics.SessionStart
	})
	m.overview = engine.ComputeOverview(m.sessions)
	m.aggStats = engine.ComputeAggregateStats(m.sessions)
	m.costSummary = engine.ComputeCostSummary(m.sessions)
	engine.SaveSessionCache(m.sessionCache)
	m.unsavedNewCount = 0
	if m.sortBy != "" {
		m.sortAndRefresh()
		return
	}
	m.rebuildFilteredView()
}

func loadNextCmd(files []string, cache engine.SessionCache, idx int) tea.Cmd {
	return func() tea.Msg {
		if idx >= len(files) {
			return loadProgressMsg{done: true, total: len(files)}
		}
		path := files[idx]

		info, err := os.Stat(path)
		if err != nil {
			return loadProgressMsg{index: idx, total: len(files), err: err}
		}

		if s, ok := engine.CachedSession(path, cache); ok {
			return loadProgressMsg{
				session:   s,
				fromCache: true,
				index:     idx,
				total:     len(files),
			}
		}

		s, err := engine.LoadSession(path)
		if err != nil {
			return loadProgressMsg{index: idx, total: len(files), err: err}
		}

		// Fill in mtime+size for future cache hits
		s.Path = path
		cache.Entries[path] = engine.CacheEntry{
			ModTime: info.ModTime().UnixNano(),
			Size:    info.Size(),
			Session: *s,
		}

		return loadProgressMsg{
			session:   *s,
			fromCache: false,
			index:     idx,
			total:     len(files),
		}
	}
}

// ── Update ──

func (m Model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	var cmd tea.Cmd

	switch msg := msg.(type) {

	case startLoadMsg:
		cmd := m.startReload()
		return m, cmd

	case loadProgressMsg:
		if msg.done {
			m.finishLoading()
			return m, nil
		}
		if msg.err != nil {
			// Skip failed files, continue loading
			m.loadProgress++
			return m, loadNextCmd(m.loadQueue, m.sessionCache, msg.index+1)
		}
		m.appendSession(msg.session, msg.fromCache)
		return m, loadNextCmd(m.loadQueue, m.sessionCache, msg.index+1)

	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height

		m.adjustColumnWidths(msg.Width)
		m.table.SetWidth(m.frameBodyWidth())
		m.table.SetHeight(m.listTableHeight(2))

		if m.detailReady {
			m.viewport.Width = m.detailViewportWidth()
			m.viewport.Height = m.detailViewportHeight()
			m.refreshDetailViewport()
		}

	case tea.KeyMsg:
		// During loading, only allow quit
		if m.loading {
			switch msg.String() {
			case "q", "ctrl+c":
				return m, tea.Quit
			}
			return m, nil
		}
		if m.commandActive {
			switch msg.String() {
			case "ctrl+c":
				return m, tea.Quit
			case "enter":
				input := m.commandInput
				m.commandActive = false
				m.commandInput = ""
				m.runCommand(input)
			case "esc":
				m.commandActive = false
				m.commandInput = ""
			case "backspace":
				m.commandInput = dropLastRune(m.commandInput)
			default:
				if len(msg.Runes) > 0 {
					m.commandInput += string(msg.Runes)
				}
			}
			return m, nil
		}
		if m.filterActive {
			switch msg.String() {
			case "ctrl+c":
				return m, tea.Quit
			case "enter":
				m.filterText = m.filterInput
				m.filterActive = false
				m.rebuildFilteredView()
			case "esc":
				if m.hasAnyFilter() {
					m.clearFilters()
					m.commandFeedback = i18n.T("cmd_cleared")
				} else {
					m.filterActive = false
					m.filterInput = ""
				}
			case "backspace":
				m.filterInput = dropLastRune(m.filterInput)
			default:
				if len(msg.Runes) > 0 {
					m.filterInput += string(msg.Runes)
				}
			}
			return m, nil
		}
		switch msg.String() {
		case "q", "ctrl+c":
			return m, tea.Quit

		case ":":
			m.commandActive = true
			m.commandInput = ""

		case "$":
			if m.view == viewOverview {
				m.runCommand("top cost")
			}

		case "!":
			if m.view == viewOverview {
				m.runCommand("critical")
			}

		case "a":
			if m.view == viewOverview {
				m.runCommand("anomalies")
			}

		case "tab", "`":
			switch m.view {
			case viewOverview:
				m.view = viewList
			case viewList:
				m.view = viewDetail
				m.openDetail()
			case viewDetail:
				m.view = viewDiagnostics
			case viewDiagnostics:
				m.openDiff()
			case viewDiff:
				m.view = viewOverview
			}

		case "r":
			if !m.loading {
				cmd := m.startReload()
				return m, cmd
			}

		case "ctrl+r":
			if !m.loading {
				cmd := m.startForceReload()
				return m, cmd
			}

		case "L", "l":
			if m.lang == i18n.EN {
				m.lang = i18n.ZH
			} else {
				m.lang = i18n.EN
			}
			i18n.SetLang(m.lang)
			m.refreshColumns()
			m.refreshDetailViewport()

		case "0":
			m.view = viewOverview
		case "1":
			m.view = viewList
		case "2":
			if len(m.filteredIndices) > 0 {
				m.view = viewDetail
				m.openDetail()
			}
		case "3":
			m.view = viewDiagnostics
		case "4":
			m.openDiff()
		case "w":
			if (m.view == viewList || m.view == viewDetail) && len(m.filteredIndices) > 0 {
				m.view = viewDiagnostics
			}

		case "/":
			if m.view == viewOverview || m.view == viewList {
				m.view = viewList
				m.filterActive = true
				m.filterInput = ""
			}

		case "enter":
			if m.view == viewList && m.tableReady && len(m.filteredIndices) > 0 {
				m.view = viewDetail
				m.openDetail()
				return m, cmd
			}
			return m, cmd
		case "esc":
			if m.view == viewDetail || m.view == viewDiff || m.view == viewDiagnostics {
				m.view = viewList
			}
		case "backspace":

		case "f":
			if !m.filterActive && m.view == viewList {
				switch m.filterHealth {
				case "":
					m.filterHealth = "good"
				case "good":
					m.filterHealth = "warn"
				case "warn":
					m.filterHealth = "crit"
				default:
					m.filterHealth = ""
				}
				m.filterMode = ""
				m.filterValue = ""
				m.rebuildFilteredView()
			}

		case "s":
			if !m.filterActive && m.view == viewList {
				m.cycleSourceFilter()
				m.filterMode = ""
				m.filterValue = ""
				m.rebuildFilteredView()
			}

		case "d":
			if m.view == viewList {
				m.openDiff()
			} else if m.view == viewDetail {
				m.openDiff()
			}

		// Sort keys (list view)
		case "h":
			if m.view == viewList && !m.filterActive {
				if m.sortBy == "health" {
					m.sortDesc = !m.sortDesc
				} else {
					m.sortBy = "health"
					m.sortDesc = true
				}
				m.sortAndRefresh()
			}
		case "c":
			if m.view == viewList && !m.filterActive {
				if m.sortBy == "cost" {
					m.sortDesc = !m.sortDesc
				} else {
					m.sortBy = "cost"
					m.sortDesc = true
				}
				m.sortAndRefresh()
			}
		case "t":
			if m.view == viewList && !m.filterActive {
				if m.sortBy == "turns" {
					m.sortDesc = !m.sortDesc
				} else {
					m.sortBy = "turns"
					m.sortDesc = true
				}
				m.sortAndRefresh()
			}
		case "n":
			if m.view == viewList && !m.filterActive {
				if m.sortBy == "name" {
					m.sortDesc = !m.sortDesc
				} else {
					m.sortBy = "name"
					m.sortDesc = false
				}
				m.sortAndRefresh()
			}

		default:
			switch m.view {
			case viewList:
				m.table, cmd = m.table.Update(msg)
				if msg.String() == "enter" && m.tableReady && len(m.filteredIndices) > 0 {
					m.view = viewDetail
					m.openDetail()
				}
			case viewDetail:
				m.viewport, cmd = m.viewport.Update(msg)
			case viewDiff:
				// No sub-component in diff view
			}
		}
	}

	return m, cmd
}

func (m *Model) openDetail() {
	m.detailReady = false
	if !m.tableReady || len(m.sessions) == 0 {
		return
	}
	idx := m.findSessionIndex()
	if idx >= 0 && idx < len(m.sessions) {
		s := m.sessions[idx]

		m.prepareDetailState(s)
		m.viewport = viewport.New(m.detailViewportWidth(), m.detailViewportHeight())
		m.viewport.SetContent(m.renderDetailViewportContent(s))
		m.detailReady = true
	}
}

func (m *Model) openDiff() {
	m.prepareDiffForCursor()
	m.view = viewDiff
}

func (m *Model) prepareDetailState(s engine.Session) {
	m.fixSuggestions = engine.GenerateFixes(s.Metrics, s.Anomalies)
	m.costAlert = engine.PredictCostAnomaly(m.sessions, s)
	m.loopResult = s.LoopResultData
	m.toolWarnings = s.ToolWarnings
}

func (m *Model) refreshDetailViewport() {
	if !m.detailReady {
		return
	}
	if idx := m.findSessionIndex(); idx >= 0 && idx < len(m.sessions) {
		s := m.sessions[idx]
		m.prepareDetailState(s)
		m.viewport.SetContent(m.renderDetailViewportContent(s))
		return
	}
	m.detailReady = false
}

func (m Model) detailViewportHeight() int {
	if m.height <= 0 {
		return 12
	}
	fixedChrome := 15
	if m.width > 0 && m.width < 100 {
		fixedChrome = 16
	}
	return maxInt(4, m.height-fixedChrome)
}

func (m Model) detailViewportWidth() int {
	return maxInt(8, m.frameBodyWidth())
}

func (m Model) renderDetailViewportContent(s engine.Session) string {
	sections := []string{
		m.renderDiagnosticSummary(),
		"",
		engine.ReportText(safeReportMetrics(s.Metrics), s.Anomalies, clampHealth(s.Health)),
	}
	if loopSection := m.renderLoopAnalysis(); loopSection != "" {
		sections = append(sections, "", loopSection)
	}
	if fixSection := m.renderFixSuggestions(); fixSection != "" {
		sections = append(sections, "", fixSection)
	}
	if toolWarnSection := m.renderToolWarnings(); toolWarnSection != "" {
		sections = append(sections, "", toolWarnSection)
	}
	if costAlertSection := m.renderCostAlert(); costAlertSection != "" {
		sections = append(sections, "", costAlertSection)
	}
	return lipgloss.JoinVertical(lipgloss.Left, sections...)
}

func (m *Model) refreshTable() {
	var rows []table.Row
	for _, s := range m.sessions {
		rows = append(rows, m.sessionRow(s))
	}
	m.table.SetRows(rows)
	m.table.SetCursor(0)
}

func (m *Model) sessionRow(s engine.Session) table.Row {
	met := s.Metrics
	okTools, failTools, totalTools, totalToolCalls := normalizedToolCounts(met)
	sr := i18n.T("not_available")
	if totalTools > 0 {
		sr = fmt.Sprintf("%.0f", float64(okTools)/float64(totalTools)*100)
	}

	sourceDisplay := met.SourceTool
	if display, ok := engine.ToolDisplayNames[met.SourceTool]; ok {
		sourceDisplay = display
	}

	health := clampHealth(s.Health)
	healthRaw := fmt.Sprintf("%d%%", health)
	healthCol := healthRaw

	failStr := fmt.Sprintf("%d", failTools)
	if failTools > 0 {
		failStr = redStyle.Render(failStr)
	} else {
		failStr = dimStyle.Render(failStr)
	}

	tokensStr := compactInt(met.TokensInput + met.TokensOutput)
	switch len(m.table.Columns()) {
	case 5:
		return table.Row{
			s.Name,
			sourceDisplay,
			costColor(met.CostEstimated),
			tokensStr,
			healthCol,
		}
	case 12:
		return table.Row{
			s.Name,
			sourceDisplay,
			met.ModelUsed,
			fmt.Sprintf("%d", nonNegativeInt(met.AssistantTurns)),
			fmt.Sprintf("%d", totalToolCalls),
			sr,
			failStr,
			costColor(met.CostEstimated),
			tokensStr,
			engine.FmtDuration(chartValue(met.DurationSec)),
			fmt.Sprintf("%d", len(s.Anomalies)),
			healthCol,
		}
	case 8:
		return table.Row{
			s.Name,
			sourceDisplay,
			fmt.Sprintf("%d", nonNegativeInt(met.AssistantTurns)),
			fmt.Sprintf("%d", totalToolCalls),
			failStr,
			costColor(met.CostEstimated),
			tokensStr,
			healthCol,
		}
	}

	return table.Row{
		s.Name,
		sourceDisplay,
		fmt.Sprintf("%d", nonNegativeInt(met.AssistantTurns)),
		fmt.Sprintf("%d", totalToolCalls),
		sr,
		failStr,
		costColor(met.CostEstimated),
		tokensStr,
		healthCol,
	}
}

// ── View ──

func (m Model) renderLoading() string {
	header := m.renderAppHeader()

	width := m.width
	if width <= 0 {
		width = 80
	}
	if width < 40 {
		width = 40
	}
	contentW := maxInt(20, width-4)
	barWidth := contentW - 18
	if barWidth > 50 {
		barWidth = 50
	}
	if barWidth < 20 {
		barWidth = 20
	}

	if m.loadTotal == 0 {
		lines := []string{
			boldStyle.Render(i18n.T("loading_discovering")),
			"",
			dimStyle.Render(truncate(i18n.T("loading_scanning_hint"), contentW)),
		}
		return strings.Join([]string{
			header,
			lipgloss.NewStyle().Width(contentW).Padding(1, 2).Render(strings.Join(lines, "\n")),
		}, "\n")
	}

	progress := m.loadProgress
	if progress < 0 {
		progress = 0
	}
	if progress > m.loadTotal {
		progress = m.loadTotal
	}

	filled := 0
	if m.loadTotal > 0 {
		filled = progress * barWidth / m.loadTotal
	}
	bar := "[" + greenStyle.Render(strings.Repeat("█", filled)) +
		dimStyle.Render(strings.Repeat("░", barWidth-filled)) + "]"

	pct := 0
	if m.loadTotal > 0 {
		pct = progress * 100 / m.loadTotal
	}

	cacheInfo := ""
	if m.loadedFromCache > 0 {
		cacheInfo = fmt.Sprintf(" (%s)", fmt.Sprintf(i18n.T("loading_from_cache"), m.loadedFromCache))
	}

	lines := []string{
		boldStyle.Render(i18n.T("loading_sessions")),
		"",
		fmt.Sprintf("  %s  %d/%d  %d%%%s", bar, progress, m.loadTotal, pct, cacheInfo),
		"",
		dimStyle.Render(truncate(i18n.T("loading_parsing_hint"), contentW)),
	}

	return strings.Join([]string{
		header,
		lipgloss.NewStyle().Width(contentW).Padding(1, 2).Render(strings.Join(lines, "\n")),
	}, "\n")
}

func (m Model) View() string {
	header := m.renderAppHeader()
	tabs := m.renderTabs()

	if m.loading {
		return m.fitTerminalFrame(m.renderLoading())
	}

	var content string
	switch m.view {
	case viewOverview:
		content = m.renderOverview()
	case viewList:
		if len(m.sessions) == 0 {
			content = m.frameContent(lipgloss.NewStyle().Padding(1).Render(fmt.Sprintf(i18n.T("empty_sessions_hint"), m.dir, m.dir, m.dir)))
		} else {
			content = m.renderListView()
		}
	case viewDetail:
		if m.detailReady {
			scrollInfo := dimStyle.Render(fmt.Sprintf(" %s: %.0f%% ", i18n.T("scroll_label"), m.viewport.ScrollPercent()*100))

			summaryBar := m.renderQuickSummary()
			detailContent := lipgloss.JoinVertical(lipgloss.Left, summaryBar, "", scrollInfo, m.viewport.View())
			content = m.frameContent(detailContent)
		} else {
			content = m.frameContent(dimStyle.Render(m.selectionHint()))
		}
	case viewDiagnostics:
		content = m.renderWaste()
	case viewDiff:
		content = m.renderDiff()
	}
	if m.commandActive {
		content = lipgloss.JoinVertical(lipgloss.Left, m.renderCommandBar(), content)
	}

	help := m.renderHelp()

	return m.fitTerminalFrameWithFooter(strings.Join([]string{header, tabs, content}, "\n"), help)
}

func (m Model) renderCommandBar() string {
	width := m.contentWidth()
	style := lipgloss.NewStyle().
		Border(lipgloss.NormalBorder()).
		BorderForeground(lipgloss.Color("39")).
		Padding(0, 1)
	body := truncate(fmt.Sprintf(": %s_", m.commandInput), maxInt(1, width-4))
	return styleForOuterWidth(style, width).Render(body)
}

func (m Model) renderListView() string {
	contentW := m.frameBodyWidth()
	var sections []string
	extraLines := 0
	if filterBar := m.renderListStatusBar(contentW); filterBar != "" {
		sections = append(sections, filterBar)
		extraLines += renderedLineCount(filterBar)
	}
	if summary := m.renderSelectedSessionSummary(contentW); summary != "" {
		sections = append(sections, summary)
		extraLines += renderedLineCount(summary)
	}
	tableView := m.table
	tableView.SetHeight(m.listTableHeight(extraLines))
	sections = append(sections, tableView.View())
	return m.frameContent(lipgloss.JoinVertical(lipgloss.Left, sections...))
}

func (m Model) listTableHeight(extraContentLines int) int {
	if m.height <= 0 {
		return 20
	}
	// 预留头部、Tab、外框和底部帮助栏，避免表格把整屏顶乱。
	return maxInt(4, m.height-12-extraContentLines)
}

func (m Model) renderListStatusBar(width int) string {
	switch {
	case m.filterActive:
		style := lipgloss.NewStyle().
			Border(lipgloss.NormalBorder()).
			BorderForeground(lipgloss.Color("39")).
			Padding(0, 1)
		body := truncate(fmt.Sprintf("/ %s_", m.filterInput), maxInt(1, width-4))
		return styleForOuterWidth(style, width).
			Render(body)
	case m.hasAnyFilter():
		return dimStyle.Render(truncate(fmt.Sprintf("%s: %s · %d/%d %s",
			i18n.T("list_filter"),
			m.filterLabel(), len(m.filteredIndices), m.overview.TotalSessions, i18n.T("sessions_label")), width))
	case m.commandFeedback != "":
		return dimStyle.Render(truncate(m.commandFeedback, width))
	default:
		return ""
	}
}

func (m Model) renderSelectedSessionSummary(width int) string {
	idx := m.findSessionIndex()
	if idx < 0 || idx >= len(m.sessions) {
		return ""
	}
	s := m.sessions[idx]
	met := s.Metrics
	okTools, _, totalTools, _ := normalizedToolCounts(met)
	success := i18n.T("not_available")
	if totalTools > 0 {
		success = fmt.Sprintf("%.0f%%", float64(okTools)/float64(totalTools)*100)
	}

	issue := i18n.T("list_no_major_anomaly")
	if len(s.Anomalies) > 0 {
		issue = anomalyTypeLabel(s.Anomalies[0].Type)
	}
	line := fmt.Sprintf("%s  %s %d%%  %s $%.4f  %s %s  %s %d  %s %d/%d %s  %s %s",
		truncate(s.Name, 28),
		i18n.T("health"), clampHealth(s.Health),
		i18n.T("cost"), safeAmount(met.CostEstimated),
		i18n.T("tokens"), compactInt(met.TokensInput+met.TokensOutput),
		i18n.T("turns_header"), nonNegativeInt(met.AssistantTurns),
		i18n.T("tools"), okTools, totalTools, success,
		i18n.T("list_issue"), issue,
	)
	return dimStyle.Render(truncate(line, width))
}

func (m Model) selectionHint() string {
	if len(m.sessions) > 0 && m.hasAnyFilter() && len(m.filteredIndices) == 0 {
		return i18n.T("no_visible_sessions_hint")
	}
	return i18n.T("select_session_hint")
}

func (m Model) renderAppHeader() string {
	width := m.width
	if width <= 0 {
		width = 80
	}
	if width < 72 {
		raw := fmt.Sprintf("agenttrace v%s  %d %s", engine.Version, len(m.sessions), i18n.T("sessions_label"))
		line := lipgloss.JoinHorizontal(lipgloss.Left,
			brandStyle.Render("agenttrace"),
			" ",
			dimStyle.Render(fmt.Sprintf("v%s", engine.Version)),
			"  ",
			dimStyle.Render(fmt.Sprintf("%d %s", len(m.sessions), i18n.T("sessions_label"))),
		)
		if lipgloss.Width(raw) > width {
			line = brandStyle.Render("agenttrace") + " " + dimStyle.Render(fmt.Sprintf("v%s", engine.Version))
		}
		return lipgloss.JoinVertical(lipgloss.Left, truncate(line, width), dimStyle.Render(strings.Repeat("-", width)))
	}
	brand := titleStyle.Render("agenttrace")
	version := dimStyle.Render(fmt.Sprintf("v%s", engine.Version))
	subtitle := dimStyle.Render(i18n.T("app_subtitle"))
	left := lipgloss.JoinHorizontal(lipgloss.Center, brand, " ", version, "  ", subtitle)

	status := statusBadge(fmt.Sprintf("%d %s", len(m.sessions), i18n.T("sessions_label")), "82")
	health := clampHealth(int(m.aggStats.AvgHealth))
	if len(m.sessions) == 0 {
		health = 0
	}
	healthBadge := statusBadge(fmt.Sprintf("%d%% %s", health, healthLabel(health)), "82")
	langBadge := statusBadge(i18n.LangLabel(), "245")
	right := lipgloss.JoinHorizontal(lipgloss.Top, status, " ", healthBadge, " ", langBadge)
	if width < 100 {
		left = lipgloss.JoinHorizontal(lipgloss.Center, brand, " ", version)
		right = dimStyle.Render(fmt.Sprintf("%d %s", len(m.sessions), i18n.T("sessions_label")))
	}

	lineW := maxInt(1, width)
	if lipgloss.Width(left)+lipgloss.Width(right)+1 > lineW {
		right = truncate(right, maxInt(1, lineW-lipgloss.Width(left)-1))
	}
	gap := lineW - lipgloss.Width(left) - lipgloss.Width(right)
	if gap < 1 {
		gap = 1
	}
	line := lipgloss.JoinHorizontal(lipgloss.Center, left, strings.Repeat(" ", gap), right)
	return lipgloss.JoinVertical(lipgloss.Left, truncate(line, width), dimStyle.Render(strings.Repeat("─", width)))
}

func (m Model) renderTabs() string {
	width := m.width
	if width <= 0 {
		width = 80
	}
	tabs := []string{i18n.T("tab_overview"), i18n.T("tab_list"), i18n.T("tab_detail"), i18n.T("tab_diag"), i18n.T("tab_diff")}
	if width < 64 {
		tabs = []string{"0", "1", "2", "3", "4"}
	}
	var rendered []string
	for i, t := range tabs {
		active := int(m.view) == i
		pad := 2
		if width < 64 {
			pad = 1
		}
		if active {
			rendered = append(rendered, lipgloss.NewStyle().
				Background(lipgloss.Color("22")).
				Foreground(lipgloss.Color("82")).
				Bold(true).
				Padding(0, pad).
				Render(t))
		} else {
			rendered = append(rendered, lipgloss.NewStyle().
				Foreground(lipgloss.Color("244")).
				Border(lipgloss.NormalBorder(), false, false, true, false).
				BorderForeground(lipgloss.Color("238")).
				Padding(0, pad).
				Render(t))
		}
	}
	return lipgloss.NewStyle().Padding(0, 1, 1, 1).Render(
		lipgloss.JoinHorizontal(lipgloss.Top, rendered...),
	)
}

func (m Model) renderHelp() string {
	if m.loading {
		return helpStyle.Render(" " + i18n.T("help_loading"))
	}
	var keys string
	if m.commandActive {
		keys = i18n.T("help_command")
	} else {
		switch m.view {
		case viewOverview:
			keys = i18n.T("help_overview_modern")
		case viewList:
			if m.filterActive {
				keys = i18n.T("help_list") + " · " + i18n.T("help_filter_clear")
			} else {
				keys = i18n.T("help_list") + " · " + i18n.T("help_force_reload")
			}
		case viewDetail:
			keys = i18n.T("help_detail")
		case viewDiagnostics:
			keys = i18n.T("help_diag")
		case viewDiff:
			keys = i18n.T("help_diff")
		}
	}
	width := m.width
	if width <= 0 {
		width = 80
	}
	if width < 40 {
		width = 40
	}
	maxKeys := maxInt(1, width-10)
	if lipgloss.Width(keys) > maxKeys {
		keys = truncate(keys, maxKeys)
	}
	meta := []string{m.viewName(), fmt.Sprintf("%d/%d", len(m.filteredIndices), len(m.sessions))}
	if m.sortBy != "" {
		dir := i18n.T("sort_asc")
		if m.sortDesc {
			dir = i18n.T("sort_desc")
		}
		meta = append(meta, i18n.T("sort_label")+" "+sortFieldLabel(m.sortBy)+" "+dir)
	}
	if m.hasAnyFilter() {
		meta = append(meta, i18n.T("list_filter")+" "+m.filterLabel())
	}
	if cache := m.cacheStatusLabel(); cache != "" {
		meta = append(meta, cache)
	}
	if m.commandFeedback != "" {
		meta = append(meta, m.commandFeedback)
	}
	lineW := maxInt(1, width-8)
	leftText := strings.Join(meta, " · ")
	rightText := keys
	if lipgloss.Width(leftText)+lipgloss.Width(rightText)+3 > lineW {
		rightBudget := minInt(lipgloss.Width(rightText), maxInt(0, lineW/3))
		leftBudget := lineW - rightBudget - 3
		if leftBudget < 12 {
			leftBudget = maxInt(1, lineW-3)
			rightBudget = 0
		}
		leftText = truncate(leftText, leftBudget)
		if rightBudget > 0 {
			rightText = truncate(rightText, rightBudget)
		} else {
			rightText = ""
		}
	}
	left := statusStyle.Render(leftText)
	right := dimStyle.Render(rightText)
	gap := lineW - lipgloss.Width(left) - lipgloss.Width(right)
	if gap < 1 {
		gap = 1
	}
	return helpStyle.Width(maxInt(1, width-4)).Render(" " + left + strings.Repeat(" ", gap) + right + " ")
}

func (m Model) cacheStatusLabel() string {
	if m.loadTotal <= 0 {
		return ""
	}
	cached := m.loadedFromCache
	if cached < 0 {
		cached = 0
	}
	if cached > m.loadTotal {
		cached = m.loadTotal
	}
	return fmt.Sprintf(i18n.T("cache_status"), cached, m.loadTotal)
}

func (m Model) viewName() string {
	switch m.view {
	case viewOverview:
		return i18n.T("view_overview")
	case viewList:
		return i18n.T("view_list")
	case viewDetail:
		return i18n.T("view_detail")
	case viewDiagnostics:
		return i18n.T("view_diagnostics")
	case viewDiff:
		return i18n.T("view_diff")
	default:
		return i18n.T("view_unknown")
	}
}

// renderQuickSummary shows key metrics at a glance above the detail viewport.
func (m Model) renderQuickSummary() string {
	idx := m.findSessionIndex()
	if idx < 0 || idx >= len(m.sessions) {
		return ""
	}
	s := m.sessions[idx]
	met := s.Metrics

	badge := func(label, value string, color lipgloss.Color) string {
		return lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(color).
			Padding(0, 1).
			Render(lipgloss.NewStyle().Foreground(color).Render(label) + " " + boldStyle.Render(value))
	}

	health := clampHealth(s.Health)
	healthBadge := badge(i18n.T("health"), fmt.Sprintf("%d/100", health), lipgloss.Color("42"))
	if health < 80 {
		healthBadge = badge(i18n.T("health"), fmt.Sprintf("%d/100", health), lipgloss.Color("220"))
	}
	if health < 50 {
		healthBadge = badge(i18n.T("health"), fmt.Sprintf("%d/100", health), lipgloss.Color("196"))
	}

	costBadge := badge(i18n.T("cost"), money4(met.CostEstimated), lipgloss.Color("39"))

	okTools, _, totalTools, _ := normalizedToolCounts(met)
	srStr := i18n.T("not_available")
	if totalTools > 0 {
		srStr = fmt.Sprintf("%.0f%%", float64(okTools)/float64(totalTools)*100)
	}
	toolColor := lipgloss.Color("42")
	if totalTools > 0 && float64(okTools)/float64(totalTools) < 0.85 {
		toolColor = lipgloss.Color("220")
	}
	if totalTools > 0 && float64(okTools)/float64(totalTools) < 0.70 {
		toolColor = lipgloss.Color("196")
	}
	toolBadge := badge(i18n.T("tools"), fmt.Sprintf("%d/%d %s", okTools, totalTools, srStr), toolColor)

	anomBadge := badge(i18n.T("diff_field_anomalies"), fmt.Sprintf("%d", len(s.Anomalies)), lipgloss.Color("42"))
	if len(s.Anomalies) > 0 {
		anomBadge = badge(i18n.T("diff_field_anomalies"), fmt.Sprintf("%d", len(s.Anomalies)), lipgloss.Color("196"))
	}

	modelBadge := badge(i18n.T("model_header"), met.ModelUsed, lipgloss.Color("99"))
	if m.width > 0 && m.width < 100 {
		lineW := m.frameBodyWidth()
		issue := i18n.T("list_no_major_anomaly")
		if len(s.Anomalies) > 0 {
			issue = anomalyTypeLabel(s.Anomalies[0].Type)
		}
		line1 := fmt.Sprintf("%s %d/100  %s %s  %s %d/%d %s",
			i18n.T("health"),
			health,
			i18n.T("cost"),
			money4(met.CostEstimated),
			i18n.T("tools"),
			okTools,
			totalTools,
			srStr)
		line2 := fmt.Sprintf("%s %d  %s %s  %s",
			i18n.T("diff_field_anomalies"),
			len(s.Anomalies),
			i18n.T("list_issue"),
			issue,
			met.ModelUsed)
		return lipgloss.JoinVertical(lipgloss.Left,
			truncate(line1, lineW),
			dimStyle.Render(truncate(line2, lineW)),
		)
	}

	return lipgloss.JoinHorizontal(lipgloss.Top,
		healthBadge, " ", costBadge, " ", toolBadge, " ", anomBadge, " ", modelBadge,
	)
}

// renderLoopAnalysis 渲染循环成本分析 section
func (m Model) renderLoopAnalysis() string {
	if !m.detailReady {
		return ""
	}
	lr := m.loopResult
	if lr.HasLoop {
		loopCard := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(lipgloss.Color("196")).
			Padding(0, 1)
		content := fmt.Sprintf("%s %s: %s — %d %s, %s $%.4f",
			redStyle.Render("⚠"),
			boldStyle.Render(i18n.T("loop_detected")),
			lr.LoopType,
			lr.Turns,
			i18n.T("loop_turns"),
			i18n.T("loop_cost"),
			lr.LoopCost)
		return loopCard.Render(content)
	}
	loopCard := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color("42")).
		Padding(0, 1)
	content := fmt.Sprintf("%s %s",
		greenStyle.Render("✅"),
		i18n.T("no_loop"))
	return loopCard.Render(content)
}

// renderFixSuggestions renders fix suggestions for the current detail view.
func (m Model) renderFixSuggestions() string {
	if len(m.fixSuggestions) == 0 {
		return ""
	}
	title := lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("42")).Render(i18n.T("fix_title"))
	var lines []string
	lines = append(lines, title)
	cardW := maxInt(20, m.detailViewportWidth())
	for _, fs := range m.fixSuggestions {
		msg := fmt.Sprintf("• [%s] %s -> %s", fs.Title, fs.Description, fs.Action)
		style := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(lipgloss.Color("42")).
			Padding(0, 1)
		lines = append(lines, styleForOuterWidth(style, cardW).Render(greenStyle.Render(msg)))
	}
	return lipgloss.JoinVertical(lipgloss.Left, lines...)
}

// renderToolWarnings renders tool call warnings for the current detail view.
func (m Model) renderToolWarnings() string {
	if len(m.toolWarnings) == 0 {
		return ""
	}
	title := lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("220")).Render(i18n.T("tool_warn_title"))
	var lines []string
	lines = append(lines, title)
	cardW := maxInt(20, m.detailViewportWidth())
	for _, tw := range m.toolWarnings {
		msg := fmt.Sprintf("! [%s] %s (x%d)", tw.Pattern, tw.Detail, tw.Count)
		style := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(lipgloss.Color("220")).
			Padding(0, 1)
		lines = append(lines, styleForOuterWidth(style, cardW).Render(yellowStyle.Render(msg)))
	}
	return lipgloss.JoinVertical(lipgloss.Left, lines...)
}

// renderCostAlert renders cost anomaly alerts for the current detail view.
func (m Model) renderCostAlert() string {
	if !m.costAlert.Triggered {
		return ""
	}
	title := lipgloss.NewStyle().Bold(true).Render(i18n.T("cost_alert_title"))
	var lines []string
	lines = append(lines, title)

	msg := m.costAlert.Message

	borderColor := lipgloss.Color("196")
	if m.costAlert.Level == "warning" {
		borderColor = lipgloss.Color("220")
	}
	style := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(borderColor).
		Padding(0, 1)
	lines = append(lines, styleForOuterWidth(style, maxInt(20, m.detailViewportWidth())).
		Render(truncate(msg, maxInt(8, m.detailViewportWidth()-4))))
	return lipgloss.JoinVertical(lipgloss.Left, lines...)
}

// ═══════════════════════════════════════════════════════════════
// WASTE DIAGNOSTICS VIEW
// ═══════════════════════════════════════════════════════════════

func (m Model) renderWaste() string {
	idx := m.findSessionIndex()
	if idx < 0 || idx >= len(m.sessions) {
		return m.frameContent(dimStyle.Render(m.selectionHint()))
	}
	s := m.sessions[idx]
	panelW := m.frameBodyWidth()
	cardW := panelW
	twoColumn := panelW >= 92
	if twoColumn {
		cardW = (panelW - 2) / 2
	}

	pCard := func(color, title string, content string) string {
		borderColor := lipgloss.Color(color)
		header := lipgloss.NewStyle().Bold(true).Foreground(borderColor).Render(truncate(title, maxInt(8, cardW-4)))
		style := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(lipgloss.Color("238"))
		body := lipgloss.JoinVertical(lipgloss.Left, header, "", truncateLines(content, maxInt(8, cardW-8)))
		if twoColumn {
			style = style.Padding(0, 1)
			body = lipgloss.JoinVertical(lipgloss.Left, header, truncateLines(content, maxInt(8, cardW-4)))
		} else {
			style = style.Padding(1, 2)
		}
		return styleForOuterWidth(style, cardW).
			Render(body)
	}

	sessionLabel := cyanStyle.Render(truncate(fmt.Sprintf(i18n.T("diag_for"), s.Name), panelW))
	var sections []string
	sections = append(sections, sessionLabel, "")

	// Waste Score Summary
	scoreCard := m.renderWasteScoreCard(s)
	if scoreCard != "" {
		sections = append(sections, scoreCard, "")
	}

	cards := []string{
		pCard("196", i18n.T("diag_loop_fingerprint"), m.renderFingerprintLoops(s)),
		pCard("39", i18n.T("diag_tool_latency"), m.renderToolLatency(s)),
		pCard("220", i18n.T("diag_context_util"), m.renderContextUtil(s)),
		pCard("208", i18n.T("diag_large_params"), m.renderLargeParams(s)),
		pCard("42", i18n.T("diag_unused_tools"), m.renderUnusedTools(s)),
	}
	if twoColumn {
		var rows []string
		for i := 0; i < len(cards); i += 2 {
			left := cards[i]
			right := ""
			if i+1 < len(cards) {
				right = cards[i+1]
			}
			if right == "" {
				rows = append(rows, left)
			} else {
				rows = append(rows, lipgloss.JoinHorizontal(lipgloss.Top, left, "  ", right))
			}
		}
		sections = append(sections, lipgloss.JoinVertical(lipgloss.Left, rows...))
		return m.frameContent(lipgloss.JoinVertical(lipgloss.Left, sections...))
	}
	sections = append(sections, lipgloss.JoinVertical(lipgloss.Left, cards...))
	return m.frameContent(lipgloss.JoinVertical(lipgloss.Left, sections...))
}

func (m Model) renderFingerprintLoops(s engine.Session) string {
	if len(s.LoopFingerprints) == 0 {
		if s.LoopCost.TotalLoopCost > 0 {
			return yellowStyle.Render(fmt.Sprintf(i18n.T("diag_simple_loop"), s.LoopCost.TotalLoopCost))
		}
		return greenStyle.Render(i18n.T("diag_no_loop_fp"))
	}
	var lines []string
	for _, fp := range s.LoopFingerprints {
		style := redStyle
		if fp.Severity == "critical" {
			style = lipgloss.NewStyle().Foreground(lipgloss.Color("196")).Bold(true)
		}
		lines = append(lines, style.Render(fmt.Sprintf("• %s", fp.Detail)))
	}
	return lipgloss.JoinVertical(lipgloss.Left, lines...)
}

func (m Model) renderWasteScoreCard(s engine.Session) string {
	met := s.Metrics
	score := 100
	hasIssues := false

	var worstLabel string
	switch {
	case len(s.LoopFingerprints) > 0:
		score -= 30 * len(s.LoopFingerprints)
		worstLabel = fmt.Sprintf("%d fingerprint loops detected", len(s.LoopFingerprints))
		hasIssues = true
	case s.LoopCost.TotalLoopCost > 0:
		score -= 15
		worstLabel = "simple loop detected"
		hasIssues = true
	}

	slowTools := 0
	for _, tl := range s.ToolLatencies {
		if tl.IsSlow {
			slowTools++
		}
	}
	if slowTools > 0 {
		score -= 10 * slowTools
		if worstLabel == "" {
			worstLabel = fmt.Sprintf("%d slow tools", slowTools)
		}
		hasIssues = true
	}

	if s.ContextUtil.RiskLevel == "critical" {
		score -= 25
		if worstLabel == "" {
			worstLabel = "context window critical"
		}
		hasIssues = true
	} else if s.ContextUtil.RiskLevel == "warning" {
		score -= 10
		if worstLabel == "" {
			worstLabel = "context window warning"
		}
		hasIssues = true
	}

	if len(s.LargeParams) > 0 {
		score -= 10 * len(s.LargeParams)
		if worstLabel == "" {
			worstLabel = fmt.Sprintf("%d large param calls", len(s.LargeParams))
		}
		hasIssues = true
	}

	if len(s.UnusedTools) > 0 {
		score -= 5 * len(s.UnusedTools)
		if worstLabel == "" {
			worstLabel = fmt.Sprintf("%d rare tools", len(s.UnusedTools))
		}
		hasIssues = true
	}

	if score < 0 {
		score = 0
	}
	if score > 100 {
		score = 100
	}

	var scoreStyle lipgloss.Style
	switch {
	case score >= 80:
		scoreStyle = lipgloss.NewStyle().Foreground(lipgloss.Color("42")).Bold(true)
	case score >= 50:
		scoreStyle = lipgloss.NewStyle().Foreground(lipgloss.Color("220")).Bold(true)
	default:
		scoreStyle = lipgloss.NewStyle().Foreground(lipgloss.Color("196")).Bold(true)
	}

	status := i18n.T("diag_all_clear")
	if hasIssues {
		status = i18n.T("diag_issues_found")
	}
	panelW := m.frameBodyWidth()
	if m.width > 0 && m.width < 100 {
		content := lipgloss.JoinVertical(lipgloss.Left,
			fmt.Sprintf("%s  %s",
				scoreStyle.Render(fmt.Sprintf(i18n.T("diag_score"), score)),
				lipgloss.NewStyle().Bold(true).Render(status)),
			fmt.Sprintf("%s  %s  %s",
				dimStyle.Render(fmt.Sprintf("%s %d", i18n.T("turns_header"), nonNegativeInt(met.AssistantTurns))),
				dimStyle.Render(fmt.Sprintf("%s %d", i18n.T("tools"), nonNegativeInt(met.ToolCallsTotal))),
				dimStyle.Render(fmt.Sprintf("%s %s", i18n.T("cost"), money4(met.CostEstimated)))),
			lipgloss.NewStyle().Foreground(lipgloss.Color("99")).Render(truncate(met.ModelUsed, panelW-4)),
		)
		style := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(lipgloss.Color("63")).
			Padding(1, 2)
		return styleForOuterWidth(style, panelW).Render(content)
	}

	style := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color("63")).
		Padding(1, 2)
	return styleForOuterWidth(style, panelW).Render(fmt.Sprintf("%s  %s  ·  %s  ·  %s  ·  %s  ·  %s",
		scoreStyle.Render(fmt.Sprintf(i18n.T("diag_score"), score)),
		lipgloss.NewStyle().Bold(true).Render(status),
		dimStyle.Render(fmt.Sprintf("%s %d", i18n.T("turns_header"), nonNegativeInt(met.AssistantTurns))),
		dimStyle.Render(fmt.Sprintf("%s %d", i18n.T("tools"), nonNegativeInt(met.ToolCallsTotal))),
		dimStyle.Render(fmt.Sprintf("%s %s", i18n.T("cost"), money4(met.CostEstimated))),
		lipgloss.NewStyle().Foreground(lipgloss.Color("99")).Render(met.ModelUsed),
	))
}

func (m Model) renderToolLatency(s engine.Session) string {
	if len(s.ToolLatencies) == 0 {
		return dimStyle.Render(i18n.T("diag_no_latency"))
	}
	if m.frameBodyWidth() < 70 {
		var lines []string
		lines = append(lines, dimStyle.Render(fmt.Sprintf("  %-18s %5s %5s",
			i18n.T("diag_tool_lat_col_name"),
			i18n.T("diag_tool_lat_col_avg"),
			i18n.T("diag_tool_lat_col_max"))))
		for i, tl := range s.ToolLatencies {
			if i >= 6 {
				break
			}
			name := truncate(tl.ToolName, 18)
			avgSec := chartValue(tl.AvgSec)
			maxSec := chartValue(tl.MaxSec)
			marker := ""
			if tl.IsSlow {
				marker = redStyle.Render(" " + i18n.T("diag_tool_slow_badge"))
			}
			lines = append(lines, fmt.Sprintf("  %-18s %5.1fs %5.1fs%s", name, avgSec, maxSec, marker))
		}
		return lipgloss.JoinVertical(lipgloss.Left, lines...)
	}
	header := lipgloss.NewStyle().Foreground(lipgloss.Color("245")).Render(
		fmt.Sprintf("  %-18s %5s %6s %6s %6s %4s",
			i18n.T("diag_tool_lat_col_name"),
			i18n.T("diag_tool_lat_col_count"),
			i18n.T("diag_tool_lat_col_avg"),
			i18n.T("diag_tool_lat_col_p95"),
			i18n.T("diag_tool_lat_col_max"),
			i18n.T("diag_tool_lat_col_timeout")))
	var rows []string
	rows = append(rows, header)
	for i, tl := range s.ToolLatencies {
		if i >= 8 {
			break
		}
		slowBadge := ""
		if tl.IsSlow {
			slowBadge = redStyle.Render(" " + i18n.T("diag_tool_slow_badge"))
		}
		timeoutStr := "0"
		if tl.Timeouts > 0 {
			timeoutStr = redStyle.Render(fmt.Sprintf("%d", nonNegativeInt(tl.Timeouts)))
		}
		count := nonNegativeInt(tl.Count)
		avgSec := chartValue(tl.AvgSec)
		p95Sec := chartValue(tl.P95Sec)
		maxSec := chartValue(tl.MaxSec)
		style := lipgloss.NewStyle()
		if tl.IsSlow {
			style = style.Foreground(lipgloss.Color("196"))
		}
		rows = append(rows, style.Render(fmt.Sprintf("  %-18s %5d %5.1fs %5.1fs %5.1fs %4s%s",
			tl.ToolName, count, avgSec, p95Sec, maxSec, timeoutStr, slowBadge)))
	}
	return lipgloss.JoinVertical(lipgloss.Left, rows...)
}

func (m Model) renderContextUtil(s engine.Session) string {
	cu := s.ContextUtil
	var style lipgloss.Style
	switch cu.RiskLevel {
	case "critical":
		style = redStyle
	case "warning":
		style = yellowStyle
	default:
		style = greenStyle
	}
	barWidth := 30
	if m.frameBodyWidth() < 70 {
		barWidth = maxInt(8, m.frameBodyWidth()-34)
	}
	utilPct := cu.UtilizationPct
	if !isFiniteNumber(utilPct) || utilPct < 0 {
		utilPct = 0
	} else if utilPct > 100 {
		utilPct = 100
	}
	filled := int(utilPct / 100 * float64(barWidth))
	bar := "[" + lipgloss.NewStyle().Foreground(lipgloss.Color("196")).Render(strings.Repeat("█", filled)) +
		lipgloss.NewStyle().Foreground(lipgloss.Color("240")).Render(strings.Repeat("░", barWidth-filled)) + "]"

	if m.frameBodyWidth() < 70 {
		content := fmt.Sprintf("  %-10s %s\n", i18n.T("diag_ctx_total"), cyanStyle.Render(tokenCount(cu.EstimatedTotal)))
		content += fmt.Sprintf("  %-10s %s\n", i18n.T("diag_ctx_tool_defs"), dimStyle.Render(tokenCount(cu.ToolDefinitions)))
		content += fmt.Sprintf("  %-10s %s\n", i18n.T("diag_ctx_history"), dimStyle.Render(tokenCount(cu.ConversationHist)))
		content += fmt.Sprintf("  %-10s %s %s\n", i18n.T("diag_ctx_available"), style.Render(fmt.Sprintf("%d", nonNegativeInt(cu.AvailableForTask))), bar)
		content += fmt.Sprintf("  %-10s %s\n", i18n.T("diag_ctx_suggestion"), style.Render(truncate(cu.Suggestion, maxInt(8, m.frameBodyWidth()-18))))
		return content
	}

	content := fmt.Sprintf("  %-22s %s\n", i18n.T("diag_ctx_total"), cyanStyle.Render(tokenCount(cu.EstimatedTotal)))
	content += fmt.Sprintf("  %-22s %s\n", i18n.T("diag_ctx_tool_defs"), dimStyle.Render(tokenCount(cu.ToolDefinitions)))
	content += fmt.Sprintf("  %-22s %s\n", i18n.T("diag_ctx_history"), dimStyle.Render(tokenCount(cu.ConversationHist)))
	content += fmt.Sprintf("  %-22s %s\n", i18n.T("diag_ctx_sysprompt"), dimStyle.Render(tokenCount(cu.SystemPrompt)))
	content += fmt.Sprintf("  ─────────────────────────────\n")
	content += fmt.Sprintf("  %-22s %s %s\n", i18n.T("diag_ctx_available"),
		style.Render(tokenCount(cu.AvailableForTask)), bar)
	content += fmt.Sprintf("  %-22s %s\n", i18n.T("diag_ctx_suggestion"), style.Render(cu.Suggestion))
	return content
}

func (m Model) renderLargeParams(s engine.Session) string {
	if len(s.LargeParams) == 0 {
		return greenStyle.Render(i18n.T("diag_large_params_none"))
	}
	var lines []string
	for _, lp := range s.LargeParams {
		style := yellowStyle
		if lp.Risk == "high" {
			style = redStyle
		}
		kb := float64(nonNegativeInt(lp.ParamSize)) / 1024.0
		lines = append(lines, style.Render(fmt.Sprintf("• [%s] %s: %.1f KB — %s", riskLabel(lp.Risk), lp.ToolName, kb, lp.Detail)))
	}
	return lipgloss.JoinVertical(lipgloss.Left, lines...)
}

func (m Model) renderUnusedTools(s engine.Session) string {
	if len(s.UnusedTools) == 0 {
		return greenStyle.Render(i18n.T("diag_unused_none"))
	}
	var lines []string
	for _, ut := range s.UnusedTools {
		style := yellowStyle
		lines = append(lines, style.Render(fmt.Sprintf("• %s", ut.Detail)))
	}
	return lipgloss.JoinVertical(lipgloss.Left, lines...)
}

// ── Cumulative Cost Panel (for Overview) ──

func (m Model) renderCostSummaryCard() string {
	cs := m.costSummary
	if cs.TotalSessions == 0 {
		return ""
	}
	content := fmt.Sprintf("  %-22s %s\n", i18n.T("diag_cost_total_sessions"), cyanStyle.Render(fmt.Sprintf("%d", cs.TotalSessions)))
	content += fmt.Sprintf("  %-22s %s\n", i18n.T("diag_cost_total_burned"), redStyle.Render(money2(cs.TotalCost)))
	content += fmt.Sprintf("  %-22s %s\n", i18n.T("diag_cost_avg_turn"), yellowStyle.Render(money4(cs.AvgCostPerTurn)))
	content += fmt.Sprintf("  %-22s %s\n", i18n.T("diag_cost_costliest"), orangeStyle.Render(fmt.Sprintf("%s %s", cs.CostliestModel, money2(cs.CostliestCost))))
	content += fmt.Sprintf("  %-22s %s\n", i18n.T("diag_tokens_total"), fmt.Sprintf("%d / %d", cs.TotalTokensIn, cs.TotalTokensOut))
	content += fmt.Sprintf("  %-22s %s", i18n.T("diag_cache_total"), fmt.Sprintf("%d / %d", cs.TotalCacheRead, cs.TotalCacheWrite))

	return lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color("208")).
		Padding(1, 2).
		Render(lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("208")).Render(i18n.T("diag_cost_summary")) + "\n" + content)
}

// renderDiff renders the side-by-side session diff view.
func (m Model) renderDiff() string {
	dr := m.diffResult
	if len(dr.Entries) == 0 {
		hint := i18n.T("select_session_hint")
		visibleCount := len(m.filteredIndices)
		if visibleCount == 0 && !m.hasAnyFilter() {
			visibleCount = len(m.sessions)
		}
		if len(m.sessions) < 2 || visibleCount < 2 {
			hint = i18n.T("diff_need_two")
		} else {
			hint = i18n.T("diff_select_neighbor")
		}
		return m.frameContent(dimStyle.Render(hint))
	}
	contentW := m.frameBodyWidth()

	winner, explanation := buildDiffInsight(dr)
	winStyle := greenStyle
	if winner == i18n.T("diff_tie") {
		winStyle = yellowStyle
	}
	insight := subtlePanel(i18n.T("diff_comparison"), winStyle.Render(fmt.Sprintf(i18n.T("diff_winner_label"), winner))+dimStyle.Render(" · ")+truncate(explanation, maxInt(8, contentW-24)), contentW)

	if contentW < 82 {
		var rows []string
		rows = append(rows, insight)
		for _, e := range dr.Entries {
			rows = append(rows, fmt.Sprintf("%s  %s → %s %s",
				dimStyle.Render(diffFieldLabel(e.Field)),
				truncate(e.ValueA, 16),
				truncate(e.ValueB, 16),
				diffDeltaStyle(e).Render(e.Delta)))
		}
		rows = append(rows, "", greenStyle.Render(truncate(dr.Summary, contentW-4)))
		return m.frameContent(lipgloss.JoinVertical(lipgloss.Left, rows...))
	}

	panelW := (contentW - 2) / 2
	left := m.renderDiffSessionPanel("A", dr.SessionA, dr.Entries, true, panelW)
	right := m.renderDiffSessionPanel("B", dr.SessionB, dr.Entries, false, contentW-panelW-2)
	summary := greenStyle.Render(truncate(dr.Summary, contentW-4))
	body := lipgloss.JoinVertical(lipgloss.Left,
		insight,
		lipgloss.JoinHorizontal(lipgloss.Top, left, "  ", right),
		"",
		summary,
	)
	return m.frameContent(body)
}

func (m Model) renderDiffSessionPanel(label, name string, entries []engine.DiffEntry, leftSide bool, width int) string {
	var lines []string
	bodyW := maxInt(8, width-8)
	lines = append(lines, boldStyle.Render(label+"  ")+cyanStyle.Render(truncate(name, bodyW-3)), dimStyle.Render(strings.Repeat("─", minInt(bodyW, 48))))
	for _, e := range entries {
		value := e.ValueB
		if leftSide {
			value = e.ValueA
		}
		style := lipgloss.NewStyle()
		if (leftSide && e.Better == "A") || (!leftSide && e.Better == "B") {
			style = greenStyle
		} else if (leftSide && e.Better == "B") || (!leftSide && e.Better == "A") {
			style = yellowStyle
		}
		delta := ""
		if !leftSide {
			deltaW := maxInt(1, bodyW-17-lipgloss.Width(value))
			delta = " " + diffDeltaStyle(e).Render(truncate(e.Delta, deltaW))
		}
		lines = append(lines, fmt.Sprintf("%-14s %s%s", dimStyle.Render(truncate(diffFieldLabel(e.Field), 13)), style.Render(truncate(value, maxInt(4, bodyW-17))), delta))
	}
	return subtlePanel("", lipgloss.JoinVertical(lipgloss.Left, lines...), width)
}

func diffDeltaStyle(e engine.DiffEntry) lipgloss.Style {
	switch e.Better {
	case "B":
		return greenStyle
	case "A":
		return yellowStyle
	default:
		return dimStyle
	}
}

// prepareDiffForCursor chooses a neighbor pair for the current selection.
func (m *Model) prepareDiffForCursor() bool {
	if len(m.sessions) < 2 {
		m.diffResult = engine.SessionDiff{}
		return false
	}
	if len(m.filteredIndices) == 0 {
		m.rebuildFilteredIndices()
	}
	if len(m.filteredIndices) < 2 {
		m.diffResult = engine.SessionDiff{}
		return false
	}
	cursor := m.table.Cursor()
	if cursor < 0 {
		cursor = 0
	}
	if cursor >= len(m.filteredIndices) {
		cursor = len(m.filteredIndices) - 1
	}
	a := cursor
	b := cursor + 1
	if b >= len(m.filteredIndices) {
		a = cursor - 1
		b = cursor
	}
	if a < 0 || b < 0 {
		m.diffResult = engine.SessionDiff{}
		return false
	}
	m.diffResult = engine.DiffSessions(safeDiffSession(m.sessions[m.filteredIndices[a]]), safeDiffSession(m.sessions[m.filteredIndices[b]]))
	return true
}

// findSessionIndex returns the index of the currently-viewed session in m.sessions.
func (m *Model) findSessionIndex() int {
	if len(m.sessions) == 0 {
		return -1
	}
	cursor := m.table.Cursor()
	if m.hasAnyFilter() || len(m.filteredIndices) > 0 {
		if cursor >= 0 && cursor < len(m.filteredIndices) {
			return m.filteredIndices[cursor]
		}
		return -1
	}
	if cursor >= 0 && cursor < len(m.sessions) {
		return cursor
	}
	return -1
}

func (m *Model) sortAndRefresh() {
	selected := m.selectedSessionKey()
	switch m.sortBy {
	case "health":
		sort.SliceStable(m.sessions, func(i, j int) bool {
			if m.sortDesc {
				return clampHealth(m.sessions[i].Health) > clampHealth(m.sessions[j].Health)
			}
			return clampHealth(m.sessions[i].Health) < clampHealth(m.sessions[j].Health)
		})
	case "cost":
		sort.SliceStable(m.sessions, func(i, j int) bool {
			if m.sortDesc {
				return safeAmount(m.sessions[i].Metrics.CostEstimated) > safeAmount(m.sessions[j].Metrics.CostEstimated)
			}
			return safeAmount(m.sessions[i].Metrics.CostEstimated) < safeAmount(m.sessions[j].Metrics.CostEstimated)
		})
	case "turns":
		sort.SliceStable(m.sessions, func(i, j int) bool {
			if m.sortDesc {
				return nonNegativeInt(m.sessions[i].Metrics.AssistantTurns) > nonNegativeInt(m.sessions[j].Metrics.AssistantTurns)
			}
			return nonNegativeInt(m.sessions[i].Metrics.AssistantTurns) < nonNegativeInt(m.sessions[j].Metrics.AssistantTurns)
		})
	case "source":
		sort.SliceStable(m.sessions, func(i, j int) bool {
			left := sourceDisplayName(m.sessions[i].Metrics.SourceTool)
			right := sourceDisplayName(m.sessions[j].Metrics.SourceTool)
			if m.sortDesc {
				return left > right
			}
			return left < right
		})
	case "name":
		sort.SliceStable(m.sessions, func(i, j int) bool {
			if m.sortDesc {
				return m.sessions[i].Name > m.sessions[j].Name
			}
			return m.sessions[i].Name < m.sessions[j].Name
		})
	}
	m.refreshColumns()
	m.restoreSelection(selected)
}

func (m *Model) sortColTitle(base, field string) string {
	if m.sortBy == "" || m.sortBy != field {
		return base
	}
	if m.sortDesc {
		return base + " ▼"
	}
	return base + " ▲"
}

// costColor returns a styled cost string based on amount thresholds.
func costColor(amount float64) string {
	amount = safeAmount(amount)
	s := money4(amount)
	switch {
	case amount >= 0.50:
		return redStyle.Render(s)
	case amount >= 0.10:
		return orangeStyle.Render(s)
	case amount >= 0.03:
		return yellowStyle.Render(s)
	default:
		return greenStyle.Render(s)
	}
}

// refreshColumns rebuilds column titles after a language switch.
func (m *Model) refreshColumns() {
	if m.width > 0 {
		m.adjustColumnWidths(m.width)
	} else {
		m.setColumnsAndRefreshRows(m.fullListColumns(20, 12, 5, 5, 5, 5, 8, 7, 9))
	}
}

// adjustColumnWidths dynamically resizes columns based on terminal width.
func (m *Model) adjustColumnWidths(width int) {
	var sessW, srcW, turnsW, toolsW, succW, failW, costW, tokensW, healthW int

	if width >= 170 {
		m.setColumnsAndRefreshRows(m.wideListColumns(24, 14, 18, 5, 5, 5, 5, 8, 7, 8, 5, 9))
		return
	} else if width > 130 {
		sessW, srcW, turnsW, toolsW, succW, failW, costW, tokensW, healthW = 20, 12, 5, 5, 5, 5, 8, 7, 9
	} else if width >= 100 {
		sessW, srcW, turnsW, toolsW, succW, failW, costW, tokensW, healthW = 17, 10, 5, 5, 5, 4, 8, 7, 8
	} else if width >= 92 {
		sessW, srcW, turnsW, toolsW, succW, failW, costW, tokensW, healthW = 10, 7, 4, 4, 4, 4, 7, 5, 7
	} else if width >= 76 {
		m.setColumnsAndRefreshRows(m.compactListColumns(14, 8, 7, 6, 5))
		return
	} else if width >= 70 {
		m.setColumnsAndRefreshRows(m.compactListColumns(12, 7, 6, 5, 4))
		return
	} else if width >= 64 {
		m.setColumnsAndRefreshRows(m.compactListColumns(14, 8, 7, 6, 5))
		return
	} else {
		m.setColumnsAndRefreshRows(m.compactListColumns(10, 6, 6, 5, 4))
		return
	}

	m.setColumnsAndRefreshRows(m.fullListColumns(sessW, srcW, turnsW, toolsW, succW, failW, costW, tokensW, healthW))
}

func (m *Model) wideListColumns(sessW, srcW, modelW, turnsW, toolsW, succW, failW, costW, tokensW, durationW, anomW, healthW int) []table.Column {
	return []table.Column{
		{Title: m.sortColTitle(i18n.T("session"), "name"), Width: sessW},
		{Title: m.sortColTitle(i18n.T("source_tool"), "source"), Width: srcW},
		{Title: i18n.T("model_col"), Width: modelW},
		{Title: m.sortColTitle(i18n.T("turns_header"), "turns"), Width: turnsW},
		{Title: m.sortColTitle(i18n.T("tools"), ""), Width: toolsW},
		{Title: m.sortColTitle(i18n.T("succ_pct"), ""), Width: succW},
		{Title: m.sortColTitle(i18n.T("fail"), ""), Width: failW},
		{Title: m.sortColTitle(i18n.T("cost"), "cost"), Width: costW},
		{Title: m.sortColTitle(i18n.T("tokens"), ""), Width: tokensW},
		{Title: i18n.T("duration_col"), Width: durationW},
		{Title: i18n.T("anomaly_count_col"), Width: anomW},
		{Title: m.sortColTitle(i18n.T("health"), "health"), Width: healthW},
	}
}

func (m *Model) fullListColumns(sessW, srcW, turnsW, toolsW, succW, failW, costW, tokensW, healthW int) []table.Column {
	return []table.Column{
		{Title: m.sortColTitle(i18n.T("session"), "name"), Width: sessW},
		{Title: m.sortColTitle(i18n.T("source_tool"), "source"), Width: srcW},
		{Title: m.sortColTitle(i18n.T("turns_header"), "turns"), Width: turnsW},
		{Title: m.sortColTitle(i18n.T("tools"), ""), Width: toolsW},
		{Title: m.sortColTitle(i18n.T("succ_pct"), ""), Width: succW},
		{Title: m.sortColTitle(i18n.T("fail"), ""), Width: failW},
		{Title: m.sortColTitle(i18n.T("cost"), "cost"), Width: costW},
		{Title: m.sortColTitle(i18n.T("tokens"), ""), Width: tokensW},
		{Title: m.sortColTitle(i18n.T("health"), "health"), Width: healthW},
	}
}

func (m *Model) compactListColumns(sessW, srcW, costW, tokensW, healthW int) []table.Column {
	return []table.Column{
		{Title: m.sortColTitle(i18n.T("session"), "name"), Width: sessW},
		{Title: m.sortColTitle(i18n.T("source_tool"), "source"), Width: srcW},
		{Title: m.sortColTitle(i18n.T("cost"), "cost"), Width: costW},
		{Title: m.sortColTitle(i18n.T("tokens"), ""), Width: tokensW},
		{Title: m.sortColTitle(i18n.T("health"), "health"), Width: healthW},
	}
}

func (m *Model) mediumListColumns(sessW, srcW, turnsW, toolsW, failW, costW, tokensW, healthW int) []table.Column {
	return []table.Column{
		{Title: m.sortColTitle(i18n.T("session"), "name"), Width: sessW},
		{Title: m.sortColTitle(i18n.T("source_tool"), "source"), Width: srcW},
		{Title: m.sortColTitle(i18n.T("turns_header"), "turns"), Width: turnsW},
		{Title: m.sortColTitle(i18n.T("tools"), ""), Width: toolsW},
		{Title: m.sortColTitle(i18n.T("fail"), ""), Width: failW},
		{Title: m.sortColTitle(i18n.T("cost"), "cost"), Width: costW},
		{Title: m.sortColTitle(i18n.T("tokens"), ""), Width: tokensW},
		{Title: m.sortColTitle(i18n.T("health"), "health"), Width: healthW},
	}
}

func (m *Model) compactListTable() bool {
	if len(m.table.Columns()) == 5 {
		return true
	}
	return m.width > 0 && m.width < 72
}

func (m *Model) setColumnsAndRefreshRows(cols []table.Column) {
	m.table.SetRows(nil)
	m.table.SetColumns(cols)
	m.rebuildFilteredView()
}

// ═══════════════════════════════════════════════════════════════
// OVERVIEW RENDER
// ═══════════════════════════════════════════════════════════════

func (m Model) renderOverview() string {
	w := m.width
	if w <= 0 {
		w = 120
	}
	if w < 40 {
		w = 40
	}
	innerW := w - 2
	contentW := innerW
	if contentW < 20 {
		contentW = 20
	}

	if m.height > 0 && m.height <= 26 && contentW < 100 {
		return lipgloss.NewStyle().Width(innerW).Render(m.renderCompactOverview(contentW))
	}

	hero := m.renderDashboardHero(contentW)
	controls := m.renderDashboardControls(contentW)
	metrics := m.renderDashboardMetrics(contentW)

	var body string
	if contentW >= 132 {
		leftW := contentW * 42 / 100
		midW := contentW * 35 / 100
		rightW := contentW - leftW - midW - 4
		row1 := lipgloss.JoinHorizontal(lipgloss.Top,
			m.renderTokenUsagePanel(leftW), "  ",
			m.renderLatencyPanel(midW), "  ",
			m.renderHealthPanel(rightW),
		)

		bottomW := (contentW - 4) / 3
		row2 := lipgloss.JoinHorizontal(lipgloss.Top,
			m.renderAnomalyPanel(bottomW), "  ",
			m.renderTopAgentsPanel(bottomW), "  ",
			m.renderRecentSessionsPanel(contentW-2*bottomW-4),
		)
		body = lipgloss.JoinVertical(lipgloss.Left, row1, "", row2)
	} else if contentW >= 86 {
		half := (contentW - 2) / 2
		row1 := lipgloss.JoinHorizontal(lipgloss.Top, m.renderTokenUsagePanel(half), "  ", m.renderHealthPanel(contentW-half-2))
		row2 := lipgloss.JoinHorizontal(lipgloss.Top, m.renderAnomalyPanel(half), "  ", m.renderRecentSessionsPanel(contentW-half-2))
		body = lipgloss.JoinVertical(lipgloss.Left, row1, "", m.renderLatencyPanel(contentW), "", row2, "", m.renderTopAgentsPanel(contentW))
	} else {
		body = lipgloss.JoinVertical(lipgloss.Left,
			m.renderTokenUsagePanel(contentW), "",
			m.renderLatencyPanel(contentW), "",
			m.renderHealthPanel(contentW), "",
			m.renderAnomalyPanel(contentW), "",
			m.renderRecentSessionsPanel(contentW), "",
			m.renderTopAgentsPanel(contentW),
		)
	}

	page := lipgloss.JoinVertical(lipgloss.Left, hero, controls, metrics, body)
	return lipgloss.NewStyle().Width(innerW).Render(page)
}

func (m Model) renderCompactOverview(width int) string {
	sections := []string{
		m.renderDashboardHero(width),
		dimStyle.Render(truncate(i18n.T("dash_controls_short"), width)),
		m.renderCompactMetricStrip(width),
		"",
		m.renderCompactFocus(width),
		"",
		m.renderCompactRecent(width),
	}
	return lipgloss.JoinVertical(lipgloss.Left, sections...)
}

func (m Model) renderCompactMetricStrip(width int) string {
	totalTokens := m.costSummary.TotalTokensIn + m.costSummary.TotalTokensOut
	toolTotal, toolFail := aggregateToolCounts(m.sessions)
	errorRate := 0.0
	if toolTotal > 0 {
		errorRate = float64(toolFail) / float64(toolTotal) * 100
	}
	health := clampHealth(int(m.aggStats.AvgHealth))
	if len(m.sessions) == 0 {
		health = 0
	}
	items := []string{
		fmt.Sprintf("%s %s", i18n.T("metric_tokens"), brandStyle.Render(compactInt(totalTokens))),
		fmt.Sprintf("%s %s", i18n.T("metric_cost"), greenStyle.Render(money2(m.costSummary.TotalCost))),
		fmt.Sprintf("%s %s", i18n.T("metric_health"), healthColor(health).Render(fmt.Sprintf("%d%%", health))),
		fmt.Sprintf("%s %s", i18n.T("metric_errors"), anomalyRateStyle(errorRate).Render(fmt.Sprintf("%.1f%%", errorRate))),
	}
	line := strings.Join(items, "  ")
	if lipgloss.Width(line) <= width {
		return line
	}
	return lipgloss.JoinVertical(lipgloss.Left,
		truncate(strings.Join(items[:2], "  "), width),
		truncate(strings.Join(items[2:], "  "), width),
	)
}

func (m Model) renderCompactFocus(width int) string {
	title := dashTitleStyle.Render(i18n.T("compact_focus"))
	if len(m.sessions) == 0 {
		return lipgloss.JoinVertical(lipgloss.Left, title, dimStyle.Render(i18n.T("no_data")))
	}
	rows := topRiskSessions(m.sessions, minInt(3, len(m.sessions)))
	nameW := maxInt(10, width-31)
	var lines []string
	lines = append(lines, title)
	for _, s := range rows {
		health := clampHealth(s.Health)
		issue := i18n.T("list_no_major_anomaly")
		if len(s.Anomalies) > 0 {
			issue = anomalyTypeLabel(s.Anomalies[0].Type)
		}
		line := fmt.Sprintf("%s %-*s %4d%% %8s  %s",
			healthColor(health).Render("●"),
			nameW,
			truncate(s.Name, nameW),
			health,
			money4(s.Metrics.CostEstimated),
			issue,
		)
		lines = append(lines, truncate(line, width))
	}
	return lipgloss.JoinVertical(lipgloss.Left, lines...)
}

func (m Model) renderCompactRecent(width int) string {
	title := dashTitleStyle.Render(i18n.T("compact_recent"))
	if len(m.sessions) == 0 {
		return lipgloss.JoinVertical(lipgloss.Left, title, dimStyle.Render(i18n.T("panel_no_recent")))
	}
	nameW := maxInt(10, width-28)
	var lines []string
	lines = append(lines, title)
	for i := 0; i < minInt(3, len(m.sessions)); i++ {
		s := m.sessions[i]
		health := clampHealth(s.Health)
		line := fmt.Sprintf("%s %-*s %7s %4d%%",
			healthColor(health).Render("●"),
			nameW,
			truncate(s.Name, nameW),
			compactInt(s.Metrics.TokensInput+s.Metrics.TokensOutput),
			health,
		)
		lines = append(lines, truncate(line, width))
	}
	return lipgloss.JoinVertical(lipgloss.Left, lines...)
}

func topRiskSessions(sessions []engine.Session, limit int) []engine.Session {
	rows := append([]engine.Session(nil), sessions...)
	sort.SliceStable(rows, func(i, j int) bool {
		leftHealth := clampHealth(rows[i].Health)
		rightHealth := clampHealth(rows[j].Health)
		if leftHealth != rightHealth {
			return leftHealth < rightHealth
		}
		return safeAmount(rows[i].Metrics.CostEstimated) > safeAmount(rows[j].Metrics.CostEstimated)
	})
	if limit < len(rows) {
		rows = rows[:limit]
	}
	return rows
}

func healthColor(health int) lipgloss.Style {
	health = clampHealth(health)
	switch {
	case health >= 80:
		return greenStyle
	case health >= 50:
		return orangeStyle
	default:
		return redStyle
	}
}

func anomalyRateStyle(rate float64) lipgloss.Style {
	rate = chartValue(rate)
	switch {
	case rate >= 40:
		return redStyle
	case rate >= 10:
		return orangeStyle
	default:
		return greenStyle
	}
}

func (m Model) renderDashboardHero(width int) string {
	totalTokens := m.costSummary.TotalTokensIn + m.costSummary.TotalTokensOut
	toolTotal, toolFail := aggregateToolCounts(m.sessions)
	errRate := 0.0
	if toolTotal > 0 {
		errRate = float64(toolFail) / float64(toolTotal) * 100
	}
	health := clampHealth(int(m.aggStats.AvgHealth))
	if len(m.sessions) == 0 {
		health = 0
	}

	title := lipgloss.JoinHorizontal(lipgloss.Top,
		brandStyle.Render("agenttrace"),
		" ",
		dimStyle.Render(i18n.T("app_monitor_subtitle")),
	)
	summary := fmt.Sprintf("%d %s · %s · $%.2f %s · %.1f%% %s · %d%% %s",
		len(m.sessions),
		i18n.T("sessions_label"),
		tokenCountCompact(totalTokens),
		safeAmount(m.costSummary.TotalCost),
		i18n.T("cost"),
		errRate,
		i18n.T("metric_errors"),
		health,
		healthLabel(health),
	)
	if width < 70 {
		subtitle := i18n.T("app_monitor_subtitle")
		if width < 46 {
			subtitle = i18n.T("app_monitor_short")
		}
		return lipgloss.NewStyle().Width(width).Render(lipgloss.JoinVertical(lipgloss.Left,
			brandStyle.Render("agenttrace")+" "+dimStyle.Render(subtitle),
			dimStyle.Render(truncate(summary, width)),
		))
	}
	leftW := width * 38 / 100
	rightW := width - leftW - 2
	left := lipgloss.NewStyle().Width(leftW).Render(title)
	right := lipgloss.NewStyle().Width(rightW).Align(lipgloss.Right).Render(dimStyle.Render(truncate(summary, rightW)))
	return lipgloss.JoinHorizontal(lipgloss.Top, left, "  ", right)
}

func dashBadge(label, color string) string {
	return lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color("238")).
		Foreground(lipgloss.Color(color)).
		Padding(0, 1).
		Render(label)
}

func statusBadge(label, color string) string {
	return lipgloss.NewStyle().
		Background(lipgloss.Color("236")).
		Foreground(lipgloss.Color(color)).
		Padding(0, 1).
		Render(label)
}

func (m Model) renderDashboardControls(width int) string {
	if width < 70 {
		return lipgloss.NewStyle().
			Width(width).
			Padding(1, 0).
			Render(dimStyle.Render(truncate(i18n.T("dash_controls_short"), width)))
	}
	items := []string{
		dashControl(i18n.T("dash_top_cost")),
		dashControl(i18n.T("dash_critical")),
		dashControl(i18n.T("dash_anomalies")),
		dashControl(i18n.T("dash_command")),
		dashControl(i18n.T("dash_search")),
	}
	line := lipgloss.JoinHorizontal(lipgloss.Top, items...)
	return lipgloss.NewStyle().Width(width).Align(lipgloss.Right).Padding(1, 0).Render(line)
}

func dashControl(label string) string {
	return lipgloss.NewStyle().
		Background(lipgloss.Color("236")).
		Foreground(lipgloss.Color("245")).
		Padding(0, 1).
		MarginLeft(1).
		Render(label)
}

func (m Model) renderDashboardMetrics(width int) string {
	if width < 70 {
		cardW := width
		totalTokens := m.costSummary.TotalTokensIn + m.costSummary.TotalTokensOut
		toolTotal, toolFail := aggregateToolCounts(m.sessions)
		errorRate := 0.0
		if toolTotal > 0 {
			errorRate = float64(toolFail) / float64(toolTotal) * 100
		}
		p95 := aggregateP95Latency(m.sessions)
		health := clampHealth(int(m.aggStats.AvgHealth))
		if len(m.sessions) == 0 {
			health = 0
		}
		cards := []string{
			metricCard(i18n.T("metric_tokens"), compactInt(totalTokens), i18n.T("metric_live"), cardW, "82"),
			metricCard(i18n.T("metric_cost"), money2(m.costSummary.TotalCost), i18n.T("metric_estimated"), cardW, "82"),
			metricCard(i18n.T("metric_sessions"), fmt.Sprintf("%d", len(m.sessions)), i18n.T("metric_loaded"), cardW, "82"),
			metricCard(i18n.T("metric_errors"), fmt.Sprintf("%.2f%%", errorRate), fmt.Sprintf(i18n.T("metric_failed"), toolFail), cardW, "82"),
			metricCard(i18n.T("metric_p95"), fmt.Sprintf("%.2fs", p95), i18n.T("metric_tool_gaps"), cardW, "39"),
			metricCard(i18n.T("metric_health"), fmt.Sprintf("%d%%", health), healthLabel(health), cardW, "82"),
		}
		return lipgloss.JoinVertical(lipgloss.Left, cards...)
	}
	cardW := (width - 10) / 6
	if cardW < 16 {
		cardW = (width - 4) / 3
	}
	totalTokens := m.costSummary.TotalTokensIn + m.costSummary.TotalTokensOut
	toolTotal, toolFail := aggregateToolCounts(m.sessions)
	errorRate := 0.0
	if toolTotal > 0 {
		errorRate = float64(toolFail) / float64(toolTotal) * 100
	}
	p95 := aggregateP95Latency(m.sessions)
	health := clampHealth(int(m.aggStats.AvgHealth))
	if len(m.sessions) == 0 {
		health = 0
	}

	cards := []string{
		metricCard(i18n.T("metric_total_tokens"), compactInt(totalTokens), i18n.T("metric_live"), cardW, "82"),
		metricCard(i18n.T("metric_total_cost_usd"), money2(m.costSummary.TotalCost), i18n.T("metric_estimated"), cardW, "82"),
		metricCard(i18n.T("metric_sessions"), fmt.Sprintf("%d", len(m.sessions)), i18n.T("metric_loaded"), cardW, "82"),
		metricCard(i18n.T("metric_error_rate"), fmt.Sprintf("%.2f%%", errorRate), fmt.Sprintf(i18n.T("metric_failed"), toolFail), cardW, "82"),
		metricCard(i18n.T("metric_p95_latency"), fmt.Sprintf("%.2fs", p95), i18n.T("metric_tool_gaps"), cardW, "39"),
		metricCard(i18n.T("metric_health_score"), fmt.Sprintf("%d%%", health), healthLabel(health), cardW, "82"),
	}

	if width >= 110 {
		return lipgloss.JoinHorizontal(lipgloss.Top, strings.Join(cards[:1], ""), "  ", cards[1], "  ", cards[2], "  ", cards[3], "  ", cards[4], "  ", cards[5])
	}
	row1 := lipgloss.JoinHorizontal(lipgloss.Top, cards[0], "  ", cards[1], "  ", cards[2])
	row2 := lipgloss.JoinHorizontal(lipgloss.Top, cards[3], "  ", cards[4], "  ", cards[5])
	return lipgloss.JoinVertical(lipgloss.Left, row1, row2)
}

func metricCard(title, value, sub string, width int, color string) string {
	body := lipgloss.JoinVertical(lipgloss.Left,
		dashTitleStyle.Render(title),
		lipgloss.NewStyle().Foreground(lipgloss.Color(color)).Bold(true).Render(value),
		dimStyle.Render(sub),
	)
	return styleForOuterWidth(dashPanelStyle, width).Height(5).Render(body)
}

func (m Model) renderTokenUsagePanel(width int) string {
	values := recentTokenSeries(m.sessions, 36)
	innerW := dashboardInnerWidth(width)
	chart := miniBarChart(values, innerW, 8, []string{"34", "82"})
	total := compactInt(m.costSummary.TotalTokensIn + m.costSummary.TotalTokensOut)
	legend := greenStyle.Render(i18n.T("legend_input")) + "  " + brandStyle.Render(i18n.T("legend_output")) + "  " + dimStyle.Render(i18n.T("legend_cache"))
	content := chart + "\n" + legend
	return dashboardPanel(i18n.T("panel_token_usage"), brandStyle.Render(fmt.Sprintf(i18n.T("metric_total"), total)), content, width)
}

func (m Model) renderLatencyPanel(width int) string {
	values := recentLatencySeries(m.sessions, 48)
	p95 := aggregateP95Latency(m.sessions)
	innerW := dashboardInnerWidth(width)
	spark := cyanStyle.Render(sparkline(values, innerW))
	axis := dimStyle.Render("P50") + "   " + cyanStyle.Render("P95") + "   " + dimStyle.Render("P99")
	return dashboardPanel(i18n.T("panel_latency_ms"), cyanStyle.Render(fmt.Sprintf("P95 %.0fms", p95*1000)), spark+"\n\n"+axis, width)
}

func (m Model) renderHealthPanel(width int) string {
	health := clampHealth(int(m.aggStats.AvgHealth))
	if len(m.sessions) == 0 {
		health = 0
	}
	content := lipgloss.NewStyle().Align(lipgloss.Center).Render(brandStyle.Render(fmt.Sprintf("%d%%", health)) + "\n" + dimStyle.Render(healthLabel(health)))
	innerW := dashboardInnerWidth(width)
	content += "\n\n" + healthMetricLine(i18n.T("health_reliability"), health+2, innerW)
	content += "\n" + healthMetricLine(i18n.T("health_performance"), health-2, innerW)
	content += "\n" + healthMetricLine(i18n.T("health_quality"), health, innerW)
	content += "\n" + healthMetricLine(i18n.T("health_efficiency"), health+1, innerW)
	if trend := m.renderHealthTrendLine(innerW); trend != "" {
		content += "\n\n" + dashTitleStyle.Render(i18n.T("trend_title"))
		content += "\n" + trend
	}
	return dashboardPanel(i18n.T("panel_health_score"), "", content, width)
}

func (m Model) renderHealthTrendLine(width int) string {
	if len(m.sessions) < 2 {
		return ""
	}
	trend := engine.AnalyzeHealthTrend(m.sessions)
	if len(trend.Points) == 0 {
		return dimStyle.Render(truncate(trend.Message, width))
	}
	values := make([]float64, 0, len(trend.Points))
	for _, p := range trend.Points {
		values = append(values, float64(clampHealth(p.Health)))
	}
	sparkW := minInt(24, maxInt(8, width/3))
	spark := sparkline(values, sparkW)
	label := i18n.T("trend_stable_label")
	style := dimStyle
	if trend.Regressing || trend.Direction == "down" {
		label = i18n.T("trend_down")
		style = redStyle
	} else if trend.Direction == "up" {
		label = i18n.T("trend_up")
		style = greenStyle
	}
	line := fmt.Sprintf("%s %s %s", style.Render(spark), dimStyle.Render(label), trend.Message)
	return truncate(line, width)
}

func healthMetricLine(label string, score int, width int) string {
	if score > 100 {
		score = 100
	}
	if score < 0 {
		score = 0
	}
	barW := width - len(label) - 12
	if barW < 8 {
		barW = 8
	}
	filled := barW * score / 100
	bar := brandStyle.Render(strings.Repeat("━", filled)) + dimStyle.Render(strings.Repeat("━", barW-filled))
	return fmt.Sprintf("%-12s %s %3d%%", label, bar, score)
}

func (m Model) renderAnomalyPanel(width int) string {
	var lines []string
	innerW := dashboardInnerWidth(width)
	if len(m.overview.AnomaliesTop) == 0 {
		lines = append(lines, greenStyle.Render(i18n.T("panel_no_anomalies")))
	} else {
		limit := minInt(4, len(m.overview.AnomaliesTop))
		nameW := maxInt(8, innerW-18)
		for i := 0; i < limit; i++ {
			a := m.overview.AnomaliesTop[i]
			name := truncate(a.Session, nameW)
			kind := truncate(anomalyTypeLabel(a.Type), maxInt(6, innerW-nameW-5))
			lines = append(lines, fmt.Sprintf("%s  %-*s %s",
				anomalyColor(a.Type).Render("△"),
				nameW,
				name,
				dimStyle.Render(kind),
			))
		}
	}
	return dashboardPanel(i18n.T("panel_anomaly_detection"), cyanStyle.Render(i18n.T("panel_view_all")), strings.Join(lines, "\n"), width)
}

func (m Model) renderTopAgentsPanel(width int) string {
	type item struct {
		name   string
		tokens int
	}
	var items []item
	bySource := map[string]int{}
	for _, s := range m.sessions {
		bySource[s.Metrics.SourceTool] += nonNegativeInt(s.Metrics.TokensInput + s.Metrics.TokensOutput)
	}
	for k, v := range bySource {
		name := k
		if d, ok := engine.ToolDisplayNames[k]; ok {
			name = d
		}
		items = append(items, item{name: name, tokens: v})
	}
	sort.Slice(items, func(i, j int) bool { return items[i].tokens > items[j].tokens })
	var lines []string
	maxTokens := 1
	if len(items) > 0 && items[0].tokens > 0 {
		maxTokens = items[0].tokens
	}
	innerW := dashboardInnerWidth(width)
	for i := 0; i < minInt(5, len(items)); i++ {
		nameW := 14
		if innerW < 42 {
			nameW = 10
		}
		barW := maxInt(4, innerW-nameW-10)
		filled := barW * nonNegativeInt(items[i].tokens) / maxTokens
		if filled > barW {
			filled = barW
		}
		bar := brandStyle.Render(strings.Repeat("█", filled)) + dimStyle.Render(strings.Repeat("░", barW-filled))
		lines = append(lines, fmt.Sprintf("%-*s %s %6s", nameW, truncate(items[i].name, nameW), bar, compactInt(items[i].tokens)))
	}
	if len(lines) == 0 {
		lines = append(lines, dimStyle.Render(i18n.T("panel_no_agent_data")))
	}
	return dashboardPanel(i18n.T("panel_top_agents"), cyanStyle.Render(i18n.T("panel_view_all")), strings.Join(lines, "\n"), width)
}

func (m Model) renderRecentSessionsPanel(width int) string {
	var lines []string
	limit := minInt(5, len(m.sessions))
	innerW := dashboardInnerWidth(width)
	nameW := maxInt(8, innerW-17)
	for i := 0; i < limit; i++ {
		s := m.sessions[i]
		health := clampHealth(s.Health)
		status := greenStyle.Render("●")
		if health < 50 {
			status = redStyle.Render("●")
		} else if health < 80 {
			status = orangeStyle.Render("●")
		}
		name := truncate(s.Name, nameW)
		tokens := compactInt(s.Metrics.TokensInput + s.Metrics.TokensOutput)
		lines = append(lines, fmt.Sprintf("%s %-*s %6s %3d%%",
			status,
			nameW,
			name,
			tokens,
			health,
		))
	}
	if len(lines) == 0 {
		lines = append(lines, dimStyle.Render(i18n.T("panel_no_recent")))
	}
	return dashboardPanel(i18n.T("panel_recent_sessions"), cyanStyle.Render(i18n.T("panel_view_all")), strings.Join(lines, "\n"), width)
}

func dashboardPanel(title, aside, content string, width int) string {
	innerW := dashboardInnerWidth(width)
	header := dashTitleStyle.Render(title)
	if aside != "" && width >= 110 && innerW >= 28 {
		asideW := minInt(14, maxInt(8, innerW/3))
		header = lipgloss.JoinHorizontal(lipgloss.Top,
			lipgloss.NewStyle().Width(maxInt(1, innerW-asideW)).Render(dashTitleStyle.Render(truncate(title, maxInt(1, innerW-asideW)))),
			lipgloss.NewStyle().Width(asideW).Align(lipgloss.Right).Render(truncate(aside, asideW)),
		)
	}
	return styleForOuterWidth(dashPanelStyle, maxInt(20, width)).
		Render(lipgloss.JoinVertical(lipgloss.Left, header, "", content))
}

func aggregateToolCounts(sessions []engine.Session) (total, failed int) {
	for _, s := range sessions {
		total += nonNegativeInt(s.Metrics.ToolCallsOK) + nonNegativeInt(s.Metrics.ToolCallsFail)
		failed += nonNegativeInt(s.Metrics.ToolCallsFail)
	}
	return total, failed
}

func aggregateP95Latency(sessions []engine.Session) float64 {
	var vals []float64
	for _, s := range sessions {
		for _, gap := range s.Metrics.GapsSec {
			vals = append(vals, chartValue(gap))
		}
	}
	if len(vals) == 0 {
		return 0
	}
	sort.Float64s(vals)
	idx := int(float64(len(vals)-1) * 0.95)
	return vals[idx]
}

func recentTokenSeries(sessions []engine.Session, n int) []float64 {
	vals := make([]float64, n)
	if len(sessions) == 0 {
		return vals
	}
	limit := minInt(n, len(sessions))
	for i := 0; i < limit; i++ {
		s := sessions[limit-1-i]
		vals[n-limit+i] = chartValue(float64(s.Metrics.TokensInput + s.Metrics.TokensOutput + s.Metrics.TokensCacheR))
	}
	return vals
}

func recentLatencySeries(sessions []engine.Session, n int) []float64 {
	var raw []float64
	for _, s := range sessions {
		for _, gap := range s.Metrics.GapsSec {
			raw = append(raw, chartValue(gap))
		}
	}
	if len(raw) == 0 {
		return make([]float64, n)
	}
	if len(raw) > n {
		raw = raw[len(raw)-n:]
	}
	vals := make([]float64, n)
	copy(vals[n-len(raw):], raw)
	return vals
}

func miniBarChart(vals []float64, width, height int, colors []string) string {
	if width < 12 {
		width = 12
	}
	if height < 4 {
		height = 4
	}
	if len(vals) == 0 {
		vals = []float64{0}
	}
	if len(vals) > width {
		vals = vals[len(vals)-width:]
	}
	maxVal := 0.0
	for _, v := range vals {
		v = chartValue(v)
		if v > maxVal {
			maxVal = v
		}
	}
	if maxVal == 0 {
		maxVal = 1
	}
	var rows []string
	for level := height; level >= 1; level-- {
		var row strings.Builder
		for i, v := range vals {
			v = chartValue(v)
			fill := int(v / maxVal * float64(height))
			if fill < 0 {
				fill = 0
			}
			if fill > height {
				fill = height
			}
			ch := " "
			if fill >= level {
				color := colors[i%len(colors)]
				ch = lipgloss.NewStyle().Foreground(lipgloss.Color(color)).Render("█")
			}
			row.WriteString(ch)
		}
		rows = append(rows, row.String())
	}
	return strings.Join(rows, "\n")
}

func sparkline(vals []float64, width int) string {
	if width < 8 {
		width = 8
	}
	if len(vals) == 0 {
		return strings.Repeat("─", width)
	}
	if len(vals) > width {
		vals = vals[len(vals)-width:]
	}
	maxVal := 0.0
	for _, v := range vals {
		v = chartValue(v)
		if v > maxVal {
			maxVal = v
		}
	}
	if maxVal == 0 {
		return strings.Repeat("▁", len(vals))
	}
	levels := []rune("▁▂▃▄▅▆▇█")
	var b strings.Builder
	for _, v := range vals {
		v = chartValue(v)
		idx := int(v / maxVal * float64(len(levels)-1))
		if idx < 0 {
			idx = 0
		}
		if idx >= len(levels) {
			idx = len(levels) - 1
		}
		b.WriteRune(levels[idx])
	}
	return b.String()
}

func chartValue(v float64) float64 {
	if !isFiniteNumber(v) || v < 0 {
		return 0
	}
	return v
}

func safeAmount(v float64) float64 {
	return chartValue(v)
}

func money2(v float64) string {
	return fmt.Sprintf("$%.2f", safeAmount(v))
}

func money4(v float64) string {
	return fmt.Sprintf("$%.4f", safeAmount(v))
}

func nonNegativeInt(v int) int {
	if v < 0 {
		return 0
	}
	return v
}

func normalizedToolCounts(m engine.Metrics) (ok, fail, observed, total int) {
	ok = nonNegativeInt(m.ToolCallsOK)
	fail = nonNegativeInt(m.ToolCallsFail)
	observed = ok + fail
	total = nonNegativeInt(m.ToolCallsTotal)
	if total < observed {
		total = observed
	}
	return ok, fail, observed, total
}

func safeReportMetrics(m engine.Metrics) engine.Metrics {
	m.EventsTotal = nonNegativeInt(m.EventsTotal)
	m.UserMessages = nonNegativeInt(m.UserMessages)
	m.AssistantTurns = nonNegativeInt(m.AssistantTurns)
	m.ToolResults = nonNegativeInt(m.ToolResults)
	ok, fail, _, total := normalizedToolCounts(m)
	m.ToolCallsOK = ok
	m.ToolCallsFail = fail
	m.ToolCallsTotal = total
	m.ReasoningBlocks = nonNegativeInt(m.ReasoningBlocks)
	m.ReasoningChars = nonNegativeInt(m.ReasoningChars)
	m.ReasoningRedact = nonNegativeInt(m.ReasoningRedact)
	m.TokensInput = nonNegativeInt(m.TokensInput)
	m.TokensOutput = nonNegativeInt(m.TokensOutput)
	m.TokensCacheW = nonNegativeInt(m.TokensCacheW)
	m.TokensCacheR = nonNegativeInt(m.TokensCacheR)
	m.DurationSec = chartValue(m.DurationSec)
	m.CostEstimated = safeAmount(m.CostEstimated)
	m.LoopRetryEvents = nonNegativeInt(m.LoopRetryEvents)
	m.LoopGroups = nonNegativeInt(m.LoopGroups)
	m.LoopCostEst = safeAmount(m.LoopCostEst)

	m.GapsSec = make([]float64, 0, len(m.GapsSec))
	for _, gap := range m.GapsSec {
		m.GapsSec = append(m.GapsSec, chartValue(gap))
	}
	m.ReasoningLens = make([]int, 0, len(m.ReasoningLens))
	for _, n := range m.ReasoningLens {
		m.ReasoningLens = append(m.ReasoningLens, nonNegativeInt(n))
	}
	if len(m.ToolUsage) > 0 {
		toolUsage := make(map[string]int, len(m.ToolUsage))
		for name, count := range m.ToolUsage {
			toolUsage[name] = nonNegativeInt(count)
		}
		m.ToolUsage = toolUsage
	}
	return m
}

func safeDiffSession(s engine.Session) engine.Session {
	s.Health = clampHealth(s.Health)
	s.Metrics = safeReportMetrics(s.Metrics)
	return s
}

func compactInt(v int) string {
	v = nonNegativeInt(v)
	switch {
	case v >= 1000000:
		return fmt.Sprintf("%.1fM", float64(v)/1000000)
	case v >= 1000:
		return fmt.Sprintf("%.1fK", float64(v)/1000)
	default:
		return fmt.Sprintf("%d", v)
	}
}

func tokenCount(v int) string {
	return fmt.Sprintf(i18n.T("token_count"), fmt.Sprintf("%d", v))
}

func tokenCountCompact(v int) string {
	return fmt.Sprintf(i18n.T("token_count"), compactInt(v))
}

func healthLabel(h int) string {
	h = clampHealth(h)
	switch {
	case h >= 90:
		return i18n.T("health_excellent")
	case h >= 80:
		return i18n.T("health_good")
	case h >= 50:
		return i18n.T("health_attention")
	default:
		return i18n.T("health_critical")
	}
}

func clampHealth(h int) int {
	if h < 0 {
		return 0
	}
	if h > 100 {
		return 100
	}
	return h
}

func sortFieldLabel(field string) string {
	switch field {
	case "name":
		return i18n.T("sort_field_name")
	case "health":
		return i18n.T("sort_field_health")
	case "cost":
		return i18n.T("sort_field_cost")
	case "turns":
		return i18n.T("sort_field_turns")
	case "source":
		return i18n.T("sort_field_source")
	default:
		return field
	}
}

func diffFieldLabel(field string) string {
	switch field {
	case "health":
		return i18n.T("diff_field_health")
	case "cost":
		return i18n.T("diff_field_cost")
	case "turns":
		return i18n.T("diff_field_turns")
	case "tools":
		return i18n.T("diff_field_tools")
	case "success_rate":
		return i18n.T("diff_field_success_rate")
	case "fail_count":
		return i18n.T("diff_field_fail_count")
	case "duration":
		return i18n.T("diff_field_duration")
	case "model":
		return i18n.T("diff_field_model")
	case "anomaly_count":
		return i18n.T("diff_field_anomaly_count")
	default:
		return field
	}
}

func anomalyTypeLabel(kind string) string {
	key := "anomaly_type_" + kind
	if translated := i18n.T(key); translated != key {
		return translated
	}
	return strings.ReplaceAll(kind, "_", " ")
}

func riskLabel(risk string) string {
	key := "risk_" + risk
	if translated := i18n.T(key); translated != key {
		return translated
	}
	return risk
}

func healthFilterLabel(filter string) string {
	switch filter {
	case "good":
		return i18n.T("list_health_good")
	case "warn":
		return i18n.T("list_health_warn")
	case "crit":
		return i18n.T("list_health_crit")
	default:
		return filter
	}
}

func anomalyColor(t string) lipgloss.Style {
	switch t {
	case "tool_failures", "hanging":
		return redStyle
	case "latency", "shallow_thinking":
		return yellowStyle
	default:
		return cyanStyle
	}
}

func truncate(s string, maxLen int) string {
	if maxLen <= 0 {
		return ""
	}
	if lipgloss.Width(s) <= maxLen {
		return s
	}
	if maxLen <= 1 {
		return ""
	}
	return ansi.Truncate(s, maxLen, "…")
}

func dropLastRune(s string) string {
	if s == "" {
		return ""
	}
	_, size := utf8.DecodeLastRuneInString(s)
	if size <= 0 {
		return ""
	}
	return s[:len(s)-size]
}

func sourceDisplayName(source string) string {
	if display, ok := engine.ToolDisplayNames[source]; ok {
		return display
	}
	return source
}

func truncateLines(s string, maxLen int) string {
	lines := strings.Split(s, "\n")
	for i, line := range lines {
		lines[i] = truncate(line, maxLen)
	}
	return strings.Join(lines, "\n")
}

func renderedLineCount(s string) int {
	if s == "" {
		return 0
	}
	return len(strings.Split(s, "\n"))
}

func (m Model) fitTerminalFrame(s string) string {
	if m.width <= 0 {
		return s
	}
	width := maxInt(1, m.width)
	lines := strings.Split(s, "\n")
	for i, line := range lines {
		if w := lipgloss.Width(line); w > width {
			lines[i] = ansi.Truncate(line, width, "")
		} else if w < width {
			lines[i] = line + strings.Repeat(" ", width-w)
		}
	}
	if m.height > 0 {
		if len(lines) > m.height {
			if m.height == 1 {
				lines = lines[len(lines)-1:]
			} else {
				// 内容过高时保留底部状态栏，避免用户看不到关键操作提示。
				lines = append(lines[:m.height-1], lines[len(lines)-1])
			}
		}
		blank := strings.Repeat(" ", width)
		for len(lines) < m.height {
			lines = append(lines, blank)
		}
	}
	return strings.Join(lines, "\n")
}

func (m Model) fitTerminalFrameWithFooter(body, footer string) string {
	if m.height <= 0 {
		return m.fitTerminalFrame(strings.Join([]string{body, footer}, "\n"))
	}
	bodyLines := strings.Split(body, "\n")
	footerLines := strings.Split(footer, "\n")
	bodyBudget := maxInt(0, m.height-len(footerLines))
	if len(bodyLines) > bodyBudget {
		bodyLines = bodyLines[:bodyBudget]
	}
	return m.fitTerminalFrame(strings.Join(append(bodyLines, footerLines...), "\n"))
}

func (m Model) contentWidth() int {
	if m.width <= 0 {
		return 100
	}
	w := m.width - 14
	if w < 40 {
		return 40
	}
	return w
}

func (m Model) frameBodyWidth() int {
	frameW, _ := baseStyle.GetFrameSize()
	return maxInt(1, m.contentWidth()-frameW)
}

func (m Model) frameContent(content string) string {
	return styleForOuterWidth(baseStyle, m.contentWidth()).Render(content)
}

func subtlePanel(title, content string, width int) string {
	var body string
	if title == "" {
		body = content
	} else {
		body = lipgloss.JoinVertical(lipgloss.Left, dashTitleStyle.Render(title), "", content)
	}
	style := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color("238")).
		Padding(1, 2)
	return styleForOuterWidth(style, maxInt(20, width)).Render(body)
}

func styleForOuterWidth(style lipgloss.Style, outerWidth int) lipgloss.Style {
	frameW := style.GetHorizontalMargins() + style.GetHorizontalBorderSize()
	return style.Width(maxInt(1, outerWidth-frameW))
}

func dashboardInnerWidth(outerWidth int) int {
	return maxInt(1, outerWidth-12)
}

func minInt(a, b int) int {
	if a < b {
		return a
	}
	return b
}

func maxInt(a, b int) int {
	if a > b {
		return a
	}
	return b
}

// ═══════════════════════════════════════════════════════════════
// FILTER HELPERS
// ═══════════════════════════════════════════════════════════════

func (m *Model) getAvailableSources() []string {
	seen := map[string]bool{}
	var sources []string
	for _, s := range m.sessions {
		src := s.Metrics.SourceTool
		if !seen[src] {
			seen[src] = true
			sources = append(sources, src)
		}
	}
	return sources
}

func (m *Model) cycleSourceFilter() {
	sources := m.getAvailableSources()
	if len(sources) == 0 {
		m.filterSource = ""
		return
	}
	if m.filterSource == "" {
		m.filterSource = sources[0]
		return
	}
	for i, src := range sources {
		if src != m.filterSource {
			continue
		}
		if i+1 < len(sources) {
			m.filterSource = sources[i+1]
		} else {
			m.filterSource = ""
		}
		return
	}
	m.filterSource = sources[0]
}

func (m *Model) rebuildFilteredIndices() {
	m.filteredIndices = nil
	for i, s := range m.sessions {
		if m.matchesFilters(s) {
			m.filteredIndices = append(m.filteredIndices, i)
		}
	}
	if len(m.filteredIndices) == 0 {
		m.filteredIndices = []int{}
	}
}

func (m *Model) matchesFilters(s engine.Session) bool {
	healthFilter := m.filterHealth
	if healthFilter == "" && m.filterMode == "health" {
		healthFilter = m.filterValue
	}
	sourceFilter := m.filterSource
	if sourceFilter == "" && m.filterMode == "source" {
		sourceFilter = m.filterValue
	}

	if healthFilter != "" {
		health := clampHealth(s.Health)
		switch healthFilter {
		case "good":
			if health < 80 {
				return false
			}
		case "warn":
			if health < 50 || health >= 80 {
				return false
			}
		case "crit":
			if health >= 50 {
				return false
			}
		default:
			if !matchHealthExpression(health, healthFilter) {
				return false
			}
		}
	}
	if sourceFilter != "" {
		sourceDisplay := s.Metrics.SourceTool
		if display, ok := engine.ToolDisplayNames[s.Metrics.SourceTool]; ok {
			sourceDisplay = display
		}
		q := strings.ToLower(sourceFilter)
		if !strings.Contains(strings.ToLower(s.Metrics.SourceTool), q) &&
			!strings.Contains(strings.ToLower(sourceDisplay), q) {
			return false
		}
	}
	if m.filterModel != "" && !strings.Contains(strings.ToLower(s.Metrics.ModelUsed), strings.ToLower(m.filterModel)) {
		return false
	}
	if m.filterCostOp != "" && !matchFloatExpression(safeAmount(s.Metrics.CostEstimated), m.filterCostOp, m.filterCostValue) {
		return false
	}
	if m.filterAnomaly && len(s.Anomalies) == 0 {
		return false
	}
	if m.filterText != "" {
		q := strings.ToLower(m.filterText)
		sourceDisplay := s.Metrics.SourceTool
		if display, ok := engine.ToolDisplayNames[s.Metrics.SourceTool]; ok {
			sourceDisplay = display
		}
		if !strings.Contains(strings.ToLower(s.Name), q) &&
			!strings.Contains(strings.ToLower(s.Metrics.SourceTool), q) &&
			!strings.Contains(strings.ToLower(sourceDisplay), q) &&
			!strings.Contains(strings.ToLower(s.Metrics.ModelUsed), q) {
			return false
		}
	}
	return true
}

func (m *Model) filterLabel() string {
	var parts []string
	if m.filterHealth != "" {
		parts = append(parts, fmt.Sprintf("%s=%s", i18n.T("list_filter_health"), healthFilterLabel(m.filterHealth)))
	} else if m.filterMode == "health" {
		parts = append(parts, fmt.Sprintf("%s=%s", i18n.T("list_filter_health"), healthFilterLabel(m.filterValue)))
	}
	if m.filterSource != "" {
		parts = append(parts, fmt.Sprintf("%s=%s", i18n.T("list_filter_source"), m.filterSource))
	} else if m.filterMode == "source" {
		parts = append(parts, fmt.Sprintf("%s=%s", i18n.T("list_filter_source"), m.filterValue))
	}
	if m.filterModel != "" {
		parts = append(parts, fmt.Sprintf("%s=%q", i18n.T("list_filter_model"), m.filterModel))
	}
	if m.filterCostOp != "" {
		parts = append(parts, fmt.Sprintf("%s%s%.4g", i18n.T("list_filter_cost"), m.filterCostOp, m.filterCostValue))
	}
	if m.filterAnomaly {
		parts = append(parts, i18n.T("list_filter_anomalies"))
	}
	if m.filterText != "" {
		parts = append(parts, fmt.Sprintf("%s=%q", i18n.T("list_filter_text"), m.filterText))
	}
	if len(parts) == 0 {
		return i18n.T("list_filter_none")
	}
	return strings.Join(parts, ", ")
}

func (m *Model) hasAnyFilter() bool {
	return m.filterText != "" ||
		m.filterMode != "" ||
		m.filterHealth != "" ||
		m.filterSource != "" ||
		m.filterModel != "" ||
		m.filterCostOp != "" ||
		m.filterAnomaly
}

func (m *Model) rebuildFilteredView() {
	selected := m.selectedSessionKey()
	m.rebuildFilteredIndices()
	var rows []table.Row
	for _, idx := range m.filteredIndices {
		rows = append(rows, m.sessionRow(m.sessions[idx]))
	}
	m.table.SetRows(rows)
	m.table.SetCursor(0)
	m.restoreSelection(selected)
}

func (m *Model) selectedSessionKey() string {
	idx := m.findSessionIndex()
	if idx < 0 || idx >= len(m.sessions) {
		return ""
	}
	return sessionKey(m.sessions[idx])
}

func (m *Model) restoreSelection(key string) {
	if key == "" {
		return
	}
	for row, idx := range m.filteredIndices {
		if idx >= 0 && idx < len(m.sessions) && sessionKey(m.sessions[idx]) == key {
			m.table.SetCursor(row)
			return
		}
	}
}

func sessionKey(s engine.Session) string {
	if s.Path != "" {
		return "path:" + s.Path
	}
	return "name:" + s.Name
}

func (m *Model) applyFilter() {
	m.rebuildFilteredView()
}

func (m *Model) applyTextFilter() {
	m.rebuildFilteredView()
}
