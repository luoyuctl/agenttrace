use crate::{
    average_health, canonical_sessions, classify_tool_authority, context_trends, cost_audit,
    delivery_evidence, fmt_duration, format_cost, format_count, format_tokens,
    highest_authority_for_metrics, is_high_authority_category, mcp_governance, recommendations,
    report_scope, round4, sorted_keys, sorted_set, total_tokens, Anomaly, GroupOverview, Overview,
    Session, ToolCall, VERSION,
};
use chrono::{DateTime, Utc};
use serde_json::{json, Map, Value};
use std::cmp::Ordering;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

#[derive(Debug, Clone, Copy, Default)]
pub struct BaselineThresholds {
    pub max_duration_delta_pct: f64,
    pub max_cost_delta_pct: f64,
    pub max_token_delta_pct: f64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ReportLanguage {
    #[default]
    En,
    Zh,
}

impl ReportLanguage {
    fn t(self, en: &'static str, zh: &'static str) -> &'static str {
        match self {
            Self::En => en,
            Self::Zh => zh,
        }
    }
}

pub fn report_json(session: &Session) -> String {
    report_json_with_language(session, ReportLanguage::En)
}

pub fn report_json_with_language(session: &Session, language: ReportLanguage) -> String {
    let metrics = &session.metrics;
    let total_tokens = total_tokens(session);
    let total_tools = metrics.tool_calls_ok + metrics.tool_calls_fail;
    let avg_reason = if metrics.reasoning_blocks > 0 {
        round4(metrics.reasoning_chars as f64 / metrics.reasoning_blocks as f64)
    } else {
        0.0
    };
    let mut gaps = metrics.gaps_sec.clone();
    gaps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let tool_rate = if total_tools > 0 {
        round4(metrics.tool_calls_ok as f64 / total_tools as f64 * 100.0)
    } else {
        0.0
    };
    let latency_avg = average(&gaps);
    let latency_max = gaps.last().copied().unwrap_or(0.0);
    let latency_median = percentile(&gaps, 0.50);
    let latency_min = gaps.first().copied().unwrap_or(0.0);
    let latency_p95 = percentile(&gaps, 0.95);

    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"activity\": {\n");
    out.push_str(&format!(
        "    \"assistant_turns\": {},\n",
        metrics.assistant_turns
    ));
    out.push_str(&format!(
        "    \"tool_calls_fail\": {},\n",
        metrics.tool_calls_fail
    ));
    out.push_str(&format!(
        "    \"tool_calls_ok\": {},\n",
        metrics.tool_calls_ok
    ));
    out.push_str(&format!(
        "    \"tool_calls_total\": {},\n",
        metrics.tool_calls_total
    ));
    out.push_str(&format!(
        "    \"tool_success_rate\": {},\n",
        json_float(tool_rate)
    ));
    out.push_str(&format!(
        "    \"user_messages\": {}\n",
        metrics.user_messages
    ));
    out.push_str("  },\n");
    out.push_str("  \"anomalies\": ");
    write_anomalies_json(&mut out, &session.anomalies, language, 1);
    out.push_str(",\n");
    out.push_str("  \"cost\": {\n");
    out.push_str(&format!(
        "    \"estimated\": {},\n",
        json_float(metrics.cost_estimated)
    ));
    out.push_str(&format!(
        "    \"model\": {}\n",
        json_string(&metrics.model_used)
    ));
    out.push_str("  },\n");
    out.push_str(&format!("  \"health_score\": {},\n", session.health));
    out.push_str("  \"latency\": {\n");
    out.push_str(&format!("    \"avg\": {},\n", json_float(latency_avg)));
    out.push_str(&format!("    \"max\": {},\n", json_float(latency_max)));
    out.push_str(&format!(
        "    \"median\": {},\n",
        json_float(latency_median)
    ));
    out.push_str(&format!("    \"min\": {},\n", json_float(latency_min)));
    out.push_str(&format!("    \"p95\": {}\n", json_float(latency_p95)));
    out.push_str("  },\n");
    out.push_str(&format!(
        "  \"model_used\": {},\n",
        json_string(&metrics.model_used)
    ));
    out.push_str("  \"reasoning\": {\n");
    out.push_str(&format!("    \"avg_chars\": {},\n", json_float(avg_reason)));
    out.push_str(&format!("    \"blocks\": {},\n", metrics.reasoning_blocks));
    out.push_str(&format!(
        "    \"redacted\": {},\n",
        metrics.reasoning_redact
    ));
    out.push_str(&format!(
        "    \"total_chars\": {}\n",
        metrics.reasoning_chars
    ));
    out.push_str("  },\n");
    out.push_str("  \"session\": {\n");
    out.push_str(&format!(
        "    \"duration_human\": {},\n",
        json_string(&fmt_duration_for_language(metrics.duration_sec, language))
    ));
    out.push_str(&format!(
        "    \"duration_seconds\": {},\n",
        json_float(metrics.duration_sec)
    ));
    out.push_str(&format!(
        "    \"end\": {},\n",
        json_string(&metrics.session_end)
    ));
    out.push_str(&format!(
        "    \"start\": {}\n",
        json_string(&metrics.session_start)
    ));
    out.push_str("  },\n");
    out.push_str(&format!(
        "  \"source_tool\": {},\n",
        json_string(&metrics.source_tool)
    ));
    out.push_str("  \"tokens\": {\n");
    out.push_str(&format!(
        "    \"cache_read\": {},\n",
        metrics.tokens_cache_r
    ));
    out.push_str(&format!(
        "    \"cache_write\": {},\n",
        metrics.tokens_cache_w
    ));
    out.push_str(&format!("    \"input\": {},\n", metrics.tokens_input));
    out.push_str(&format!("    \"output\": {},\n", metrics.tokens_output));
    out.push_str(&format!("    \"total\": {}\n", total_tokens));
    out.push_str("  },\n");
    out.push_str("  \"tool_authority\": {\n");
    out.push_str("    \"counts\": ");
    write_usize_map_json(&mut out, &metrics.tool_authority, 2);
    out.push_str(",\n");
    out.push_str(&format!(
        "    \"highest\": {}\n",
        json_string(&highest_authority_for_metrics(metrics))
    ));
    out.push_str("  },\n");
    out.push_str("  \"tools_top\": ");
    write_usize_map_json(&mut out, &top_tools(&metrics.tool_usage), 1);
    out.push_str(",\n");
    out.push_str(&format!("  \"version\": {}\n", json_string(VERSION)));
    out.push('}');
    out
}

pub fn report_text(session: &Session) -> String {
    report_text_with_language(session, ReportLanguage::En)
}

pub fn report_text_with_language(session: &Session, language: ReportLanguage) -> String {
    let metrics = &session.metrics;
    let total_tokens = total_tokens(session);
    let total_tools = metrics.tool_calls_ok + metrics.tool_calls_fail;
    let success_rate = success_rate(metrics.tool_calls_ok, total_tools);
    let avg_reason = if metrics.reasoning_blocks > 0 {
        metrics.reasoning_chars as f64 / metrics.reasoning_blocks as f64
    } else {
        0.0
    };
    let mut gaps = metrics.gaps_sec.clone();
    gaps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let sep = "━".repeat(60);
    let sub = "─".repeat(40);
    let mut out = String::new();

    out.push_str(&sep);
    out.push('\n');
    out.push_str(&format!(
        "  AGENTTRACE v{} — {}\n",
        VERSION,
        language.t(
            "AI Agent Session Performance Report",
            "AI 智能体会话性能报告"
        )
    ));
    out.push_str(&sep);
    out.push_str("\n\n");

    out.push_str(language.t("💸 MONEY WASTE\n", "💸 成本与 Token\n"));
    out.push_str(&sub);
    out.push('\n');
    out.push_str(&format!(
        "  {}:       {:>10}  {}\n",
        language.t("Input", "输入"),
        format_tokens(metrics.tokens_input),
        language.t("tokens", "Token")
    ));
    out.push_str(&format!(
        "  {}:      {:>10}  {}\n",
        language.t("Output", "输出"),
        format_tokens(metrics.tokens_output),
        language.t("tokens", "Token")
    ));
    if metrics.tokens_cache_w > 0 || metrics.tokens_cache_r > 0 {
        out.push_str(&format!(
            "  {}: {:>10}  {}\n",
            language.t("Cache write", "缓存写入"),
            format_tokens(metrics.tokens_cache_w),
            language.t("tokens", "Token")
        ));
        out.push_str(&format!(
            "  {}:  {:>10}  {}\n",
            language.t("Cache read", "缓存读取"),
            format_tokens(metrics.tokens_cache_r),
            language.t("tokens", "Token")
        ));
    }
    out.push_str("  ────────────────────────────────────\n");
    out.push_str(&format!(
        "  {}: {:>10}\n",
        language.t("Total tokens", "Token 总数"),
        format_tokens(total_tokens)
    ));
    out.push_str(&format!(
        "  {}: {:>12}  ({}: {})\n\n",
        language.t("Estimated cost", "估算成本"),
        format_cost(metrics.cost_estimated),
        language.t("model", "模型"),
        metrics.model_used
    ));

    out.push_str(language.t("📊 ACTIVITY\n", "📊 活动\n"));
    out.push_str(&sub);
    out.push('\n');
    out.push_str(&format!(
        "  {}:    {} {}  |  {} {}\n",
        language.t("Messages", "消息"),
        metrics.user_messages,
        language.t("user", "用户"),
        metrics.assistant_turns,
        language.t("turns", "轮次")
    ));
    out.push_str(&format!(
        "  {}:  {}\n",
        language.t("Tool calls", "工具调用"),
        metrics.tool_calls_total
    ));
    if total_tools > 0 {
        let rate = metrics.tool_calls_ok as f64 / total_tools as f64;
        let success_emoji = if rate < 0.70 {
            "🔴"
        } else if rate < 0.85 {
            "🟡"
        } else {
            "🟢"
        };
        out.push_str(&format!(
            "  {}:     {} ({}/{}) {}\n",
            language.t("Success", "成功率"),
            success_rate,
            metrics.tool_calls_ok,
            total_tools,
            success_emoji
        ));
    }
    out.push('\n');

    out.push_str(language.t("⏱️  LATENCY\n", "⏱️  延迟\n"));
    out.push_str(&sub);
    out.push('\n');
    if gaps.is_empty() {
        out.push_str(language.t("  (no gap data)\n", "  （无间隔数据）\n"));
    } else {
        out.push_str(&format!(
            "  {}:     {:.1}s\n",
            language.t("min", "最小"),
            gaps[0]
        ));
        out.push_str(&format!(
            "  {}:  {:.1}s\n",
            language.t("median", "中位数"),
            percentile(&gaps, 0.50)
        ));
        out.push_str(&format!("  p95:     {:.1}s\n", percentile(&gaps, 0.95)));
        out.push_str(&format!(
            "  {}:     {:.1}s\n",
            language.t("max", "最大"),
            gaps[gaps.len() - 1]
        ));
        out.push_str(&format!(
            "  {}:     {:.1}s\n",
            language.t("avg", "平均"),
            average(&gaps)
        ));
    }
    out.push_str(&format!(
        "  {}: {}\n\n",
        language.t("Duration", "总耗时"),
        fmt_duration_for_language(metrics.duration_sec, language)
    ));

    if !metrics.tool_usage.is_empty() {
        out.push_str(language.t("🔧 TOP TOOLS\n", "🔧 高频工具\n"));
        out.push_str(&sub);
        out.push('\n');
        for (tool, count) in top_tool_rows(&metrics.tool_usage).into_iter().take(8) {
            out.push_str(&format!("  {:<35} {:>4}\n", tool, count));
        }
        out.push('\n');
    }

    out.push_str(language.t("🧠 THINKING / COT\n", "🧠 推理 / 思维链\n"));
    out.push_str(&sub);
    out.push('\n');
    if metrics.reasoning_blocks > 0 {
        let (quality_emoji, quality_label) = if avg_reason < 400.0 {
            ("🔴", language.t("shallow", "浅"))
        } else if avg_reason < 800.0 {
            ("🟡", language.t("moderate", "中等"))
        } else {
            ("🟢", language.t("deep", "深入"))
        };
        out.push_str(&format!(
            "  {}: {}\n",
            language.t("Blocks", "块数"),
            metrics.reasoning_blocks
        ));
        out.push_str(&format!(
            "  {}:    {:.0} {}\n",
            language.t("Avg", "平均"),
            avg_reason,
            language.t("chars", "字符")
        ));
        out.push_str(&format!(
            "  {}:  {} {}\n",
            language.t("Total", "总计"),
            metrics.reasoning_chars,
            language.t("chars", "字符")
        ));
        out.push_str(&format!(
            "  {}: {} {}\n",
            language.t("Quality", "质量"),
            quality_emoji,
            quality_label
        ));
        if metrics.reasoning_redact > 0 {
            out.push_str(&format!(
                "  ⚠️  {} {}\n",
                metrics.reasoning_redact,
                language.t("blocks REDACTED", "个块已脱敏")
            ));
        }
    } else {
        out.push_str(language.t("  (no thinking blocks)\n", "  （无推理块）\n"));
    }
    out.push('\n');

    out.push_str(language.t("🚨 ANOMALIES\n", "🚨 异常\n"));
    out.push_str(&sub);
    out.push('\n');
    if session.anomalies.is_empty() {
        out.push_str(language.t("  ✅ No anomalies detected\n", "  ✅ 未检测到异常\n"));
    } else {
        for anomaly in &session.anomalies {
            out.push_str(&format!(
                "  {} [{}] {}: {}\n",
                anomaly_emoji(&anomaly.severity),
                severity_label_for_language(&anomaly.severity, language),
                anomaly_type_label_for_language(&anomaly.kind, language),
                anomaly_detail_for_language(anomaly, language)
            ));
        }
    }
    out.push('\n');

    out.push_str(language.t("💯 HEALTH SCORE\n", "💯 健康评分\n"));
    out.push_str(&sub);
    out.push('\n');
    out.push_str(&format!(
        "  {}  {}/100  {}\n\n",
        health_emoji(session.health),
        session.health,
        health_bar(session.health)
    ));
    out.push_str(&sep);
    out.push('\n');
    out
}

pub fn report_overview_json(overview: &Overview, sessions: &[Session]) -> String {
    report_overview_json_with_health(overview, sessions, None)
}

pub fn report_overview_json_with_health(
    overview: &Overview,
    sessions: &[Session],
    data_health: Option<&crate::DataHealth>,
) -> String {
    let ordered = canonical_sessions(sessions);
    let summary = overview_summary(overview, &ordered);
    let agents = group_items(&overview.by_agent, true);
    let models = group_items(&overview.by_model, false);
    let projects = group_items(&overview.by_project, false);
    let recent_sessions: Vec<Value> = ordered
        .iter()
        .take(10)
        .map(|session| {
            json!({
                "name": session.name,
                "source_tool": session.metrics.source_tool,
                "model": session.metrics.model_used,
                "cwd": optional_string(&session.cwd),
                "turns": session.metrics.assistant_turns,
                "tools": session.metrics.tool_calls_ok + session.metrics.tool_calls_fail,
                "tokens": total_tokens(session),
                "cost": round4(session.metrics.cost_estimated),
                "health": session.health,
                "anomalies": session.anomalies.len(),
                "highest_tool_authority": highest_authority_for_metrics(&session.metrics),
                "possible_cost_driver": possible_cost_driver_note_strict(session),
            })
        })
        .map(strip_nulls)
        .collect();
    let anomalies: Vec<Value> = overview
        .anomalies_top
        .iter()
        .take(50)
        .map(|item| {
            json!({
                "Session": item.session,
                "Type": item.kind,
                "Age": item.age,
            })
        })
        .collect();
    let mut payload = json!({
        "version": VERSION,
        "summary": summary,
        "failure_families": failure_families(&ordered),
        "surfaces": surfaces(&ordered),
        "by_agent": agents,
        "by_model": models,
        "by_project": projects,
        "recent_sessions": recent_sessions,
        "incident_timelines": incident_timelines(&ordered),
        "anomalies": anomalies,
        "data_health": data_health,
    });
    if let Value::Object(obj) = &mut payload {
        let summary = obj.remove("summary").unwrap_or(Value::Null);
        obj.insert("summary".to_string(), strip_nulls(summary));
    }
    serde_json::to_string_pretty(&payload).expect("overview report serializes")
}

pub fn report_overview_json_with_context(
    overview: &Overview,
    sessions: &[Session],
    data_health: Option<&crate::DataHealth>,
    range: crate::TimeRange,
    includes_preserved_history: bool,
) -> String {
    let base = report_overview_json_with_health(overview, sessions, data_health);
    let mut payload: Value = serde_json::from_str(&base).expect("overview JSON is valid");
    let context = json!({
        "scope": report_scope(sessions, range, includes_preserved_history),
        "cost_audit": cost_audit(sessions),
        "recommendations": recommendations(sessions),
        "mcp_governance": mcp_governance(sessions),
        "context_trends": context_trends(sessions),
        "delivery_evidence": delivery_evidence(sessions),
    });
    if let (Value::Object(payload), Value::Object(context)) = (&mut payload, context) {
        payload.extend(context);
    }
    serde_json::to_string_pretty(&payload).expect("overview context serializes")
}

pub fn report_overview_text_with_context(
    overview: &Overview,
    sessions: &[Session],
    data_health: &crate::DataHealth,
    range: crate::TimeRange,
    includes_preserved_history: bool,
) -> String {
    let scope = report_scope(sessions, range, includes_preserved_history);
    let audit = cost_audit(sessions);
    let mut out = report_overview_text(overview, sessions);
    out.push_str("\n── Scope and confidence ──\n");
    out.push_str(&format!(
        "  Range: {} | sessions: {} | {} to {}\n",
        scope.range, scope.sessions_in_scope, scope.earliest_session_at, scope.latest_session_at
    ));
    out.push_str(&format!(
        "  Parse: {}/{} parsed, {} skipped, {} cache hits | confidence: {}\n",
        data_health.parsed,
        data_health.discovered,
        data_health.skipped,
        data_health.cache_hits,
        data_health.confidence
    ));
    out.push_str(&format!(
        "  Pricing: {} | exact={} fallback={} unknown={}\n",
        audit.pricing_source,
        audit.pricing_coverage.priced_sessions,
        audit.pricing_coverage.fallback_priced_sessions,
        audit.pricing_coverage.unpriced_or_unknown_sessions
    ));
    render_recommendations_text(&mut out, &recommendations(sessions));
    out
}

pub fn report_overview_markdown_with_context(
    overview: &Overview,
    sessions: &[Session],
    data_health: &crate::DataHealth,
    range: crate::TimeRange,
    includes_preserved_history: bool,
) -> String {
    let scope = report_scope(sessions, range, includes_preserved_history);
    let audit = cost_audit(sessions);
    let mut out = report_overview_markdown(overview, sessions);
    out.push_str("\n## Scope and confidence\n\n| Field | Value |\n|---|---|\n");
    out.push_str(&format!("| Range | {} |\n| Session window | {} → {} |\n| Parse coverage | {}/{} parsed; {} skipped; {} cache hits |\n| Confidence | {} |\n| Pricing | {} |\n| Pricing coverage | exact: {}; fallback: {}; unknown: {} |\n", scope.range, scope.earliest_session_at, scope.latest_session_at, data_health.parsed, data_health.discovered, data_health.skipped, data_health.cache_hits, data_health.confidence, markdown_cell(&audit.pricing_source), audit.pricing_coverage.priced_sessions, audit.pricing_coverage.fallback_priced_sessions, audit.pricing_coverage.unpriced_or_unknown_sessions));
    render_recommendations_markdown(&mut out, &recommendations(sessions));
    out
}

pub fn report_overview_html_with_context(
    overview: &Overview,
    sessions: &[Session],
    data_health: &crate::DataHealth,
    range: crate::TimeRange,
    includes_preserved_history: bool,
) -> String {
    let scope = report_scope(sessions, range, includes_preserved_history);
    let audit = cost_audit(sessions);
    let recommendations = recommendations(sessions);
    let mut appendix = String::from("<section><h2>Scope and confidence</h2><table><tbody>");
    appendix.push_str(&format!("<tr><th>Range</th><td>{}</td></tr><tr><th>Session window</th><td>{} → {}</td></tr><tr><th>Parse coverage</th><td>{}/{} parsed; {} skipped; {} cache hits</td></tr><tr><th>Confidence</th><td>{}</td></tr><tr><th>Pricing</th><td>{}</td></tr>", html_escape(&scope.range), html_escape(&scope.earliest_session_at), html_escape(&scope.latest_session_at), data_health.parsed, data_health.discovered, data_health.skipped, data_health.cache_hits, html_escape(&data_health.confidence), html_escape(&audit.pricing_source)));
    appendix.push_str("</tbody></table></section>");
    appendix.push_str("<section><h2>Prioritized recommendations</h2><table><thead><tr><th>Priority</th><th>Finding</th><th>Impact</th><th>Action</th></tr></thead><tbody>");
    for item in recommendations.iter().take(12) {
        appendix.push_str(&format!(
            "<tr><td>{}</td><td>{}: {}</td><td>${:.4}; {} tokens; {}</td><td>{}</td></tr>",
            html_escape(&item.priority),
            html_escape(&item.category),
            html_escape(&item.rationale),
            item.estimated_savings_usd,
            item.estimated_savings_tokens,
            html_escape(&item.confidence),
            html_escape(&item.action)
        ));
    }
    appendix.push_str("</tbody></table></section>");
    report_overview_html(overview, sessions).replacen("</main>", &(appendix + "</main>"), 1)
}

fn render_recommendations_text(out: &mut String, items: &[crate::Recommendation]) {
    if items.is_empty() {
        return;
    }
    out.push_str("\n── Prioritized recommendations ──\n");
    for item in items.iter().take(12) {
        out.push_str(&format!(
            "  [{}] {} — {} | ${:.4}, {} tokens | {}\n    Action: {}\n    Verify: {}\n",
            item.priority,
            item.title,
            item.rationale,
            item.estimated_savings_usd,
            item.estimated_savings_tokens,
            item.confidence,
            item.action,
            item.validation_command
        ));
    }
}

fn render_recommendations_markdown(out: &mut String, items: &[crate::Recommendation]) {
    if items.is_empty() {
        return;
    }
    out.push_str("\n## Prioritized recommendations\n\n| Priority | Finding | Estimated impact | Confidence | Action |\n|---|---|---:|---|---|\n");
    for item in items.iter().take(12) {
        out.push_str(&format!(
            "| {} | {} | ${:.4}; {} tokens | {} | {} |\n",
            item.priority,
            markdown_cell(&item.title),
            item.estimated_savings_usd,
            item.estimated_savings_tokens,
            markdown_cell(&item.confidence),
            markdown_cell(&item.action)
        ));
    }
}

pub fn add_baseline_comparison(
    report_json: &str,
    baseline_path: &str,
    thresholds: BaselineThresholds,
) -> anyhow::Result<String> {
    let mut report: Value = serde_json::from_str(report_json)?;
    let baseline: Value = serde_json::from_str(&fs::read_to_string(baseline_path)?)?;
    let summary = report
        .get("summary")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let base_summary = baseline
        .get("summary")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let duration_delta = delta_pct(
        number(&summary, "total_duration_seconds"),
        number(&base_summary, "total_duration_seconds"),
    );
    let cost_delta = delta_pct(
        number(&summary, "total_cost"),
        number(&base_summary, "total_cost"),
    );
    let token_delta = delta_pct(
        number(&summary, "total_tokens"),
        number(&base_summary, "total_tokens"),
    );
    let new_tools = diff_array(&report, &baseline, "/surfaces/tools");
    let new_files = diff_array(&report, &baseline, "/surfaces/files");

    let comparison = json!({
        "baseline_path": baseline_path,
        "thresholds": {
            "max_duration_delta_pct": thresholds.max_duration_delta_pct,
            "max_cost_delta_pct": thresholds.max_cost_delta_pct,
            "max_token_delta_pct": thresholds.max_token_delta_pct,
        },
        "current": baseline_snapshot(&report),
        "baseline": baseline_snapshot(&baseline),
        "duration_delta_pct": duration_delta,
        "cost_delta_pct": cost_delta,
        "token_delta_pct": token_delta,
        "slower_than_baseline": duration_delta > thresholds.max_duration_delta_pct,
        "cost_above_threshold": cost_delta > thresholds.max_cost_delta_pct,
        "tokens_above_threshold": token_delta > thresholds.max_token_delta_pct,
        "new_failure_families": diff_array(&report, &baseline, "/failure_families"),
        "broader_tool_surface": !new_tools.is_empty(),
        "new_tools": new_tools,
        "broader_file_surface": !new_files.is_empty(),
        "new_files": new_files,
        "new_tool_authority_categories": diff_array(&report, &baseline, "/surfaces/authority_categories"),
        "new_high_authority_tool_use": diff_array(&report, &baseline, "/surfaces/high_authority_tools"),
    });
    if let Value::Object(obj) = &mut report {
        obj.insert("baseline_comparison".to_string(), comparison);
    }
    Ok(serde_json::to_string_pretty(&report)?)
}

pub fn report_overview_text(overview: &Overview, sessions: &[Session]) -> String {
    let ordered = canonical_sessions(sessions);
    let authority = overview_authority_summary(&ordered);
    let sep = "━".repeat(70);
    let mut out = String::new();

    out.push_str(&sep);
    out.push('\n');
    out.push_str(&format!(
        "  AGENTTRACE v{} — Global Overview  ({} Sessions)\n",
        VERSION, overview.total_sessions
    ));
    out.push_str(&sep);
    out.push_str("\n\n");

    let healthy_pct = (overview.healthy * 100)
        .checked_div(overview.total_sessions)
        .unwrap_or(0);
    let warning_pct = (overview.warning * 100)
        .checked_div(overview.total_sessions)
        .unwrap_or(0);
    let critical_pct = (overview.critical * 100)
        .checked_div(overview.total_sessions)
        .unwrap_or(0);
    out.push_str(&format!(
        "  Total Sessions:     {}\n",
        overview.total_sessions
    ));
    out.push_str(&format!(
        "  🟢 Healthy:   {} ({}%)\n",
        format_count(overview.healthy),
        healthy_pct
    ));
    out.push_str(&format!(
        "  🟡 Warning:   {} ({}%)\n",
        format_count(overview.warning),
        warning_pct
    ));
    out.push_str(&format!(
        "  🔴 Critical:   {} ({}%)\n",
        format_count(overview.critical),
        critical_pct
    ));
    out.push_str(&format!(
        "  💰 Total estimated cost:      {}\n\n",
        format_cost(overview.total_cost)
    ));

    let timelines = overview_incident_timelines(&ordered, 3);
    if !timelines.is_empty() {
        out.push_str("  ── Incident timeline ──\n");
        let mut rendered = 0;
        'timeline: for timeline in timelines {
            for item in timeline.items {
                out.push_str(&format!(
                    "    {:<30} {}: {}\n",
                    text_cell(&timeline.session, 30),
                    item.label,
                    text_cell(&item.detail, text_incident_detail_limit(&item.label))
                ));
                rendered += 1;
                if rendered >= 5 {
                    break 'timeline;
                }
            }
        }
        out.push('\n');
    }

    if authority.has_data {
        out.push_str("  ── Tool authority ──\n");
        if !authority.highest.is_empty() {
            out.push_str(&format!("    Highest category: {}\n", authority.highest));
        }
        if !authority.counts.is_empty() {
            for line in text_wrapped_key_values(
                "Authority category counts",
                &text_authority_count_values(&authority.counts),
                96,
            ) {
                out.push_str(&format!("    {line}\n"));
            }
        }
        if !authority.high_tools.is_empty() {
            for line in text_wrapped_key_values(
                "High-authority tools",
                &text_tool_values(&authority.high_tools),
                96,
            ) {
                out.push_str(&format!("    {line}\n"));
            }
        }
        out.push('\n');
    }

    let notes = overview_cost_driver_notes(&ordered, 3);
    if !notes.is_empty() {
        out.push_str("  ── Possible cost drivers ──\n");
        for note in notes {
            out.push_str(&format!(
                "    {:<30} {}\n",
                text_cell(&note.session, 30),
                text_cell(&note.note, 80)
            ));
        }
        out.push('\n');
    }

    out.push_str("  ── By Agent ──\n");
    for (agent, group) in overview_text_agent_groups(&overview.by_agent) {
        out.push_str(&format!(
            "    {:<30} {:>4} Sessions  {:>8}\n",
            tool_display_name(&agent),
            format_count(group.sessions),
            format_cost(group.cost)
        ));
    }
    out.push('\n');

    out.push_str("  ── By Model ──\n");
    for (model, group) in overview_text_model_groups(&overview.by_model)
        .into_iter()
        .take(8)
    {
        out.push_str(&format!(
            "    {:<25} {:>4} Sessions  {:>8}\n",
            model,
            format_count(group.sessions),
            format_cost(group.cost)
        ));
    }
    out.push('\n');

    out.push_str("  ── Recent Anomalies ──\n");
    if overview.anomalies_top.is_empty() {
        out.push_str("    ✅ No anomalies\n");
    } else {
        for anomaly in overview.anomalies_top.iter().take(8) {
            out.push_str(&format!(
                "    ⚠️  {:<30} {}\n",
                text_cell(&anomaly.session, 30),
                anomaly_type_label(&anomaly.kind)
            ));
        }
    }
    out.push('\n');
    out.push_str(&sep);
    out.push('\n');
    out
}

pub fn report_overview_markdown(overview: &Overview, sessions: &[Session]) -> String {
    let ordered = canonical_sessions(sessions);
    let summary = overview_summary(overview, &ordered);
    let authority = overview_authority_summary(&ordered);
    let trend = analyze_health_trend(sessions);
    let mut out = String::new();

    out.push_str("# agenttrace overview\n\n");
    out.push_str("| Metric | Value |\n|---|---:|\n");
    out.push_str(&format!(
        "| Sessions | {} |\n",
        format_count(overview.total_sessions)
    ));
    out.push_str(&format!(
        "| Healthy / Warning / Critical | {} / {} / {} |\n",
        format_count(overview.healthy),
        format_count(overview.warning),
        format_count(overview.critical)
    ));
    out.push_str(&format!(
        "| Average health | {:.1} |\n",
        number_obj(&summary, "avg_health")
    ));
    out.push_str(&format!(
        "| Health Trend | {} |\n",
        markdown_cell(&trend.message)
    ));
    out.push_str(&format!(
        "| Total estimated cost | {} |\n",
        format_cost(overview.total_cost)
    ));
    out.push_str(&format!(
        "| Total tokens | {} |\n",
        format_tokens(number_obj(&summary, "total_tokens") as i64)
    ));
    out.push_str(&format!(
        "| Tool failures | {:.0} / {:.0} ({:.1}%) |\n\n",
        number_obj(&summary, "tool_failures"),
        number_obj(&summary, "tool_calls"),
        number_obj(&summary, "tool_fail_rate")
    ));

    if authority.has_data {
        out.push_str("## Tool authority\n\n");
        out.push_str("| Metric | Value |\n|---|---:|\n");
        if !authority.highest.is_empty() {
            out.push_str(&format!(
                "| Highest category | `{}` |\n",
                markdown_inline_code(&authority.highest)
            ));
        }
        if !authority.high_tools.is_empty() {
            out.push_str(&format!(
                "| High-authority tools | {} |\n",
                report_markdown_code_list(&authority.high_tools)
            ));
        }
        if !authority.counts.is_empty() {
            out.push_str("\n### Authority category counts\n\n");
            out.push_str("| Authority category | Count |\n|---|---:|\n");
            for item in &authority.counts {
                out.push_str(&format!(
                    "| `{}` | {} |\n",
                    markdown_inline_code(&item.category),
                    item.count
                ));
            }
            out.push('\n');
        }
    }

    let cost_notes = overview_cost_driver_notes(&ordered, 6);
    if !cost_notes.is_empty() {
        out.push_str("## Possible cost drivers\n\n");
        for note in cost_notes {
            out.push_str(&format!(
                "- **{}**: {}\n",
                markdown_cell(&note.session),
                markdown_cell(&note.note)
            ));
        }
        out.push('\n');
    }

    out.push_str("## Incident timeline\n\n");
    let timelines = overview_incident_timelines(&ordered, 6);
    if timelines.is_empty() {
        out.push_str("No incident timeline evidence yet.\n\n");
    } else {
        out.push_str("| Session | Signal | Evidence | Severity |\n|---|---|---|---|\n");
        for timeline in timelines {
            for item in timeline.items {
                out.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    markdown_cell(&timeline.session),
                    markdown_cell(&item.label),
                    markdown_cell(&item.detail),
                    markdown_cell(&severity_label(&item.severity))
                ));
            }
        }
        out.push('\n');
    }

    out.push_str("## By agent\n\n");
    out.push_str("| Agent | Sessions | Cost |\n|---|---:|---:|\n");
    for (agent, group) in sorted_agent_groups(&overview.by_agent) {
        out.push_str(&format!(
            "| {} | {} | {} |\n",
            markdown_cell(&tool_display_name(&agent)),
            format_count(group.sessions),
            format_cost(group.cost)
        ));
    }

    out.push_str("\n## Recent sessions\n\n");
    out.push_str(
        "| Session | Source | Model | Health | Cost | Anomalies |\n|---|---|---|---:|---:|---:|\n",
    );
    for session in ordered.iter().take(10) {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            markdown_cell(&session.name),
            markdown_cell(&tool_display_name(&session.metrics.source_tool)),
            markdown_cell(&session.metrics.model_used),
            session.health,
            format_cost(session.metrics.cost_estimated),
            format_count(session.anomalies.len())
        ));
    }

    out.push_str("\n## Recent anomalies\n\n");
    if overview.anomalies_top.is_empty() {
        out.push_str("No anomalies detected.\n");
        return out;
    }
    out.push_str("| Session | Type | Age |\n|---|---|---|\n");
    for anomaly in overview.anomalies_top.iter().take(10) {
        out.push_str(&format!(
            "| {} | {} | {} |\n",
            markdown_cell(&anomaly.session),
            markdown_cell(&anomaly_type_label(&anomaly.kind)),
            markdown_cell(&anomaly.age)
        ));
    }
    out
}

pub fn report_overview_html(overview: &Overview, sessions: &[Session]) -> String {
    let ordered = canonical_sessions(sessions);
    let summary = overview_summary(overview, &ordered);
    let authority = overview_authority_summary(&ordered);
    let trend = analyze_health_trend(&ordered);
    let agents = sorted_agent_groups(&overview.by_agent);
    let models = sorted_model_groups(&overview.by_model);

    let mut out = String::new();
    let mut w = |line: String| {
        out.push_str(&line);
        out.push('\n');
    };

    w("<!doctype html>".to_string());
    w("<html lang=\"en\">".to_string());
    w("<head>".to_string());
    w("<meta charset=\"utf-8\">".to_string());
    w("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">".to_string());
    w("<title>agenttrace overview</title>".to_string());
    w("<link rel=\"icon\" href=\"data:,\">".to_string());
    w("<style>".to_string());
    w(":root{color-scheme:dark;--bg:#07090b;--panel:#101419;--line:#273039;--text:#f4f0dd;--muted:#a9a391;--green:#54ff00;--cyan:#00d8ff;--amber:#ffb000;--red:#ff4a4a}".to_string());
    w("*{box-sizing:border-box}body{margin:0;background:linear-gradient(180deg,#0b0f12,#050607);color:var(--text);font:15px/1.55 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace}".to_string());
    w("main{max-width:1180px;margin:0 auto;padding:32px 18px 48px}header{display:flex;justify-content:space-between;gap:24px;align-items:flex-start;border-bottom:1px solid var(--line);padding-bottom:24px;margin-bottom:24px}".to_string());
    w("h1{font-size:clamp(42px,7vw,88px);line-height:.9;margin:0;letter-spacing:0}h2{margin:0 0 14px;font-size:20px;color:var(--cyan)}p{margin:10px 0 0;color:var(--muted)}".to_string());
    w(".brand{color:var(--green);font-weight:800}.meta{text-align:right;color:var(--muted)}.grid{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:1px;background:var(--line);border:1px solid var(--line);margin:24px 0}.metric{background:var(--panel);padding:18px;min-height:120px}.metric span{display:block;color:var(--muted);font-size:12px;text-transform:uppercase}.metric strong{display:block;margin-top:12px;font-size:30px;color:var(--green)}.warn strong{color:var(--amber)}.bad strong{color:var(--red)}".to_string());
    w("section{border:1px solid var(--line);background:rgba(16,20,25,.78);padding:20px;margin-top:20px}table{width:100%;border-collapse:collapse}th,td{padding:10px;border-bottom:1px solid var(--line);text-align:left;vertical-align:top}th{color:var(--muted);font-size:12px;text-transform:uppercase}td.num,th.num{text-align:right}.health-good{color:var(--green)}.health-warn{color:var(--amber)}.health-bad{color:var(--red)}code{color:var(--cyan)}@media(max-width:760px){header{display:block}.meta{text-align:left;margin-top:16px}.grid{grid-template-columns:1fr}table{font-size:13px}}".to_string());
    w("</style>".to_string());
    w("</head>".to_string());
    w("<body>".to_string());
    w("<main>".to_string());
    w("<header>".to_string());
    w("<div><div class=\"brand\">agenttrace</div><h1>AI agent session overview</h1><p>Static report generated from local coding-agent traces.</p></div>".to_string());
    w(format!(
        "<div class=\"meta\">v{}<br>{} Sessions<br><code>agenttrace --overview -f html</code></div>",
        html_escape(VERSION),
        overview.total_sessions
    ));
    w("</header>".to_string());
    w("<div class=\"grid\" aria-label=\"summary metrics\">".to_string());
    w(format!(
        "<div class=\"metric\"><span>Sessions</span><strong>{}</strong><p>{} Healthy / {} Warning / {} Critical</p></div>",
        overview.total_sessions, overview.healthy, overview.warning, overview.critical
    ));
    w(format!(
        "<div class=\"metric\"><span>Total tokens</span><strong>{}</strong><p>+ live</p></div>",
        format_tokens(number_obj(&summary, "total_tokens") as i64)
    ));
    w(format!(
        "<div class=\"metric\"><span>Average health</span><strong>{:.1}</strong><p>Fleet quality score</p></div>",
        number_obj(&summary, "avg_health")
    ));
    w(format!(
        "<div class=\"metric\"><span>Total estimated cost</span><strong>{}</strong><p>Estimated session cost</p></div>",
        format_cost(overview.total_cost)
    ));
    w(format!(
        "<div class=\"metric {}\"><span>Tool failures</span><strong>{:.0}/{:.0}</strong><p>{:.1}% failure rate</p></div>",
        html_escape(failure_class(number_obj(&summary, "tool_fail_rate"))),
        number_obj(&summary, "tool_failures"),
        number_obj(&summary, "tool_calls"),
        number_obj(&summary, "tool_fail_rate")
    ));
    w("</div>".to_string());

    if authority.has_data {
        w("<section><h2>Tool authority</h2>".to_string());
        if !authority.highest.is_empty() {
            w(format!(
                "<p><strong>Highest category</strong>: <code>{}</code></p>",
                html_escape(&authority.highest)
            ));
        }
        if !authority.counts.is_empty() {
            w("<table><caption>Authority category counts</caption><thead><tr><th>Authority category</th><th class=\"num\">Count</th></tr></thead><tbody>".to_string());
            for item in &authority.counts {
                w(format!(
                    "<tr><td><code>{}</code></td><td class=\"num\">{}</td></tr>",
                    html_escape(&item.category),
                    item.count
                ));
            }
            w("</tbody></table>".to_string());
        }
        if !authority.high_tools.is_empty() {
            w(format!(
                "<p><strong>High-authority tools</strong>: {}</p>",
                report_html_code_list(&authority.high_tools)
            ));
        }
        w("</section>".to_string());
    }

    let cost_notes = overview_cost_driver_notes(&ordered, 8);
    if !cost_notes.is_empty() {
        w("<section><h2>Possible cost drivers</h2><table><thead><tr><th>Session</th><th>Evidence</th></tr></thead><tbody>".to_string());
        for note in cost_notes {
            w(format!(
                "<tr><td>{}</td><td>{}</td></tr>",
                html_escape(&note.session),
                html_escape(&note.note)
            ));
        }
        w("</tbody></table></section>".to_string());
    }
    if ordered.len() > 1 {
        w(format!(
            "<section><h2>Health Trend</h2><p>{}</p></section>",
            html_escape(&trend.message)
        ));
    }

    w("<section><h2>Incident timeline</h2>".to_string());
    let timelines = overview_incident_timelines(&ordered, 8);
    if timelines.is_empty() {
        w("<p>No incident timeline evidence yet.</p>".to_string());
    } else {
        w("<table><thead><tr><th>Session</th><th>Signal</th><th>Evidence</th><th>Severity</th></tr></thead><tbody>".to_string());
        for timeline in timelines {
            for item in timeline.items {
                w(format!(
                    "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                    html_escape(&timeline.session),
                    html_escape(&item.label),
                    html_escape(&item.detail),
                    html_escape(&severity_label(&item.severity))
                ));
            }
        }
        w("</tbody></table>".to_string());
    }
    w("</section>".to_string());

    w("<section><h2>Recent sessions</h2><table><thead><tr><th>Session</th><th>Source</th><th>Model</th><th class=\"num\">Total tokens</th><th class=\"num\">Cost</th><th class=\"num\">Health</th><th class=\"num\">Anomalies</th></tr></thead><tbody>".to_string());
    for session in ordered.iter().take(20) {
        w(format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num {}\">{}</td><td class=\"num\">{}</td></tr>",
            html_escape(&session.name),
            html_escape(&tool_display_name(&session.metrics.source_tool)),
            html_escape(&session.metrics.model_used),
            format_tokens(total_tokens(session)),
            format_cost(session.metrics.cost_estimated),
            html_escape(health_class(session.health)),
            session.health,
            format_count(session.anomalies.len())
        ));
    }
    w("</tbody></table></section>".to_string());

    w("<section><h2>By agent</h2><table><thead><tr><th>Agent</th><th class=\"num\">Sessions</th><th class=\"num\">Cost</th></tr></thead><tbody>".to_string());
    for (agent, group) in agents {
        w(format!(
            "<tr><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td></tr>",
            html_escape(&tool_display_name(&agent)),
            format_count(group.sessions),
            format_cost(group.cost)
        ));
    }
    w("</tbody></table></section>".to_string());

    w("<section><h2>By model</h2><table><thead><tr><th>Model</th><th class=\"num\">Sessions</th><th class=\"num\">Cost</th></tr></thead><tbody>".to_string());
    for (model, group) in models.iter().take(12) {
        w(format!(
            "<tr><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td></tr>",
            html_escape(model),
            format_count(group.sessions),
            format_cost(group.cost)
        ));
    }
    w("</tbody></table></section>".to_string());

    w("<section><h2>Recent anomalies</h2>".to_string());
    if overview.anomalies_top.is_empty() {
        w("<p>No anomalies detected.</p>".to_string());
    } else {
        w(
            "<table><thead><tr><th>Session</th><th>Type</th><th>Age</th></tr></thead><tbody>"
                .to_string(),
        );
        for anomaly in overview.anomalies_top.iter().take(20) {
            w(format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                html_escape(&anomaly.session),
                html_escape(&anomaly_type_label(&anomaly.kind)),
                html_escape(&anomaly.age)
            ));
        }
        w("</tbody></table>".to_string());
    }
    w("</section>".to_string());
    w("</main>".to_string());
    w("</body>".to_string());
    w("</html>".to_string());
    out
}

pub fn report_compare(sessions: &[Session], model: &str) -> String {
    report_compare_with_language(sessions, model, ReportLanguage::En)
}

pub fn report_compare_with_language(
    sessions: &[Session],
    model: &str,
    language: ReportLanguage,
) -> String {
    let sep = "━".repeat(76);
    let mut out = String::new();
    out.push_str(&sep);
    out.push('\n');
    out.push_str(&format!(
        "  AGENTTRACE — {}  ({}: {})\n",
        language.t("Multi-Session Comparison", "多会话对比"),
        language.t("model", "模型"),
        model
    ));
    out.push_str(&sep);
    out.push('\n');
    out.push('\n');
    out.push_str(&format!(
        "  {:<28} {:>4} {:>5} {:>5} {:>5} {:>9} {:>7}\n",
        language.t("SESSION", "会话"),
        language.t("TURNS", "轮次"),
        language.t("TOOLS", "工具"),
        language.t("SUCC%", "成功%"),
        language.t("FAIL", "失败"),
        language.t("COST", "成本"),
        language.t("HEALTH", "健康")
    ));
    out.push_str(&format!("  {}\n", "─".repeat(70)));
    for session in sessions {
        let metrics = &session.metrics;
        let total_tools = metrics.tool_calls_ok + metrics.tool_calls_fail;
        let success_rate = if total_tools > 0 {
            format!(
                "{:.0}%",
                metrics.tool_calls_ok as f64 / total_tools as f64 * 100.0
            )
        } else {
            "N/A".to_string()
        };
        let name = truncate_runes(&session.name, 27);
        out.push_str(&format!(
            "  {:<28} {:>4} {:>5} {:>5} {:>5} {:>9} {} {}/100\n",
            name,
            format_count(metrics.assistant_turns),
            format_count(metrics.tool_calls_total),
            success_rate,
            format_count(metrics.tool_calls_fail),
            format_cost(metrics.cost_estimated),
            health_emoji(session.health),
            session.health
        ));
    }
    out.push_str(&sep);
    out.push('\n');
    out
}

pub fn report_compare_json(sessions: &[Session]) -> String {
    if sessions.is_empty() {
        return "[]".to_string();
    }
    let mut out = String::new();
    out.push_str("[\n");
    for (index, session) in sessions.iter().enumerate() {
        let metrics = &session.metrics;
        let total_tools = metrics.tool_calls_ok + metrics.tool_calls_fail;
        let success_rate = if total_tools > 0 {
            format!(
                "{:.0}%",
                metrics.tool_calls_ok as f64 / total_tools as f64 * 100.0
            )
        } else {
            "N/A".to_string()
        };
        out.push_str("  {\n");
        out.push_str(&format!("    \"name\": {},\n", json_string(&session.name)));
        out.push_str("    \"metrics\": {\n");
        out.push_str(&format!("      \"turns\": {},\n", metrics.assistant_turns));
        out.push_str(&format!("      \"tools\": {},\n", metrics.tool_calls_total));
        out.push_str(&format!(
            "      \"success_rate\": {},\n",
            json_string(&success_rate)
        ));
        out.push_str(&format!("      \"fail\": {},\n", metrics.tool_calls_fail));
        out.push_str(&format!(
            "      \"cost\": {}\n",
            json_float(metrics.cost_estimated)
        ));
        out.push_str("    },\n");
        out.push_str(&format!("    \"health\": {}\n", session.health));
        out.push_str("  }");
        if index + 1 < sessions.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push(']');
    out
}

fn health_emoji(health: i32) -> &'static str {
    if health >= 80 {
        "🟢"
    } else if health >= 50 {
        "🟡"
    } else {
        "🔴"
    }
}

fn health_bar(health: i32) -> String {
    let blocks = (health / 5).clamp(0, 20) as usize;
    let empty = 20usize.saturating_sub(blocks);
    format!("[{}{}]", "█".repeat(blocks), "░".repeat(empty))
}

fn success_rate(ok: usize, total: usize) -> String {
    if total == 0 {
        "N/A".to_string()
    } else {
        format!("{:.0}%", ok as f64 / total as f64 * 100.0)
    }
}

fn top_tool_rows(tools: &BTreeMap<String, usize>) -> Vec<(String, usize)> {
    let mut items: Vec<_> = tools
        .iter()
        .map(|(key, value)| (key.clone(), *value))
        .collect();
    items.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    items
}

fn anomaly_emoji(severity: &str) -> &'static str {
    match severity {
        "high" => "🔴",
        "medium" => "🟡",
        "low" => "🟢",
        _ => "",
    }
}

fn severity_label(severity: &str) -> String {
    severity_label_for_language(severity, ReportLanguage::En)
}

fn severity_label_for_language(severity: &str, language: ReportLanguage) -> String {
    match severity.to_ascii_lowercase().as_str() {
        "critical" => language.t("CRITICAL", "严重").to_string(),
        "high" => language.t("HIGH", "高").to_string(),
        "warning" | "medium" => language.t("MEDIUM", "中").to_string(),
        "good" | "low" => language.t("LOW", "低").to_string(),
        _ => severity.to_ascii_uppercase(),
    }
}

fn anomaly_type_label(kind: &str) -> String {
    anomaly_type_label_for_language(kind, ReportLanguage::En)
}

fn anomaly_type_label_for_language(kind: &str, language: ReportLanguage) -> String {
    if language == ReportLanguage::Zh {
        return match kind {
            "hanging" => "卡顿".to_string(),
            "latency" => "延迟".to_string(),
            "tool_failures" => "工具失败".to_string(),
            "shallow_thinking" => "推理过浅".to_string(),
            "redacted" | "redaction" => "推理脱敏".to_string(),
            "no_tools" => "未使用工具".to_string(),
            other => other.replace('_', " "),
        };
    }
    match kind {
        "hanging" => "hanging".to_string(),
        "latency" => "latency".to_string(),
        "tool_failures" => "tool failures".to_string(),
        "shallow_thinking" => "shallow thinking".to_string(),
        "redacted" | "redaction" => "redacted thinking".to_string(),
        "no_tools" => "no tools".to_string(),
        other => other.replace('_', " "),
    }
}

pub(crate) fn json_string(value: &str) -> String {
    go_json_escape(serde_json::to_string(value).expect("string serializes"))
}

pub(crate) fn go_json_escape(value: String) -> String {
    value
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026")
}

pub(crate) fn json_float(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        serde_json::to_string(&value).expect("float serializes")
    }
}

fn write_usize_map_json(out: &mut String, values: &BTreeMap<String, usize>, base_indent: usize) {
    if values.is_empty() {
        out.push_str("{}");
        return;
    }
    let current_indent = "  ".repeat(base_indent);
    let item_indent = "  ".repeat(base_indent + 1);
    out.push_str("{\n");
    for (index, (key, value)) in values.iter().enumerate() {
        out.push_str(&item_indent);
        out.push_str(&json_string(key));
        out.push_str(": ");
        out.push_str(&value.to_string());
        if index + 1 < values.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&current_indent);
    out.push('}');
}

fn write_anomalies_json(
    out: &mut String,
    anomalies: &[Anomaly],
    language: ReportLanguage,
    base_indent: usize,
) {
    if anomalies.is_empty() {
        out.push_str("[]");
        return;
    }
    let array_indent = "  ".repeat(base_indent);
    let object_indent = "  ".repeat(base_indent + 1);
    let field_indent = "  ".repeat(base_indent + 2);
    out.push_str("[\n");
    for (index, anomaly) in anomalies.iter().enumerate() {
        out.push_str(&object_indent);
        out.push_str("{\n");
        out.push_str(&field_indent);
        out.push_str("\"detail\": ");
        out.push_str(&json_string(&anomaly_detail_for_language(
            anomaly, language,
        )));
        out.push_str(",\n");
        out.push_str(&field_indent);
        out.push_str("\"severity\": ");
        out.push_str(&json_string(&anomaly.severity));
        out.push_str(",\n");
        out.push_str(&field_indent);
        out.push_str("\"type\": ");
        out.push_str(&json_string(&anomaly.kind));
        out.push('\n');
        out.push_str(&object_indent);
        out.push('}');
        if index + 1 < anomalies.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&array_indent);
    out.push(']');
}

fn fmt_duration_for_language(seconds: f64, language: ReportLanguage) -> String {
    match language {
        ReportLanguage::En => fmt_duration(seconds),
        ReportLanguage::Zh => {
            if seconds < 60.0 {
                format!("{seconds:.0}秒")
            } else if seconds < 3600.0 {
                format!("{:.1}分钟", seconds / 60.0)
            } else {
                let hours = (seconds / 3600.0) as i64;
                let minutes = ((seconds as i64) % 3600) / 60;
                format!("{hours}小时{minutes}分钟")
            }
        }
    }
}

fn anomaly_detail_for_language(anomaly: &Anomaly, language: ReportLanguage) -> String {
    if language == ReportLanguage::En {
        return anomaly.detail.clone();
    }
    match anomaly.kind.as_str() {
        "shallow_thinking" => {
            if let Some(avg) = parse_avg_reasoning_chars(&anomaly.detail) {
                if anomaly.severity == "high" {
                    format!("平均推理 = {avg:.0} 字符 (极浅)")
                } else {
                    format!("平均推理 = {avg:.0} 字符")
                }
            } else {
                anomaly.detail.clone()
            }
        }
        "no_tools" => "无工具调用 — 纯对话会话".to_string(),
        "hanging" => anomaly
            .detail
            .strip_suffix('s')
            .map(|detail| detail.replace(" gap(s) >60s, max=", "个间隔 >60秒, 最长=") + "秒")
            .unwrap_or_else(|| anomaly.detail.clone()),
        "latency" => anomaly
            .detail
            .strip_prefix("p95 latency = ")
            .and_then(|value| value.strip_suffix('s'))
            .map(|value| format!("P95延迟 = {value}秒"))
            .unwrap_or_else(|| anomaly.detail.clone()),
        "tool_failures" => anomaly.detail.replace(" failed", " 失败"),
        "redaction" | "redacted" => anomaly
            .detail
            .strip_suffix(" block(s) redacted")
            .map(|count| format!("{count} 思维块已脱敏"))
            .unwrap_or_else(|| anomaly.detail.clone()),
        _ => anomaly.detail.clone(),
    }
}

fn parse_avg_reasoning_chars(detail: &str) -> Option<f64> {
    let value = detail.strip_prefix("avg reasoning = ")?;
    let value = value.split_whitespace().next()?;
    value.parse().ok()
}

fn overview_summary(overview: &Overview, sessions: &[Session]) -> Value {
    let total_tokens: i64 = sessions.iter().map(total_tokens).sum();
    let total_tools: usize = sessions
        .iter()
        .map(|s| s.metrics.tool_calls_ok + s.metrics.tool_calls_fail)
        .sum();
    let failed_tools: usize = sessions.iter().map(|s| s.metrics.tool_calls_fail).sum();
    let total_duration: f64 = sessions.iter().map(|s| s.metrics.duration_sec).sum();
    let anomalies_total = overview.anomalies_top.len();
    let authority_counts = authority_counts(sessions);
    let highest = highest_authority(sessions);
    let trend = analyze_health_trend_full(sessions);
    json!({
        "total_sessions": overview.total_sessions,
        "healthy": overview.healthy,
        "warning": overview.warning,
        "critical": overview.critical,
        "avg_health": round4(average_health(sessions)),
        "total_cost": round4(overview.total_cost),
        "total_duration_seconds": round4(total_duration),
        "total_tokens": total_tokens,
        "tool_calls": total_tools,
        "tool_failures": failed_tools,
        "tool_fail_rate": if total_tools > 0 { round4(failed_tools as f64 / total_tools as f64 * 100.0) } else { 0.0 },
        "anomalies_total": anomalies_total,
        "anomalies_returned": anomalies_total.min(50),
        "anomalies_truncated": anomalies_total > 50,
        "health_trend": {
            "direction": trend.direction,
            "regressing": trend.regressing,
            "avg_health": round4(trend.avg_health),
            "message": trend.message,
            "points": trend.points.iter().map(|point| json!({"name": point.name, "health": point.health, "cost": round4(point.cost)})).collect::<Vec<_>>(),
        },
        "tool_authority": {
            "highest": highest,
            "counts": authority_counts,
        },
    })
}

fn group_items(groups: &BTreeMap<String, GroupOverview>, agent_display: bool) -> Vec<Value> {
    let mut items: Vec<_> = groups
        .iter()
        .map(|(name, group)| {
            json!({
                "name": if agent_display { tool_display_name(name) } else { name.clone() },
                "sessions": group.sessions,
                "cost": round4(group.cost),
            })
        })
        .collect();
    if agent_display {
        items.sort_by(|a, b| {
            number_value(b, "sessions")
                .partial_cmp(&number_value(a, "sessions"))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    number_value(b, "cost")
                        .partial_cmp(&number_value(a, "cost"))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| string_value(a, "name").cmp(&string_value(b, "name")))
        });
    } else {
        items.sort_by(|a, b| {
            number_value(b, "cost")
                .partial_cmp(&number_value(a, "cost"))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    number_value(b, "sessions")
                        .partial_cmp(&number_value(a, "sessions"))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| string_value(a, "name").cmp(&string_value(b, "name")))
        });
    }
    items
}

fn surfaces(sessions: &[Session]) -> Value {
    let mut tools = BTreeMap::new();
    let mut files = BTreeMap::new();
    let mut authority = BTreeMap::new();
    for session in sessions {
        for key in session.metrics.tool_usage.keys() {
            tools.insert(key.clone(), ());
        }
        for key in session.metrics.file_usage.keys() {
            files.insert(report_file_surface(key), ());
        }
        for (key, count) in &session.metrics.tool_authority {
            if *count > 0 {
                authority.insert(key.clone(), *count);
            }
        }
    }
    let tool_names = sorted_keys(&tools);
    json!({
        "tools": tool_names,
        "files": sorted_keys(&files),
        "authority_categories": sorted_keys(&authority),
        "high_authority_tools": high_authority_tools(&tool_names),
    })
}

fn failure_families(sessions: &[Session]) -> Vec<String> {
    let mut families = BTreeSet::new();
    for session in sessions {
        for anomaly in &session.anomalies {
            families.insert(anomaly.kind.clone());
        }
    }
    sorted_set(families)
}

fn report_file_surface(value: &str) -> String {
    serde_json::from_str::<Value>(value)
        .ok()
        .and_then(|json| {
            json.get("path")
                .or_else(|| json.get("file_path"))
                .or_else(|| json.get("file"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .filter(|path| !path.is_empty())
        .unwrap_or_else(|| value.to_string())
}

fn incident_timelines(sessions: &[Session]) -> Vec<Value> {
    overview_incident_timelines(sessions, 10)
        .into_iter()
        .map(|timeline| {
            json!({
                "session": timeline.session,
                "items": timeline.items.into_iter().map(|item| json!({
                    "kind": item.kind,
                    "label": item.label,
                    "detail": item.detail,
                    "severity": item.severity,
                })).collect::<Vec<_>>(),
            })
        })
        .collect()
}

fn high_authority_tools(tools: &[String]) -> Vec<String> {
    tools
        .iter()
        .filter(|tool| is_high_authority_category(&classify_tool_authority_name(tool)))
        .cloned()
        .collect()
}

fn classify_tool_authority_name(name: &str) -> String {
    classify_tool_authority(&ToolCall {
        name: name.to_string(),
        ..ToolCall::default()
    })
}

fn authority_counts(sessions: &[Session]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for session in sessions {
        for (authority, count) in &session.metrics.tool_authority {
            if *count > 0 {
                *counts.entry(authority.clone()).or_insert(0) += count;
            }
        }
    }
    counts
}

fn highest_authority(sessions: &[Session]) -> String {
    let mut highest = String::new();
    for session in sessions {
        let current = highest_authority_for_metrics(&session.metrics);
        highest = crate::higher_tool_authority(&highest, &current);
    }
    highest
}

fn top_tools(tools: &BTreeMap<String, usize>) -> BTreeMap<String, usize> {
    let mut items: Vec<_> = tools.iter().collect();
    items.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
    items
        .into_iter()
        .take(10)
        .map(|(key, value)| (key.clone(), *value))
        .collect()
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn average(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

fn delta_pct(current: f64, baseline: f64) -> f64 {
    if baseline == 0.0 {
        if current == 0.0 {
            0.0
        } else {
            100.0
        }
    } else {
        round4((current - baseline) / baseline * 100.0)
    }
}

fn diff_array(current: &Value, baseline: &Value, pointer: &str) -> Vec<String> {
    let current_items = current
        .pointer(pointer)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let baseline_items = baseline
        .pointer(pointer)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let baseline_set: BTreeSet<_> = baseline_items
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();
    current_items
        .iter()
        .filter_map(Value::as_str)
        .filter(|item| !baseline_set.contains(*item))
        .map(str::to_string)
        .collect()
}

fn baseline_snapshot(report: &Value) -> Value {
    json!({
        "duration_seconds": round4(report.pointer("/summary/total_duration_seconds").and_then(Value::as_f64).unwrap_or(0.0)),
        "cost": round4(report.pointer("/summary/total_cost").and_then(Value::as_f64).unwrap_or(0.0)),
        "tokens": report.pointer("/summary/total_tokens").and_then(Value::as_i64).unwrap_or(0),
        "failure_families": sorted_string_array(report.pointer("/failure_families")),
        "tools": sorted_string_array(report.pointer("/surfaces/tools")),
        "files": sorted_string_array(report.pointer("/surfaces/files")),
        "authority_categories": sorted_string_array(report.pointer("/surfaces/authority_categories")),
        "high_authority_tools": sorted_string_array(report.pointer("/surfaces/high_authority_tools")),
    })
}

fn sorted_string_array(value: Option<&Value>) -> Vec<String> {
    let mut items: Vec<String> = value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect();
    items.sort();
    items.dedup();
    items
}

fn number(map: &Map<String, Value>, key: &str) -> f64 {
    map.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

fn number_obj(value: &Value, key: &str) -> f64 {
    value.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

fn number_value(value: &Value, key: &str) -> f64 {
    value.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

fn string_value(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn optional_string(value: &str) -> Value {
    if value.is_empty() {
        Value::Null
    } else {
        Value::String(value.to_string())
    }
}

fn strip_nulls(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .filter_map(|(key, value)| {
                    if value.is_null() {
                        None
                    } else {
                        Some((key, strip_nulls(value)))
                    }
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(strip_nulls).collect()),
        other => other,
    }
}

fn tool_display_name(name: &str) -> String {
    match name {
        "hermes_jsonl" => "Hermes Agent (JSONL)".to_string(),
        "hermes_json" => "Hermes Agent (.json)".to_string(),
        "hermes_db" => "Hermes Agent (DB)".to_string(),
        "claude_code" => "Claude Code".to_string(),
        "claude_code_jsonl" => "Claude Code (JSONL)".to_string(),
        "codex_cli" => "Codex CLI".to_string(),
        "codex_rollout" => "Codex CLI (Rollout)".to_string(),
        "gemini_cli" => "Gemini CLI".to_string(),
        "qwen_code" => "Qwen Code".to_string(),
        "opencode" => "OpenCode".to_string(),
        "opencode_db" => "OpenCode (DB)".to_string(),
        "openclaw" => "OpenClaw".to_string(),
        "copilot_cli" => "Copilot CLI".to_string(),
        "kimi_cli" => "Kimi CLI".to_string(),
        "pi" => "Pi".to_string(),
        "oh_my_pi" => "Oh My Pi".to_string(),
        "aider" => "Aider".to_string(),
        "cursor" => "Cursor".to_string(),
        "cline" => "Cline".to_string(),
        "generic" => "Generic JSON/JSONL".to_string(),
        other => other.to_string(),
    }
}

#[derive(Debug)]
struct AuthorityCount {
    category: String,
    count: usize,
}

#[derive(Debug)]
struct OverviewAuthority {
    highest: String,
    counts: Vec<AuthorityCount>,
    high_tools: Vec<String>,
    has_data: bool,
}

#[derive(Debug)]
struct CostDriverNote {
    session: String,
    note: String,
}

#[derive(Debug)]
struct HealthTrend {
    message: String,
}

#[derive(Debug)]
struct FullHealthTrend {
    direction: String,
    regressing: bool,
    avg_health: f64,
    message: String,
    points: Vec<TrendPoint>,
}

#[derive(Debug)]
struct TrendPoint {
    name: String,
    health: i32,
    cost: f64,
}

#[derive(Debug)]
struct IncidentTimelineItem {
    kind: String,
    label: String,
    detail: String,
    severity: String,
}

#[derive(Debug)]
struct IncidentTimelineSummary {
    session: String,
    items: Vec<IncidentTimelineItem>,
}

fn overview_authority_summary(sessions: &[Session]) -> OverviewAuthority {
    let mut counts = BTreeMap::new();
    let mut tool_surface = BTreeMap::new();
    let mut highest = String::new();
    for session in sessions {
        for tool in session.metrics.tool_usage.keys() {
            tool_surface.insert(tool.clone(), ());
        }
        for (category, count) in &session.metrics.tool_authority {
            if *count > 0 {
                *counts.entry(category.clone()).or_insert(0) += *count;
                highest = crate::higher_tool_authority(&highest, category);
            }
        }
        highest = crate::higher_tool_authority(&highest, &session.metrics.highest_authority);
    }
    let counts_vec = counts
        .iter()
        .filter(|(category, count)| !category.is_empty() && **count > 0)
        .map(|(category, count)| AuthorityCount {
            category: category.clone(),
            count: *count,
        })
        .collect::<Vec<_>>();
    let tool_names = sorted_keys(&tool_surface);
    let high_tools = high_authority_tools(&tool_names);
    let has_data = !highest.is_empty() || !counts_vec.is_empty() || !high_tools.is_empty();
    OverviewAuthority {
        highest,
        counts: counts_vec,
        high_tools,
        has_data,
    }
}

fn overview_cost_driver_notes(sessions: &[Session], limit: usize) -> Vec<CostDriverNote> {
    if limit == 0 {
        return Vec::new();
    }
    let mut notes = Vec::new();
    for session in sessions {
        if let Some(note) = possible_cost_driver_note_strict(session) {
            notes.push(CostDriverNote {
                session: session.name.clone(),
                note,
            });
            if notes.len() >= limit {
                break;
            }
        }
    }
    notes
}

fn analyze_health_trend(sessions: &[Session]) -> HealthTrend {
    HealthTrend {
        message: analyze_health_trend_full(sessions).message,
    }
}

fn analyze_health_trend_full(sessions: &[Session]) -> FullHealthTrend {
    if sessions.is_empty() {
        return FullHealthTrend {
            direction: String::new(),
            regressing: false,
            avg_health: 0.0,
            message: "No session data available".to_string(),
            points: Vec::new(),
        };
    }

    let mut ordered = sessions.to_vec();
    ordered.sort_by(|a, b| {
        match (
            parse_rfc3339(&a.metrics.session_start),
            parse_rfc3339(&b.metrics.session_start),
        ) {
            (Some(a_ts), Some(b_ts)) if a_ts != b_ts => a_ts.cmp(&b_ts),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            _ => Ordering::Equal,
        }
    });

    let n = ordered.len().min(10);
    let recent = &ordered[ordered.len() - n..];
    let points = recent
        .iter()
        .map(|session| TrendPoint {
            name: session.name.clone(),
            health: session.health,
            cost: session.metrics.cost_estimated,
        })
        .collect::<Vec<_>>();
    let health_points: Vec<i32> = points.iter().map(|point| point.health).collect();
    let mut smoothed = Vec::with_capacity(health_points.len());
    for index in 0..health_points.len() {
        let start = index.saturating_sub(1);
        let end = (index + 1).min(health_points.len() - 1);
        let mut sum = 0;
        let mut count = 0;
        for health in &health_points[start..=end] {
            sum += *health;
            count += 1;
        }
        smoothed.push(sum as f64 / count as f64);
    }

    let avg_health = health_points
        .iter()
        .map(|health| *health as f64)
        .sum::<f64>()
        / health_points.len() as f64;
    let direction = if smoothed.len() >= 2 {
        let diff = smoothed[smoothed.len() - 1] - smoothed[0];
        if diff > 5.0 {
            "up"
        } else if diff < -5.0 {
            "down"
        } else {
            "stable"
        }
    } else {
        "stable"
    };

    let regressing = if smoothed.len() >= 3 {
        let last3 = &smoothed[smoothed.len() - 3..];
        last3.windows(2).all(|pair| pair[1] < pair[0]) && last3[last3.len() - 1] < avg_health
    } else {
        false
    };

    let start_index = health_points.len().saturating_sub(3);
    let last3_values = health_points[start_index..]
        .iter()
        .map(i32::to_string)
        .collect::<Vec<_>>();
    let endpoint = if health_points.len() >= 2 {
        format!(
            "{}→{}",
            health_points[0],
            health_points[health_points.len() - 1]
        )
    } else {
        last3_values.join("→")
    };

    let message = if regressing {
        format!("Declining: {}", last3_values.join("→"))
    } else if direction == "down" && last3_values.len() >= 2 {
        format!("Declining: {endpoint}")
    } else if direction == "up" && last3_values.len() >= 2 {
        format!("Improving: {endpoint}")
    } else {
        format!("Health score stable at {avg_health:.0}")
    };

    FullHealthTrend {
        direction: direction.to_string(),
        regressing,
        avg_health,
        message,
        points,
    }
}

fn overview_incident_timelines(sessions: &[Session], limit: usize) -> Vec<IncidentTimelineSummary> {
    if limit == 0 {
        return Vec::new();
    }
    let mut items = Vec::new();
    for session in sessions {
        let timeline = build_incident_timeline(session);
        if timeline.items.is_empty() {
            continue;
        }
        items.push(timeline);
        if items.len() >= limit {
            break;
        }
    }
    items
}

fn build_incident_timeline(session: &Session) -> IncidentTimelineSummary {
    let metrics = &session.metrics;
    let mut items = Vec::new();
    let mut add = |kind: &str, label: &str, detail: String, severity: &str| {
        let detail = detail.trim().to_string();
        if detail.is_empty() {
            return;
        }
        items.push(IncidentTimelineItem {
            kind: kind.to_string(),
            label: label.to_string(),
            detail,
            severity: severity.to_string(),
        });
    };

    if metrics.assistant_turns > 0 {
        add(
            "milestone",
            "Last milestone",
            format!(
                "{} assistant turn(s) completed over {}",
                metrics.assistant_turns,
                fmt_duration(metrics.duration_sec)
            ),
            "low",
        );
    }

    if let Some(gap) = max_incident_gap(&metrics.gaps_sec) {
        if gap >= 30.0 {
            let severity = if gap >= 300.0 {
                "high"
            } else if gap >= 60.0 {
                "medium"
            } else {
                "low"
            };
            add(
                "idle_gap",
                "Longest idle gap",
                format!("{gap:.1}s gap between recorded events"),
                severity,
            );
        }
    }

    let total_tools = metrics.tool_calls_ok + metrics.tool_calls_fail;
    if total_tools > 0 && metrics.tool_calls_fail > 1 {
        let fail_rate = metrics.tool_calls_fail as f64 / total_tools as f64 * 100.0;
        let severity = if fail_rate >= 30.0 { "high" } else { "medium" };
        add(
            "failure_loop",
            "Failure loop",
            format!(
                "{} failed tool result(s) out of {} ({fail_rate:.1}%)",
                metrics.tool_calls_fail, total_tools
            ),
            severity,
        );
    }

    if total_tools > 0 && !metrics.tool_usage.is_empty() {
        if let Some((tool, count)) = top_incident_tool(&metrics.tool_usage) {
            add(
                "touched_surface",
                "Touched surface",
                format!(
                    "{} unique tool(s), {} total calls; top tool {} x{}",
                    metrics.tool_usage.len(),
                    total_tools,
                    incident_safe_name(&tool),
                    count
                ),
                "low",
            );
        }
    }

    let total_tokens = total_tokens(session);
    if total_tokens > 0 && metrics.assistant_turns > 0 {
        let tokens_per_turn = total_tokens / metrics.assistant_turns as i64;
        if tokens_per_turn >= 10000 {
            add(
                "burn_divergence",
                "Burn divergence",
                format!(
                    "{} tokens per assistant turn across {} turn(s)",
                    format_tokens(tokens_per_turn),
                    metrics.assistant_turns
                ),
                "medium",
            );
        }
    }

    IncidentTimelineSummary {
        session: session.name.clone(),
        items,
    }
}

fn sorted_agent_groups(groups: &BTreeMap<String, GroupOverview>) -> Vec<(String, GroupOverview)> {
    let mut items: Vec<_> = groups
        .iter()
        .map(|(name, group)| (name.clone(), group.clone()))
        .collect();
    items.sort_by(|a, b| {
        b.1.sessions
            .cmp(&a.1.sessions)
            .then_with(|| b.1.cost.partial_cmp(&a.1.cost).unwrap_or(Ordering::Equal))
            .then_with(|| a.0.cmp(&b.0))
    });
    items
}

fn overview_text_agent_groups(
    groups: &BTreeMap<String, GroupOverview>,
) -> Vec<(String, GroupOverview)> {
    let mut items: Vec<_> = groups
        .iter()
        .map(|(name, group)| (name.clone(), group.clone()))
        .collect();
    items.sort_by_key(|item| Reverse(item.1.sessions));
    items
}

fn overview_text_model_groups(
    groups: &BTreeMap<String, GroupOverview>,
) -> Vec<(String, GroupOverview)> {
    let mut items: Vec<_> = groups
        .iter()
        .map(|(name, group)| (name.clone(), group.clone()))
        .collect();
    items.sort_by(|a, b| b.1.cost.partial_cmp(&a.1.cost).unwrap_or(Ordering::Equal));
    items
}

fn sorted_model_groups(groups: &BTreeMap<String, GroupOverview>) -> Vec<(String, GroupOverview)> {
    let mut items: Vec<_> = groups
        .iter()
        .map(|(name, group)| (name.clone(), group.clone()))
        .collect();
    items.sort_by(|a, b| {
        b.1.cost
            .partial_cmp(&a.1.cost)
            .unwrap_or(Ordering::Equal)
            .then_with(|| b.1.sessions.cmp(&a.1.sessions))
            .then_with(|| a.0.cmp(&b.0))
    });
    items
}

fn health_class(health: i32) -> &'static str {
    if health >= 80 {
        "health-good"
    } else if health >= 50 {
        "health-warn"
    } else {
        "health-bad"
    }
}

fn failure_class(rate: f64) -> &'static str {
    if rate >= 25.0 {
        "bad"
    } else if rate >= 10.0 {
        "warn"
    } else {
        ""
    }
}

fn report_html_code_list(values: &[String]) -> String {
    values
        .iter()
        .filter(|value| !value.is_empty())
        .map(|value| format!("<code>{}</code>", html_escape(value)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn html_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&#34;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

fn text_authority_count_values(items: &[AuthorityCount]) -> Vec<String> {
    items
        .iter()
        .map(|item| format!("{}={}", item.category, item.count))
        .collect()
}

fn text_tool_values(items: &[String]) -> Vec<String> {
    items.iter().map(|item| text_cell(item, 40)).collect()
}

fn text_incident_detail_limit(label: &str) -> usize {
    const LINE_LIMIT: usize = 96;
    const INDENT_WIDTH: usize = 4;
    const SESSION_WIDTH: usize = 30;
    const SEPARATORS_WIDTH: usize = 3;

    let label_width = label.chars().count();
    let limit = LINE_LIMIT
        .saturating_sub(INDENT_WIDTH)
        .saturating_sub(SESSION_WIDTH)
        .saturating_sub(SEPARATORS_WIDTH)
        .saturating_sub(label_width);
    limit.max(24)
}

fn text_wrapped_key_values(label: &str, values: &[String], limit: usize) -> Vec<String> {
    if values.is_empty() {
        return vec![format!("{label}:")];
    }
    let prefix = format!("{label}: ");
    let continuation = " ".repeat(label.chars().count() + 2);
    let mut lines = Vec::with_capacity(1);
    let mut current = prefix.clone();
    for value in values {
        let separator = if current != prefix && current != continuation {
            ", "
        } else {
            ""
        };
        let mut next = format!("{separator}{value}");
        if rune_count(&current) + rune_count(&next) > limit
            && current != prefix
            && current != continuation
        {
            lines.push(current);
            current = format!("{continuation}{value}");
            continue;
        }
        if rune_count(&current) + rune_count(&next) > limit {
            let value_limit = limit.saturating_sub(rune_count(&current)).max(4);
            next = text_cell(value, value_limit);
        }
        current.push_str(&next);
    }
    lines.push(current);
    lines
}

fn text_cell(value: &str, limit: usize) -> String {
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if limit > 3 {
        truncate_text_runes(&value, limit, "...")
    } else {
        value
    }
}

fn truncate_text_runes(value: &str, limit: usize, suffix: &str) -> String {
    if limit == 0 {
        return String::new();
    }
    if value.chars().count() <= limit {
        return value.to_string();
    }
    let suffix_len = suffix.chars().count();
    let (cut, suffix) = if !suffix.is_empty() && suffix_len < limit {
        (limit - suffix_len, suffix)
    } else {
        (limit, "")
    };
    format!("{}{}", value.chars().take(cut).collect::<String>(), suffix)
}

fn rune_count(value: &str) -> usize {
    value.chars().count()
}

fn max_incident_gap(gaps: &[f64]) -> Option<f64> {
    let mut max_gap = 0.0;
    for gap in gaps {
        if !gap.is_finite() || *gap < 0.0 {
            continue;
        }
        if *gap > max_gap {
            max_gap = *gap;
        }
    }
    Some(max_gap)
}

fn top_incident_tool(items: &BTreeMap<String, usize>) -> Option<(String, usize)> {
    items
        .iter()
        .filter(|(_, count)| **count > 0)
        .map(|(name, count)| (name.clone(), *count))
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
}

fn incident_safe_name(value: &str) -> String {
    let value = value.split_whitespace().next().unwrap_or("").trim();
    truncate_runes(value, 48)
}

fn truncate_runes(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_string();
    }
    value.chars().take(limit).collect()
}

fn parse_rfc3339(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|ts| ts.with_timezone(&Utc))
}

fn markdown_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', "<br>")
}

fn markdown_inline_code(value: &str) -> String {
    value
        .replace('`', "'")
        .replace('|', "\\|")
        .replace('\n', "<br>")
}

fn report_markdown_code_list(values: &[String]) -> String {
    values
        .iter()
        .filter(|value| !value.is_empty())
        .map(|value| format!("`{}`", markdown_inline_code(value)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn possible_cost_driver_note_strict(session: &Session) -> Option<String> {
    let metrics = &session.metrics;
    let total_tools = metrics.tool_calls_ok + metrics.tool_calls_fail;
    if total_tools > 0 {
        let fail_rate = metrics.tool_calls_fail as f64 / total_tools as f64 * 100.0;
        if fail_rate >= 25.0 {
            return Some(format!(
                "possible driver: {}/{} failed tool result(s) ({fail_rate:.1}%)",
                metrics.tool_calls_fail, total_tools
            ));
        }
    }
    if metrics.assistant_turns > 0 {
        let tokens_per_turn = total_tokens(session) / metrics.assistant_turns as i64;
        if tokens_per_turn >= 50000 {
            return Some(format!(
                "possible driver: {} tokens per assistant turn across {} turn(s)",
                format_tokens(tokens_per_turn),
                metrics.assistant_turns
            ));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Metrics;

    #[test]
    fn compare_json_formats_zero_cost_like_go() {
        let session = Session {
            name: "session".to_string(),
            path: "/tmp/session.jsonl".to_string(),
            cwd: String::new(),
            metrics: Metrics {
                assistant_turns: 2,
                tool_calls_total: 4,
                tool_calls_ok: 4,
                cost_estimated: 0.0,
                ..Metrics::default()
            },
            anomalies: Vec::new(),
            health: 100,
            tool_warnings: Vec::new(),
            diagnostics: crate::Diagnostics::default(),
        };

        let report = report_compare_json(&[session]);
        assert!(report.contains("\"cost\": 0"));
        assert!(!report.contains("\"cost\": 0.0"));
    }

    #[test]
    fn compare_report_truncates_utf8_session_names_on_char_boundaries() {
        let session = Session {
            name: "打开中文文件并生成排查报告的长会话名称".to_string(),
            path: "/tmp/session.jsonl".to_string(),
            cwd: String::new(),
            metrics: Metrics {
                assistant_turns: 2,
                tool_calls_total: 4,
                tool_calls_ok: 4,
                cost_estimated: 0.0,
                ..Metrics::default()
            },
            anomalies: Vec::new(),
            health: 100,
            tool_warnings: Vec::new(),
            diagnostics: crate::Diagnostics::default(),
        };

        let report = report_compare(&[session], "default");
        assert!(report.contains("打开中文文件"));
    }

    #[test]
    fn text_and_compare_reports_support_chinese() {
        let session = Session {
            name: "会话".to_string(),
            path: "/tmp/session.jsonl".to_string(),
            cwd: String::new(),
            metrics: Metrics {
                assistant_turns: 2,
                tool_calls_total: 4,
                tool_calls_ok: 4,
                ..Metrics::default()
            },
            anomalies: Vec::new(),
            health: 100,
            tool_warnings: Vec::new(),
            diagnostics: crate::Diagnostics::default(),
        };

        let report = report_text_with_language(&session, ReportLanguage::Zh);
        assert!(report.contains("AI 智能体会话性能报告"));
        assert!(report.contains("成本与 Token"));
        assert!(!report.contains("MONEY WASTE"));

        let compare = report_compare_with_language(&[session], "default", ReportLanguage::Zh);
        assert!(compare.contains("多会话对比"));
        assert!(compare.contains("会话"));
    }
}
