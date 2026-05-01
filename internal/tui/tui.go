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
	viewList view = iota
	viewDetail
	viewCompare
)

type Model struct {
	view     view
	sessions []engine.Session
	dir      string
	lang     i18n.Lang

	// List view
	table       table.Model
	tableReady  bool

	// Detail view
	viewport    viewport.Model
	detailReady bool

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
		view:         viewList,
		sessions:     sessions,
		dir:          dir,
		lang:         i18n.EN,
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

		case "esc":
			if m.view == viewDetail || m.view == viewCompare {
				m.view = viewList
			}

		case "tab", "`":
			// Cycle views
			switch m.view {
			case viewList:
				m.view = viewDetail
				m.openDetail()
			case viewDetail:
				m.view = viewCompare
			case viewCompare:
				m.view = viewList
			}

		case "r":
			// Reload
			m.sessions = engine.LoadAll(m.dir)
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

		case "1":
			m.view = viewList
		case "2":
			m.view = viewDetail
			m.openDetail()
		case "3":
			m.view = viewCompare

		default:
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
	case viewList:
		if len(m.sessions) == 0 {
			content = baseStyle.Render(lipgloss.NewStyle().Padding(1).Render(i18n.T("empty_sessions_hint")))
		} else {
			content = baseStyle.Render(m.table.View())
		}
	case viewDetail:
		if m.detailReady {
			scrollInfo := dimStyle.Render(fmt.Sprintf(" Scroll: %.0f%% ", m.viewport.ScrollPercent()*100))
			content = baseStyle.Render(lipgloss.JoinVertical(lipgloss.Left, scrollInfo, m.viewport.View()))
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
	tabs := []string{i18n.T("tab_list"), i18n.T("tab_detail"), i18n.T("tab_compare")}
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
	case viewList:
		keys = i18n.T("help_list") + " | L " + i18n.LangLabel()
	case viewDetail:
		keys = i18n.T("help_detail") + " | L " + i18n.LangLabel()
	case viewCompare:
		keys = i18n.T("help_compare") + " | h sort-health | L " + i18n.LangLabel()
	}
	return helpStyle.Render(" " + keys + " ")
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
