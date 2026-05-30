use crate::round4;
use anyhow::{anyhow, Context};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

const PRICING_URL: &str =
    "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json";
const CACHE_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);
static PRICING_CATALOG: OnceLock<PricingCatalog> = OnceLock::new();

#[derive(Debug, Clone, Copy, Default)]
pub struct Price {
    pub input: f64,
    pub output: f64,
    pub cw: f64,
    pub cr: f64,
}

#[derive(Debug, Clone)]
pub struct PricingCatalog {
    pub entries: BTreeMap<String, Price>,
    pub source: String,
    pub loaded_at: Option<SystemTime>,
}

#[derive(Debug, Deserialize)]
struct LiteLlmModel {
    #[serde(default, rename = "input_cost_per_token")]
    input_cost: f64,
    #[serde(default, rename = "output_cost_per_token")]
    output_cost: f64,
    #[serde(default, rename = "cache_creation_input_token_cost")]
    cache_write_cost: f64,
    #[serde(default, rename = "cache_read_input_token_cost")]
    cache_read_cost: f64,
    #[serde(default)]
    mode: String,
    #[serde(default, rename = "litellm_provider")]
    provider: String,
}

pub(crate) fn lookup_price(model: &str) -> Price {
    let catalog = pricing_catalog();
    lookup_price_in(model, &catalog.entries)
}

pub(crate) fn has_specific_price(model: &str) -> bool {
    if matches!(model.trim(), "" | "default" | "unknown") {
        return false;
    }
    match_variants(model)
        .into_iter()
        .any(|variant| pricing_catalog().entries.contains_key(&variant))
}

pub fn list_pricing() -> BTreeMap<String, Price> {
    let mut entries = builtin_pricing();
    entries.remove("default");
    let catalog = pricing_catalog();
    for (name, price) in &catalog.entries {
        entries.insert(name.clone(), *price);
    }
    entries
}

pub fn default_price() -> Price {
    builtin_pricing().get("default").copied().unwrap_or(Price {
        input: 3.0,
        output: 15.0,
        cw: 0.0,
        cr: 0.0,
    })
}

pub fn pricing_source() -> String {
    let catalog = pricing_catalog();
    match (catalog.source.as_str(), catalog.loaded_at) {
        ("cache", Some(time)) => format!("LiteLLM (cached {})", format_cache_time(time)),
        ("cache(stale)", Some(time)) => {
            format!("LiteLLM (stale cache {})", format_cache_time(time))
        }
        ("remote", Some(time)) => format!("LiteLLM (fetched {})", format_cache_time(time)),
        _ => "built-in fallback (use --update-pricing for latest)".to_string(),
    }
}

pub fn pricing_cache_path() -> PathBuf {
    user_cache_dir().join("agenttrace").join("pricing.json")
}

pub fn update_pricing() -> anyhow::Result<usize> {
    let raw = ureq::get(PRICING_URL)
        .timeout(Duration::from_secs(30))
        .call()
        .map_err(|err| anyhow!("download failed: {err}"))?
        .into_string()
        .context("read pricing response")?;
    let entries = convert_litellm(raw.as_bytes());
    if entries.is_empty() {
        return Err(anyhow!("no chat models found in downloaded data"));
    }
    let path = pricing_cache_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, raw)?;
    Ok(entries.len())
}

pub fn render_model_pricing_list() -> String {
    let prices = list_pricing();
    let default = default_price();
    let names = prices.keys().cloned().collect::<Vec<_>>();
    let name_width = pricing_name_width(&names);
    let mut out = String::new();
    out.push_str(&format!(
        "agenttrace v{} - Supported Models\n",
        crate::VERSION
    ));
    out.push_str(&format!("{}\n", "=".repeat(58.max(name_width + 28))));
    out.push_str(&format!("Source: {}\n", pricing_source()));
    out.push_str(&format!(
        "{} model prices loaded. Common/default models are shown first; the complete catalog follows.\n\n",
        prices.len()
    ));
    out.push_str("Common/default pricing\n");
    write_pricing_header(&mut out, name_width);
    out.push_str(&format!(
        "  {:<width$} ${:>8.2}  ${:>8.2}\n",
        "default",
        default.input,
        default.output,
        width = name_width
    ));
    for &name in common_pricing_models() {
        if let Some(price) = prices.get(name) {
            out.push_str(&format!(
                "  {:<width$} ${:>8.2}  ${:>8.2}\n",
                name,
                price.input,
                price.output,
                width = name_width
            ));
        }
    }
    out.push('\n');
    out.push_str(&format!("Full pricing catalog ({} models)\n", prices.len()));
    write_pricing_header(&mut out, name_width);
    for (name, price) in prices {
        out.push_str(&format!(
            "  {:<width$} ${:>8.2}  ${:>8.2}\n",
            name,
            price.input,
            price.output,
            width = name_width
        ));
    }
    out.push('\n');
    out
}

pub fn render_test_match() -> String {
    let mut out = format!("Pricing: {}\n\n", pricing_source());
    for model in [
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
    ] {
        let p = lookup_price(model);
        out.push_str(&format!(
            "  {:<50} → in=${:>7.2}/M  out=${:>7.2}/M  cw=${:>6.2}/M  cr=${:>6.2}/M\n",
            model, p.input, p.output, p.cw, p.cr
        ));
    }
    out
}

pub(crate) fn token_cost(
    input: i64,
    output: i64,
    cache_write: i64,
    cache_read: i64,
    model: &str,
) -> f64 {
    let price = lookup_price(model);
    round4(
        input as f64 / 1e6 * price.input
            + output as f64 / 1e6 * price.output
            + cache_write as f64 / 1e6 * price.cw
            + cache_read as f64 / 1e6 * price.cr,
    )
}

fn pricing_catalog() -> &'static PricingCatalog {
    PRICING_CATALOG.get_or_init(|| {
        load_pricing_cache().unwrap_or_else(|| PricingCatalog {
            entries: builtin_pricing(),
            source: "builtin".to_string(),
            loaded_at: None,
        })
    })
}

fn load_pricing_cache() -> Option<PricingCatalog> {
    let path = pricing_cache_path();
    let metadata = path.metadata().ok()?;
    let loaded_at = metadata.modified().ok();
    let raw = std::fs::read(&path).ok()?;
    let entries = convert_litellm(&raw);
    if entries.is_empty() {
        return None;
    }
    let stale = loaded_at
        .and_then(|time| SystemTime::now().duration_since(time).ok())
        .map(|age| age > CACHE_MAX_AGE)
        .unwrap_or(false);
    Some(PricingCatalog {
        entries,
        source: if stale { "cache(stale)" } else { "cache" }.to_string(),
        loaded_at,
    })
}

fn lookup_price_in(model: &str, entries: &BTreeMap<String, Price>) -> Price {
    for variant in match_variants(model) {
        if let Some(price) = entries.get(&variant) {
            return *price;
        }
    }
    let builtin = builtin_pricing();
    for variant in match_variants(model) {
        if let Some(price) = builtin.get(&variant) {
            return *price;
        }
    }
    builtin.get("default").copied().unwrap_or_default()
}

fn convert_litellm(raw: &[u8]) -> BTreeMap<String, Price> {
    let Ok(Value::Object(source)) = serde_json::from_slice::<Value>(raw) else {
        return BTreeMap::new();
    };
    let mut selected: BTreeMap<String, (i32, Price)> = BTreeMap::new();
    for (key, value) in source {
        let Ok(model) = serde_json::from_value::<LiteLlmModel>(value) else {
            continue;
        };
        if model.mode != "chat" || (model.input_cost == 0.0 && model.output_cost == 0.0) {
            continue;
        }
        let normalized = normalize_model(&key);
        if normalized == "default" || normalized == "unknown" {
            continue;
        }
        let price = Price {
            input: model.input_cost * 1e6,
            output: model.output_cost * 1e6,
            cw: model.cache_write_cost * 1e6,
            cr: model.cache_read_cost * 1e6,
        };
        let priority = provider_priority(&model.provider);
        match selected.get(&normalized) {
            Some((existing, _)) if *existing >= priority => {}
            _ => {
                selected.insert(normalized, (priority, price));
            }
        }
    }
    selected
        .into_iter()
        .map(|(name, (_, price))| (name, price))
        .collect()
}

fn provider_priority(provider: &str) -> i32 {
    match provider {
        "anthropic" | "openai" | "deepseek" | "gemini" | "xai" | "mistral" => 10,
        "cohere" => 9,
        "openrouter" => 8,
        "vercel_ai_gateway" => 7,
        "github_copilot" => 6,
        "bedrock_converse"
        | "bedrock"
        | "vertex_ai-anthropic_models"
        | "vertex_ai-language-models"
        | "azure"
        | "azure_ai" => 5,
        _ => 0,
    }
}

fn match_variants(raw: &str) -> Vec<String> {
    let normalized = normalize_model(raw);
    let mut variants = vec![raw.to_string(), normalized.clone()];
    if normalized.matches('-').count() >= 2 {
        let parts = normalized.split('-').collect::<Vec<_>>();
        let last = parts.last().copied().unwrap_or("");
        let minor = last.len() <= 3
            && (last.chars().next().is_some_and(|c| c.is_ascii_digit())
                || matches!(last, "mini" | "nano" | "flash" | "lite" | "pro"));
        if minor {
            variants.push(parts[..parts.len() - 1].join("-"));
            if parts.len() >= 3 {
                variants.push(parts[..parts.len() - 2].join("-"));
            }
        }
    }
    if normalized.contains("deepseek") {
        if normalized.contains("v3") || normalized.contains("chat") {
            variants.push("deepseek-chat".to_string());
            variants.push("deepseek-v3".to_string());
        }
        if normalized.contains("r1") || normalized.contains("reasoner") {
            variants.push("deepseek-reasoner".to_string());
            variants.push("deepseek-r1".to_string());
        }
    }
    variants
}

fn normalize_model(raw: &str) -> String {
    if raw.is_empty() || raw == "unknown" {
        return "default".to_string();
    }
    let mut value = raw.trim().to_ascii_lowercase();
    if let Some((_, candidate)) = value.rsplit_once('/') {
        if !candidate.starts_with('v') && !candidate.starts_with("20") {
            value = candidate.to_string();
        }
    }
    for marker in [".anthropic.", ".google.", ".meta.", ".amazon."] {
        if let Some(idx) = value.find(marker) {
            if idx > 0 {
                value = value[idx + marker.len()..].to_string();
                break;
            }
        }
    }
    value = strip_date_suffix(&value);
    value = strip_version_suffix(&value);
    while value.contains("--") {
        value = value.replace("--", "-");
    }
    let value = value.trim_matches(['-', '.']).to_string();
    if value.is_empty() {
        "default".to_string()
    } else {
        value
    }
}

fn strip_date_suffix(value: &str) -> String {
    for (idx, sep) in value.char_indices().rev() {
        if sep != '-' && sep != '@' {
            continue;
        }
        let suffix = &value[idx + sep.len_utf8()..];
        let digit_count = suffix.chars().take_while(|c| c.is_ascii_digit()).count();
        if digit_count >= 4
            && suffix
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return value[..idx].to_string();
        }
    }
    value.to_string()
}

fn strip_version_suffix(value: &str) -> String {
    for sep in [':', '@'] {
        if let Some(idx) = value.rfind(sep) {
            let suffix = &value[idx + 1..];
            let suffix = suffix.strip_prefix('v').unwrap_or(suffix);
            if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit() || c == '.') {
                return value[..idx].to_string();
            }
        }
    }
    value.to_string()
}

fn builtin_pricing() -> BTreeMap<String, Price> {
    [
        (
            "claude-opus-4.7",
            Price {
                input: 5.0,
                output: 25.0,
                cw: 6.25,
                cr: 0.50,
            },
        ),
        (
            "claude-opus-4.6",
            Price {
                input: 5.0,
                output: 25.0,
                cw: 6.25,
                cr: 0.50,
            },
        ),
        (
            "claude-opus-4-7",
            Price {
                input: 5.0,
                output: 25.0,
                cw: 6.25,
                cr: 0.50,
            },
        ),
        (
            "claude-opus-4-6",
            Price {
                input: 5.0,
                output: 25.0,
                cw: 6.25,
                cr: 0.50,
            },
        ),
        (
            "claude-opus-4.5",
            Price {
                input: 5.0,
                output: 25.0,
                cw: 6.25,
                cr: 0.50,
            },
        ),
        (
            "claude-opus-4",
            Price {
                input: 15.0,
                output: 75.0,
                cw: 18.75,
                cr: 1.50,
            },
        ),
        (
            "claude-sonnet-4.6",
            Price {
                input: 3.0,
                output: 15.0,
                cw: 3.75,
                cr: 0.30,
            },
        ),
        (
            "claude-sonnet-4-6",
            Price {
                input: 3.0,
                output: 15.0,
                cw: 3.75,
                cr: 0.30,
            },
        ),
        (
            "claude-sonnet-4.5",
            Price {
                input: 3.0,
                output: 15.0,
                cw: 3.75,
                cr: 0.30,
            },
        ),
        (
            "claude-sonnet-4-5",
            Price {
                input: 3.0,
                output: 15.0,
                cw: 3.75,
                cr: 0.30,
            },
        ),
        (
            "claude-sonnet-4",
            Price {
                input: 3.0,
                output: 15.0,
                cw: 3.75,
                cr: 0.30,
            },
        ),
        (
            "claude-haiku-4-5",
            Price {
                input: 1.0,
                output: 5.0,
                cw: 1.25,
                cr: 0.10,
            },
        ),
        (
            "claude-haiku-4.5",
            Price {
                input: 1.0,
                output: 5.0,
                cw: 1.25,
                cr: 0.10,
            },
        ),
        (
            "claude-haiku-3.5",
            Price {
                input: 0.80,
                output: 4.0,
                cw: 1.0,
                cr: 0.08,
            },
        ),
        (
            "gemini-3.1-pro-preview",
            Price {
                input: 2.0,
                output: 12.0,
                cw: 0.0,
                cr: 0.20,
            },
        ),
        (
            "gemini-3-flash-preview",
            Price {
                input: 0.5,
                output: 3.0,
                cw: 0.0,
                cr: 0.05,
            },
        ),
        (
            "gemini-2.5-pro",
            Price {
                input: 1.25,
                output: 10.0,
                cw: 0.0,
                cr: 0.0,
            },
        ),
        (
            "gemini-2.5-flash",
            Price {
                input: 0.15,
                output: 0.60,
                cw: 0.0,
                cr: 0.0,
            },
        ),
        (
            "gpt-5.5",
            Price {
                input: 5.0,
                output: 30.0,
                cw: 0.0,
                cr: 0.50,
            },
        ),
        (
            "gpt-5.4",
            Price {
                input: 2.5,
                output: 15.0,
                cw: 0.0,
                cr: 0.25,
            },
        ),
        (
            "pa/gpt-5.4",
            Price {
                input: 0.0,
                output: 0.0,
                cw: 0.0,
                cr: 0.0,
            },
        ),
        (
            "gpt-5.4-mini",
            Price {
                input: 0.75,
                output: 4.5,
                cw: 0.0,
                cr: 0.075,
            },
        ),
        (
            "gpt-5.3-codex",
            Price {
                input: 1.75,
                output: 14.0,
                cw: 0.0,
                cr: 0.175,
            },
        ),
        (
            "gpt-5.2-codex",
            Price {
                input: 1.75,
                output: 14.0,
                cw: 0.0,
                cr: 0.175,
            },
        ),
        (
            "gpt-5.1",
            Price {
                input: 1.25,
                output: 10.0,
                cw: 0.0,
                cr: 0.0,
            },
        ),
        (
            "gpt-5.1-mini",
            Price {
                input: 0.25,
                output: 2.0,
                cw: 0.0,
                cr: 0.0,
            },
        ),
        (
            "gpt-5.1-codex-mini",
            Price {
                input: 0.25,
                output: 2.0,
                cw: 0.0,
                cr: 0.025,
            },
        ),
        (
            "gpt-4.1",
            Price {
                input: 2.0,
                output: 8.0,
                cw: 0.0,
                cr: 0.0,
            },
        ),
        (
            "gpt-4.1-mini",
            Price {
                input: 0.40,
                output: 1.60,
                cw: 0.0,
                cr: 0.0,
            },
        ),
        (
            "gpt-4.1-nano",
            Price {
                input: 0.10,
                output: 0.40,
                cw: 0.0,
                cr: 0.0,
            },
        ),
        (
            "deepseek-v4-pro",
            Price {
                input: 0.435,
                output: 0.87,
                cw: 0.0,
                cr: 0.003625,
            },
        ),
        (
            "deepseek-v4-flash",
            Price {
                input: 0.14,
                output: 0.28,
                cw: 0.0,
                cr: 0.0028,
            },
        ),
        (
            "deepseek-chat",
            Price {
                input: 0.27,
                output: 1.10,
                cw: 0.07,
                cr: 0.014,
            },
        ),
        (
            "deepseek-reasoner",
            Price {
                input: 0.55,
                output: 2.19,
                cw: 0.14,
                cr: 0.028,
            },
        ),
        (
            "glm-5",
            Price {
                input: 1.0,
                output: 3.20,
                cw: 0.0,
                cr: 0.20,
            },
        ),
        (
            "glm-5-turbo",
            Price {
                input: 1.20,
                output: 4.0,
                cw: 0.0,
                cr: 0.24,
            },
        ),
        (
            "glm-5.1",
            Price {
                input: 1.40,
                output: 4.40,
                cw: 0.0,
                cr: 0.26,
            },
        ),
        (
            "kimi-k2.5",
            Price {
                input: 0.60,
                output: 3.0,
                cw: 0.0,
                cr: 0.10,
            },
        ),
        (
            "kimi-k2.6",
            Price {
                input: 0.95,
                output: 4.0,
                cw: 0.0,
                cr: 0.16,
            },
        ),
        (
            "mimo-v2-pro",
            Price {
                input: 0.10,
                output: 0.30,
                cw: 0.0,
                cr: 0.02,
            },
        ),
        (
            "mimo-v2.5-pro",
            Price {
                input: 0.0,
                output: 0.0,
                cw: 0.0,
                cr: 0.0,
            },
        ),
        (
            "minimax-2.5",
            Price {
                input: 0.30,
                output: 2.40,
                cw: 0.375,
                cr: 0.03,
            },
        ),
        (
            "minimax-2.7",
            Price {
                input: 0.30,
                output: 2.40,
                cw: 0.375,
                cr: 0.03,
            },
        ),
        (
            "minimax-2.7-highspeed",
            Price {
                input: 0.30,
                output: 2.40,
                cw: 0.375,
                cr: 0.03,
            },
        ),
        (
            "minimax-m2.5",
            Price {
                input: 0.30,
                output: 1.20,
                cw: 0.375,
                cr: 0.03,
            },
        ),
        (
            "minimax-m2.5-free",
            Price {
                input: 0.30,
                output: 2.40,
                cw: 0.375,
                cr: 0.03,
            },
        ),
        (
            "minimax-m2.7",
            Price {
                input: 0.30,
                output: 2.40,
                cw: 0.375,
                cr: 0.03,
            },
        ),
        (
            "qwen/qwen3.6-plus-04-02:free",
            Price {
                input: 0.325,
                output: 1.95,
                cw: 0.40625,
                cr: 0.0,
            },
        ),
        (
            "qwen3.6-plus",
            Price {
                input: 0.325,
                output: 1.95,
                cw: 0.40625,
                cr: 0.0,
            },
        ),
        (
            "qwen3.5-plus",
            Price {
                input: 0.40,
                output: 2.40,
                cw: 0.0,
                cr: 0.0,
            },
        ),
        (
            "qwen3.6:35b-a3b-coding-nvfp4",
            Price {
                input: 0.0,
                output: 0.0,
                cw: 0.0,
                cr: 0.0,
            },
        ),
        (
            "doubao-seed-2-0-pro",
            Price {
                input: 0.0,
                output: 0.0,
                cw: 0.0,
                cr: 0.0,
            },
        ),
        (
            "stepfun/step-3.5-flash:free",
            Price {
                input: 0.0,
                output: 0.0,
                cw: 0.0,
                cr: 0.0,
            },
        ),
        (
            "grok-code-fast-1",
            Price {
                input: 0.20,
                output: 1.50,
                cw: 0.0,
                cr: 0.02,
            },
        ),
        (
            "x-ai/grok-code-fast-1",
            Price {
                input: 0.20,
                output: 1.50,
                cw: 0.0,
                cr: 0.02,
            },
        ),
        (
            "<synthetic>",
            Price {
                input: 0.0,
                output: 0.0,
                cw: 0.0,
                cr: 0.0,
            },
        ),
        (
            "grok-3",
            Price {
                input: 3.0,
                output: 15.0,
                cw: 0.0,
                cr: 0.0,
            },
        ),
        (
            "default",
            Price {
                input: 3.0,
                output: 15.0,
                cw: 0.0,
                cr: 0.0,
            },
        ),
    ]
    .into_iter()
    .map(|(name, price)| (name.to_string(), price))
    .collect()
}

fn common_pricing_models() -> &'static [&'static str] {
    &[
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
    ]
}

fn pricing_name_width(names: &[String]) -> usize {
    names
        .iter()
        .map(|name| name.len())
        .max()
        .unwrap_or(0)
        .max("Model".len())
        .max(22)
}

fn write_pricing_header(out: &mut String, name_width: usize) {
    out.push_str(&format!(
        "  {:<width$} {:>10} {:>10}\n",
        "Model",
        "Input $/M",
        "Output $/M",
        width = name_width
    ));
    out.push_str(&format!("  {}\n", "-".repeat(name_width + 24)));
}

fn format_cache_time(time: SystemTime) -> String {
    let datetime: chrono::DateTime<chrono::Local> = time.into();
    datetime.format("%Y-%m-%d %H:%M").to_string()
}

fn user_cache_dir() -> PathBuf {
    if cfg!(target_os = "macos") {
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            return home.join("Library").join("Caches");
        }
    }
    if let Some(cache) = std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from) {
        if !cache.as_os_str().is_empty() {
            return cache;
        }
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        return home.join(".cache");
    }
    std::env::temp_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_fallback_includes_go_alias_slice() {
        let prices = builtin_pricing();
        for name in [
            "claude-haiku-3.5",
            "pa/gpt-5.4",
            "gpt-5.1-codex-mini",
            "deepseek-v4-pro",
            "glm-5.1",
            "mimo-v2.5-pro",
            "minimax-2.7-highspeed",
            "qwen/qwen3.6-plus-04-02:free",
            "qwen3.6:35b-a3b-coding-nvfp4",
            "stepfun/step-3.5-flash:free",
            "grok-3",
        ] {
            assert!(
                prices.contains_key(name),
                "missing builtin pricing for {name}"
            );
        }
    }
}
