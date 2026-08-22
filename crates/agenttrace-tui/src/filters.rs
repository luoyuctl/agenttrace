#![cfg_attr(not(test), allow(dead_code))]

use super::*;

pub(super) fn session_matches(session: &Session, query: &str) -> bool {
    contains(&session.name, query)
        || contains(&session.path, query)
        || contains(&session.cwd, query)
        || contains(&session.metrics.source_tool, query)
        || contains(&display_session_source(session), query)
        || contains(&session.metrics.model_used, query)
        || session
            .metrics
            .tool_usage
            .keys()
            .any(|tool| contains(tool, query))
        || session
            .metrics
            .file_usage
            .keys()
            .any(|file| contains(file, query))
        || session
            .anomalies
            .iter()
            .any(|anomaly| contains(&anomaly.kind, query) || contains(&anomaly.detail, query))
}

pub(super) fn matches_text_filter(value: &str, filter: &str) -> bool {
    let filter = filter.trim().to_ascii_lowercase();
    filter.is_empty() || contains(value, &filter)
}

pub(super) fn matches_source_filter(session: &Session, filter: &str) -> bool {
    let filter = filter.trim().to_ascii_lowercase();
    filter.is_empty()
        || contains(&session.metrics.source_tool, &filter)
        || contains(&display_session_source(session), &filter)
}

pub(super) fn matches_health_filter(session: &Session, filter: &str) -> bool {
    let filter = filter.trim().to_ascii_lowercase();
    if filter.is_empty() {
        return true;
    }
    match filter.as_str() {
        "good" | "healthy" => session.health >= 80,
        "warn" | "warning" => (50..80).contains(&session.health),
        "crit" | "critical" => session.health < 50,
        _ => parse_numeric_i32_filter(&filter)
            .map(|(op, value)| compare_i32(session.health, op, value))
            .unwrap_or(false),
    }
}

pub(super) fn parse_health_filter(filter: &str) -> Option<()> {
    let filter = filter.trim().to_ascii_lowercase();
    match filter.as_str() {
        "good" | "healthy" | "warn" | "warning" | "crit" | "critical" => Some(()),
        _ => parse_numeric_i32_filter(&filter).map(|_| ()),
    }
}

pub(super) fn matches_cost_filter(session: &Session, filter: Option<(CostOp, f64)>) -> bool {
    let Some((op, value)) = filter else {
        return true;
    };
    compare_f64(session.metrics.cost_estimated, op, value)
}

pub(super) fn matches_anomaly_filter(session: &Session, filter: Option<&str>) -> bool {
    let Some(filter) = filter else {
        return true;
    };
    let filter = filter.trim().to_ascii_lowercase();
    if filter.is_empty() {
        return !session.anomalies.is_empty();
    }
    session
        .anomalies
        .iter()
        .any(|anomaly| contains(&anomaly.kind, &filter) || contains(&anomaly.detail, &filter))
}

pub(super) fn matches_issue_filter(session: &Session, filter: &str) -> bool {
    match filter {
        "failures" => session.metrics.tool_calls_fail > 0,
        "stuck" => !session.diagnostics.stuck_patterns.is_empty(),
        "context" => matches!(
            session.diagnostics.context_utilization.risk_level.as_str(),
            "warning" | "critical"
        ),
        "loops" => session.diagnostics.loop_cost.loop_groups > 0,
        _ => true,
    }
}

pub(super) fn parse_numeric_i32_filter(filter: &str) -> Option<(CostOp, i32)> {
    let (op, value) = parse_operator_value(filter)?;
    value.parse::<i32>().ok().map(|value| (op, value))
}

pub(super) fn parse_cost_filter(filter: &str) -> Option<(CostOp, f64)> {
    let (op, value) = parse_operator_value(filter.trim())?;
    value.parse::<f64>().ok().map(|value| (op, value))
}

pub(super) fn parse_operator_value(filter: &str) -> Option<(CostOp, &str)> {
    let filter = filter.trim();
    for (prefix, op) in [
        (">=", CostOp::Gte),
        ("<=", CostOp::Lte),
        (">", CostOp::Gt),
        ("<", CostOp::Lt),
        ("=", CostOp::Eq),
    ] {
        if let Some(value) = filter.strip_prefix(prefix) {
            return Some((op, value.trim()));
        }
    }
    filter.parse::<f64>().ok().map(|_| (CostOp::Gte, filter))
}

pub(super) fn compare_i32(left: i32, op: CostOp, right: i32) -> bool {
    match op {
        CostOp::Gt => left > right,
        CostOp::Gte => left >= right,
        CostOp::Lt => left < right,
        CostOp::Lte => left <= right,
        CostOp::Eq => left == right,
    }
}

pub(super) fn compare_f64(left: f64, op: CostOp, right: f64) -> bool {
    match op {
        CostOp::Gt => left > right,
        CostOp::Gte => left >= right,
        CostOp::Lt => left < right,
        CostOp::Lte => left <= right,
        CostOp::Eq => (left - right).abs() < f64::EPSILON,
    }
}

pub(super) fn command_value(command: &str) -> String {
    command
        .split_once(char::is_whitespace)
        .map(|(_, value)| value.trim().to_string())
        .unwrap_or_default()
}

pub(super) fn parse_sort_key(value: &str) -> Option<SortKey> {
    match value.trim().to_ascii_lowercase().as_str() {
        "recent" | "time" => Some(SortKey::Recent),
        "health" => Some(SortKey::Health),
        "cost" => Some(SortKey::Cost),
        "turn" | "turns" => Some(SortKey::Turns),
        "fail" | "fails" | "failure" | "failures" | "errors" => Some(SortKey::Failures),
        "source" | "agent" => Some(SortKey::Source),
        "name" | "session" => Some(SortKey::Name),
        "anom" | "anomaly" | "anomalies" => Some(SortKey::Anomalies),
        _ => None,
    }
}

pub(super) fn active_filter_summary(app: &App, language: Language) -> String {
    let mut filters = Vec::new();
    let label = |en, zh| text(language, en, zh);
    if !app.query.is_empty() {
        filters.push(format!("{}: {}", label("text", "关键词"), app.query));
    }
    if !app.health_filter.is_empty() {
        filters.push(format!(
            "{}: {}",
            label("health", "健康度"),
            health_filter_label(&app.health_filter, language)
        ));
    }
    if !app.source_filter.is_empty() {
        filters.push(format!(
            "{}: {}",
            label("source", "来源"),
            app.source_filter
        ));
    }
    if !app.model_filter.is_empty() {
        filters.push(format!("{}: {}", label("model", "模型"), app.model_filter));
    }
    if !app.project_filter.is_empty() {
        filters.push(format!(
            "{}: {}",
            label("project", "项目"),
            app.project_filter
        ));
    }
    if app.range_filter != TimeRange::All {
        filters.push(format!(
            "{}: {}",
            label("range", "时间范围"),
            range_label(app.range_filter, language)
        ));
    }
    if let Some((op, value)) = app.cost_filter {
        filters.push(format!(
            "{}: {}{}",
            label("cost", "花费"),
            cost_op_label(op),
            value
        ));
    }
    if let Some((op, value)) = app.failure_filter {
        filters.push(format!(
            "{}: {}{}",
            label("failed", "失败次数"),
            cost_op_label(op),
            value
        ));
    }
    if let Some((op, value)) = app.context_filter {
        filters.push(format!(
            "{}: {}{}%",
            label("context", "上下文"),
            cost_op_label(op),
            value
        ));
    }
    if let Some(value) = &app.anomaly_filter {
        let value = if value.is_empty() {
            label("any", "全部")
        } else {
            value
        };
        filters.push(format!("{}: {value}", label("anomaly", "异常")));
    }
    if !app.capability_filter.is_empty() {
        filters.push(format!(
            "{}: {}",
            label("data", "数据完整度"),
            capability_filter_label(&app.capability_filter, language)
        ));
    }
    if !app.issue_filter.is_empty() {
        filters.push(format!(
            "{}: {}",
            label("issue", "问题"),
            issue_filter_label(&app.issue_filter, language)
        ));
    }
    filters.join(" · ")
}

fn capability_filter_label(value: &str, language: Language) -> &'static str {
    match value {
        "detailed" => text(language, "detailed", "详细"),
        "aggregate" => text(language, "aggregate", "聚合"),
        _ => text(language, "limited", "有限"),
    }
}

fn issue_filter_label(value: &str, language: Language) -> String {
    match value {
        "failures" => text(language, "tool failures", "工具失败"),
        "stuck" => text(language, "stuck", "卡住"),
        "context" => text(language, "context pressure", "上下文压力"),
        "loops" => text(language, "repeat loops", "重复循环"),
        _ => value,
    }
    .to_string()
}

pub(super) fn capability_label(session: &Session, language: Language) -> &'static str {
    match session_capability(session) {
        "detailed" => text(language, "Detailed", "详细"),
        "aggregate" => text(language, "Aggregate", "聚合"),
        _ => text(language, "Limited", "有限"),
    }
}

pub(super) fn evidence_confidence(session: &Session, language: Language) -> String {
    match session_capability(session) {
        "detailed" => format!(
            "{} — {}",
            text(language, "high", "高"),
            text(
                language,
                "event timing and diagnostics available",
                "有事件时间与诊断数据"
            )
        ),
        "aggregate" => format!(
            "{} — {}",
            text(language, "medium", "中"),
            text(language, "aggregate metrics only", "仅有聚合指标")
        ),
        _ => format!(
            "{} — {}",
            text(language, "low", "低"),
            text(language, "key fields missing", "关键字段缺失")
        ),
    }
}

pub(super) fn cost_op_label(op: CostOp) -> &'static str {
    match op {
        CostOp::Gt => ">",
        CostOp::Gte => ">=",
        CostOp::Lt => "<",
        CostOp::Lte => "<=",
        CostOp::Eq => "=",
    }
}

pub(super) fn compare_sessions(a: &Session, b: &Session, key: SortKey, desc: bool) -> Ordering {
    let ord = match key {
        SortKey::Recent => a
            .metrics
            .session_start
            .cmp(&b.metrics.session_start)
            .then_with(|| a.name.cmp(&b.name)),
        SortKey::Health => a.health.cmp(&b.health).then_with(|| a.name.cmp(&b.name)),
        SortKey::Cost => cmp_f64(a.metrics.cost_estimated, b.metrics.cost_estimated)
            .then_with(|| a.name.cmp(&b.name)),
        SortKey::Turns => a
            .metrics
            .assistant_turns
            .cmp(&b.metrics.assistant_turns)
            .then_with(|| a.name.cmp(&b.name)),
        SortKey::Failures => a
            .metrics
            .tool_calls_fail
            .cmp(&b.metrics.tool_calls_fail)
            .then_with(|| a.name.cmp(&b.name)),
        SortKey::Source => a
            .metrics
            .source_tool
            .cmp(&b.metrics.source_tool)
            .then_with(|| a.name.cmp(&b.name)),
        SortKey::Name => a.name.cmp(&b.name),
        SortKey::Anomalies => a
            .anomalies
            .len()
            .cmp(&b.anomalies.len())
            .then_with(|| a.name.cmp(&b.name)),
    };
    if desc {
        ord.reverse()
    } else {
        ord
    }
}

pub(super) fn cmp_f64(a: f64, b: f64) -> Ordering {
    a.partial_cmp(&b).unwrap_or(Ordering::Equal)
}

pub(super) fn contains(value: &str, query: &str) -> bool {
    value.to_ascii_lowercase().contains(query)
}

pub(super) fn status_width(width: u16) -> usize {
    if width >= 118 {
        64
    } else if width >= 84 {
        40
    } else {
        24
    }
}

pub(super) fn short(value: &str, max: usize) -> String {
    use unicode_width::UnicodeWidthChar;

    if unicode_width::UnicodeWidthStr::width(value) <= max {
        return value.to_string();
    }
    if max <= 3 {
        let mut out = String::new();
        let mut width = 0;
        for ch in value.chars() {
            let ch_width = ch.width().unwrap_or(0);
            if width + ch_width > max {
                break;
            }
            out.push(ch);
            width += ch_width;
        }
        return out;
    }
    let mut out = String::new();
    let mut width = 0;
    for ch in value.chars() {
        let ch_width = ch.width().unwrap_or(0);
        if width + ch_width > max - 3 {
            break;
        }
        out.push(ch);
        width += ch_width;
    }
    out.push_str("...");
    out
}

pub(super) fn pad_display_width(value: &str, width: usize) -> String {
    let mut out = short(value, width);
    let padding = width.saturating_sub(unicode_width::UnicodeWidthStr::width(out.as_str()));
    out.push_str(&" ".repeat(padding));
    out
}

pub(super) fn terminal_safe_report(text: &str) -> String {
    text.chars()
        .map(|ch| match ch {
            '\n' | '\t' => ch,
            '━' | '─' | '═' | '—' | '–' => '-',
            '│' | '┃' => '|',
            '┌' | '┐' | '└' | '┘' | '┬' | '┴' | '├' | '┤' | '┼' => '+',
            _ => ch,
        })
        .collect()
}
