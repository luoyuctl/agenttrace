use crate::{
    format_cost, format_tokens, loop_waste_percent, pricing, round4, Metrics, ReportLanguage,
    Session, VERSION,
};

#[derive(Debug, Clone)]
pub struct CacheEfficiency {
    cache_read_tokens: i64,
    total_input_tokens: i64,
    hit_rate: f64,
    wasted_cost: f64,
    rating: &'static str,
    suggestion: &'static str,
}

#[derive(Debug, Clone)]
pub struct ToolBloatItem {
    tool_name: String,
    call_count: usize,
    total_cost: f64,
    is_redundant: bool,
}

#[derive(Debug, Clone)]
pub struct ToolBloatAnalysis {
    tools_per_turn: f64,
    bloat_score: i32,
    bloat_level: &'static str,
    top_bloat: Vec<ToolBloatItem>,
}

#[derive(Debug, Clone)]
pub struct StuckPattern {
    description: String,
    severity: &'static str,
}

#[derive(Debug, Clone)]
pub struct WasteReport {
    cache: CacheEfficiency,
    bloat: ToolBloatAnalysis,
    stuck: Vec<StuckPattern>,
    waste_score: i32,
    waste_level: &'static str,
    total_wasted: f64,
    loop_percent: f64,
    summary: String,
    top_actions: Vec<String>,
}

pub fn compute_waste_report(session: &Session) -> WasteReport {
    let cache = analyze_cache_efficiency(&session.metrics);
    let bloat = analyze_tool_bloat(&session.metrics);
    let mut stuck = detect_stuck_from_metrics(&session.metrics);
    stuck.extend(
        session
            .diagnostics
            .stuck_patterns
            .iter()
            .map(|item| StuckPattern {
                description: item.description.clone(),
                severity: if item.severity == "critical" {
                    "critical"
                } else {
                    "warning"
                },
            }),
    );
    let loop_cost = session.diagnostics.loop_cost.total_loop_cost;
    let loop_percent = loop_waste_percent(loop_cost, session.metrics.cost_estimated);
    let mut total_wasted = cache.wasted_cost + loop_cost;
    if bloat.bloat_score > 50 {
        total_wasted += session.metrics.cost_estimated * 0.05;
    }

    let mut score = match cache.rating {
        "none" => 20.0,
        "poor" => 15.0,
        "good" => 5.0,
        _ => 0.0,
    };
    score += bloat.bloat_score as f64 * 0.25;
    score += loop_percent * 0.6;
    if score > 30.0 {
        score = 30.0;
    }
    let mut stuck_score = stuck.len() as f64 * 7.0;
    for item in &stuck {
        if item.severity == "critical" {
            stuck_score += 5.0;
        }
    }
    if stuck_score > 20.0 {
        stuck_score = 20.0;
    }
    score += stuck_score;
    if session.metrics.tokens_cache_r > 0
        && session.metrics.tokens_input > 0
        && session.metrics.tokens_cache_r as f64 / (session.metrics.tokens_input as f64) < 0.3
    {
        score += 6.0;
    }
    let waste_score = (score as i32).clamp(0, 100);
    let waste_level = match waste_score {
        70.. => "red",
        40..=69 => "orange",
        15..=39 => "yellow",
        _ => "green",
    };
    let summary = match waste_level {
        "green" => "efficient session - no significant waste".to_string(),
        "yellow" => format!(
            "minor waste - cache {:.0}% hit, room for optimization",
            cache.hit_rate
        ),
        "orange" => format!(
            "wasting ${:.2}: loops {:.0}%, tools {:.1}/turn",
            total_wasted, loop_percent, bloat.tools_per_turn
        ),
        "red" => format!(
            "severe waste ${:.2}: loops {:.0}%, {} stuck, no cache",
            total_wasted,
            loop_percent,
            stuck.len()
        ),
        _ => String::new(),
    };

    let mut top_actions = Vec::new();
    if cache.rating == "none" || cache.rating == "poor" {
        top_actions.push(cache.suggestion.to_string());
    }
    if bloat.bloat_level == "severe" || bloat.bloat_level == "high" {
        if let Some(top) = bloat.top_bloat.first() {
            top_actions.push(format!(
                "top tool {:?} called {}x - reduce or batch",
                top.tool_name, top.call_count
            ));
        } else {
            top_actions.push(bloat_suggestion(bloat.bloat_level).to_string());
        }
    }
    if loop_percent > 20.0 {
        top_actions.push(format!(
            "loop waste ${:.2} ({:.0}%) - add max retries limit",
            loop_cost, loop_percent
        ));
    }
    if top_actions.is_empty() {
        top_actions.push("session running optimally".to_string());
    }

    WasteReport {
        cache,
        bloat,
        stuck,
        waste_score,
        waste_level,
        total_wasted,
        loop_percent,
        summary,
        top_actions,
    }
}

pub fn render_waste_report(session: &Session) -> String {
    render_waste_report_with_language(session, ReportLanguage::En)
}

pub fn render_waste_report_with_language(session: &Session, language: ReportLanguage) -> String {
    waste_report_text(&compute_waste_report(session), language)
}

fn analyze_cache_efficiency(metrics: &Metrics) -> CacheEfficiency {
    let hit_rate = if metrics.tokens_input > 0 {
        metrics.tokens_cache_r as f64 / metrics.tokens_input as f64 * 100.0
    } else {
        0.0
    };
    let wasted_tokens = (metrics.tokens_input - metrics.tokens_cache_r).max(0);
    let price = pricing::lookup_price(&metrics.model_used);
    let wasted_cost = round4(wasted_tokens as f64 / 1e6 * price.input);
    let (rating, suggestion) = if hit_rate >= 80.0 {
        (
            "excellent",
            "cache utilization excellent - keep current prompt structure",
        )
    } else if hit_rate >= 40.0 {
        (
            "good",
            "moderate cache hit - place static system instructions at prompt prefix",
        )
    } else if metrics.tokens_cache_w > 0 {
        (
            "poor",
            "low cache hit rate - enable prompt caching with static prefix content",
        )
    } else {
        (
            "none",
            "caching not enabled - enable Anthropic prompt caching to save up to 90% on input cost",
        )
    };
    CacheEfficiency {
        cache_read_tokens: metrics.tokens_cache_r,
        total_input_tokens: metrics.tokens_input,
        hit_rate,
        wasted_cost,
        rating,
        suggestion,
    }
}

fn analyze_tool_bloat(metrics: &Metrics) -> ToolBloatAnalysis {
    let tools_per_turn = if metrics.assistant_turns > 0 {
        metrics.tool_calls_total as f64 / metrics.assistant_turns as f64
    } else {
        0.0
    };
    let avg_cost_per_turn = if metrics.assistant_turns > 0 && metrics.cost_estimated > 0.0 {
        metrics.cost_estimated / metrics.assistant_turns as f64
    } else {
        0.0
    };
    let (bloat_score, bloat_level) = if tools_per_turn > 5.0 {
        (90, "severe")
    } else if tools_per_turn > 3.0 {
        (65, "high")
    } else if tools_per_turn > 1.5 {
        (35, "medium")
    } else {
        (10, "low")
    };
    let mut tools = metrics.tool_usage.iter().collect::<Vec<_>>();
    tools.sort_by(|a, b| b.1.cmp(a.1));
    let top_bloat = tools
        .into_iter()
        .take(5)
        .map(|(tool_name, call_count)| ToolBloatItem {
            tool_name: tool_name.clone(),
            call_count: *call_count,
            total_cost: avg_cost_per_turn * *call_count as f64,
            is_redundant: *call_count > metrics.assistant_turns && metrics.assistant_turns > 0,
        })
        .collect();
    ToolBloatAnalysis {
        tools_per_turn,
        bloat_score,
        bloat_level,
        top_bloat,
    }
}

fn detect_stuck_from_metrics(metrics: &Metrics) -> Vec<StuckPattern> {
    let long_gaps = metrics.gaps_sec.iter().filter(|gap| **gap > 120.0).count();
    if long_gaps >= 3 {
        vec![StuckPattern {
            description: format!("{long_gaps} gaps >120s - agent appears stuck"),
            severity: "critical",
        }]
    } else {
        Vec::new()
    }
}

fn waste_report_text(report: &WasteReport, language: ReportLanguage) -> String {
    let sep = "━".repeat(60);
    let mut out = String::new();
    out.push_str(&sep);
    out.push('\n');
    out.push_str(&format!(
        "  AGENTTRACE v{} - {}\n",
        VERSION,
        t(language, "Waste Analysis", "浪费分析")
    ));
    out.push_str(&sep);
    out.push('\n');
    out.push('\n');
    out.push_str(&format!(
        "  {}: {}/100 ({} {})\n",
        t(language, "Score", "评分"),
        report.waste_score,
        level_emoji(report.waste_level),
        waste_level_label(report.waste_level, language)
    ));
    out.push_str(&format!(
        "  {}: {}\n",
        t(language, "Wasted", "浪费成本"),
        format_cost(report.total_wasted)
    ));
    out.push_str(&format!("  {}\n", waste_summary(report, language)));
    out.push('\n');
    out.push_str(t(language, "  -- Cache --\n", "  -- 缓存 --\n"));
    out.push_str(&format!(
        "  {} ({} {:.0}%, {} {} / {} {})\n",
        cache_rating_label(report.cache.rating, language),
        t(language, "hit", "命中"),
        report.cache.hit_rate,
        format_tokens(report.cache.cache_read_tokens),
        t(language, "read", "读取"),
        format_tokens(report.cache.total_input_tokens),
        t(language, "input", "输入")
    ));
    if report.cache.wasted_cost > 0.0 {
        out.push_str(&format!(
            "  {}: {}\n",
            t(language, "Cache waste", "缓存浪费"),
            format_cost(report.cache.wasted_cost)
        ));
    }
    out.push_str(&format!(
        "  {}: {}\n",
        t(language, "Suggestion", "建议"),
        cache_suggestion(report.cache.rating, language)
    ));
    out.push('\n');
    out.push_str(t(language, "  -- Tool Bloat --\n", "  -- 工具膨胀 --\n"));
    out.push_str(&format!(
        "  {} ({:.1} {})\n",
        bloat_level_label(report.bloat.bloat_level, language),
        report.bloat.tools_per_turn,
        t(language, "tools/turn", "工具/轮")
    ));
    for item in &report.bloat.top_bloat {
        let redundant = if item.is_redundant {
            t(language, " *redundant", " *冗余")
        } else {
            ""
        };
        out.push_str(&format!(
            "    {:<25} {:>3}x {}{}\n",
            item.tool_name,
            item.call_count,
            format_cost(item.total_cost),
            redundant
        ));
    }
    out.push('\n');
    out.push_str(t(language, "  -- Stuck --\n", "  -- 卡住 --\n"));
    if report.stuck.is_empty() {
        out.push_str(t(language, "  none\n", "  无\n"));
    } else {
        for stuck in &report.stuck {
            out.push_str(&format!(
                "  [{}] {}\n",
                severity_label(stuck.severity, language),
                stuck_description(&stuck.description, language)
            ));
        }
    }
    out.push('\n');
    out.push_str(t(language, "  -- Actions --\n", "  -- 建议动作 --\n"));
    for (index, action) in report.top_actions.iter().enumerate() {
        out.push_str(&format!(
            "  {}. {}\n",
            index + 1,
            action_text(action, language)
        ));
    }
    out.push('\n');
    out.push_str(&sep);
    out.push('\n');
    out
}

fn t(language: ReportLanguage, en: &'static str, zh: &'static str) -> &'static str {
    match language {
        ReportLanguage::En => en,
        ReportLanguage::Zh => zh,
    }
}

fn waste_summary(report: &WasteReport, language: ReportLanguage) -> String {
    if language == ReportLanguage::En {
        return report.summary.clone();
    }
    match report.waste_level {
        "green" => "会话效率良好，未发现明显浪费".to_string(),
        "yellow" => format!(
            "轻微浪费：缓存命中率 {:.0}%，仍有优化空间",
            report.cache.hit_rate
        ),
        "orange" => format!(
            "浪费 ${:.2}：循环 {:.0}%，每轮工具调用 {:.1} 次",
            report.total_wasted, report.loop_percent, report.bloat.tools_per_turn
        ),
        _ => format!(
            "严重浪费 ${:.2}：{} 个卡住信号，且未有效使用缓存",
            report.total_wasted,
            report.stuck.len()
        ),
    }
}

fn cache_suggestion(rating: &str, language: ReportLanguage) -> &'static str {
    if language == ReportLanguage::En {
        return match rating {
            "excellent" => "cache utilization excellent - keep current prompt structure",
            "good" => "moderate cache hit - place static system instructions at prompt prefix",
            "poor" => "low cache hit rate - enable prompt caching with static prefix content",
            _ => "caching not enabled - enable Anthropic prompt caching to save up to 90% on input cost",
        };
    }
    match rating {
        "excellent" => "缓存利用率优秀，保持当前提示词结构",
        "good" => "缓存命中率中等，将静态系统指令放在提示词前缀",
        "poor" => "缓存命中率较低，使用静态前缀启用提示词缓存",
        _ => "未启用缓存，启用提示词缓存可降低输入成本",
    }
}

fn severity_label(severity: &str, language: ReportLanguage) -> &str {
    if language == ReportLanguage::En {
        return severity;
    }
    match severity {
        "critical" => "严重",
        "warning" => "警告",
        "high" => "高",
        "medium" => "中",
        _ => severity,
    }
}

fn stuck_description(description: &str, language: ReportLanguage) -> String {
    if language == ReportLanguage::En {
        return description.to_string();
    }
    description
        .replace(
            " gaps >120s - agent appears stuck",
            " 个间隔超过 120 秒，智能体可能卡住",
        )
        .replace(" gaps exceed 120s", " 个间隔超过 120 秒")
        .replace("Repeated assistant response ", "助手重复响应 ")
        .replace(" times", " 次")
        .replace(" tool calls have no result", " 个工具调用没有结果")
}

fn action_text(action: &str, language: ReportLanguage) -> String {
    if language == ReportLanguage::En {
        return action.to_string();
    }
    match action {
        "cache utilization excellent - keep current prompt structure" => {
            "缓存利用率优秀，保持当前提示词结构".to_string()
        }
        "moderate cache hit - place static system instructions at prompt prefix" => {
            "缓存命中率中等，将静态系统指令放在提示词前缀".to_string()
        }
        "low cache hit rate - enable prompt caching with static prefix content" => {
            "缓存命中率较低，使用静态前缀启用提示词缓存".to_string()
        }
        "caching not enabled - enable Anthropic prompt caching to save up to 90% on input cost" => {
            "未启用缓存，启用提示词缓存可降低输入成本".to_string()
        }
        "session running optimally" => "会话运行良好".to_string(),
        _ if action.starts_with("top tool ") => action
            .replace("top tool ", "最高频工具 ")
            .replace(" called ", " 调用 ")
            .replace("x - reduce or batch", " 次，请减少调用或批处理"),
        _ if action.starts_with("loop waste ") => action
            .replace("loop waste ", "循环浪费 ")
            .replace(" - add max retries limit", "，请限制最大重试次数"),
        _ => action.to_string(),
    }
}

fn cache_rating_label(rating: &str, language: ReportLanguage) -> &'static str {
    match rating {
        "excellent" => t(language, "excellent", "优秀"),
        "good" => t(language, "good", "良好"),
        "poor" => t(language, "poor", "较差"),
        _ => t(language, "none", "未启用"),
    }
}

fn bloat_level_label(level: &str, language: ReportLanguage) -> &'static str {
    match level {
        "severe" => t(language, "severe", "严重"),
        "high" => t(language, "high", "高"),
        "medium" => t(language, "medium", "中"),
        _ => t(language, "low", "低"),
    }
}

fn bloat_suggestion(level: &str) -> &'static str {
    match level {
        "severe" => "severe tool bloat: limit max tool calls per turn or split into smaller tasks",
        "high" => "too many tool calls: check if simple tasks use over-complex agent orchestration",
        "medium" => "moderate tool usage: watch for unnecessary tool call patterns",
        _ => "tool usage is lean",
    }
}

fn waste_level_label(level: &str, language: ReportLanguage) -> &'static str {
    match level {
        "red" => t(language, "SEVERE", "严重"),
        "orange" => t(language, "HIGH", "高"),
        "yellow" => t(language, "MODERATE", "中"),
        _ => t(language, "LOW", "低"),
    }
}

fn level_emoji(level: &str) -> &'static str {
    match level {
        "red" => "🔴",
        "orange" => "🟠",
        "yellow" => "🟡",
        _ => "🟢",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waste_report_supports_chinese() {
        let session = Session {
            name: "会话".to_string(),
            path: "/tmp/session.jsonl".to_string(),
            cwd: String::new(),
            metrics: Metrics::default(),
            anomalies: Vec::new(),
            health: 100,
            tool_warnings: Vec::new(),
            diagnostics: crate::Diagnostics::default(),
        };

        let report = render_waste_report_with_language(&session, ReportLanguage::Zh);
        assert!(report.contains("浪费分析"));
        assert!(report.contains("建议动作"));
        assert!(!report.contains("Waste Analysis"));
    }
}
