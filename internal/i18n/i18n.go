// Package i18n provides bilingual (English/Chinese) translation support for agentwaste.
package i18n

// Lang represents a language code.
type Lang string

const (
	EN Lang = "en"
	ZH Lang = "zh"
)

// Current is the active language for translation lookups.
var Current Lang = "en"

// translations maps translation keys to their per-language strings.
// Format strings use Go's fmt conventions (%s, %d, %.1f, etc.).
var translations = map[string]map[Lang]string{
	// ── Report title ──
	"title": {
		EN: "AGENTWASTE v%s — AI Agent Session Performance Report",
		ZH: "AGENTWASTE v%s — AI 代理会话性能报告",
	},
	"compare_title": {
		EN: "AGENTWASTE — Multi-Session Comparison",
		ZH: "AGENTWASTE — 多会话对比",
	},

	// ── Section headers ──
	"waste_cost": {
		EN: "💸 MONEY WASTE",
		ZH: "💸 金钱浪费",
	},
	"activity": {
		EN: "📊 ACTIVITY",
		ZH: "📊 活动概览",
	},
	"latency": {
		EN: "⏱️  LATENCY",
		ZH: "⏱️  延迟分析",
	},
	"top_tools": {
		EN: "🔧 TOP TOOLS",
		ZH: "🔧 常用工具",
	},
	"thinking_cot": {
		EN: "🧠 THINKING / COT",
		ZH: "🧠 思维链分析",
	},
	"anomalies": {
		EN: "🚨 ANOMALIES",
		ZH: "🚨 异常检测",
	},
	"health_score": {
		EN: "💯 HEALTH SCORE",
		ZH: "💯 健康评分",
	},

	// ── Token cost labels ──
	"input": {
		EN: "Input:       %10d  tokens",
		ZH: "输入:        %10d  tokens",
	},
	"output": {
		EN: "Output:      %10d  tokens",
		ZH: "输出:        %10d  tokens",
	},
	"cache_write": {
		EN: "Cache write: %10d  tokens",
		ZH: "缓存写入:    %10d  tokens",
	},
	"cache_read": {
		EN: "Cache read:  %10d  tokens",
		ZH: "缓存读取:    %10d  tokens",
	},
	"total_tokens": {
		EN: "Total tokens: %10d",
		ZH: "总计 tokens:  %10d",
	},
	"est_cost": {
		EN: "Money wasted: $%11.4f  (model: %s)",
		ZH: "烧掉的钱:   $%11.4f  (模型: %s)",
	},

	// ── Activity labels ──
	"messages_label": {
		EN: "Messages:    %d user  |  %d turns",
		ZH: "消息数:      %d 用户  |  %d 轮次",
	},
	"tool_calls_label": {
		EN: "Tool calls:  %d",
		ZH: "工具调用:    %d",
	},
	"success_label": {
		EN: "Success:     %s (%d/%d) %s",
		ZH: "成功率:      %s (%d/%d) %s",
	},

	// ── Latency labels ──
	"min_lat": {
		EN: "min:     %.1fs",
		ZH: "最小:    %.1fs",
	},
	"median": {
		EN: "median:  %.1fs",
		ZH: "中位数:  %.1fs",
	},
	"p95": {
		EN: "p95:     %.1fs",
		ZH: "P95:     %.1fs",
	},
	"max_lat": {
		EN: "max:     %.1fs",
		ZH: "最大:    %.1fs",
	},
	"avg_lat": {
		EN: "avg:     %.1fs",
		ZH: "平均:    %.1fs",
	},
	"no_gap_data": {
		EN: "(no gap data)",
		ZH: "(无间隔数据)",
	},
	"duration": {
		EN: "Duration: %s",
		ZH: "持续时长: %s",
	},

	// ── Thinking/COT labels ──
	"blocks": {
		EN: "Blocks: %d",
		ZH: "思维块: %d",
	},
	"avg_chars": {
		EN: "Avg:    %.0f chars",
		ZH: "平均:   %.0f 字符",
	},
	"total_chars": {
		EN: "Total:  %d chars",
		ZH: "总计:   %d 字符",
	},
	"quality_label": {
		EN: "Quality: %s %s",
		ZH: "质量:   %s %s",
	},
	"quality_deep": {
		EN: "deep",
		ZH: "深度",
	},
	"quality_shallow": {
		EN: "shallow",
		ZH: "浅层",
	},
	"quality_moderate": {
		EN: "moderate",
		ZH: "中等",
	},
	"no_thinking_blocks": {
		EN: "(no thinking blocks)",
		ZH: "(无思维块)",
	},
	"redacted_blocks": {
		EN: "⚠️  %d blocks REDACTED",
		ZH: "⚠️  %d 思维块已脱敏",
	},

	// ── Anomalies ──
	"no_anomalies": {
		EN: "✅ No anomalies detected",
		ZH: "✅ 未检测到异常",
	},

	// ── TUI table headers ──
	"session": {
		EN: "SESSION",
		ZH: "会话",
	},
	"turns_header": {
		EN: "TURNS",
		ZH: "轮次",
	},
	"tools": {
		EN: "TOOLS",
		ZH: "工具",
	},
	"succ_pct": {
		EN: "SUCC%",
		ZH: "成功率",
	},
	"cost": {
		EN: "COST",
		ZH: "费用",
	},
	"health": {
		EN: "HEALTH",
		ZH: "健康",
	},
	"fail": {
		EN: "FAIL",
		ZH: "失败",
	},
	"tokens": {
		EN: "TOKENS",
		ZH: "TOKEN",
	},

	// ── TUI views ──
	"agentwaste_title": {
		EN: "💸 AGENTWASTE v%s",
		ZH: "💸 AGENTWASTE v%s",
	},
	"sessions_count": {
		EN: " %d sessions ",
		ZH: " %d 个会话 ",
	},
	"tab_list": {
		EN: "1 Session List",
		ZH: "1 会话列表",
	},
	"tab_detail": {
		EN: "2 Detail",
		ZH: "2 详情",
	},
	"tab_compare": {
		EN: "5 Compare",
		ZH: "5 对比",
	},
	"select_session_hint": {
		EN: " Select a session and press Enter to see details ",
		ZH: " 选择会话并按回车查看详情 ",
	},

	// ── TUI help bars ──
	"help_overview": {
		EN: "0-4: jump · Tab: list · q: quit",
		ZH: "0-4: 跳转 · Tab: 列表 · q: 退出",
	},
	"help_list": {
		EN: "↑↓:sel Enter:detail 0-4:jump h/c/t:sort /:filt f:health s:source d:diff w:diag Tab:next q:quit",
		ZH: "↑↓:选择 Enter:详情 0-4:跳转 h/c/t:排序 /:筛选 f:健康 s:来源 d:对比 w:诊断 Tab:下页 q:退出",
	},
	"help_detail": {
		EN: "Esc: back · d: diff prev · w: diag · Tab: next · q: quit",
		ZH: "Esc: 返回 · d: 对比上一 · w: 诊断 · Tab: 下一个 · q: 退出",
	},
	"help_compare": {
		EN: "h: sort health · 0-5: jump · Tab: overview · q: quit",
		ZH: "h: 健康排序 · 0-5: 跳转 · Tab: 概览 · q: 退出",
	},

	// ── CLI / main ──
	"supported_models": {
		EN: "agentwaste v%s — Supported Models",
		ZH: "agentwaste v%s — 支持模型",
	},
	"model_header": {
		EN: "Model",
		ZH: "模型",
	},
	"input_per_m": {
		EN: "Input $/M",
		ZH: "输入$/M",
	},
	"output_per_m": {
		EN: "Output $/M",
		ZH: "输出$/M",
	},

	// ── Anomaly details ──
	"severity_high": {
		EN: "HIGH",
		ZH: "严重",
	},
	"severity_medium": {
		EN: "MEDIUM",
		ZH: "中等",
	},
	"severity_low": {
		EN: "LOW",
		ZH: "轻微",
	},
	"anomaly_hanging_detail": {
		EN: "%d gap(s) >60s, max=%.0fs",
		ZH: "%d个间隔 >60秒, 最长=%.0f秒",
	},
	"anomaly_latency_detail": {
		EN: "p95 latency = %.1fs",
		ZH: "P95延迟 = %.1f秒",
	},
	"anomaly_tool_fail_detail": {
		EN: "%d/%d failed (%.0f%%)",
		ZH: "%d/%d 失败 (%.0f%%)",
	},
	"anomaly_shallow_detail": {
		EN: "avg reasoning = %.0f chars (very shallow)",
		ZH: "平均推理 = %.0f 字符 (极浅)",
	},
	"anomaly_shallow_medium_detail": {
		EN: "avg reasoning = %.0f chars",
		ZH: "平均推理 = %.0f 字符",
	},
	"anomaly_redaction_detail": {
		EN: "%d block(s) redacted",
		ZH: "%d 思维块已脱敏",
	},
	"anomaly_no_tools_detail": {
		EN: "no tool calls — chat-only session",
		ZH: "无工具调用 — 纯对话会话",
	},

	// ── CLI / main extras ──
	"default_model_label": {
		EN: "  default = claude-sonnet-4 ($%.2f/$%.2f)",
		ZH: "  默认 = claude-sonnet-4 ($%.2f/$%.2f)",
	},

	// ── Source tool ──
	"source_tool": {
		EN: "SOURCE",
		ZH: "来源",
	},

	// ── Compare JSON ──
	"no_session_files": {
		EN: "No session files found in %s",
		ZH: "在 %s 中未找到会话文件",
	},

	"compare_truncated": {
		EN: "Found %d session files, showing the most recent 15. Use -d <dir> or remove old sessions to compare all.",
		ZH: "发现 %d 个会话文件，仅展示最近 15 个。使用 -d <dir> 或清理旧会话以对比全部。",
	},

	// ── Empty state ──
	"empty_sessions_hint": {
		EN: " No AI agent sessions found.\n\n Try: agentwaste --latest -d %s\n      agentwaste --compare -d %s\n\n Place session JSON/JSONL files in %s ",
		ZH: " 未找到 AI 代理会话。\n\n 尝试: agentwaste --latest -d %s\n       agentwaste --compare -d %s\n\n 将会话 JSON/JSONL 文件放在 %s ",
	},
	"lang_label": {
		EN: "EN",
		ZH: "中文",
	},

	// ── Generic / misc ──
	"model_label": {
		EN: "model: %s",
		ZH: "模型: %s",
	},
	"separator_double": {
		EN: "━",
		ZH: "━",
	},
	"separator_single": {
		EN: "─",
		ZH: "─",
	},

	// ── Overview ──
	"overview_title": {
		EN: "Global Overview",
		ZH: "全局概览",
	},
	"overview_total": {
		EN: "Total Sessions",
		ZH: "总会话数",
	},
	"overview_healthy": {
		EN: "Healthy",
		ZH: "健康",
	},
	"overview_warning": {
		EN: "Warning",
		ZH: "警告",
	},
	"overview_critical": {
		EN: "Critical",
		ZH: "严重",
	},
	"overview_agents": {
		EN: "By Agent",
		ZH: "按 Agent",
	},
	"overview_models": {
		EN: "By Model",
		ZH: "按模型",
	},
	"overview_recent_anomalies": {
		EN: "Recent Anomalies",
		ZH: "近期异常",
	},
	"overview_no_anomalies": {
		EN: "✅ No anomalies",
		ZH: "✅ 无异常",
	},
	"tab_overview": {
		EN: "0 Overview",
		ZH: "0 概览",
	},
	"total_sessions": {
		EN: "Total Sessions",
		ZH: "总会话",
	},
	"healthy": {
		EN: "Healthy",
		ZH: "健康",
	},
	"warning": {
		EN: "Warning",
		ZH: "警告",
	},
	"critical": {
		EN: "Critical",
		ZH: "严重",
	},
	"anomalies_detected": {
		EN: "Anomalies Detected",
		ZH: "检测到异常",
	},
	"total_cost": {
		EN: "Money Wasted",
		ZH: "烧掉的钱",
	},
	"health_trend": {
		EN: "Health Trend",
		ZH: "健康趋势",
	},
	"top_models": {
		EN: "Top Models",
		ZH: "热门模型",
	},
	"agent_breakdown": {
		EN: "Agent Breakdown",
		ZH: "Agent 分布",
	},
	"recent_anomalies": {
		EN: "Recent Anomalies",
		ZH: "近期异常",
	},
	"avg_health": {
		EN: "Avg Health",
		ZH: "平均健康",
	},
	"sessions_label": {
		EN: "sessions",
		ZH: "会话",
	},
	"last_scan": {
		EN: "Last scan",
		ZH: "上次扫描",
	},
	"no_data": {
		EN: "No data",
		ZH: "暂无数据",
	},

	// ── Filter ──
	"filter_label": {
		EN: "Filter",
		ZH: "筛选",
	},
	"filter_placeholder": {
		EN: "Type to filter...",
		ZH: "输入关键词筛选...",
	},
	"filter_all": {
		EN: "All",
		ZH: "全部",
	},
	"filter_healthy": {
		EN: "Healthy",
		ZH: "健康",
	},
	"filter_warning": {
		EN: "Warning",
		ZH: "警告",
	},
	"filter_critical": {
		EN: "Critical",
		ZH: "严重",
	},
	"filter_has_anomaly": {
		EN: "Has Anomaly",
		ZH: "有异常",
	},
	"filter_source": {
		EN: "Source",
		ZH: "来源",
	},
	"filter_clear": {
		EN: "Clear Filter",
		ZH: "清除筛选",
	},

	// ── Loop Cost ──
	"loop_detected": {
		EN: "Loop Detected",
		ZH: "检测到循环",
	},
	"retry_loop": {
		EN: "Retry Loop",
		ZH: "重试循环",
	},
	"tool_loop": {
		EN: "Tool Loop",
		ZH: "工具循环",
	},
	"loop_turns": {
		EN: "Loop Turns",
		ZH: "循环轮次",
	},
	"loop_cost": {
		EN: "Loop Cost",
		ZH: "循环成本",
	},
	"no_loop": {
		EN: "No Loop Detected",
		ZH: "未检测到循环",
	},

	// ── TUI help bars (extended) ──
	"help_filter": {
		EN: "Filter",
		ZH: "筛选",
	},
	"help_tab": {
		EN: "Tab switch view",
		ZH: "Tab 切换视图",
	},
	"key_tab": {
		EN: "Tab",
		ZH: "Tab",
	},

	// ── Fix suggestions ──
	"fix_title": {EN: "💡 Fix Suggestions", ZH: "💡 修复建议"},
	"fix_hanging": {EN: "Add tool timeout protection — detected %d gaps >60s", ZH: "添加工具超时保护 — 检测到 %d 个间隔 >60s"},
	"fix_tool_fail": {EN: "Check tool schemas — %d/%d calls failed", ZH: "检查工具 Schema — %d/%d 调用失败"},
	"fix_shallow": {EN: "Increase reasoning depth — avg %.0f chars per block", ZH: "增加推理深度 — 平均每块 %.0f 字符"},
	"fix_redaction": {EN: "Review redaction config — %d blocks hidden", ZH: "检查脱敏配置 — %d 思维块被隐藏"},
	"fix_no_tools": {EN: "Enable tool calling for this agent", ZH: "为此 agent 启用工具调用"},

	// ── Diff ──
	"diff_title": {EN: "Session Diff", ZH: "会话对比"},
	"diff_field_health": {EN: "Health", ZH: "健康"},
	"diff_field_cost": {EN: "Cost", ZH: "费用"},
	"diff_field_turns": {EN: "Turns", ZH: "轮次"},
	"diff_field_tools": {EN: "Tools", ZH: "工具"},
	"diff_field_success": {EN: "Success", ZH: "成功率"},
	"diff_field_fail": {EN: "Fail", ZH: "失败"},
	"diff_field_duration": {EN: "Duration", ZH: "时长"},
	"diff_field_model": {EN: "Model", ZH: "模型"},
	"diff_field_anomalies": {EN: "Anomalies", ZH: "异常"},
	"diff_better": {EN: "better", ZH: "更好"},
	"diff_worse": {EN: "worse", ZH: "更差"},
	"diff_same": {EN: "same", ZH: "相同"},

	// ── Cost alert ──
	"cost_alert_title": {EN: "🚨 Waste Alert", ZH: "🚨 浪费预警"},
	"cost_alert_critical": {EN: "This session burned $%.2f/turn (%.0fx avg $%.2f/turn)", ZH: "本会话单轮烧掉 $%.2f（是平均 $%.2f 的 %.0f 倍）"},
	"cost_alert_warning": {EN: "Loop waste is %.0f%% of total — consider adding circuit breaker", ZH: "循环浪费占总浪费 %.0f%% — 建议添加熔断机制"},

	// ── Health trend ──
	"trend_title": {EN: "Health Trend", ZH: "健康趋势"},
	"trend_regressing": {EN: "📉 Declining: %d→%d→%d", ZH: "📉 持续下降: %d→%d→%d"},
	"trend_stable": {EN: "📊 Stable at %d", ZH: "📊 稳定在 %d"},
	"trend_improving": {EN: "📈 Improving: %d→%d→%d", ZH: "📈 持续上升: %d→%d→%d"},

	// ── Tool warnings ──
	"tool_warn_title": {EN: "⚠️ Tool Warnings", ZH: "⚠️ 工具调用警告"},
	"tool_warn_dead_loop": {EN: "%s called %dx consecutively — possible dead loop", ZH: "%s 连续调用 %d 次 — 疑似死循环"},
	"tool_warn_empty_args": {EN: "%s called with empty arguments", ZH: "%s 调用参数为空"},
	"tool_warn_retry": {EN: "%s retried %dx after failures", ZH: "%s 失败后重试 %d 次"},
	"tool_warn_redundant": {EN: "%s called %dx with same args — redundant", ZH: "%s 重复调用 %d 次 — 冗余"},

	// ── Prompt impact ──
	"prompt_impact_title": {EN: "📝 Prompt Impact", ZH: "📝 Prompt 影响"},
	"prompt_impact_improving": {EN: "Trend: improving — keep current approach", ZH: "趋势: 改善中 — 保持当前策略"},
	"prompt_impact_worsening": {EN: "Trend: worsening — consider rolling back", ZH: "趋势: 恶化 — 考虑回滚"},
	"prompt_impact_mixed": {EN: "Trend: mixed — need more data", ZH: "趋势: 波动 — 需要更多数据"},

	// ── Tab ──
	"tab_diff": {EN: "4 Diff", ZH: "4 对比"},

	// ── Help ──
	"help_diag": {EN: "Esc: back · Tab: next · q: quit", ZH: "Esc: 返回 · Tab: 下一个 · q: 退出"},
	"help_diff": {EN: "Esc: back · Tab: overview · q: quit", ZH: "Esc: 返回 · Tab: 概览 · q: 退出"},

	// ── v0.2 Community Diagnostics ──
	"diag_loop_fingerprint": {
		EN: "🔍 LOOP FINGERPRINT DETECTION",
		ZH: "🔍 指纹级死循环检测",
	},
	"diag_loop_critical": {
		EN: "CRITICAL: '%s' returned identical result %dx — likely dead loop",
		ZH: "严重: '%s' 返回相同结果 %d 次 — 疑似死循环",
	},
	"diag_loop_high": {
		EN: "WARNING: '%s' repeated %dx without progress",
		ZH: "警告: '%s' 无进展重复 %d 次",
	},
	"diag_no_loop_fp": {
		EN: "No fingerprint loops detected — agent progressing normally",
		ZH: "未检测到指纹级循环 — Agent 正常推进中",
	},
	"diag_tool_latency": {
		EN: "⏱️  PER-TOOL LATENCY RANKING",
		ZH: "⏱️  工具延迟排名",
	},
	"diag_tool_lat_col_name":    {EN: "Tool", ZH: "工具"},
	"diag_tool_lat_col_count":   {EN: "Calls", ZH: "调用"},
	"diag_tool_lat_col_avg":     {EN: "Avg", ZH: "平均"},
	"diag_tool_lat_col_p95":     {EN: "P95", ZH: "P95"},
	"diag_tool_lat_col_max":     {EN: "Max", ZH: "最大"},
	"diag_tool_lat_col_timeout": {EN: "T/O", ZH: "超时"},
	"diag_tool_slow_badge":      {EN: "SLOW", ZH: "慢"},
	"diag_context_util": {
		EN: "📐 CONTEXT WINDOW UTILIZATION",
		ZH: "📐 上下文窗口利用率",
	},
	"diag_ctx_total":     {EN: "Est. Context Window", ZH: "预估上下文窗口"},
	"diag_ctx_tool_defs": {EN: "Tool Definitions", ZH: "工具定义"},
	"diag_ctx_history":   {EN: "Conversation History", ZH: "对话历史"},
	"diag_ctx_sysprompt": {EN: "System Prompt", ZH: "系统提示"},
	"diag_ctx_available": {EN: "Available for Task", ZH: "任务可用空间"},
	"diag_ctx_suggestion": {EN: "Recommendation", ZH: "建议"},
	"diag_large_params": {
		EN: "📦 LARGE PARAMETER CALLS",
		ZH: "📦 大参数调用检测",
	},
	"diag_large_params_none": {
		EN: "No oversized calls detected",
		ZH: "未检测到大参数调用",
	},
	"diag_unused_tools": {
		EN: "🔧 TOOL USAGE EFFICIENCY",
		ZH: "🔧 工具使用效率",
	},
	"diag_unused_none": {
		EN: "All registered tools used efficiently",
		ZH: "所有工具使用效率正常",
	},
	"diag_cost_summary": {
		EN: "💰 CUMULATIVE COST SUMMARY",
		ZH: "💰 累计成本概览",
	},
	"diag_cost_total_sessions": {EN: "Total Sessions", ZH: "总会话"},
	"diag_cost_total_burned":   {EN: "Total $ Burned", ZH: "累计烧钱"},
	"diag_cost_avg_turn":       {EN: "Avg Cost/Turn", ZH: "平均每轮成本"},
	"diag_cost_costliest":      {EN: "Costliest Model", ZH: "最烧钱模型"},
	"diag_tokens_total":        {EN: "Total Tokens (in/out)", ZH: "总Token (输入/输出)"},
	"diag_cache_total":         {EN: "Cache (read/write)", ZH: "缓存 (读/写)"},
	"diag_zombie_calls": {
		EN: "Zombie tool calls: %d call(s) with no result — may indicate timeout",
		ZH: "僵尸工具调用: %d 个调用无响应 — 可能超时",
	},
	"diag_repeated_response": {
		EN: "Repeated response: assistant said same thing %dx — possible drift",
		ZH: "重复回复: assistant 重复相同内容 %d 次 — 可能偏移",
	},
	"diag_no_stuck": {
		EN: "No stuck patterns detected",
		ZH: "未检测到卡顿模式",
	},
	"tab_diag": {EN: "3 Diag", ZH: "3 诊断"},
}

// T returns the translation for key in the current language.
// Falls back to the English translation if the key or language is missing.
func T(key string) string {
	return TL(key, Current)
}

// TL returns the translation for key in the specified language.
// Falls back to English if the key or language is not found.
func TL(key string, lang Lang) string {
	if m, ok := translations[key]; ok {
		if s, ok2 := m[lang]; ok2 {
			return s
		}
		return m[EN]
	}
	return key
}

// SetLang sets the current language for translation lookups.
func SetLang(l Lang) {
	Current = l
}

// LangLabel returns the display label for the current language.
func LangLabel() string {
	return TL("lang_label", Current)
}
