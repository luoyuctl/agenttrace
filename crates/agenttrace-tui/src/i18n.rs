use super::Language;

#[derive(Debug, Clone, Copy)]
pub(super) enum UiText {
    Overview,
    List,
    Detail,
    Diagnostics,
    Diff,
    Workspace,
    Help,
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
            Self::Overview => "Overview",
            Self::List => "List",
            Self::Detail => "Detail",
            Self::Diagnostics => "Diagnostics",
            Self::Diff => "Diff",
            Self::Workspace => "Workspace",
            Self::Help => "Help",
            Self::ActionCenter => "Action Center",
            Self::Efficiency => "Efficiency",
            Self::EstimatedSavings => "estimated savings",
            Self::PricingConfidence => "pricing confidence",
            Self::ExactPriceMatch => "exact match",
            Self::NoObservedMcpCalls => "No observed MCP calls.",
            Self::NoPriorityFindings => "No prioritized findings in the current scope.",
            Self::CurrentSourceUnavailable => "source filter unavailable",
            Self::LanguageSaveFailed => "could not save language preference",
        };
        let zh = match self {
            Self::Overview => "概览",
            Self::List => "列表",
            Self::Detail => "详情",
            Self::Diagnostics => "诊断",
            Self::Diff => "对比",
            Self::Workspace => "工作台",
            Self::Help => "帮助",
            Self::ActionCenter => "行动中心",
            Self::Efficiency => "效率",
            Self::EstimatedSavings => "预估节省",
            Self::PricingConfidence => "价格可信度",
            Self::ExactPriceMatch => "精确匹配",
            Self::NoObservedMcpCalls => "未观察到 MCP 调用。",
            Self::NoPriorityFindings => "当前范围没有优先问题。",
            Self::CurrentSourceUnavailable => "来源筛选不可用",
            Self::LanguageSaveFailed => "无法保存语言偏好",
        };
        match language {
            Language::En => en,
            Language::Zh => zh,
        }
    }
}
