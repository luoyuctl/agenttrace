use crate::{parse_ts, pricing, Session};
use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SessionComparison {
    pub outcome: &'static str,
    pub reasons: Vec<&'static str>,
}

pub fn compare_session_outcome(current: &Session, previous: &Session) -> SessionComparison {
    let outcome = match (
        current.metrics.duration_sec <= previous.metrics.duration_sec,
        current.metrics.cost_estimated <= previous.metrics.cost_estimated,
    ) {
        (true, true) => "faster_cheaper",
        (true, false) => "faster_costlier",
        (false, true) => "slower_cheaper",
        (false, false) => "slower_costlier",
    };
    let mut reasons = Vec::new();
    if current.metrics.tool_calls_fail < previous.metrics.tool_calls_fail {
        reasons.push("fewer_failures");
    }
    if current.diagnostics.loop_cost.total_loop_cost
        < previous.diagnostics.loop_cost.total_loop_cost
    {
        reasons.push("less_repeated_work");
    }
    if reasons.is_empty() {
        reasons.push("review_metric_changes");
    }
    SessionComparison { outcome, reasons }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TimeRange {
    Today,
    Days7,
    Days30,
    #[default]
    All,
}

impl TimeRange {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "today" | "day" | "1d" => Some(Self::Today),
            "7d" | "week" | "weekly" => Some(Self::Days7),
            "30d" | "month" | "monthly" => Some(Self::Days30),
            "all" | "" => Some(Self::All),
            _ => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Today => "today",
            Self::Days7 => "7d",
            Self::Days30 => "30d",
            Self::All => "all",
        }
    }
    pub fn since(self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        match self {
            Self::Today => now
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .map(|value| value.and_utc()),
            Self::Days7 => Some(now - chrono::Duration::days(7)),
            Self::Days30 => Some(now - chrono::Duration::days(30)),
            Self::All => None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DataHealth {
    pub discovered: usize,
    pub parsed: usize,
    pub skipped: usize,
    pub cache_hits: usize,
    pub unknown_sources: usize,
    pub unknown_models: usize,
    pub fallback_pricing: usize,
    pub latest_session_at: String,
    pub confidence: String,
    pub with_tokens: usize,
    pub with_duration: usize,
    pub with_tools: usize,
    pub with_event_timing: usize,
    pub with_diagnostics: usize,
}

pub fn session_capability(session: &Session) -> &'static str {
    let m = &session.metrics;
    if !m.gaps_sec.is_empty()
        || !session.diagnostics.tool_latencies.is_empty()
        || session.diagnostics.context_utilization.estimated_total > 0
    {
        "detailed"
    } else if m.duration_sec > 0.0
        || m.tokens_input + m.tokens_output > 0
        || m.tool_calls_total > 0
        || !matches!(m.model_used.as_str(), "" | "default" | "unknown")
    {
        "aggregate"
    } else {
        "limited"
    }
}

pub fn project_name(session: &Session) -> String {
    let raw = if session.cwd.trim().is_empty()
        && (session.path.contains("://") || !session.path.contains(['/', '\\']))
    {
        "unknown"
    } else if session.cwd.trim().is_empty() {
        std::path::Path::new(&session.path)
            .parent()
            .and_then(std::path::Path::file_name)
            .and_then(|value| value.to_str())
            .unwrap_or("unknown")
    } else {
        std::path::Path::new(&session.cwd)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(&session.cwd)
    };
    if raw.trim().is_empty() {
        "unknown".to_string()
    } else {
        raw.to_string()
    }
}

pub fn filter_sessions(
    sessions: &[Session],
    range: TimeRange,
    project: &str,
    source: &str,
    model: &str,
    now: DateTime<Utc>,
) -> Vec<Session> {
    sessions
        .iter()
        .filter(|session| {
            session_matches_time_range(session, range, now)
                && contains(&project_name(session), project)
                && contains(&session.metrics.source_tool, source)
                && contains(&session.metrics.model_used, model)
        })
        .cloned()
        .collect()
}
pub fn session_matches_time_range(session: &Session, range: TimeRange, now: DateTime<Utc>) -> bool {
    range.since(now).map_or(true, |since| {
        parse_ts(&session.metrics.session_start).is_some_and(|time| time >= since)
    })
}

pub fn data_health(sessions: &[Session], discovered: usize, cache_hits: usize) -> DataHealth {
    let parsed = sessions.len();
    let unknown_sources = sessions
        .iter()
        .filter(|s| matches!(s.metrics.source_tool.as_str(), "" | "generic" | "unknown"))
        .count();
    let unknown_models = sessions
        .iter()
        .filter(|s| matches!(s.metrics.model_used.as_str(), "" | "default" | "unknown"))
        .count();
    let fallback_pricing = sessions
        .iter()
        .filter(|s| !pricing::has_specific_price(&s.metrics.model_used))
        .count();
    let skipped = discovered.saturating_sub(parsed);
    DataHealth {
        discovered,
        parsed,
        skipped,
        cache_hits,
        unknown_sources,
        unknown_models,
        fallback_pricing,
        latest_session_at: sessions
            .iter()
            .filter_map(|s| parse_ts(&s.metrics.session_start))
            .max()
            .map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
            .unwrap_or_default(),
        confidence: if parsed == 0 || skipped > 0 || unknown_sources > 0 || unknown_models > 0 {
            "low"
        } else if fallback_pricing > 0 {
            "medium"
        } else {
            "high"
        }
        .to_string(),
        with_tokens: sessions
            .iter()
            .filter(|s| s.metrics.tokens_input + s.metrics.tokens_output > 0)
            .count(),
        with_duration: sessions
            .iter()
            .filter(|s| s.metrics.duration_sec > 0.0)
            .count(),
        with_tools: sessions
            .iter()
            .filter(|s| s.metrics.tool_calls_total > 0)
            .count(),
        with_event_timing: sessions
            .iter()
            .filter(|s| !s.metrics.gaps_sec.is_empty())
            .count(),
        with_diagnostics: sessions
            .iter()
            .filter(|s| session_capability(s) == "detailed")
            .count(),
    }
}

fn contains(value: &str, filter: &str) -> bool {
    filter.trim().is_empty()
        || value
            .to_ascii_lowercase()
            .contains(&filter.trim().to_ascii_lowercase())
}
