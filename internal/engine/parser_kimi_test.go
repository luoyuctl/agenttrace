package engine

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestParseKimiCLIToolArguments(t *testing.T) {
	path := filepath.Join("..", "..", "testdata", "kimi-tool-args.json")
	if got := DetectFormat(path).Format; got != "kimi_cli" {
		t.Fatalf("kimi tool args format: %s", got)
	}
	events, err := Parse(path)
	if err != nil {
		t.Fatal(err)
	}
	var foundTool bool
	for _, ev := range events {
		for _, tc := range ev.ToolCalls {
			if tc.Name == "read_file" && strings.Contains(tc.Args, "go.mod") {
				foundTool = true
			}
		}
	}
	if !foundTool {
		t.Fatalf("expected Kimi tool args in events: %+v", events)
	}
	m := Analyze(events, "kimi-k2.6")
	if m.SourceTool != "kimi_cli" || m.ToolCallsTotal != 1 || m.ToolCallsOK != 1 || m.ToolUsage["read_file"] != 1 {
		t.Fatalf("bad kimi metrics: %+v", m)
	}
	if m.TokensInput != 120 || m.TokensOutput < 40 {
		t.Fatalf("bad kimi usage: %+v", m)
	}
}

func TestParseKimiCLIMalformed(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "kimi-broken.json")
	raw := `{"model":"kimi-k2.6","messages":[{"role":"assistant","content":[{"type":"unknown"}]}]}`
	if err := os.WriteFile(path, []byte(raw), 0644); err != nil {
		t.Fatal(err)
	}
	if got := DetectFormat(path).Format; got != "kimi_cli" {
		t.Fatalf("kimi malformed format: %s", got)
	}
	if _, err := Parse(path); err == nil {
		t.Fatal("expected malformed kimi session to fail")
	}
}
