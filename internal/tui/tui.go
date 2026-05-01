// Package tui provides the Bubble Tea interactive terminal UI for agentwaste.
// btop-style modern dashboard with four views: Overview, Session List, Detail, Diagnostics, Diff.
package tui

import (
	"fmt"
	"sort"
	"strings"

	"github.com/charmbracelet/bubbles/table"
	"github.com/charmbracelet/bubbles/viewport"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/luoyuctl/agentwaste/internal/engine"
	"github.com/luoyuctl/agentwaste/internal/i18n"
)

// ── Styles ──

var (
	baseStyle = lipgloss.NewStyle().
			BorderStyle(lipgloss.NormalBorder()).
			BorderForeground(lipgloss.Color("240"))

	titleStyle = lipgloss.NewStyle().
			Background(lipgloss.Color("63")).
			Foreground(lipgloss.Color("230")).
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
		Foreground(lipgloss.Color("241")).
		Background(lipgloss.Color("235")).
		Padding(0, 1)

	// Health score background colors
	healthGoodStyle = lipgloss.NewStyle().Background(lipgloss.Color("34")).Foreground(lipgloss.Color("15"))
	healthWarnStyle = lipgloss.NewStyle().Background(lipgloss.Color("178")).Foreground(lipgloss.Color("16"))
	healthBadStyle  = lipgloss.NewStyle().Background(lipgloss.Color("160")).Foreground(lipgloss.Color("15"))
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

type Model struct {
	view     view
	sessions []engine.Session
	dir      string
	lang     i18n.Lang

	// Overview data
	overview engine.Overview

	// Filter state
	filterText       string
	filterMode       string // "", "health", "source"
	filterValue      string // e.g. "good", "hermes_jsonl"
	filteredIndices  []int  // maps table row → sessions index

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
	healthTrend    engine.HealthTrend
	costSummary    engine.CostSummary
	stuckPatterns  []engine.StuckPattern

	// Dimensions
	width  int
	height int
}

func New(dir string) Model {
	sessions := engine.LoadAll(dir)
	overview := engine.ComputeOverview(sessions)

	// Build unified list table (8 columns with FAIL + TOKENS merged in)
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

	var rows []table.Row

	for _, s := range sessions {
		m := s.Metrics
		totalTools := m.ToolCallsOK + m.ToolCallsFail
		sr := "N/A"
		if totalTools > 0 {
			sr = fmt.Sprintf("%.0f", float64(m.ToolCallsOK)/float64(totalTools)*100)
		}

		sourceDisplay := m.SourceTool
		if display, ok := engine.ToolDisplayNames[m.SourceTool]; ok {
			sourceDisplay = display
		}

		healthRaw := fmt.Sprintf("%d/100 %s", s.Health, engine.HealthEmoji(s.Health))
		var healthCol string
		switch {
		case s.Health >= 80:
			healthCol = healthGoodStyle.Render(healthRaw)
		case s.Health >= 50:
			healthCol = healthWarnStyle.Render(healthRaw)
		default:
			healthCol = healthBadStyle.Render(healthRaw)
		}

		failStr := fmt.Sprintf("%d", m.ToolCallsFail)
		if m.ToolCallsFail > 0 {
			failStr = redStyle.Render(failStr)
		} else {
			failStr = dimStyle.Render(failStr)
		}
		tokensStr := fmt.Sprintf("%d", m.TokensInput+m.TokensOutput)

		rows = append(rows, table.Row{
			s.Name,
			sourceDisplay,
			fmt.Sprintf("%d", m.AssistantTurns),
			fmt.Sprintf("%d", m.ToolCallsTotal),
			sr,
			failStr,
			costColor(m.CostEstimated),
			tokensStr,
			healthCol,
		})
	}

	t := table.New(
		table.WithColumns(columns),
		table.WithRows(rows),
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
		Foreground(lipgloss.Color("229")).
		Background(lipgloss.Color("63")).
		Bold(false)
	t.SetStyles(s)

	m := Model{
		view:        viewOverview,
		sessions:    sessions,
		dir:         dir,
		lang:        i18n.Current,
		overview:    overview,
		aggStats:    engine.ComputeAggregateStats(sessions),
		table:       t,
		tableReady:  true,
		costSummary: engine.ComputeCostSummary(sessions),
	}
	m.rebuildFilteredIndices()
	return m
}

func (m Model) Init() tea.Cmd {
	return nil
}

// ── Update ──

func (m Model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	var cmd tea.Cmd

	switch msg := msg.(type) {

	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height

		m.table.SetWidth(msg.Width - 4)
		m.table.SetHeight(msg.Height - 8)
		m.adjustColumnWidths(msg.Width)

		if m.detailReady {
			m.viewport.Width = msg.Width - 4
			m.viewport.Height = msg.Height - 6
		}

	case tea.KeyMsg:
		switch msg.String() {
		case "q", "ctrl+c":
			return m, tea.Quit

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
				m.view = viewDiff
			case viewDiff:
				m.view = viewOverview
			}

		case "r":
			m.sessions = engine.LoadAll(m.dir)
			m.overview = engine.ComputeOverview(m.sessions)
			m.aggStats = engine.ComputeAggregateStats(m.sessions)
			m.costSummary = engine.ComputeCostSummary(m.sessions)
			m.refreshTable()
			m.rebuildFilteredIndices()

		case "L", "l":
			if m.lang == i18n.EN {
				m.lang = i18n.ZH
			} else {
				m.lang = i18n.EN
			}
			i18n.SetLang(m.lang)
			m.refreshColumns()

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
			m.view = viewDiff
		case "w":
			if m.view == viewDetail {
				m.view = viewDiagnostics
			}

		case "/":
			if m.view == viewList {
				m.filterActive = true
				m.filterInput = ""
			}

		case "enter":
			if m.filterActive {
				m.filterText = m.filterInput
				m.filterActive = false
				m.applyTextFilter()
				return m, cmd
			}
			if m.view == viewList && m.tableReady && len(m.filteredIndices) > 0 {
				m.view = viewDetail
				m.openDetail()
				return m, cmd
			}
			return m, cmd
		case "esc":
			if m.filterActive {
				m.filterActive = false
				m.filterInput = ""
			} else if m.view == viewDetail || m.view == viewDiff || m.view == viewDiagnostics {
				m.view = viewList
			}
		case "backspace":
			if m.filterActive && len(m.filterInput) > 0 {
				m.filterInput = m.filterInput[:len(m.filterInput)-1]
			}

		case "f":
			if !m.filterActive && m.view == viewList {
				switch m.filterMode {
				case "health":
					if m.filterValue == "good" {
						m.filterValue = "warn"
					} else if m.filterValue == "warn" {
						m.filterValue = "crit"
					} else {
						m.filterMode = ""
						m.filterValue = ""
					}
				default:
					m.filterMode = "health"
					m.filterValue = "good"
				}
				m.applyFilter()
			}

		case "s":
			if !m.filterActive && m.view == viewList {
				if m.filterMode != "source" {
					m.filterMode = "source"
					m.filterValue = "hermes_jsonl"
				} else {
					sources := m.getAvailableSources()
					for i, src := range sources {
						if src == m.filterValue && i+1 < len(sources) {
							m.filterValue = sources[i+1]
							break
						} else if src == m.filterValue {
							m.filterMode = ""
							m.filterValue = ""
							break
						}
					}
				}
				m.applyFilter()
			}

		case "d":
			if m.view == viewList {
				cursor := m.table.Cursor()
				if cursor >= 0 && len(m.filteredIndices) > cursor+1 {
					idxA := m.filteredIndices[cursor]
					idxB := m.filteredIndices[cursor+1]
					m.diffResult = engine.DiffSessions(m.sessions[idxA], m.sessions[idxB])
					m.view = viewDiff
				}
			} else if m.view == viewDetail {
				idx := m.findSessionIndex()
				if idx > 0 {
					m.diffResult = engine.DiffSessions(m.sessions[idx-1], m.sessions[idx])
					m.view = viewDiff
				}
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
			if m.filterActive {
				if msg.String() == "q" || msg.String() == "ctrl+c" {
					return m, tea.Quit
				}
				if len(msg.Runes) > 0 {
					m.filterInput += string(msg.Runes)
				}
				return m, nil
			}
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
	if !m.tableReady || len(m.sessions) == 0 {
		return
	}
	idx := m.findSessionIndex()
	if idx >= 0 && idx < len(m.sessions) {
		s := m.sessions[idx]
		text := engine.ReportText(s.Metrics, s.Anomalies, s.Health)

		vw := m.width - 4
		vh := m.height - 6
		if vw < 40 {
			vw = 80
		}
		if vh < 10 {
			vh = 20
		}

		m.viewport = viewport.New(vw, vh)
		m.viewport.SetContent(text)
		m.detailReady = true

		m.fixSuggestions = engine.GenerateFixes(s.Metrics, s.Anomalies, string(m.lang))
		m.costAlert = engine.PredictCostAnomaly(m.sessions, s)

		m.loopResult = s.LoopResultData
		m.toolWarnings = s.ToolWarnings
	}
}

func (m *Model) refreshTable() {
	var rows []table.Row
	for _, s := range m.sessions {
		m := s.Metrics
		totalTools := m.ToolCallsOK + m.ToolCallsFail
		sr := "N/A"
		if totalTools > 0 {
			sr = fmt.Sprintf("%.0f", float64(m.ToolCallsOK)/float64(totalTools)*100)
		}

		sourceDisplay := m.SourceTool
		if display, ok := engine.ToolDisplayNames[m.SourceTool]; ok {
			sourceDisplay = display
		}

		healthRaw := fmt.Sprintf("%d/100 %s", s.Health, engine.HealthEmoji(s.Health))
		var healthCol string
		switch {
		case s.Health >= 80:
			healthCol = healthGoodStyle.Render(healthRaw)
		case s.Health >= 50:
			healthCol = healthWarnStyle.Render(healthRaw)
		default:
			healthCol = healthBadStyle.Render(healthRaw)
		}

		failStr := fmt.Sprintf("%d", m.ToolCallsFail)
		if m.ToolCallsFail > 0 {
			failStr = redStyle.Render(failStr)
		} else {
			failStr = dimStyle.Render(failStr)
		}
		tokensStr := fmt.Sprintf("%d", m.TokensInput+m.TokensOutput)

		rows = append(rows, table.Row{
			s.Name,
			sourceDisplay,
			fmt.Sprintf("%d", m.AssistantTurns),
			fmt.Sprintf("%d", m.ToolCallsTotal),
			sr,
			failStr,
			costColor(m.CostEstimated),
			tokensStr,
			healthCol,
		})
	}
	m.table.SetRows(rows)
	m.table.SetCursor(0)
}

// ── View ──

func (m Model) View() string {
	title := titleStyle.Render(fmt.Sprintf(i18n.T("agentwaste_title"), engine.Version))
	countBadge := lipgloss.NewStyle().
		Background(lipgloss.Color("240")).
		Foreground(lipgloss.Color("229")).
		Padding(0, 1).
		Render(fmt.Sprintf(i18n.T("sessions_count"), len(m.sessions)))
	langBadge := lipgloss.NewStyle().
		Background(lipgloss.Color("99")).
		Foreground(lipgloss.Color("229")).
		Padding(0, 1).
		Render(i18n.LangLabel())

	header := lipgloss.JoinHorizontal(lipgloss.Top, title, "  ", countBadge, " ", langBadge)
	header = lipgloss.NewStyle().Padding(0, 0, 1, 0).Render(header)

	tabs := m.renderTabs()

	var content string
	switch m.view {
	case viewOverview:
		content = m.renderOverview()
	case viewList:
		if len(m.sessions) == 0 {
			content = baseStyle.Render(lipgloss.NewStyle().Padding(1).Render(fmt.Sprintf(i18n.T("empty_sessions_hint"), m.dir, m.dir, m.dir)))
		} else {
			var filterBar string
			if m.filterActive {
				filterBar = lipgloss.NewStyle().
					Border(lipgloss.NormalBorder()).
					BorderForeground(lipgloss.Color("63")).
					Padding(0, 1).
					Render(fmt.Sprintf("/ %s_", m.filterInput))
				filterBar += "\n"
			} else if m.filterText != "" || m.filterMode != "" {
				filtInfo := fmt.Sprintf("Filter: %s=%s | %d/%d sessions",
					m.filterMode, m.filterValue,
					len(m.getFilteredSessions()), m.overview.TotalSessions)
				filterBar = dimStyle.Render(filtInfo) + "\n"
			}
			content = baseStyle.Render(filterBar + m.table.View())
		}
	case viewDetail:
		if m.detailReady {
			scrollInfo := dimStyle.Render(fmt.Sprintf(" Scroll: %.0f%% ", m.viewport.ScrollPercent()*100))

			summaryBar := m.renderQuickSummary()
			detailContent := lipgloss.JoinVertical(lipgloss.Left, summaryBar, "", scrollInfo, m.viewport.View())

			loopSection := m.renderLoopAnalysis()
			if loopSection != "" {
				detailContent = lipgloss.JoinVertical(lipgloss.Left, detailContent, "", loopSection)
			}

			fixSection := m.renderFixSuggestions()
			if fixSection != "" {
				detailContent = lipgloss.JoinVertical(lipgloss.Left, detailContent, "", fixSection)
			}

			toolWarnSection := m.renderToolWarnings()
			if toolWarnSection != "" {
				detailContent = lipgloss.JoinVertical(lipgloss.Left, detailContent, "", toolWarnSection)
			}

			costAlertSection := m.renderCostAlert()
			if costAlertSection != "" {
				detailContent = lipgloss.JoinVertical(lipgloss.Left, detailContent, "", costAlertSection)
			}

			content = baseStyle.Render(detailContent)
		} else {
			content = baseStyle.Render(dimStyle.Render(i18n.T("select_session_hint")))
		}
	case viewDiagnostics:
		content = m.renderWaste()
	case viewDiff:
		content = m.renderDiff()
	}

	help := m.renderHelp()

	return lipgloss.JoinVertical(lipgloss.Left, header, tabs, content, help)
}

func (m Model) renderTabs() string {
	tabs := []string{i18n.T("tab_overview"), i18n.T("tab_list"), i18n.T("tab_detail"), i18n.T("tab_diag"), i18n.T("tab_diff")}
	var rendered []string
	for i, t := range tabs {
		active := int(m.view) == i
		if active {
			rendered = append(rendered, lipgloss.NewStyle().
				Background(lipgloss.Color("63")).
				Foreground(lipgloss.Color("229")).
				Bold(true).
				Padding(0, 2).
				Render(t))
		} else {
			rendered = append(rendered, lipgloss.NewStyle().
				Foreground(lipgloss.Color("245")).
				Padding(0, 2).
				Render(t))
		}
	}
	return lipgloss.NewStyle().Padding(0, 0, 1, 0).Render(
		lipgloss.JoinHorizontal(lipgloss.Top, rendered...),
	)
}

func (m Model) renderHelp() string {
	var keys string
	switch m.view {
	case viewOverview:
		keys = i18n.T("help_overview")
	case viewList:
		if m.filterActive {
			keys = i18n.T("help_list") + " · Esc: clear filter"
		} else {
			keys = i18n.T("help_list")
		}
	case viewDetail:
		keys = i18n.T("help_detail")
	case viewDiagnostics:
		keys = "Esc: back · Tab: next · q: quit"
	case viewDiff:
		keys = i18n.T("help_diff")
	}
	return helpStyle.Render(" " + keys + " ")
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

	healthBadge := badge("Health", fmt.Sprintf("%d/100", s.Health), lipgloss.Color("42"))
	if s.Health < 80 {
		healthBadge = badge("Health", fmt.Sprintf("%d/100", s.Health), lipgloss.Color("220"))
	}
	if s.Health < 50 {
		healthBadge = badge("Health", fmt.Sprintf("%d/100", s.Health), lipgloss.Color("196"))
	}

	costBadge := badge("Cost", fmt.Sprintf("$%.4f", met.CostEstimated), lipgloss.Color("39"))

	totalTools := met.ToolCallsOK + met.ToolCallsFail
	srStr := "N/A"
	if totalTools > 0 {
		srStr = fmt.Sprintf("%.0f%%", float64(met.ToolCallsOK)/float64(totalTools)*100)
	}
	toolColor := lipgloss.Color("42")
	if totalTools > 0 && float64(met.ToolCallsOK)/float64(totalTools) < 0.85 {
		toolColor = lipgloss.Color("220")
	}
	if totalTools > 0 && float64(met.ToolCallsOK)/float64(totalTools) < 0.70 {
		toolColor = lipgloss.Color("196")
	}
	toolBadge := badge("Tools", fmt.Sprintf("%d/%d %s", met.ToolCallsOK, totalTools, srStr), toolColor)

	anomBadge := badge("Anomalies", fmt.Sprintf("%d", len(s.Anomalies)), lipgloss.Color("42"))
	if len(s.Anomalies) > 0 {
		anomBadge = badge("Anomalies", fmt.Sprintf("%d", len(s.Anomalies)), lipgloss.Color("196"))
	}

	modelBadge := badge("Model", met.ModelUsed, lipgloss.Color("99"))

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
	for _, fs := range m.fixSuggestions {
		msg := fmt.Sprintf("【%s】%s → %s", fs.Title, fs.Description, fs.Action)
		card := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(lipgloss.Color("42")).
			Padding(0, 1).
			Render(greenStyle.Render("  • " + msg))
		lines = append(lines, card)
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
	for _, tw := range m.toolWarnings {
		msg := fmt.Sprintf("[%s] %s (×%d)", tw.Pattern, tw.Detail, tw.Count)
		card := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(lipgloss.Color("220")).
			Padding(0, 1).
			Render(yellowStyle.Render("  ⚠ " + msg))
		lines = append(lines, card)
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
	card := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(borderColor).
		Padding(0, 1).
		Render(msg)
	lines = append(lines, card)
	return lipgloss.JoinVertical(lipgloss.Left, lines...)
}

// ═══════════════════════════════════════════════════════════════
// WASTE DIAGNOSTICS VIEW
// ═══════════════════════════════════════════════════════════════

func (m Model) renderWaste() string {
	idx := m.findSessionIndex()
	if idx < 0 || idx >= len(m.sessions) {
		return baseStyle.Render(dimStyle.Render(i18n.T("select_session_hint")))
	}
	s := m.sessions[idx]

	pCard := func(color, title string, content string) string {
		borderColor := lipgloss.Color(color)
		header := lipgloss.NewStyle().Bold(true).Foreground(borderColor).Render(title)
		return lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(borderColor).
			Padding(0, 1).
			Render(lipgloss.JoinVertical(lipgloss.Left, header, "", content))
	}

	sessionLabel := cyanStyle.Render(fmt.Sprintf("Diagnostics for: %s", s.Name))
	var sections []string
	sections = append(sections, sessionLabel, "")

	// Waste Score Summary
	scoreCard := m.renderWasteScoreCard(s)
	if scoreCard != "" {
		sections = append(sections, scoreCard, "")
	}

	// 1. Fingerprint Loop Detection
	loopFP := m.renderFingerprintLoops(s)
	sections = append(sections, pCard("196", i18n.T("diag_loop_fingerprint"), loopFP), "")

	// 2. Per-Tool Latency
	latency := m.renderToolLatency(s)
	sections = append(sections, pCard("39", i18n.T("diag_tool_latency"), latency), "")

	// 3. Context Window Utilization
	ctxUtil := m.renderContextUtil(s)
	sections = append(sections, pCard("220", i18n.T("diag_context_util"), ctxUtil), "")

	// 4. Large Parameter Calls
	largeParams := m.renderLargeParams(s)
	sections = append(sections, pCard("208", i18n.T("diag_large_params"), largeParams), "")

	// 5. Unused / Rare Tools
	unused := m.renderUnusedTools(s)
	sections = append(sections, pCard("42", i18n.T("diag_unused_tools"), unused), "")

	return baseStyle.Render(lipgloss.JoinVertical(lipgloss.Left, sections...))
}

func (m Model) renderFingerprintLoops(s engine.Session) string {
	if len(s.LoopFingerprints) == 0 {
		if s.LoopCost.TotalLoopCost > 0 {
			return yellowStyle.Render(fmt.Sprintf("⚠ Simple loop detected (cost $%.4f) — see Detail view", s.LoopCost.TotalLoopCost))
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
			worstLabel = fmt.Sprintf("%d unused tools", len(s.UnusedTools))
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

	status := "All Clear"
	if hasIssues {
		status = "Issues Found"
	}

	return lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color("63")).
		Padding(1, 2).
		Render(fmt.Sprintf("%s  %s  ·  %s  ·  %s  ·  %s  ·  %s",
			scoreStyle.Render(fmt.Sprintf("Score %d/100", score)),
			lipgloss.NewStyle().Bold(true).Render(status),
			dimStyle.Render(fmt.Sprintf("Turns %d", met.AssistantTurns)),
			dimStyle.Render(fmt.Sprintf("Tools %d", met.ToolCallsTotal)),
			dimStyle.Render(fmt.Sprintf("Cost $%.4f", met.CostEstimated)),
			lipgloss.NewStyle().Foreground(lipgloss.Color("99")).Render(met.ModelUsed),
		))
}

func (m Model) renderToolLatency(s engine.Session) string {
	if len(s.ToolLatencies) == 0 {
		return dimStyle.Render("No latency data available")
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
			timeoutStr = redStyle.Render(fmt.Sprintf("%d", tl.Timeouts))
		}
		style := lipgloss.NewStyle()
		if tl.IsSlow {
			style = style.Foreground(lipgloss.Color("196"))
		}
		rows = append(rows, style.Render(fmt.Sprintf("  %-18s %5d %5.1fs %5.1fs %5.1fs %4s%s",
			tl.ToolName, tl.Count, tl.AvgSec, tl.P95Sec, tl.MaxSec, timeoutStr, slowBadge)))
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
	filled := int(cu.UtilizationPct / 100 * float64(barWidth))
	if filled > barWidth {
		filled = barWidth
	}
	bar := "[" + lipgloss.NewStyle().Foreground(lipgloss.Color("196")).Render(strings.Repeat("█", filled)) +
		lipgloss.NewStyle().Foreground(lipgloss.Color("240")).Render(strings.Repeat("░", barWidth-filled)) + "]"

	content := fmt.Sprintf("  %-22s %s\n", i18n.T("diag_ctx_total"), cyanStyle.Render(fmt.Sprintf("%d tokens", cu.EstimatedTotal)))
	content += fmt.Sprintf("  %-22s %s\n", i18n.T("diag_ctx_tool_defs"), dimStyle.Render(fmt.Sprintf("%d tokens", cu.ToolDefinitions)))
	content += fmt.Sprintf("  %-22s %s\n", i18n.T("diag_ctx_history"), dimStyle.Render(fmt.Sprintf("%d tokens", cu.ConversationHist)))
	content += fmt.Sprintf("  %-22s %s\n", i18n.T("diag_ctx_sysprompt"), dimStyle.Render(fmt.Sprintf("%d tokens", cu.SystemPrompt)))
	content += fmt.Sprintf("  ─────────────────────────────\n")
	content += fmt.Sprintf("  %-22s %s %s\n", i18n.T("diag_ctx_available"),
		style.Render(fmt.Sprintf("%d tokens", cu.AvailableForTask)), bar)
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
		kb := float64(lp.ParamSize) / 1024.0
		lines = append(lines, style.Render(fmt.Sprintf("• [%s] %s: %.1f KB — %s", lp.Risk, lp.ToolName, kb, lp.Detail)))
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
		if ut.Level == "unused" {
			style = redStyle
		}
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
	content += fmt.Sprintf("  %-22s %s\n", i18n.T("diag_cost_total_burned"), redStyle.Render(fmt.Sprintf("$%.2f", cs.TotalCost)))
	content += fmt.Sprintf("  %-22s %s\n", i18n.T("diag_cost_avg_turn"), yellowStyle.Render(fmt.Sprintf("$%.4f", cs.AvgCostPerTurn)))
	content += fmt.Sprintf("  %-22s %s\n", i18n.T("diag_cost_costliest"), orangeStyle.Render(fmt.Sprintf("%s $%.2f", cs.CostliestModel, cs.CostliestCost)))
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

	headerA := lipgloss.NewStyle().Bold(true).Width(22).Render(dr.SessionA)
	headerB := lipgloss.NewStyle().Bold(true).Width(22).Foreground(lipgloss.Color("39")).Render(dr.SessionB)
	header := lipgloss.JoinHorizontal(lipgloss.Top, headerA, lipgloss.NewStyle().Width(3).Render(""), headerB)

	sep := dimStyle.Render("──────────────────────────────────────────────")

	rows := []string{header, sep}

	for _, e := range dr.Entries {
		valA := lipgloss.NewStyle().Width(22).Render(fmt.Sprintf("%s: %s", e.Field, e.ValueA))
		var valBStyle lipgloss.Style
		if e.Better == "B" {
			valBStyle = lipgloss.NewStyle().Width(22).Foreground(lipgloss.Color("42"))
		} else if e.Better == "A" {
			valBStyle = lipgloss.NewStyle().Width(22).Foreground(lipgloss.Color("196"))
		} else {
			valBStyle = lipgloss.NewStyle().Width(22)
		}
		valB := valBStyle.Render(fmt.Sprintf("%s: %s %s", e.Field, e.ValueB, e.Delta))
		rows = append(rows, lipgloss.JoinHorizontal(lipgloss.Top, valA, lipgloss.NewStyle().Width(3).Render(""), valB))
	}

	rows = append(rows, sep)

	summary := greenStyle.Render(dr.Summary)
	rows = append(rows, summary)

	return baseStyle.Render(lipgloss.JoinVertical(lipgloss.Left, rows...))
}

// findSessionIndex returns the index of the currently-viewed session in m.sessions.
func (m *Model) findSessionIndex() int {
	if len(m.sessions) == 0 {
		return -1
	}
	cursor := m.table.Cursor()
	// When a filter is active, map through filteredIndices
	if len(m.filteredIndices) > 0 {
		if cursor >= 0 && cursor < len(m.filteredIndices) {
			return m.filteredIndices[cursor]
		}
		return -1
	}
	// No filter active: use cursor directly
	if cursor >= 0 && cursor < len(m.sessions) {
		return cursor
	}
	return -1
}

func (m *Model) sortAndRefresh() {
	switch m.sortBy {
	case "health":
		sort.SliceStable(m.sessions, func(i, j int) bool {
			if m.sortDesc {
				return m.sessions[i].Health > m.sessions[j].Health
			}
			return m.sessions[i].Health < m.sessions[j].Health
		})
	case "cost":
		sort.SliceStable(m.sessions, func(i, j int) bool {
			if m.sortDesc {
				return m.sessions[i].Metrics.CostEstimated > m.sessions[j].Metrics.CostEstimated
			}
			return m.sessions[i].Metrics.CostEstimated < m.sessions[j].Metrics.CostEstimated
		})
	case "turns":
		sort.SliceStable(m.sessions, func(i, j int) bool {
			if m.sortDesc {
				return m.sessions[i].Metrics.AssistantTurns > m.sessions[j].Metrics.AssistantTurns
			}
			return m.sessions[i].Metrics.AssistantTurns < m.sessions[j].Metrics.AssistantTurns
		})
	case "name":
		sort.SliceStable(m.sessions, func(i, j int) bool {
			if m.sortDesc {
				return m.sessions[i].Name > m.sessions[j].Name
			}
			return m.sessions[i].Name < m.sessions[j].Name
		})
	}
	m.refreshTable()
	m.rebuildFilteredIndices()
	// Re-apply filter to table rows if a filter is active
	if m.filterMode != "" || m.filterText != "" {
		m.rebuildTableWithFilteredIndices()
	}
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
	s := fmt.Sprintf("$%.4f", amount)
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
	listCols := []table.Column{
		{Title: m.sortColTitle(i18n.T("session"), "name"), Width: 20},
		{Title: m.sortColTitle(i18n.T("source_tool"), ""), Width: 12},
		{Title: m.sortColTitle(i18n.T("turns_header"), "turns"), Width: 5},
		{Title: m.sortColTitle(i18n.T("tools"), ""), Width: 5},
		{Title: m.sortColTitle(i18n.T("succ_pct"), ""), Width: 5},
		{Title: m.sortColTitle(i18n.T("fail"), ""), Width: 5},
		{Title: m.sortColTitle(i18n.T("cost"), "cost"), Width: 8},
		{Title: m.sortColTitle(i18n.T("tokens"), ""), Width: 7},
		{Title: m.sortColTitle(i18n.T("health"), "health"), Width: 9},
	}
	m.table.SetColumns(listCols)

	if m.width > 0 {
		m.adjustColumnWidths(m.width)
	}
}

// adjustColumnWidths dynamically resizes columns based on terminal width.
func (m *Model) adjustColumnWidths(width int) {
	var sessW, srcW, turnsW, toolsW, succW, failW, costW, tokensW, healthW int

	if width > 130 {
		sessW, srcW, turnsW, toolsW, succW, failW, costW, tokensW, healthW = 20, 12, 5, 5, 5, 5, 8, 7, 9
	} else if width >= 100 {
		sessW, srcW, turnsW, toolsW, succW, failW, costW, tokensW, healthW = 17, 10, 5, 5, 5, 4, 8, 7, 8
	} else if width >= 80 {
		sessW, srcW, turnsW, toolsW, succW, failW, costW, tokensW, healthW = 14, 9, 5, 5, 5, 4, 8, 6, 8
	} else {
		sessW, srcW, turnsW, toolsW, succW, failW, costW, tokensW, healthW = 12, 8, 4, 4, 4, 4, 7, 5, 8
	}

	listCols := []table.Column{
		{Title: m.sortColTitle(i18n.T("session"), "name"), Width: sessW},
		{Title: m.sortColTitle(i18n.T("source_tool"), ""), Width: srcW},
		{Title: m.sortColTitle(i18n.T("turns_header"), "turns"), Width: turnsW},
		{Title: m.sortColTitle(i18n.T("tools"), ""), Width: toolsW},
		{Title: m.sortColTitle(i18n.T("succ_pct"), ""), Width: succW},
		{Title: m.sortColTitle(i18n.T("fail"), ""), Width: failW},
		{Title: m.sortColTitle(i18n.T("cost"), "cost"), Width: costW},
		{Title: m.sortColTitle(i18n.T("tokens"), ""), Width: tokensW},
		{Title: m.sortColTitle(i18n.T("health"), "health"), Width: healthW},
	}
	m.table.SetColumns(listCols)
}

// ═══════════════════════════════════════════════════════════════
// OVERVIEW RENDER
// ═══════════════════════════════════════════════════════════════

func (m Model) renderOverview() string {
	ov := m.overview

	healthyPct := 0
	warnPct := 0
	critPct := 0
	if ov.TotalSessions > 0 {
		healthyPct = ov.Healthy * 100 / ov.TotalSessions
		warnPct = ov.Warning * 100 / ov.TotalSessions
		critPct = ov.Critical * 100 / ov.TotalSessions
	}

	usableW := m.width - 4
	if usableW < 50 {
		usableW = 50
	}

	card := func(color, content string, width int) string {
		return lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(lipgloss.Color(color)).
			Padding(1, 2).
			Width(width).
			Render(content)
	}

	var colW, colW2, colW3 int
	var layoutMode string
	if usableW > 110 {
		colW = (usableW - 8) / 3
		colW2 = colW
		colW3 = colW
		layoutMode = "3col"
	} else if usableW >= 80 {
		colW = (usableW - 4) / 2
		colW2 = colW
		colW3 = colW
		layoutMode = "2col"
	} else {
		colW = usableW
		colW2 = usableW
		colW3 = usableW
		layoutMode = "stack"
	}

	// ── Stats Card ──
	statsContent := fmt.Sprintf("%s  %d\n", boldStyle.Render(i18n.T("overview_total")), ov.TotalSessions)
	statsContent += fmt.Sprintf("%s     %s %d (%d%%)\n",
		greenStyle.Render("🟢"), i18n.T("overview_healthy"), ov.Healthy, healthyPct)
	statsContent += fmt.Sprintf("%s     %s %d (%d%%)\n",
		yellowStyle.Render("🟡"), i18n.T("overview_warning"), ov.Warning, warnPct)
	statsContent += fmt.Sprintf("%s     %s %d (%d%%)\n",
		redStyle.Render("🔴"), i18n.T("overview_critical"), ov.Critical, critPct)
	statsContent += fmt.Sprintf("%s     %d\n", orangeStyle.Render("⚠️"), len(ov.AnomaliesTop))
	statsContent += fmt.Sprintf("%s  $%.2f", cyanStyle.Render("💰"), ov.TotalCost)
	statsCard := card("63", statsContent, colW)

	// ── Agents Card ──
	agentContent := boldStyle.Render(i18n.T("overview_agents")) + "\n"
	type akv struct {
		k string
		v engine.AgentOverview
	}
	var agents []akv
	for k, v := range ov.ByAgent {
		agents = append(agents, akv{k, v})
	}
	sort.Slice(agents, func(i, j int) bool { return agents[i].v.Sessions > agents[j].v.Sessions })
	for _, a := range agents {
		display := a.k
		if d, ok := engine.ToolDisplayNames[a.k]; ok {
			display = d
		}
		maxLen := colW2 - 15
		if maxLen > 20 {
			maxLen = 20
		}
		if maxLen < 8 {
			maxLen = 8
		}
		if len(display) > maxLen {
			display = display[:maxLen]
		}
		agentContent += fmt.Sprintf("  %-*s %3d  $%7.2f\n", maxLen, display, a.v.Sessions, a.v.Cost)
	}
	agentCard := card("39", agentContent, colW2)

	// ── Models Card ──
	modelContent := boldStyle.Render(i18n.T("overview_models")) + "\n"
	type mkv struct {
		k string
		v engine.ModelOverview
	}
	var models []mkv
	for k, v := range ov.ByModel {
		models = append(models, mkv{k, v})
	}
	sort.Slice(models, func(i, j int) bool { return models[i].v.Cost > models[j].v.Cost })
	maxModels := 6
	if layoutMode == "stack" {
		maxModels = 4
	}
	for i, mdl := range models {
		if i >= maxModels {
			break
		}
		display := mdl.k
		if display == "unknown" {
			continue
		}
		maxLen := colW3 - 15
		if maxLen > 18 {
			maxLen = 18
		}
		if maxLen < 8 {
			maxLen = 8
		}
		if len(display) > maxLen {
			display = display[:maxLen]
		}
		modelContent += fmt.Sprintf("  %-*s %3d  $%7.2f\n", maxLen, display, mdl.v.Sessions, mdl.v.Cost)
	}
	modelCard := card("220", modelContent, colW3)

	// ── Anomalies Card ──
	anomalyCardW := colW
	if layoutMode == "2col" {
		anomalyCardW = colW2
	}
	anomalyContent := boldStyle.Render(i18n.T("overview_recent_anomalies")) + "\n"
	if len(ov.AnomaliesTop) == 0 {
		anomalyContent += greenStyle.Render("  " + i18n.T("overview_no_anomalies"))
	} else {
		maxAnom := 5
		if layoutMode == "stack" {
			maxAnom = 3
		}
		for i, a := range ov.AnomaliesTop {
			if i >= maxAnom {
				break
			}
			name := a.Session
			nameMax := anomalyCardW - 15
			if nameMax > 22 {
				nameMax = 22
			}
			if nameMax < 8 {
				nameMax = 8
			}
			if len(name) > nameMax {
				name = name[:nameMax]
			}
			anomalyContent += fmt.Sprintf("  ⚠️ %-*s %s\n", nameMax, name, a.Type)
		}
	}
	anomalyCard := card("196", anomalyContent, anomalyCardW)

	// ── Health Trend ──
	m.healthTrend = engine.AnalyzeHealthTrend(m.sessions)
	var trendLine string
	if m.healthTrend.Regressing {
		trendLine = redStyle.Render(m.healthTrend.Message)
	} else if m.healthTrend.Direction == "down" {
		trendLine = redStyle.Render(m.healthTrend.Message)
	} else if m.healthTrend.Direction == "up" {
		trendLine = cyanStyle.Render(m.healthTrend.Message)
	} else {
		trendLine = greenStyle.Render(m.healthTrend.Message)
	}
	trendCardW := colW
	if layoutMode == "2col" {
		trendCardW = colW3
	}
	trendCard := card("63", boldStyle.Render(i18n.T("trend_title"))+"\n"+trendLine, trendCardW)

	// ── Cost Summary Card ──
	costSumCard := m.renderCostSummaryCard()

	// ── Responsive Layout ──
	var result string
	switch layoutMode {
	case "3col":
		topRow := lipgloss.JoinHorizontal(lipgloss.Top, statsCard, "  ", agentCard, "  ", modelCard)
		bottomRow := lipgloss.JoinHorizontal(lipgloss.Top, anomalyCard, "  ", trendCard)
		result = lipgloss.JoinVertical(lipgloss.Left, topRow, "", bottomRow)
		if costSumCard != "" {
			result = lipgloss.JoinVertical(lipgloss.Left, result, "", costSumCard)
		}
	case "2col":
		row1 := lipgloss.JoinHorizontal(lipgloss.Top, statsCard, "  ", agentCard)
		row2 := lipgloss.JoinHorizontal(lipgloss.Top, modelCard, "  ", anomalyCard)
		result = lipgloss.JoinVertical(lipgloss.Left, row1, "", row2, "", trendCard)
		if costSumCard != "" {
			result = lipgloss.JoinVertical(lipgloss.Left, result, "", costSumCard)
		}
	default:
		result = lipgloss.JoinVertical(lipgloss.Left,
			statsCard, "", agentCard, "", modelCard, "", anomalyCard, "", trendCard)
		if costSumCard != "" {
			result = lipgloss.JoinVertical(lipgloss.Left, result, "", costSumCard)
		}
	}

	return baseStyle.Render(result)
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

func (m *Model) getFilteredSessions() []engine.Session {
	var filtered []engine.Session
	for _, s := range m.sessions {
		keep := true
		switch m.filterMode {
		case "health":
			switch m.filterValue {
			case "good":
				keep = s.Health >= 80
			case "warn":
				keep = s.Health >= 50 && s.Health < 80
			case "crit":
				keep = s.Health < 50
			default:
				keep = false
			}
		case "source":
			keep = s.Metrics.SourceTool == m.filterValue
		}
		if keep {
			filtered = append(filtered, s)
		}
	}
	return filtered
}

func (m *Model) rebuildTableWithFilteredIndices() {
	if len(m.filteredIndices) == 0 {
		m.refreshTable()
		return
	}
	var rows []table.Row
	for _, idx := range m.filteredIndices {
		s := m.sessions[idx]
		met := s.Metrics
		totalTools := met.ToolCallsOK + met.ToolCallsFail
		sr := "N/A"
		if totalTools > 0 {
			sr = fmt.Sprintf("%.0f", float64(met.ToolCallsOK)/float64(totalTools)*100)
		}
		sourceDisplay := met.SourceTool
		if display, ok := engine.ToolDisplayNames[met.SourceTool]; ok {
			sourceDisplay = display
		}
		healthRaw := fmt.Sprintf("%d/100 %s", s.Health, engine.HealthEmoji(s.Health))
		var healthCol string
		switch {
		case s.Health >= 80:
			healthCol = healthGoodStyle.Render(healthRaw)
		case s.Health >= 50:
			healthCol = healthWarnStyle.Render(healthRaw)
		default:
			healthCol = healthBadStyle.Render(healthRaw)
		}
		rows = append(rows, table.Row{
			s.Name,
			sourceDisplay,
			fmt.Sprintf("%d", met.AssistantTurns),
			fmt.Sprintf("%d", met.ToolCallsTotal),
			sr,
			costColor(met.CostEstimated),
			healthCol,
		})
	}
	m.table.SetRows(rows)
	m.table.SetCursor(0)
}

func (m *Model) rebuildFilteredIndices() {
	m.filteredIndices = nil
	for i, s := range m.sessions {
		include := true
		if m.filterMode == "health" {
			switch m.filterValue {
			case "good":
				include = s.Health >= 80
			case "warn":
				include = s.Health >= 50 && s.Health < 80
			case "crit":
				include = s.Health < 50
			}
		} else if m.filterMode == "source" {
			include = s.Metrics.SourceTool == m.filterValue
		}
		if m.filterText != "" {
			q := strings.ToLower(m.filterText)
			if !strings.Contains(strings.ToLower(s.Name), q) &&
				!strings.Contains(strings.ToLower(s.Metrics.SourceTool), q) {
				include = false
			}
		}
		if include {
			m.filteredIndices = append(m.filteredIndices, i)
		}
	}
	if len(m.filteredIndices) == 0 {
		m.filteredIndices = []int{}
	}
}

func (m *Model) applyFilter() {
	filtered := m.getFilteredSessions()
	m.rebuildTableWithSessions(filtered)
	m.rebuildFilteredIndices()
}

func (m *Model) applyTextFilter() {
	filtered := engine.FilterSessions(m.sessions, m.filterText)
	m.rebuildTableWithSessions(filtered)
	m.rebuildFilteredIndices()
}

func (m *Model) rebuildTableWithSessions(filtered []engine.Session) {
	var rows []table.Row
	for _, s := range filtered {
		met := s.Metrics
		totalTools := met.ToolCallsOK + met.ToolCallsFail
		sr := "N/A"
		if totalTools > 0 {
			sr = fmt.Sprintf("%.0f", float64(met.ToolCallsOK)/float64(totalTools)*100)
		}
		sourceDisplay := met.SourceTool
		if display, ok := engine.ToolDisplayNames[met.SourceTool]; ok {
			sourceDisplay = display
		}
		healthRaw := fmt.Sprintf("%d/100 %s", s.Health, engine.HealthEmoji(s.Health))
		var healthCol string
		switch {
		case s.Health >= 80:
			healthCol = healthGoodStyle.Render(healthRaw)
		case s.Health >= 50:
			healthCol = healthWarnStyle.Render(healthRaw)
		default:
			healthCol = healthBadStyle.Render(healthRaw)
		}
		failStr := fmt.Sprintf("%d", met.ToolCallsFail)
		if met.ToolCallsFail > 0 {
			failStr = redStyle.Render(failStr)
		} else {
			failStr = dimStyle.Render(failStr)
		}
		tokensStr := fmt.Sprintf("%d", met.TokensInput+met.TokensOutput)
		rows = append(rows, table.Row{
			s.Name, sourceDisplay,
			fmt.Sprintf("%d", met.AssistantTurns),
			fmt.Sprintf("%d", met.ToolCallsTotal),
			sr,
			failStr,
			costColor(met.CostEstimated),
			tokensStr,
			healthCol,
		})
	}
	m.table.SetRows(rows)
	m.table.SetCursor(0)
}
