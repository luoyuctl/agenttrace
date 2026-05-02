package main

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/luoyuctl/agenttrace/internal/engine"
	"github.com/luoyuctl/agenttrace/internal/i18n"
)

type doctorReport struct {
	Version         string            `json:"version"`
	Mode            string            `json:"mode"`
	CachePath       string            `json:"cache_path"`
	CacheEntries    int               `json:"cache_entries"`
	CacheDirs       int               `json:"cache_dirs"`
	CachedValid     int               `json:"cached_valid"`
	SessionFiles    int               `json:"session_files"`
	Directories     []doctorDirReport `json:"directories"`
	Recommendations []string          `json:"recommendations"`
}

type doctorDirReport struct {
	Name   string `json:"name"`
	Path   string `json:"path"`
	Exists bool   `json:"exists"`
	Files  int    `json:"files"`
}

func renderDoctorReport(dir string, demo bool, format string) (string, error) {
	report := buildDoctorReport(dir, demo)
	if format == "json" {
		data, err := json.MarshalIndent(report, "", "  ")
		if err != nil {
			return "", err
		}
		return string(data) + "\n", nil
	}
	return doctorReportText(report), nil
}

func buildDoctorReport(dir string, demo bool) doctorReport {
	cache := engine.LoadSessionCache()
	files := engine.FindSessionFilesCached(dir, cache)
	valid := 0
	for _, file := range files {
		if _, ok := engine.CachedSession(file, cache); ok {
			valid++
		}
	}

	mode := i18n.T("doctor_mode_auto")
	if dir != "" {
		mode = i18n.T("doctor_mode_custom")
	}
	if demo {
		mode = i18n.T("doctor_mode_demo")
	}

	report := doctorReport{
		Version:      engine.Version,
		Mode:         mode,
		CachePath:    engine.SessionCachePath(),
		CacheEntries: len(cache.Entries),
		CacheDirs:    len(cache.Dirs),
		CachedValid:  valid,
		SessionFiles: len(files),
		Directories:  doctorDirectories(dir, files),
	}
	report.Recommendations = doctorRecommendations(report, dir, demo)
	return report
}

func doctorDirectories(dir string, files []string) []doctorDirReport {
	var dirs []doctorDirReport
	if dir != "" {
		abs, err := filepath.Abs(dir)
		if err != nil {
			abs = dir
		}
		return []doctorDirReport{{
			Name:   "custom",
			Path:   abs,
			Exists: dirExists(abs),
			Files:  len(files),
		}}
	}

	countByRoot := make(map[string]int)
	for _, candidate := range engine.KnownSessionDirs() {
		countByRoot[candidate.Path] = 0
	}
	for _, file := range files {
		for root := range countByRoot {
			if strings.HasPrefix(file, root+string(os.PathSeparator)) || file == root {
				countByRoot[root]++
			}
		}
	}
	for _, candidate := range engine.KnownSessionDirs() {
		dirs = append(dirs, doctorDirReport{
			Name:   candidate.Name,
			Path:   candidate.Path,
			Exists: dirExists(candidate.Path),
			Files:  countByRoot[candidate.Path],
		})
	}
	return dirs
}

func doctorRecommendations(report doctorReport, dir string, demo bool) []string {
	if report.SessionFiles == 0 {
		if dir != "" {
			return []string{i18n.T("doctor_next_custom")}
		}
		return []string{i18n.T("doctor_next_demo")}
	}
	recs := []string{i18n.T("doctor_next_ready")}
	if demo {
		recs = append(recs, i18n.T("doctor_next_demo_cache"))
		return recs
	}
	if report.CachedValid == 0 {
		recs = append(recs, i18n.T("doctor_next_cache"))
	}
	return recs
}

func doctorReportText(report doctorReport) string {
	var b strings.Builder
	fmt.Fprintf(&b, "%s\n", i18n.T("doctor_title"))
	fmt.Fprintf(&b, "%s: %s\n", i18n.T("doctor_version"), report.Version)
	fmt.Fprintf(&b, "%s: %s\n", i18n.T("doctor_mode"), report.Mode)
	fmt.Fprintf(&b, "%s: %d\n", i18n.T("doctor_session_files"), report.SessionFiles)
	fmt.Fprintf(&b, "%s: %s\n", i18n.T("doctor_cache"), report.CachePath)
	fmt.Fprintf(&b, "  "+i18n.T("doctor_cache_detail")+"\n", report.CacheEntries, report.CachedValid, report.CacheDirs)
	fmt.Fprintf(&b, "\n%s:\n", i18n.T("doctor_directories"))
	for _, dir := range report.Directories {
		status := i18n.T("doctor_missing")
		if dir.Exists {
			status = i18n.T("doctor_found")
		}
		fmt.Fprintf(&b, "  %-20s %-7s %5d  %s\n", dir.Name, status, dir.Files, dir.Path)
	}
	fmt.Fprintf(&b, "\n%s:\n", i18n.T("doctor_recommendations"))
	for _, rec := range report.Recommendations {
		fmt.Fprintf(&b, "  - %s\n", rec)
	}
	return b.String()
}

func dirExists(path string) bool {
	info, err := os.Stat(path)
	return err == nil && info.IsDir()
}
