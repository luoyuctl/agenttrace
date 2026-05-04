package main

import (
	"fmt"
	"sort"
	"strings"

	"github.com/luoyuctl/agenttrace/internal/engine"
	"github.com/luoyuctl/agenttrace/internal/i18n"
)

var commonPricingModels = []string{
	"claude-sonnet-4",
	"claude-opus-4.5",
	"gpt-5.1",
	"gpt-5.1-mini",
	"gpt-4.1",
	"gpt-4.1-mini",
	"gemini-2.5-pro",
	"gemini-2.5-flash",
	"deepseek-chat",
	"deepseek-reasoner",
	"grok-code-fast-1",
}

func renderModelPricingList(version, source string, prices map[string]engine.Price, defaultPrice engine.Price) string {
	names := sortedPricingNames(prices)
	nameWidth := pricingNameWidth(names)
	var b strings.Builder

	fmt.Fprintf(&b, i18n.T("supported_models")+"\n", version)
	fmt.Fprintln(&b, strings.Repeat("=", maxPricingInt(58, nameWidth+28)))
	fmt.Fprintf(&b, "%s: %s\n", i18n.T("pricing_source_label"), source)
	fmt.Fprintf(&b, i18n.T("pricing_catalog_hint")+"\n", len(names))
	fmt.Fprintln(&b)

	fmt.Fprintln(&b, i18n.T("pricing_common_section"))
	writePricingHeader(&b, nameWidth)
	fmt.Fprintf(&b, "  %-*s $%8.2f  $%8.2f\n", nameWidth, "default", defaultPrice.Input, defaultPrice.Output)
	for _, name := range commonPricingModels {
		price, ok := prices[name]
		if !ok {
			continue
		}
		fmt.Fprintf(&b, "  %-*s $%8.2f  $%8.2f\n", nameWidth, name, price.Input, price.Output)
	}
	fmt.Fprintln(&b)

	fmt.Fprintf(&b, i18n.T("pricing_full_catalog_section")+"\n", len(names))
	writePricingHeader(&b, nameWidth)
	for _, name := range names {
		price := prices[name]
		fmt.Fprintf(&b, "  %-*s $%8.2f  $%8.2f\n", nameWidth, name, price.Input, price.Output)
	}
	fmt.Fprintln(&b)
	return b.String()
}

func sortedPricingNames(prices map[string]engine.Price) []string {
	names := make([]string, 0, len(prices))
	for name := range prices {
		names = append(names, name)
	}
	sort.Strings(names)
	return names
}

func pricingNameWidth(names []string) int {
	width := len(i18n.T("model_header"))
	for _, name := range names {
		if len(name) > width {
			width = len(name)
		}
	}
	if width < 22 {
		return 22
	}
	return width
}

func writePricingHeader(b *strings.Builder, nameWidth int) {
	fmt.Fprintf(b, "  %-*s %10s %10s\n", nameWidth, i18n.T("model_header"), i18n.T("input_per_m"), i18n.T("output_per_m"))
	fmt.Fprintf(b, "  %s\n", strings.Repeat("-", nameWidth+24))
}

func maxPricingInt(a, b int) int {
	if a > b {
		return a
	}
	return b
}
