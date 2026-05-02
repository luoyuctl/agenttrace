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
	"github.com/luoyuctl/agenttrace/internal/i18n"
	"github.com/luoyuctl/agenttrace/internal/index"
	"github.com/luoyuctl/agenttrace/internal/tui"
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
		fmt.Printf(i18n.T("cli_version"), engine.Version) //nolint:printf
		return
	}

	// Update pricing (before --list-models so it reflects new data)
	if *updatePricing {
		os.Stdout.WriteString(i18n.T("cli_downloading_pricing"))
		n, err := engine.UpdatePricing()
		if err != nil {
			fmt.Fprintf(os.Stderr, i18n.T("cli_error"), err)
			os.Exit(1)
		}
		fmt.Printf(i18n.T("cli_loaded_pricing"), n)
		fmt.Printf(i18n.T("cli_cache_saved"), engine.CachePath())
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
		// Refresh index cache to eliminate startup latency
		index.BuildOrUpdate(sessionsDir)

		// Launch TUI
		m := tui.New(sessionsDir)
		p := tea.NewProgram(m, tea.WithAltScreen())
		if _, err := p.Run(); err != nil {
			fmt.Fprintf(os.Stderr, i18n.T("cli_error"), err)
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
		if *format == "json" {
			out = engine.ReportOverviewJSON(ov, sessions)
		}
		if *output != "" {
			os.MkdirAll(filepath.Dir(*output), 0755)
			os.WriteFile(*output, []byte(out+"\n"), 0644)
			fmt.Fprintf(os.Stderr, i18n.T("cli_saved"), *output)
		}
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
			fmt.Fprintf(os.Stderr, i18n.T("cli_saved"), *output)
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
			fmt.Fprintf(os.Stderr, i18n.T("cli_error"), err)
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
		fmt.Fprint(os.Stderr, i18n.T("cli_no_session_files"))
		os.Exit(1)
	}

	// Single session analysis
	s, err := engine.LoadSession(targetPath)
	if err != nil {
		fmt.Fprintf(os.Stderr, i18n.T("cli_error_loading"), targetPath, err)
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
		fmt.Fprintf(os.Stderr, i18n.T("cli_saved"), *output)
	}
	fmt.Print(out)
}

// resolveDefaultDir returns "" to signal auto-discovery across all known agent directories.
// The engine's LoadAll("") and FindSessionFiles("") will use ScanAllDirs()
// to discover sessions from ~/.hermes, ~/.claude, ~/.codex, ~/.gemini simultaneously.
func resolveDefaultDir() string {
	return ""
}
