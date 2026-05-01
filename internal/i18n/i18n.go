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
	"help_list": {
		EN: "↑↓/kj navigate | Enter detail | ←→ sort | s reverse | r refresh | Tab next | 1/2/3 views | q quit",
		ZH: "↑↓/kj 导航 | Enter 详情 | ←→ 排序 | s 反转 | r 刷新 | Tab 切换 | 1/2/3 视图 | q 退出",
	},
	"help_detail": {
		EN: "↑↓/kj/PgUp/PgDn scroll | Esc back | Tab next | q quit",
		ZH: "↑↓/kj/PgUp/PgDn 滚动 | Esc 返回 | Tab 切换 | q 退出",
	},
	"help_compare": {
		EN: "↑↓/kj scroll | Esc back | Tab next | r refresh | q quit",
		ZH: "↑↓/kj 滚动 | Esc 返回 | Tab 切换 | r 刷新 | q 退出",
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
