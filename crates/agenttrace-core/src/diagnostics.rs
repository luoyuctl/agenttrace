use crate::{Event, Metrics, Session};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Diagnostics {
    pub loop_cost: LoopCost,
    pub loop_fingerprints: Vec<LoopFingerprint>,
    pub tool_latencies: Vec<ToolLatency>,
    pub context_utilization: ContextUtilization,
    pub large_params: Vec<LargeParam>,
    pub unused_tools: Vec<UnusedTool>,
    pub stuck_patterns: Vec<StuckPattern>,
    pub steps: Vec<TraceStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStep {
    pub kind: String,
    pub name: String,
    pub started_at: String,
    pub ended_at: String,
    pub duration_sec: f64,
    pub status: String,
    pub tokens: i64,
    pub call_id: String,
    pub parent_id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoopCost {
    pub retry_cost: f64,
    pub tool_loop_cost: f64,
    pub total_loop_cost: f64,
    pub retry_events: usize,
    pub loop_groups: usize,
    pub loop_type: String,
    pub turns: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopFingerprint {
    pub tool_name: String,
    pub result_hash: String,
    pub count: usize,
    pub first_index: usize,
    pub last_index: usize,
    pub severity: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolLatency {
    pub tool_name: String,
    pub count: usize,
    pub avg_sec: f64,
    pub p95_sec: f64,
    pub max_sec: f64,
    pub min_sec: f64,
    pub timeouts: usize,
    pub is_slow: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextUtilization {
    pub estimated_total: usize,
    pub tool_definitions: usize,
    pub conversation_history: usize,
    pub system_prompt: usize,
    pub available_for_task: usize,
    pub utilization_pct: f64,
    pub risk_level: String,
    pub suggestion: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LargeParam {
    pub tool_name: String,
    pub size: usize,
    pub risk: String,
    pub timestamp: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnusedTool {
    pub tool_name: String,
    pub call_count: usize,
    pub level: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StuckPattern {
    pub pattern: String,
    pub description: String,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FixSuggestion {
    pub title: String,
    pub description: String,
    pub action: String,
    pub severity: String,
    pub category: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CostAlert {
    pub triggered: bool,
    pub level: String,
    pub message: String,
    pub current: f64,
    pub baseline: f64,
    pub ratio: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FindingEvidence {
    pub kind: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionFinding {
    pub kind: String,
    pub value: f64,
    pub detail: String,
    pub severity: String,
    pub evidence: Vec<FindingEvidence>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InspectFirst {
    pub reason: &'static str,
    pub index: usize,
}

pub fn inspect_first(sessions: &[Session]) -> Vec<InspectFirst> {
    let mut items = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let candidates = [
        (
            "critical",
            inspect_by(
                sessions,
                |session| session.health < 50,
                |left, right| {
                    right
                        .health
                        .cmp(&left.health)
                        .then_with(|| cmp_cost(left, right))
                },
            ),
        ),
        (
            "anomaly",
            inspect_by(
                sessions,
                |session| !session.anomalies.is_empty(),
                |left, right| {
                    left.anomalies
                        .len()
                        .cmp(&right.anomalies.len())
                        .then_with(|| right.health.cmp(&left.health))
                        .then_with(|| cmp_cost(left, right))
                },
            ),
        ),
        (
            "failures",
            inspect_by(
                sessions,
                |session| session.metrics.tool_calls_fail > 0,
                |left, right| {
                    left.metrics
                        .tool_calls_fail
                        .cmp(&right.metrics.tool_calls_fail)
                        .then_with(|| right.health.cmp(&left.health))
                        .then_with(|| cmp_cost(left, right))
                },
            ),
        ),
        ("cost", inspect_by(sessions, |_| true, cmp_cost)),
        (
            "latency",
            inspect_by(
                sessions,
                |session| session.metrics.duration_sec > 0.0 || p95_gap(session) > 0.0,
                |left, right| {
                    left.metrics
                        .duration_sec
                        .total_cmp(&right.metrics.duration_sec)
                        .then_with(|| p95_gap(left).total_cmp(&p95_gap(right)))
                        .then_with(|| cmp_cost(left, right))
                },
            ),
        ),
    ];
    for (reason, index) in candidates {
        if let Some(index) = index.filter(|index| seen.insert(*index)) {
            items.push(InspectFirst { reason, index });
        }
    }
    items
}

fn inspect_by(
    sessions: &[Session],
    include: fn(&Session) -> bool,
    compare: fn(&Session, &Session) -> std::cmp::Ordering,
) -> Option<usize> {
    sessions
        .iter()
        .enumerate()
        .filter(|(_, session)| include(session))
        .max_by(|(left_index, left), (right_index, right)| {
            compare(left, right).then_with(|| right_index.cmp(left_index))
        })
        .map(|(index, _)| index)
}

fn cmp_cost(left: &Session, right: &Session) -> std::cmp::Ordering {
    left.metrics
        .cost_estimated
        .total_cmp(&right.metrics.cost_estimated)
        .then_with(|| crate::total_tokens(left).cmp(&crate::total_tokens(right)))
}

fn p95_gap(session: &Session) -> f64 {
    let mut gaps = session
        .metrics
        .gaps_sec
        .iter()
        .copied()
        .filter(|value| value.is_finite() && *value > 0.0)
        .collect::<Vec<_>>();
    gaps.sort_by(f64::total_cmp);
    let index = ((gaps.len() as f64) * 0.95) as usize;
    gaps.get(index.min(gaps.len().saturating_sub(1)))
        .copied()
        .unwrap_or(0.0)
}

pub(crate) fn analyze_diagnostics(events: &[Event], metrics: &Metrics) -> Diagnostics {
    Diagnostics {
        loop_cost: loop_cost(events, metrics.cost_estimated),
        loop_fingerprints: loop_fingerprints(events),
        tool_latencies: tool_latencies(events),
        context_utilization: context_utilization(events, &metrics.model_used),
        large_params: large_params(events),
        unused_tools: unused_tools(events),
        stuck_patterns: stuck_patterns(events, metrics),
        steps: trace_steps(events, &metrics.model_used),
    }
}

fn trace_steps(events: &[Event], _model: &str) -> Vec<TraceStep> {
    let results = events
        .iter()
        .filter(|event| event.role == "tool" && !event.tool_call_id.is_empty())
        .map(|event| (event.tool_call_id.as_str(), event))
        .collect::<HashMap<_, _>>();
    let mut steps = events
        .iter()
        .flat_map(|event| event.tool_calls.iter().map(move |call| (event, call)))
        .map(|(event, call)| {
            let result = results.get(call.id.as_str());
            let duration_sec = parse_time(&event.timestamp)
                .zip(result.and_then(|event| parse_time(&event.timestamp)))
                .map(|(start, end)| (end - start).num_milliseconds() as f64 / 1000.0)
                .filter(|duration| (0.0..3600.0).contains(duration))
                .unwrap_or(0.0);
            TraceStep {
                kind: "tool".to_string(),
                name: call.name.clone(),
                started_at: event.timestamp.clone(),
                ended_at: result
                    .map(|event| event.timestamp.clone())
                    .unwrap_or_default(),
                duration_sec,
                status: match result {
                    Some(event) if event.is_error => "error",
                    Some(_) => "ok",
                    None => "missing",
                }
                .to_string(),
                tokens: 0,
                call_id: call.id.clone(),
                parent_id: String::new(),
            }
        })
        .collect::<Vec<_>>();
    // ponytail: keep the failure boundary, add pagination only if users need full local traces.
    if steps.len() > 200 {
        let omitted = steps.len() - 200;
        steps = steps.split_off(omitted);
        steps.insert(
            0,
            TraceStep {
                kind: "meta".to_string(),
                name: format!("{omitted} earlier tool steps omitted"),
                started_at: String::new(),
                ended_at: String::new(),
                duration_sec: 0.0,
                status: "truncated".to_string(),
                tokens: 0,
                call_id: String::new(),
                parent_id: String::new(),
            },
        );
    }
    steps
}

pub fn fix_suggestions(session: &Session) -> Vec<FixSuggestion> {
    let mut fixes = Vec::new();
    let total = session.metrics.tool_calls_total;
    for anomaly in &session.anomalies {
        let (title, description, action, category) = match anomaly.kind.as_str() {
            "hanging" => (
                "Add tool timeout",
                "Long gaps indicate an unbounded operation.",
                "Add cancellation and a bounded timeout to long-running tools.",
                "hanging",
            ),
            "tool_failures" if total > 0 => (
                "Reduce tool failures",
                "Repeated failures increase latency and cost.",
                "Inspect failed arguments and stop retrying unchanged calls.",
                "tool_failure",
            ),
            "shallow_thinking" => (
                "Add an explicit plan",
                "The session executed risky steps with little planning evidence.",
                "Plan and verify the risky steps before execution.",
                "thinking",
            ),
            "redaction" => (
                "Review redacted reasoning",
                "Redacted reasoning limits failure attribution.",
                "Check whether missing reasoning hides the failure boundary.",
                "redaction",
            ),
            "no_tools" => (
                "Use available tools",
                "The session did not inspect concrete artifacts.",
                "Inspect concrete artifacts instead of relying on chat-only reasoning.",
                "no_tools",
            ),
            _ => continue,
        };
        fixes.push(FixSuggestion {
            title: title.to_string(),
            description: description.to_string(),
            action: action.to_string(),
            severity: anomaly.severity.clone(),
            category: category.to_string(),
        });
    }
    fixes
}

pub fn predict_cost_anomaly(history: &[Session], current: &Session) -> CostAlert {
    let costs = history
        .iter()
        .filter(|session| session.path != current.path && session.metrics.assistant_turns > 0)
        .map(|session| session.metrics.cost_estimated / session.metrics.assistant_turns as f64)
        .collect::<Vec<_>>();
    if costs.is_empty() || current.metrics.assistant_turns == 0 {
        return CostAlert {
            level: "info".to_string(),
            message: "No comparable cost history.".to_string(),
            ..CostAlert::default()
        };
    }
    let baseline = costs.iter().sum::<f64>() / costs.len() as f64;
    let value = current.metrics.cost_estimated / current.metrics.assistant_turns as f64;
    let ratio = if baseline > 0.0 {
        value / baseline
    } else {
        0.0
    };
    let loop_pct = loop_waste_percent(
        current.diagnostics.loop_cost.total_loop_cost,
        current.metrics.cost_estimated,
    );
    let (triggered, level, message) = if ratio > 3.0 {
        (
            true,
            "critical",
            format!("Cost/turn is {ratio:.1}x the session baseline."),
        )
    } else if ratio > 2.0 {
        (
            true,
            "warning",
            format!("Cost/turn is {ratio:.1}x the session baseline."),
        )
    } else if loop_pct > 50.0 {
        (
            true,
            "critical",
            format!("Loop waste is {loop_pct:.0}% of session cost."),
        )
    } else if loop_pct > 30.0 {
        (
            true,
            "warning",
            format!("Loop waste is {loop_pct:.0}% of session cost."),
        )
    } else {
        (
            false,
            "info",
            format!("Cost/turn is {ratio:.1}x the session baseline."),
        )
    };
    CostAlert {
        triggered,
        level: level.to_string(),
        message,
        current: value,
        baseline,
        ratio,
    }
}

pub fn session_findings(session: &Session, history: &[Session]) -> Vec<SessionFinding> {
    let diagnostics = &session.diagnostics;
    let mut findings = Vec::new();
    if diagnostics.loop_cost.loop_groups > 0 {
        findings.push(finding(
            "loop",
            diagnostics.loop_cost.total_loop_cost,
            "",
            "high",
            "repeated_groups",
            diagnostics.loop_cost.loop_groups.to_string(),
        ));
    }
    if session.metrics.tool_calls_fail > 0 {
        findings.push(finding(
            "retry",
            session.metrics.tool_calls_fail as f64,
            "",
            if session.metrics.tool_calls_fail >= 3 {
                "high"
            } else {
                "medium"
            },
            "failed_calls",
            session.metrics.tool_calls_fail.to_string(),
        ));
    }
    if let Some(latency) = diagnostics
        .tool_latencies
        .iter()
        .filter(|item| item.max_sec >= 5.0 || item.timeouts > 0)
        .max_by(|left, right| left.max_sec.total_cmp(&right.max_sec))
    {
        findings.push(finding(
            "latency",
            latency.max_sec,
            &latency.tool_name,
            if latency.timeouts > 0 {
                "high"
            } else {
                "medium"
            },
            "slowest_tool",
            latency.tool_name.clone(),
        ));
    }
    if matches!(
        diagnostics.context_utilization.risk_level.as_str(),
        "warning" | "critical"
    ) {
        findings.push(finding(
            "context",
            diagnostics.context_utilization.utilization_pct,
            "",
            &diagnostics.context_utilization.risk_level,
            "context_used",
            format!("{:.0}%", diagnostics.context_utilization.utilization_pct),
        ));
    }
    if let Some(largest) = diagnostics.large_params.iter().max_by_key(|item| item.size) {
        findings.push(finding(
            "large_params",
            (largest.size / 1024) as f64,
            &largest.tool_name,
            &largest.risk,
            "largest_input",
            format!("{} KB", largest.size / 1024),
        ));
    }
    if let Some(pattern) = diagnostics.stuck_patterns.first() {
        findings.push(finding(
            "stuck",
            0.0,
            &pattern.description,
            &pattern.severity,
            "pattern",
            pattern.pattern.replace('_', " "),
        ));
    }
    let alert = predict_cost_anomaly(history, session);
    if alert.triggered {
        findings.push(finding(
            "cost",
            session.metrics.cost_estimated,
            &alert.message,
            &alert.level,
            "session_cost",
            format!("${:.2}", session.metrics.cost_estimated),
        ));
    }
    findings
}

fn finding(
    kind: &str,
    value: f64,
    detail: &str,
    severity: &str,
    evidence_kind: &str,
    evidence_value: String,
) -> SessionFinding {
    SessionFinding {
        kind: kind.to_string(),
        value,
        detail: detail.to_string(),
        severity: severity.to_string(),
        evidence: vec![FindingEvidence {
            kind: evidence_kind.to_string(),
            value: evidence_value,
        }],
    }
}

pub fn loop_waste_percent(loop_cost: f64, total_cost: f64) -> f64 {
    if !total_cost.is_finite() || total_cost <= 0.0 {
        return 0.0;
    }
    loop_cost.clamp(0.0, total_cost) / total_cost * 100.0
}

fn loop_cost(events: &[Event], total_cost: f64) -> LoopCost {
    let mut last = "";
    let mut consecutive = 0;
    let mut max_consecutive = 0;
    let mut max_tool = "";
    let mut retries = 0;
    for call in events.iter().flat_map(|event| &event.tool_calls) {
        if call.name == last {
            consecutive += 1;
            if consecutive >= 3 {
                retries += 1;
            }
        } else {
            if consecutive > max_consecutive {
                max_consecutive = consecutive;
                max_tool = last;
            }
            consecutive = 1;
            last = &call.name;
        }
    }
    if consecutive > max_consecutive {
        max_consecutive = consecutive;
        max_tool = last;
    }
    let tool = if max_consecutive >= 3 {
        max_consecutive as f64 * 0.015
    } else {
        0.0
    };
    let retry = retries as f64 * 0.0075;
    let raw_total = tool + retry;
    let total = if total_cost.is_finite() && total_cost > 0.0 {
        raw_total.min(total_cost)
    } else {
        0.0
    };
    let scale = if raw_total > 0.0 {
        total / raw_total
    } else {
        0.0
    };
    LoopCost {
        retry_cost: retry * scale,
        tool_loop_cost: tool * scale,
        total_loop_cost: total,
        retry_events: retries,
        loop_groups: usize::from(max_consecutive >= 3),
        loop_type: if max_consecutive >= 3 {
            format!("{max_tool}_loop")
        } else {
            String::new()
        },
        turns: max_consecutive,
    }
}

fn loop_fingerprints(events: &[Event]) -> Vec<LoopFingerprint> {
    let results = events
        .iter()
        .filter(|event| event.role == "tool" && !event.tool_call_id.is_empty())
        .map(|event| (event.tool_call_id.as_str(), hash(&event.content)))
        .collect::<HashMap<_, _>>();
    let pairs = events
        .iter()
        .flat_map(|event| &event.tool_calls)
        .filter_map(|call| {
            results
                .get(call.id.as_str())
                .map(|hash| (call.name.as_str(), *hash))
        })
        .collect::<Vec<_>>();
    let mut out = Vec::new();
    let mut start = 0;
    while start < pairs.len() {
        let mut end = start + 1;
        while end < pairs.len() && pairs[end] == pairs[start] {
            end += 1;
        }
        let count = end - start;
        if count >= 3 {
            out.push(LoopFingerprint {
                tool_name: pairs[start].0.to_string(),
                result_hash: format!("{:x}", pairs[start].1),
                count,
                first_index: start,
                last_index: end - 1,
                severity: if count >= 5 { "critical" } else { "high" }.to_string(),
                detail: format!(
                    "Tool '{}' returned the same result {count} times",
                    pairs[start].0
                ),
            });
        }
        start = end;
    }
    out
}

fn tool_latencies(events: &[Event]) -> Vec<ToolLatency> {
    let results = events
        .iter()
        .filter_map(|event| {
            parse_time(&event.timestamp).map(|time| (event.tool_call_id.as_str(), time))
        })
        .filter(|(id, _)| !id.is_empty())
        .collect::<HashMap<_, _>>();
    let mut values: BTreeMap<String, (Vec<f64>, usize)> = BTreeMap::new();
    for event in events {
        let Some(start) = parse_time(&event.timestamp) else {
            continue;
        };
        for call in &event.tool_calls {
            let entry = values.entry(call.name.clone()).or_default();
            if let Some(end) = results.get(call.id.as_str()) {
                let seconds = (*end - start).num_milliseconds() as f64 / 1000.0;
                if (0.0..3600.0).contains(&seconds) {
                    entry.0.push(seconds);
                }
            } else {
                entry.1 += 1;
            }
        }
    }
    let mut out = values
        .into_iter()
        .map(|(tool_name, (mut values, timeouts))| {
            values.sort_by(f64::total_cmp);
            let count = values.len() + timeouts;
            let avg_sec = if values.is_empty() {
                0.0
            } else {
                values.iter().sum::<f64>() / values.len() as f64
            };
            let p95_sec = values
                .get(((values.len() as f64 * 0.95).ceil() as usize).saturating_sub(1))
                .copied()
                .unwrap_or(0.0);
            ToolLatency {
                tool_name,
                count,
                avg_sec,
                p95_sec,
                max_sec: values.last().copied().unwrap_or(0.0),
                min_sec: values.first().copied().unwrap_or(0.0),
                timeouts,
                is_slow: p95_sec > 30.0,
            }
        })
        .collect::<Vec<_>>();
    out.sort_by(|a, b| {
        b.max_sec
            .total_cmp(&a.max_sec)
            .then_with(|| a.tool_name.cmp(&b.tool_name))
    });
    out
}

fn context_utilization(events: &[Event], model: &str) -> ContextUtilization {
    let lower = model.to_ascii_lowercase();
    let total: usize = if lower.contains("gemini") {
        1_048_576
    } else if lower.contains("claude") {
        200_000
    } else if lower.contains("gpt") || lower.contains("deepseek") {
        128_000
    } else {
        131_072
    };
    let tools = events
        .iter()
        .flat_map(|event| &event.tool_calls)
        .map(|call| call.name.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        .max(8)
        * 300;
    let history = events
        .iter()
        .map(|event| event.content.len() + event.reasoning.len())
        .sum::<usize>()
        / 2;
    let system = 12_000;
    let used = tools + history + system;
    let available = total.saturating_sub(used);
    ContextUtilization {
        estimated_total: total,
        tool_definitions: tools,
        conversation_history: history,
        system_prompt: system,
        available_for_task: available,
        utilization_pct: used as f64 / total as f64 * 100.0,
        risk_level: if available < 20_000 {
            "critical"
        } else if available < 50_000 {
            "warning"
        } else {
            "good"
        }
        .to_string(),
        suggestion: if available < 50_000 {
            "Reduce conversation or tool context before continuing.".to_string()
        } else {
            String::new()
        },
    }
}

fn large_params(events: &[Event]) -> Vec<LargeParam> {
    events
        .iter()
        .flat_map(|event| event.tool_calls.iter().map(move |call| (event, call)))
        .filter_map(|(event, call)| {
            let size = call.args.len();
            (size > 10_000).then(|| LargeParam {
                tool_name: call.name.clone(),
                size,
                risk: if size > 50_000 { "high" } else { "medium" }.to_string(),
                timestamp: event.timestamp.clone(),
                detail: format!("Tool '{}' received {size} bytes of arguments", call.name),
            })
        })
        .collect()
}

fn unused_tools(events: &[Event]) -> Vec<UnusedTool> {
    let mut usage = BTreeMap::new();
    for call in events.iter().flat_map(|event| &event.tool_calls) {
        *usage.entry(call.name.clone()).or_insert(0) += 1;
    }
    usage
        .into_iter()
        .filter(|(_, count)| *count <= 2)
        .map(|(tool_name, call_count)| UnusedTool {
            tool_name,
            call_count,
            level: "rare".to_string(),
            detail: format!("Tool was used {call_count} time(s)."),
        })
        .collect()
}

fn stuck_patterns(events: &[Event], metrics: &Metrics) -> Vec<StuckPattern> {
    let mut out = Vec::new();
    let long_gaps = metrics.gaps_sec.iter().filter(|gap| **gap > 120.0).count();
    if long_gaps >= 3 {
        out.push(StuckPattern {
            pattern: "long_gaps".to_string(),
            description: format!("{long_gaps} gaps exceed 120s"),
            severity: "critical".to_string(),
        });
    }
    let mut content = BTreeMap::new();
    for event in events
        .iter()
        .filter(|event| event.role == "assistant" && event.content.len() > 50)
    {
        *content
            .entry(event.content.chars().take(50).collect::<String>())
            .or_insert(0) += 1;
    }
    for count in content.into_values().filter(|count| *count >= 4) {
        out.push(StuckPattern {
            pattern: "repeated_response".to_string(),
            description: format!("Repeated assistant response {count} times"),
            severity: "warning".to_string(),
        });
    }
    let result_ids = events
        .iter()
        .filter(|event| event.role == "tool")
        .map(|event| event.tool_call_id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let zombies = events
        .iter()
        .flat_map(|event| &event.tool_calls)
        .filter(|call| !call.id.is_empty() && !result_ids.contains(call.id.as_str()))
        .count();
    if zombies > 0 {
        out.push(StuckPattern {
            pattern: "zombie_tool_calls".to_string(),
            description: format!("{zombies} tool calls have no result"),
            severity: "warning".to_string(),
        });
    }
    out
}

fn parse_time(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|time| time.with_timezone(&Utc))
}

fn hash(value: &str) -> u32 {
    value.bytes().take(200).fold(5381_u32, |hash, byte| {
        hash.wrapping_mul(33).wrapping_add(byte as u32)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ToolCall;

    #[test]
    fn detects_loop_latency_large_params_and_stuck_signals() {
        let mut events = vec![Event {
            role: "user".to_string(),
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            ..Event::default()
        }];
        for index in 0..4 {
            events.push(Event {
                role: "assistant".to_string(),
                timestamp: format!("2026-01-01T00:00:0{}Z", index * 2 + 1),
                content: "same response long enough to qualify as repeated assistant output"
                    .to_string(),
                tool_calls: vec![ToolCall {
                    id: index.to_string(),
                    name: "bash".to_string(),
                    args: "x".repeat(10_001),
                }],
                ..Event::default()
            });
            events.push(Event {
                role: "tool".to_string(),
                tool_call_id: index.to_string(),
                content: "same failure".to_string(),
                is_error: true,
                timestamp: format!("2026-01-01T00:00:0{}Z", index * 2 + 2),
                ..Event::default()
            });
        }
        let metrics = Metrics {
            model_used: "gpt-5.1".to_string(),
            cost_estimated: 0.1,
            ..Metrics::default()
        };
        let diagnostics = analyze_diagnostics(&events, &metrics);
        assert_eq!(diagnostics.loop_fingerprints[0].count, 4);
        assert_eq!(diagnostics.loop_fingerprints[0].first_index, 0);
        assert_eq!(diagnostics.loop_fingerprints[0].last_index, 3);
        assert!(!diagnostics.loop_fingerprints[0].result_hash.is_empty());
        assert!(diagnostics.loop_cost.total_loop_cost > 0.0);
        assert_eq!(diagnostics.loop_cost.loop_type, "bash_loop");
        assert_eq!(diagnostics.loop_cost.turns, 4);
        assert_eq!(diagnostics.tool_latencies[0].count, 4);
        assert_eq!(diagnostics.tool_latencies[0].min_sec, 1.0);
        assert_eq!(diagnostics.large_params.len(), 4);
        assert!(!diagnostics.large_params[0].timestamp.is_empty());
        assert!(!diagnostics.large_params[0].detail.is_empty());
        assert!(diagnostics
            .stuck_patterns
            .iter()
            .any(|item| item.pattern == "repeated_response"));
    }

    #[test]
    fn trace_steps_keep_metadata_without_content_or_args() {
        let events = vec![
            Event {
                role: "assistant".to_string(),
                content: "private response".to_string(),
                timestamp: "2026-01-01T00:00:00Z".to_string(),
                tool_calls: vec![crate::ToolCall {
                    id: "call-1".to_string(),
                    name: "shell".to_string(),
                    args: "secret=true".to_string(),
                }],
                ..Event::default()
            },
            Event {
                role: "tool".to_string(),
                content: "private result".to_string(),
                timestamp: "2026-01-01T00:00:02Z".to_string(),
                tool_call_id: "call-1".to_string(),
                ..Event::default()
            },
        ];
        let steps = trace_steps(&events, "gpt-test");
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].name, "shell");
        assert_eq!(steps[0].duration_sec, 2.0);
        let json = serde_json::to_string(&steps).unwrap();
        assert!(!json.contains("private"));
        assert!(!json.contains("secret"));
    }

    #[test]
    fn session_findings_reuse_diagnostic_rules() {
        let mut session = Session {
            name: "failed".to_string(),
            path: "failed".to_string(),
            metrics: Metrics {
                tool_calls_fail: 3,
                ..Metrics::default()
            },
            anomalies: Vec::new(),
            health: 70,
            tool_warnings: Vec::new(),
            diagnostics: Diagnostics::default(),
            cwd: String::new(),
        };
        session.diagnostics.loop_cost.loop_groups = 1;
        let findings = session_findings(&session, &[]);
        assert!(findings.iter().any(|finding| finding.kind == "loop"));
        assert!(findings
            .iter()
            .any(|finding| finding.kind == "retry" && finding.severity == "high"));
    }

    #[test]
    fn cost_alert_exposes_structured_baseline() {
        let historical = session_with_cost("old", 2, 0.2);
        let current = session_with_cost("new", 2, 0.8);
        let alert = predict_cost_anomaly(&[historical], &current);
        assert!(alert.triggered);
        assert_eq!(alert.current, 0.4);
        assert_eq!(alert.baseline, 0.1);
        assert_eq!(alert.ratio, 4.0);
    }

    fn session_with_cost(path: &str, turns: usize, cost: f64) -> Session {
        Session {
            name: path.to_string(),
            path: path.to_string(),
            metrics: Metrics {
                assistant_turns: turns,
                cost_estimated: cost,
                ..Metrics::default()
            },
            anomalies: Vec::new(),
            health: 100,
            tool_warnings: Vec::new(),
            diagnostics: Diagnostics::default(),
            cwd: String::new(),
        }
    }
}
