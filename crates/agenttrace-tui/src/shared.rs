#![allow(dead_code)]

use super::*;

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct DriverItem {
    pub(super) label: String,
    pub(super) sessions: usize,
    pub(super) failures: usize,
    pub(super) tokens: i64,
    pub(super) cost: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct InspectFirstItem {
    pub(super) label: &'static str,
    pub(super) index: usize,
}

pub(super) fn cache_state_label() -> String {
    match agenttrace_core::session_cache_path().metadata() {
        Ok(metadata) if metadata.len() > 0 => "cache warm".to_string(),
        _ => "cache empty".to_string(),
    }
}

pub(super) fn source_counts(sessions: &[Session]) -> Vec<(String, usize)> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for session in sessions {
        *counts.entry(driver_source(session)).or_default() += 1;
    }
    let mut items = counts.into_iter().collect::<Vec<_>>();
    items.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    items
}

pub(super) fn render_loading_status(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(app.t("Loading", "加载中"));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let rows = Layout::vertical([Constraint::Length(2), Constraint::Min(1)]).split(inner);
    let state = &app.load_state;
    let ratio = if state.discovered == 0 {
        0.0
    } else {
        state.processed.min(state.discovered) as f64 / state.discovered as f64
    };
    let label = if state.discovered == 0 {
        app.t("Discovering sessions…", "正在发现会话…").to_string()
    } else if state.processed >= state.discovered {
        app.t(
            "Files processed · loading databases and finishing…",
            "文件已处理 · 正在加载数据库并汇总…",
        )
        .to_string()
    } else {
        format!(
            "{}/{} · {:.0}%",
            state.processed,
            state.discovered,
            ratio * 100.0
        )
    };
    frame.render_widget(
        ratatui::widgets::Gauge::default()
            .ratio(ratio)
            .label(label)
            .gauge_style(Style::default().fg(Color::Cyan)),
        rows[0],
    );
    frame.render_widget(
        Paragraph::new(loading_status_lines(app)).wrap(Wrap { trim: true }),
        rows[1],
    );
}

pub(super) fn loading_status_lines(app: &App) -> Vec<Line<'static>> {
    let state = &app.load_state;
    let processed = state.processed.min(state.discovered);
    let percent = processed
        .saturating_mul(100)
        .checked_div(state.discovered)
        .unwrap_or(0);
    vec![
        Line::from(format!(
            "{} · {}",
            load_phase_label(state.phase, app.language),
            display_source_label(&state.source)
        )),
        Line::from(format!(
            "{} {}/{} · {} {} · {}%",
            app.t("processed", "已处理"),
            format_count(processed as i64),
            format_count(state.discovered as i64),
            format_count(state.cache_hits as i64),
            app.t("cache hits", "缓存命中"),
            percent
        )),
        Line::from(format!(
            "{} · {}",
            state.cache_state,
            if state.showing_cached {
                app.t("showing cached sessions", "正在显示缓存会话")
            } else {
                app.t("waiting for sessions", "正在等待会话")
            }
        )),
    ]
}

pub(super) fn load_summary_line(app: &App) -> String {
    if app.pending_load.is_some() && !app.sessions.is_empty() {
        let frames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        let frame =
            frames[(app.last_auto_refresh.elapsed().as_millis() / 120) as usize % frames.len()];
        return format!(
            "{frame} {} {}/{}",
            app.t("Refreshing", "刷新中"),
            app.load_state.processed,
            app.load_state.discovered
        );
    }
    match app.load_state.phase {
        LoadPhase::Discovering | LoadPhase::Parsing => format!(
            "{} {}/{}",
            load_phase_label(app.load_state.phase, app.language),
            format_count(app.load_state.processed as i64),
            format_count(app.load_state.discovered as i64)
        ),
        LoadPhase::Failed => app.t("load failed", "加载失败").to_string(),
        _ => format!(
            "{} {}",
            app.t("loaded", "已加载"),
            format_count(app.sessions.len() as i64)
        ),
    }
}

pub(super) fn load_phase_label(phase: LoadPhase, language: Language) -> &'static str {
    match phase {
        LoadPhase::Idle => text(language, "Ready", "就绪"),
        LoadPhase::Discovering => text(language, "Finding sessions", "正在查找会话"),
        LoadPhase::Parsing => text(language, "Reading sessions", "正在读取会话"),
        LoadPhase::Ready => text(language, "Ready", "就绪"),
        LoadPhase::Failed => text(language, "Load failed", "加载失败"),
    }
}

pub(super) fn top_driver<T: Borrow<Session>>(
    sessions: &[T],
    label: fn(&Session) -> String,
) -> Option<DriverItem> {
    let mut groups: BTreeMap<String, DriverItem> = BTreeMap::new();
    for session in sessions {
        let session = session.borrow();
        let label = label(session);
        let entry = groups.entry(label.clone()).or_insert_with(|| DriverItem {
            label,
            ..DriverItem::default()
        });
        entry.sessions += 1;
        entry.failures += session.metrics.tool_calls_fail;
        entry.tokens += total_tokens(session);
        entry.cost += session.metrics.cost_estimated;
    }
    groups.into_values().max_by(compare_driver_items)
}

pub(super) fn top_anomaly_driver<T: Borrow<Session>>(sessions: &[T]) -> Option<DriverItem> {
    let mut groups: BTreeMap<String, DriverItem> = BTreeMap::new();
    for session in sessions {
        let session = session.borrow();
        for anomaly in &session.anomalies {
            let entry = groups
                .entry(anomaly.kind.clone())
                .or_insert_with(|| DriverItem {
                    label: anomaly.kind.clone(),
                    ..DriverItem::default()
                });
            entry.sessions += 1;
            entry.failures += session.metrics.tool_calls_fail;
            entry.tokens += total_tokens(session);
            entry.cost += session.metrics.cost_estimated;
        }
    }
    groups.into_values().max_by(compare_driver_items)
}

fn compare_driver_items(left: &DriverItem, right: &DriverItem) -> Ordering {
    left.sessions
        .cmp(&right.sessions)
        .then_with(|| left.failures.cmp(&right.failures))
        .then_with(|| left.cost.total_cmp(&right.cost))
        .then_with(|| right.label.cmp(&left.label))
}

pub(super) fn inspect_first_items_for_app(app: &App) -> Vec<InspectFirstItem> {
    let indices = app.filtered.clone();
    let sessions = indices
        .iter()
        .map(|index| app.sessions[*index].clone())
        .collect::<Vec<_>>();
    inspect_first(&sessions)
        .into_iter()
        .filter_map(|item| {
            indices.get(item.index).map(|index| InspectFirstItem {
                label: item.reason,
                index: *index,
            })
        })
        .collect()
}

pub(super) fn inspect_target_view(label: &str) -> View {
    match label {
        "cost" => View::Detail,
        _ => View::Diagnostics,
    }
}

pub(super) fn driver_source(session: &Session) -> String {
    if session.metrics.source_tool.is_empty() {
        "unknown".to_string()
    } else {
        display_source_label(&session.metrics.source_tool)
    }
}

pub(super) fn display_session_source(session: &Session) -> String {
    driver_source(session)
}

pub(super) fn display_source_label(source: &str) -> String {
    let source = source.trim();
    match source {
        "" | "auto-discovery" => "auto discovery".to_string(),
        "pi" => "Pi sessions".to_string(),
        "oh_my_pi" => "Oh My Pi sessions".to_string(),
        "claude_code" => "Claude Code".to_string(),
        "codex_cli" => "Codex".to_string(),
        "hermes_db" => "Hermes DB".to_string(),
        "opencode_db" => "OpenCode DB".to_string(),
        _ if source.contains('/') => source
            .rsplit('/')
            .find(|part| !part.is_empty())
            .unwrap_or(source)
            .to_string(),
        _ => source.to_string(),
    }
}

pub(super) fn driver_model(session: &Session) -> String {
    if session.metrics.model_used.is_empty() {
        "unknown".to_string()
    } else {
        session.metrics.model_used.clone()
    }
}

pub(super) fn format_compact_cost(cost: f64) -> String {
    format_cost(cost)
}

pub(super) fn total_tokens_all<T: Borrow<Session>>(sessions: &[T]) -> i64 {
    sessions
        .iter()
        .map(|session| total_tokens(session.borrow()))
        .sum()
}

pub(super) fn total_duration<T: Borrow<Session>>(sessions: &[T]) -> f64 {
    sessions
        .iter()
        .map(|session| session.borrow().metrics.duration_sec)
        .sum()
}

pub(super) fn p95_gap<T: Borrow<Session>>(sessions: &[T]) -> f64 {
    let mut gaps = sessions
        .iter()
        .flat_map(|session| session.borrow().metrics.gaps_sec.iter().copied())
        .filter(|value| value.is_finite() && *value > 0.0)
        .collect::<Vec<_>>();
    if gaps.is_empty() {
        return 0.0;
    }
    gaps.sort_by(f64::total_cmp);
    let index = ((gaps.len() as f64) * 0.95) as usize;
    gaps[index.min(gaps.len() - 1)]
}

pub(super) fn health_color(health: i32) -> Color {
    match health {
        80.. => Color::Green,
        50..=79 => Color::Yellow,
        _ => Color::LightRed,
    }
}

pub(super) fn format_count(value: i64) -> String {
    format_tokens(value)
}

pub(super) fn format_duration(seconds: f64) -> String {
    if !seconds.is_finite() || seconds <= 0.0 {
        "0s".to_string()
    } else if seconds < 60.0 {
        format!("{seconds:.0}s")
    } else if seconds < 3600.0 {
        format!("{:.1}m", seconds / 60.0)
    } else if seconds < 86_400.0 {
        format!("{:.1}h", seconds / 3600.0)
    } else {
        format!("{:.1}d", seconds / 86_400.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_helpers_render_their_own_loading_summary() {
        let app = App::new(Vec::new(), "test", None);
        let lines = loading_status_lines(&app);
        assert_eq!(lines.len(), 3);
        assert!(format!("{:?}", lines[0]).contains("Ready"));
        assert_eq!(display_source_label("codex_cli"), "Codex");
    }
}
