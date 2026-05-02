package tui

import (
	"fmt"
	"math"
	"strconv"
	"strings"

	"github.com/luoyuctl/agenttrace/internal/i18n"
)

func (m *Model) runCommand(input string) {
	cmd := strings.TrimSpace(input)
	if cmd == "" {
		return
	}
	fields := strings.Fields(cmd)
	if len(fields) == 0 {
		return
	}

	switch strings.ToLower(fields[0]) {
	case "clear", "reset":
		if len(fields) != 1 {
			m.commandFeedback = i18n.T("cmd_usage_clear")
			return
		}
		m.clearFilters()
		m.view = viewList
		m.commandFeedback = i18n.T("cmd_cleared")
	case "help", "?":
		if len(fields) != 1 {
			m.commandFeedback = i18n.T("cmd_usage_help")
			return
		}
		m.commandFeedback = i18n.T("cmd_help")
	case "health":
		if len(fields) < 2 {
			m.commandFeedback = i18n.T("cmd_usage_health")
			return
		}
		healthFilter, ok := parseHealthCommandFilter(commandValue(fields[1:]))
		if !ok {
			m.commandFeedback = i18n.T("cmd_usage_health")
			return
		}
		m.filterHealth = healthFilter
		m.filterMode = ""
		m.filterValue = ""
		m.view = viewList
		m.rebuildFilteredView()
		m.commandFeedback = fmt.Sprintf(i18n.T("cmd_filter_health"), m.filterHealth)
	case "source":
		if len(fields) < 2 {
			m.commandFeedback = i18n.T("cmd_usage_source")
			return
		}
		m.filterSource = strings.Join(fields[1:], " ")
		m.filterMode = ""
		m.filterValue = ""
		m.view = viewList
		m.rebuildFilteredView()
		m.commandFeedback = fmt.Sprintf(i18n.T("cmd_filter_source"), m.filterSource)
	case "model":
		if len(fields) < 2 {
			m.commandFeedback = i18n.T("cmd_usage_model")
			return
		}
		m.filterModel = strings.Join(fields[1:], " ")
		m.view = viewList
		m.rebuildFilteredView()
		m.commandFeedback = fmt.Sprintf(i18n.T("cmd_filter_model"), m.filterModel)
	case "cost":
		if len(fields) < 2 {
			m.commandFeedback = i18n.T("cmd_usage_cost")
			return
		}
		op, value, ok := parseNumericExpression(commandValue(fields[1:]))
		if !ok {
			m.commandFeedback = i18n.T("cmd_cost_expect")
			return
		}
		m.filterCostOp = op
		m.filterCostValue = value
		m.view = viewList
		m.rebuildFilteredView()
		m.commandFeedback = fmt.Sprintf(i18n.T("cmd_filter_cost"), op, value)
	case "anomalies", "anomaly":
		if len(fields) != 1 {
			m.commandFeedback = i18n.T("cmd_usage_anomalies")
			return
		}
		m.filterAnomaly = true
		m.view = viewList
		m.rebuildFilteredView()
		m.commandFeedback = i18n.T("cmd_filter_anomalies")
	case "critical":
		if len(fields) != 1 {
			m.commandFeedback = i18n.T("cmd_usage_critical")
			return
		}
		m.filterHealth = "crit"
		m.filterMode = ""
		m.filterValue = ""
		m.view = viewList
		m.rebuildFilteredView()
		m.commandFeedback = i18n.T("cmd_filter_critical")
	case "top":
		if len(fields) != 2 {
			m.commandFeedback = i18n.T("cmd_usage_top")
			return
		}
		if !isTopSortField(fields[1]) {
			m.commandFeedback = i18n.T("cmd_usage_top")
			return
		}
		m.applySortCommand(fields[1], true)
	case "sort":
		if len(fields) < 2 || len(fields) > 3 {
			m.commandFeedback = i18n.T("cmd_usage_sort")
			return
		}
		desc := true
		if len(fields) >= 3 {
			switch strings.ToLower(fields[2]) {
			case "asc":
				desc = false
			case "desc":
				desc = true
			default:
				m.commandFeedback = fmt.Sprintf(i18n.T("cmd_unknown_sort_direction"), fields[2])
				return
			}
		}
		m.applySortCommand(fields[1], desc)
	default:
		m.filterText = cmd
		m.view = viewList
		m.rebuildFilteredView()
		m.commandFeedback = fmt.Sprintf(i18n.T("cmd_filter_text"), cmd)
	}
}

func commandValue(fields []string) string {
	if len(fields) == 0 {
		return ""
	}
	if len(fields) == 2 && isNumericOperator(fields[0]) {
		return fields[0] + fields[1]
	}
	return strings.Join(fields, " ")
}

func isNumericOperator(s string) bool {
	switch s {
	case ">", ">=", "<", "<=", "=":
		return true
	default:
		return false
	}
}

func normalizeHealthFilter(expr string) string {
	switch strings.ToLower(strings.TrimSpace(expr)) {
	case "healthy":
		return "good"
	case "warning":
		return "warn"
	case "critical":
		return "crit"
	default:
		return strings.ToLower(strings.TrimSpace(expr))
	}
}

func parseHealthCommandFilter(expr string) (string, bool) {
	healthFilter := normalizeHealthFilter(expr)
	switch healthFilter {
	case "good", "warn", "crit":
		return healthFilter, true
	default:
		_, _, ok := parseNumericExpression(healthFilter)
		return healthFilter, ok
	}
}

func isTopSortField(field string) bool {
	switch strings.ToLower(field) {
	case "cost", "health", "turns", "source":
		return true
	default:
		return false
	}
}

func (m *Model) applySortCommand(field string, desc bool) {
	switch strings.ToLower(field) {
	case "cost", "health", "turns", "name", "source":
		m.sortBy = strings.ToLower(field)
	default:
		m.commandFeedback = fmt.Sprintf(i18n.T("cmd_unknown_sort"), field)
		return
	}
	m.sortDesc = desc
	m.sortAndRefresh()
	m.view = viewList
	dir := i18n.T("sort_desc")
	if !desc {
		dir = i18n.T("sort_asc")
	}
	m.commandFeedback = fmt.Sprintf(i18n.T("cmd_sorted"), sortFieldLabel(m.sortBy), dir)
}

func (m *Model) clearFilters() {
	m.filterText = ""
	m.filterInput = ""
	m.filterActive = false
	m.filterMode = ""
	m.filterValue = ""
	m.filterHealth = ""
	m.filterSource = ""
	m.filterModel = ""
	m.filterCostOp = ""
	m.filterCostValue = 0
	m.filterAnomaly = false
	m.rebuildFilteredView()
}

func parseNumericExpression(expr string) (string, float64, bool) {
	for _, op := range []string{">=", "<=", ">", "<", "="} {
		if strings.HasPrefix(expr, op) {
			value, err := strconv.ParseFloat(strings.TrimSpace(strings.TrimPrefix(expr, op)), 64)
			return op, value, err == nil && isFiniteNumber(value)
		}
	}
	value, err := strconv.ParseFloat(expr, 64)
	return "=", value, err == nil && isFiniteNumber(value)
}

func isFiniteNumber(value float64) bool {
	return !math.IsNaN(value) && !math.IsInf(value, 0)
}

func matchFloatExpression(value float64, op string, target float64) bool {
	switch op {
	case ">":
		return value > target
	case ">=":
		return value >= target
	case "<":
		return value < target
	case "<=":
		return value <= target
	case "=":
		return value == target
	default:
		return true
	}
}

func matchHealthExpression(health int, expr string) bool {
	op, value, ok := parseNumericExpression(expr)
	if !ok {
		return false
	}
	return matchFloatExpression(float64(health), op, value)
}
