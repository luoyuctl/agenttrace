mod demo;
mod diagnostics;
mod discovery;
mod doctor;
mod history;
mod insights;
mod parser;
mod pricing;
mod reports;
mod search;
mod session_cache;
mod sqlite_sessions;
mod waste;

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub use demo::demo_sessions;
pub use diagnostics::{
    fix_suggestions, inspect_first, loop_waste_percent, predict_cost_anomaly, session_findings,
    ContextUtilization, CostAlert, Diagnostics, FindingEvidence, FixSuggestion, InspectFirst,
    LargeParam, LoopCost, LoopFingerprint, SessionFinding, StuckPattern, ToolLatency, TraceStep,
    UnusedTool,
};

pub use discovery::{
    collect_session_files, discover_session_dirs, find_session_files, known_session_dirs,
    load_sessions_from_dir, load_sessions_with_options, load_sessions_with_progress,
    load_sessions_with_progress_from_cache, load_sessions_with_progress_from_cache_mode,
    KnownSessionDir, LoadOptions, LoadProgress, LoadReport,
};
pub use doctor::{build_doctor_report, render_doctor_report, DoctorDirReport, DoctorReport};
pub use history::{history_path, merge_preserved_history, preserve_derived_history};
pub use insights::{
    compare_session_outcome, data_health, filter_sessions, project_name, session_capability,
    session_matches_time_range, DataHealth, SessionComparison, TimeRange,
};
pub use parser::{parse_file, parse_raw_session};
pub use pricing::{
    pricing_cache_path, pricing_source, render_model_pricing_list, render_test_match,
    update_pricing,
};
pub use reports::{
    add_baseline_comparison, report_compare, report_compare_json, report_compare_with_language,
    report_json, report_json_with_language, report_overview_html, report_overview_json,
    report_overview_json_with_health, report_overview_markdown, report_overview_text, report_text,
    report_text_with_language, BaselineThresholds, ReportLanguage,
};
pub use search::{report_search_json, report_search_text, search_sessions};
pub use session_cache::{
    cached_session, clear_session_cache, load_cached_sessions, load_cached_sessions_from_cache,
    load_session_cache, save_session_cache, session_cache_path, store_session, SessionCache,
};
pub use sqlite_sessions::{load_sqlite_backed_sessions, skip_sqlite_backed_file_dir};
pub use waste::{
    compute_waste_report, render_waste_report, render_waste_report_with_language, WasteReport,
};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Event {
    #[serde(
        default,
        rename = "role",
        alias = "Role",
        deserialize_with = "deserialize_string"
    )]
    pub role: String,
    #[serde(
        default,
        rename = "content",
        alias = "Content",
        deserialize_with = "deserialize_string"
    )]
    pub content: String,
    #[serde(
        default,
        rename = "timestamp",
        alias = "Timestamp",
        deserialize_with = "deserialize_string"
    )]
    pub timestamp: String,
    #[serde(
        default,
        rename = "reasoning",
        alias = "Reasoning",
        deserialize_with = "deserialize_string"
    )]
    pub reasoning: String,
    #[serde(default, rename = "redacted", alias = "Redacted")]
    pub redacted: bool,
    #[serde(
        default,
        rename = "cwd",
        alias = "CWD",
        deserialize_with = "deserialize_string"
    )]
    pub cwd: String,
    #[serde(
        default,
        rename = "tool_calls",
        alias = "ToolCalls",
        deserialize_with = "deserialize_tool_calls"
    )]
    pub tool_calls: Vec<ToolCall>,
    #[serde(
        default,
        rename = "tool_call_id",
        alias = "ToolCallID",
        deserialize_with = "deserialize_string"
    )]
    pub tool_call_id: String,
    #[serde(default, rename = "is_error", alias = "IsError")]
    pub is_error: bool,
    #[serde(default, rename = "Usage")]
    pub usage: BTreeMap<String, i64>,
    #[serde(default, rename = "ModelUsed", deserialize_with = "deserialize_string")]
    pub model_used: String,
    #[serde(
        default,
        rename = "SourceTool",
        deserialize_with = "deserialize_string"
    )]
    pub source_tool: String,
}

#[derive(Debug, Clone, Default)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: String,
}

impl<'de> Deserialize<'de> for ToolCall {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let Some(obj) = value.as_object() else {
            return Ok(Self::default());
        };
        let function = obj.get("function").and_then(Value::as_object);
        Ok(Self {
            id: string_from_value(
                obj.get("id")
                    .or_else(|| obj.get("ID"))
                    .or_else(|| obj.get("call_id")),
            ),
            name: string_from_value(
                obj.get("name")
                    .or_else(|| obj.get("Name"))
                    .or_else(|| function.and_then(|function| function.get("name"))),
            ),
            args: json_value_string(
                obj.get("args")
                    .or_else(|| obj.get("Args"))
                    .or_else(|| obj.get("arguments"))
                    .or_else(|| obj.get("input"))
                    .or_else(|| function.and_then(|function| function.get("arguments"))),
            ),
        })
    }
}

fn deserialize_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(match Option::<Value>::deserialize(deserializer)? {
        Some(Value::String(text)) => text,
        Some(Value::Null) | None => String::new(),
        Some(value) => serde_json::to_string(&value).unwrap_or_default(),
    })
}

fn deserialize_tool_calls<'de, D>(deserializer: D) -> Result<Vec<ToolCall>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?.unwrap_or(Value::Null);
    let Some(items) = value.as_array() else {
        return Ok(Vec::new());
    };
    Ok(items.iter().map(tool_call_from_value).collect())
}

fn tool_call_from_value(value: &Value) -> ToolCall {
    let Some(obj) = value.as_object() else {
        return ToolCall::default();
    };
    let function = obj.get("function").and_then(Value::as_object);
    ToolCall {
        id: string_from_value(
            obj.get("id")
                .or_else(|| obj.get("ID"))
                .or_else(|| obj.get("call_id")),
        ),
        name: string_from_value(
            obj.get("name")
                .or_else(|| obj.get("Name"))
                .or_else(|| function.and_then(|function| function.get("name"))),
        ),
        args: json_value_string(
            obj.get("args")
                .or_else(|| obj.get("Args"))
                .or_else(|| obj.get("arguments"))
                .or_else(|| obj.get("input"))
                .or_else(|| function.and_then(|function| function.get("arguments"))),
        ),
    }
}

fn string_from_value(value: Option<&Value>) -> String {
    value.and_then(Value::as_str).unwrap_or("").to_string()
}

fn json_value_string(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.to_string(),
        Some(Value::Null) | None => String::new(),
        Some(value) => serde_json::to_string(value).unwrap_or_default(),
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Metrics {
    pub events_total: usize,
    pub user_messages: usize,
    pub assistant_turns: usize,
    pub tool_results: usize,
    pub tool_calls_total: usize,
    pub tool_calls_ok: usize,
    pub tool_calls_fail: usize,
    pub tool_usage: BTreeMap<String, usize>,
    pub file_usage: BTreeMap<String, usize>,
    pub tool_arg_usage: BTreeMap<String, usize>,
    pub tool_authority: BTreeMap<String, usize>,
    pub highest_authority: String,
    pub reasoning_blocks: usize,
    pub reasoning_chars: usize,
    pub reasoning_lens: Vec<usize>,
    pub reasoning_redact: usize,
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub tokens_cache_w: i64,
    pub tokens_cache_r: i64,
    #[serde(skip)]
    pub timestamps: Vec<DateTime<Utc>>,
    pub gaps_sec: Vec<f64>,
    pub model_used: String,
    pub source_tool: String,
    pub session_start: String,
    pub session_end: String,
    pub duration_sec: f64,
    pub cost_estimated: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Anomaly {
    #[serde(rename = "type")]
    pub kind: String,
    pub severity: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolWarning {
    pub tool_name: String,
    pub pattern: String,
    pub count: usize,
    pub detail: String,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Session {
    pub name: String,
    pub path: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub cwd: String,
    pub metrics: Metrics,
    pub anomalies: Vec<Anomaly>,
    pub health: i32,
    pub tool_warnings: Vec<ToolWarning>,
    pub diagnostics: Diagnostics,
}

#[derive(Debug, Clone, Default)]
pub struct Overview {
    pub total_sessions: usize,
    pub total_cost: f64,
    pub healthy: usize,
    pub warning: usize,
    pub critical: usize,
    pub by_agent: BTreeMap<String, GroupOverview>,
    pub by_model: BTreeMap<String, GroupOverview>,
    pub by_project: BTreeMap<String, GroupOverview>,
    pub anomalies_top: Vec<AnomalyTop>,
}

#[derive(Debug, Clone, Default)]
pub struct GroupOverview {
    pub sessions: usize,
    pub cost: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnomalyTop {
    pub session: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub age: String,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub name: String,
    pub path: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub cwd: String,
    pub source_tool: String,
    pub model: String,
    pub health: i32,
    pub cost: f64,
    pub tokens: i64,
    pub matches: Vec<String>,
}

pub fn parse_jsonl_session(name: &str, path: &str, raw: &str) -> anyhow::Result<Session> {
    let mut events = Vec::new();
    let mut line_objects = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if line_objects.len() < 20 {
            if let Some(object) = value.as_object() {
                line_objects.push(object.clone());
            }
        }
        let has_event_type = value
            .as_object()
            .is_some_and(|object| object.contains_key("type"));
        let Ok(mut event) = serde_json::from_value::<Event>(value) else {
            continue;
        };
        if event.role.is_empty() && !has_event_type {
            continue;
        }
        if event.source_tool.is_empty() {
            event.source_tool = "generic".to_string();
        }
        events.push(event);
    }
    if events.is_empty() {
        anyhow::bail!("generic: unable to parse as JSONL or JSON array");
    }
    let source_tool = jsonl_source_tool(&line_objects);
    for event in &mut events {
        event.source_tool = source_tool.to_string();
    }
    session_from_events(name, path, events)
}

fn jsonl_source_tool(objects: &[serde_json::Map<String, Value>]) -> &'static str {
    for object in objects {
        let role = object
            .get("role")
            .or_else(|| object.get("Role"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if role == "session_meta" {
            return "hermes_jsonl";
        }
        if matches!(role, "user" | "assistant" | "tool")
            && (object.contains_key("timestamp") || object.contains_key("Timestamp"))
        {
            return "hermes_jsonl";
        }
    }
    "generic"
}

pub fn session_from_events(name: &str, path: &str, events: Vec<Event>) -> anyhow::Result<Session> {
    let mut model = "default".to_string();
    let mut cwd = String::new();
    for event in &events {
        if !event.model_used.is_empty() && event.model_used != "unknown" {
            model = event.model_used.clone();
        }
        if cwd.is_empty() && !event.cwd.is_empty() {
            cwd = event.cwd.clone();
        }
        if model != "default" && !cwd.is_empty() {
            break;
        }
    }
    let metrics = analyze(&events, &model);
    let anomalies = detect_anomalies(&metrics);
    let health = health_score(&anomalies);
    let tool_warnings = validate_tool_warnings(&events);
    let diagnostics = diagnostics::analyze_diagnostics(&events, &metrics);
    Ok(Session {
        name: session_display_name(name, &events),
        path: path.to_string(),
        cwd,
        metrics,
        anomalies,
        health,
        tool_warnings,
        diagnostics,
    })
}

fn session_display_name(fallback: &str, events: &[Event]) -> String {
    events
        .iter()
        .filter(|event| event.role == "user")
        .filter_map(|event| {
            let mut text = event.content.trim();
            if text.starts_with("# AGENTS.md instructions")
                || text.starts_with("<environment_context>")
                || text.starts_with("Another language model started")
            {
                return None;
            }
            if let Some((_, request)) = text.split_once("## My request for Codex:") {
                text = request.trim();
            }
            let title = text.split_whitespace().collect::<Vec<_>>().join(" ");
            (!title.is_empty()).then(|| {
                let mut chars = title.chars();
                let short = chars.by_ref().take(60).collect::<String>();
                if chars.next().is_some() {
                    format!("{short}…")
                } else {
                    short
                }
            })
        })
        .next()
        .unwrap_or_else(|| fallback.to_string())
}

pub fn analyze(events: &[Event], model: &str) -> Metrics {
    let price = pricing::lookup_price(model);
    let mut metrics = Metrics {
        model_used: model.to_string(),
        tool_usage: BTreeMap::new(),
        file_usage: BTreeMap::new(),
        tool_arg_usage: BTreeMap::new(),
        tool_authority: BTreeMap::new(),
        ..Metrics::default()
    };
    let mut has_meta_usage = false;

    for event in events {
        if metrics.source_tool.is_empty()
            && !event.source_tool.is_empty()
            && event.role != "meta"
            && event.role != "session_meta"
        {
            metrics.source_tool = event.source_tool.clone();
        }

        if let Some(ts) = parse_ts(&event.timestamp) {
            metrics.timestamps.push(ts);
        }

        match event.role.as_str() {
            "session_meta" | "meta" => {
                if !event.usage.is_empty() {
                    metrics.tokens_input += event.usage.get("input_tokens").copied().unwrap_or(0);
                    metrics.tokens_output += event.usage.get("output_tokens").copied().unwrap_or(0);
                    metrics.tokens_cache_w += event
                        .usage
                        .get("cache_creation_input_tokens")
                        .copied()
                        .unwrap_or(0);
                    metrics.tokens_cache_r += event
                        .usage
                        .get("cache_read_input_tokens")
                        .copied()
                        .unwrap_or(0);
                    has_meta_usage = true;
                }
            }
            "user" => {
                metrics.user_messages += 1;
                if !event.content.is_empty() && !has_meta_usage {
                    metrics.tokens_input += std::cmp::max(1, event.content.len() as i64 / 4);
                }
            }
            "assistant" => {
                metrics.assistant_turns += 1;
                if !event.reasoning.is_empty() {
                    metrics.reasoning_blocks += 1;
                    let chars = event.reasoning.len();
                    metrics.reasoning_chars += chars;
                    metrics.reasoning_lens.push(chars);
                    if event.redacted {
                        metrics.reasoning_redact += 1;
                    }
                    if !has_meta_usage {
                        metrics.tokens_output += std::cmp::max(1, chars as i64 / 4);
                    }
                }
                if !event.content.is_empty() && !has_meta_usage {
                    metrics.tokens_output += std::cmp::max(1, event.content.len() as i64 / 4);
                }
                metrics.tool_calls_total += event.tool_calls.len();
                for tool_call in &event.tool_calls {
                    let name = if tool_call.name.is_empty() {
                        "unknown"
                    } else {
                        &tool_call.name
                    };
                    *metrics.tool_usage.entry(name.to_string()).or_insert(0) += 1;
                    for arg in searchable_tool_args(&tool_call.args) {
                        *metrics.tool_arg_usage.entry(arg).or_insert(0) += 1;
                    }
                    let authority = classify_tool_authority(tool_call);
                    *metrics.tool_authority.entry(authority.clone()).or_insert(0) += 1;
                    metrics.highest_authority =
                        higher_tool_authority(&metrics.highest_authority, &authority);
                    for file in extract_tool_call_files(&tool_call.args) {
                        *metrics.file_usage.entry(file).or_insert(0) += 1;
                    }
                }
            }
            "tool" => {
                metrics.tool_results += 1;
                let mut is_error = event.is_error;
                if !is_error && !event.content.is_empty() {
                    if let Ok(Value::Object(obj)) = serde_json::from_str::<Value>(&event.content) {
                        if obj.get("success") == Some(&Value::Bool(false))
                            || obj
                                .get("error")
                                .is_some_and(|value| !matches!(value, Value::Null))
                        {
                            is_error = true;
                        }
                    }
                }
                if is_error {
                    metrics.tool_calls_fail += 1;
                } else {
                    metrics.tool_calls_ok += 1;
                }
            }
            _ => {}
        }
    }

    metrics.events_total = events.len();
    metrics.timestamps.sort();
    if let (Some(first), Some(last)) = (metrics.timestamps.first(), metrics.timestamps.last()) {
        metrics.session_start = first.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        metrics.session_end = last.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        metrics.duration_sec = (*last - *first).num_milliseconds() as f64 / 1000.0;
    }
    for pair in metrics.timestamps.windows(2) {
        let gap = (pair[1] - pair[0]).num_milliseconds() as f64 / 1000.0;
        if gap > 0.0 {
            metrics.gaps_sec.push(gap);
        }
    }
    let max_ok = metrics
        .tool_calls_total
        .saturating_sub(metrics.tool_calls_fail);
    if metrics.tool_calls_ok > max_ok {
        metrics.tool_calls_ok = max_ok;
    }
    metrics.cost_estimated = round4(
        metrics.tokens_input as f64 / 1e6 * price.input
            + metrics.tokens_output as f64 / 1e6 * price.output
            + metrics.tokens_cache_w as f64 / 1e6 * price.cw
            + metrics.tokens_cache_r as f64 / 1e6 * price.cr,
    );
    metrics
}

pub fn detect_anomalies(metrics: &Metrics) -> Vec<Anomaly> {
    let mut anomalies = Vec::new();
    if !metrics.gaps_sec.is_empty() {
        let mut gaps = metrics.gaps_sec.clone();
        gaps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let long_gaps = gaps.iter().filter(|gap| **gap > 60.0).count();
        let max_gap = *gaps.last().unwrap_or(&0.0);
        let has_super_long = gaps.iter().any(|gap| *gap > 300.0);
        if has_super_long {
            anomalies.push(Anomaly {
                kind: "hanging".to_string(),
                severity: "high".to_string(),
                detail: format!("{long_gaps} gap(s) >60s, max={max_gap:.0}s"),
            });
        } else if long_gaps > 0 {
            anomalies.push(Anomaly {
                kind: "hanging".to_string(),
                severity: "medium".to_string(),
                detail: format!("{long_gaps} gap(s) >60s, max={max_gap:.0}s"),
            });
        } else if percentile(&gaps, 0.95) > 30.0 {
            anomalies.push(Anomaly {
                kind: "latency".to_string(),
                severity: "low".to_string(),
                detail: format!("p95 latency = {:.1}s", percentile(&gaps, 0.95)),
            });
        }
    }

    let total_tools = metrics.tool_calls_ok + metrics.tool_calls_fail;
    if total_tools > 0 {
        let fail_rate = metrics.tool_calls_fail as f64 / total_tools as f64;
        if fail_rate > 0.30 {
            anomalies.push(Anomaly {
                kind: "tool_failures".to_string(),
                severity: "high".to_string(),
                detail: format!(
                    "{}/{} failed ({:.0}%)",
                    metrics.tool_calls_fail,
                    total_tools,
                    fail_rate * 100.0
                ),
            });
        } else if fail_rate > 0.15 {
            anomalies.push(Anomaly {
                kind: "tool_failures".to_string(),
                severity: "medium".to_string(),
                detail: format!(
                    "{}/{} failed ({:.0}%)",
                    metrics.tool_calls_fail,
                    total_tools,
                    fail_rate * 100.0
                ),
            });
        }
    }

    if !metrics.reasoning_lens.is_empty() && metrics.reasoning_blocks > 0 {
        let avg_reason = metrics.reasoning_chars as f64 / metrics.reasoning_blocks as f64;
        if avg_reason < 200.0 {
            anomalies.push(Anomaly {
                kind: "shallow_thinking".to_string(),
                severity: "high".to_string(),
                detail: format!("avg reasoning = {avg_reason:.0} chars (very shallow)"),
            });
        } else if avg_reason < 500.0 {
            anomalies.push(Anomaly {
                kind: "shallow_thinking".to_string(),
                severity: "medium".to_string(),
                detail: format!("avg reasoning = {avg_reason:.0} chars"),
            });
        }
    }

    if metrics.reasoning_redact > 0 {
        anomalies.push(Anomaly {
            kind: "redaction".to_string(),
            severity: "medium".to_string(),
            detail: format!("{} block(s) redacted", metrics.reasoning_redact),
        });
    }

    if metrics.tool_calls_total == 0 && metrics.assistant_turns > 2 {
        anomalies.push(Anomaly {
            kind: "no_tools".to_string(),
            severity: "low".to_string(),
            detail: "no tool calls — chat-only session".to_string(),
        });
    }

    anomalies
}

pub fn health_score(anomalies: &[Anomaly]) -> i32 {
    let mut score = 100;
    for anomaly in anomalies {
        match anomaly.severity.as_str() {
            "high" => score -= 30,
            "medium" => score -= 12,
            "low" => score -= 4,
            _ => {}
        }
    }
    score.clamp(0, 100)
}

fn validate_tool_warnings(events: &[Event]) -> Vec<ToolWarning> {
    let mut empty_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut invalid_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut warnings = Vec::new();
    let mut last_key = String::new();
    let mut last_tool = String::new();
    let mut consecutive = 0;
    let mut calls_by_id = BTreeMap::new();
    let mut redundant: BTreeMap<String, (String, usize, BTreeSet<usize>)> = BTreeMap::new();
    let mut turn = 0;
    for event in events {
        if event.role == "user" {
            if consecutive >= 4 {
                warnings.push(tool_warning(&last_tool, "dead_loop", consecutive, "high"));
            }
            last_key.clear();
            last_tool.clear();
            consecutive = 0;
            turn += 1;
        }
        if event.role != "assistant" || event.tool_calls.is_empty() {
            continue;
        }
        for tool_call in &event.tool_calls {
            let name = tool_call.name.trim();
            let args = tool_call.args.trim();
            if !tool_call.id.is_empty() {
                calls_by_id.insert(tool_call.id.clone(), name.to_string());
            }
            let normalized = normalize_tool_args(args);
            let key = format!("{name}\0{normalized}");
            if key == last_key {
                consecutive += 1;
            } else {
                if consecutive >= 4 {
                    warnings.push(tool_warning(&last_tool, "dead_loop", consecutive, "high"));
                }
                last_key = key.clone();
                last_tool = name.to_string();
                consecutive = 1;
            }
            if !normalized.is_empty() {
                let entry = redundant
                    .entry(key)
                    .or_insert_with(|| (name.to_string(), 0, BTreeSet::new()));
                entry.1 += 1;
                entry.2.insert(turn);
            }
            if (args.is_empty() || args == "{}") && tool_requires_args(name) {
                *empty_counts.entry(name.to_string()).or_insert(0) += 1;
                continue;
            }
            if looks_like_structured_args(args) && serde_json::from_str::<Value>(args).is_err() {
                *invalid_counts.entry(name.to_string()).or_insert(0) += 1;
            }
        }
    }
    if consecutive >= 4 {
        warnings.push(tool_warning(&last_tool, "dead_loop", consecutive, "high"));
    }
    for (name, count) in empty_counts {
        warnings.push(tool_warning(&name, "empty_args", count, "medium"));
    }
    for (name, count) in invalid_counts {
        warnings.push(tool_warning(&name, "invalid_args", count, "medium"));
    }
    let mut last_failed = String::new();
    let mut failure_chain = 0;
    for event in events {
        if event.role == "user" {
            if failure_chain >= 3 {
                warnings.push(tool_warning(
                    &last_failed,
                    "fail_retry_chain",
                    failure_chain,
                    "high",
                ));
            }
            last_failed.clear();
            failure_chain = 0;
        } else if event.role == "tool" && event.is_error {
            let name = calls_by_id
                .get(&event.tool_call_id)
                .cloned()
                .unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            if name == last_failed {
                failure_chain += 1;
            } else {
                if failure_chain >= 3 {
                    warnings.push(tool_warning(
                        &last_failed,
                        "fail_retry_chain",
                        failure_chain,
                        "high",
                    ));
                }
                last_failed = name;
                failure_chain = 1;
            }
        }
    }
    if failure_chain >= 3 {
        warnings.push(tool_warning(
            &last_failed,
            "fail_retry_chain",
            failure_chain,
            "high",
        ));
    }
    for (_, (name, count, turns)) in redundant {
        if count >= 4 && turns.len() >= 3 {
            warnings.push(tool_warning(&name, "redundant", count, "low"));
        }
    }
    warnings.sort_by(|left, right| {
        warning_rank(&right.severity)
            .cmp(&warning_rank(&left.severity))
            .then_with(|| left.tool_name.cmp(&right.tool_name))
            .then_with(|| left.pattern.cmp(&right.pattern))
    });
    warnings
}

fn tool_warning(name: &str, pattern: &str, count: usize, severity: &str) -> ToolWarning {
    let detail = match pattern {
        "empty_args" => format!("Tool '{name}' had {count} call(s) with empty arguments"),
        "invalid_args" => format!("Tool '{name}' had {count} call(s) with malformed arguments"),
        "dead_loop" => format!("Tool '{name}' repeated the same call {count} times"),
        "fail_retry_chain" => format!("Tool '{name}' failed {count} consecutive times"),
        "redundant" => format!("Tool '{name}' repeated the same call {count} times across turns"),
        _ => String::new(),
    };
    ToolWarning {
        tool_name: name.to_string(),
        pattern: pattern.to_string(),
        count,
        detail,
        severity: severity.to_string(),
    }
}

fn normalize_tool_args(args: &str) -> String {
    let args = args.trim();
    serde_json::from_str::<Value>(args)
        .ok()
        .and_then(|value| serde_json::to_string(&value).ok())
        .unwrap_or_else(|| args.split_whitespace().collect::<Vec<_>>().join(" "))
}

fn warning_rank(severity: &str) -> u8 {
    match severity {
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
}

fn tool_requires_args(name: &str) -> bool {
    let name = name.trim().to_ascii_lowercase();
    if name.is_empty() {
        return false;
    }
    [
        "apply_patch",
        "bash",
        "click",
        "edit",
        "exec_command",
        "fetch",
        "find",
        "grep",
        "image",
        "open",
        "patch",
        "read",
        "replace",
        "rg",
        "run_command",
        "scrape",
        "search",
        "shell",
        "terminal",
        "view",
        "web",
        "write",
    ]
    .iter()
    .any(|token| name.contains(token))
}

fn looks_like_structured_args(args: &str) -> bool {
    let args = args.trim();
    args.starts_with('{') || args.starts_with('[')
}

pub fn compute_overview(sessions: &[Session]) -> Overview {
    compute_overview_iter(sessions.iter())
}

pub fn compute_overview_iter<'a>(sessions: impl Iterator<Item = &'a Session>) -> Overview {
    let mut overview = Overview::default();
    for session in sessions {
        overview.total_sessions += 1;
        overview.total_cost += session.metrics.cost_estimated;
        if session.health >= 80 {
            overview.healthy += 1;
        } else if session.health >= 50 {
            overview.warning += 1;
        } else {
            overview.critical += 1;
        }
        let agent = session.metrics.source_tool.clone();
        let agent_entry = overview.by_agent.entry(agent).or_default();
        agent_entry.sessions += 1;
        agent_entry.cost += session.metrics.cost_estimated;

        let model = if session.metrics.model_used.is_empty() {
            "unknown".to_string()
        } else {
            session.metrics.model_used.clone()
        };
        let model_entry = overview.by_model.entry(model).or_default();
        model_entry.sessions += 1;
        model_entry.cost += session.metrics.cost_estimated;

        let project_entry = overview
            .by_project
            .entry(project_name(session))
            .or_default();
        project_entry.sessions += 1;
        project_entry.cost += session.metrics.cost_estimated;

        for anomaly in &session.anomalies {
            overview.anomalies_top.push(AnomalyTop {
                session: session.name.clone(),
                kind: anomaly.kind.clone(),
                age: "now".to_string(),
                severity: anomaly.severity.clone(),
            });
        }
    }
    overview.anomalies_top.sort_by(|a, b| {
        severity_rank(&a.severity)
            .cmp(&severity_rank(&b.severity))
            .then_with(|| a.session.cmp(&b.session))
            .then_with(|| a.kind.cmp(&b.kind))
            .then_with(|| a.age.cmp(&b.age))
    });
    overview
}

pub fn canonical_sessions(sessions: &[Session]) -> Vec<Session> {
    let mut out = sessions.to_vec();
    out.sort_by(|a, b| {
        let a_ts = parse_ts(&a.metrics.session_start);
        let b_ts = parse_ts(&b.metrics.session_start);
        match (a_ts, b_ts) {
            (Some(a_ts), Some(b_ts)) if a_ts != b_ts => b_ts.cmp(&a_ts),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name).then_with(|| a.path.cmp(&b.path)),
        }
    });
    out
}

pub fn evaluate_overview_gate(
    overview: &Overview,
    sessions: &[Session],
    fail_under_health: i32,
    fail_on_critical: bool,
    max_tool_fail_rate: Option<f64>,
) -> Vec<String> {
    let mut failures = Vec::new();
    let avg_health = average_health(sessions);
    let tool_fail_rate = tool_fail_rate(sessions);
    if fail_under_health > 0 && avg_health < fail_under_health as f64 {
        failures.push(format!(
            "average health {:.1} is below {}",
            avg_health, fail_under_health
        ));
    }
    if fail_on_critical && overview.critical > 0 {
        failures.push(format!("{} critical sessions found", overview.critical));
    }
    if let Some(max_tool_fail_rate) = max_tool_fail_rate {
        if tool_fail_rate > max_tool_fail_rate {
            failures.push(format!(
                "tool failure rate {:.1}% exceeds {:.1}%",
                tool_fail_rate, max_tool_fail_rate
            ));
        }
    }
    failures
}

pub fn average_health<T: std::borrow::Borrow<Session>>(sessions: &[T]) -> f64 {
    if sessions.is_empty() {
        return 0.0;
    }
    let total: i32 = sessions.iter().map(|session| session.borrow().health).sum();
    total as f64 / sessions.len() as f64
}

pub fn tool_fail_rate(sessions: &[Session]) -> f64 {
    let ok: usize = sessions
        .iter()
        .map(|session| session.metrics.tool_calls_ok)
        .sum();
    let fail: usize = sessions
        .iter()
        .map(|session| session.metrics.tool_calls_fail)
        .sum();
    let total = ok + fail;
    if total == 0 {
        0.0
    } else {
        fail as f64 / total as f64 * 100.0
    }
}

pub fn total_tokens(session: &Session) -> i64 {
    session.metrics.tokens_input
        + session.metrics.tokens_output
        + session.metrics.tokens_cache_w
        + session.metrics.tokens_cache_r
}

pub fn format_tokens(value: i64) -> String {
    let (divisor, suffix) = match value.unsigned_abs() {
        999_950_000_000.. => (1_000_000_000_000.0, "T"),
        999_950_000.. => (1_000_000_000.0, "B"),
        999_950.. => (1_000_000.0, "M"),
        1_000.. => (1_000.0, "K"),
        _ => return value.to_string(),
    };
    format!("{:.1}{suffix}", value as f64 / divisor)
}

pub fn format_count(value: usize) -> String {
    format_tokens(value as i64)
}

pub fn format_cost(value: f64) -> String {
    if !value.is_finite() {
        return "$0.0000".to_string();
    }
    let abs = value.abs();
    if abs >= 999_950_000_000.0 {
        format!("${:.1}T", value / 1_000_000_000_000.0)
    } else if abs >= 999_950_000.0 {
        format!("${:.1}B", value / 1_000_000_000.0)
    } else if abs >= 999_950.0 {
        format!("${:.1}M", value / 1_000_000.0)
    } else if abs >= 999.95 {
        format!("${:.1}K", value / 1_000.0)
    } else if abs >= 10.0 {
        format!("${value:.2}")
    } else {
        format!("${value:.4}")
    }
}

pub fn round4(value: f64) -> f64 {
    let scaled = value * 10000.0;
    let rounded = if scaled.fract().abs() >= 0.5 - 1e-9 {
        scaled.trunc() + scaled.signum()
    } else {
        scaled.trunc()
    };
    rounded / 10000.0
}

pub(crate) use pricing::token_cost;

pub fn fmt_duration(seconds: f64) -> String {
    if seconds < 60.0 {
        format!("{:.0}s", seconds)
    } else if seconds < 3600.0 {
        format!("{:.1}m", seconds / 60.0)
    } else {
        let hours = (seconds / 3600.0) as i64;
        let minutes = ((seconds as i64) % 3600) / 60;
        format!("{hours}h {minutes}m")
    }
}

pub fn highest_authority_for_metrics(metrics: &Metrics) -> String {
    let mut highest = metrics.highest_authority.clone();
    for (authority, count) in &metrics.tool_authority {
        if *count > 0 {
            highest = higher_tool_authority(&highest, authority);
        }
    }
    highest
}

pub fn higher_tool_authority(a: &str, b: &str) -> String {
    if a.is_empty() {
        return b.to_string();
    }
    if b.is_empty() {
        return a.to_string();
    }
    if authority_rank(b) > authority_rank(a) {
        b.to_string()
    } else {
        a.to_string()
    }
}

pub fn is_high_authority_category(category: &str) -> bool {
    authority_rank(category) >= authority_rank("write_files")
}

pub fn sorted_keys<T>(map: &BTreeMap<String, T>) -> Vec<String> {
    map.keys().cloned().collect()
}

pub fn sorted_set(set: BTreeSet<String>) -> Vec<String> {
    set.into_iter().collect()
}

fn parse_ts(value: &str) -> Option<DateTime<Utc>> {
    if value.is_empty() {
        return None;
    }
    let normalized = value.replace('Z', "+00:00");
    if let Ok(ts) = DateTime::parse_from_rfc3339(&normalized) {
        return Some(ts.with_timezone(&Utc));
    }
    NaiveDateTime::parse_from_str(&normalized, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(&normalized, "%Y-%m-%dT%H:%M:%S"))
        .ok()
        .map(|ts| ts.and_utc())
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let mut idx = (sorted.len() as f64 * p) as isize;
    if idx >= sorted.len() as isize {
        idx = sorted.len() as isize - 1;
    }
    if idx < 0 {
        idx = 0;
    }
    sorted[idx as usize]
}

fn searchable_tool_args(args: &str) -> Vec<String> {
    let args = args.trim();
    if args.is_empty() || args.contains('\n') || args.len() > 512 {
        return Vec::new();
    }
    let Ok(value) = serde_json::from_str::<Value>(args) else {
        return vec![args.to_string()];
    };
    let mut seen = BTreeSet::new();
    collect_searchable_tool_args(&value, &mut seen);
    seen.into_iter().collect()
}

fn collect_searchable_tool_args(value: &Value, seen: &mut BTreeSet<String>) {
    match value {
        Value::String(s) => {
            let item = s.trim();
            if !item.is_empty() && !item.contains('\n') && item.len() <= 512 {
                seen.insert(item.to_string());
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_searchable_tool_args(item, seen);
            }
        }
        Value::Object(items) => {
            for item in items.values() {
                collect_searchable_tool_args(item, seen);
            }
        }
        _ => {}
    }
}

pub(crate) fn classify_tool_authority(tool_call: &ToolCall) -> String {
    let name = tool_call.name.trim().to_ascii_lowercase();
    let args = tool_call.args.trim().to_ascii_lowercase();
    let combined = format!("{} {}", name, args).trim().to_string();
    if combined.is_empty() || name == "unknown" {
        return "unknown_authority".to_string();
    }
    if contains_any(
        &combined,
        &[
            "npm publish",
            "pnpm publish",
            "yarn publish",
            "twine upload",
            "docker push",
            "gh release create",
            "goreleaser release",
        ],
    ) {
        "external_publish".to_string()
    } else if contains_any(
        &combined,
        &[
            "git push",
            "git commit",
            "git tag",
            "git merge",
            "git rebase",
            "git cherry-pick",
            "git reset",
            "git checkout",
            "git switch",
        ],
    ) {
        "git_write".to_string()
    } else if contains_any(
        &combined,
        &[
            "npm install",
            "npm i ",
            "pnpm install",
            "pnpm add",
            "yarn add",
            "pip install",
            "uv add",
            "go get",
            "cargo add",
            "brew install",
        ],
    ) {
        "package_install".to_string()
    } else if contains_any(&combined, &["curl ", "wget ", "http://", "https://"])
        || contains_any(&name, &["web_fetch", "web_search", "network"])
    {
        "network_access".to_string()
    } else if contains_any(
        &combined,
        &[
            "go test",
            "go build",
            "npm test",
            "npm run test",
            "pytest",
            "cargo test",
            "cargo build",
            "make test",
            "make build",
            "mvn test",
            "gradle test",
        ],
    ) {
        "test_or_build".to_string()
    } else if contains_any(
        &name,
        &[
            "write",
            "edit",
            "patch",
            "delete",
            "remove",
            "create_file",
            "replace",
        ],
    ) {
        "write_files".to_string()
    } else if contains_any(
        &name,
        &[
            "bash",
            "shell",
            "terminal",
            "exec",
            "run_command",
            "command",
        ],
    ) {
        "shell_exec".to_string()
    } else if contains_any(&name, &["go_test", "test", "build"]) {
        "test_or_build".to_string()
    } else if contains_any(
        &name,
        &["read", "view", "list", "grep", "rg", "find", "cat", "ls"],
    ) {
        "read_only_files".to_string()
    } else {
        "unknown_authority".to_string()
    }
}

fn extract_tool_call_files(args: &str) -> Vec<String> {
    if args.trim().is_empty() {
        return Vec::new();
    }
    let Ok(value) = serde_json::from_str::<Value>(args) else {
        return Vec::new();
    };
    let mut seen = BTreeSet::new();
    collect_tool_call_files(&value, "", &mut seen);
    seen.into_iter().collect()
}

fn collect_tool_call_files(value: &Value, key: &str, seen: &mut BTreeSet<String>) {
    match value {
        Value::Object(items) => {
            for (child_key, child) in items {
                collect_tool_call_files(child, &child_key.to_ascii_lowercase(), seen);
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_tool_call_files(child, key, seen);
            }
        }
        Value::String(text) if is_file_surface_key(key) => {
            if let Some(file) = normalize_tool_call_file(text) {
                seen.insert(file);
            }
        }
        _ => {}
    }
}

fn is_file_surface_key(key: &str) -> bool {
    let key = key.replace('-', "_");
    matches!(
        key.as_str(),
        "path"
            | "file"
            | "files"
            | "filename"
            | "file_name"
            | "filepath"
            | "file_path"
            | "target"
            | "target_file"
            | "uri"
    ) || key.contains("file")
        || key.contains("path")
}

fn normalize_tool_call_file(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty()
        || value.contains('\n')
        || value.starts_with("http://")
        || value.starts_with("https://")
    {
        return None;
    }
    let value = value.strip_prefix("file://").unwrap_or(value);
    let mut parts = Vec::new();
    for component in std::path::Path::new(value).components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                parts.pop();
            }
            std::path::Component::RootDir => parts.clear(),
            std::path::Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            std::path::Component::Prefix(prefix) => {
                parts.push(prefix.as_os_str().to_string_lossy().to_string())
            }
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn authority_rank(category: &str) -> i32 {
    match category {
        "read_only_files" => 1,
        "test_or_build" => 2,
        "write_files" => 3,
        "package_install" => 4,
        "network_access" => 5,
        "shell_exec" => 6,
        "git_write" => 7,
        "external_publish" => 8,
        "unknown_authority" => 9,
        _ => 0,
    }
}

fn severity_rank(severity: &str) -> i32 {
    match severity {
        "high" => 1,
        "medium" => 2,
        "low" => 3,
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_name_uses_first_real_user_request() {
        let session = session_from_events(
            "rollout-2026-07-19-random",
            "session.jsonl",
            vec![
                Event {
                    role: "user".to_string(),
                    content: "# AGENTS.md instructions\n<INSTRUCTIONS>...</INSTRUCTIONS>".to_string(),
                    ..Event::default()
                },
                Event {
                    role: "user".to_string(),
                    content: "# Files mentioned by the user:\n\n## My request for Codex:\n修复全局 Token 展示格式".to_string(),
                    ..Event::default()
                },
            ],
        )
        .expect("build session");

        assert_eq!(session.name, "修复全局 Token 展示格式");
    }

    #[test]
    fn parsers_preserve_codex_and_pi_workspaces() {
        let codex = parse_raw_session(
            "codex",
            "rollout.jsonl",
            r#"{"type":"session_meta","timestamp":"2026-07-19T00:00:00Z","payload":{"cwd":"/work/codex"}}
{"type":"response_item","timestamp":"2026-07-19T00:00:01Z","payload":{"type":"message","role":"user","content":"task"}}"#,
        )
        .expect("parse codex");
        assert_eq!(codex.cwd, "/work/codex");

        let pi = parse_raw_session(
            "pi",
            "/home/user/.pi/agent/sessions/session.jsonl",
            r#"{"type":"session","version":3,"id":"1","cwd":"/work/pi"}
{"type":"message","timestamp":"2026-07-19T00:00:00Z","message":{"role":"user","content":"task"}}"#,
        )
        .expect("parse pi");
        assert_eq!(pi.cwd, "/work/pi");
    }

    #[test]
    fn tool_file_surfaces_follow_go_structured_key_rules() {
        let files = extract_tool_call_files(
            r#"{"path":"./README.md","command":"go test ./...","nested":{"file_path":"file://src/lib.rs"},"url":"https://example.com/file.txt","body":"a\nb"}"#,
        );

        assert_eq!(
            files,
            vec!["README.md".to_string(), "src/lib.rs".to_string()]
        );
    }

    #[test]
    fn searchable_tool_args_match_go_leaf_string_rules() {
        assert_eq!(
            searchable_tool_args(
                r#"{"cmd":"read_file","path":"src/lib.rs","body":"line one\nline two","repeat":"src/lib.rs","nested":[" cargo test "]}"#,
            ),
            vec![
                "cargo test".to_string(),
                "read_file".to_string(),
                "src/lib.rs".to_string()
            ]
        );
        assert_eq!(
            searchable_tool_args("  read_file src/lib.rs  "),
            vec!["read_file src/lib.rs".to_string()]
        );
        assert!(searchable_tool_args("{\"cmd\":\"read_file\"}\n{\"cmd\":\"rg\"}").is_empty());
        assert!(searchable_tool_args(&"x".repeat(513)).is_empty());
    }

    #[test]
    fn percentile_matches_go_index_rule() {
        let sorted = [1.0, 2.0, 3.0, 31.0, 32.0];
        assert_eq!(percentile(&sorted, 0.95), 32.0);
        assert_eq!(percentile(&sorted, 1.0), 32.0);
        assert_eq!(percentile(&sorted, -1.0), 1.0);
    }

    #[test]
    fn naive_iso_timestamps_match_go_utc_gap_rules() {
        let metrics = analyze(
            &[
                Event {
                    role: "user".to_string(),
                    timestamp: "2026-06-21T01:00:00.100000".to_string(),
                    ..Event::default()
                },
                Event {
                    role: "assistant".to_string(),
                    timestamp: "2026-06-21T01:06:01.100000".to_string(),
                    ..Event::default()
                },
            ],
            "default",
        );

        assert_eq!(metrics.session_start, "2026-06-21T01:00:00Z");
        assert_eq!(metrics.session_end, "2026-06-21T01:06:01Z");
        assert_eq!(metrics.gaps_sec, vec![361.0]);
        assert_eq!(detect_anomalies(&metrics)[0].kind, "hanging");
    }

    #[test]
    fn fmt_duration_matches_go_shape() {
        assert_eq!(fmt_duration(40.0), "40s");
        assert_eq!(fmt_duration(90.0), "1.5m");
        assert_eq!(fmt_duration(6180.0), "1h 43m");
    }

    #[test]
    fn token_counts_use_compact_units() {
        assert_eq!(format_tokens(999), "999");
        assert_eq!(format_tokens(1_000), "1.0K");
        assert_eq!(format_tokens(1_250_000), "1.2M");
        assert_eq!(format_tokens(43_719_584_174), "43.7B");
        assert_eq!(format_tokens(2_500_000_000_000), "2.5T");
        assert_eq!(format_tokens(999_950), "1.0M");
        assert_eq!(format_cost(35_240.741_6), "$35.2K");
    }

    #[test]
    fn tool_result_error_null_matches_go_success_rules() {
        let metrics = analyze(
            &[
                Event {
                    role: "assistant".to_string(),
                    tool_calls: vec![
                        ToolCall {
                            name: "ok_null_error".to_string(),
                            ..ToolCall::default()
                        },
                        ToolCall {
                            name: "fail_error_object".to_string(),
                            ..ToolCall::default()
                        },
                        ToolCall {
                            name: "fail_success_false".to_string(),
                            ..ToolCall::default()
                        },
                    ],
                    ..Event::default()
                },
                Event {
                    role: "tool".to_string(),
                    content: r#"{"success":true,"error":null}"#.to_string(),
                    ..Event::default()
                },
                Event {
                    role: "tool".to_string(),
                    content: r#"{"error":{"message":"failed"}}"#.to_string(),
                    ..Event::default()
                },
                Event {
                    role: "tool".to_string(),
                    content: r#"{"success":false,"error":null}"#.to_string(),
                    ..Event::default()
                },
            ],
            "default",
        );

        assert_eq!(metrics.tool_calls_total, 3);
        assert_eq!(metrics.tool_calls_ok, 1);
        assert_eq!(metrics.tool_calls_fail, 2);
    }

    #[test]
    fn detects_dead_loop_failure_chain_and_redundant_tool_calls() {
        let mut events = vec![Event {
            role: "user".to_string(),
            ..Event::default()
        }];
        for turn in 0..4 {
            events.push(Event {
                role: "assistant".to_string(),
                tool_calls: vec![ToolCall {
                    id: turn.to_string(),
                    name: "bash".to_string(),
                    args: r#"{"cmd":"test"}"#.to_string(),
                }],
                ..Event::default()
            });
            events.push(Event {
                role: "tool".to_string(),
                tool_call_id: turn.to_string(),
                is_error: true,
                ..Event::default()
            });
        }
        let warnings = validate_tool_warnings(&events);
        assert!(warnings
            .iter()
            .any(|item| item.pattern == "fail_retry_chain"));

        let redundant = (0..4)
            .flat_map(|index| {
                [
                    Event {
                        role: "user".to_string(),
                        ..Event::default()
                    },
                    Event {
                        role: "assistant".to_string(),
                        tool_calls: vec![ToolCall {
                            id: index.to_string(),
                            name: "bash".to_string(),
                            args: r#"{"cmd":"test"}"#.to_string(),
                        }],
                        ..Event::default()
                    },
                ]
            })
            .collect::<Vec<_>>();
        assert!(validate_tool_warnings(&redundant)
            .iter()
            .any(|item| item.pattern == "redundant"));

        let repeated = (0..4)
            .map(|index| Event {
                role: "assistant".to_string(),
                tool_calls: vec![ToolCall {
                    id: index.to_string(),
                    name: "bash".to_string(),
                    args: "same".to_string(),
                }],
                ..Event::default()
            })
            .collect::<Vec<_>>();
        assert!(validate_tool_warnings(&repeated)
            .iter()
            .any(|item| item.pattern == "dead_loop"));
    }

    #[test]
    fn tool_authority_classification_matches_go_rules() {
        let cases = [
            (
                ToolCall {
                    name: "read_file".to_string(),
                    args: r#"{"path":"README.md"}"#.to_string(),
                    ..ToolCall::default()
                },
                "read_only_files",
            ),
            (
                ToolCall {
                    name: "write_file".to_string(),
                    args: r#"{"path":"README.md"}"#.to_string(),
                    ..ToolCall::default()
                },
                "write_files",
            ),
            (
                ToolCall {
                    name: "terminal".to_string(),
                    args: r#"{"cmd":"go test ./..."}"#.to_string(),
                    ..ToolCall::default()
                },
                "test_or_build",
            ),
            (
                ToolCall {
                    name: "bash".to_string(),
                    args: r#"{"cmd":"npm install"}"#.to_string(),
                    ..ToolCall::default()
                },
                "package_install",
            ),
            (
                ToolCall {
                    name: "terminal".to_string(),
                    args: r#"{"cmd":"sed -n '1,20p' main.go"}"#.to_string(),
                    ..ToolCall::default()
                },
                "shell_exec",
            ),
            (
                ToolCall {
                    name: "bash".to_string(),
                    args: r#"{"cmd":"git push origin HEAD"}"#.to_string(),
                    ..ToolCall::default()
                },
                "git_write",
            ),
            (
                ToolCall {
                    name: "bash".to_string(),
                    args: r#"{"cmd":"curl https://example.com"}"#.to_string(),
                    ..ToolCall::default()
                },
                "network_access",
            ),
            (
                ToolCall {
                    name: "bash".to_string(),
                    args: r#"{"cmd":"npm publish"}"#.to_string(),
                    ..ToolCall::default()
                },
                "external_publish",
            ),
            (
                ToolCall {
                    name: "mystery".to_string(),
                    args: r#"{"value":"x"}"#.to_string(),
                    ..ToolCall::default()
                },
                "unknown_authority",
            ),
        ];

        for (call, want) in cases {
            assert_eq!(classify_tool_authority(&call), want);
        }
        assert_eq!(
            higher_tool_authority("shell_exec", "unknown_authority"),
            "unknown_authority"
        );
        assert!(is_high_authority_category("write_files"));
        assert!(!is_high_authority_category("read_only_files"));
    }

    #[test]
    fn session_capability_and_coverage_degrade_with_available_data() {
        let detailed = Session {
            metrics: Metrics {
                tokens_input: 10,
                duration_sec: 1.0,
                gaps_sec: vec![1.0],
                ..Metrics::default()
            },
            ..test_session()
        };
        let aggregate = Session {
            metrics: Metrics {
                tokens_input: 10,
                duration_sec: 1.0,
                ..Metrics::default()
            },
            ..test_session()
        };
        let limited = test_session();
        assert_eq!(session_capability(&detailed), "detailed");
        assert_eq!(session_capability(&aggregate), "aggregate");
        assert_eq!(session_capability(&limited), "limited");
        let health = data_health(&[detailed, aggregate, limited], 3, 0);
        assert_eq!(health.with_tokens, 2);
        assert_eq!(health.with_duration, 2);
        assert_eq!(health.with_event_timing, 1);
        assert_eq!(health.with_diagnostics, 1);
    }

    #[test]
    fn session_comparison_reuses_shared_outcome_rules() {
        let mut current = test_session();
        current.metrics.duration_sec = 5.0;
        current.metrics.cost_estimated = 1.0;
        current.metrics.tool_calls_fail = 0;
        let mut previous = test_session();
        previous.metrics.duration_sec = 10.0;
        previous.metrics.cost_estimated = 2.0;
        previous.metrics.tool_calls_fail = 2;
        let comparison = compare_session_outcome(&current, &previous);
        assert_eq!(comparison.outcome, "faster_cheaper");
        assert_eq!(comparison.reasons, vec!["fewer_failures"]);
    }

    fn test_session() -> Session {
        Session {
            name: "test".to_string(),
            path: "test".to_string(),
            cwd: String::new(),
            metrics: Metrics::default(),
            anomalies: Vec::new(),
            health: 100,
            tool_warnings: Vec::new(),
            diagnostics: Diagnostics::default(),
        }
    }
}
