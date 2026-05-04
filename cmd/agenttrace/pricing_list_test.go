package main

import (
	"strings"
	"testing"

	"github.com/luoyuctl/agenttrace/internal/engine"
	"github.com/luoyuctl/agenttrace/internal/i18n"
)

func TestRenderModelPricingListPrioritizesCommonModelsBeforeFullCatalog(t *testing.T) {
	prev := i18n.Current
	i18n.SetLang(i18n.EN)
	t.Cleanup(func() { i18n.SetLang(prev) })

	out := renderModelPricingList("test", "LiteLLM (cached now)", map[string]engine.Price{
		"databricks-gpt-oss-120b": {Input: 0.10, Output: 0.20},
		"claude-sonnet-4":         {Input: 3.00, Output: 15.00},
		"aaa-provider-model":      {Input: 0.01, Output: 0.02},
	}, engine.Price{Input: 3.00, Output: 15.00})

	for _, want := range []string{
		"Source: LiteLLM (cached now)",
		"Common/default pricing",
		"Full pricing catalog (3 models)",
		"databricks-gpt-oss-120b",
	} {
		if !strings.Contains(out, want) {
			t.Fatalf("pricing list missing %q:\n%s", want, out)
		}
	}

	commonIdx := strings.Index(out, "Common/default pricing")
	commonModelIdx := strings.Index(out, "claude-sonnet-4")
	fullIdx := strings.Index(out, "Full pricing catalog")
	if commonIdx < 0 || commonModelIdx < commonIdx || commonModelIdx > fullIdx {
		t.Fatalf("common model should appear in the quick-scan section before full catalog:\n%s", out)
	}

	full := out[fullIdx:]
	if strings.Index(full, "aaa-provider-model") > strings.Index(full, "databricks-gpt-oss-120b") {
		t.Fatalf("full catalog should be sorted by model name:\n%s", full)
	}
}

func TestRenderModelPricingListKeepsFallbackDefaultVisible(t *testing.T) {
	prev := i18n.Current
	i18n.SetLang(i18n.EN)
	t.Cleanup(func() { i18n.SetLang(prev) })

	out := renderModelPricingList("test", "built-in fallback (use --update-pricing for latest)", map[string]engine.Price{
		"qwen2p5-coder-32b-instruct-128k": {Input: 0.20, Output: 0.80},
	}, engine.Price{Input: 3.00, Output: 15.00})

	for _, want := range []string{
		"Source: built-in fallback",
		"default",
		"$    3.00",
		"qwen2p5-coder-32b-instruct-128k",
	} {
		if !strings.Contains(out, want) {
			t.Fatalf("fallback pricing list missing %q:\n%s", want, out)
		}
	}
}
