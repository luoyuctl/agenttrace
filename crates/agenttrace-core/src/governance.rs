use crate::{
    pricing, project_name, resolve_project, round4, session_capability, total_tokens, Session,
};
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

#[derive(Debug, Clone, Serialize)]
pub struct CostAudit {
    pub pricing_source: String,
    pub total_estimated_cost: f64,
    pub stored_estimated_cost_usd: f64,
    pub current_estimated_cost_usd: Option<f64>,
    pub pricing_coverage: PricingCoverage,
    pub by_provider_model: Vec<ModelCostAudit>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PricingCoverage {
    pub priced_sessions: usize,
    pub fallback_priced_sessions: usize,
    pub unpriced_or_unknown_sessions: usize,
    pub exact_pricing_pct: f64,
    pub confidence: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelCostAudit {
    pub provider: String,
    pub model: String,
    pub pricing_source: String,
    pub sessions: usize,
    pub tokens: TokenBreakdown,
    pub rates_per_million_usd: Option<PriceBreakdown>,
    pub component_cost_usd: Option<PriceBreakdown>,
    pub estimated_cost_usd: Option<f64>,
    pub stored_estimated_cost_usd: f64,
    pub pricing_status: String,
    pub pricing_note: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TokenBreakdown {
    pub input: i64,
    pub output: i64,
    pub cache_write: i64,
    pub cache_read: i64,
    pub total: i64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PriceBreakdown {
    pub input: f64,
    pub output: f64,
    pub cache_write: f64,
    pub cache_read: f64,
    pub total: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionCostAudit {
    pub pricing_source: String,
    pub stored_pricing_source: String,
    pub capability: &'static str,
    pub provider: String,
    pub model: String,
    pub tokens: TokenBreakdown,
    pub rates_per_million_usd: Option<PriceBreakdown>,
    pub component_cost_usd: Option<PriceBreakdown>,
    pub estimated_cost_usd: Option<f64>,
    pub stored_estimated_cost_usd: f64,
    pub pricing_status: String,
    pub pricing_note: String,
}

fn current_cost_estimate(
    model: &str,
    tokens: &TokenBreakdown,
) -> Option<(String, PriceBreakdown, PriceBreakdown)> {
    if model == "multiple" {
        return None;
    }
    let rate = pricing::lookup_price(model);
    let rates = PriceBreakdown {
        input: rate.input,
        output: rate.output,
        cache_write: rate.cw,
        cache_read: rate.cr,
        total: 0.0,
    };
    let mut components = PriceBreakdown {
        input: round4(tokens.input as f64 / 1e6 * rate.input),
        output: round4(tokens.output as f64 / 1e6 * rate.output),
        cache_write: round4(tokens.cache_write as f64 / 1e6 * rate.cw),
        cache_read: round4(tokens.cache_read as f64 / 1e6 * rate.cr),
        total: 0.0,
    };
    components.total = round4(
        components.input + components.output + components.cache_write + components.cache_read,
    );
    Some((pricing::pricing_source_for(model), rates, components))
}

#[derive(Debug, Clone, Serialize)]
pub struct Recommendation {
    pub id: String,
    pub priority: String,
    pub severity: String,
    pub category: String,
    pub title: String,
    pub rationale: String,
    pub evidence: Vec<String>,
    pub estimated_savings_usd: f64,
    pub estimated_savings_tokens: i64,
    pub confidence: String,
    pub action: String,
    pub validation_command: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpGovernance {
    pub items: Vec<McpGovernanceItem>,
    pub methodology: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpGovernanceItem {
    pub server: String,
    pub loaded_sessions: Option<usize>,
    pub invoked_sessions: usize,
    pub tool_calls: usize,
    pub failed_calls: usize,
    pub coverage_pct: Option<f64>,
    pub estimated_schema_tokens: Option<usize>,
    pub recommendation: String,
    pub confidence: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextTrend {
    pub methodology: String,
    pub totals: ContextTrendTotals,
    pub projects: Vec<ProjectContextTrend>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ContextTrendTotals {
    pub sessions: usize,
    pub context_warning_sessions: usize,
    pub context_critical_sessions: usize,
    pub repeated_file_reads: usize,
    pub cache_effectiveness_pct: f64,
    pub read_to_write_ratio: f64,
    pub output_cost_per_million_tokens: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectContextTrend {
    pub project: String,
    pub sessions: usize,
    pub avg_context_utilization_pct: f64,
    pub cache_effectiveness_pct: f64,
    pub repeated_file_reads: usize,
    pub read_to_write_ratio: f64,
    pub cost_per_output_token: f64,
}

#[derive(Default)]
struct ContextAggregate {
    sessions: usize,
    context: f64,
    warnings: usize,
    critical: usize,
    cache_read: i64,
    input: i64,
    output: i64,
    cost: f64,
    reads: usize,
    writes: usize,
    files: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeliveryEvidence {
    pub methodology: String,
    pub summary: DeliverySummary,
    pub sessions: Vec<SessionDeliveryEvidence>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DeliverySummary {
    pub strong: usize,
    pub medium: usize,
    pub weak: usize,
    pub none: usize,
    pub non_code: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionDeliveryEvidence {
    pub session: String,
    pub project: String,
    pub level: String,
    pub evidence: Vec<String>,
    pub confidence: String,
}

pub fn cost_audit(sessions: &[Session]) -> CostAudit {
    #[derive(Default)]
    struct Aggregate {
        sessions: usize,
        tokens: TokenBreakdown,
        cost: f64,
        specific: usize,
        fallback: usize,
        unknown: usize,
    }
    let mut rows: BTreeMap<(String, String), Aggregate> = BTreeMap::new();
    let mut coverage = PricingCoverage::default();
    for session in sessions {
        let provider = session.metrics.source_tool.clone();
        let model = normalized_model(&session.metrics.model_used);
        let row = rows.entry((provider, model.clone())).or_default();
        row.sessions += 1;
        row.tokens.input += session.metrics.tokens_input;
        row.tokens.output += session.metrics.tokens_output;
        row.tokens.cache_write += session.metrics.tokens_cache_w;
        row.tokens.cache_read += session.metrics.tokens_cache_r;
        row.tokens.total += total_tokens(session);
        row.cost += session.metrics.cost_estimated;
        if matches!(model.as_str(), "default" | "unknown" | "multiple") {
            row.unknown += 1;
            coverage.unpriced_or_unknown_sessions += 1;
        } else if pricing::has_specific_price(&model) {
            row.specific += 1;
            coverage.priced_sessions += 1;
        } else {
            row.fallback += 1;
            coverage.fallback_priced_sessions += 1;
        }
    }
    coverage.confidence = if coverage.unpriced_or_unknown_sessions > 0 {
        "low"
    } else if coverage.fallback_priced_sessions > 0 {
        "medium"
    } else {
        "high"
    }
    .to_string();
    coverage.exact_pricing_pct = pct(
        coverage.priced_sessions as i64,
        (coverage.priced_sessions
            + coverage.fallback_priced_sessions
            + coverage.unpriced_or_unknown_sessions) as i64,
    );
    let mut by_provider_model = rows
        .into_iter()
        .map(|((provider, model), row)| {
            let current = current_cost_estimate(&model, &row.tokens);
            let pricing_source = current
                .as_ref()
                .map(|(source, _, _)| source.clone())
                .unwrap_or_else(|| "unavailable: multiple models".to_string());
            let (pricing_status, pricing_note) = if model == "multiple" {
                (
                    "aggregate_estimate",
                    "SQLite aggregated multiple model IDs; no single-model exact price applies",
                )
            } else if row.unknown > 0 {
                ("unpriced_or_unknown", "model name is missing or generic")
            } else if row.fallback > 0 {
                (
                    "fallback_estimate",
                    "no exact catalog match; built-in fallback rate used",
                )
            } else {
                (
                    "catalog_estimate",
                    "exact normalized model match in pricing catalog",
                )
            };
            let pricing_note = if current
                .as_ref()
                .is_some_and(|(_, _, components)| (components.total - row.cost).abs() > 0.0001)
            {
                format!(
                    "{pricing_note}; current rates recalculate a different total than the stored estimate"
                )
            } else {
                pricing_note.to_string()
            };
            let (rates_per_million_usd, component_cost_usd, estimated_cost_usd) = current
                .map(|(_, rates, components)| {
                    let total = components.total;
                    (Some(rates), Some(components), Some(total))
                })
                .unwrap_or((None, None, None));
            ModelCostAudit {
                provider,
                model,
                pricing_source,
                sessions: row.sessions,
                tokens: row.tokens,
                rates_per_million_usd,
                component_cost_usd,
                estimated_cost_usd,
                stored_estimated_cost_usd: round4(row.cost),
                pricing_status: pricing_status.to_string(),
                pricing_note,
            }
        })
        .collect::<Vec<_>>();
    let stored_estimated_cost_usd = round4(
        sessions
            .iter()
            .map(|session| session.metrics.cost_estimated)
            .sum(),
    );
    let current_estimated_cost_usd = by_provider_model
        .iter()
        .try_fold(0.0, |total, row| {
            row.estimated_cost_usd.map(|cost| total + cost)
        })
        .map(round4);
    by_provider_model.sort_by(|left, right| {
        let left_cost = left
            .estimated_cost_usd
            .unwrap_or(left.stored_estimated_cost_usd);
        let right_cost = right
            .estimated_cost_usd
            .unwrap_or(right.stored_estimated_cost_usd);
        right_cost
            .total_cmp(&left_cost)
            .then_with(|| left.provider.cmp(&right.provider))
            .then_with(|| left.model.cmp(&right.model))
    });
    CostAudit {
        pricing_source: pricing::pricing_source(),
        total_estimated_cost: stored_estimated_cost_usd,
        stored_estimated_cost_usd,
        current_estimated_cost_usd,
        pricing_coverage: coverage,
        by_provider_model,
    }
}

pub fn session_cost_audit(session: &Session) -> SessionCostAudit {
    let model = normalized_model(&session.metrics.model_used);
    let tokens = TokenBreakdown {
        input: session.metrics.tokens_input,
        output: session.metrics.tokens_output,
        cache_write: session.metrics.tokens_cache_w,
        cache_read: session.metrics.tokens_cache_r,
        total: total_tokens(session),
    };
    let current = current_cost_estimate(&model, &tokens);
    let (pricing_status, pricing_note) = if model == "multiple" {
        (
            "aggregate_estimate",
            "SQLite aggregated multiple model IDs; no single-model exact price applies",
        )
    } else if matches!(model.as_str(), "default" | "unknown") {
        (
            "unpriced_or_unknown",
            "model name is missing or generic; cost is not a catalog match",
        )
    } else if pricing::has_specific_price(&model) {
        (
            "catalog_estimate",
            "exact normalized model match in pricing catalog",
        )
    } else {
        (
            "fallback_estimate",
            "no exact catalog match; built-in fallback rate used",
        )
    };
    let pricing_note = if current.as_ref().is_some_and(|(_, _, components)| {
        (components.total - session.metrics.cost_estimated).abs() > 0.0001
    }) {
        format!(
            "{pricing_note}; current rates recalculate a different total than the stored estimate"
        )
    } else {
        pricing_note.to_string()
    };
    let pricing_source = current
        .as_ref()
        .map(|(source, _, _)| source.clone())
        .unwrap_or_else(|| "unavailable: multiple models".to_string());
    let (rates_per_million_usd, component_cost_usd, estimated_cost_usd) = current
        .map(|(_, rates, components)| {
            let total = components.total;
            (Some(rates), Some(components), Some(total))
        })
        .unwrap_or((None, None, None));
    SessionCostAudit {
        pricing_source,
        stored_pricing_source: if session.metrics.provenance.pricing_source.is_empty() {
            "not recorded".to_string()
        } else {
            session.metrics.provenance.pricing_source.clone()
        },
        capability: session_capability(session),
        provider: session.metrics.source_tool.clone(),
        model,
        tokens,
        rates_per_million_usd,
        component_cost_usd,
        estimated_cost_usd,
        stored_estimated_cost_usd: round4(session.metrics.cost_estimated),
        pricing_status: pricing_status.to_string(),
        pricing_note,
    }
}

pub fn recommendations(sessions: &[Session]) -> Vec<Recommendation> {
    let mut items = Vec::new();
    for session in sessions {
        let loop_cost = session.diagnostics.loop_cost.total_loop_cost;
        if loop_cost > 0.0 {
            items.push(recommendation(
                "retry-loop",
                severity_from_cost(loop_cost),
                "retry_loop",
                "Bound repeated tool retries",
                format!(
                    "{} repeated tool events or loop group(s) were detected.",
                    session.diagnostics.loop_cost.retry_events
                        + session.diagnostics.loop_cost.loop_groups
                ),
                vec![
                    format!("session={}", session.name),
                    format!("loop_cost=${loop_cost:.4}"),
                ],
                loop_cost,
                0,
                "medium",
                "Stop after two unchanged failures; inspect the failure boundary before retrying.",
                "agenttrace --diagnostics --inspect 1 -f json",
            ));
        }
        if session.metrics.tool_calls_fail > 0 {
            items.push(recommendation(
                "tool-failures",
                if session.metrics.tool_calls_fail >= 3 { "high" } else { "medium" },
                "tool_failure",
                "Reduce failing tool calls",
                "Tool failures increase wall time and often precede repeated work.".to_string(),
                vec![format!("session={}", session.name), format!("failed_calls={}", session.metrics.tool_calls_fail)],
                session.metrics.cost_estimated * session.metrics.tool_calls_fail as f64
                    / session.metrics.tool_calls_total.max(1) as f64,
                0,
                "high",
                "Inspect arguments and results, then change the approach instead of retrying unchanged calls.",
                "agenttrace --sessions --sort failures --limit 20",
            ));
        }
        let context = &session.diagnostics.context_utilization;
        if matches!(context.risk_level.as_str(), "warning" | "critical") {
            items.push(recommendation(
                "context-pressure",
                &context.risk_level,
                "context",
                "Start a narrower follow-up session",
                "Conversation and tool context leave little room for the active task.".to_string(),
                vec![format!("session={}", session.name), format!("utilization={:.1}%", context.utilization_pct)],
                session.metrics.cost_estimated * 0.2,
                (context.conversation_history / 5) as i64,
                "medium",
                "Carry only the current goal, relevant files, and failing output into a fresh session.",
                "agenttrace --context-trends --project <project> -f json",
            ));
        }
        if let Some(slow) = session
            .diagnostics
            .tool_latencies
            .iter()
            .find(|item| item.is_slow || item.timeouts > 0)
        {
            items.push(recommendation(
                "slow-tool",
                if slow.timeouts > 0 { "high" } else { "medium" },
                "latency",
                "Bound slow tool execution",
                "A tool exceeded the latency threshold or returned without a result.".to_string(),
                vec![
                    format!("session={}", session.name),
                    format!(
                        "tool={} p95={:.1}s timeouts={}",
                        slow.tool_name, slow.p95_sec, slow.timeouts
                    ),
                ],
                session.metrics.cost_estimated * 0.1,
                0,
                "high",
                "Set a timeout and batch independent work instead of serial retries.",
                "agenttrace --diagnostics --inspect 1 -f json",
            ));
        }
    }
    let mut deduped: BTreeMap<(String, String), Recommendation> = BTreeMap::new();
    for item in items {
        let key = (item.category.clone(), item.title.clone());
        match deduped.get(&key) {
            Some(existing) if recommendation_rank(existing) >= recommendation_rank(&item) => {}
            _ => {
                deduped.insert(key, item);
            }
        }
    }
    let mut items = deduped.into_values().collect::<Vec<_>>();
    items.sort_by(|left, right| {
        recommendation_rank(right)
            .cmp(&recommendation_rank(left))
            .then_with(|| left.id.cmp(&right.id))
    });
    items
}

pub fn mcp_governance(sessions: &[Session]) -> McpGovernance {
    #[derive(Default)]
    struct Aggregate {
        invoked: BTreeSet<String>,
        calls: usize,
        failed: usize,
    }
    let mut rows: BTreeMap<String, Aggregate> = BTreeMap::new();
    for session in sessions {
        let session_key = format!("{}:{}", session.path, session.metrics.session_start);
        for (tool, count) in &session.metrics.tool_usage {
            let Some(server) = mcp_server_name(tool) else {
                continue;
            };
            let row = rows.entry(server).or_default();
            row.invoked.insert(session_key.clone());
            row.calls += count;
        }
        for warning in &session.tool_warnings {
            if let Some(server) = mcp_server_name(&warning.tool_name) {
                rows.entry(server).or_default().failed += warning.count;
            }
        }
    }
    let items = rows.into_iter().map(|(server, row)| {
        let invoked_sessions = row.invoked.len();
        let recommendation = if row.failed > 0 {
            "investigate failed calls before changing server scope"
        } else {
            "observed usage is material; loading coverage cannot be inferred from invocation-only logs"
        };
        McpGovernanceItem {
            server,
            loaded_sessions: None,
            invoked_sessions,
            tool_calls: row.calls,
            failed_calls: row.failed,
            coverage_pct: None,
            estimated_schema_tokens: None,
            recommendation: recommendation.to_string(),
            confidence: "low: loaded-session counts and schema tokens are unavailable because these logs expose invocations, not complete MCP inventories".to_string(),
        }
    }).collect();
    McpGovernance {
        items,
        methodology: "MCP server names are inferred from tool-name prefixes. Invocation coverage is reported only among observed calls; loaded-server inventory and schema-token cost are intentionally left unmeasured.".to_string(),
    }
}

pub fn context_trends(sessions: &[Session]) -> ContextTrend {
    let mut totals = ContextAggregate::default();
    let mut projects: BTreeMap<String, ContextAggregate> = BTreeMap::new();
    for session in sessions {
        let project = project_name(session);
        add_context_session(&mut totals, session);
        add_context_session(projects.entry(project).or_default(), session);
    }
    let totals_view = context_totals(&totals);
    let mut projects = projects
        .into_iter()
        .map(|(project, value)| ProjectContextTrend {
            project,
            sessions: value.sessions,
            avg_context_utilization_pct: if value.sessions == 0 {
                0.0
            } else {
                round4(value.context / value.sessions as f64)
            },
            cache_effectiveness_pct: pct(value.cache_read, value.input + value.cache_read),
            repeated_file_reads: value
                .files
                .values()
                .map(|count| count.saturating_sub(1))
                .sum(),
            read_to_write_ratio: ratio(value.reads, value.writes),
            cost_per_output_token: if value.output == 0 {
                0.0
            } else {
                round4(value.cost / value.output as f64)
            },
        })
        .collect::<Vec<_>>();
    projects.sort_by(|left, right| {
        right
            .sessions
            .cmp(&left.sessions)
            .then_with(|| left.project.cmp(&right.project))
    });
    ContextTrend {
        methodology: "Cross-session aggregate. Repeated reads are file surface occurrences, and cache effectiveness uses cache-read / (input + cache-read).".to_string(),
        totals: totals_view,
        projects,
    }
}

pub fn delivery_evidence(sessions: &[Session]) -> DeliveryEvidence {
    delivery_evidence_inner(sessions, false)
}

pub fn delivery_evidence_with_git(sessions: &[Session]) -> DeliveryEvidence {
    delivery_evidence_inner(sessions, true)
}

fn delivery_evidence_inner(sessions: &[Session], inspect_git: bool) -> DeliveryEvidence {
    let commits = if inspect_git {
        git_commits_by_root(sessions)
    } else {
        Default::default()
    };
    let mut summary = DeliverySummary::default();
    let mut records = Vec::new();
    for session in sessions {
        let project = resolve_project(session);
        let authority = &session.metrics.tool_authority;
        let matching_commits = commits
            .get(&project.root)
            .map(|commits| commits_for_session(commits, session))
            .unwrap_or_default();
        let (level, mut evidence, confidence) = if !matching_commits.is_empty() {
            (
                "strong",
                vec![format!(
                    "{} local Git commit(s) overlap the session time window",
                    matching_commits.len()
                )],
                "medium",
            )
        } else if authority.get("external_publish").copied().unwrap_or(0) > 0 {
            (
                "medium",
                vec!["observed external publish command category".to_string()],
                "medium",
            )
        } else if authority.get("git_write").copied().unwrap_or(0) > 0 {
            (
                "medium",
                vec!["observed git write command category".to_string()],
                "medium",
            )
        } else if authority.get("write_files").copied().unwrap_or(0) > 0 {
            (
                "weak",
                vec!["observed file write/edit command category".to_string()],
                "medium",
            )
        } else if authority.get("network_access").copied().unwrap_or(0) > 0
            || session.metrics.tool_calls_total > 0
        {
            (
                "non_code",
                vec!["tool activity observed without code-delivery evidence".to_string()],
                "low",
            )
        } else {
            (
                "none",
                vec!["no write, Git, publish, or tool evidence observed".to_string()],
                "low",
            )
        };
        if inspect_git && matching_commits.is_empty() && !project.root.is_empty() {
            evidence.push("no overlapping local commit found; this does not rule out uncommitted, remote, non-code, or later-delivered work".to_string());
        }
        match level {
            "strong" => summary.strong += 1,
            "medium" => summary.medium += 1,
            "weak" => summary.weak += 1,
            "non_code" => summary.non_code += 1,
            _ => summary.none += 1,
        }
        records.push(SessionDeliveryEvidence {
            session: session.name.clone(),
            project: project.display_name,
            level: level.to_string(),
            evidence,
            confidence: format!("{confidence}: time-window heuristic; Git commits are correlated, not attributable proof of main-merge or business value"),
        });
    }
    records.sort_by(|left, right| {
        left.level
            .cmp(&right.level)
            .then_with(|| left.session.cmp(&right.session))
    });
    DeliveryEvidence {
        methodology: if inspect_git {
            "Read-only local Git heuristic: commit timestamps are matched to session start/end with a 2-minute lead and 5-minute tail. It does not prove authorship, merge-to-main, or business value."
        } else {
            "Lightweight heuristic based on observed tool authority only. Run --delivery-evidence for read-only local Git timestamp correlation."
        }.to_string(),
        summary,
        sessions: records,
    }
}

#[derive(Clone)]
struct GitCommit {
    timestamp: DateTime<Utc>,
}

fn git_commits_by_root(sessions: &[Session]) -> BTreeMap<String, Vec<GitCommit>> {
    let roots = sessions
        .iter()
        .map(resolve_project)
        .map(|project| project.root)
        .filter(|root| !root.is_empty())
        .collect::<BTreeSet<_>>();
    roots
        .into_iter()
        .filter_map(|root| git_commits(&root).map(|commits| (root, commits)))
        .collect()
}

fn git_commits(root: &str) -> Option<Vec<GitCommit>> {
    let output = Command::new("git")
        .args(["-C", root, "log", "--all", "--format=%ct"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| line.parse::<i64>().ok())
            .filter_map(|timestamp| DateTime::from_timestamp(timestamp, 0))
            .map(|timestamp| GitCommit { timestamp })
            .collect(),
    )
}

fn commits_for_session<'a>(commits: &'a [GitCommit], session: &Session) -> Vec<&'a GitCommit> {
    let Some(start) = parse_timestamp(&session.metrics.session_start) else {
        return Vec::new();
    };
    let end = parse_timestamp(&session.metrics.session_end).unwrap_or(start);
    let start = start - Duration::minutes(2);
    let end = end + Duration::minutes(5);
    commits
        .iter()
        .filter(|commit| commit.timestamp >= start && commit.timestamp <= end)
        .collect()
}

fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

fn normalized_model(model: &str) -> String {
    if model.trim().is_empty() {
        "unknown".to_string()
    } else {
        model.to_string()
    }
}

#[allow(clippy::too_many_arguments)]
fn recommendation(
    id: &str,
    severity: &str,
    category: &str,
    title: &str,
    rationale: String,
    evidence: Vec<String>,
    estimated_savings_usd: f64,
    estimated_savings_tokens: i64,
    confidence: &str,
    action: &str,
    validation_command: &str,
) -> Recommendation {
    Recommendation {
        id: id.to_string(),
        priority: priority(severity).to_string(),
        severity: severity.to_string(),
        category: category.to_string(),
        title: title.to_string(),
        rationale,
        evidence,
        estimated_savings_usd: round4(estimated_savings_usd.max(0.0)),
        estimated_savings_tokens: estimated_savings_tokens.max(0),
        confidence: confidence.to_string(),
        action: action.to_string(),
        validation_command: validation_command.to_string(),
    }
}

fn recommendation_rank(item: &Recommendation) -> (u8, i64, i64, String) {
    (
        severity_rank(&item.severity),
        (item.estimated_savings_usd * 10_000.0) as i64,
        item.estimated_savings_tokens,
        item.title.clone(),
    )
}
fn severity_rank(value: &str) -> u8 {
    match value {
        "critical" => 4,
        "high" => 3,
        "warning" | "medium" => 2,
        _ => 1,
    }
}
fn priority(value: &str) -> &'static str {
    match severity_rank(value) {
        4 => "P0",
        3 => "P1",
        2 => "P2",
        _ => "P3",
    }
}
fn severity_from_cost(value: f64) -> &'static str {
    if value >= 10.0 {
        "high"
    } else {
        "medium"
    }
}
fn mcp_server_name(tool: &str) -> Option<String> {
    let tool = tool.trim();
    if let Some(value) = tool.strip_prefix("mcp__") {
        return value
            .split("__")
            .next()
            .filter(|value| !value.is_empty())
            .map(str::to_string);
    }
    if tool.starts_with("mcp_rca_mcp_") {
        return Some("rca-mcp".to_string());
    }
    None
}
fn pct(numerator: i64, denominator: i64) -> f64 {
    if denominator <= 0 {
        0.0
    } else {
        round4(numerator as f64 / denominator as f64 * 100.0)
    }
}
fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        round4(numerator as f64 / denominator as f64)
    }
}
fn add_context_session(aggregate: &mut ContextAggregate, session: &Session) {
    aggregate.sessions += 1;
    aggregate.context += session.diagnostics.context_utilization.utilization_pct;
    match session.diagnostics.context_utilization.risk_level.as_str() {
        "critical" => aggregate.critical += 1,
        "warning" => aggregate.warnings += 1,
        _ => {}
    }
    aggregate.cache_read += session.metrics.tokens_cache_r;
    aggregate.input += session.metrics.tokens_input;
    aggregate.output += session.metrics.tokens_output;
    aggregate.cost += session.metrics.cost_estimated;
    aggregate.reads += session
        .metrics
        .tool_usage
        .iter()
        .filter(|(tool, _)| is_read_tool(tool))
        .map(|(_, count)| count)
        .sum::<usize>();
    aggregate.writes += session
        .metrics
        .tool_usage
        .iter()
        .filter(|(tool, _)| is_write_tool(tool))
        .map(|(_, count)| count)
        .sum::<usize>();
    for (file, count) in &session.metrics.file_usage {
        *aggregate.files.entry(file.clone()).or_default() += count;
    }
}
fn context_totals(value: &ContextAggregate) -> ContextTrendTotals {
    ContextTrendTotals {
        sessions: value.sessions,
        context_warning_sessions: value.warnings,
        context_critical_sessions: value.critical,
        repeated_file_reads: value
            .files
            .values()
            .map(|count| count.saturating_sub(1))
            .sum(),
        cache_effectiveness_pct: pct(value.cache_read, value.input + value.cache_read),
        read_to_write_ratio: ratio(value.reads, value.writes),
        output_cost_per_million_tokens: if value.output == 0 {
            0.0
        } else {
            round4(value.cost / value.output as f64 * 1e6)
        },
    }
}
fn is_read_tool(tool: &str) -> bool {
    let tool = tool.to_ascii_lowercase();
    ["read", "glob", "grep", "find", "list", "view"]
        .iter()
        .any(|token| tool.contains(token))
}
fn is_write_tool(tool: &str) -> bool {
    let tool = tool.to_ascii_lowercase();
    ["write", "edit", "patch", "replace", "delete", "create"]
        .iter()
        .any(|token| tool.contains(token))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Diagnostics, Metrics, ToolWarning};

    fn session(name: &str) -> Session {
        Session {
            name: name.to_string(),
            path: format!("/tmp/{name}.jsonl"),
            cwd: "/tmp/project".to_string(),
            metrics: Metrics {
                model_used: "unknown".to_string(),
                tokens_input: 100,
                tokens_output: 20,
                cost_estimated: 1.0,
                ..Metrics::default()
            },
            anomalies: Vec::new(),
            health: 80,
            tool_warnings: Vec::new(),
            diagnostics: Diagnostics::default(),
        }
    }

    #[test]
    fn audit_marks_unknown_models_without_claiming_exact_pricing() {
        let audit = cost_audit(&[session("unknown-model")]);
        assert_eq!(audit.pricing_coverage.unpriced_or_unknown_sessions, 1);
        assert_eq!(audit.pricing_coverage.exact_pricing_pct, 0.0);
        assert_eq!(
            audit.by_provider_model[0].pricing_status,
            "unpriced_or_unknown"
        );
        let audit_json = serde_json::to_value(&audit).expect("cost audit json");
        assert_eq!(audit_json["stored_estimated_cost_usd"], 1.0);
        assert!(audit_json["current_estimated_cost_usd"].as_f64().is_some());
        assert!(
            audit_json["by_provider_model"][0]["stored_estimated_cost_usd"]
                .as_f64()
                .is_some()
        );
        let mut historical = session("unknown-model");
        historical.metrics.provenance.pricing_source = "historic price source".to_string();
        let session_audit = session_cost_audit(&historical);
        assert_eq!(session_audit.pricing_status, "unpriced_or_unknown");
        assert_eq!(session_audit.capability, "aggregate");
        assert_eq!(
            session_audit.pricing_source,
            pricing::pricing_source_for("unknown")
        );
        assert_eq!(session_audit.stored_pricing_source, "historic price source");
        assert!(session_audit.pricing_note.contains("generic"));
        let json = serde_json::to_value(&session_audit).expect("session audit json");
        assert_eq!(json["stored_estimated_cost_usd"], 1.0);
        assert!(json["estimated_cost_usd"].as_f64().is_some());
    }

    #[test]
    fn audit_marks_sqlite_multi_model_aggregates_without_exact_pricing() {
        let mut value = session("aggregate");
        value.metrics.model_used = "multiple".to_string();
        value.metrics.source_tool = "opencode_db".to_string();
        value.metrics.provenance.pricing_source = "SQLite aggregate: multiple models".to_string();
        let audit = session_cost_audit(&value);
        assert_eq!(audit.pricing_status, "aggregate_estimate");
        assert_eq!(audit.pricing_source, "unavailable: multiple models");
        assert_eq!(
            audit.stored_pricing_source,
            "SQLite aggregate: multiple models"
        );
        assert!(audit.rates_per_million_usd.is_none());
        assert!(audit.component_cost_usd.is_none());
        assert!(audit.estimated_cost_usd.is_none());
        assert_eq!(audit.stored_estimated_cost_usd, 1.0);
        assert!(audit.pricing_note.contains("multiple model IDs"));

        let aggregate = cost_audit(&[value]);
        assert!(aggregate.current_estimated_cost_usd.is_none());
        assert!(aggregate.by_provider_model[0].estimated_cost_usd.is_none());
        assert!(aggregate.by_provider_model[0].component_cost_usd.is_none());
    }

    #[test]
    fn recommendations_rank_context_pressure_before_low_severity_findings() {
        let mut pressured = session("pressured");
        pressured.diagnostics.context_utilization.risk_level = "critical".to_string();
        pressured.diagnostics.context_utilization.utilization_pct = 110.0;
        pressured
            .diagnostics
            .context_utilization
            .conversation_history = 1_000;
        let mut looping = session("looping");
        looping.diagnostics.loop_cost.total_loop_cost = 0.5;
        looping.diagnostics.loop_cost.loop_groups = 1;
        let items = recommendations(&[looping, pressured]);
        assert_eq!(items[0].category, "context");
        assert_eq!(items[0].priority, "P0");
    }

    #[test]
    fn mcp_governance_never_invents_loaded_coverage() {
        let mut value = session("mcp");
        value
            .metrics
            .tool_usage
            .insert("mcp__demo__lookup".to_string(), 2);
        value.tool_warnings.push(ToolWarning {
            tool_name: "mcp__demo__lookup".to_string(),
            pattern: "fail_retry_chain".to_string(),
            count: 1,
            detail: String::new(),
            severity: "high".to_string(),
        });
        let item = &mcp_governance(&[value]).items[0];
        assert_eq!(item.server, "demo");
        assert_eq!(item.invoked_sessions, 1);
        assert_eq!(item.loaded_sessions, None);
        assert_eq!(item.coverage_pct, None);
        assert_eq!(item.failed_calls, 1);
    }

    #[test]
    fn delivery_without_git_keeps_authority_evidence_heuristic() {
        let mut value = session("delivery");
        value
            .metrics
            .tool_authority
            .insert("git_write".to_string(), 1);
        let report = delivery_evidence(&[value]);
        assert_eq!(report.summary.medium, 1);
        assert!(report.methodology.contains("heuristic"));
        assert!(report.sessions[0].confidence.contains("not attributable"));
    }
}
