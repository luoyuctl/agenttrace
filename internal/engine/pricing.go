// Package engine — dynamic model pricing backed by LiteLLM community data.
// Runtime download + 24h cache + built-in fallback. Fuzzy model name matching.
package engine

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"time"

	"github.com/luoyuctl/agentwaste/internal/i18n"
)

// ── LiteLLM source ────────────────────────────────────────────

const (
	pricingURL      = "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json"
	pricingCacheDir = "agentwaste"
	pricingCacheFile = "pricing.json"
	cacheMaxAge     = 24 * time.Hour
)

// ── Backward-compat export ────────────────────────────────────

// Pricing is the built-in model pricing fallback (USD per 1M tokens).
// Deprecated: use LookupPrice() which merges dynamic + fallback pricing.
var Pricing = BuiltinPricing

// ── Built-in fallback (updated to current official pricing) ──

var BuiltinPricing = map[string]Price{
	// Anthropic (official API, updated May 2026)
	"claude-opus-4.7":    {5.00, 25.00, 6.25, 0.50},
	"claude-opus-4.6":    {5.00, 25.00, 6.25, 0.50},
	"claude-opus-4.5":    {5.00, 25.00, 6.25, 0.50},
	"claude-opus-4":      {15.00, 75.00, 18.75, 1.50},
	"claude-sonnet-4.6":  {3.00, 15.00, 3.75, 0.30},
	"claude-sonnet-4.5":  {3.00, 15.00, 3.75, 0.30},
	"claude-sonnet-4":    {3.00, 15.00, 3.75, 0.30},
	"claude-haiku-4.5":   {1.00, 5.00, 1.25, 0.10},
	"claude-haiku-3.5":   {0.80, 4.00, 1.00, 0.08},
	// Google Gemini (official API)
	"gemini-2.5-pro":     {1.25, 10.00, 0, 0},
	"gemini-2.5-flash":   {0.15, 0.60, 0, 0},
	// OpenAI (official API)
	"gpt-5.1":            {1.25, 10.00, 0, 0},
	"gpt-5.1-mini":       {0.25, 2.00, 0, 0},
	"gpt-4.1":            {2.00, 8.00, 0, 0},
	"gpt-4.1-mini":       {0.40, 1.60, 0, 0},
	"gpt-4.1-nano":       {0.10, 0.40, 0, 0},
	// DeepSeek (official API, updated)
	"deepseek-chat":      {0.27, 1.10, 0.07, 0.014},
	"deepseek-reasoner":  {0.55, 2.19, 0.14, 0.028},
	// Grok
	"grok-3":             {3.00, 15.00, 0, 0},
	// Default fallback
	"default":            {3.00, 15.00, 0, 0},
}

// ── Dynamic pricing store ─────────────────────────────────────

// pricingStore holds the merged pricing data (dynamic + fallback).
type pricingStore struct {
	entries  map[string]Price // normalized-key → Price
	loadedAt time.Time
	source   string // "builtin", "cache", "remote"
}

var dynamicPricing = &pricingStore{
	entries:  nil, // lazy init
	loadedAt: time.Time{},
	source:   "builtin",
}

// ── Model name normalization ──────────────────────────────────

// Regexps for stripping provider prefixes and date/version suffixes.
var (
	reDateSuffix    = regexp.MustCompile(`[-@]\d{4,}[-\w]*$`) // "-20250514" "@20250929"
	reVersionSuffix  = regexp.MustCompile(`[:@]v?\d+[\.\d]*$`)  // ":v1:0" "@default"
	reDoubleDash     = regexp.MustCompile(`-+`)
)

// normalizeModel reduces a raw model name from any provider to a canonical short form.
func normalizeModel(raw string) string {
	if raw == "" || raw == "unknown" {
		return "default"
	}
	s := strings.ToLower(strings.TrimSpace(raw))

	// Strategy: strip everything before the rightmost segment that looks like a model name.
	// Model names are in the last path segment (after the final "/") in almost all cases.
	// Handle "fireworks/models/nous-hermes-2" → "nous-hermes-2"
	if idx := strings.LastIndex(s, "/"); idx >= 0 {
		candidate := s[idx+1:]
		// Only use the rightmost segment if it's not a version/date tag
		if !strings.HasPrefix(candidate, "v") && !strings.HasPrefix(candidate, "20") {
			s = candidate
		}
	}

	// Strip region prefix from bedrock-style: "us.anthropic.claude-..." → "claude-..."
	for _, mid := range []string{".anthropic.", ".google.", ".meta.", ".amazon."} {
		if idx := strings.Index(s, mid); idx > 0 {
			s = s[idx+len(mid):]
			break
		}
	}

	// Strip date suffix: "-20250514", "@20250929"
	s = reDateSuffix.ReplaceAllString(s, "")

	// Strip version suffixes: ":v1:0", "@default"
	s = reVersionSuffix.ReplaceAllString(s, "")

	// Cleanup
	s = reDoubleDash.ReplaceAllString(s, "-")
	s = strings.Trim(s, "-.")

	if s == "" {
		return "default"
	}
	return s
}

// matchVariants returns a list of progressively fuzzier variant keys to try.
func matchVariants(raw string) []string {
	normalized := normalizeModel(raw)
	variants := []string{raw, normalized}

	// Versioned model: "claude-sonnet-4-5" → also try "claude-sonnet-4"
	// "gpt-5.1-mini" → also try not splitting
	if strings.Count(normalized, "-") >= 2 {
		parts := strings.Split(normalized, "-")
		// Try without the last segment if it looks like a minor version
		last := parts[len(parts)-1]
		if len(last) <= 3 && (last[0] >= '0' && last[0] <= '9' || last == "mini" || last == "nano" || last == "flash" || last == "lite" || last == "pro") {
			base := strings.Join(parts[:len(parts)-1], "-")
			variants = append(variants, base)
			// Also try with only the major: "claude-sonnet"
			if len(parts) >= 3 {
				grandBase := strings.Join(parts[:len(parts)-2], "-")
				variants = append(variants, grandBase)
			}
		}
	}

	// DeepSeek special: "deepseek-chat" is v3, "deepseek-reasoner" is r1
	if strings.Contains(normalized, "deepseek") {
		if strings.Contains(normalized, "v3") || strings.Contains(normalized, "chat") {
			variants = append(variants, "deepseek-chat", "deepseek-v3")
		}
		if strings.Contains(normalized, "r1") || strings.Contains(normalized, "reasoner") {
			variants = append(variants, "deepseek-reasoner", "deepseek-r1")
		}
	}

	return variants
}

// ── Price lookup ──────────────────────────────────────────────

// LookupPrice finds the best matching price for a raw model name.
// Tries: exact match → normalized → fuzzy variants → builtin → default.
func LookupPrice(model string) Price {
	ensurePricingLoaded()

	store := builtinMap()
	if dynamicPricing.entries != nil && len(dynamicPricing.entries) > 0 {
		store = dynamicPricing.entries
	}

	for _, variant := range matchVariants(model) {
		if p, ok := store[variant]; ok {
			return p
		}
	}

	// Try builtin as last resort
	for _, variant := range matchVariants(model) {
		if p, ok := BuiltinPricing[variant]; ok {
			return p
		}
	}

	return BuiltinPricing["default"]
}

// ListPricing returns all pricing entries (for --list-models display).
func ListPricing() map[string]Price {
	ensurePricingLoaded()

	result := make(map[string]Price)

	// Start with builtin as base
	for k, v := range BuiltinPricing {
		if k != "default" {
			result[k] = v
		}
	}

	// Merge dynamic on top
	if dynamicPricing.entries != nil {
		for k, v := range dynamicPricing.entries {
			result[k] = v
		}
	}

	return result
}

// PricingSource returns a human-readable description of active pricing.
func PricingSource() string {
	ensurePricingLoaded()
	switch dynamicPricing.source {
	case "remote":
		return fmt.Sprintf(i18n.T("pricing_litellm_fetched"), dynamicPricing.loadedAt.Format("2006-01-02 15:04"))
	case "cache":
		return fmt.Sprintf(i18n.T("pricing_litellm_cached"), dynamicPricing.loadedAt.Format("2006-01-02 15:04"))
	default:
		return "built-in fallback (use --update-pricing for latest)"
	}
}

func ensurePricingLoaded() {
	if dynamicPricing.entries != nil {
		return
	}
	// Try loading from cache
	if loaded := loadPricingCache(); loaded {
		return
	}
	// Nothing loaded — will rely on builtin
}

func builtinMap() map[string]Price {
	m := make(map[string]Price, len(BuiltinPricing))
	for k, v := range BuiltinPricing {
		m[k] = v
	}
	return m
}

// ── Download & cache ──────────────────────────────────────────

// LiteLLMModel is the raw LiteLLM JSON entry for a chat model.
type LiteLLMModel struct {
	InputCost  float64 `json:"input_cost_per_token"`
	OutputCost float64 `json:"output_cost_per_token"`
	CacheWCost float64 `json:"cache_creation_input_token_cost"`
	CacheRCost float64 `json:"cache_read_input_token_cost"`
	Mode       string  `json:"mode"`
	Provider   string  `json:"litellm_provider"`
}

// UpdatePricing downloads the latest LiteLLM pricing and caches it.
// Returns the number of chat models loaded.
func UpdatePricing() (int, error) {
	data, err := downloadPricing()
	if err != nil {
		return 0, err
	}

	entries := convertLiteLLM(data)
	if len(entries) == 0 {
		return 0, fmt.Errorf("no chat models found in downloaded data")
	}

	if err := savePricingCache(data); err != nil {
		// Non-fatal: cache save failed but pricing is loaded in memory
		fmt.Fprintf(os.Stderr, i18n.T("pricing_save_warning"), err)
	}

	dynamicPricing.entries = entries
	dynamicPricing.loadedAt = time.Now()
	dynamicPricing.source = "remote"

	return len(entries), nil
}

func downloadPricing() ([]byte, error) {
	client := &http.Client{Timeout: 30 * time.Second}
	resp, err := client.Get(pricingURL)
	if err != nil {
		return nil, fmt.Errorf("download failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != 200 {
		return nil, fmt.Errorf("HTTP %d", resp.StatusCode)
	}

	return io.ReadAll(resp.Body)
}

func convertLiteLLM(raw []byte) map[string]Price {
	var source map[string]json.RawMessage
	if err := json.Unmarshal(raw, &source); err != nil {
		return nil
	}

	// Provider priority: official API > gateway > region-specific
	providerPriority := map[string]int{
		"anthropic":         10,
		"openai":            10,
		"deepseek":          10,
		"gemini":            10,
		"xai":               10,
		"mistral":           10,
		"cohere":             9,
		"openrouter":         8,
		"vercel_ai_gateway":  7,
		"github_copilot":     6,
		"bedrock_converse":   5,
		"bedrock":            5,
		"vertex_ai-anthropic_models": 5,
		"vertex_ai-language-models":  5,
		"azure":              5,
		"azure_ai":           5,
	}

	entries := make(map[string]Price, 2000)
	entrySrc := make(map[string]struct {
		priority int
		price    Price
	})

	for key, blob := range source {
		var m LiteLLMModel
		if err := json.Unmarshal(blob, &m); err != nil {
			continue
		}
		if m.Mode != "chat" {
			continue
		}
		if m.InputCost == 0 && m.OutputCost == 0 {
			continue
		}

		normalized := normalizeModel(key)
		if normalized == "default" || normalized == "unknown" {
			continue
		}

		price := Price{
			Input:  m.InputCost * 1e6,
			Output: m.OutputCost * 1e6,
			CW:     m.CacheWCost * 1e6,
			CR:     m.CacheRCost * 1e6,
		}

		priority := providerPriority[m.Provider]
		if existing, ok := entrySrc[normalized]; ok {
			if priority > existing.priority {
				entrySrc[normalized] = struct {
					priority int
					price    Price
				}{priority, price}
			}
		} else {
			entrySrc[normalized] = struct {
				priority int
				price    Price
			}{priority, price}
		}
	}

	for k, v := range entrySrc {
		entries[k] = v.price
	}
	return entries
}

// CachePath returns the file path where pricing cache is stored.
func CachePath() string {
	return cachePath()
}

func cachePath() string {
	dir, err := os.UserCacheDir()
	if err != nil {
		dir = filepath.Join(os.TempDir(), pricingCacheDir)
	} else {
		dir = filepath.Join(dir, pricingCacheDir)
	}
	os.MkdirAll(dir, 0755)
	return filepath.Join(dir, pricingCacheFile)
}

func savePricingCache(data []byte) error {
	return os.WriteFile(cachePath(), data, 0644)
}

func loadPricingCache() bool {
	path := cachePath()
	info, err := os.Stat(path)
	if err != nil {
		return false
	}

	// Check cache age
	if time.Since(info.ModTime()) > cacheMaxAge {
		// Stale but still usable
		dynamicPricing.source = "cache(stale)"
	} else {
		dynamicPricing.source = "cache"
	}

	data, err := os.ReadFile(path)
	if err != nil {
		return false
	}

	entries := convertLiteLLM(data)
	if len(entries) == 0 {
		return false
	}

	dynamicPricing.entries = entries
	dynamicPricing.loadedAt = info.ModTime()
	return true
}


