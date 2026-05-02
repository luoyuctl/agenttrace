package main

import (
	"encoding/json"
	"strings"
	"testing"

	"github.com/luoyuctl/agenttrace/internal/engine"
	"github.com/luoyuctl/agenttrace/internal/i18n"
)

func TestDoctorReportWithDemoSessions(t *testing.T) {
	dir, cleanup, err := writeDemoSessions()
	if err != nil {
		t.Fatal(err)
	}
	defer cleanup()

	out, err := renderDoctorReport(dir, true, "json")
	if err != nil {
		t.Fatal(err)
	}
	var report doctorReport
	if err := json.Unmarshal([]byte(out), &report); err != nil {
		t.Fatal(err)
	}
	if report.Version != engine.Version || report.SessionFiles != 3 || len(report.Directories) != 1 {
		t.Fatalf("unexpected doctor report: %+v", report)
	}
	if !report.Directories[0].Exists || report.Directories[0].Files != 3 {
		t.Fatalf("bad directory diagnosis: %+v", report.Directories[0])
	}
	if len(report.Recommendations) != 2 || report.Recommendations[1] != i18n.T("doctor_next_demo_cache") {
		t.Fatalf("demo doctor should explain temporary cache behavior: %+v", report.Recommendations)
	}
}

func TestDoctorReportChineseText(t *testing.T) {
	prev := i18n.Current
	i18n.SetLang(i18n.ZH)
	t.Cleanup(func() { i18n.SetLang(prev) })

	out, err := renderDoctorReport(t.TempDir(), false, "text")
	if err != nil {
		t.Fatal(err)
	}
	for _, want := range []string{"AGENTTRACE 诊断", "会话文件", "建议"} {
		if !strings.Contains(out, want) {
			t.Fatalf("doctor output missing %q:\n%s", want, out)
		}
	}
}
