use crate::i18n::{tr, Message};
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
    description: Message,
    english: String,
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
    summary: Message,
    top_actions: Vec<Message>,
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
                description: item.i18n.clone(),
                english: item.description.clone(),
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
        "green" => Message::new("waste.msg.summary_green"),
        "yellow" => {
            Message::new("waste.msg.summary_yellow").arg("hit", format!("{:.0}", cache.hit_rate))
        }
        "orange" => Message::new("waste.msg.summary_orange")
            .arg("wasted", format!("{total_wasted:.2}"))
            .arg("loops", format!("{loop_percent:.0}"))
            .arg("tools", format!("{:.1}", bloat.tools_per_turn)),
        _ => Message::new("waste.msg.summary_red")
            .arg("wasted", format!("{total_wasted:.2}"))
            .arg("loops", format!("{loop_percent:.0}"))
            .arg("stuck", stuck.len()),
    };

    let mut top_actions = Vec::new();
    if cache.rating == "none" || cache.rating == "poor" {
        top_actions.push(cache_suggestion(cache.rating));
    }
    if bloat.bloat_level == "severe" || bloat.bloat_level == "high" {
        if let Some(top) = bloat.top_bloat.first() {
            top_actions.push(
                Message::new("waste.msg.top_tool")
                    .arg("tool", format!("{:?}", top.tool_name))
                    .arg("count", top.call_count),
            );
        } else {
            top_actions.push(bloat_suggestion(bloat.bloat_level));
        }
    }
    if loop_percent > 20.0 {
        top_actions.push(
            Message::new("waste.msg.loop_waste")
                .arg("cost", format!("{loop_cost:.2}"))
                .arg("pct", format!("{loop_percent:.0}")),
        );
    }
    if top_actions.is_empty() {
        top_actions.push(Message::new("waste.msg.optimal"));
    }

    WasteReport {
        cache,
        bloat,
        stuck,
        waste_score,
        waste_level,
        total_wasted,
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
    let rating = if hit_rate >= 80.0 {
        "excellent"
    } else if hit_rate >= 40.0 {
        "good"
    } else if metrics.tokens_cache_w > 0 {
        "poor"
    } else {
        "none"
    };
    CacheEfficiency {
        cache_read_tokens: metrics.tokens_cache_r,
        total_input_tokens: metrics.tokens_input,
        hit_rate,
        wasted_cost,
        rating,
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
        let description = Message::new("waste.msg.stuck_gaps").arg("count", long_gaps);
        vec![StuckPattern {
            english: description.render_or(ReportLanguage::En, ""),
            description,
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
        tr(language, "waste.waste_analysis")
    ));
    out.push_str(&sep);
    out.push('\n');
    out.push('\n');
    out.push_str(&format!(
        "  {}: {}/100 ({} {})\n",
        tr(language, "waste.score"),
        report.waste_score,
        level_emoji(report.waste_level),
        waste_level_label(report.waste_level, language)
    ));
    out.push_str(&format!(
        "  {}: {}\n",
        tr(language, "waste.wasted"),
        format_cost(report.total_wasted)
    ));
    out.push_str(&format!("  {}\n", waste_summary(report, language)));
    out.push('\n');
    out.push_str(tr(language, "waste.cache"));
    out.push_str(&format!(
        "  {} ({} {:.0}%, {} {} / {} {})\n",
        cache_rating_label(report.cache.rating, language),
        tr(language, "waste.hit"),
        report.cache.hit_rate,
        format_tokens(report.cache.cache_read_tokens),
        tr(language, "waste.read"),
        format_tokens(report.cache.total_input_tokens),
        tr(language, "waste.input")
    ));
    if report.cache.wasted_cost > 0.0 {
        out.push_str(&format!(
            "  {}: {}\n",
            tr(language, "waste.cache_waste"),
            format_cost(report.cache.wasted_cost)
        ));
    }
    out.push_str(&format!(
        "  {}: {}\n",
        tr(language, "waste.suggestion"),
        cache_suggestion(report.cache.rating).render_or(language, "")
    ));
    out.push('\n');
    out.push_str(tr(language, "waste.tool_bloat"));
    out.push_str(&format!(
        "  {} ({:.1} {})\n",
        bloat_level_label(report.bloat.bloat_level, language),
        report.bloat.tools_per_turn,
        tr(language, "waste.tools_turn")
    ));
    for item in &report.bloat.top_bloat {
        let redundant = if item.is_redundant {
            tr(language, "waste.redundant")
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
    out.push_str(tr(language, "waste.stuck"));
    if report.stuck.is_empty() {
        out.push_str(tr(language, "waste.none"));
    } else {
        for stuck in &report.stuck {
            out.push_str(&format!(
                "  [{}] {}\n",
                severity_label(stuck.severity, language),
                stuck.description.render_or(language, &stuck.english)
            ));
        }
    }
    out.push('\n');
    out.push_str(tr(language, "waste.actions"));
    for (index, action) in report.top_actions.iter().enumerate() {
        out.push_str(&format!(
            "  {}. {}\n",
            index + 1,
            action.render_or(language, "")
        ));
    }
    out.push('\n');
    out.push_str(&sep);
    out.push('\n');
    out
}

fn waste_summary(report: &WasteReport, language: ReportLanguage) -> String {
    report.summary.render_or(language, "")
}

fn cache_suggestion(rating: &str) -> Message {
    Message::new(match rating {
        "excellent" => "waste.msg.cache_excellent",
        "good" => "waste.msg.cache_good",
        "poor" => "waste.msg.cache_poor",
        _ => "waste.msg.cache_none",
    })
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

fn cache_rating_label(rating: &str, language: ReportLanguage) -> &'static str {
    match rating {
        "excellent" => tr(language, "waste.excellent"),
        "good" => tr(language, "waste.good"),
        "poor" => tr(language, "waste.poor"),
        _ => tr(language, "waste.none_2"),
    }
}

fn bloat_level_label(level: &str, language: ReportLanguage) -> &'static str {
    match level {
        "severe" => tr(language, "waste.severe"),
        "high" => tr(language, "waste.high"),
        "medium" => tr(language, "waste.medium"),
        _ => tr(language, "waste.low"),
    }
}

fn bloat_suggestion(level: &str) -> Message {
    Message::new(match level {
        "severe" => "waste.msg.bloat_severe",
        "high" => "waste.msg.bloat_high",
        "medium" => "waste.msg.bloat_medium",
        _ => "waste.msg.bloat_lean",
    })
}

fn waste_level_label(level: &str, language: ReportLanguage) -> &'static str {
    match level {
        "red" => tr(language, "waste.severe_2"),
        "orange" => tr(language, "waste.high_2"),
        "yellow" => tr(language, "waste.moderate"),
        _ => tr(language, "waste.low_2"),
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
