// Package tui provides the Bubble Tea interactive terminal UI for agenttrace.
// btop-style modern dashboard with three views: Session List, Detail, Compare.
package tui

import (
	"fmt"

	"github.com/charmbracelet/bubbles/table"
	"github.com/charmbracelet/bubbles/viewport"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/luoyuctl/agenttrace/internal/engine"
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
)

// ── Model ──

type view int

const (
	viewList view = iota
	viewDetail
	viewCompare
)

type Model struct {
	view    view
	sessions []engine.Session
	dir     string

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
		{Title: "SESSION", Width: 28},
		{Title: "TURNS", Width: 7},
		{Title: "TOOLS", Width: 7},
		{Title: "SUCC%", Width: 7},
		{Title: "COST", Width: 10},
		{Title: "HEALTH", Width: 8},
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

		healthCol := fmt.Sprintf("  %d/100 %s", s.Health, engine.HealthEmoji(s.Health))

		rows = append(rows, table.Row{
			s.Name,
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
			fmt.Sprintf("%d", m.AssistantTurns),
			fmt.Sprintf("%d", m.ToolCallsTotal),
			sr,
			failStr,
			fmt.Sprintf("$%.4f", m.CostEstimated),
			tokensStr,
			healthCol,
		})
		compareData = append(compareData, []string{
			s.Name, fmt.Sprintf("%d", m.AssistantTurns),
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
		{Title: "SESSION", Width: 28},
		{Title: "TURNS", Width: 7},
		{Title: "TOOLS", Width: 7},
		{Title: "SUCC%", Width: 7},
		{Title: "FAIL", Width: 6},
		{Title: "COST", Width: 10},
		{Title: "TOKENS", Width: 8},
		{Title: "HEALTH", Width: 10},
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
		healthCol := fmt.Sprintf("  %d/100 %s", s.Health, engine.HealthEmoji(s.Health))
		rows = append(rows, table.Row{
			s.Name,
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
		healthCol := fmt.Sprintf("  %d/100 %s", s.Health, engine.HealthEmoji(s.Health))
		rows = append(rows, table.Row{
			s.Name,
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
	title := titleStyle.Render(fmt.Sprintf("🔍 AGENTTRACE v%s", engine.Version))
	countBadge := lipgloss.NewStyle().
		Background(lipgloss.Color("240")).
		Foreground(lipgloss.Color("229")).
		Padding(0, 1).
		Render(fmt.Sprintf(" %d sessions ", len(m.sessions)))

	header := lipgloss.JoinHorizontal(lipgloss.Top, title, "  ", countBadge)
	header = lipgloss.NewStyle().Padding(0, 0, 1, 0).Render(header)

	// View tabs
	tabs := m.renderTabs()

	// Content
	var content string
	switch m.view {
	case viewList:
		content = baseStyle.Render(m.table.View())
	case viewDetail:
		if m.detailReady {
			content = baseStyle.Render(m.viewport.View())
		} else {
			content = baseStyle.Render(dimStyle.Render(" Select a session and press Enter to see details "))
		}
	case viewCompare:
		content = baseStyle.Render(m.compareTable.View())
	}

	// Help bar
	help := m.renderHelp()

	return lipgloss.JoinVertical(lipgloss.Left, header, tabs, content, help)
}

func (m Model) renderTabs() string {
	tabs := []string{"1 Session List", "2 Detail", "3 Compare"}
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
		keys = "↑↓/kj navigate | Enter detail | ←→ sort | s reverse | r refresh | Tab next | 1/2/3 views | q quit"
	case viewDetail:
		keys = "↑↓/kj/PgUp/PgDn scroll | Esc back | Tab next | q quit"
	case viewCompare:
		keys = "↑↓/kj scroll | Esc back | Tab next | r refresh | q quit"
	}
	return helpStyle.Render(" " + keys + " ")
}
