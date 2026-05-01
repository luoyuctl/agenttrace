// Command agentwaste — multi-format AI agent session performance analyzer.
// No args: launch interactive Bubble Tea TUI.
package main

import (
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/luoyuctl/agentwaste/internal/engine"
	"github.com/luoyuctl/agentwaste/internal/i18n"
	"github.com/luoyuctl/agentwaste/internal/tui"
)

func main() {
	// Parse flags
	format := flag.String("f", "text", "Output format: text, json")
	dir := flag.String("d", "", "Directory containing session JSONL files")
	compare := flag.Bool("compare", false, "Compare all sessions")
	overview := flag.Bool("overview", false, "Show global overview dashboard")
	model := flag.String("m", "default", "Model for pricing")
	output := flag.String("o", "", "Save report to file")
	latest := flag.Bool("latest", false, "Analyze latest session")
	wasteFlag := flag.Bool("waste", false, "Show waste analysis for latest session")
	listModels := flag.Bool("list-models", false, "List models with pricing")
	updatePricing := flag.Bool("update-pricing", false, "Download latest model pricing from LiteLLM")
	testMatch := flag.Bool("test-match", false, "Test model name fuzzy matching")
	version := flag.Bool("version", false, "Show version")
	lang := flag.String("lang", "en", "Language for report output: en, zh")
	flag.Parse()

	// Set language
	switch strings.ToLower(*lang) {
	case "zh", "zh-cn", "zh_cn", "chinese":
		i18n.Current = i18n.ZH
	default:
		i18n.Current = i18n.EN
	}

	// Version
	if *version {
		fmt.Printf("agentwaste v%s\n", engine.Version)
		return
	}

	// Update pricing (before --list-models so it reflects new data)
	if *updatePricing {
		fmt.Printf("Downloading latest model pricing from LiteLLM...\n")
		n, err := engine.UpdatePricing()
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			os.Exit(1)
		}
		fmt.Printf("Loaded %d model pricings.\n", n)
		fmt.Printf("Cache saved to %s\n", engine.CachePath())
		// Fall through to allow --list-models after update
	}

	// Test fuzzy matching (dev helper)
	if *testMatch {
		tests := []string{
			"claude-sonnet-4-5-20250929",
			"anthropic/claude-sonnet-4-6",
			"vertex_ai/claude-opus-4-5@20251101",
			"us.anthropic.claude-sonnet-4-5-20250929-v1:0",
			"openai/gpt-4.1",
			"gpt-4.1-mini-2025-04-14",
			"deepseek-chat",
			"deepseek/deepseek-v3.2",
			"gemini-2.5-pro",
			"unknown-model-xyz",
		}
		fmt.Printf("Pricing: %s\n\n", engine.PricingSource())
		for _, m := range tests {
			p := engine.LookupPrice(m)
			fmt.Printf("  %-50s → in=$%7.2f/M  out=$%7.2f/M  cw=$%6.2f/M  cr=$%6.2f/M\n",
				m, p.Input, p.Output, p.CW, p.CR)
		}
		return
	}

	sessionsDir := *dir
	if sessionsDir == "" {
		sessionsDir = resolveDefaultDir()
	}

	// List models
	if *listModels {
		fmt.Printf(i18n.T("supported_models")+"\n", engine.Version)
		fmt.Println(strings.Repeat("=", 58))
		fmt.Printf("  %-22s %10s %10s\n", i18n.T("model_header"), i18n.T("input_per_m"), i18n.T("output_per_m"))
		fmt.Println("  " + strings.Repeat("-", 44))
		for k, v := range engine.ListPricing() {
			fmt.Printf("  %-22s $%8.2f  $%8.2f\n", k, v.Input, v.Output)
		}
		fmt.Println()
		fmt.Printf(i18n.T("default_model_label")+"\n",
			engine.LookupPrice("default").Input, engine.LookupPrice("default").Output)
		fmt.Println("  " + engine.PricingSource())
		return
	}

	path := flag.Arg(0)
	hasAction := path != "" || *latest || *compare || *overview || *wasteFlag

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

	// Overview mode
	if *overview {
		files := engine.FindSessionFiles(sessionsDir)
		if len(files) == 0 {
			fmt.Fprintf(os.Stderr, i18n.T("no_session_files")+"\n", sessionsDir)
			os.Exit(1)
		}
		var sessions []engine.Session
		for _, f := range files {
			s, err := engine.LoadSession(f)
			if err != nil {
				continue
			}
			sessions = append(sessions, *s)
		}
		ov := engine.ComputeOverview(sessions)
		out := engine.ReportOverview(ov, sessions)
		fmt.Print(out)
		return
	}

	// Compare mode
	if *compare {
		files := engine.FindSessionFiles(sessionsDir)
		if len(files) == 0 {
			fmt.Fprintf(os.Stderr, i18n.T("no_session_files")+"\n", sessionsDir)
			os.Exit(1)
		}
		if len(files) > 15 {
			fmt.Fprintf(os.Stderr, i18n.T("compare_truncated")+"\n", len(files))
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

		var out string
		if *format == "json" {
			out = engine.ReportCompareJSON(sessions, *model)
		} else {
			out = engine.ReportCompare(sessions, *model)
		}

		if *output != "" {
			os.MkdirAll(filepath.Dir(*output), 0755)
			os.WriteFile(*output, []byte(out+"\n"), 0644)
			fmt.Fprintf(os.Stderr, "Saved: %s\n", *output)
		}
		fmt.Print(out)
		return
	}

	// Waste analysis for latest session
	if *wasteFlag {
		files := engine.FindSessionFiles(sessionsDir)
		if len(files) == 0 {
			fmt.Fprintf(os.Stderr, i18n.T("no_session_files")+"\n", sessionsDir)
			os.Exit(1)
		}
		s, err := engine.LoadSession(files[0])
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			os.Exit(1)
		}
		events, _ := engine.Parse(s.Path)
		loopResult := engine.AnalyzeLoops(events)
		wr := engine.ComputeWasteReport(s.Metrics, events, loopResult)
		fmt.Print(engine.WasteReportText(wr))
		return
	}

	// Resolve target path
	targetPath := path
	if *latest {
		files := engine.FindSessionFiles(sessionsDir)
		if len(files) == 0 {
			fmt.Fprintf(os.Stderr, i18n.T("no_session_files")+"\n", sessionsDir)
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

// resolveDefaultDir scans common agent session directories.
// Returns the first directory that exists and contains session files.
func resolveDefaultDir() string {
	home, err := os.UserHomeDir()
	if err != nil {
		return filepath.Join(".hermes", "sessions")
	}
	candidates := []string{
		filepath.Join(home, ".hermes", "sessions"),
		filepath.Join(home, ".claude", "sessions"),
		filepath.Join(home, ".gemini", "sessions"),
		filepath.Join(home, ".codex", "sessions"),
	}
	for _, dir := range candidates {
		if entries, err := os.ReadDir(dir); err == nil {
			for _, e := range entries {
				if !e.IsDir() {
					name := e.Name()
					if strings.HasSuffix(name, ".jsonl") || strings.HasSuffix(name, ".json") {
						return dir
					}
				}
			}
		}
	}
	return filepath.Join(home, ".hermes", "sessions")
}
