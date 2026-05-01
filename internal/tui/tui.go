// Package tui provides the Bubble Tea interactive terminal UI for agenttrace.
// btop-style modern dashboard with three views: Session List, Detail, Compare.
package tui

import (
	"fmt"
	"sort"

	"github.com/charmbracelet/bubbles/table"
	"github.com/charmbracelet/bubbles/viewport"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/luoyuctl/agenttrace/internal/engine"
	"github.com/luoyuctl/agenttrace/internal/i18n"
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
	viewCompare
)

type Model struct {
	view     view
	sessions []engine.Session
	dir      string
	lang     i18n.Lang

	// Overview data
	overview engine.Overview

	// Filter state
	filterText  string
	filterMode  string // "", "health", "source"
	filterValue string // e.g. "good", "hermes_jsonl"

	// 文本筛选输入
	filterInput  string // 用户输入的筛选文本
	filterActive bool   // 是否处于文本筛选模式

	// 全局聚合统计
	aggStats engine.AggregateStats

	// List view
	table       table.Model
	tableReady  bool

	// Detail view
	viewport    viewport.Model
	detailReady bool
	loopResult  engine.LoopResult // 循环检测结果

	// Compare view
	compareLines []string
	compareTable table.Model
	compareReady bool

	// Dimensions
	width  int
	height int
}

func New(dir string) Model {
	sessions := engine.LoadAll(dir)
	overview := engine.ComputeOverview(sessions)

	// Build table
	columns := []table.Column{
		{Title: i18n.T("session"), Width: 22},
		{Title: i18n.T("source_tool"), Width: 14},
		{Title: i18n.T("turns_header"), Width: 6},
		{Title: i18n.T("tools"), Width: 6},
		{Title: i18n.T("succ_pct"), Width: 6},
		{Title: i18n.T("cost"), Width: 9},
		{Title: i18n.T("health"), Width: 9},
	}

	var rows []table.Row
	var compareRows []table.Row
	var compareData [][]string

	for _, s := range sessions {
		m := s.Metrics
		totalTools := m.ToolCallsOK + m.ToolCallsFail
		sr := "N/A"
		if totalTools > 0 {
			sr = fmt.Sprintf("%.0f", float64(m.ToolCallsOK)/float64(totalTools)*100)
		}

		// Source tool display
		sourceDisplay := m.SourceTool
		if display, ok := engine.ToolDisplayNames[m.SourceTool]; ok {
			sourceDisplay = display
		}

		// Health with color
		healthRaw := fmt.Sprintf("  %d/100 %s", s.Health, engine.HealthEmoji(s.Health))
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
			fmt.Sprintf("%d", m.AssistantTurns),
			fmt.Sprintf("%d", m.ToolCallsTotal),
			sr,
			fmt.Sprintf("$%.4f", m.CostEstimated),
			healthCol,
		})

		failStr := fmt.Sprintf("%d", m.ToolCallsFail)
		tokensStr := fmt.Sprintf("%d", m.TokensInput+m.TokensOutput)
		compareRows = append(compareRows, table.Row{
			s.Name,
			sourceDisplay,
			fmt.Sprintf("%d", m.AssistantTurns),
			fmt.Sprintf("%d", m.ToolCallsTotal),
			sr,
			failStr,
			fmt.Sprintf("$%.4f", m.CostEstimated),
			tokensStr,
			healthCol,
		})
		compareData = append(compareData, []string{
			s.Name, sourceDisplay,
			fmt.Sprintf("%d", m.AssistantTurns),
			fmt.Sprintf("%d", m.ToolCallsTotal), sr, failStr,
			fmt.Sprintf("$%.4f", m.CostEstimated), tokensStr, healthCol,
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

	// Compare table
	compCols := []table.Column{
		{Title: i18n.T("session"), Width: 22},
		{Title: i18n.T("source_tool"), Width: 14},
		{Title: i18n.T("turns_header"), Width: 6},
		{Title: i18n.T("tools"), Width: 6},
		{Title: i18n.T("succ_pct"), Width: 6},
		{Title: i18n.T("fail"), Width: 5},
		{Title: i18n.T("cost"), Width: 9},
		{Title: i18n.T("tokens"), Width: 7},
		{Title: i18n.T("health"), Width: 10},
	}
	ct := table.New(
		table.WithColumns(compCols),
		table.WithRows(compareRows),
		table.WithHeight(20),
	)
	cs := table.DefaultStyles()
	cs.Header = cs.Header.
		BorderStyle(lipgloss.NormalBorder()).
		BorderForeground(lipgloss.Color("240")).
		BorderBottom(true).
		Bold(true).
		Foreground(lipgloss.Color("39"))
	cs.Selected = cs.Selected.
		Foreground(lipgloss.Color("229")).
		Background(lipgloss.Color("63"))
	ct.SetStyles(cs)

	return Model{
		view:         viewOverview,
		sessions:     sessions,
		dir:          dir,
		lang:         i18n.EN,
		overview:     overview,
		aggStats:     engine.ComputeAggregateStats(sessions),
		table:        t,
		tableReady:   true,
		compareTable: ct,
		compareReady: true,
	}
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

		// Resize tables
		m.table.SetWidth(msg.Width - 4)
		m.table.SetHeight(msg.Height - 8)
		m.compareTable.SetWidth(msg.Width - 4)
		m.compareTable.SetHeight(msg.Height - 8)

		// Adjust column widths responsively
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
			// Cycle views
			switch m.view {
			case viewOverview:
				m.view = viewList
			case viewList:
				m.view = viewDetail
				m.openDetail()
			case viewDetail:
				m.view = viewCompare
			case viewCompare:
				m.view = viewOverview
			}

		case "r":
			// Reload
			m.sessions = engine.LoadAll(m.dir)
			m.overview = engine.ComputeOverview(m.sessions)
			m.aggStats = engine.ComputeAggregateStats(m.sessions)
			m.refreshTable()
			m.refreshCompare()

		case "L", "l":
			// Toggle language
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
			m.view = viewDetail
			m.openDetail()
		case "3":
			m.view = viewCompare

		case "/":
			if m.view == viewList {
				m.filterActive = true
				m.filterInput = ""
			}

		// 文本筛选输入处理
		case "enter":
			if m.filterActive {
				// 应用文本筛选
				m.filterText = m.filterInput
				m.filterActive = false
				m.applyTextFilter()
			}
		case "esc":
			if m.filterActive {
				// 退出筛选模式
				m.filterActive = false
				m.filterInput = ""
			} else if m.view == viewDetail || m.view == viewCompare {
				m.view = viewList
			}
		case "backspace":
			if m.filterActive && len(m.filterInput) > 0 {
				m.filterInput = m.filterInput[:len(m.filterInput)-1]
			}

		case "f":
			if !m.filterActive && m.view == viewList {
				// Cycle health filter
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
				// Cycle source filter
				if m.filterMode != "source" {
					m.filterMode = "source"
					m.filterValue = "hermes_jsonl"
				} else {
					// Cycle through available sources
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

		default:
			// 筛选输入模式：捕获按键
			if m.filterActive {
				// 可打印字符追加到 filterInput
				if len(msg.Runes) > 0 {
					m.filterInput += string(msg.Runes)
				}
				return m, nil
			}
			// Pass keys to the active component
			switch m.view {
			case viewList:
				m.table, cmd = m.table.Update(msg)
				if msg.String() == "enter" && m.tableReady {
					m.view = viewDetail
					m.openDetail()
				}
			case viewDetail:
				m.viewport, cmd = m.viewport.Update(msg)
			case viewCompare:
				m.compareTable, cmd = m.compareTable.Update(msg)
				if msg.String() == "h" && m.compareReady {
					// Sort sessions by health descending
					sort.SliceStable(m.sessions, func(i, j int) bool {
						return m.sessions[i].Health > m.sessions[j].Health
					})
					m.refreshCompare()
				}
			}
		}
	}

	return m, cmd
}

func (m *Model) openDetail() {
	if !m.tableReady || len(m.sessions) == 0 {
		return
	}
	idx := m.table.Cursor()
	if idx >= 0 && idx < len(m.sessions) {
		s := m.sessions[idx]
		text := engine.ReportText(s.Metrics, s.Anomalies, s.Health)
		m.viewport = viewport.New(m.width-4, m.height-6)
		m.viewport.SetContent(text)
		m.detailReady = true

		// 循环检测：解析会话事件并分析循环
		events, err := engine.Parse(s.Path)
		if err == nil {
			m.loopResult = engine.AnalyzeLoops(events)
		} else {
			m.loopResult = engine.LoopResult{}
		}
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

		// Source tool display
		sourceDisplay := m.SourceTool
		if display, ok := engine.ToolDisplayNames[m.SourceTool]; ok {
			sourceDisplay = display
		}

		// Health with color
		healthRaw := fmt.Sprintf("  %d/100 %s", s.Health, engine.HealthEmoji(s.Health))
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
			fmt.Sprintf("%d", m.AssistantTurns),
			fmt.Sprintf("%d", m.ToolCallsTotal),
			sr,
			fmt.Sprintf("$%.4f", m.CostEstimated),
			healthCol,
		})
	}
	m.table.SetRows(rows)
	m.table.SetCursor(0)
}

func (m *Model) refreshCompare() {
	var rows []table.Row
	for _, s := range m.sessions {
		m := s.Metrics
		totalTools := m.ToolCallsOK + m.ToolCallsFail
		sr := "N/A"
		if totalTools > 0 {
			sr = fmt.Sprintf("%.0f", float64(m.ToolCallsOK)/float64(totalTools)*100)
		}

		// Source tool display
		sourceDisplay := m.SourceTool
		if display, ok := engine.ToolDisplayNames[m.SourceTool]; ok {
			sourceDisplay = display
		}

		// Health with color
		healthRaw := fmt.Sprintf("  %d/100 %s", s.Health, engine.HealthEmoji(s.Health))
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
			fmt.Sprintf("%d", m.AssistantTurns),
			fmt.Sprintf("%d", m.ToolCallsTotal),
			sr,
			fmt.Sprintf("%d", m.ToolCallsFail),
			fmt.Sprintf("$%.4f", m.CostEstimated),
			fmt.Sprintf("%d", m.TokensInput+m.TokensOutput),
			healthCol,
		})
	}
	m.compareTable.SetRows(rows)
}

// ── View ──

func (m Model) View() string {
	// Title bar
	title := titleStyle.Render(fmt.Sprintf(i18n.T("agenttrace_title"), engine.Version))
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

	// View tabs
	tabs := m.renderTabs()

	// Content
	var content string
	switch m.view {
	case viewOverview:
		content = m.renderOverview()
	case viewList:
		if len(m.sessions) == 0 {
			content = baseStyle.Render(lipgloss.NewStyle().Padding(1).Render(i18n.T("empty_sessions_hint")))
		} else {
			var filterBar string
			// 文本筛选输入栏
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
			detailContent := lipgloss.JoinVertical(lipgloss.Left, scrollInfo, m.viewport.View())

			// 循环成本展示
			loopSection := m.renderLoopAnalysis()
			if loopSection != "" {
				detailContent = lipgloss.JoinVertical(lipgloss.Left, detailContent, "", loopSection)
			}
			content = baseStyle.Render(detailContent)
		} else {
			content = baseStyle.Render(dimStyle.Render(i18n.T("select_session_hint")))
		}
	case viewCompare:
		if len(m.sessions) == 0 {
			content = baseStyle.Render(lipgloss.NewStyle().Padding(1).Render(i18n.T("empty_sessions_hint")))
		} else {
			content = baseStyle.Render(m.compareTable.View())
		}
	}

	// Help bar
	help := m.renderHelp()

	return lipgloss.JoinVertical(lipgloss.Left, header, tabs, content, help)
}

func (m Model) renderTabs() string {
	tabs := []string{i18n.T("tab_overview"), i18n.T("tab_list"), i18n.T("tab_detail"), i18n.T("tab_compare")}
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
	case viewCompare:
		keys = i18n.T("help_compare")
	}
	return helpStyle.Render(" " + keys + " ")
}

// renderLoopAnalysis 渲染循环成本分析 section
func (m Model) renderLoopAnalysis() string {
	if !m.detailReady {
		return ""
	}
	lr := m.loopResult
	if lr.HasLoop {
		// 检测到循环：红色警告
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
	// 未检测到循环：绿色
	loopCard := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color("42")).
		Padding(0, 1)
	content := fmt.Sprintf("%s %s",
		greenStyle.Render("✅"),
		i18n.T("no_loop"))
	return loopCard.Render(content)
}

// refreshColumns rebuilds column titles for both tables after a language switch.
func (m *Model) refreshColumns() {
	listCols := []table.Column{
		{Title: i18n.T("session"), Width: 22},
		{Title: i18n.T("source_tool"), Width: 14},
		{Title: i18n.T("turns_header"), Width: 6},
		{Title: i18n.T("tools"), Width: 6},
		{Title: i18n.T("succ_pct"), Width: 6},
		{Title: i18n.T("cost"), Width: 9},
		{Title: i18n.T("health"), Width: 9},
	}
	m.table.SetColumns(listCols)

	compCols := []table.Column{
		{Title: i18n.T("session"), Width: 22},
		{Title: i18n.T("source_tool"), Width: 14},
		{Title: i18n.T("turns_header"), Width: 6},
		{Title: i18n.T("tools"), Width: 6},
		{Title: i18n.T("succ_pct"), Width: 6},
		{Title: i18n.T("fail"), Width: 5},
		{Title: i18n.T("cost"), Width: 9},
		{Title: i18n.T("tokens"), Width: 7},
		{Title: i18n.T("health"), Width: 10},
	}
	m.compareTable.SetColumns(compCols)

	// Re-apply responsive sizing if we know the width
	if m.width > 0 {
		m.adjustColumnWidths(m.width)
	}
}

// adjustColumnWidths dynamically resizes columns based on terminal width.
func (m *Model) adjustColumnWidths(width int) {
	var (
		sessW, srcW, turnsW, toolsW, succW, costW, healthW int
		failW, tokensW                                      int
	)

	if width > 120 {
		sessW, srcW, turnsW, toolsW, succW, costW, healthW = 22, 14, 6, 6, 6, 9, 9
		failW, tokensW = 5, 7
	} else if width >= 80 {
		sessW, srcW, turnsW, toolsW, succW, costW, healthW = 18, 12, 5, 5, 5, 8, 8
		failW, tokensW = 5, 6
	} else {
		sessW, srcW, turnsW, toolsW, succW, costW, healthW = 16, 10, 5, 5, 5, 8, 8
		failW, tokensW = 4, 5
	}

	listCols := []table.Column{
		{Title: i18n.T("session"), Width: sessW},
		{Title: i18n.T("source_tool"), Width: srcW},
		{Title: i18n.T("turns_header"), Width: turnsW},
		{Title: i18n.T("tools"), Width: toolsW},
		{Title: i18n.T("succ_pct"), Width: succW},
		{Title: i18n.T("cost"), Width: costW},
		{Title: i18n.T("health"), Width: healthW},
	}
	m.table.SetColumns(listCols)

	compCols := []table.Column{
		{Title: i18n.T("session"), Width: sessW},
		{Title: i18n.T("source_tool"), Width: srcW},
		{Title: i18n.T("turns_header"), Width: turnsW},
		{Title: i18n.T("tools"), Width: toolsW},
		{Title: i18n.T("succ_pct"), Width: succW},
		{Title: i18n.T("fail"), Width: failW},
		{Title: i18n.T("cost"), Width: costW},
		{Title: i18n.T("tokens"), Width: tokensW},
		{Title: i18n.T("health"), Width: healthW},
	}
	m.compareTable.SetColumns(compCols)
}

// ═══════════════════════════════════════════════════════════════
// OVERVIEW RENDER
// ═══════════════════════════════════════════════════════════════

func (m Model) renderOverview() string {
	ov := m.overview

	// Stats percentages
	healthyPct := 0
	warnPct := 0
	critPct := 0
	if ov.TotalSessions > 0 {
		healthyPct = ov.Healthy * 100 / ov.TotalSessions
		warnPct = ov.Warning * 100 / ov.TotalSessions
		critPct = ov.Critical * 100 / ov.TotalSessions
	}

	// Card: Stats
	statsCard := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color("63")).
		Padding(1, 2).
		Width(40)

	statsContent := fmt.Sprintf("%s  %d\n", boldStyle.Render(i18n.T("overview_total")), ov.TotalSessions)
	statsContent += fmt.Sprintf("%s     %s %d (%d%%)\n",
		greenStyle.Render("🟢"), i18n.T("overview_healthy"), ov.Healthy, healthyPct)
	statsContent += fmt.Sprintf("%s     %s %d (%d%%)\n",
		yellowStyle.Render("🟡"), i18n.T("overview_warning"), ov.Warning, warnPct)
	statsContent += fmt.Sprintf("%s     %s %d (%d%%)\n",
		redStyle.Render("🔴"), i18n.T("overview_critical"), ov.Critical, critPct)
	statsContent += fmt.Sprintf("%s     %d\n", orangeStyle.Render("⚠️"), len(ov.AnomaliesTop))
	statsContent += fmt.Sprintf("%s  $%.2f", cyanStyle.Render("💰"), ov.TotalCost)

	leftPanel := statsCard.Render(statsContent)

	// Card: By Agent
	agentCard := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color("39")).
		Padding(1, 2).
		Width(35)

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
		if len(display) > 20 {
			display = display[:20]
		}
		agentContent += fmt.Sprintf("  %-20s %3d  $%7.2f\n", display, a.v.Sessions, a.v.Cost)
	}

	midPanel := agentCard.Render(agentContent)

	// Card: By Model
	modelCard := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color("220")).
		Padding(1, 2).
		Width(35)

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
	for i, mdl := range models {
		if i >= 6 {
			break
		}
		display := mdl.k
		if len(display) > 18 {
			display = display[:18]
		}
		modelContent += fmt.Sprintf("  %-18s %3d  $%7.2f\n", display, mdl.v.Sessions, mdl.v.Cost)
	}

	rightPanel := modelCard.Render(modelContent)

	// Card: Anomalies
	anomalyCard := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color("196")).
		Padding(1, 2).
		Width(35)

	anomalyContent := boldStyle.Render(i18n.T("overview_recent_anomalies")) + "\n"
	if len(ov.AnomaliesTop) == 0 {
		anomalyContent += greenStyle.Render("  " + i18n.T("overview_no_anomalies"))
	} else {
		for i, a := range ov.AnomaliesTop {
			if i >= 5 {
				break
			}
			name := a.Session
			if len(name) > 22 {
				name = name[:22]
			}
			anomalyContent += fmt.Sprintf("  ⚠️ %-22s %s\n", name, a.Type)
		}
	}

	bottomPanel := anomalyCard.Render(anomalyContent)

	// Layout
	topRow := lipgloss.JoinHorizontal(lipgloss.Top, leftPanel, "  ", midPanel, "  ", rightPanel)
	result := lipgloss.JoinVertical(lipgloss.Left, topRow, "", bottomPanel)

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

func (m *Model) applyFilter() {
	filtered := m.getFilteredSessions()
	m.rebuildTableWithSessions(filtered)
}

// applyTextFilter 应用文本模糊筛选
func (m *Model) applyTextFilter() {
	filtered := engine.FilterSessions(m.sessions, m.filterText)
	m.rebuildTableWithSessions(filtered)
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
		healthRaw := fmt.Sprintf("  %d/100 %s", s.Health, engine.HealthEmoji(s.Health))
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
			s.Name, sourceDisplay,
			fmt.Sprintf("%d", met.AssistantTurns),
			fmt.Sprintf("%d", met.ToolCallsTotal),
			sr,
			fmt.Sprintf("$%.4f", met.CostEstimated),
			healthCol,
		})
	}
	m.table.SetRows(rows)
	m.table.SetCursor(0)
}
