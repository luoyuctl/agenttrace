// Package i18n provides bilingual (English/Chinese) translation support for agenttrace.
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
		EN: "AGENTTRACE v%s — AI Agent Session Performance Report",
		ZH: "AGENTTRACE v%s — AI 代理会话性能报告",
	},
	"compare_title": {
		EN: "AGENTTRACE — Multi-Session Comparison",
		ZH: "AGENTTRACE — 多会话对比",
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
	"agenttrace_title": {
		EN: "💸 AGENTTRACE v%s",
		ZH: "💸 AGENTTRACE v%s",
	},
	"sessions_count": {
		EN: " %d sessions ",
		ZH: " %d 个会话 ",
	},
	"not_available": {
		EN: "N/A",
		ZH: "N/A",
	},
	"token_count": {
		EN: "%s tokens",
		ZH: "%s 个 token",
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
	"no_visible_sessions_hint": {
		EN: " No visible sessions match the current filters ",
		ZH: " 当前筛选没有可见会话 ",
	},
	"no_visible_sessions_title": {
		EN: "No matching sessions",
		ZH: "没有匹配会话",
	},
	"no_visible_sessions_active": {
		EN: "Active filter: %s",
		ZH: "当前筛选: %s",
	},
	"no_visible_sessions_clear": {
		EN: "Esc or :clear to show all sessions",
		ZH: "按 Esc 或输入 :clear 显示全部会话",
	},

	// ── TUI help bars ──
	"help_overview": {
		EN: "0-4: jump · Tab: list · q: quit",
		ZH: "0-4: 跳转 · Tab: 列表 · q: 退出",
	},
	"help_list": {
		EN: "↑↓:sel Enter:detail 0-4:jump h/c/t/e/a/n:sort /:filt f:health s:source d:diff w:diag Tab:next q:quit",
		ZH: "↑↓:选择 Enter:详情 0-4:跳转 h/c/t/e/a/n:排序 /:筛选 f:健康 s:来源 d:对比 w:诊断 Tab:下页 q:退出",
	},
	"help_detail": {
		EN: "Esc: back · d: diff neighbor · w: diag · Tab: next · q: quit",
		ZH: "Esc: 返回 · d: 对比相邻 · w: 诊断 · Tab: 下一个 · q: 退出",
	},
	"help_compare": {
		EN: "h: sort health · 0-4: jump · Tab: overview · q: quit",
		ZH: "h: 健康排序 · 0-4: 跳转 · Tab: 概览 · q: 退出",
	},

	// ── CLI / main ──
	"supported_models": {
		EN: "agenttrace v%s — Supported Models",
		ZH: "agenttrace v%s — 支持模型",
	},
	"model_header": {
		EN: "Model",
		ZH: "模型",
	},
	"model_col": {
		EN: "MODEL",
		ZH: "模型",
	},
	"duration_col": {
		EN: "DURATION",
		ZH: "时长",
	},
	"anomaly_count_col": {
		EN: "ANOM",
		ZH: "异常",
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
		EN: " No AI agent sessions found.\n\n Try: agenttrace --latest -d %s\n      agenttrace --compare -d %s\n\n Place session JSON/JSONL files in %s ",
		ZH: " 未找到 AI 代理会话。\n\n 尝试: agenttrace --latest -d %s\n       agenttrace --compare -d %s\n\n 将会话 JSON/JSONL 文件放在 %s ",
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
	"report_md_title": {
		EN: "agenttrace overview",
		ZH: "agenttrace 概览",
	},
	"report_html_title": {
		EN: "agenttrace overview",
		ZH: "agenttrace 概览",
	},
	"report_html_h1": {
		EN: "AI agent session overview",
		ZH: "AI 代理会话概览",
	},
	"report_html_subtitle": {
		EN: "Static report generated from local coding-agent traces.",
		ZH: "基于本地编码 Agent 轨迹生成的静态报告。",
	},
	"report_metric": {
		EN: "Metric",
		ZH: "指标",
	},
	"report_value": {
		EN: "Value",
		ZH: "值",
	},
	"report_sessions": {
		EN: "Sessions",
		ZH: "会话",
	},
	"report_health_breakdown": {
		EN: "Healthy / Warning / Critical",
		ZH: "健康 / 警告 / 严重",
	},
	"report_avg_health": {
		EN: "Average health",
		ZH: "平均健康分",
	},
	"report_fleet_quality": {
		EN: "Fleet quality score",
		ZH: "整体质量评分",
	},
	"report_total_tokens": {
		EN: "Total tokens",
		ZH: "总 token",
	},
	"report_tool_failures": {
		EN: "Tool failures",
		ZH: "工具失败",
	},
	"report_failure_rate": {
		EN: "%.1f%% failure rate",
		ZH: "%.1f%% 失败率",
	},
	"report_by_agent": {
		EN: "By agent",
		ZH: "按 Agent",
	},
	"report_by_model": {
		EN: "By model",
		ZH: "按模型",
	},
	"report_recent_sessions": {
		EN: "Recent sessions",
		ZH: "最近会话",
	},
	"report_recent_anomalies": {
		EN: "Recent anomalies",
		ZH: "近期异常",
	},
	"report_agent": {
		EN: "Agent",
		ZH: "Agent",
	},
	"report_session": {
		EN: "Session",
		ZH: "会话",
	},
	"report_source": {
		EN: "Source",
		ZH: "来源",
	},
	"report_model": {
		EN: "Model",
		ZH: "模型",
	},
	"report_cost": {
		EN: "Cost",
		ZH: "费用",
	},
	"report_health": {
		EN: "Health",
		ZH: "健康",
	},
	"report_anomalies": {
		EN: "Anomalies",
		ZH: "异常",
	},
	"report_type": {
		EN: "Type",
		ZH: "类型",
	},
	"report_age": {
		EN: "Age",
		ZH: "时间",
	},
	"report_no_anomalies": {
		EN: "No anomalies detected.",
		ZH: "未检测到异常。",
	},
	"report_estimated_cost": {
		EN: "Estimated session cost",
		ZH: "估算会话费用",
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
	"fix_title":     {EN: "💡 Fix Suggestions", ZH: "💡 修复建议"},
	"fix_hanging":   {EN: "Add tool timeout protection — detected %d gaps >60s", ZH: "添加工具超时保护 — 检测到 %d 个间隔 >60s"},
	"fix_tool_fail": {EN: "Check tool schemas — %d/%d calls failed", ZH: "检查工具 Schema — %d/%d 调用失败"},
	"fix_shallow":   {EN: "Increase reasoning depth — avg %.0f chars per block", ZH: "增加推理深度 — 平均每块 %.0f 字符"},
	"fix_redaction": {EN: "Review redaction config — %d blocks hidden", ZH: "检查脱敏配置 — %d 思维块被隐藏"},
	"fix_no_tools":  {EN: "Enable tool calling for this agent", ZH: "为此 agent 启用工具调用"},

	// ── Diff ──
	"diff_title":               {EN: "Session Diff", ZH: "会话对比"},
	"diff_field_health":        {EN: "Health", ZH: "健康"},
	"diff_field_cost":          {EN: "Cost", ZH: "费用"},
	"diff_field_turns":         {EN: "Turns", ZH: "轮次"},
	"diff_field_tools":         {EN: "Tools", ZH: "工具"},
	"diff_field_success":       {EN: "Success", ZH: "成功率"},
	"diff_field_fail":          {EN: "Fail", ZH: "失败"},
	"diff_field_fail_count":    {EN: "Fail count", ZH: "失败数"},
	"diff_field_duration":      {EN: "Duration", ZH: "时长"},
	"diff_field_model":         {EN: "Model", ZH: "模型"},
	"diff_field_anomalies":     {EN: "Anomalies", ZH: "异常"},
	"diff_field_anomaly_count": {EN: "Anomaly count", ZH: "异常数"},
	"diff_field_success_rate":  {EN: "Success rate", ZH: "成功率"},
	"diff_better":              {EN: "better", ZH: "更好"},
	"diff_worse":               {EN: "worse", ZH: "更差"},
	"diff_same":                {EN: "same", ZH: "相同"},

	// ── Cost alert ──
	"cost_alert_title":    {EN: "🚨 Waste Alert", ZH: "🚨 浪费预警"},
	"cost_alert_critical": {EN: "This session burned $%.2f/turn (%.0fx avg $%.2f/turn)", ZH: "本会话单轮烧掉 $%.2f（是平均 $%.2f 的 %.0f 倍）"},
	"cost_alert_warning":  {EN: "Loop waste is %.0f%% of total — consider adding circuit breaker", ZH: "循环浪费占总浪费 %.0f%% — 建议添加熔断机制"},

	// ── Health trend ──
	"trend_title":      {EN: "Health Trend", ZH: "健康趋势"},
	"trend_regressing": {EN: "Declining: %s", ZH: "持续下降: %s"},
	"trend_stable":     {EN: "📊 Stable at %d", ZH: "📊 稳定在 %d"},
	"trend_improving":  {EN: "Improving: %s", ZH: "持续上升: %s"},

	// ── Tool warnings ──
	"tool_warn_title":      {EN: "⚠️ Tool Warnings", ZH: "⚠️ 工具调用警告"},
	"tool_warn_dead_loop":  {EN: "%s called %dx consecutively — possible dead loop", ZH: "%s 连续调用 %d 次 — 疑似死循环"},
	"tool_warn_empty_args": {EN: "%s called with empty arguments", ZH: "%s 调用参数为空"},
	"tool_warn_retry":      {EN: "%s retried %dx after failures", ZH: "%s 失败后重试 %d 次"},
	"tool_warn_redundant":  {EN: "%s called %dx with same args — redundant", ZH: "%s 重复调用 %d 次 — 冗余"},

	// ── Prompt impact ──
	"prompt_impact_title":     {EN: "📝 Prompt Impact", ZH: "📝 Prompt 影响"},
	"prompt_impact_improving": {EN: "Trend: improving — keep current approach", ZH: "趋势: 改善中 — 保持当前策略"},
	"prompt_impact_worsening": {EN: "Trend: worsening — consider rolling back", ZH: "趋势: 恶化 — 考虑回滚"},
	"prompt_impact_mixed":     {EN: "Trend: mixed — need more data", ZH: "趋势: 波动 — 需要更多数据"},

	// ── Tab ──
	"tab_diff": {EN: "4 Diff", ZH: "4 对比"},

	// ── Help ──
	"help_diag": {EN: "Esc: back · Tab: next · q: quit", ZH: "Esc: 返回 · Tab: 下一个 · q: 退出"},
	"help_diff": {EN: "Esc: back · Tab: overview · q: quit", ZH: "Esc: 返回 · Tab: 概览 · q: 退出"},

	// ── v0.2 Community Diagnostics ──
	"diag_loop_fingerprint": {
		EN: "LOOP FINGERPRINT DETECTION",
		ZH: "指纹级死循环检测",
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
		EN: "PER-TOOL LATENCY RANKING",
		ZH: "工具延迟排名",
	},
	"diag_tool_lat_col_name":    {EN: "Tool", ZH: "工具"},
	"diag_tool_lat_col_count":   {EN: "Calls", ZH: "调用"},
	"diag_tool_lat_col_avg":     {EN: "Avg", ZH: "平均"},
	"diag_tool_lat_col_p95":     {EN: "P95", ZH: "P95"},
	"diag_tool_lat_col_max":     {EN: "Max", ZH: "最大"},
	"diag_tool_lat_col_timeout": {EN: "T/O", ZH: "超时"},
	"diag_tool_slow_badge":      {EN: "SLOW", ZH: "慢"},
	"diag_context_util": {
		EN: "CONTEXT WINDOW UTILIZATION",
		ZH: "上下文窗口利用率",
	},
	"diag_ctx_total":      {EN: "Est. Context Window", ZH: "预估上下文窗口"},
	"diag_ctx_tool_defs":  {EN: "Tool Definitions", ZH: "工具定义"},
	"diag_ctx_history":    {EN: "Conversation History", ZH: "对话历史"},
	"diag_ctx_sysprompt":  {EN: "System Prompt", ZH: "系统提示"},
	"diag_ctx_available":  {EN: "Available for Task", ZH: "任务可用空间"},
	"diag_ctx_suggestion": {EN: "Recommendation", ZH: "建议"},
	"diag_large_params": {
		EN: "LARGE PARAMETER CALLS",
		ZH: "大参数调用检测",
	},
	"diag_large_params_none": {
		EN: "No oversized calls detected",
		ZH: "未检测到大参数调用",
	},
	"diag_unused_tools": {
		EN: "RARE TOOL USAGE",
		ZH: "低频工具使用",
	},
	"diag_unused_none": {
		EN: "No rarely used tools detected in this session",
		ZH: "本会话未检测到低频工具",
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

	// ── TUI shell / list ──
	"app_subtitle":          {EN: "AI coding agent observability", ZH: "AI 编码代理可观测性"},
	"app_monitor_subtitle":  {EN: "AI agent session monitor", ZH: "AI 代理会话监控"},
	"app_monitor_short":     {EN: "session monitor", ZH: "会话监控"},
	"loading_sessions":      {EN: "Loading sessions...", ZH: "正在加载会话..."},
	"loading_discovering":   {EN: "Discovering sessions...", ZH: "正在发现会话..."},
	"loading_scanning_hint": {EN: "Scanning known agent directories. Press q to quit.", ZH: "正在扫描已知 Agent 目录。按 q 退出。"},
	"loading_parsing_hint":  {EN: "Parsing and analyzing session files. Press q to quit.", ZH: "正在解析和分析会话文件。按 q 退出。"},
	"loading_from_cache":    {EN: "%d from cache", ZH: "%d 个来自缓存"},
	"cache_status":          {EN: "cache %d/%d", ZH: "缓存 %d/%d"},
	"scroll_label":          {EN: "Scroll", ZH: "滚动"},
	"list_filter":           {EN: "filter", ZH: "筛选"},
	"list_issue":            {EN: "issue", ZH: "问题"},
	"list_no_major_anomaly": {EN: "No major anomaly", ZH: "无主要异常"},
	"list_filter_none":      {EN: "none", ZH: "无"},
	"list_filter_anomalies": {EN: "anomalies", ZH: "异常"},
	"list_filter_text":      {EN: "text", ZH: "文本"},
	"list_filter_model":     {EN: "model", ZH: "模型"},
	"list_filter_health":    {EN: "health", ZH: "健康"},
	"list_filter_source":    {EN: "source", ZH: "来源"},
	"list_filter_cost":      {EN: "cost", ZH: "费用"},
	"list_health_good":      {EN: "good", ZH: "良好"},
	"list_health_warn":      {EN: "warn", ZH: "警告"},
	"list_health_crit":      {EN: "crit", ZH: "严重"},
	"sort_label":            {EN: "sort", ZH: "排序"},
	"sort_asc":              {EN: "asc", ZH: "升序"},
	"sort_desc":             {EN: "desc", ZH: "降序"},
	"sort_field_name":       {EN: "name", ZH: "名称"},
	"sort_field_health":     {EN: "health", ZH: "健康"},
	"sort_field_cost":       {EN: "cost", ZH: "费用"},
	"sort_field_turns":      {EN: "turns", ZH: "轮次"},
	"sort_field_source":     {EN: "source", ZH: "来源"},
	"sort_field_failures":   {EN: "failures", ZH: "失败"},
	"sort_field_anomalies":  {EN: "anomalies", ZH: "异常"},
	"view_overview":         {EN: "Overview", ZH: "概览"},
	"view_list":             {EN: "List", ZH: "列表"},
	"view_detail":           {EN: "Detail", ZH: "详情"},
	"view_diagnostics":      {EN: "Diagnostics", ZH: "诊断"},
	"view_diff":             {EN: "Diff", ZH: "对比"},
	"view_unknown":          {EN: "View", ZH: "视图"},
	"help_loading":          {EN: "Loading sessions... · q: quit", ZH: "正在加载会话... · q: 退出"},
	"help_overview_modern":  {EN: "$: top cost · !: critical · a: anomalies · /: search · 0-4: jump · : command · q: quit", ZH: "$: 高费用 · !: 严重 · a: 异常 · /: 搜索 · 0-4: 跳转 · : 命令 · q: 退出"},
	"help_command":          {EN: "Enter: run · Esc: cancel · examples: :health <80  :cost >0.1  :sort cost desc", ZH: "Enter: 执行 · Esc: 取消 · 示例: :health <80  :cost >0.1  :sort cost desc"},
	"help_filter_clear":     {EN: "Esc: clear filter", ZH: "Esc: 清除筛选"},
	"help_force_reload":     {EN: ": command · ctrl+r: force reload", ZH: ": 命令 · ctrl+r: 强制重载"},
	"help_keymap":           {EN: "Esc/?: close · q: quit", ZH: "Esc/?: 关闭 · q: 退出"},
	"keymap_title":          {EN: "Keyboard Shortcuts", ZH: "快捷键"},
	"keymap_subtitle":       {EN: "Triage, filters, diagnostics, and cache reloads.", ZH: "排查、筛选、诊断和缓存重载。"},
	"keymap_nav":            {EN: "Navigation", ZH: "导航"},
	"keymap_actions":        {EN: "Actions", ZH: "操作"},
	"keymap_filters":        {EN: "Filters & Sort", ZH: "筛选和排序"},
	"keymap_system":         {EN: "System", ZH: "系统"},
	"keymap_jump":           {EN: "jump view", ZH: "跳视图"},
	"keymap_next":           {EN: "next view", ZH: "下一个视图"},
	"keymap_back":           {EN: "back or close", ZH: "返回或关闭"},
	"keymap_select":         {EN: "select row", ZH: "选行"},
	"keymap_enter":          {EN: "open detail", ZH: "开详情"},
	"keymap_diag":           {EN: "diagnostics", ZH: "诊断"},
	"keymap_diff":           {EN: "diff neighbor", ZH: "对比相邻"},
	"keymap_search":         {EN: "search text", ZH: "搜索文本"},
	"keymap_help":           {EN: "keymap", ZH: "键位"},
	"keymap_health_filter":  {EN: "health filter", ZH: "健康筛选"},
	"keymap_source_filter":  {EN: "source filter", ZH: "来源筛选"},
	"keymap_sort_metrics":   {EN: "sort metrics", ZH: "指标排序"},
	"keymap_sort_triage":    {EN: "sort triage", ZH: "排查排序"},
	"keymap_cost_critical":  {EN: "cost / critical", ZH: "费用 / 严重"},
	"keymap_command":        {EN: "command mode", ZH: "命令模式"},
	"keymap_reload":         {EN: "reload cache", ZH: "缓存重载"},
	"keymap_force_reload":   {EN: "rebuild cache", ZH: "重建缓存"},
	"keymap_lang":           {EN: "language", ZH: "语言"},
	"keymap_quit":           {EN: "quit", ZH: "退出"},

	// ── Command mode ──
	"cmd_cleared":                {EN: "cleared filters", ZH: "已清除筛选"},
	"cmd_help":                   {EN: "commands: health <80|good|warn|crit · source codex · model claude · cost >0.1 · anomalies · sort failures desc · top anomalies · critical", ZH: "命令: health <80|good|warn|crit · source codex · model claude · cost >0.1 · anomalies · sort failures desc · top anomalies · critical"},
	"cmd_usage_clear":            {EN: "usage: clear", ZH: "用法: clear"},
	"cmd_usage_help":             {EN: "usage: help", ZH: "用法: help"},
	"cmd_usage_health":           {EN: "usage: health good|warn|crit|<80|>=90", ZH: "用法: health good|warn|crit|<80|>=90"},
	"cmd_usage_source":           {EN: "usage: source codex|claude|hermes", ZH: "用法: source codex|claude|hermes"},
	"cmd_usage_model":            {EN: "usage: model claude|gpt|gemini", ZH: "用法: model claude|gpt|gemini"},
	"cmd_usage_cost":             {EN: "usage: cost >0.1|<0.03|>=1", ZH: "用法: cost >0.1|<0.03|>=1"},
	"cmd_cost_expect":            {EN: "cost filter expects >, >=, <, <=, or =", ZH: "费用筛选需要 >、>=、<、<= 或 ="},
	"cmd_usage_anomalies":        {EN: "usage: anomalies", ZH: "用法: anomalies"},
	"cmd_usage_critical":         {EN: "usage: critical", ZH: "用法: critical"},
	"cmd_usage_top":              {EN: "usage: top cost|health|turns|source|failures|anomalies", ZH: "用法: top cost|health|turns|source|failures|anomalies"},
	"cmd_usage_sort":             {EN: "usage: sort cost|health|turns|name|source|failures|anomalies [asc|desc]", ZH: "用法: sort cost|health|turns|name|source|failures|anomalies [asc|desc]"},
	"cmd_filter_health":          {EN: "health filter %s", ZH: "健康筛选 %s"},
	"cmd_filter_source":          {EN: "source filter %q", ZH: "来源筛选 %q"},
	"cmd_filter_model":           {EN: "model filter %q", ZH: "模型筛选 %q"},
	"cmd_filter_cost":            {EN: "cost filter %s%.4g", ZH: "费用筛选 %s%.4g"},
	"cmd_filter_anomalies":       {EN: "showing sessions with anomalies", ZH: "显示有异常的会话"},
	"cmd_filter_critical":        {EN: "showing critical sessions", ZH: "显示严重会话"},
	"cmd_filter_text":            {EN: "text filter %q", ZH: "文本筛选 %q"},
	"cmd_unknown_sort":           {EN: "unknown sort field: %s", ZH: "未知排序字段: %s"},
	"cmd_unknown_sort_direction": {EN: "unknown sort direction: %s (use asc or desc)", ZH: "未知排序方向: %s (使用 asc 或 desc)"},
	"cmd_sorted":                 {EN: "sorted by %s %s", ZH: "按 %s %s 排序"},

	// ── Insight / diagnostics extras ──
	"insight_no_major_waste":        {EN: "No major waste pattern", ZH: "无主要浪费模式"},
	"insight_default_next":          {EN: "Use diagnostics for deeper inspection", ZH: "使用诊断视图深入检查"},
	"insight_medium":                {EN: "medium", ZH: "中"},
	"insight_high":                  {EN: "high", ZH: "高"},
	"insight_impact_fmt":            {EN: "$%.4f cost, %d turns", ZH: "$%.4f 费用，%d 轮"},
	"insight_evidence_fmt":          {EN: "%d anomalies, %.0f%% tool success", ZH: "%d 个异常，工具成功率 %.0f%%"},
	"insight_loop_cost":             {EN: "Loop cost detected", ZH: "检测到循环成本"},
	"insight_loop_impact":           {EN: "$%.4f loop cost inside $%.4f total", ZH: "循环成本 $%.4f，总成本 $%.4f"},
	"insight_loop_evidence":         {EN: "%d retry events, %d loop groups", ZH: "%d 个重试事件，%d 组循环"},
	"insight_cost_anomaly":          {EN: "Cost anomaly", ZH: "费用异常"},
	"insight_primary_issue":         {EN: "Primary issue: ", ZH: "主要问题: "},
	"insight_impact":                {EN: "Impact: ", ZH: "影响: "},
	"insight_evidence":              {EN: "Evidence: ", ZH: "证据: "},
	"insight_next":                  {EN: "Next: ", ZH: "下一步: "},
	"insight_confidence":            {EN: "Confidence: ", ZH: "置信度: "},
	"diag_for":                      {EN: "Diagnostics for: %s", ZH: "诊断对象: %s"},
	"diag_simple_loop":              {EN: "⚠ Simple loop detected (cost $%.4f) — see Detail view", ZH: "⚠ 检测到简单循环 (成本 $%.4f) — 查看详情视图"},
	"diag_score":                    {EN: "Score %d/100", ZH: "评分 %d/100"},
	"diag_all_clear":                {EN: "All Clear", ZH: "全部正常"},
	"diag_issues_found":             {EN: "Issues Found", ZH: "发现问题"},
	"diag_no_latency":               {EN: "No latency data available", ZH: "暂无延迟数据"},
	"anomaly_type_hanging":          {EN: "hanging", ZH: "挂起"},
	"anomaly_type_latency":          {EN: "latency", ZH: "延迟异常"},
	"anomaly_type_tool_failures":    {EN: "tool failures", ZH: "工具失败"},
	"anomaly_type_shallow_thinking": {EN: "shallow thinking", ZH: "浅层思考"},
	"anomaly_type_redacted":         {EN: "redacted thinking", ZH: "思维脱敏"},
	"anomaly_type_no_tools":         {EN: "no tools", ZH: "未使用工具"},
	"risk_critical":                 {EN: "critical", ZH: "严重"},
	"risk_warning":                  {EN: "warning", ZH: "警告"},
	"risk_high":                     {EN: "high", ZH: "高"},
	"risk_medium":                   {EN: "medium", ZH: "中"},
	"risk_good":                     {EN: "good", ZH: "良好"},
	"risk_normal":                   {EN: "normal", ZH: "正常"},
	"risk_rare":                     {EN: "rare", ZH: "低频"},
	"diff_need_two":                 {EN: "Need at least two visible sessions to compare.", ZH: "至少需要两个可见会话才能对比。"},
	"diff_select_neighbor":          {EN: "Select a session and press d to compare nearby sessions.", ZH: "选择会话后按 d 对比相邻会话。"},
	"diff_comparison":               {EN: "COMPARISON", ZH: "对比"},
	"diff_winner_label":             {EN: "Winner: %s", ZH: "胜出: %s"},
	"diff_tie":                      {EN: "tie", ZH: "平局"},
	"diff_favors":                   {EN: "%s favors %s", ZH: "%s 倾向 %s"},
	"diff_similar":                  {EN: "Sessions are broadly similar across tracked metrics.", ZH: "两个会话在跟踪指标上整体接近。"},

	// ── Dashboard labels ──
	"dash_controls_short":       {EN: "$ top cost · ! critical · a anomalies · / search · : command", ZH: "$ 高费用 · ! 严重 · a 异常 · / 搜索 · : 命令"},
	"dash_top_cost":             {EN: "$ top cost", ZH: "$ 高费用"},
	"dash_critical":             {EN: "! critical", ZH: "! 严重"},
	"dash_anomalies":            {EN: "a anomalies", ZH: "a 异常"},
	"dash_command":              {EN: ": command", ZH: ": 命令"},
	"dash_search":               {EN: "/ search", ZH: "/ 搜索"},
	"overview_action_prefix":    {EN: "Next", ZH: "下一步"},
	"overview_action_empty":     {EN: "run --doctor to check discovery, or --demo to try sample sessions", ZH: "运行 --doctor 检查发现路径，或 --demo 体验示例会话"},
	"overview_action_critical":  {EN: "press ! to inspect %d critical session(s)", ZH: "按 ! 查看 %d 个严重会话"},
	"overview_action_anomalies": {EN: "press a to inspect sessions with anomalies", ZH: "按 a 查看有异常的会话"},
	"overview_action_cost":      {EN: "press $ to inspect top-cost sessions", ZH: "按 $ 查看高费用会话"},
	"overview_action_search":    {EN: "press / to search sessions, or ? for the keymap", ZH: "按 / 搜索会话，或按 ? 查看快捷键"},
	"metric_tokens":             {EN: "TOKENS", ZH: "TOKEN"},
	"metric_total_tokens":       {EN: "TOTAL TOKENS", ZH: "总 TOKEN"},
	"metric_cost":               {EN: "COST", ZH: "费用"},
	"metric_total_cost_usd":     {EN: "TOTAL COST (USD)", ZH: "总费用 (USD)"},
	"metric_sessions":           {EN: "SESSIONS", ZH: "会话"},
	"metric_errors":             {EN: "ERRORS", ZH: "错误"},
	"metric_error_rate":         {EN: "ERROR RATE", ZH: "错误率"},
	"metric_p95":                {EN: "P95", ZH: "P95"},
	"metric_p95_latency":        {EN: "P95 LATENCY", ZH: "P95 延迟"},
	"metric_health":             {EN: "HEALTH", ZH: "健康"},
	"metric_health_score":       {EN: "HEALTH SCORE", ZH: "健康评分"},
	"metric_live":               {EN: "+ live", ZH: "+ 实时"},
	"metric_estimated":          {EN: "estimated", ZH: "估算"},
	"metric_loaded":             {EN: "loaded", ZH: "已加载"},
	"metric_failed":             {EN: "%d failed", ZH: "%d 失败"},
	"metric_tool_gaps":          {EN: "tool gaps", ZH: "工具间隔"},
	"panel_token_usage":         {EN: "TOKEN USAGE", ZH: "TOKEN 使用"},
	"panel_latency_ms":          {EN: "LATENCY (MS)", ZH: "延迟 (MS)"},
	"panel_health_score":        {EN: "HEALTH SCORE", ZH: "健康评分"},
	"panel_anomaly_detection":   {EN: "ANOMALY DETECTION", ZH: "异常检测"},
	"panel_top_agents":          {EN: "TOP AGENTS BY TOKEN USAGE", ZH: "Agent Token 使用排行"},
	"panel_recent_sessions":     {EN: "RECENT SESSIONS", ZH: "最近会话"},
	"panel_view_all":            {EN: "View all", ZH: "查看全部"},
	"panel_no_anomalies":        {EN: "No anomalies detected", ZH: "未检测到异常"},
	"panel_no_agent_data":       {EN: "No agent data", ZH: "暂无 Agent 数据"},
	"panel_no_recent":           {EN: "No recent sessions", ZH: "暂无最近会话"},
	"compact_focus":             {EN: "FOCUS", ZH: "关注"},
	"compact_recent":            {EN: "RECENT", ZH: "最近"},
	"legend_input":              {EN: "● Input", ZH: "● 输入"},
	"legend_output":             {EN: "● Output", ZH: "● 输出"},
	"legend_cache":              {EN: "● Cache", ZH: "● 缓存"},
	"metric_total":              {EN: "%s Total", ZH: "%s 总计"},
	"health_excellent":          {EN: "Excellent", ZH: "优秀"},
	"health_good":               {EN: "Good", ZH: "良好"},
	"health_attention":          {EN: "Needs attention", ZH: "需要关注"},
	"health_critical":           {EN: "Critical", ZH: "严重"},
	"health_reliability":        {EN: "Reliability", ZH: "可靠性"},
	"health_performance":        {EN: "Performance", ZH: "性能"},
	"health_quality":            {EN: "Quality", ZH: "质量"},
	"health_efficiency":         {EN: "Efficiency", ZH: "效率"},

	// ── Waste analysis ──
	"waste_analysis_title":  {EN: "AGENTTRACE v%s - Waste Analysis", ZH: "AGENTTRACE v%s - 浪费分析"},
	"waste_score":           {EN: "Score: %d/100 (%s)", ZH: "评分: %d/100 (%s)"},
	"waste_wasted":          {EN: "Wasted: $%.4f", ZH: "浪费: $%.4f"},
	"waste_cache_header":    {EN: "-- Cache --", ZH: "-- 缓存 --"},
	"waste_cache_detail":    {EN: "%s (%.0f%% hit, %d read / %d input)", ZH: "%s (%.0f%% 命中, %d 读 / %d 输入)"},
	"waste_cache_waste":     {EN: "Cache waste: $%.4f", ZH: "缓存浪费: $%.4f"},
	"waste_bloat_header":    {EN: "-- Tool Bloat --", ZH: "-- 工具膨胀 --"},
	"waste_bloat_detail":    {EN: "%s (%.1f tools/turn)", ZH: "%s (%.1f 工具/轮)"},
	"waste_bloat_redundant": {EN: "*redundant", ZH: "*冗余"},
	"waste_stuck_header":    {EN: "-- Stuck --", ZH: "-- 卡顿 --"},
	"waste_stuck_none":      {EN: "none", ZH: "无"},
	"waste_actions_header":  {EN: "-- Actions --", ZH: "-- 建议措施 --"},
	"waste_level_green":     {EN: "LOW", ZH: "低浪费"},
	"waste_level_yellow":    {EN: "MODERATE", ZH: "中度浪费"},
	"waste_level_orange":    {EN: "HIGH", ZH: "高浪费"},
	"waste_level_red":       {EN: "SEVERE", ZH: "严重浪费"},
	"waste_summary_green":   {EN: "efficient session - no significant waste", ZH: "高效会话 - 无明显浪费"},
	"waste_summary_yellow":  {EN: "minor waste - cache %.0f%% hit, room for optimization", ZH: "轻微浪费 - 缓存命中率 %.0f%%，有优化空间"},
	"waste_summary_orange":  {EN: "wasting $%.2f: loops %.0f%%, tools %.1f/turn", ZH: "浪费 $%.2f: 循环 %.0f%%，工具 %.1f/轮"},
	"waste_summary_red":     {EN: "severe waste $%.2f: loops %.0f%%, %d stuck, no cache", ZH: "严重浪费 $%.2f: 循环 %.0f%%，%d 卡顿，无缓存"},
	"waste_action_top_tool": {EN: "top tool %q called %dx - reduce or batch", ZH: "常用工具 %q 调用 %d 次 - 减少或批量处理"},
	"waste_action_loop":     {EN: "loop waste $%.2f (%.0f%%) - add max retries limit", ZH: "循环浪费 $%.2f (%.0f%%) - 添加最大重试限制"},
	"waste_action_optimal":  {EN: "session running optimally", ZH: "会话运行正常"},

	// ── Cache efficiency ratings ──
	"cache_rating_excellent":     {EN: "excellent", ZH: "优秀"},
	"cache_rating_good":          {EN: "good", ZH: "良好"},
	"cache_rating_poor":          {EN: "poor", ZH: "较差"},
	"cache_rating_none":          {EN: "none", ZH: "未启用"},
	"cache_suggestion_excellent": {EN: "cache utilization excellent - keep current prompt structure", ZH: "缓存利用率优秀 - 保持当前 prompt 结构"},
	"cache_suggestion_good":      {EN: "moderate cache hit - place static system instructions at prompt prefix", ZH: "缓存命中率中等 - 将静态系统指令放在 prompt 前缀"},
	"cache_suggestion_poor":      {EN: "low cache hit rate - enable prompt caching with static prefix content", ZH: "缓存命中率低 - 启用 prompt 缓存并使用静态前缀内容"},
	"cache_suggestion_none":      {EN: "caching not enabled - enable Anthropic prompt caching to save up to 90%% on input cost", ZH: "未启用缓存 - 启用 Anthropic prompt 缓存可节省最多 90%% 输入成本"},

	// ── Tool bloat levels ──
	"bloat_level_severe":      {EN: "severe", ZH: "严重"},
	"bloat_level_high":        {EN: "high", ZH: "高"},
	"bloat_level_medium":      {EN: "medium", ZH: "中等"},
	"bloat_level_low":         {EN: "low", ZH: "低"},
	"bloat_suggestion_severe": {EN: "severe tool bloat: limit max tool calls per turn or split into smaller tasks", ZH: "严重工具膨胀: 限制每轮最大工具调用次数或拆分为小任务"},
	"bloat_suggestion_high":   {EN: "too many tool calls: check if simple tasks use over-complex agent orchestration", ZH: "工具调用过多: 检查简单任务是否使用了过于复杂的代理编排"},
	"bloat_suggestion_medium": {EN: "moderate tool usage: watch for unnecessary tool call patterns", ZH: "工具使用适中: 注意不必要的工具调用模式"},
	"bloat_suggestion_low":    {EN: "tool usage is lean", ZH: "工具使用精简"},

	// ── GenerateFixes (per anomaly type, Title / Description / Action) ──
	"fix_hanging_title":             {EN: "Add Tool Timeout Protection", ZH: "添加工具超时保护"},
	"fix_hanging_desc":              {EN: "Detected %d gaps >60s, max=%.0fs", ZH: "检测到 %d 个间隔 >60s, 最长=%.0fs"},
	"fix_hanging_action":            {EN: "Add timeout to tool calls and limit retry attempts", ZH: "为工具调用添加 timeout 并限制重试次数"},
	"fix_tool_fail_critical_title":  {EN: "Check Tool Schema", ZH: "检查工具 Schema"},
	"fix_tool_fail_critical_desc":   {EN: "Tool failure rate %.0f%% (%d/%d)", ZH: "工具失败率 %.0f%% (%d/%d)"},
	"fix_tool_fail_critical_action": {EN: "Validate tool parameter formats, ensure LLM passes correct argument types", ZH: "验证工具参数格式，确保 LLM 传入正确类型的参数"},
	"fix_tool_fail_warning_title":   {EN: "Improve Tool Descriptions", ZH: "优化工具描述"},
	"fix_tool_fail_warning_desc":    {EN: "Tool failure rate %.0f%% (%d/%d)", ZH: "工具失败率 %.0f%% (%d/%d)"},
	"fix_tool_fail_warning_action":  {EN: "Provide more precise parameter examples and constraints in tool descriptions", ZH: "在 tool description 中提供更精确的参数示例和约束"},
	"fix_shallow_title":             {EN: "Increase Reasoning Depth", ZH: "增加推理深度"},
	"fix_shallow_desc":              {EN: "Average reasoning only %.0f chars", ZH: "平均推理仅 %.0f 字符"},
	"fix_shallow_action":            {EN: "Add 'Think step by step' to system prompt or increase max_tokens", ZH: "在 system prompt 中添加 '请一步步思考' 或增加 max_tokens"},
	"fix_redact_title":              {EN: "Check Redaction Config", ZH: "检查脱敏配置"},
	"fix_redact_desc":               {EN: "Found %d redacted thinking blocks", ZH: "发现 %d 个思维块被脱敏"},
	"fix_redact_action":             {EN: "Reasoning content is redacted, check the redact setting in hermes config", ZH: "推理内容被脱敏，检查 hermes config 中的 redact 设置"},
	"fix_no_tools_title":            {EN: "Enable Tool Calling", ZH: "启用工具调用"},
	"fix_no_tools_desc":             {EN: "%d turns with zero tool calls", ZH: "共 %d 轮对话，零工具调用"},
	"fix_no_tools_action":           {EN: "Currently in chat-only mode, consider configuring tools for the agent", ZH: "当前为纯对话模式，考虑为 agent 配置工具以提升效率"},

	// ── Tool pattern validation details ──
	"tool_warn_dead_loop_detail":  {EN: "Tool '%s' called %dx consecutively — possible dead loop", ZH: "工具 '%s' 连续调用 %d 次，可能存在死循环"},
	"tool_warn_empty_args_detail": {EN: "Tool '%s' had %d call(s) with empty arguments", ZH: "工具 '%s' 有 %d 次调用参数为空"},
	"tool_warn_fail_retry_detail": {EN: "Tool '%s' retried %dx after failures", ZH: "工具 '%s' 失败后连续重试 %d 次"},
	"tool_warn_redundant_detail":  {EN: "Tool '%s' called %dx across %d turns — possibly redundant", ZH: "工具 '%s' 在 %d 个轮次中被调用 %d 次，可能存在冗余调用"},

	// ── Cost alert ──
	"cost_alert_no_history":       {EN: "No historical data for comparison", ZH: "无历史数据用于比较"},
	"cost_alert_no_valid_history": {EN: "No valid historical data", ZH: "无有效历史数据"},
	"cost_alert_info":             {EN: "Cost/turn $%.4f, within normal range (avg $%.4f)", ZH: "单轮成本 $%.4f，在正常范围内 (平均 $%.4f)"},

	// ── Health trend ──
	"trend_no_data":   {EN: "No session data available", ZH: "无可用会话数据"},
	"trend_stable_at": {EN: "Health score stable at %.0f", ZH: "健康分稳定在 %.0f"},

	// ── Prompt impact ──
	"prompt_impact_no_data":            {EN: "Not enough data to determine trend", ZH: "无足够数据判断趋势"},
	"prompt_impact_need_more":          {EN: "Need at least 2 sessions with the same name prefix to analyze trends", ZH: "需要至少 2 个同名 session 才能分析趋势"},
	"prompt_impact_suggestion_improve": {EN: "Health score steadily improving — prompt tuning direction is correct, keep going", ZH: "健康分稳步提升，prompt 优化方向正确，建议继续保持"},
	"prompt_impact_suggestion_worsen":  {EN: "Health score declining — consider rolling back recent prompt changes and re-evaluate", ZH: "健康分持续下降，建议回滚最近的 prompt 变更并重新评估"},
	"prompt_impact_suggestion_mixed":   {EN: "Health score fluctuating — investigate each change individually to isolate positive and negative factors", ZH: "健康分波动较大，建议逐一排查每次变更的影响，找出正负因素"},

	// ── Fingerprint loop detail ──
	"diag_loop_fp_detail": {EN: "tool '%s' returned same result %dx consecutively — no progress", ZH: "工具 '%s' 返回相同结果 %d 次 — 无进展"},

	// ── Large params detail ──
	"diag_large_param_args": {EN: "%s arguments are %d chars — may cause timeout or context bloat", ZH: "%s 参数 %d 字符 — 可能导致超时或上下文膨胀"},
	"diag_large_param_call": {EN: "%s call with %d chars — may cause timeout or hang", ZH: "%s 调用 %d 字符 — 可能导致超时或卡顿"},

	// ── Unused tools detail ──
	"diag_unused_detail": {EN: "tool '%s' called only %dx in this session — review whether it belongs in the default toolset", ZH: "工具 '%s' 仅调用 %d 次 — 审视是否应保留在默认工具集中"},

	// ── Stuck pattern descriptions ──
	"stuck_empty_response":    {EN: "%d consecutive empty assistant responses — agent likely stuck", ZH: "%d 次连续空 assistant 回复 — agent 可能卡住"},
	"stuck_repeated_response": {EN: "assistant repeated same response %dx (drift pattern)", ZH: "assistant 重复相同回复 %d 次 (偏移模式)"},
	"stuck_zombie_calls":      {EN: "%d tool call(s) with no response — may indicate hang/timeout", ZH: "%d 个工具调用无响应 — 可能挂起/超时"},
	"stuck_long_gaps":         {EN: "%d gaps >120s — agent appears stuck", ZH: "%d 个间隔 >120秒 — agent 可能卡住"},

	// ── Context utilization suggestion ──
	"diag_ctx_suggestion_critical": {EN: "context nearly full — reduce MCP tools or compact conversation immediately", ZH: "上下文几乎满 — 立即减少 MCP 工具或压缩对话"},
	"diag_ctx_suggestion_warning":  {EN: "context filling up — consider compacting or trimming rarely used tools", ZH: "上下文趋于饱和 — 考虑压缩或裁剪低频工具"},
	"diag_ctx_suggestion_good":     {EN: "plenty of context headroom", ZH: "上下文空间充足"},

	// ── Diff summary ──
	"diff_summary_session_vs":  {EN: "Session %s vs %s", ZH: "会话 %s vs %s"},
	"diff_summary_health_up":   {EN: "health +%d", ZH: "健康 +%d"},
	"diff_summary_health_down": {EN: "health %d", ZH: "健康 %d"},
	"diff_summary_health_same": {EN: "health unchanged", ZH: "健康不变"},
	"diff_summary_cost_up":     {EN: "cost +$%.4f", ZH: "费用 +$%.4f"},
	"diff_summary_cost_down":   {EN: "cost -$%.4f", ZH: "费用 -$%.4f"},

	// ── Loop cost section (CLI) ──
	"loop_section_title":     {EN: "LOOP COST", ZH: "循环成本"},
	"loop_tool_loop_cost":    {EN: "Tool Loop Cost:    $%9.4f  (%d groups)", ZH: "工具循环成本:    $%9.4f  (%d 组)"},
	"loop_retry_cost":        {EN: "Retry Cost:        $%9.4f  (%d events)", ZH: "重试成本:        $%9.4f  (%d 次)"},
	"loop_format_retry_cost": {EN: "Format Retry Cost: $%9.4f", ZH: "格式重试成本:    $%9.4f"},
	"loop_total_waste":       {EN: "Total Loop Waste:  $%9.4f", ZH: "循环总浪费:      $%9.4f"},
	"loop_retry_events":      {EN: "Retry Events:       %d", ZH: "重试事件:          %d"},

	// ── Duration units ──
	"duration_seconds": {EN: "%.0fs", ZH: "%.0f秒"},
	"duration_minutes": {EN: "%.1fm", ZH: "%.1f分"},
	"duration_hours":   {EN: "%dh %dm", ZH: "%d时 %d分"},

	// ── Pricing ──
	"pricing_litellm_fetched": {EN: "LiteLLM (fetched %s)", ZH: "LiteLLM (获取于 %s)"},
	"pricing_litellm_cached":  {EN: "LiteLLM (cached %s)", ZH: "LiteLLM (缓存于 %s)"},
	"pricing_save_warning":    {EN: "Warning: failed to save pricing cache: %v\n", ZH: "警告: 保存定价缓存失败: %v\n"},

	// ── Impact labels (for prompt impact) ──
	"impact_positive":      {EN: "positive", ZH: "正面"},
	"impact_negative":      {EN: "negative", ZH: "负面"},
	"impact_neutral":       {EN: "neutral", ZH: "中性"},
	"impact_positive_cost": {EN: "positive (higher cost)", ZH: "正面 (费用较高)"},
	"impact_negative_cost": {EN: "negative (lower cost)", ZH: "负面 (费用较低)"},

	// ── Prompt change before/after ──
	"prompt_change_before_after": {EN: "health=%d cost=$%.4f", ZH: "健康=%d 费用=$%.4f"},

	// ── Trend direction ──
	"trend_up":           {EN: "up", ZH: "上升"},
	"trend_down":         {EN: "down", ZH: "下降"},
	"trend_stable_label": {EN: "stable", ZH: "稳定"},

	// ── Waste level emoji labels ──
	"waste_level_label": {EN: "%s %s", ZH: "%s %s"},

	// ── CLI / main.go ──
	"cli_version":                {EN: "agenttrace v%s\n", ZH: "agenttrace v%s\n"},
	"cli_downloading_pricing":    {EN: "Downloading latest model pricing from LiteLLM...\n", ZH: "正在从 LiteLLM 下载最新模型定价...\n"},
	"cli_error":                  {EN: "Error: %v\n", ZH: "错误: %v\n"},
	"cli_loaded_pricing":         {EN: "Loaded %d model pricings.\n", ZH: "已加载 %d 个模型定价。\n"},
	"cli_cache_saved":            {EN: "Cache saved to %s\n", ZH: "缓存已保存至 %s\n"},
	"cli_saved":                  {EN: "Saved: %s\n", ZH: "已保存: %s\n"},
	"cli_no_session_files":       {EN: "No session files found.\n", ZH: "未找到会话文件。\n"},
	"cli_error_loading":          {EN: "Error loading %s: %v\n", ZH: "加载 %s 时出错: %v\n"},
	"cli_retry_events":           {EN: "Retry Events:       %d", ZH: "重试事件:          %d"},
	"doctor_title":               {EN: "AGENTTRACE Doctor", ZH: "AGENTTRACE 诊断"},
	"doctor_mode":                {EN: "Mode", ZH: "模式"},
	"doctor_mode_auto":           {EN: "auto-discovery", ZH: "自动发现"},
	"doctor_mode_custom":         {EN: "custom directory", ZH: "指定目录"},
	"doctor_mode_demo":           {EN: "demo sessions", ZH: "演示会话"},
	"doctor_version":             {EN: "Version", ZH: "版本"},
	"doctor_session_files":       {EN: "Session files", ZH: "会话文件"},
	"doctor_cache":               {EN: "Cache", ZH: "缓存"},
	"doctor_cache_detail":        {EN: "%d entries, %d valid for current scan, %d cached directories", ZH: "%d 条记录，当前扫描命中 %d 条，%d 个目录缓存"},
	"doctor_directories":         {EN: "Directories", ZH: "目录"},
	"doctor_found":               {EN: "found", ZH: "存在"},
	"doctor_missing":             {EN: "missing", ZH: "缺失"},
	"doctor_recommendations":     {EN: "Recommendations", ZH: "建议"},
	"doctor_next_demo":           {EN: "No sessions found. Run `agenttrace --demo` to try the TUI immediately.", ZH: "未找到会话。运行 `agenttrace --demo` 可以立即体验 TUI。"},
	"doctor_next_custom":         {EN: "No sessions found in this directory. Check `-d <dir>` or point it at a session JSON/JSONL directory.", ZH: "指定目录中未找到会话。检查 `-d <dir>` 是否指向 session JSON/JSONL 目录。"},
	"doctor_next_ready":          {EN: "Ready: run `agenttrace` for the TUI or `agenttrace --overview -f json` for automation.", ZH: "已就绪：运行 `agenttrace` 打开 TUI，或运行 `agenttrace --overview -f json` 用于自动化。"},
	"doctor_next_demo_cache":     {EN: "Demo sessions use a temporary directory, so cache reuse is not expected in this mode.", ZH: "演示会话使用临时目录，因此该模式下不会复用缓存。"},
	"doctor_next_cache":          {EN: "Cache is cold for this scan. The next TUI startup should reuse parsed sessions incrementally.", ZH: "当前扫描缓存较冷。下一次启动 TUI 应会增量复用已解析会话。"},
	"gate_failed":                {EN: "Gate failed: %s\n", ZH: "门禁失败: %s\n"},
	"gate_avg_health_failed":     {EN: "avg health %.1f is below %d", ZH: "平均健康分 %.1f 低于 %d"},
	"gate_critical_failed":       {EN: "%d critical session(s) found", ZH: "发现 %d 个严重会话"},
	"gate_tool_fail_rate_failed": {EN: "tool fail rate %.1f%% exceeds %.1f%%", ZH: "工具失败率 %.1f%% 超过 %.1f%%"},
	"waste_title":                {EN: "WASTE ANALYSIS", ZH: "浪费分析"},
	"waste_score_label":          {EN: "Waste Score", ZH: "浪费评分"},
	"waste_wasted_label":         {EN: "Wasted", ZH: "浪费金额"},
	"waste_cache_label":          {EN: "Cache", ZH: "缓存"},
	"waste_bloat_label":          {EN: "Tool Bloat", ZH: "工具膨胀"},
	"waste_stuck_label":          {EN: "Stuck", ZH: "卡死"},
	"waste_actions_label":        {EN: "Actions", ZH: "优化建议"},
	"waste_none":                 {EN: "none", ZH: "无"},
	"tab_waste":                  {EN: "W Waste", ZH: "W 浪费"},
	"waste_col_score":            {EN: "SCORE", ZH: "评分"},
	"waste_col_level":            {EN: "LEVEL", ZH: "等级"},
	"waste_col_action":           {EN: "TOP ACTION", ZH: "首要建议"},
	"help_waste":                 {EN: "w: waste · Tab: switch · q: quit", ZH: "w: 浪费 · Tab: 切换 · q: 退出"},
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
