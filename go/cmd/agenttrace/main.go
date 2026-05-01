// Command agenttrace — multi-format AI agent session performance analyzer.
// No args: launch interactive Bubble Tea TUI.
package main

import (
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/luoyuctl/agenttrace/internal/engine"
	"github.com/luoyuctl/agenttrace/internal/tui"
)

func main() {
	// Parse flags
	format := flag.String("f", "text", "Output format: text, json")
	dir := flag.String("d", "", "Directory containing session JSONL files")
	compare := flag.Bool("compare", false, "Compare all sessions")
	model := flag.String("m", "default", "Model for pricing")
	output := flag.String("o", "", "Save report to file")
	latest := flag.Bool("latest", false, "Analyze latest session")
	listModels := flag.Bool("list-models", false, "List models with pricing")
	flag.Parse()

	sessionsDir := *dir
	if sessionsDir == "" {
		home, _ := os.UserHomeDir()
		sessionsDir = filepath.Join(home, ".hermes", "sessions")
	}

	// List models
	if *listModels {
		fmt.Printf("agenttrace v%s — Supported Models\n", engine.Version)
		fmt.Println(strings.Repeat("=", 58))
		fmt.Printf("  %-22s %10s %10s\n", "Model", "Input $/M", "Output $/M")
		fmt.Println("  " + strings.Repeat("-", 44))
		for k, v := range engine.Pricing {
			if k == "default" {
				continue
			}
			fmt.Printf("  %-22s $%8.2f  $%8.2f\n", k, v.Input, v.Output)
		}
		fmt.Println()
		fmt.Printf("  default = claude-sonnet-4 ($%.2f/$%.2f)\n",
			engine.Pricing["default"].Input, engine.Pricing["default"].Output)
		return
	}

	path := flag.Arg(0)
	hasAction := path != "" || *latest || *compare

	if !hasAction {
		// Launch TUI
		m := tui.New(sessionsDir)
		p := tea.NewProgram(m, tea.WithAltScreen())
		if _, err := p.Run(); err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			os.Exit(1)
		}
		return
	}

	// Compare mode
	if *compare {
		files := engine.FindSessionFiles(sessionsDir)
		if len(files) == 0 {
			fmt.Fprintf(os.Stderr, "No session files found in %s\n", sessionsDir)
			os.Exit(1)
		}
		if len(files) > 15 {
			files = files[:15]
		}

		var sessions []engine.Session
		for _, f := range files {
			s, err := engine.LoadSession(f)
			if err != nil {
				continue
			}
			sessions = append(sessions, *s)
		}

		out := engine.ReportCompare(sessions, *model)

		if *output != "" {
			os.MkdirAll(filepath.Dir(*output), 0755)
			os.WriteFile(*output, []byte(out+"\n"), 0644)
			fmt.Fprintf(os.Stderr, "Saved: %s\n", *output)
		}
		fmt.Print(out)
		return
	}

	// Resolve target path
	targetPath := path
	if *latest {
		files := engine.FindSessionFiles(sessionsDir)
		if len(files) == 0 {
			fmt.Fprintf(os.Stderr, "No session files found in %s\n", sessionsDir)
			os.Exit(1)
		}
		// Find latest by mtime
		var latestFile string
		var latestTime int64
		for _, f := range files {
			info, err := os.Stat(f)
			if err != nil {
				continue
			}
			if info.ModTime().Unix() > latestTime {
				latestTime = info.ModTime().Unix()
				latestFile = f
			}
		}
		targetPath = latestFile
	}

	if targetPath == "" {
		files := engine.FindSessionFiles(sessionsDir)
		if len(files) > 0 {
			targetPath = files[0]
		}
	}

	if targetPath == "" {
		fmt.Fprintln(os.Stderr, "No session files found.")
		os.Exit(1)
	}

	// Single session analysis
	s, err := engine.LoadSession(targetPath)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error loading %s: %v\n", targetPath, err)
		os.Exit(1)
	}

	var out string
	if *format == "json" {
		out = engine.ReportJSON(s.Metrics, s.Anomalies, s.Health)
	} else {
		out = engine.ReportText(s.Metrics, s.Anomalies, s.Health)
	}

	if *output != "" {
		os.MkdirAll(filepath.Dir(*output), 0755)
		os.WriteFile(*output, []byte(out+"\n"), 0644)
		fmt.Fprintf(os.Stderr, "Saved: %s\n", *output)
	}
	fmt.Print(out)
}
