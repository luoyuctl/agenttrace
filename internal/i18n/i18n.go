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
	"token_cost": {
		EN: "💰 TOKEN COST",
		ZH: "💰 TOKEN 费用",
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
		EN: "Est. cost:    $%11.4f  (model: %s)",
		ZH: "预估费用:     $%11.4f  (模型: %s)",
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
		EN: "🔍 AGENTTRACE v%s",
		ZH: "🔍 AGENTTRACE v%s",
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
		EN: "3 Compare",
		ZH: "3 对比",
	},
	"select_session_hint": {
		EN: " Select a session and press Enter to see details ",
		ZH: " 选择会话并按回车查看详情 ",
	},

	// ── TUI help bars ──
	"help_overview": {
		EN: "Tab: sessions · /: filter · q: quit",
		ZH: "Tab: 会话 · /: 筛选 · q: 退出",
	},
	"help_list": {
		EN: "↑↓: select · Enter: detail · /: filter · Tab: overview · q: quit",
		ZH: "↑↓: 选择 · Enter: 详情 · /: 筛选 · Tab: 概览 · q: 退出",
	},
	"help_detail": {
		EN: "Esc: back · Tab: overview · q: quit",
		ZH: "Esc: 返回 · Tab: 概览 · q: 退出",
	},
	"help_compare": {
		EN: "h: sort health · Tab: overview · q: quit",
		ZH: "h: 健康排序 · Tab: 概览 · q: 退出",
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

	// ── Empty state ──
	"empty_sessions_hint": {
		EN: " No AI agent sessions found.\n\n Try: agenttrace --latest -d ~/.hermes/sessions\n      agenttrace --compare -d ~/.hermes/sessions\n\n Place session JSON/JSONL files in ~/.hermes/sessions/ ",
		ZH: " 未找到 AI 代理会话。\n\n 尝试: agenttrace --latest -d ~/.hermes/sessions\n       agenttrace --compare -d ~/.hermes/sessions\n\n 将会话 JSON/JSONL 文件放在 ~/.hermes/sessions/ ",
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
		EN: "Total Cost",
		ZH: "总费用",
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
