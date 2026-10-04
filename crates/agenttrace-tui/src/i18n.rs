#![cfg_attr(not(test), allow(dead_code))]

use super::{DetailSection, ExplorerView, Language};

#[derive(Debug, Clone, Copy)]
pub(super) enum UiText {
    ActionCenter,
    Efficiency,
    EstimatedSavings,
    PricingConfidence,
    ExactPriceMatch,
    NoObservedMcpCalls,
    NoPriorityFindings,
    CurrentSourceUnavailable,
    LanguageSaveFailed,
}

impl UiText {
    pub(super) fn get(self, language: Language) -> &'static str {
        pick(
            language,
            match self {
                Self::ActionCenter => "tui.ui.action_center",
                Self::Efficiency => "tui.ui.efficiency",
                Self::EstimatedSavings => "tui.ui.estimated_savings",
                Self::PricingConfidence => "tui.ui.pricing_confidence",
                Self::ExactPriceMatch => "tui.ui.exact_price_match",
                Self::NoObservedMcpCalls => "tui.ui.no_observed_mcp_calls",
                Self::NoPriorityFindings => "tui.ui.no_priority_findings",
                Self::CurrentSourceUnavailable => "tui.ui.current_source_unavailable",
                Self::LanguageSaveFailed => "tui.ui.language_save_failed",
            },
        )
    }
}

pub(super) fn pick(language: Language, key: &'static str) -> &'static str {
    agenttrace_core::tr(language, key)
}

pub(super) fn explorer_view_label(view: ExplorerView, language: Language) -> &'static str {
    match view {
        ExplorerView::Attention => pick(language, "tui.look_here_first"),
        ExplorerView::Recent => pick(language, "tui.recent"),
        ExplorerView::All => pick(language, "tui.all_sessions"),
        ExplorerView::Projects => pick(language, "tui.projects"),
        ExplorerView::Context => pick(language, "tui.context_size"),
        ExplorerView::Storage => pick(language, "tui.disk_size"),
        ExplorerView::Cost => pick(language, "tui.spend"),
        ExplorerView::Tools => pick(language, "tui.tools"),
    }
}

pub(super) fn explorer_view_description(view: ExplorerView, language: Language) -> &'static str {
    match view {
        ExplorerView::Attention => pick(language, "tui.sessions_that_look_unhealthy_or_expensive"),
        ExplorerView::Recent => pick(language, "tui.latest_sessions"),
        ExplorerView::All => pick(language, "tui.search_everything"),
        ExplorerView::Projects => pick(language, "tui.sessions_grouped_by_project"),
        ExplorerView::Context => pick(language, "tui.sessions_filling_up_their_context_window"),
        ExplorerView::Storage => pick(language, "tui.biggest_session_files_on_disk"),
        ExplorerView::Cost => pick(language, "tui.rough_token_spend_not_a_bill"),
        ExplorerView::Tools => pick(language, "tui.failures_slowness_and_loops"),
    }
}

pub(super) fn explorer_list_title(view: ExplorerView, language: Language) -> &'static str {
    match view {
        ExplorerView::Attention => pick(language, "tui.look_here_first"),
        ExplorerView::Recent => pick(language, "tui.recent_sessions_2"),
        ExplorerView::All => pick(language, "tui.all_sessions"),
        ExplorerView::Projects => pick(language, "tui.projects"),
        ExplorerView::Context => pick(language, "tui.context_filling_up"),
        ExplorerView::Storage => pick(language, "tui.largest_session_files"),
        ExplorerView::Cost => pick(language, "tui.estimated_spend"),
        ExplorerView::Tools => pick(language, "tui.tool_trouble"),
    }
}

pub(super) fn detail_section_label(section: DetailSection, language: Language) -> &'static str {
    match section {
        DetailSection::Summary => pick(language, "tui.summary"),
        DetailSection::Timeline => pick(language, "tui.what_happened"),
        DetailSection::Context => pick(language, "tui.context_2"),
        DetailSection::Files => pick(language, "tui.files_2"),
    }
}

pub(super) fn inspect_reason_label(reason: &str, language: Language) -> &'static str {
    match reason {
        "critical" => pick(language, "tui.unhealthy"),
        "anomaly" => pick(language, "tui.unusual"),
        "failures" => pick(language, "tui.tool_fails"),
        "context" => pick(language, "tui.context_risk_2"),
        "loops" => pick(language, "tui.repeat_loop"),
        "latency" => pick(language, "tui.slow"),
        "cost" => pick(language, "tui.costly"),
        "warning" => pick(language, "tui.needs_a_look"),
        _ => pick(language, "tui.ok_3"),
    }
}

pub(super) fn pricing_status_label(status: &str, language: Language) -> &'static str {
    match status {
        "catalog_estimate" => pick(language, "tui.priced_from_our_model_list"),
        "fallback_estimate" => pick(language, "tui.best_effort_price_no_exact_model_match"),
        "unpriced_or_unknown" => pick(language, "tui.can_t_price_this_model_yet"),
        "aggregate_estimate" => pick(language, "tui.aggregate_estimate_across_multiple_models"),
        _ => pick(language, "tui.unknown_price_status"),
    }
}

pub(super) fn capability_label(capability: &str, language: Language) -> &'static str {
    match capability {
        "detailed" => pick(language, "tui.full_details"),
        "aggregate" => pick(language, "tui.totals_only"),
        "limited" => pick(language, "tui.sparse_data"),
        _ => pick(language, "tui.unknown_data_coverage"),
    }
}

pub(super) fn provenance_label(value: &str, language: Language) -> &'static str {
    match value {
        "reported_by_agent" => pick(language, "tui.recorded_by_the_agent"),
        "estimated_from_text" => pick(language, "tui.estimated_from_text"),
        "timestamp_span" => pick(language, "tui.calculated_from_timestamps"),
        "reported_or_inferred" => pick(language, "tui.recorded_or_inferred"),
        "calculated_from_tokens" => pick(language, "tui.calculated_from_tokens"),
        "calculated_per_message_tokens" => {
            pick(language, "tui.calculated_per_sqlite_message_tokens")
        }
        "tool_arguments" => pick(language, "tui.found_in_tool_arguments"),
        "unavailable" | "" => pick(language, "tui.not_available_2"),
        _ => pick(language, "tui.source_unknown"),
    }
}

pub(super) fn risk_label(risk: &str, language: Language) -> &'static str {
    match risk {
        "critical" => pick(language, "tui.critical"),
        "warning" => pick(language, "tui.warning"),
        "ok" | "normal" | "" => pick(language, "tui.ok_3"),
        _ => pick(language, "tui.unknown_risk"),
    }
}

pub(super) fn command_choices(language: Language) -> [(&'static str, &'static str); 10] {
    [
        (pick(language, "tui.open_look_here_first"), "view:attention"),
        (pick(language, "tui.open_context_size"), "view:context"),
        (pick(language, "tui.open_disk_size"), "view:storage"),
        (pick(language, "tui.open_projects"), "view:projects"),
        (pick(language, "tui.open_spend"), "view:cost"),
        (pick(language, "tui.open_tools"), "view:tools"),
        (
            pick(language, "tui.only_sessions_with_context_risk"),
            "filter:context",
        ),
        (pick(language, "tui.clear_filters"), "clear"),
        (pick(language, "tui.switch_language"), "language"),
        (pick(language, "tui.reload_sessions"), "reload"),
    ]
}
