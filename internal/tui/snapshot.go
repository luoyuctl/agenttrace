package tui

import (
	"fmt"
	"sort"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/x/ansi"
	"github.com/luoyuctl/agenttrace/internal/engine"
)

type SnapshotOptions struct {
	Dir      string
	View     string
	Command  string
	Keys     string
	Width    int
	Height   int
	Row      int
	NoColor  bool
	MaxFiles int
}

func RenderSnapshot(opts SnapshotOptions) (string, error) {
	if opts.Width <= 0 {
		opts.Width = 120
	}
	if opts.Height <= 0 {
		opts.Height = 36
	}
	viewName := strings.ToLower(strings.TrimSpace(opts.View))
	if viewName == "" {
		viewName = "overview"
	}

	m, err := snapshotModel(opts)
	if err != nil {
		return "", err
	}
	m = resizeSnapshotModel(m, opts.Width, opts.Height)
	for _, command := range splitSnapshotList(opts.Command) {
		m.runCommand(command)
	}
	if opts.Row > 0 && len(m.filteredIndices) > 0 {
		m.table.SetCursor(minInt(opts.Row-1, len(m.filteredIndices)-1))
	}

	if viewName == "all" {
		views := []string{"overview", "list", "detail", "diagnostics", "diff", "help"}
		var sections []string
		for _, name := range views {
			vm := m
			if err := vm.applySnapshotView(name); err != nil {
				return "", err
			}
			rendered := vm.View()
			if opts.NoColor {
				rendered = ansi.Strip(rendered)
			}
			sections = append(sections, fmt.Sprintf("### %s\n%s", name, rendered))
		}
		return strings.Join(sections, "\n\n"), nil
	}

	if err := m.applySnapshotView(viewName); err != nil {
		return "", err
	}
	for _, key := range splitSnapshotList(opts.Keys) {
		next, _ := m.Update(snapshotKeyMsg(key))
		if got, ok := next.(Model); ok {
			m = got
		}
	}
	rendered := m.View()
	if opts.NoColor {
		rendered = ansi.Strip(rendered)
	}
	return rendered, nil
}

func snapshotModel(opts SnapshotOptions) (Model, error) {
	m := New(opts.Dir)
	m.loading = false

	files := engine.FindSessionFiles(opts.Dir)
	if opts.MaxFiles > 0 && len(files) > opts.MaxFiles {
		files = files[:opts.MaxFiles]
	}
	var sessions []engine.Session
	var skipped int
	for _, path := range files {
		s, err := engine.LoadSession(path)
		if err != nil {
			skipped++
			continue
		}
		sessions = append(sessions, *s)
	}
	if len(files) > 0 && len(sessions) == 0 {
		return m, fmt.Errorf("no loadable session files found (%d skipped)", skipped)
	}

	sort.SliceStable(sessions, func(i, j int) bool {
		return sessions[i].Metrics.SessionStart > sessions[j].Metrics.SessionStart
	})
	m.sessions = sessions
	m.overview = engine.ComputeOverview(sessions)
	m.aggStats = engine.ComputeAggregateStats(sessions)
	m.costSummary = engine.ComputeCostSummary(sessions)
	m.rebuildFilteredView()
	return m, nil
}

func resizeSnapshotModel(m Model, width, height int) Model {
	next, _ := m.Update(tea.WindowSizeMsg{Width: width, Height: height})
	if got, ok := next.(Model); ok {
		return got
	}
	return m
}

func (m *Model) applySnapshotView(name string) error {
	m.helpOpen = false
	m.commandActive = false
	m.filterActive = false
	switch name {
	case "overview":
		m.view = viewOverview
	case "list", "sessions":
		m.view = viewList
	case "detail":
		m.view = viewDetail
		m.openDetail()
	case "diagnostics", "diag", "waste":
		m.view = viewDiagnostics
	case "diff":
		m.openDiff()
	case "help", "keymap", "?":
		m.helpOpen = true
	default:
		return fmt.Errorf("unknown TUI view %q", name)
	}
	return nil
}

func splitSnapshotList(input string) []string {
	var out []string
	for _, item := range strings.Split(input, ";") {
		item = strings.TrimSpace(item)
		if item != "" {
			out = append(out, item)
		}
	}
	return out
}

func snapshotKeyMsg(key string) tea.KeyMsg {
	switch strings.ToLower(strings.TrimSpace(key)) {
	case "enter":
		return tea.KeyMsg{Type: tea.KeyEnter}
	case "tab":
		return tea.KeyMsg{Type: tea.KeyTab}
	case "esc", "escape":
		return tea.KeyMsg{Type: tea.KeyEsc}
	case "backspace":
		return tea.KeyMsg{Type: tea.KeyBackspace}
	case "ctrl+r":
		return tea.KeyMsg{Type: tea.KeyCtrlR}
	case "ctrl+c":
		return tea.KeyMsg{Type: tea.KeyCtrlC}
	case "up":
		return tea.KeyMsg{Type: tea.KeyUp}
	case "down":
		return tea.KeyMsg{Type: tea.KeyDown}
	case "left":
		return tea.KeyMsg{Type: tea.KeyLeft}
	case "right":
		return tea.KeyMsg{Type: tea.KeyRight}
	default:
		return tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune(key)}
	}
}
