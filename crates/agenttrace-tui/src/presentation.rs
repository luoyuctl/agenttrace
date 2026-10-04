use super::*;
use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};

fn panel(title: impl Into<String>) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .title(title.into())
        .title_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
}

pub(super) fn render(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();
    if area.width < 48 || area.height < 14 {
        frame.render_widget(
            Paragraph::new(app.t("tui.terminal_too_small_resize_to_at_least"))
                .block(Block::default().borders(Borders::ALL)),
            area,
        );
        return;
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(3),
            Constraint::Min(4),
            Constraint::Length(3),
        ])
        .split(area);

    render_header(frame, app, chunks[0]);
    render_tabs(frame, app, chunks[1]);
    if app.pending_load.is_some() && app.sessions.is_empty() {
        render_loading_status(frame, app, chunks[2]);
    } else if chunks[2].width >= 140 {
        render_workbench(frame, app, chunks[2]);
    } else {
        render_view(frame, app, chunks[2]);
    }
    render_footer(frame, app, chunks[3]);
}

fn render_workbench(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
        .split(area);
    render_session_pane(frame, app, columns[0]);
    if app.view == View::List {
        render_list_workspace(frame, app, columns[1]);
    } else if app.view == View::Detail && !app.raw_report_expanded {
        render_detail_columns(frame, app, columns[1]);
    } else {
        render_view(frame, app, columns[1]);
    }
}

fn render_view(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    match app.view {
        View::Overview => render_overview(frame, app, area),
        View::List => render_list(frame, app, area),
        View::Detail => render_detail(frame, app, area),
        View::Diagnostics => render_report(
            frame,
            app,
            area,
            report_title(app, app.t("tui.diagnostics")),
            diagnostics_text(app),
        ),
        View::Diff => render_report(frame, app, area, diff_title(app), diff_text(app)),
        View::Governance(panel) => render_workspace(frame, app, area, panel),
        View::Help => render_report(
            frame,
            app,
            area,
            format!(
                "{} - {}",
                app.t("tui.help"),
                context_view_label(app.help_context, app.language)
            ),
            help_text(app.help_context, app.language),
        ),
    }
}

fn render_session_pane(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(4)])
        .split(area);
    let filters = active_filter_summary(app, app.language);
    render_list_status(frame, app, chunks[0], &filters);
    render_session_table(frame, app, chunks[1], &filters, true);
}

fn render_list_workspace(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let loading_height = if matches!(
        app.load_state.phase,
        LoadPhase::Discovering | LoadPhase::Parsing
    ) {
        6
    } else {
        3
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Length(loading_height),
            Constraint::Min(4),
        ])
        .split(area);
    render_selected_summary(frame, app, chunks[0]);
    render_driver_summary(frame, app, chunks[1]);
    render_loading_status(frame, app, chunks[2]);
    render_selected_detail(frame, app, chunks[3]);
}

pub(super) fn render_header(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let visible_count = app.filtered.len();
    let focus = app
        .selected_session()
        .map(|session| short(&session.name, 18))
        .unwrap_or_else(|| app.t("tui.none").to_string());
    let source = if area.width >= 118 {
        format!(
            "{}={}  {}={}  {}={}  {}={}",
            app.t("tui.view"),
            context_view_label(app.view, app.language),
            app.t("tui.focus"),
            focus,
            app.t("tui.source_2"),
            short(&display_source_label(&app.source_label), 18),
            app.t("tui.sessions_3"),
            format_count(visible_count as i64)
        )
    } else if area.width >= 96 {
        format!(
            "{}={}  {}={}  {}={}",
            app.t("tui.view"),
            context_view_label(app.view, app.language),
            app.t("tui.focus"),
            focus,
            app.t("tui.sessions_3"),
            format_count(visible_count as i64)
        )
    } else {
        format!(
            "{}={}  n={}",
            app.t("tui.src"),
            short(&display_source_label(&app.source_label), 16),
            format_count(visible_count as i64)
        )
    };
    let text = vec![
        Line::from(vec![
            Span::styled(
                format!("AGENTTRACE v{}", VERSION),
                Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::raw(source),
            Span::raw(format!("  {}=", app.t("tui.next_2"))),
            Span::styled(
                next_action(app),
                Style::default()
                    .fg(priority_color(app))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw(format!("{} ", app.t("tui.health_4"))),
            Span::styled(
                format!("{:.1}", app.derived.average_health),
                Style::default()
                    .fg(health_color(app.derived.average_health as i32))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                format!("{}={}", app.t("tui.ok_2"), app.overview.healthy),
                Style::default().fg(Color::Green),
            ),
            Span::raw(" "),
            Span::styled(
                format!("{}={}", app.t("tui.warn"), app.overview.warning),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw(" "),
            Span::styled(
                format!("{}={}", app.t("tui.crit"), app.overview.critical),
                Style::default().fg(Color::Red),
            ),
            Span::raw(format!(
                "  {}={}  {}={}  {}",
                app.t("tui.cost_2"),
                format_compact_cost(app.overview.total_cost),
                app.t("tui.tokens"),
                format_tokens(app.derived.total_tokens),
                load_summary_line(app)
            )),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(text).block(Block::default().borders(Borders::ALL)),
        area,
    );
}

pub(super) fn render_tabs(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let selected = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let normal = Style::default().fg(Color::Gray);
    let primary = |label: &'static str, active: bool| {
        Span::styled(
            if active {
                format!("[ {label} ]")
            } else {
                format!("  {label}  ")
            },
            if active { selected } else { normal },
        )
    };
    let sessions = matches!(
        app.view,
        View::List | View::Detail | View::Diagnostics | View::Diff
    );
    let insights = matches!(
        app.view,
        View::Overview | View::Governance(GovernancePanel::Efficiency)
    );
    let actions = matches!(
        app.view,
        View::Governance(GovernancePanel::ActionCenter | GovernancePanel::Delivery)
    );
    let section = match app.view {
        View::List => app.t("tui.browse"),
        View::Detail => app.t("tui.summary"),
        View::Diagnostics => app.t("tui.issues"),
        View::Diff => app.t("tui.compare"),
        View::Overview => app.t("tui.overview"),
        View::Governance(GovernancePanel::Efficiency) => app.t("tui.efficiency"),
        View::Governance(GovernancePanel::ActionCenter) => app.t("tui.recommendations"),
        View::Governance(GovernancePanel::Delivery) => app.t("tui.delivery"),
        View::Help => app.t("tui.help"),
    };
    let line = Line::from(vec![
        primary(app.t("tui.sessions_4"), sessions),
        Span::raw(" "),
        primary(app.t("tui.insights"), insights),
        Span::raw(" "),
        primary(app.t("tui.actions"), actions),
        Span::styled(
            format!("   {}: {section}", app.t("tui.section")),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            app.t("tui.tab_switch_section"),
            Style::default().fg(Color::Gray),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(line).block(Block::default().borders(Borders::ALL)),
        area,
    );
}

pub(super) fn render_overview(frame: &mut Frame<'_>, app: &App, area: Rect) {
    if area.width < 96 {
        render_overview_compact(frame, app, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(44), Constraint::Percentage(56)])
        .split(area);

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9),
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Min(4),
        ])
        .split(chunks[0]);
    render_scoreboard(frame, app, left_chunks[0]);
    render_health_distribution(frame, app, left_chunks[1]);
    render_loading_status(frame, app, left_chunks[2]);
    render_driver_charts(frame, app, left_chunks[3]);

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(12), Constraint::Min(4)])
        .split(chunks[1]);
    render_inspect_first(frame, app, right_chunks[0]);
    render_recent_sessions(frame, app, right_chunks[1]);
}

pub(super) fn render_overview_compact(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9),
            Constraint::Length(7),
            Constraint::Length(10),
            Constraint::Min(4),
        ])
        .split(area);
    render_scoreboard(frame, app, chunks[0]);
    render_health_distribution(frame, app, chunks[1]);
    render_inspect_first(frame, app, chunks[2]);
    render_recent_sessions(frame, app, chunks[3]);
}

pub(super) fn coverage_pct(count: usize, total: usize) -> usize {
    count.saturating_mul(100).checked_div(total).unwrap_or(0)
}

pub(super) fn render_scoreboard(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let health = &app.derived.health;
    let lines = vec![
        Line::from(vec![
            Span::raw(format!("{} ", app.t("tui.health_4"))),
            Span::styled(
                format!("{:.1}", app.derived.average_health),
                Style::default()
                    .fg(health_color(app.derived.average_health as i32))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                "  {} {}  {} {}  {} {}",
                app.t("tui.sessions_3"),
                format_count(app.overview.total_sessions as i64),
                app.t("tui.critical"),
                format_count(app.overview.critical as i64),
                app.t("tui.warning"),
                format_count(app.overview.warning as i64)
            )),
        ]),
        Line::from(format!(
            "{} {}  {} {}  {} {}  p95 {}",
            app.t("tui.cost_2"),
            format_compact_cost(app.overview.total_cost),
            app.t("tui.tokens"),
            format_tokens(app.derived.total_tokens),
            app.t("tui.elapsed"),
            format_duration(app.derived.total_duration),
            format_duration(app.derived.p95_gap)
        )),
        Line::from(format!("{}: {}", app.t("tui.next_2"), next_action(app))),
        Line::from(format!(
            "{}  {}={}  {}={}  {}={}%",
            top_model_line(app),
            app.t("tui.data_health"),
            localized_level(&health.confidence, app.language),
            app.t("tui.range"),
            range_label(app.range_filter, app.language),
            app.t("tui.detail_coverage"),
            coverage_pct(health.with_diagnostics, health.parsed)
        )),
        Line::from(scope_confidence_line(app)),
        Line::from(project_resolution_line(app)),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(app.t("tui.scoreboard")),
            )
            .wrap(Wrap { trim: true }),
        area,
    );
}

pub(super) fn render_health_distribution(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let total = app.overview.total_sessions;
    let bar_width = area.width.saturating_sub(4).clamp(12, 48) as usize;
    let healthy = bar_share(app.overview.healthy, total, bar_width);
    let warning = bar_share(app.overview.warning, total, bar_width);
    let critical = bar_width.saturating_sub(healthy + warning);
    let pct = |count: usize| count.saturating_mul(100).checked_div(total).unwrap_or(0);
    let lines = vec![
        Line::from(vec![
            Span::styled("█".repeat(healthy), Style::default().fg(Color::Green)),
            Span::styled("█".repeat(warning), Style::default().fg(Color::Yellow)),
            Span::styled("█".repeat(critical), Style::default().fg(Color::Red)),
        ]),
        Line::from(format!(
            "{} {} ({}%)  {} {} ({}%)",
            app.t("tui.healthy_2"),
            format_count(app.overview.healthy as i64),
            pct(app.overview.healthy),
            app.t("tui.warning"),
            format_count(app.overview.warning as i64),
            pct(app.overview.warning)
        )),
        Line::from(format!(
            "{} {} ({}%)  {} {:.1}",
            app.t("tui.critical"),
            format_count(app.overview.critical as i64),
            pct(app.overview.critical),
            app.t("tui.average"),
            app.derived.average_health
        )),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(app.t("tui.health_distribution")),
            )
            .wrap(Wrap { trim: true }),
        area,
    );
}

pub(super) fn render_inspect_first(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let mut lines = vec![Line::from(format!(
        "{} {}  {} {}  {} {}  {} {}",
        app.t("tui.tool_failures_3"),
        format_count(app.derived.tool_failure_sessions as i64),
        app.t("tui.stuck"),
        format_count(app.derived.stuck_sessions as i64),
        app.t("tui.context_risk_2"),
        format_count(app.derived.context_risk_sessions as i64),
        app.t("tui.loops"),
        format_count(app.derived.loop_sessions as i64),
    ))];
    lines.extend(inspect_first_lines(app, area.width));
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(app.t("tui.inspect_first_enter_opens_1_inspect_n")),
            )
            .wrap(Wrap { trim: true }),
        area,
    );
}

pub(super) fn render_recent_sessions(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let name_width = if area.width >= 92 { 28 } else { 20 };
    let mut lines = Vec::new();
    for session in app
        .filtered
        .iter()
        .filter_map(|index| app.sessions.get(*index))
        .take(recent_limit(area.height))
    {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{:>3} ", session.health),
                Style::default().fg(health_color(session.health)),
            ),
            Span::raw(format!(
                "{} {} {} {}",
                pad_display_width(&session.name, name_width),
                pad_display_width(&format_compact_cost(session.metrics.cost_estimated), 8),
                pad_display_width(&display_session_source(session), 14),
                short(&triage_reason(session, app.language), 24),
            )),
        ]));
    }
    if lines.is_empty() {
        lines.push(Line::from(app.t("tui.no_sessions_visible")));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(app.t("tui.recent_sessions")),
            )
            .wrap(Wrap { trim: true }),
        area,
    );
}

pub(super) fn render_list(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    if app.sessions.is_empty() {
        frame.render_widget(
            Paragraph::new(app.t("tui.no_sessions_loaded_yet_wait_for_loading")).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(app.t("tui.sessions_4")),
            ),
            area,
        );
        return;
    }
    let active_filters = active_filter_summary(app, app.language);
    if app.filtered.is_empty() && !active_filters.is_empty() {
        let text = vec![
            Line::from(app.t("tui.no_visible_sessions_match_the_active_filters")),
            Line::from(format!(
                "{}: {}",
                app.t("tui.active_filters"),
                active_filters
            )),
            Line::from(app.t("tui.press_esc_or_run_clear_to_show")),
        ];
        frame.render_widget(
            Paragraph::new(text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(app.t("tui.sessions_0_visible")),
                )
                .wrap(Wrap { trim: true }),
            area,
        );
        return;
    }

    if area.width < 96 || area.height < 24 {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(4)])
            .split(area);
        render_list_status(frame, app, chunks[0], &active_filters);
        render_session_table(frame, app, chunks[1], &active_filters, true);
        return;
    }

    let loading_height = if matches!(
        app.load_state.phase,
        LoadPhase::Discovering | LoadPhase::Parsing
    ) {
        6
    } else {
        3
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(6),
            Constraint::Length(loading_height),
            Constraint::Min(4),
        ])
        .split(area);
    render_list_status(frame, app, chunks[0], &active_filters);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(48), Constraint::Percentage(52)])
        .split(chunks[1]);
    render_driver_summary(frame, app, top[0]);
    render_selected_summary(frame, app, top[1]);

    render_loading_status(frame, app, chunks[2]);
    if area.width >= 180 {
        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(72), Constraint::Percentage(28)])
            .split(chunks[3]);
        render_session_table(frame, app, body[0], &active_filters, false);
        render_selected_detail(frame, app, body[1]);
    } else {
        render_session_table(frame, app, chunks[3], &active_filters, false);
    }
}

pub(super) fn render_session_table(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: Rect,
    active_filters: &str,
    compact: bool,
) {
    let extra = area.width.saturating_sub(130) as usize;
    let name_width = (22 + extra * 2 / 3).min(52);
    let reason_width = (20 + extra / 3).min(40);
    let visible_rows = area.height.saturating_sub(3).max(1) as usize;
    let start = app
        .selected
        .saturating_sub(visible_rows / 2)
        .min(app.filtered.len().saturating_sub(visible_rows));
    let end = (start + visible_rows).min(app.filtered.len());
    let rows = app.filtered[start..end].iter().filter_map(|idx| {
        let session = app.sessions.get(*idx)?;
        let metrics = &session.metrics;
        let success_rate = tool_success_rate(session);
        if compact {
            Some(Row::new(vec![
                Cell::from(short(&session.name, 18)),
                Cell::from(session.health.to_string())
                    .style(Style::default().fg(health_color(session.health))),
                Cell::from(format_compact_cost(metrics.cost_estimated)),
                Cell::from(metrics.tool_calls_fail.to_string()),
                Cell::from(short(&triage_reason(session, app.language), 16)),
            ]))
        } else {
            Some(
                Row::new(vec![
                    Cell::from(short(&session.name, name_width)),
                    Cell::from(health_label(session.health, app.language))
                        .style(Style::default().fg(health_color(session.health))),
                    Cell::from(capability_label(session, app.language)),
                    Cell::from(short(&display_session_source(session), 14)),
                    Cell::from(short(&metrics.model_used, 14)),
                    Cell::from(format_compact_cost(metrics.cost_estimated)),
                    Cell::from(format_tokens(total_tokens(session))),
                    Cell::from(format!("{success_rate:.0}%")),
                    Cell::from(format_count(metrics.tool_calls_fail as i64)),
                    Cell::from(format_count(session.anomalies.len() as i64)),
                    Cell::from(short(&triage_reason(session, app.language), reason_width)),
                ])
                .style(session_row_style(session)),
            )
        }
    });
    let title = session_table_title(app, active_filters);
    let table = if compact {
        Table::new(
            rows,
            [
                Constraint::Length(18),
                Constraint::Length(6),
                Constraint::Length(8),
                Constraint::Length(5),
                Constraint::Min(12),
            ],
        )
        .header(
            Row::new([
                app.t("tui.session"),
                app.t("tui.score"),
                app.t("tui.cost_2"),
                app.t("tui.fail"),
                app.t("tui.reason"),
            ])
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(panel(title))
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ")
    } else {
        Table::new(
            rows,
            [
                Constraint::Length(name_width as u16),
                Constraint::Length(8),
                Constraint::Length(9),
                Constraint::Length(14),
                Constraint::Length(14),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(6),
                Constraint::Length(6),
                Constraint::Length(5),
                Constraint::Min(16),
            ],
        )
        .header(
            Row::new([
                app.t("tui.session"),
                app.t("tui.health_2"),
                app.t("tui.data"),
                app.t("tui.source_2"),
                app.t("tui.model_2"),
                app.t("tui.cost_2"),
                app.t("tui.tokens"),
                "ok%",
                app.t("tui.fail"),
                app.t("tui.anom"),
                app.t("tui.reason"),
            ])
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(panel(title))
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ")
    };
    app.table_state
        .select(Some(app.selected.saturating_sub(start)));
    frame.render_stateful_widget(table, area, &mut app.table_state);
}

pub(super) fn render_selected_detail(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let text = app
        .selected_session()
        .map(|session| detail_summary_text(session, app.language))
        .unwrap_or_else(|| app.t("tui.no_selected_session").to_string());
    frame.render_widget(
        Paragraph::new(text)
            .block(panel(app.t("tui.selected_detail")))
            .wrap(Wrap { trim: true }),
        area,
    );
}

pub(super) fn render_list_status(
    frame: &mut Frame<'_>,
    app: &App,
    area: Rect,
    active_filters: &str,
) {
    let filter = if active_filters.is_empty() {
        app.t("tui.none").to_string()
    } else {
        active_filters.to_string()
    };
    let hint = if active_filters.is_empty() {
        app.t("tui.enter_detail_3_diagnostics_4_diff")
    } else {
        app.t("tui.esc_clear_resets_filters")
    };
    let text = format!(
        "{}/{} {}  {}: {}  {}: {} {}  {}",
        format_count(app.filtered.len() as i64),
        format_count(app.sessions.len() as i64),
        app.t("tui.visible"),
        app.t("tui.filters"),
        filter,
        app.t("tui.sort"),
        sort_key_label(app.sort_key, app.language),
        if app.sort_desc {
            app.t("tui.desc")
        } else {
            app.t("tui.asc")
        },
        hint
    );
    frame.render_widget(
        Paragraph::new(text)
            .block(panel(app.t("tui.list_status")))
            .wrap(Wrap { trim: true }),
        area,
    );
}

pub(super) fn render_loading_status(frame: &mut Frame<'_>, app: &App, area: Rect) {
    frame.render_widget(
        Paragraph::new(loading_status_lines(app))
            .block(panel(app.t("tui.loading_status")))
            .wrap(Wrap { trim: true }),
        area,
    );
}

pub(super) fn render_driver_summary(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let total = app.filtered.len();
    let text = vec![
        Line::from(format!(
            "{}: {} {}",
            app.t("tui.visible_2"),
            format_count(total as i64),
            app.t("tui.sessions_3")
        )),
        Line::from(driver_summary_line(
            app.t("tui.source"),
            app.derived.top_source.clone(),
            total,
            app.language,
        )),
        Line::from(driver_summary_line(
            app.t("tui.model"),
            app.derived.top_model.clone(),
            total,
            app.language,
        )),
        Line::from(driver_summary_line(
            app.t("tui.project"),
            app.derived.top_project.clone(),
            total,
            app.language,
        )),
        Line::from(driver_summary_line(
            app.t("tui.anomaly_2"),
            app.derived.top_anomaly.clone(),
            total,
            app.language,
        )),
    ];
    frame.render_widget(
        Paragraph::new(text)
            .block(panel(app.t("tui.driver_summary")))
            .wrap(Wrap { trim: true }),
        area,
    );
}

pub(super) fn render_driver_charts(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let total = app.filtered.len();
    let bar_width = area.width.saturating_sub(36).clamp(4, 28) as usize;
    let lines = [
        (app.t("tui.source"), app.derived.top_source.clone()),
        (app.t("tui.model"), app.derived.top_model.clone()),
        (app.t("tui.project"), app.derived.top_project.clone()),
        (app.t("tui.anomaly_2"), app.derived.top_anomaly.clone()),
    ]
    .into_iter()
    .map(|(kind, item)| driver_chart_line(kind, item, total, bar_width, app.language))
    .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(app.t("tui.driver_distribution")),
            )
            .wrap(Wrap { trim: true }),
        area,
    );
}

pub(super) fn render_selected_summary(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let text = if let Some(session) = app.selected_session() {
        vec![
            Line::from(format!(
                "{}: {}  {}={}  ok={:.0}%  {}={}  {}={}  {}={}  {}={}",
                app.t("tui.selected"),
                short(&session.name, 24),
                app.t("tui.reason"),
                short(&triage_reason(session, app.language), 22),
                tool_success_rate(session),
                app.t("tui.fail"),
                format_count(session.metrics.tool_calls_fail as i64),
                app.t("tui.anom"),
                format_count(session.anomalies.len() as i64),
                app.t("tui.health_4"),
                session.health,
                app.t("tui.cost_2"),
                format_compact_cost(session.metrics.cost_estimated)
            )),
            Line::from(format!(
                "{}={}  {}={}  {}={}  {}={}  p95 {}={}",
                app.t("tui.source_2"),
                short(&display_session_source(session), 18),
                app.t("tui.model_2"),
                short(&driver_model(session), 24),
                app.t("tui.tokens"),
                format_tokens(total_tokens(session)),
                app.t("tui.elapsed"),
                format_duration(session.metrics.duration_sec),
                app.t("tui.latency"),
                format_duration(session_p95_gap(session))
            )),
            Line::from(format!(
                "{}={}",
                app.t("tui.action"),
                selected_next_action(session, app.language)
            )),
        ]
    } else {
        vec![Line::from(app.t("tui.selected_none"))]
    };
    frame.render_widget(
        Paragraph::new(text)
            .block(panel(app.t("tui.selected_triage")))
            .wrap(Wrap { trim: true }),
        area,
    );
}

pub(super) fn render_workspace(
    frame: &mut Frame<'_>,
    app: &mut App,
    area: Rect,
    panel: GovernancePanel,
) {
    app.ensure_governance(panel);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(4)])
        .split(area);
    let card_areas = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(chunks[0]);
    let cards = workspace_cards(app, panel);
    for (area, (title, body, color)) in card_areas.iter().zip(cards) {
        frame.render_widget(
            Paragraph::new(body)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(title)
                        .border_style(Style::default().fg(color)),
                )
                .wrap(Wrap { trim: true }),
            *area,
        );
    }
    render_report(
        frame,
        app,
        chunks[1],
        governance_title(panel, app.language),
        governance_text(app, panel),
    );
}

fn workspace_cards(app: &App, panel: GovernancePanel) -> Vec<(String, String, Color)> {
    let empty = GovernanceSnapshot::default();
    let governance = app.governance.as_ref().unwrap_or(&empty);
    let label = |value: &'static str| value.to_string();
    match panel {
        GovernancePanel::ActionCenter => {
            let recommendations = governance.recommendations.as_deref().unwrap_or_default();
            let urgent = recommendations
                .iter()
                .filter(|item| matches!(item.priority.as_str(), "P0" | "P1"))
                .count();
            let savings = recommendations
                .iter()
                .map(|item| item.estimated_savings_usd)
                .sum::<f64>();
            let pricing = governance
                .audit
                .as_ref()
                .map(|audit| localized_level(&audit.pricing_coverage.confidence, app.language))
                .unwrap_or_else(|| app.t("tui.loading_2").to_string());
            vec![
                (
                    label(app.t("tui.prioritized_actions")),
                    format!(
                        "{} {}\n{} {}",
                        recommendations.len(),
                        app.t("tui.findings"),
                        urgent,
                        app.t("tui.p0_p1")
                    ),
                    Color::LightRed,
                ),
                (
                    label(app.t("tui.estimated_savings_2")),
                    format!(
                        "{}\n{}",
                        format_compact_cost(savings),
                        app.t("tui.evidence_backed_estimate")
                    ),
                    Color::LightGreen,
                ),
                (
                    label(app.t("tui.pricing_confidence")),
                    format!(
                        "{}\n{}",
                        pricing,
                        app.t("tui.n_5_action_6_efficiency_7_delivery")
                    ),
                    Color::Cyan,
                ),
            ]
        }
        GovernancePanel::Efficiency => {
            let totals = governance.context.as_ref().map(|report| &report.totals);
            let mcp = governance.mcp.as_ref();
            let failures = mcp
                .map(|report| {
                    report
                        .items
                        .iter()
                        .map(|item| item.failed_calls)
                        .sum::<usize>()
                })
                .unwrap_or(0);
            let calls = mcp
                .map(|report| {
                    report
                        .items
                        .iter()
                        .map(|item| item.tool_calls)
                        .sum::<usize>()
                })
                .unwrap_or(0);
            vec![
                (
                    label(app.t("tui.context_pressure")),
                    format!(
                        "{} {}  {} {}",
                        totals.map_or(0, |value| value.context_critical_sessions),
                        app.t("tui.critical"),
                        totals.map_or(0, |value| value.context_warning_sessions),
                        app.t("tui.warning")
                    ),
                    Color::Yellow,
                ),
                (
                    label(app.t("tui.repeated_work")),
                    format!(
                        "{} {}\n{}={:.1}%",
                        totals.map_or(0, |value| value.repeated_file_reads),
                        app.t("tui.repeat_reads_2"),
                        app.t("tui.cache"),
                        totals.map_or(0.0, |value| value.cache_effectiveness_pct)
                    ),
                    Color::LightMagenta,
                ),
                (
                    label(app.t("tui.mcp_tools")),
                    format!(
                        "{} {}  {} {}",
                        calls,
                        app.t("tui.calls_2"),
                        failures,
                        app.t("tui.failed")
                    ),
                    Color::Cyan,
                ),
            ]
        }
        GovernancePanel::Delivery => {
            let summary = governance.delivery.as_ref().map(|report| &report.summary);
            let status = if governance.delivery_pending.is_some() {
                app.t("tui.scanning_git_roots")
            } else if summary.is_some() {
                app.t("tui.evidence_ready")
            } else {
                app.t("tui.preparing")
            };
            vec![
                (
                    label(app.t("tui.delivery_status")),
                    status.to_string(),
                    if summary.is_some() {
                        Color::LightGreen
                    } else {
                        Color::Yellow
                    },
                ),
                (
                    label(app.t("tui.strong_medium")),
                    format!(
                        "{} / {}",
                        summary.map_or(0, |value| value.strong),
                        summary.map_or(0, |value| value.medium)
                    ),
                    Color::Green,
                ),
                (
                    label(app.t("tui.weak_no_evidence")),
                    format!(
                        "{} / {}",
                        summary.map_or(0, |value| value.weak),
                        summary.map_or(0, |value| value.none)
                    ),
                    Color::Yellow,
                ),
            ]
        }
    }
}

pub(super) fn governance_title(panel: GovernancePanel, language: Language) -> String {
    let item = |candidate, num, msg_key| {
        let label = format!("{num} {}", text(language, msg_key));
        if candidate == panel {
            format!("[{label}]")
        } else {
            label
        }
    };
    format!(
        "{} | {}  {}  {}",
        text(language, "tui.workspace_2"),
        item(
            GovernancePanel::ActionCenter,
            "5",
            "tui.governance.action_center"
        ),
        item(
            GovernancePanel::Efficiency,
            "6",
            "tui.governance.efficiency"
        ),
        item(GovernancePanel::Delivery, "7", "tui.governance.delivery")
    )
}

pub(super) fn governance_text(app: &App, panel: GovernancePanel) -> String {
    let Some(governance) = app.governance.as_ref() else {
        return app.t("tui.workspace_data_is_loading").to_string();
    };
    let body = match panel {
        GovernancePanel::ActionCenter => action_center_text(governance, app.language),
        GovernancePanel::Efficiency => efficiency_text(governance, app.language),
        GovernancePanel::Delivery => governance_loading_or(
            governance
                .delivery
                .as_ref()
                .map(|report| delivery_evidence_text(report, app.language)),
            app.language,
            "tui.governance.correlating_git",
        ),
    };
    format!(
        "{}\n{}\n{}\n\n{body}",
        scope_confidence_line(app),
        project_resolution_line(app),
        active_filter_context(app)
    )
}

fn action_center_text(governance: &GovernanceSnapshot, language: Language) -> String {
    let recommendations = governance.recommendations.as_deref().unwrap_or_default();
    let audit = governance.audit.as_ref();
    let mut lines = vec![
        UiText::ActionCenter.get(language).to_string(),
        "=============".to_string(),
        text(language, "tui.ranked_actions_first_cost_evidence_is_shown").to_string(),
        String::new(),
    ];
    if recommendations.is_empty() {
        lines.push(UiText::NoPriorityFindings.get(language).to_string());
    }
    for item in recommendations.iter().take(8) {
        lines.push(format!(
            "[{} {}] {}",
            item.priority,
            localized_level(&item.severity, language),
            recommendation_title(item, language)
        ));
        lines.push(format!(
            "  {} | {}={}",
            recommendation_action(item, language),
            UiText::EstimatedSavings.get(language),
            format_compact_cost(item.estimated_savings_usd)
        ));
    }
    if let Some(audit) = audit {
        lines.push(String::new());
        lines.push(format!(
            "{}: {} | {}={} | {}={:.1}%",
            text(language, "tui.cost_evidence"),
            format_compact_cost(audit.total_estimated_cost),
            UiText::PricingConfidence.get(language),
            localized_level(&audit.pricing_coverage.confidence, language),
            UiText::ExactPriceMatch.get(language),
            audit.pricing_coverage.exact_pricing_pct
        ));
        lines.push(format!(
            "  {}={}  {}={}  {}={}",
            text(language, "tui.stored"),
            format_compact_cost(audit.stored_estimated_cost_usd),
            text(language, "tui.current"),
            format_optional_cost(audit.current_estimated_cost_usd, language),
            text(language, "tui.difference_2"),
            format_optional_cost(
                audit
                    .current_estimated_cost_usd
                    .map(|cost| cost - audit.stored_estimated_cost_usd),
                language
            )
        ));
        for item in audit.by_provider_model.iter().take(3) {
            lines.push(format!(
                "  {} / {}  current={} stored={}  {}",
                item.provider,
                item.model,
                format_optional_cost(item.estimated_cost_usd, language),
                format_compact_cost(item.stored_estimated_cost_usd),
                pricing_status_label(&item.pricing_status, language)
            ));
        }
    }
    lines.push(String::new());
    lines.push(text(language, "tui.next_6_efficiency_for_bottlenecks_7_delivery").to_string());
    lines.join("\n")
}

fn efficiency_text(governance: &GovernanceSnapshot, language: Language) -> String {
    let context = governance.context.as_ref();
    let mcp = governance.mcp.as_ref();
    let mut lines = vec![
        UiText::Efficiency.get(language).to_string(),
        "==========".to_string(),
        text(
            language,
            "tui.context_pressure_repeated_work_cache_behavior_an",
        )
        .to_string(),
        String::new(),
    ];
    if let Some(context) = context {
        let totals = &context.totals;
        lines.push(format!(
            "{}: {}={} {}={} {}={} {}={:.1}% {}={:.2}",
            text(language, "tui.context_2"),
            text(language, "tui.warning"),
            totals.context_warning_sessions,
            text(language, "tui.critical"),
            totals.context_critical_sessions,
            text(language, "tui.repeat_reads"),
            totals.repeated_file_reads,
            text(language, "tui.cache"),
            totals.cache_effectiveness_pct,
            text(language, "tui.read_write"),
            totals.read_to_write_ratio
        ));
        for item in context.projects.iter().take(5) {
            lines.push(format!(
                "  {}  sessions={} context={:.1}% cache={:.1}% repeats={}",
                item.project,
                item.sessions,
                item.avg_context_utilization_pct,
                item.cache_effectiveness_pct,
                item.repeated_file_reads
            ));
        }
    }
    lines.push(String::new());
    lines.push(text(language, "tui.mcp_tools").to_string());
    if let Some(mcp) = mcp {
        if mcp.items.is_empty() {
            lines.push(format!("  {}", UiText::NoObservedMcpCalls.get(language)));
        }
        for item in mcp.items.iter().take(8) {
            lines.push(format!(
                "  {}  calls={} failed={} sessions={} — {}",
                item.server,
                item.tool_calls,
                item.failed_calls,
                item.invoked_sessions,
                mcp_recommendation(item, language)
            ));
        }
    }
    lines.push(String::new());
    lines.push(
        text(
            language,
            "tui.next_5_action_center_for_prioritized_remediation",
        )
        .to_string(),
    );
    lines.join("\n")
}

pub(super) fn recommendation_title(item: &Recommendation, language: Language) -> String {
    if language == Language::En {
        return item.title.clone();
    }
    match item.id.as_str() {
        "retry-loop" => "停止重复重试".to_string(),
        "tool-failures" => "减少失败的工具调用".to_string(),
        "context-pressure" => "换一个更聚焦的新会话".to_string(),
        "slow-tool" => "给慢工具设定时间上限".to_string(),
        _ => item.title.clone(),
    }
}

pub(super) fn recommendation_action(item: &Recommendation, language: Language) -> String {
    if language == Language::En {
        return item.action.clone();
    }
    match item.id.as_str() {
        "retry-loop" => "同一错误连续两次后先停下来检查，再决定是否重试。".to_string(),
        "tool-failures" => "先看失败原因，换一种做法，不要原样重试。".to_string(),
        "context-pressure" => "只带着当前目标、相关文件和报错，重新开一个短会话。".to_string(),
        "slow-tool" => "设置超时，并把能并行的任务一起执行。".to_string(),
        _ => item.action.clone(),
    }
}

fn mcp_recommendation(item: &agenttrace_core::McpGovernanceItem, language: Language) -> String {
    if language == Language::En {
        return item.recommendation.clone();
    }
    if item.failed_calls > 0 {
        "先检查失败调用，再考虑调整服务设置。".to_string()
    } else {
        "已观察到较多调用；日志无法判断服务是否一直处于加载状态。".to_string()
    }
}

fn delivery_methodology(language: Language) -> &'static str {
    text(language, "tui.matches_local_git_commit_times_with_the")
}

fn delivery_evidence_label(value: &str, language: Language) -> String {
    if language == Language::En {
        return value.to_string();
    }
    if let Some(count) = value.strip_suffix(" local Git commit(s) overlap the session time window")
    {
        return format!("会话时间段内发现 {count} 个本地 Git 提交");
    }
    match value {
        "observed external publish command category" => "观察到发布相关操作。".to_string(),
        "observed git write command category" => "观察到 Git 写入操作。".to_string(),
        "observed file write/edit command category" => "观察到文件编辑或写入操作。".to_string(),
        "tool activity observed without code-delivery evidence" => {
            "观察到工具活动，但没有代码交付证据。".to_string()
        }
        "no write, Git, publish, or tool evidence observed" => {
            "没有观察到编辑、Git、发布或工具操作。".to_string()
        }
        "no overlapping local commit found; this does not rule out uncommitted, remote, non-code, or later-delivered work" => {
            "没有匹配的本地提交；可能是未提交、远端交付、非代码工作，或之后才交付。".to_string()
        }
        _ => value.to_string(),
    }
}

fn delivery_confidence(value: &str, language: Language) -> String {
    if language == Language::En {
        return value.to_string();
    }
    if value.starts_with("medium:") {
        "中等：时间匹配只能作为线索，不能证明作者、合入主分支或业务价值。".to_string()
    } else {
        "较低：时间匹配只能作为线索，不能证明作者、合入主分支或业务价值。".to_string()
    }
}

fn governance_loading_or(value: Option<String>, language: Language, key: &'static str) -> String {
    value.unwrap_or_else(|| {
        format!(
            "{}\n\n{}",
            text(language, key),
            text(language, "tui.press_esc_to_return_this_does_not")
        )
    })
}

pub(super) fn scope_confidence_line(app: &App) -> String {
    let health = &app.derived.health;
    format!(
        "{}: {}={} {}={}/{} {}={} {}={} {}={}",
        app.t("tui.scope"),
        app.t("tui.sessions_3"),
        app.overview.total_sessions,
        app.t("tui.parse"),
        health.parsed,
        health.discovered,
        app.t("tui.skipped"),
        health.skipped,
        app.t("tui.cache_hits"),
        health.cache_hits,
        app.t("tui.confidence"),
        localized_level(&health.confidence, app.language)
    )
}

pub(super) fn project_resolution_line(app: &App) -> String {
    let mut resolutions: BTreeMap<String, usize> = BTreeMap::new();
    let mut roots = std::collections::BTreeSet::new();
    for session in app.visible_sessions() {
        let project = resolve_project(session);
        *resolutions.entry(project.resolution).or_default() += 1;
        if !project.root.is_empty() {
            roots.insert(project.root);
        }
    }
    let resolution_summary = if resolutions.is_empty() {
        app.t("tui.none").to_string()
    } else {
        resolutions
            .into_iter()
            .map(|(resolution, count)| format!("{resolution}={count}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let selected = app.selected_session().map(resolve_project);
    let selected = selected.map_or_else(
        || app.t("tui.none").to_string(),
        |project| {
            let root = if project.root.is_empty() {
                app.t("tui.unattributed").to_string()
            } else {
                short_path(&project.root, 56)
            };
            format!("{} [{}] {root}", project.display_name, project.resolution)
        },
    );
    format!(
        "{}: {} {} | {}: {}",
        app.t("tui.projects"),
        roots.len(),
        resolution_summary,
        app.t("tui.selected_resolver"),
        selected
    )
}

fn active_filter_context(app: &App) -> String {
    let filters = active_filter_summary(app, app.language);
    format!(
        "{}: {}",
        app.t("tui.active_filters"),
        if filters.is_empty() {
            app.t("tui.none").to_string()
        } else {
            filters
        }
    )
}

#[allow(dead_code)]
fn audit_text(audit: &CostAudit, language: Language) -> String {
    let coverage = &audit.pricing_coverage;
    let mut lines = vec![
        text(language, "tui.cost_audit").to_string(),
        "----------".to_string(),
        format!(
            "{}: {}  {}: {}  {}: {}",
            text(language, "tui.stored_estimate_2"),
            format_compact_cost(audit.total_estimated_cost),
            text(language, "tui.current_estimate"),
            format_optional_cost(audit.current_estimated_cost_usd, language),
            text(language, "tui.pricing_source"),
            audit.pricing_source
        ),
        format!(
            "{}: {}  {}: {:.1}%  {}={} {}={} {}={}",
            text(language, "tui.pricing_confidence"),
            localized_level(&coverage.confidence, language),
            text(language, "tui.exact"),
            coverage.exact_pricing_pct,
            text(language, "tui.catalog"),
            coverage.priced_sessions,
            text(language, "tui.fallback"),
            coverage.fallback_priced_sessions,
            text(language, "tui.unknown"),
            coverage.unpriced_or_unknown_sessions
        ),
        String::new(),
        format!("{}", text(language, "tui.provider_model_rows")),
    ];
    if audit.by_provider_model.is_empty() {
        lines.push(format!("- {}", text(language, "tui.none")));
    }
    for item in audit.by_provider_model.iter().take(20) {
        lines.push(format!(
            "- {} / {}  sessions={}  current={}  stored={}  tokens={}  {}",
            item.provider,
            item.model,
            item.sessions,
            format_optional_cost(item.estimated_cost_usd, language),
            format_compact_cost(item.stored_estimated_cost_usd),
            format_tokens(item.tokens.total),
            item.pricing_status
        ));
        let rates = item.rates_per_million_usd.as_ref().map_or_else(
            || text(language, "tui.rates_m_unavailable").to_string(),
            |rates| format!("rates/M in=${:.2} out=${:.2}", rates.input, rates.output),
        );
        lines.push(format!(
            "  in={} out={} cache-w={} cache-r={} | {} | {}",
            format_tokens(item.tokens.input),
            format_tokens(item.tokens.output),
            format_tokens(item.tokens.cache_write),
            format_tokens(item.tokens.cache_read),
            rates,
            item.pricing_note
        ));
    }
    lines.join("\n")
}

#[allow(dead_code)]
fn recommendations_text(items: &[Recommendation], language: Language) -> String {
    let mut lines = vec![
        text(language, "tui.prioritized_recommendations").to_string(),
        "--------------------------".to_string(),
    ];
    if items.is_empty() {
        lines.push(format!(
            "- {}",
            text(language, "tui.no_prioritized_findings")
        ));
    }
    for item in items.iter().take(20) {
        lines.push(format!(
            "[{} {}] {} — {}",
            item.priority,
            localized_level(&item.severity, language),
            item.title,
            item.rationale
        ));
        lines.push(format!(
            "  {}={}  {}={}  {}={}",
            text(language, "tui.estimated_savings"),
            format_compact_cost(item.estimated_savings_usd),
            text(language, "tui.tokens"),
            format_tokens(item.estimated_savings_tokens),
            text(language, "tui.confidence"),
            localized_level(&item.confidence, language)
        ));
        lines.push(format!(
            "  {}: {}",
            text(language, "tui.action"),
            item.action
        ));
        lines.push(format!(
            "  {}: {}",
            text(language, "tui.verify"),
            item.validation_command
        ));
        for evidence in &item.evidence {
            lines.push(format!("  {}: {evidence}", text(language, "tui.evidence")));
        }
    }
    lines.join("\n")
}

#[allow(dead_code)]
fn mcp_text(report: &McpGovernance, language: Language) -> String {
    let mut lines = vec![
        text(language, "tui.mcp_governance").to_string(),
        "--------------".to_string(),
        format!("{}: {}", text(language, "tui.method"), report.methodology),
        String::new(),
    ];
    if report.items.is_empty() {
        lines.push(format!(
            "- {}",
            text(language, "tui.no_observed_mcp_invocations")
        ));
    }
    for item in &report.items {
        lines.push(format!(
            "- {}  {}={} {}={} {}={} {}={}",
            item.server,
            text(language, "tui.sessions_3"),
            item.invoked_sessions,
            text(language, "tui.calls_2"),
            item.tool_calls,
            text(language, "tui.failed"),
            item.failed_calls,
            text(language, "tui.loaded"),
            item.loaded_sessions
                .map_or_else(|| "unavailable".to_string(), |value| value.to_string())
        ));
        lines.push(format!(
            "  {}: {}",
            text(language, "tui.recommendation"),
            item.recommendation
        ));
        lines.push(format!(
            "  {}: {}",
            text(language, "tui.confidence"),
            item.confidence
        ));
    }
    lines.join("\n")
}

#[allow(dead_code)]
fn context_trends_text(report: &ContextTrend, language: Language) -> String {
    let totals = &report.totals;
    let mut lines = vec![
        text(language, "tui.cross_session_context_trends").to_string(),
        "----------------------------".to_string(),
        format!("{}: {}", text(language, "tui.method"), report.methodology),
        format!(
            "{}={} {}={} {}={} {}={} {}={:.1}% {}={:.2} {}={}",
            text(language, "tui.sessions_3"),
            totals.sessions,
            text(language, "tui.warnings"),
            totals.context_warning_sessions,
            text(language, "tui.critical"),
            totals.context_critical_sessions,
            text(language, "tui.repeat_reads"),
            totals.repeated_file_reads,
            text(language, "tui.cache_effectiveness"),
            totals.cache_effectiveness_pct,
            text(language, "tui.read_write"),
            totals.read_to_write_ratio,
            text(language, "tui.output_m"),
            format_compact_cost(totals.output_cost_per_million_tokens)
        ),
        String::new(),
        text(language, "tui.by_project").to_string(),
    ];
    if report.projects.is_empty() {
        lines.push(format!("- {}", text(language, "tui.none")));
    }
    for item in report.projects.iter().take(20) {
        lines.push(format!(
            "- {}  sessions={} context={:.1}% cache={:.1}% repeat-reads={} read/write={:.2} output-cost={}",
            item.project,
            item.sessions,
            item.avg_context_utilization_pct,
            item.cache_effectiveness_pct,
            item.repeated_file_reads,
            item.read_to_write_ratio,
            format_compact_cost(item.cost_per_output_token)
        ));
    }
    lines.join("\n")
}

fn delivery_evidence_text(report: &DeliveryEvidence, language: Language) -> String {
    let summary = &report.summary;
    let mut lines = vec![
        text(language, "tui.delivery_evidence").to_string(),
        "-----------------".to_string(),
        format!(
            "{}: {}",
            text(language, "tui.how_to_read_this"),
            delivery_methodology(language)
        ),
        format!(
            "{}={} {}={} {}={} {}={} {}={}",
            text(language, "tui.strong"),
            summary.strong,
            text(language, "tui.medium"),
            summary.medium,
            text(language, "tui.weak"),
            summary.weak,
            text(language, "tui.non_code"),
            summary.non_code,
            text(language, "tui.none"),
            summary.none
        ),
        String::new(),
    ];
    if report.sessions.is_empty() {
        lines.push(format!("- {}", text(language, "tui.none")));
    }
    for item in report.sessions.iter().take(30) {
        lines.push(format!(
            "- [{}] {}  {}={}",
            localized_level(&item.level, language),
            item.session,
            text(language, "tui.project_2"),
            item.project
        ));
        for evidence in &item.evidence {
            lines.push(format!("  {}", delivery_evidence_label(evidence, language)));
        }
        lines.push(format!(
            "  {}: {}",
            text(language, "tui.how_sure"),
            delivery_confidence(&item.confidence, language)
        ));
    }
    lines.join("\n")
}

pub(super) fn render_report(
    frame: &mut Frame<'_>,
    app: &App,
    area: Rect,
    title: impl Into<String>,
    text: String,
) {
    render_scrollable_text(frame, area, title, text, app.scroll);
}

fn render_scrollable_text(
    frame: &mut Frame<'_>,
    area: Rect,
    title: impl Into<String>,
    text: String,
    scroll: u16,
) {
    let text = terminal_safe_report(&text);
    let content_length = text.lines().count().max(1);
    let viewport_length = area.height.saturating_sub(2).max(1) as usize;
    let title = title.into();
    frame.render_widget(
        Paragraph::new(text)
            .block(panel(title))
            .scroll((scroll, 0))
            .wrap(Wrap { trim: false }),
        area,
    );
    if content_length > viewport_length {
        let mut state = ScrollbarState::new(content_length)
            .position(scroll as usize)
            .viewport_content_length(viewport_length);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            area,
            &mut state,
        );
    }
}

pub(super) fn render_detail(frame: &mut Frame<'_>, app: &App, area: Rect) {
    if app.selected_session().is_none() {
        render_report(
            frame,
            app,
            area,
            app.t("tui.detail"),
            app.t("tui.no_selected_session").to_string(),
        );
        return;
    }
    if app.raw_report_expanded || area.width < 110 {
        render_report(
            frame,
            app,
            area,
            report_title(app, app.t("tui.detail")),
            detail_text(app),
        );
        return;
    }

    render_detail_columns(frame, app, area);
}

fn render_detail_columns(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let Some(session) = app.selected_session() else {
        render_report(
            frame,
            app,
            area,
            app.t("tui.detail"),
            app.t("tui.no_selected_session").to_string(),
        );
        return;
    };
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(52), Constraint::Percentage(48)])
        .split(area);
    render_scrollable_text(
        frame,
        columns[0],
        app.t("tui.session_overview"),
        detail_summary_text(session, app.language),
        app.scroll,
    );
    render_scrollable_text(
        frame,
        columns[1],
        app.t("tui.diagnosis"),
        detail_diagnosis_text(session, app.language),
        app.scroll,
    );
}

pub(super) fn render_footer(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let prompt = match app.mode {
        InputMode::Search => format!(
            "/ {}  ({}/{})",
            app.input,
            format_count(app.filtered.len() as i64),
            format_count(app.sessions.len() as i64)
        ),
        InputMode::Normal => {
            let base = context_actions(app, area.width);
            if app.status.is_empty() {
                base
            } else {
                format!("{} | {base}", short(&app.status, status_width(area.width)))
            }
        }
    };
    frame.render_widget(
        Paragraph::new(prompt).block(Block::default().borders(Borders::ALL)),
        area,
    );
}

pub(super) fn context_actions(app: &App, width: u16) -> String {
    if width < 84 {
        return app.t("tui.select_enter_open_esc_back_help_q").to_string();
    }
    match app.view {
        View::List => app.t("tui.select_enter_open_search_f_filter_ctrl"),
        View::Detail | View::Diagnostics | View::Diff => {
            app.t("tui.section_pgup_pgdn_scroll_esc_back_ctrl")
        }
        View::Overview | View::Governance(_) => app.t("tui.section_enter_open_tab_switch_ctrl_k"),
        View::Help => app.t("tui.or_esc_back_q_quit"),
    }
    .to_string()
}

pub(super) fn report_title(app: &App, base: &str) -> String {
    let Some(session) = app.selected_session() else {
        return base.to_string();
    };
    format!(
        "{} - {} {}={}",
        base,
        short(&session.name, 18),
        app.t("tui.reason"),
        short(&triage_reason(session, app.language), 18)
    )
}

pub(super) fn diff_title(app: &App) -> String {
    let active_filters = active_filter_summary(app, app.language);
    if active_filters.is_empty() {
        format!(
            "{} - {} {} - {} {} {}",
            app.t("tui.diff"),
            app.filtered.len(),
            app.t("tui.visible"),
            app.t("tui.sort"),
            sort_key_label(app.sort_key, app.language),
            if app.sort_desc {
                app.t("tui.desc")
            } else {
                app.t("tui.asc")
            }
        )
    } else {
        format!(
            "{} - {} {} - {} {} - {} {} {}",
            app.t("tui.diff"),
            app.filtered.len(),
            app.t("tui.visible"),
            app.t("tui.filter"),
            active_filters,
            app.t("tui.sort"),
            sort_key_label(app.sort_key, app.language),
            if app.sort_desc {
                app.t("tui.desc")
            } else {
                app.t("tui.asc")
            }
        )
    }
}

pub(super) fn detail_text(app: &App) -> String {
    app.selected_session()
        .map(|session| {
            let summary = detail_native_text(session, app.language);
            if app.raw_report_expanded {
                report_with_context(
                    summary,
                    app.t("tui.raw_report"),
                    report_text_with_language(session, app.language),
                )
            } else {
                summary
            }
        })
        .unwrap_or_else(|| app.t("tui.no_selected_session").to_string())
}

pub(super) fn detail_summary_text(session: &Session, language: Language) -> String {
    let metrics = &session.metrics;
    let mut lines = vec![
        format!(
            "{}  {}  {}  {}",
            health_label(session.health, language),
            format_compact_cost(metrics.cost_estimated),
            format_duration(metrics.duration_sec),
            format_tokens(total_tokens(session))
        ),
        format!(
            "{}={}  {}={}  {}={}  {}={}",
            text(language, "tui.source_2"),
            display_session_source(session),
            text(language, "tui.model_2"),
            driver_model(session),
            text(language, "tui.data"),
            capability_label(session, language),
            text(language, "tui.p95_gap"),
            format_duration(session_p95_gap(session))
        ),
        format!(
            "{}={}  {}={:.0}%  {}={}",
            text(language, "tui.failures_2"),
            format_count(metrics.tool_calls_fail as i64),
            text(language, "tui.tool_success"),
            tool_success_rate(session),
            text(language, "tui.anomalies_2"),
            format_count(session.anomalies.len() as i64)
        ),
        String::new(),
        format!(
            "{}: {}",
            text(language, "tui.name"),
            short(&session.name, 52)
        ),
        format!(
            "{}: {}",
            text(language, "tui.workspace"),
            if session.cwd.is_empty() {
                text(language, "tui.unknown").to_string()
            } else {
                short_path(&session.cwd, 58)
            }
        ),
        format!(
            "{}: {}",
            text(language, "tui.session_file"),
            short_path(&session.path, 58)
        ),
    ];
    lines.extend([
        String::new(),
        format!(
            "{}: {}={}  {}={}  {}={}  {}={}",
            text(language, "tui.tokens_2"),
            text(language, "tui.input"),
            format_tokens(metrics.tokens_input),
            text(language, "tui.output"),
            format_tokens(metrics.tokens_output),
            text(language, "tui.cache_write_3"),
            format_tokens(metrics.tokens_cache_w),
            text(language, "tui.cache_read_3"),
            format_tokens(metrics.tokens_cache_r)
        ),
        format!(
            "{}: {}={}  {}={}  {}={}",
            text(language, "tui.turns"),
            text(language, "tui.user"),
            format_count(metrics.user_messages as i64),
            text(language, "tui.assistant"),
            format_count(metrics.assistant_turns as i64),
            text(language, "tui.tool_results"),
            format_count(metrics.tool_results as i64)
        ),
    ]);
    lines.join("\n")
}

pub(super) fn detail_diagnosis_text(session: &Session, language: Language) -> String {
    let mut lines = vec![
        format!(
            "{}: {}",
            text(language, "tui.primary_issue"),
            triage_reason(session, language)
        ),
        format!(
            "{}: {}",
            text(language, "tui.next_action"),
            selected_next_action(session, language)
        ),
        String::new(),
        format!(
            "{}: {}",
            text(language, "tui.evidence_confidence"),
            evidence_confidence(session, language)
        ),
    ];
    lines.extend(anomaly_lines(session, 4, language));
    lines.push(String::new());
    lines.extend(
        signal_lines(session, language)
            .into_iter()
            .filter(|line| !line.contains("unknown_authority")),
    );
    lines.push(String::new());
    lines.extend(step_lines(session, language, 6));
    lines.push(String::new());
    lines.push(text(language, "tui.press_v_to_view_the_raw_report").to_string());
    lines.join("\n")
}

pub(super) fn short_path(path: &str, max: usize) -> String {
    let home = std::env::var("HOME").ok();
    let display = home
        .as_deref()
        .and_then(|home| path.strip_prefix(home))
        .map(|path| format!("~{path}"))
        .unwrap_or_else(|| path.to_string());
    short(&display, max)
}

pub(super) fn diagnostics_text(app: &App) -> String {
    app.selected_session()
        .map(|session| {
            let mut summary = diagnostics_native_text(session, app.language);
            let alert = predict_cost_anomaly(&app.sessions, session);
            if alert.triggered {
                summary.push_str(&format!(
                    "\n{} [{}]: {} (current={:.4}, baseline={:.4}, ratio={:.1}x)",
                    app.t("tui.cost_alert"),
                    localized_level(&alert.level, app.language),
                    alert.message_for(app.language),
                    alert.current,
                    alert.baseline,
                    alert.ratio
                ));
            }
            report_with_context(
                summary,
                app.t("tui.raw_diagnostics"),
                render_waste_report_with_language(session, app.language),
            )
        })
        .unwrap_or_else(|| app.t("tui.no_selected_session").to_string())
}

pub(super) fn report_with_context(summary: String, raw_title: &str, report: String) -> String {
    format!(
        "{}\n\n{}\n{}\n{}",
        summary,
        raw_title,
        "-".repeat(raw_title.len()),
        report
    )
}

pub(super) fn report_context_line(session: &Session, language: Language) -> String {
    format!(
        "{}: {}={} {}={} {}={} {}={} {}={} {}={}",
        text(language, "tui.context_2"),
        text(language, "tui.reason"),
        triage_reason(session, language),
        text(language, "tui.health_4"),
        session.health,
        text(language, "tui.cost_2"),
        format_compact_cost(session.metrics.cost_estimated),
        text(language, "tui.fail"),
        format_count(session.metrics.tool_calls_fail as i64),
        text(language, "tui.anom"),
        format_count(session.anomalies.len() as i64),
        text(language, "tui.source_2"),
        display_session_source(session)
    )
}

pub(super) fn detail_native_text(session: &Session, language: Language) -> String {
    let metrics = &session.metrics;
    let mut lines = vec![
        text(language, "tui.session_summary").to_string(),
        "---------------".to_string(),
        report_context_line(session, language),
        format!("{}: {}", text(language, "tui.name"), session.name),
        format!(
            "{}: {}",
            text(language, "tui.workspace"),
            if session.cwd.is_empty() {
                text(language, "tui.unknown")
            } else {
                &session.cwd
            }
        ),
        format!("{}: {}", text(language, "tui.session_file"), session.path),
        format!(
            "{}: {}={} {}={}",
            text(language, "tui.driver"),
            text(language, "tui.source_2"),
            display_session_source(session),
            text(language, "tui.model_2"),
            driver_model(session)
        ),
        format!(
            "{}: {}={} {}={} {}={} {}={}",
            text(language, "tui.timeline"),
            text(language, "tui.start"),
            empty_as_unknown(&metrics.session_start),
            text(language, "tui.end"),
            empty_as_unknown(&metrics.session_end),
            text(language, "tui.elapsed"),
            format_duration(metrics.duration_sec),
            text(language, "tui.p95_gap"),
            format_duration(session_p95_gap(session))
        ),
        format!(
            "{}: {}={} {}={} {}={} {}={}",
            text(language, "tui.turns"),
            text(language, "tui.events"),
            format_count(metrics.events_total as i64),
            text(language, "tui.user"),
            format_count(metrics.user_messages as i64),
            text(language, "tui.assistant"),
            format_count(metrics.assistant_turns as i64),
            text(language, "tui.tool_results"),
            format_count(metrics.tool_results as i64)
        ),
        format!(
            "{}: {}={} {}={} {}={:.0}%",
            text(language, "tui.tools"),
            text(language, "tui.total"),
            format_count(metrics.tool_calls_total as i64),
            text(language, "tui.failed"),
            format_count(metrics.tool_calls_fail as i64),
            text(language, "tui.success"),
            tool_success_rate(session)
        ),
        format!(
            "{}: {}={} {}={} {}={} {}={} {}={}",
            text(language, "tui.tokens_2"),
            text(language, "tui.input"),
            format_tokens(metrics.tokens_input),
            text(language, "tui.output"),
            format_tokens(metrics.tokens_output),
            text(language, "tui.cache_write_3"),
            format_tokens(metrics.tokens_cache_w),
            text(language, "tui.cache_read_3"),
            format_tokens(metrics.tokens_cache_r),
            text(language, "tui.total"),
            format_tokens(total_tokens(session))
        ),
        format!(
            "{}: {}",
            text(language, "tui.cost"),
            format_compact_cost(metrics.cost_estimated)
        ),
        String::new(),
        text(language, "tui.next_action_2").to_string(),
        "-----------".to_string(),
        format!("- {}", selected_next_action(session, language)),
    ];
    lines.extend(signal_lines(session, language));
    lines.push(String::new());
    lines.extend(anomaly_lines(session, 4, language));
    lines.join("\n")
}

pub(super) fn diagnostics_native_text(session: &Session, language: Language) -> String {
    let metrics = &session.metrics;
    let mut lines = vec![
        text(language, "tui.problem").to_string(),
        "-------".to_string(),
        report_context_line(session, language),
        String::new(),
        text(language, "tui.evidence_2").to_string(),
        "--------".to_string(),
        format!(
            "{}={} {}={}",
            text(language, "tui.health_4"),
            session.health,
            text(language, "tui.reason"),
            triage_reason(session, language)
        ),
        format!(
            "{}={} {}={}",
            text(language, "tui.source_2"),
            display_session_source(session),
            text(language, "tui.model_2"),
            driver_model(session)
        ),
        format!(
            "{}={} {}={} {}={} {}={}",
            text(language, "tui.duration"),
            format_duration(metrics.duration_sec),
            text(language, "tui.p95_gap"),
            format_duration(session_p95_gap(session)),
            text(language, "tui.failures_2"),
            format_count(metrics.tool_calls_fail as i64),
            text(language, "tui.anomalies_2"),
            format_count(session.anomalies.len() as i64)
        ),
        format!(
            "{}={} {}={} {}={} {}={:.0}%",
            text(language, "tui.cost_2"),
            format_compact_cost(metrics.cost_estimated),
            text(language, "tui.tokens"),
            format_tokens(total_tokens(session)),
            text(language, "tui.cache_read_share"),
            token_share(metrics.tokens_cache_r, total_tokens(session)),
            text(language, "tui.tool_success"),
            tool_success_rate(session)
        ),
        String::new(),
        text(language, "tui.next").to_string(),
        "----".to_string(),
    ];
    lines.extend(
        diagnostic_actions(session, language)
            .into_iter()
            .take(4)
            .map(|line| format!("- {line}")),
    );
    lines.push(String::new());
    lines.push(text(language, "tui.raw_signals").to_string());
    lines.push("-----------".to_string());
    lines.extend(signal_lines(session, language).into_iter().take(5));
    lines.extend(anomaly_lines(session, 6, language));
    lines.push(String::new());
    lines.extend(step_lines(session, language, 12));
    let diagnostics = &session.diagnostics;
    if diagnostics.loop_cost.total_loop_cost > 0.0 {
        lines.push(format!(
            "{}: {}={} {}={} {}={} type={} turns={}",
            text(language, "tui.loop_analysis"),
            text(language, "tui.cost_2"),
            format_compact_cost(diagnostics.loop_cost.total_loop_cost),
            text(language, "tui.retries"),
            diagnostics.loop_cost.retry_events,
            text(language, "tui.groups"),
            diagnostics.loop_cost.loop_groups,
            diagnostics.loop_cost.loop_type,
            diagnostics.loop_cost.turns
        ));
    }
    for warning in &session.tool_warnings {
        lines.push(format!(
            "{}: {}",
            text(language, "tui.tool_warning"),
            warning.detail_for(language)
        ));
    }
    for latency in diagnostics
        .tool_latencies
        .iter()
        .filter(|item| item.p95_sec > 30.0 || item.timeouts > 0 || item.unmatched > 0)
        .take(3)
    {
        lines.push(format!(
            "{}: {} min={:.1}s p95={:.1}s {}={:.1}s {}={} {}={}",
            text(language, "tui.tool_latency"),
            latency.tool_name,
            latency.min_sec,
            latency.p95_sec,
            text(language, "tui.max"),
            latency.max_sec,
            text(language, "tui.timeouts"),
            latency.timeouts,
            text(language, "tui.no_result_2"),
            latency.unmatched
        ));
    }
    lines.push(format!(
        "{}: {:.1}% {}={} {}={}",
        text(language, "tui.context_utilization"),
        diagnostics.context_utilization.utilization_pct,
        text(language, "tui.risk"),
        localized_level(&diagnostics.context_utilization.risk_level, language),
        text(language, "tui.available"),
        format_tokens(diagnostics.context_utilization.available_for_task as i64)
    ));
    if !diagnostics.context_utilization.suggestion.is_empty() {
        lines.push(format!(
            "{}: {}",
            text(language, "tui.context_suggestion"),
            diagnostics.context_utilization.suggestion_for(language)
        ));
    }
    for item in diagnostics.large_params.iter().take(3) {
        lines.push(format!(
            "{}: {} {} {} {}={}",
            text(language, "tui.large_parameter"),
            item.tool_name,
            item.size,
            text(language, "tui.bytes"),
            text(language, "tui.risk"),
            localized_level(&item.risk, language)
        ));
        lines.push(format!(
            "  {} {}",
            item.timestamp,
            item.detail_for(language)
        ));
    }
    for item in diagnostics.unused_tools.iter().take(3) {
        lines.push(format!(
            "{}: {} {} {} {}",
            text(language, "tui.rare_tool"),
            item.tool_name,
            text(language, "tui.called"),
            item.call_count,
            text(language, "tui.time_s")
        ));
        lines.push(format!(
            "  [{}] {}",
            localized_level(&item.level, language),
            item.detail_for(language)
        ));
    }
    for item in diagnostics.stuck_patterns.iter().take(3) {
        lines.push(format!(
            "{}: {}",
            text(language, "tui.stuck_pattern"),
            item.description_for(language)
        ));
    }
    for fix in fix_suggestions(session).into_iter().take(3) {
        let (_, description, action) = fix.text_for(language);
        lines.push(format!(
            "{} [{}]: {} — {}",
            text(language, "tui.fix"),
            localized_level(&fix.severity, language),
            fix.category,
            action
        ));
        lines.push(format!("  {description}"));
    }
    lines.join("\n")
}

pub(super) fn step_lines(session: &Session, language: Language, limit: usize) -> Vec<String> {
    let mut lines = vec![text(language, "tui.steps_metadata_only").to_string()];
    if session.diagnostics.steps.is_empty() {
        lines.push(text(language, "tui.unavailable_for_this_source").to_string());
        return lines;
    }
    lines.extend(session.diagnostics.steps.iter().take(limit).map(|step| {
        format!(
            "- {} {}  {}  {}",
            localized_step_kind(&step.kind, language),
            short(&step.name, 28),
            localized_step_status(&step.status, language),
            if step.duration_sec > 0.0 {
                format_duration(step.duration_sec)
            } else {
                "—".to_string()
            }
        )
    }));
    if session.diagnostics.steps.len() > limit {
        lines.push(format!(
            "  +{} {}",
            session.diagnostics.steps.len() - limit,
            text(language, "tui.more")
        ));
    }
    lines
}

pub(super) fn diagnostic_actions(session: &Session, language: Language) -> Vec<String> {
    let mut actions = Vec::new();
    actions.push(selected_next_action(session, language));
    if session.health < 50 {
        actions.push(text(language, "tui.check_failed_tool_calls_and_high_severity").to_string());
    }
    if session.metrics.tool_calls_fail > 0 {
        actions.push(text(language, "tui.filter_by_failed_tools_or_inspect_raw").to_string());
    }
    if !session.anomalies.is_empty() {
        actions.push(
            text(
                language,
                "tui.review_anomaly_details_and_compare_against_nearb",
            )
            .to_string(),
        );
    }
    if session.metrics.cost_estimated >= 1.0 {
        actions.push(text(language, "tui.open_diff_to_compare_cost_drivers_against").to_string());
    }
    if actions.len() == 1 {
        actions.push(text(language, "tui.use_raw_report_when_you_need_the").to_string());
    }
    actions
}

fn pricing_status_label(value: &str, language: Language) -> String {
    if language == Language::En {
        return value.to_string();
    }
    match value {
        "catalog_estimate" => "目录匹配估算",
        "fallback_estimate" => "回退费率估算",
        "aggregate_estimate" => "多模型聚合估算",
        "unpriced_or_unknown" => "模型价格未知",
        _ => value,
    }
    .to_string()
}

pub(super) fn signal_lines(session: &Session, language: Language) -> Vec<String> {
    let metrics = &session.metrics;
    let mut lines = Vec::new();
    if let Some(line) = top_usage_line(text(language, "tui.top_tool"), &metrics.tool_usage, 42) {
        lines.push(line);
    }
    if let Some(line) = top_usage_line(text(language, "tui.top_file"), &metrics.file_usage, 42) {
        lines.push(line);
    }
    if let Some(line) = top_usage_line(text(language, "tui.top_arg"), &metrics.tool_arg_usage, 42) {
        lines.push(line);
    }
    if let Some(line) = top_usage_line(text(language, "tui.authority"), &metrics.tool_authority, 42)
    {
        lines.push(line);
    }
    if !metrics.highest_authority.is_empty() && metrics.highest_authority != "unknown_authority" {
        lines.push(format!(
            "{}: {}",
            text(language, "tui.highest_authority"),
            metrics.highest_authority
        ));
    }
    if metrics.reasoning_blocks > 0 {
        lines.push(format!(
            "{}: {}={} {}={} {}={}",
            text(language, "tui.reasoning"),
            text(language, "tui.blocks"),
            format_count(metrics.reasoning_blocks as i64),
            text(language, "tui.chars"),
            format_count(metrics.reasoning_chars as i64),
            text(language, "tui.redacted"),
            format_count(metrics.reasoning_redact as i64)
        ));
    }
    if lines.is_empty() {
        lines.push(text(language, "tui.signals_no_tool_file_hotspots_recorded").to_string());
    }
    lines
}

pub(super) fn top_usage_line(
    label: &str,
    usage: &BTreeMap<String, usize>,
    max_name: usize,
) -> Option<String> {
    usage
        .iter()
        .max_by(|(left_name, left_count), (right_name, right_count)| {
            left_count
                .cmp(right_count)
                .then_with(|| right_name.cmp(left_name))
        })
        .map(|(name, count)| {
            format!(
                "{label}: {} ({})",
                short(name, max_name),
                format_count(*count as i64)
            )
        })
}

pub(super) fn anomaly_lines(session: &Session, limit: usize, language: Language) -> Vec<String> {
    let mut lines = vec![
        text(language, "tui.anomalies").to_string(),
        "---------".to_string(),
    ];
    if session.anomalies.is_empty() {
        lines.push(format!("- {}", text(language, "tui.none")));
        return lines;
    }
    for anomaly in session.anomalies.iter().take(limit) {
        lines.push(format!(
            "- {} {}: {}",
            localized_level(empty_as_unknown(&anomaly.severity), language),
            localized_anomaly(empty_as_unknown(&anomaly.kind), language),
            empty_as_unknown(&anomaly.detail_for(language))
        ));
    }
    if session.anomalies.len() > limit {
        lines.push(format!(
            "- ... {} {}",
            format_count((session.anomalies.len() - limit) as i64),
            text(language, "tui.more")
        ));
    }
    lines
}

pub(super) fn token_share(part: i64, total: i64) -> String {
    if total <= 0 || part <= 0 {
        return "0%".to_string();
    }
    format!("{:.0}%", (part as f64 / total as f64) * 100.0)
}

pub(super) fn empty_as_unknown(value: &str) -> &str {
    if value.is_empty() {
        "unknown"
    } else {
        value
    }
}

pub(super) fn diff_text(app: &App) -> String {
    let sessions = app.visible_sessions();
    let context = diff_context_line(app, sessions.len());
    if sessions.len() < 2 {
        let filters = active_filter_summary(app, app.language);
        let filter_hint = if filters.is_empty() {
            format!("{}: {}", app.t("tui.active_filters"), app.t("tui.none"))
        } else {
            format!("{}: {filters}", app.t("tui.active_filters"))
        };
        return format!(
            "{context}\n\n{}\n{filter_hint}\n{}",
            app.t("tui.need_at_least_two_visible_sessions_for"),
            app.t("tui.press_esc_or_run_clear_reset_to")
        );
    }
    let (left, right) = diff_pair(sessions.len(), app.selected);
    format!(
        "{context}\n\n{}",
        report_compare_with_language(
            &[sessions[left].clone(), sessions[right].clone()],
            "default",
            app.language,
        )
    )
}

pub(super) fn diff_pair(len: usize, selected: usize) -> (usize, usize) {
    let selected = selected.min(len - 1);
    if selected + 1 < len {
        (selected, selected + 1)
    } else {
        (selected - 1, selected)
    }
}

pub(super) fn diff_context_line(app: &App, visible_count: usize) -> String {
    let filters = active_filter_summary(app, app.language);
    let filter_text = if filters.is_empty() {
        app.t("tui.none").to_string()
    } else {
        filters
    };
    let top_source = app
        .derived
        .top_source
        .as_ref()
        .map(|item| format!("{}:{}", item.label, format_count(item.sessions as i64)))
        .unwrap_or_else(|| app.t("tui.none").to_string());
    format!(
        "{}: {}={} {}={} {}={} {} {}={}",
        app.t("tui.context_2"),
        app.t("tui.visible"),
        format_count(visible_count as i64),
        app.t("tui.filter"),
        filter_text,
        app.t("tui.sort"),
        sort_key_label(app.sort_key, app.language),
        if app.sort_desc {
            app.t("tui.desc")
        } else {
            app.t("tui.asc")
        },
        app.t("tui.top_source"),
        top_source
    )
}

pub(super) fn help_text(view: View, language: Language) -> String {
    let context = match (view, language) {
        (View::Overview, Language::En) => [
            "Current view: Overview",
            "  enter opens the first recommended session",
            "  ! critical sessions, $ costly sessions, f cycles health filters",
            "  S/M/A filter by the top source/model/anomaly driver",
        ],
        (View::Overview, Language::Zh) => [
            "当前视图：概览",
            "  enter 打开首个推荐会话",
            "  ! 筛严重会话，$ 筛高成本会话，f 循环健康度筛选",
            "  S/M/A 按主要来源/模型/异常筛选",
        ],
        (View::List, Language::En) => [
            "Current view: List",
            "  j/k selects a session; enter opens detail; 3 opens diagnostics",
            "  / searches; f/s/$/! filters; h/c/t/e/a/n sorts",
            "  Esc clears filters, then returns to Overview",
        ],
        (View::List, Language::Zh) => [
            "当前视图：列表",
            "  j/k 选择会话；enter 打开详情；3 打开诊断",
            "  / 搜索；f/s/$/! 筛选；h/c/t/e/a/n 排序",
            "  Esc 先清除筛选，再返回概览",
        ],
        (View::Detail, Language::En) => [
            "Current view: Detail",
            "  page up/down scrolls; 3 opens diagnostics; 4 opens diff",
            "  Esc returns to List",
            "",
        ],
        (View::Detail, Language::Zh) => [
            "当前视图：详情",
            "  PageUp/PageDown 滚动；3 打开诊断；4 打开对比",
            "  Esc 返回列表",
            "",
        ],
        (View::Diagnostics, Language::En) => [
            "Current view: Diagnostics",
            "  page up/down scrolls; 2 opens detail; 4 opens diff",
            "  Esc returns to List",
            "",
        ],
        (View::Diagnostics, Language::Zh) => [
            "当前视图：诊断",
            "  PageUp/PageDown 滚动；2 打开详情；4 打开对比",
            "  Esc 返回列表",
            "",
        ],
        (View::Diff, Language::En) => [
            "Current view: Diff",
            "  j/k changes the selected pair; 2 opens detail; 3 opens diagnostics",
            "  Esc returns to List",
            "",
        ],
        (View::Diff, Language::Zh) => [
            "当前视图：对比",
            "  j/k 更换对比会话；2 打开详情；3 打开诊断",
            "  Esc 返回列表",
            "",
        ],
        (View::Governance(_), Language::En) => [
            "Current view: Workspace",
            "  5 Action Center, 6 Efficiency, 7 Delivery",
            "  g cycles workspaces; page up/down scrolls; Esc returns to List",
            "",
        ],
        (View::Governance(_), Language::Zh) => [
            "当前视图：工作台",
            "  5 行动中心，6 效率，7 交付",
            "  g 循环工作台；PageUp/PageDown 滚动；Esc 返回列表",
            "",
        ],
        (View::Help, Language::En) => ["Current view: Help", "  ? or Esc returns", "", ""],
        (View::Help, Language::Zh) => ["当前视图：帮助", "  ? 或 Esc 返回", "", ""],
    };
    let common = match language {
        Language::En => [
            "Triage workflow",
            "  Start in Sessions. Use arrows to select and enter to open.",
            "  Tab switches Sessions, Insights, and Actions; left/right changes section.",
            "  l switches language between English and Chinese.",
            "  ! critical sessions, $ costly sessions, f cycles health filters",
            "  R cycles Today/7d/30d/All ranges",
            "  / searches; Ctrl+K or : opens commands; Esc returns or clears filters.",
            "",
            "Navigation",
            "  Tab switches primary areas; left/right changes the current section.",
            "  j/k or arrows move selection; Ctrl+d/u moves half a page; G jumps to the end",
            "",
            "Filters and sorting",
            "  / live text search (Esc cancels), s selected source, h health sort, c cost sort",
            "  t turns, e failures, n name, a anomalies",
            "",
            "Command mode",
            "  :overview, :list, :detail, :diagnostics, :diff, :inspect [rank], :search <text>, :clear/:reset, :reload, :quit",
            "  :range today|7d|30d|all, :project <name>, :source <name>, :model <name>",
            "  :health good|warn|crit|<80, :cost >0.10",
            "  :anomaly [type], :critical, :top cost|failures|source, :sort <field> [asc|desc]",
            "  :capability detailed|aggregate|limited, :issues failures|stuck|context|loops",
            "",
            "Automation",
            "  agenttrace --overview -f json",
            "  agenttrace --overview -f html -o agenttrace-overview.html",
        ],
        Language::Zh => [
            "分诊流程",
            "  默认从会话开始。用方向键选择，按 Enter 打开。",
            "  Tab 切换会话、洞察和行动；左右键切换当前分区。",
            "  按 l 在英文和中文之间切换。",
            "  ! 筛严重会话，$ 筛高成本会话，f 循环健康度筛选。",
            "  R 在今天/7天/30天/全部之间切换。",
            "  / 搜索；Ctrl+K 或 : 打开命令；Esc 返回或清除筛选。",
            "",
            "导航",
            "  Tab 切换一级入口；左右键切换当前分区。",
            "  j/k 或方向键移动选择；Ctrl+d/u 半页移动；G 跳到末尾。",
            "",
            "筛选和排序",
            "  / 实时文本搜索（Esc 取消），s 选中来源，h 健康度排序，c 成本排序。",
            "  t 轮次，e 失败，n 名称，a 异常。",
            "",
            "命令模式",
            "  :overview, :list, :detail, :diagnostics, :diff, :inspect [rank], :search <text>, :clear/:reset, :reload, :quit",
            "  :range today|7d|30d|all, :project <name>, :source <name>, :model <name>",
            "  :health good|warn|crit|<80, :cost >0.10",
            "  :anomaly [type], :critical, :top cost|failures|source, :sort <field> [asc|desc]",
            "  :capability detailed|aggregate|limited, :issues failures|stuck|context|loops",
            "",
            "自动化",
            "  agenttrace --overview -f json",
            "  agenttrace --overview -f html -o agenttrace-overview.html",
        ],
    };
    context
        .into_iter()
        .chain(common)
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn cache_state_label() -> String {
    match agenttrace_core::session_cache_path().metadata() {
        Ok(metadata) if metadata.len() > 0 => "cache warm".to_string(),
        Ok(_) => "cache empty".to_string(),
        Err(_) => "cache empty".to_string(),
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

pub(super) fn loading_status_lines(app: &App) -> Vec<Line<'static>> {
    let state = &app.load_state;
    let health = &app.derived.health;
    let mode = if state.force {
        app.t("tui.force_reload")
    } else {
        app.t("tui.normal_load")
    };
    let processed = state.processed.min(state.discovered);
    let progress_width = 32;
    let filled = processed
        .saturating_mul(progress_width)
        .checked_div(state.discovered)
        .unwrap_or(0);
    let percent = processed
        .saturating_mul(100)
        .checked_div(state.discovered)
        .unwrap_or(0);
    let source_text = if state.sources.is_empty() {
        format!("{}={}", app.t("tui.sources"), app.t("tui.none"))
    } else {
        format!(
            "{}={}",
            app.t("tui.sources"),
            state
                .sources
                .iter()
                .take(4)
                .map(|(source, count)| format!("{}:{count}", short(source, 18)))
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    vec![
        Line::from(format!(
            "{} - {} {} {}",
            load_phase_label(state.phase, app.language),
            mode,
            app.t("tui.from"),
            short(&display_source_label(&state.source), 36)
        )),
        Line::from(format!(
            "{} {}/{} {}, {} {}, {}",
            app.t("tui.loaded"),
            format_count(processed as i64),
            format_count(state.discovered as i64),
            app.t("tui.files_processed"),
            format_count(state.cache_hits as i64),
            app.t("tui.cache_hits"),
            cache_state_for_language(&state.cache_state, app.language)
        )),
        Line::from(vec![
            Span::raw("["),
            Span::styled("█".repeat(filled), Style::default().fg(Color::Green)),
            Span::styled(
                "░".repeat(progress_width - filled),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw(format!("] {percent}%")),
        ]),
        Line::from(source_text),
        Line::from(format!(
            "{}={}  {}={}  {}={}  {}={}  {}={}",
            app.t("tui.sessions_parsed"),
            format_count(state.parsed as i64),
            app.t("tui.confidence"),
            localized_level(&health.confidence, app.language),
            app.t("tui.skipped"),
            format_count(state.skipped as i64),
            app.t("tui.pricing_fallback"),
            format_count(health.fallback_pricing as i64),
            app.t("tui.latest"),
            if health.latest_session_at.is_empty() {
                app.t("tui.unknown").to_string()
            } else {
                short(&health.latest_session_at, 20)
            }
        )),
    ]
}

pub(super) fn load_summary_line(app: &App) -> String {
    let state = &app.load_state;
    match state.phase {
        LoadPhase::Idle => app.t("tui.idle_2").to_string(),
        LoadPhase::Discovering => format!(
            "{} {} {}",
            app.t("tui.discovering_2"),
            format_count(state.discovered as i64),
            app.t("tui.files")
        ),
        LoadPhase::Parsing => format!(
            "{} {} {}, {} {}",
            app.t("tui.loading_2"),
            format_count(state.discovered as i64),
            app.t("tui.files"),
            format_count(state.cache_hits as i64),
            app.t("tui.cache_hits")
        ),
        LoadPhase::Ready => {
            let source = state
                .sources
                .first()
                .map(|(source, count)| {
                    format!(
                        "{}:{}",
                        display_source_label(source),
                        format_count(*count as i64)
                    )
                })
                .unwrap_or_else(|| app.t("tui.none").to_string());
            format!(
                "{} {} {}, {} {}, {source}",
                app.t("tui.loaded"),
                format_count(state.parsed as i64),
                app.t("tui.sessions"),
                format_count(state.cache_hits as i64),
                app.t("tui.cache_hits")
            )
        }
        LoadPhase::Failed => app.t("tui.load_failed").to_string(),
    }
}

pub(super) fn load_phase_label(phase: LoadPhase, language: Language) -> &'static str {
    match phase {
        LoadPhase::Idle => text(language, "tui.idle"),
        LoadPhase::Discovering => text(language, "tui.discovering"),
        LoadPhase::Parsing => text(language, "tui.loading"),
        LoadPhase::Ready => text(language, "tui.ready"),
        LoadPhase::Failed => text(language, "tui.failed_3"),
    }
}

pub(super) fn top_group(
    groups: &std::collections::BTreeMap<String, agenttrace_core::GroupOverview>,
) -> Option<(&String, &agenttrace_core::GroupOverview)> {
    groups
        .iter()
        .max_by(|(left_name, left), (right_name, right)| {
            left.sessions
                .cmp(&right.sessions)
                .then_with(|| cmp_f64(left.cost, right.cost))
                .then_with(|| right_name.cmp(left_name))
        })
}

pub(super) fn top_model_line(app: &App) -> String {
    if let Some((model, group)) = top_group(&app.overview.by_model) {
        format!(
            "{} {}  {} {}  {} {}",
            app.t("tui.top_model"),
            short(model, 24),
            app.t("tui.sessions_3"),
            format_count(group.sessions as i64),
            app.t("tui.cost_2"),
            format_compact_cost(group.cost)
        )
    } else {
        format!("{} {}", app.t("tui.top_model"), app.t("tui.none"))
    }
}

pub(super) fn health_color(health: i32) -> Color {
    if health >= 80 {
        Color::Gray
    } else if health >= 50 {
        Color::Yellow
    } else {
        Color::LightRed
    }
}

pub(super) fn session_row_style(session: &Session) -> Style {
    if session.health < 50 {
        Style::default().fg(Color::LightRed)
    } else if session.metrics.tool_calls_fail > 0 || !session.anomalies.is_empty() {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    }
}

pub(super) fn priority_color(app: &App) -> Color {
    if app.overview.critical > 0 {
        Color::LightRed
    } else if app.overview.warning > 0 {
        Color::Yellow
    } else {
        Color::LightGreen
    }
}

pub(super) fn health_label(health: i32, language: Language) -> String {
    if health >= 80 {
        format!("{health} {}", text(language, "tui.ok_2"))
    } else if health >= 50 {
        format!("{health} {}", text(language, "tui.warn"))
    } else {
        format!("{health} {}", text(language, "tui.crit"))
    }
}

pub(super) fn session_table_title(app: &App, active_filters: &str) -> String {
    if active_filters.is_empty() {
        format!(
            "{} - {} {} - {} {} {}",
            app.t("tui.sessions_4"),
            app.filtered.len(),
            app.t("tui.visible"),
            app.t("tui.sort"),
            sort_key_label(app.sort_key, app.language),
            if app.sort_desc {
                app.t("tui.desc")
            } else {
                app.t("tui.asc")
            }
        )
    } else {
        format!(
            "{} - {} {} - {} {} - {} {} {}",
            app.t("tui.sessions_4"),
            app.filtered.len(),
            app.t("tui.visible"),
            app.t("tui.filters"),
            active_filters,
            app.t("tui.sort"),
            sort_key_label(app.sort_key, app.language),
            if app.sort_desc {
                app.t("tui.desc")
            } else {
                app.t("tui.asc")
            }
        )
    }
}

pub(super) fn recent_limit(height: u16) -> usize {
    height.saturating_sub(2).max(1) as usize
}

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
        let mut seen = BTreeMap::new();
        for anomaly in &session.anomalies {
            seen.insert(anomaly.kind.clone(), ());
        }
        for label in seen.keys() {
            let entry = groups.entry(label.clone()).or_insert_with(|| DriverItem {
                label: label.clone(),
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

pub(super) fn compare_driver_items(left: &DriverItem, right: &DriverItem) -> Ordering {
    left.sessions
        .cmp(&right.sessions)
        .then_with(|| left.failures.cmp(&right.failures))
        .then_with(|| cmp_f64(left.cost, right.cost))
        .then_with(|| right.label.cmp(&left.label))
}

pub(super) fn inspect_first_lines(app: &App, width: u16) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(app.t("tui.rank_target_open_why"))];
    let items = &app.derived.inspect_first;
    if items.is_empty() {
        lines.push(Line::from(app.t("tui.no_priority_sessions")));
        return lines;
    }
    let name_width = if width >= 70 { 24 } else { 18 };
    for (rank, item) in items.iter().take(4).enumerate() {
        let Some(session) = app.sessions.get(item.index) else {
            continue;
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("{:<5}", rank + 1),
                Style::default()
                    .fg(if rank == 0 { Color::Cyan } else { Color::Gray })
                    .add_modifier(if rank == 0 {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
            Span::raw(format!("{} ", pad_display_width(&session.name, name_width))),
            Span::styled(
                pad_display_width(inspect_open_label(item.label, app.language), 12),
                Style::default().fg(inspect_label_color(item.label)),
            ),
            Span::raw(short(&triage_reason(session, app.language), 28)),
        ]));
        lines.push(Line::from(vec![
            Span::raw(format!("      {}=", app.t("tui.health_4"))),
            Span::styled(
                format!("{:<3} ", session.health),
                Style::default().fg(health_color(session.health)),
            ),
            Span::raw(format!(
                "{:<8} {}: {}",
                format_compact_cost(session.metrics.cost_estimated),
                app.t("tui.action"),
                short(
                    &selected_next_action(session, app.language),
                    width.saturating_sub(28).max(24) as usize
                )
            )),
        ]));
    }
    lines
}

pub(super) fn inspect_first_items<T: Borrow<Session>>(sessions: &[T]) -> Vec<InspectFirstItem> {
    let sessions = sessions
        .iter()
        .map(Borrow::borrow)
        .cloned()
        .collect::<Vec<_>>();
    inspect_first(&sessions)
        .into_iter()
        .map(|item| InspectFirstItem {
            label: item.reason,
            index: item.index,
        })
        .collect()
}

pub(super) fn inspect_first_items_for_app(app: &App) -> Vec<InspectFirstItem> {
    let indices = app.filtered.clone();
    let sessions = indices
        .iter()
        .map(|index| &app.sessions[*index])
        .collect::<Vec<_>>();
    inspect_first_items(&sessions)
        .into_iter()
        .filter_map(|item| {
            indices.get(item.index).map(|index| InspectFirstItem {
                index: *index,
                ..item
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

pub(super) fn inspect_open_label(label: &str, language: Language) -> &'static str {
    match inspect_target_view(label) {
        View::Detail => text(language, "tui.detail"),
        View::Diagnostics => text(language, "tui.diagnostics"),
        _ => text(language, "tui.open"),
    }
}

pub(super) fn inspect_label_color(label: &str) -> Color {
    match label {
        "critical" | "failures" => Color::LightRed,
        "anomaly" | "latency" => Color::Yellow,
        "cost" => Color::LightMagenta,
        _ => Color::Gray,
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
    if source.is_empty() || source == "auto-discovery" {
        return "auto discovery".to_string();
    }
    if source == "pi" || source.ends_with("/.pi/agent/sessions") {
        return "Pi sessions".to_string();
    }
    if source == "oh_my_pi" || source.ends_with("/.omp/agent/sessions") {
        return "Oh My Pi sessions".to_string();
    }
    if source == "claude_code" || source.ends_with("/.claude/projects") {
        return "Claude Code".to_string();
    }
    if source == "codex_cli" || source.contains("/.codex/") {
        return "Codex".to_string();
    }
    if source == "hermes_db" || source.ends_with("/.hermes/state.db") {
        return "Hermes DB".to_string();
    }
    if source == "opencode_db" || source.ends_with("/opencode.db") {
        return "OpenCode DB".to_string();
    }
    if source.contains('/') {
        return source
            .rsplit('/')
            .find(|part| !part.is_empty())
            .unwrap_or(source)
            .to_string();
    }
    source.to_string()
}

pub(super) fn driver_model(session: &Session) -> String {
    if session.metrics.model_used.is_empty() {
        "unknown".to_string()
    } else {
        session.metrics.model_used.clone()
    }
}

pub(super) fn driver_summary_line(
    label: &str,
    item: Option<DriverItem>,
    total_sessions: usize,
    language: Language,
) -> String {
    let Some(item) = item else {
        return format!("{label:<7} {}", text(language, "tui.none"));
    };
    let pct = (item.sessions * 100)
        .checked_div(total_sessions)
        .unwrap_or(0);
    format!(
        "{label:<7} {}  {}/{} {}%  {}{}  {}",
        short(&localized_anomaly(&item.label, language), 18),
        format_count(item.sessions as i64),
        format_count(total_sessions as i64),
        pct,
        text(language, "tui.fail"),
        format_count(item.failures as i64),
        format_compact_cost(item.cost)
    )
}

pub(super) fn bar_share(count: usize, total: usize, width: usize) -> usize {
    count.saturating_mul(width).checked_div(total).unwrap_or(0)
}

pub(super) fn driver_chart_line(
    kind: &str,
    item: Option<DriverItem>,
    total_sessions: usize,
    width: usize,
    language: Language,
) -> Line<'static> {
    let Some(item) = item else {
        return Line::from(format!("{kind:<7} {}", text(language, "tui.none")));
    };
    let filled = bar_share(item.sessions, total_sessions, width);
    let pct = item
        .sessions
        .saturating_mul(100)
        .checked_div(total_sessions)
        .unwrap_or(0);
    Line::from(vec![
        Span::raw(format!(
            "{kind:<7} {:<12} ",
            short(&localized_anomaly(&item.label, language), 12)
        )),
        Span::styled("█".repeat(filled), Style::default().fg(Color::Cyan)),
        Span::styled(
            "░".repeat(width - filled),
            Style::default().fg(Color::DarkGray),
        ),
        Span::raw(format!(" {pct:>3}% {}", format_compact_cost(item.cost))),
    ])
}

pub(super) fn format_compact_cost(cost: f64) -> String {
    format_cost(cost)
}

fn format_optional_cost(cost: Option<f64>, language: Language) -> String {
    cost.map(format_compact_cost)
        .unwrap_or_else(|| text(language, "tui.not_available").to_string())
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
    let mut gaps: Vec<f64> = sessions
        .iter()
        .flat_map(|session| session.borrow().metrics.gaps_sec.iter().copied())
        .filter(|value| value.is_finite() && *value > 0.0)
        .collect();
    if gaps.is_empty() {
        return 0.0;
    }
    gaps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    let index = ((gaps.len() as f64) * 0.95) as usize;
    gaps[index.min(gaps.len() - 1)]
}

pub(super) fn session_p95_gap(session: &Session) -> f64 {
    let mut gaps = session
        .metrics
        .gaps_sec
        .iter()
        .copied()
        .filter(|value| value.is_finite() && *value > 0.0)
        .collect::<Vec<_>>();
    if gaps.is_empty() {
        return 0.0;
    }
    gaps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    let index = ((gaps.len() as f64) * 0.95) as usize;
    gaps[index.min(gaps.len() - 1)]
}

pub(super) fn tool_success_rate(session: &Session) -> f64 {
    let total = session.metrics.tool_calls_total;
    if total == 0 {
        return 100.0;
    }
    let ok = total.saturating_sub(session.metrics.tool_calls_fail);
    ok as f64 / total as f64 * 100.0
}

pub(super) fn triage_reason(session: &Session, language: Language) -> String {
    if session.health < 50 {
        return text(language, "tui.critical_health").to_string();
    }
    if let Some(anomaly) = session
        .anomalies
        .iter()
        .find(|anomaly| anomaly.severity == "high")
        .or_else(|| session.anomalies.first())
    {
        return format!(
            "{} {}",
            localized_anomaly(&anomaly.kind, language),
            text(language, "tui.anomaly")
        );
    }
    if session.metrics.tool_calls_fail > 0 {
        return format!(
            "{} {}",
            session.metrics.tool_calls_fail,
            text(language, "tui.failed_tools")
        );
    }
    if session.metrics.cost_estimated >= 1.0 {
        return text(language, "tui.high_cost").to_string();
    }
    text(language, "tui.healthy").to_string()
}

pub(super) fn selected_next_action(session: &Session, language: Language) -> String {
    if session.health < 50 {
        return text(language, "tui.open_diagnostics_for_critical_health").to_string();
    }
    if let Some(anomaly) = session
        .anomalies
        .iter()
        .find(|anomaly| anomaly.severity == "high")
        .or_else(|| session.anomalies.first())
    {
        return match language {
            Language::En => format!("inspect {} anomaly in diagnostics", anomaly.kind),
            Language::Zh => format!(
                "在诊断中检查 {} 异常",
                localized_anomaly(&anomaly.kind, language)
            ),
        };
    }
    if session.metrics.tool_calls_fail > 0 {
        return text(language, "tui.inspect_failed_tool_results").to_string();
    }
    if session.metrics.cost_estimated >= 1.0 {
        return text(language, "tui.compare_cost_drivers_in_diff").to_string();
    }
    text(language, "tui.open_detail_for_full_report").to_string()
}

pub(super) fn next_action(app: &App) -> String {
    if app.sessions.is_empty() {
        if app.pending_load.is_some() {
            return app.t("tui.wait_for_loader").to_string();
        }
        return app.t("tui.load_sessions").to_string();
    }
    if matches!(app.view, View::Detail | View::Diagnostics) {
        if let Some(session) = app.selected_session() {
            return selected_next_action(session, app.language);
        }
    }
    if app.overview.critical > 0 {
        return app.t("tui.open_critical_sessions").to_string();
    }
    if app
        .sessions
        .iter()
        .any(|session| !session.anomalies.is_empty())
    {
        return app.t("tui.review_anomalies").to_string();
    }
    if app
        .sessions
        .iter()
        .any(|session| session.metrics.tool_calls_fail > 0)
    {
        return app.t("tui.inspect_failed_tools").to_string();
    }
    app.t("tui.watch_cost_and_latency").to_string()
}

pub(super) fn format_count(value: i64) -> String {
    format_tokens(value)
}

pub(super) fn format_duration(seconds: f64) -> String {
    if !seconds.is_finite() || seconds <= 0.0 {
        return "0s".to_string();
    }
    if seconds < 60.0 {
        return format!("{seconds:.0}s");
    }
    if seconds < 3600.0 {
        return format!("{:.1}m", seconds / 60.0);
    }
    if seconds < 86_400.0 {
        return format!("{:.1}h", seconds / 3600.0);
    }
    if seconds >= 365.0 * 86_400.0 {
        return format!("{:.1}y", seconds / (365.0 * 86_400.0));
    }
    format!("{:.1}d", seconds / 86_400.0)
}
