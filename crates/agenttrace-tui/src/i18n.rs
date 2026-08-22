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
        let en = match self {
            Self::ActionCenter => "Next steps",
            Self::Efficiency => "Efficiency",
            Self::EstimatedSavings => "estimated savings",
            Self::PricingConfidence => "how sure the prices are",
            Self::ExactPriceMatch => "exact model match",
            Self::NoObservedMcpCalls => "No MCP tool calls showed up in these sessions.",
            Self::NoPriorityFindings => "Nothing urgent in the current filter.",
            Self::CurrentSourceUnavailable => "can't filter by this source",
            Self::LanguageSaveFailed => "couldn't save the language setting",
        };
        let zh = match self {
            Self::ActionCenter => "接下来做什么",
            Self::Efficiency => "效率",
            Self::EstimatedSavings => "大概能省多少",
            Self::PricingConfidence => "价格把握有多大",
            Self::ExactPriceMatch => "模型对得上",
            Self::NoObservedMcpCalls => "这些会话里没看到 MCP 工具调用。",
            Self::NoPriorityFindings => "按当前筛选，没有急需处理的问题。",
            Self::CurrentSourceUnavailable => "没法按这个来源筛选",
            Self::LanguageSaveFailed => "语言设置没保存成功",
        };
        pick(language, en, zh)
    }
}

pub(super) fn pick(language: Language, en: &'static str, zh: &'static str) -> &'static str {
    match language {
        Language::En => en,
        Language::Zh => zh,
    }
}

pub(super) fn explorer_view_label(view: ExplorerView, language: Language) -> &'static str {
    match view {
        ExplorerView::Attention => pick(language, "Look here first", "先看这些"),
        ExplorerView::Recent => pick(language, "Recent", "最近"),
        ExplorerView::All => pick(language, "All sessions", "全部会话"),
        ExplorerView::Projects => pick(language, "Projects", "项目"),
        ExplorerView::Context => pick(language, "Context size", "上下文占用"),
        ExplorerView::Storage => pick(language, "Disk size", "占用空间"),
        ExplorerView::Cost => pick(language, "Spend", "花费"),
        ExplorerView::Tools => pick(language, "Tools", "工具"),
    }
}

pub(super) fn explorer_view_description(view: ExplorerView, language: Language) -> &'static str {
    match view {
        ExplorerView::Attention => pick(
            language,
            "Sessions that look unhealthy or expensive",
            "看起来不健康或很贵的会话",
        ),
        ExplorerView::Recent => pick(language, "Latest sessions", "最近用过的会话"),
        ExplorerView::All => pick(language, "Search everything", "搜索全部会话"),
        ExplorerView::Projects => pick(language, "Sessions grouped by project", "按项目查看会话"),
        ExplorerView::Context => pick(
            language,
            "Sessions filling up their context window",
            "上下文快撑满的会话",
        ),
        ExplorerView::Storage => pick(
            language,
            "Biggest session files on disk",
            "磁盘上最大的会话文件",
        ),
        ExplorerView::Cost => pick(
            language,
            "Rough token spend, not a bill",
            "大概花了多少，不是账单",
        ),
        ExplorerView::Tools => pick(
            language,
            "Failures, slowness, and loops",
            "失败、偏慢和反复调用",
        ),
    }
}

pub(super) fn explorer_list_title(view: ExplorerView, language: Language) -> &'static str {
    match view {
        ExplorerView::Attention => pick(language, "Look here first", "先看这些"),
        ExplorerView::Recent => pick(language, "Recent sessions", "最近会话"),
        ExplorerView::All => pick(language, "All sessions", "全部会话"),
        ExplorerView::Projects => pick(language, "Projects", "项目"),
        ExplorerView::Context => pick(language, "Context filling up", "上下文快满了"),
        ExplorerView::Storage => pick(language, "Largest session files", "最大的会话文件"),
        ExplorerView::Cost => pick(language, "Estimated spend", "估算花费"),
        ExplorerView::Tools => pick(language, "Tool trouble", "工具出问题"),
    }
}

pub(super) fn detail_section_label(section: DetailSection, language: Language) -> &'static str {
    match section {
        DetailSection::Summary => pick(language, "Summary", "摘要"),
        DetailSection::Timeline => pick(language, "What happened", "发生了什么"),
        DetailSection::Context => pick(language, "Context", "上下文"),
        DetailSection::Files => pick(language, "Files", "文件"),
    }
}

pub(super) fn inspect_reason_label(reason: &str, language: Language) -> &'static str {
    match reason {
        "critical" => pick(language, "unhealthy", "不健康"),
        "anomaly" => pick(language, "unusual", "异常"),
        "failures" => pick(language, "tool fails", "工具失败"),
        "context" => pick(language, "context risk", "上下文风险"),
        "loops" => pick(language, "repeat loop", "反复调用"),
        "latency" => pick(language, "slow", "偏慢"),
        "cost" => pick(language, "costly", "偏贵"),
        "warning" => pick(language, "needs a look", "建议看看"),
        _ => pick(language, "ok", "正常"),
    }
}

pub(super) fn pricing_status_label(status: &str, language: Language) -> &'static str {
    match status {
        "catalog_estimate" => pick(language, "priced from our model list", "按模型价目表估算"),
        "fallback_estimate" => pick(
            language,
            "best-effort price, no exact model match",
            "没有对上确切模型，按兜底价格估算",
        ),
        "unpriced_or_unknown" => pick(
            language,
            "can't price this model yet",
            "这个模型暂时没法估价",
        ),
        "aggregate_estimate" => pick(
            language,
            "aggregate estimate across multiple models",
            "多个模型聚合估算",
        ),
        _ => pick(language, "unknown price status", "价格状态未知"),
    }
}

pub(super) fn capability_label(capability: &str, language: Language) -> &'static str {
    match capability {
        "detailed" => pick(language, "full details", "细节齐全"),
        "aggregate" => pick(language, "totals only", "只有合计"),
        "limited" => pick(language, "sparse data", "数据很少"),
        _ => pick(language, "unknown data coverage", "数据覆盖未知"),
    }
}

pub(super) fn provenance_label(value: &str, language: Language) -> &'static str {
    match value {
        "reported_by_agent" => pick(language, "recorded by the agent", "Agent 直接记录"),
        "estimated_from_text" => pick(language, "estimated from text", "根据文本估算"),
        "timestamp_span" => pick(language, "calculated from timestamps", "根据时间戳计算"),
        "reported_or_inferred" => pick(language, "recorded or inferred", "直接记录或推断"),
        "calculated_from_tokens" => pick(language, "calculated from tokens", "根据 token 重算"),
        "calculated_per_message_tokens" => pick(
            language,
            "calculated per SQLite message tokens",
            "按 SQLite 消息 token 计算",
        ),
        "tool_arguments" => pick(language, "found in tool arguments", "从工具参数中提取"),
        "unavailable" | "" => pick(language, "not available", "没有数据"),
        _ => pick(language, "source unknown", "来源未知"),
    }
}

pub(super) fn risk_label(risk: &str, language: Language) -> &'static str {
    match risk {
        "critical" => pick(language, "critical", "严重"),
        "warning" => pick(language, "warning", "警告"),
        "ok" | "normal" | "" => pick(language, "ok", "正常"),
        _ => pick(language, "unknown risk", "风险未知"),
    }
}

pub(super) fn command_choices(language: Language) -> [(&'static str, &'static str); 10] {
    [
        (
            pick(language, "Open Look here first", "打开「先看这些」"),
            "view:attention",
        ),
        (
            pick(language, "Open Context size", "打开「上下文占用」"),
            "view:context",
        ),
        (
            pick(language, "Open Disk size", "打开「占用空间」"),
            "view:storage",
        ),
        (
            pick(language, "Open Projects", "打开「项目」"),
            "view:projects",
        ),
        (pick(language, "Open Spend", "打开「花费」"), "view:cost"),
        (pick(language, "Open Tools", "打开「工具」"), "view:tools"),
        (
            pick(
                language,
                "Only sessions with context risk",
                "只看上下文有风险的会话",
            ),
            "filter:context",
        ),
        (pick(language, "Clear filters", "清除筛选"), "clear"),
        (pick(language, "Switch language", "切换语言"), "language"),
        (pick(language, "Reload sessions", "重新加载"), "reload"),
    ]
}
