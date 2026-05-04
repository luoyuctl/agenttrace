// Command agenttrace — multi-format AI agent session performance analyzer.
// No args: launch interactive Bubble Tea TUI.
package main

import (
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/luoyuctl/agenttrace/internal/engine"
	"github.com/luoyuctl/agenttrace/internal/i18n"
	"github.com/luoyuctl/agenttrace/internal/tui"
)

func main() {
	// Parse flags
	format := flag.String("f", "text", "Output format: text, json, markdown, html")
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
	demo := flag.Bool("demo", false, "Use built-in demo sessions")
	doctor := flag.Bool("doctor", false, "Check detected session directories, cache status, and next steps")
	failUnderHealth := flag.Int("fail-under-health", 0, "Exit non-zero when overview average health is below this score")
	failOnCritical := flag.Bool("fail-on-critical", false, "Exit non-zero when overview contains critical sessions")
	maxToolFailRate := flag.Float64("max-tool-fail-rate", -1, "Exit non-zero when overview tool failure rate exceeds this percent")
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
	if *demo {
		demoDir, cleanup, err := writeDemoSessions()
		if err != nil {
			fmt.Fprintf(os.Stderr, i18n.T("cli_error"), err)
			os.Exit(1)
		}
		os.Setenv("AGENTTRACE_SESSION_CACHE_DIR", filepath.Join(demoDir, ".agenttrace-cache"))
		defer cleanup()
		sessionsDir = demoDir
	}

	// List models
	if *listModels {
		fmt.Print(renderModelPricingList(engine.Version, engine.PricingSource(), engine.ListPricing(), engine.LookupPrice("default")))
		return
	}

	path := flag.Arg(0)
	if *doctor {
		out, err := renderDoctorReport(sessionsDir, *demo, *format)
		if err != nil {
			fmt.Fprintf(os.Stderr, i18n.T("cli_error"), err)
			os.Exit(1)
		}
		if *output != "" {
			os.MkdirAll(filepath.Dir(*output), 0755)
			os.WriteFile(*output, []byte(out), 0644)
			fmt.Fprintf(os.Stderr, i18n.T("cli_saved"), *output)
		}
		fmt.Print(out)
		return
	}

	hasAction := path != "" || *latest || *compare || *overview || *wasteFlag

	if !hasAction {
		// Launch TUI
		m := tui.New(sessionsDir)
		p := tea.NewProgram(m, tea.WithAltScreen())
		if _, err := p.Run(); err != nil {
			fmt.Fprint(os.Stderr, tuiLaunchErrorMessage(err, *demo))
			os.Exit(1)
		}
		return
	}

	// Overview mode
	if *overview {
		var sessions []engine.Session
		if sessionsDir == "" {
			sessions = engine.LoadAll("")
		} else {
			files := engine.FindSessionFiles(sessionsDir)
			for _, f := range files {
				s, err := engine.LoadSession(f)
				if err != nil {
					continue
				}
				sessions = append(sessions, *s)
			}
		}
		if len(sessions) == 0 {
			fmt.Fprintf(os.Stderr, i18n.T("no_session_files")+"\n", sessionsDir)
			os.Exit(1)
		}
		ov := engine.ComputeOverview(sessions)
		out := engine.ReportOverview(ov, sessions)
		switch *format {
		case "json":
			out = engine.ReportOverviewJSON(ov, sessions)
		case "markdown", "md":
			out = engine.ReportOverviewMarkdown(ov, sessions)
		case "html":
			out = engine.ReportOverviewHTML(ov, sessions)
		}
		if *output != "" {
			os.MkdirAll(filepath.Dir(*output), 0755)
			os.WriteFile(*output, []byte(out+"\n"), 0644)
			fmt.Fprintf(os.Stderr, i18n.T("cli_saved"), *output)
		}
		fmt.Print(out)
		if failures := evaluateOverviewGate(ov, sessions, overviewGate{
			FailUnderHealth: *failUnderHealth,
			FailOnCritical:  *failOnCritical,
			MaxToolFailRate: *maxToolFailRate,
		}); len(failures) > 0 {
			for _, failure := range failures {
				fmt.Fprintf(os.Stderr, i18n.T("gate_failed"), failure)
			}
			fmt.Fprint(os.Stderr, renderOverviewGateEvidence(ov, sessions, overviewGateEvidenceOptions{
				OutputPath:  *output,
				SessionsDir: sessionsDir,
				Demo:        *demo,
			}))
			os.Exit(2)
		}
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
		targetPath := latestSessionFile(files)
		if targetPath == "" {
			fmt.Fprint(os.Stderr, i18n.T("cli_no_session_files"))
			os.Exit(1)
		}
		s, err := engine.LoadSession(targetPath)
		if err != nil {
			fmt.Fprintf(os.Stderr, i18n.T("cli_error"), err)
			os.Exit(1)
		}
		wr := engine.ComputeWasteReportFromSession(s)
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
		targetPath = latestSessionFile(files)
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

func latestSessionFile(files []string) string {
	type candidate struct {
		path           string
		sessionTime    time.Time
		hasSessionTime bool
		modTime        time.Time
	}

	var latest candidate
	for _, f := range files {
		info, err := os.Stat(f)
		if err != nil {
			continue
		}
		current := candidate{path: f, modTime: info.ModTime()}
		if session, err := engine.LoadSession(f); err == nil && session.Metrics.SessionStart != "" {
			if ts, err := time.Parse(time.RFC3339, session.Metrics.SessionStart); err == nil {
				current.sessionTime = ts
				current.hasSessionTime = true
			}
		}
		if latest.path == "" || newerSessionCandidate(current, latest) {
			latest = current
		}
	}
	return latest.path
}

func newerSessionCandidate(a, b struct {
	path           string
	sessionTime    time.Time
	hasSessionTime bool
	modTime        time.Time
}) bool {
	if a.hasSessionTime != b.hasSessionTime {
		return a.hasSessionTime
	}
	if a.hasSessionTime {
		if !a.sessionTime.Equal(b.sessionTime) {
			return a.sessionTime.After(b.sessionTime)
		}
		return a.path > b.path
	}
	if !a.modTime.Equal(b.modTime) {
		return a.modTime.After(b.modTime)
	}
	return a.path > b.path
}

func tuiLaunchErrorMessage(err error, demo bool) string {
	msg := fmt.Sprintf(i18n.T("cli_error"), err)
	if demo && isTTYOpenError(err) {
		msg += i18n.T("cli_demo_tty_hint")
	}
	return msg
}

func isTTYOpenError(err error) bool {
	if err == nil {
		return false
	}
	msg := strings.ToLower(err.Error())
	return strings.Contains(msg, "/dev/tty") ||
		strings.Contains(msg, "not a terminal") ||
		strings.Contains(msg, "could not open a new tty")
}
