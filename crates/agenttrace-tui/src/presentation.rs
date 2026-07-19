use super::*;

pub(super) fn render(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();
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
    } else {
        match app.view {
            View::Overview => render_overview(frame, app, chunks[2]),
            View::List => render_list(frame, app, chunks[2]),
            View::Detail => render_detail(frame, app, chunks[2]),
            View::Diagnostics => render_report(
                frame,
                app,
                chunks[2],
                report_title(app, app.t("Diagnostics", "诊断")),
                diagnostics_text(app),
            ),
            View::Diff => render_report(frame, app, chunks[2], diff_title(app), diff_text(app)),
            View::Help => render_report(
                frame,
                app,
                chunks[2],
                format!(
                    "{} - {}",
                    app.t("Help", "帮助"),
                    context_view_label(app.help_context, app.language)
                ),
                help_text(app.help_context, app.language),
            ),
        }
    }
    render_footer(frame, app, chunks[3]);
}

pub(super) fn render_header(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let visible_count = app.filtered.len();
    let focus = app
        .selected_session()
        .map(|session| short(&session.name, 18))
        .unwrap_or_else(|| app.t("none", "无").to_string());
    let source = if area.width >= 118 {
        format!(
            "{}={}  {}={}  {}={}  {}={}",
            app.t("view", "视图"),
            context_view_label(app.view, app.language),
            app.t("focus", "焦点"),
            focus,
            app.t("source", "来源"),
            short(&display_source_label(&app.source_label), 18),
            app.t("sessions", "会话"),
            format_count(visible_count as i64)
        )
    } else if area.width >= 96 {
        format!(
            "{}={}  {}={}  {}={}",
            app.t("view", "视图"),
            context_view_label(app.view, app.language),
            app.t("focus", "焦点"),
            focus,
            app.t("sessions", "会话"),
            format_count(visible_count as i64)
        )
    } else {
        format!(
            "{}={}  n={}",
            app.t("src", "来源"),
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
            Span::raw(format!("  {}=", app.t("next", "下一步"))),
            Span::styled(
                next_action(app),
                Style::default()
                    .fg(priority_color(app))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw(format!("{} ", app.t("health", "健康度"))),
            Span::styled(
                format!("{:.1}", app.derived.average_health),
                Style::default()
                    .fg(health_color(app.derived.average_health as i32))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                format!("{}={}", app.t("ok", "良好"), app.overview.healthy),
                Style::default().fg(Color::Green),
            ),
            Span::raw(" "),
            Span::styled(
                format!("{}={}", app.t("warn", "警告"), app.overview.warning),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw(" "),
            Span::styled(
                format!("{}={}", app.t("crit", "严重"), app.overview.critical),
                Style::default().fg(Color::Red),
            ),
            Span::raw(format!(
                "  {}={}  {}={}  {}",
                app.t("cost", "成本"),
                format_compact_cost(app.overview.total_cost),
                app.t("tokens", "Token"),
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
    let selected = match app.view {
        View::Overview => 0,
        View::List => 1,
        View::Detail => 2,
        View::Diagnostics => 3,
        View::Diff => 4,
        View::Help => 5,
    };
    let tabs = Tabs::new([
        format!("0 {}", app.t("Overview", "概览")),
        format!("1 {}", app.t("List", "列表")),
        format!("2 {}", app.t("Detail", "详情")),
        format!("3 {}", app.t("Diagnostics", "诊断")),
        format!("4 {}", app.t("Diff", "对比")),
        format!("? {}", app.t("Help", "帮助")),
    ])
    .select(selected)
    .style(Style::default().fg(Color::Gray))
    .highlight_style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(tabs, area);
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
            Constraint::Length(7),
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
            Constraint::Length(6),
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
            Span::raw(format!("{} ", app.t("health", "健康度"))),
            Span::styled(
                format!("{:.1}", app.derived.average_health),
                Style::default()
                    .fg(health_color(app.derived.average_health as i32))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                "  {} {}  {} {}  {} {}",
                app.t("sessions", "会话"),
                format_count(app.overview.total_sessions as i64),
                app.t("critical", "严重"),
                format_count(app.overview.critical as i64),
                app.t("warning", "警告"),
                format_count(app.overview.warning as i64)
            )),
        ]),
        Line::from(format!(
            "{} {}  {} {}  {} {}  p95 {}",
            app.t("cost", "成本"),
            format_compact_cost(app.overview.total_cost),
            app.t("tokens", "Token"),
            format_tokens(app.derived.total_tokens),
            app.t("elapsed", "耗时"),
            format_duration(app.derived.total_duration),
            format_duration(app.derived.p95_gap)
        )),
        Line::from(format!("{}: {}", app.t("next", "下一步"), next_action(app))),
        Line::from(format!(
            "{}  {}={}  {}={}  {}={}%",
            top_model_line(app),
            app.t("Data Health", "数据健康"),
            localized_level(&health.confidence, app.language),
            app.t("range", "范围"),
            range_label(app.range_filter, app.language),
            app.t("detail coverage", "详细覆盖"),
            coverage_pct(health.with_diagnostics, health.parsed)
        )),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(app.t("Scoreboard", "指标总览")),
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
            app.t("healthy", "良好"),
            format_count(app.overview.healthy as i64),
            pct(app.overview.healthy),
            app.t("warning", "警告"),
            format_count(app.overview.warning as i64),
            pct(app.overview.warning)
        )),
        Line::from(format!(
            "{} {} ({}%)  {} {:.1}",
            app.t("critical", "严重"),
            format_count(app.overview.critical as i64),
            pct(app.overview.critical),
            app.t("average", "平均"),
            app.derived.average_health
        )),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(app.t("Health Distribution", "健康分布")),
            )
            .wrap(Wrap { trim: true }),
        area,
    );
}

pub(super) fn render_inspect_first(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let mut lines = vec![Line::from(format!(
        "{} {}  {} {}  {} {}  {} {}",
        app.t("tool failures", "工具失败"),
        format_count(app.derived.tool_failure_sessions as i64),
        app.t("stuck", "卡住"),
        format_count(app.derived.stuck_sessions as i64),
        app.t("context risk", "上下文风险"),
        format_count(app.derived.context_risk_sessions as i64),
        app.t("loops", "循环"),
        format_count(app.derived.loop_sessions as i64),
    ))];
    lines.extend(inspect_first_lines(app, area.width));
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(app.t(
                "Inspect First - Enter opens #1, :inspect N jumps",
                "优先检查 - Enter 打开 #1，:inspect N 跳转",
            )))
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
                "{:<name_width$} {:<8} {:<14} {}",
                short(&session.name, name_width),
                format_compact_cost(session.metrics.cost_estimated),
                short(&display_session_source(session), 14),
                short(&triage_reason(session, app.language), 24),
                name_width = name_width
            )),
        ]));
    }
    if lines.is_empty() {
        lines.push(Line::from(app.t("no sessions visible", "没有可见会话")));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(app.t("Recent Sessions", "最近会话")),
            )
            .wrap(Wrap { trim: true }),
        area,
    );
}

pub(super) fn render_list(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    if app.sessions.is_empty() {
        frame.render_widget(
            Paragraph::new(app.t(
                "No sessions loaded yet. Wait for loading or press r to reload.",
                "尚未加载会话。等待加载完成，或按 r 重新加载。",
            ))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(app.t("Sessions", "会话")),
            ),
            area,
        );
        return;
    }
    let active_filters = active_filter_summary(app);
    if app.filtered.is_empty() && !active_filters.is_empty() {
        let text = vec![
            Line::from(app.t(
                "No visible sessions match the active filters.",
                "没有会话匹配当前筛选。",
            )),
            Line::from(format!(
                "{}: {}",
                app.t("Active filters", "当前筛选"),
                active_filters
            )),
            Line::from(app.t(
                "Press Esc or run :clear to show all sessions.",
                "按 Esc 或运行 :clear 显示全部会话。",
            )),
        ];
        frame.render_widget(
            Paragraph::new(text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(app.t("Sessions - 0 visible", "会话 - 0 个可见")),
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
                app.t("session", "会话"),
                app.t("score", "评分"),
                app.t("cost", "成本"),
                app.t("fail", "失败"),
                app.t("reason", "原因"),
            ])
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(Block::default().borders(Borders::ALL).title(title))
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
                app.t("session", "会话"),
                app.t("health", "健康"),
                app.t("data", "数据"),
                app.t("source", "来源"),
                app.t("model", "模型"),
                app.t("cost", "成本"),
                app.t("tokens", "Token"),
                "ok%",
                app.t("fail", "失败"),
                app.t("anom", "异常"),
                app.t("reason", "原因"),
            ])
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(Block::default().borders(Borders::ALL).title(title))
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ")
    };
    let mut table_state = TableState::default();
    table_state.select(Some(app.selected.saturating_sub(start)));
    frame.render_stateful_widget(table, area, &mut table_state);
}

pub(super) fn render_selected_detail(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let text = app
        .selected_session()
        .map(|session| detail_summary_text(session, app.language))
        .unwrap_or_else(|| app.t("No selected session.", "未选中会话。").to_string());
    frame.render_widget(
        Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(app.t("Selected Detail", "选中会话详情")),
            )
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
        app.t("none", "无").to_string()
    } else {
        active_filters.to_string()
    };
    let hint = if active_filters.is_empty() {
        app.t(
            "Enter detail | 3 diagnostics | 4 diff",
            "Enter 详情 | 3 诊断 | 4 对比",
        )
    } else {
        app.t("Esc/:clear resets filters", "Esc/:clear 重置筛选")
    };
    let text = format!(
        "{}/{} {}  {}: {}  {}: {} {}  {}",
        format_count(app.filtered.len() as i64),
        format_count(app.sessions.len() as i64),
        app.t("visible", "可见"),
        app.t("filters", "筛选"),
        filter,
        app.t("sort", "排序"),
        sort_key_label(app.sort_key, app.language),
        if app.sort_desc {
            app.t("desc", "降序")
        } else {
            app.t("asc", "升序")
        },
        hint
    );
    frame.render_widget(
        Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(app.t("List Status", "列表状态")),
            )
            .wrap(Wrap { trim: true }),
        area,
    );
}

pub(super) fn render_loading_status(frame: &mut Frame<'_>, app: &App, area: Rect) {
    frame.render_widget(
        Paragraph::new(loading_status_lines(app))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(app.t("Loading Status", "加载状态")),
            )
            .wrap(Wrap { trim: true }),
        area,
    );
}

pub(super) fn render_driver_summary(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let total = app.filtered.len();
    let text = vec![
        Line::from(format!(
            "{}: {} {}",
            app.t("Visible", "可见"),
            format_count(total as i64),
            app.t("sessions", "会话")
        )),
        Line::from(driver_summary_line(
            app.t("Source", "来源"),
            app.derived.top_source.clone(),
            total,
            app.language,
        )),
        Line::from(driver_summary_line(
            app.t("Model", "模型"),
            app.derived.top_model.clone(),
            total,
            app.language,
        )),
        Line::from(driver_summary_line(
            app.t("Project", "项目"),
            app.derived.top_project.clone(),
            total,
            app.language,
        )),
        Line::from(driver_summary_line(
            app.t("Anomaly", "异常"),
            app.derived.top_anomaly.clone(),
            total,
            app.language,
        )),
    ];
    frame.render_widget(
        Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(app.t("Driver Summary", "主要影响因素")),
            )
            .wrap(Wrap { trim: true }),
        area,
    );
}

pub(super) fn render_driver_charts(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let total = app.filtered.len();
    let bar_width = area.width.saturating_sub(36).clamp(4, 28) as usize;
    let lines = [
        (app.t("Source", "来源"), app.derived.top_source.clone()),
        (app.t("Model", "模型"), app.derived.top_model.clone()),
        (app.t("Project", "项目"), app.derived.top_project.clone()),
        (app.t("Anomaly", "异常"), app.derived.top_anomaly.clone()),
    ]
    .into_iter()
    .map(|(kind, item)| driver_chart_line(kind, item, total, bar_width, app.language))
    .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(app.t("Driver Distribution", "影响因素分布")),
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
                app.t("selected", "选中"),
                short(&session.name, 24),
                app.t("reason", "原因"),
                short(&triage_reason(session, app.language), 22),
                tool_success_rate(session),
                app.t("fail", "失败"),
                format_count(session.metrics.tool_calls_fail as i64),
                app.t("anom", "异常"),
                format_count(session.anomalies.len() as i64),
                app.t("health", "健康度"),
                session.health,
                app.t("cost", "成本"),
                format_compact_cost(session.metrics.cost_estimated)
            )),
            Line::from(format!(
                "{}={}  {}={}  {}={}  {}={}  p95 {}={}",
                app.t("source", "来源"),
                short(&display_session_source(session), 18),
                app.t("model", "模型"),
                short(&driver_model(session), 24),
                app.t("tokens", "Token"),
                format_tokens(total_tokens(session)),
                app.t("elapsed", "耗时"),
                format_duration(session.metrics.duration_sec),
                app.t("latency", "延迟"),
                format_duration(session_p95_gap(session))
            )),
            Line::from(format!(
                "{}={}",
                app.t("action", "动作"),
                selected_next_action(session, app.language)
            )),
        ]
    } else {
        vec![Line::from(app.t("selected: none", "未选中会话"))]
    };
    frame.render_widget(
        Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(app.t("Selected Triage", "当前会话分析")),
            )
            .wrap(Wrap { trim: true }),
        area,
    );
}

pub(super) fn render_report(
    frame: &mut Frame<'_>,
    app: &App,
    area: Rect,
    title: impl Into<String>,
    text: String,
) {
    let text = terminal_safe_report(&text);
    frame.render_widget(
        Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title(title.into()))
            .scroll((app.scroll, 0))
            .wrap(Wrap { trim: false }),
        area,
    );
}

pub(super) fn render_detail(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let Some(session) = app.selected_session() else {
        render_report(
            frame,
            app,
            area,
            app.t("Detail", "详情"),
            app.t("No selected session.", "未选中会话。").to_string(),
        );
        return;
    };
    if app.raw_report_expanded || area.width < 110 {
        render_report(
            frame,
            app,
            area,
            report_title(app, app.t("Detail", "详情")),
            detail_text(app),
        );
        return;
    }

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(52), Constraint::Percentage(48)])
        .split(area);
    frame.render_widget(
        Paragraph::new(terminal_safe_report(&detail_summary_text(
            session,
            app.language,
        )))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(app.t("Session Overview", "会话概况")),
        )
        .scroll((app.scroll, 0))
        .wrap(Wrap { trim: false }),
        columns[0],
    );
    frame.render_widget(
        Paragraph::new(terminal_safe_report(&detail_diagnosis_text(
            session,
            app.language,
        )))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(app.t("Diagnosis", "诊断结论")),
        )
        .scroll((app.scroll, 0))
        .wrap(Wrap { trim: false }),
        columns[1],
    );
}

pub(super) fn render_footer(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let prompt = match app.mode {
        InputMode::Search => format!("/ {}", app.input),
        InputMode::Command => format!(": {}", app.input),
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
        return match app.view {
            View::Overview => app.t(
                "enter inspect | ! critical | ? help | q quit",
                "enter 检查 | ! 严重 | ? 帮助 | q 退出",
            ),
            View::List => app.t(
                "j/k move | Ctrl+d/u page | G end | enter detail | / search | ? help | q quit",
                "j/k 移动 | Ctrl+d/u 翻页 | G 末尾 | enter 详情 | / 搜索 | ? 帮助 | q 退出",
            ),
            View::Detail | View::Diagnostics | View::Diff => app.t(
                "pgup/pgdn scroll | Esc back | ? help | q quit",
                "pgup/pgdn 滚动 | Esc 返回 | ? 帮助 | q 退出",
            ),
            View::Help => app.t("? or Esc back | q quit", "? 或 Esc 返回 | q 退出"),
        }
        .to_string();
    }
    match app.view {
        View::Overview => app.t(
            "enter inspect | ! critical | $ cost | f health | R range | tab view | : cmd | ? help | q quit",
            "enter 检查 | ! 严重 | $ 成本 | f 健康 | R 范围 | tab 视图 | : 命令 | ? 帮助 | q 退出",
        ),
        View::List => app.t(
            "j/k select | Ctrl+d/u page | G end | enter detail | 3 diag | / search | ? help | q quit",
            "j/k 选择 | Ctrl+d/u 翻页 | G 末尾 | enter 详情 | 3 诊断 | / 搜索 | ? 帮助 | q 退出",
        ),
        View::Detail => app.t(
            "v raw report | 3 diagnostics | 4 diff | Esc back | ? help | q quit",
            "v 原始报告 | 3 诊断 | 4 对比 | Esc 返回 | ? 帮助 | q 退出",
        ),
        View::Diagnostics => app.t(
            "pgup/pgdn scroll | 2 detail | 4 diff | Esc back | ? help | q quit",
            "pgup/pgdn 滚动 | 2 详情 | 4 对比 | Esc 返回 | ? 帮助 | q 退出",
        ),
        View::Diff => app.t(
            "j/k pair | 2 detail | 3 diagnostics | Esc back | ? help | q quit",
            "j/k 对比项 | 2 详情 | 3 诊断 | Esc 返回 | ? 帮助 | q 退出",
        ),
        View::Help => app.t("? or Esc back | q quit", "? 或 Esc 返回 | q 退出"),
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
        app.t("reason", "原因"),
        short(&triage_reason(session, app.language), 18)
    )
}

pub(super) fn diff_title(app: &App) -> String {
    let active_filters = active_filter_summary(app);
    if active_filters.is_empty() {
        format!(
            "{} - {} {} - {} {} {}",
            app.t("Diff", "对比"),
            app.filtered.len(),
            app.t("visible", "可见"),
            app.t("sort", "排序"),
            sort_key_label(app.sort_key, app.language),
            if app.sort_desc {
                app.t("desc", "降序")
            } else {
                app.t("asc", "升序")
            }
        )
    } else {
        format!(
            "{} - {} {} - {} {} - {} {} {}",
            app.t("Diff", "对比"),
            app.filtered.len(),
            app.t("visible", "可见"),
            app.t("filter", "筛选"),
            active_filters,
            app.t("sort", "排序"),
            sort_key_label(app.sort_key, app.language),
            if app.sort_desc {
                app.t("desc", "降序")
            } else {
                app.t("asc", "升序")
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
                    app.t("Raw report", "原始报告"),
                    report_text_with_language(session, app.language.report()),
                )
            } else {
                summary
            }
        })
        .unwrap_or_else(|| app.t("No selected session.", "未选中会话。").to_string())
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
            text(language, "source", "来源"),
            display_session_source(session),
            text(language, "model", "模型"),
            driver_model(session),
            text(language, "data", "数据"),
            capability_label(session, language),
            text(language, "p95 gap", "P95 间隔"),
            format_duration(session_p95_gap(session))
        ),
        format!(
            "{}={}  {}={:.0}%  {}={}",
            text(language, "failures", "失败"),
            format_count(metrics.tool_calls_fail as i64),
            text(language, "tool success", "工具成功率"),
            tool_success_rate(session),
            text(language, "anomalies", "异常"),
            format_count(session.anomalies.len() as i64)
        ),
        String::new(),
        format!(
            "{}: {}",
            text(language, "Name", "名称"),
            short(&session.name, 52)
        ),
        format!(
            "{}: {}",
            text(language, "Workspace", "工作区"),
            if session.cwd.is_empty() {
                text(language, "unknown", "未知").to_string()
            } else {
                short_path(&session.cwd, 58)
            }
        ),
        format!(
            "{}: {}",
            text(language, "Session file", "会话文件"),
            short_path(&session.path, 58)
        ),
    ];
    lines.extend([
        String::new(),
        format!(
            "{}: {}={}  {}={}  {}={}  {}={}",
            text(language, "Tokens", "Token"),
            text(language, "input", "输入"),
            format_tokens(metrics.tokens_input),
            text(language, "output", "输出"),
            format_tokens(metrics.tokens_output),
            text(language, "cache write", "缓存写入"),
            format_tokens(metrics.tokens_cache_w),
            text(language, "cache read", "缓存读取"),
            format_tokens(metrics.tokens_cache_r)
        ),
        format!(
            "{}: {}={}  {}={}  {}={}",
            text(language, "Turns", "轮次"),
            text(language, "user", "用户"),
            format_count(metrics.user_messages as i64),
            text(language, "assistant", "助手"),
            format_count(metrics.assistant_turns as i64),
            text(language, "tool results", "工具结果"),
            format_count(metrics.tool_results as i64)
        ),
    ]);
    lines.join("\n")
}

pub(super) fn detail_diagnosis_text(session: &Session, language: Language) -> String {
    let mut lines = vec![
        format!(
            "{}: {}",
            text(language, "Primary issue", "主要问题"),
            triage_reason(session, language)
        ),
        format!(
            "{}: {}",
            text(language, "Next action", "下一步动作"),
            selected_next_action(session, language)
        ),
        String::new(),
        format!(
            "{}: {}",
            text(language, "Evidence confidence", "证据可信度"),
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
    lines.push(
        text(
            language,
            "Press v to view the raw report",
            "按 v 查看原始报告",
        )
        .to_string(),
    );
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
                    app.t("Cost alert", "成本预警"),
                    localized_level(&alert.level, app.language),
                    cost_alert_message(&alert.message, app.language),
                    alert.current,
                    alert.baseline,
                    alert.ratio
                ));
            }
            report_with_context(
                summary,
                app.t("Raw diagnostics", "原始诊断"),
                render_waste_report_with_language(session, app.language.report()),
            )
        })
        .unwrap_or_else(|| app.t("No selected session.", "未选中会话。").to_string())
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
        text(language, "Context", "上下文"),
        text(language, "reason", "原因"),
        triage_reason(session, language),
        text(language, "health", "健康度"),
        session.health,
        text(language, "cost", "成本"),
        format_compact_cost(session.metrics.cost_estimated),
        text(language, "fail", "失败"),
        format_count(session.metrics.tool_calls_fail as i64),
        text(language, "anom", "异常"),
        format_count(session.anomalies.len() as i64),
        text(language, "source", "来源"),
        display_session_source(session)
    )
}

pub(super) fn detail_native_text(session: &Session, language: Language) -> String {
    let metrics = &session.metrics;
    let mut lines = vec![
        text(language, "Session Summary", "会话摘要").to_string(),
        "---------------".to_string(),
        report_context_line(session, language),
        format!("{}: {}", text(language, "Name", "名称"), session.name),
        format!(
            "{}: {}",
            text(language, "Workspace", "工作区"),
            if session.cwd.is_empty() {
                text(language, "unknown", "未知")
            } else {
                &session.cwd
            }
        ),
        format!(
            "{}: {}",
            text(language, "Session file", "会话文件"),
            session.path
        ),
        format!(
            "{}: {}={} {}={}",
            text(language, "Driver", "驱动"),
            text(language, "source", "来源"),
            display_session_source(session),
            text(language, "model", "模型"),
            driver_model(session)
        ),
        format!(
            "{}: {}={} {}={} {}={} {}={}",
            text(language, "Timeline", "时间线"),
            text(language, "start", "开始"),
            empty_as_unknown(&metrics.session_start),
            text(language, "end", "结束"),
            empty_as_unknown(&metrics.session_end),
            text(language, "elapsed", "耗时"),
            format_duration(metrics.duration_sec),
            text(language, "p95 gap", "P95 间隔"),
            format_duration(session_p95_gap(session))
        ),
        format!(
            "{}: {}={} {}={} {}={} {}={}",
            text(language, "Turns", "轮次"),
            text(language, "events", "事件"),
            format_count(metrics.events_total as i64),
            text(language, "user", "用户"),
            format_count(metrics.user_messages as i64),
            text(language, "assistant", "助手"),
            format_count(metrics.assistant_turns as i64),
            text(language, "tool results", "工具结果"),
            format_count(metrics.tool_results as i64)
        ),
        format!(
            "{}: {}={} {}={} {}={:.0}%",
            text(language, "Tools", "工具"),
            text(language, "total", "总数"),
            format_count(metrics.tool_calls_total as i64),
            text(language, "failed", "失败"),
            format_count(metrics.tool_calls_fail as i64),
            text(language, "success", "成功率"),
            tool_success_rate(session)
        ),
        format!(
            "{}: {}={} {}={} {}={} {}={} {}={}",
            text(language, "Tokens", "Token"),
            text(language, "input", "输入"),
            format_tokens(metrics.tokens_input),
            text(language, "output", "输出"),
            format_tokens(metrics.tokens_output),
            text(language, "cache write", "缓存写入"),
            format_tokens(metrics.tokens_cache_w),
            text(language, "cache read", "缓存读取"),
            format_tokens(metrics.tokens_cache_r),
            text(language, "total", "总数"),
            format_tokens(total_tokens(session))
        ),
        format!(
            "{}: {}",
            text(language, "Cost", "成本"),
            format_compact_cost(metrics.cost_estimated)
        ),
        String::new(),
        text(language, "Next Action", "下一步动作").to_string(),
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
        text(language, "Problem", "问题").to_string(),
        "-------".to_string(),
        report_context_line(session, language),
        String::new(),
        text(language, "Evidence", "证据").to_string(),
        "--------".to_string(),
        format!(
            "{}={} {}={}",
            text(language, "health", "健康度"),
            session.health,
            text(language, "reason", "原因"),
            triage_reason(session, language)
        ),
        format!(
            "{}={} {}={}",
            text(language, "source", "来源"),
            display_session_source(session),
            text(language, "model", "模型"),
            driver_model(session)
        ),
        format!(
            "{}={} {}={} {}={} {}={}",
            text(language, "duration", "时长"),
            format_duration(metrics.duration_sec),
            text(language, "p95 gap", "P95 间隔"),
            format_duration(session_p95_gap(session)),
            text(language, "failures", "失败"),
            format_count(metrics.tool_calls_fail as i64),
            text(language, "anomalies", "异常"),
            format_count(session.anomalies.len() as i64)
        ),
        format!(
            "{}={} {}={} {}={} {}={:.0}%",
            text(language, "cost", "成本"),
            format_compact_cost(metrics.cost_estimated),
            text(language, "tokens", "Token"),
            format_tokens(total_tokens(session)),
            text(language, "cache read share", "缓存读取占比"),
            token_share(metrics.tokens_cache_r, total_tokens(session)),
            text(language, "tool success", "工具成功率"),
            tool_success_rate(session)
        ),
        String::new(),
        text(language, "Next", "下一步").to_string(),
        "----".to_string(),
    ];
    lines.extend(
        diagnostic_actions(session, language)
            .into_iter()
            .take(4)
            .map(|line| format!("- {line}")),
    );
    lines.push(String::new());
    lines.push(text(language, "Raw Signals", "原始信号").to_string());
    lines.push("-----------".to_string());
    lines.extend(signal_lines(session, language).into_iter().take(5));
    lines.extend(anomaly_lines(session, 6, language));
    lines.push(String::new());
    lines.extend(step_lines(session, language, 12));
    let diagnostics = &session.diagnostics;
    if diagnostics.loop_cost.total_loop_cost > 0.0 {
        lines.push(format!(
            "{}: {}={} {}={} {}={} type={} turns={}",
            text(language, "Loop analysis", "循环分析"),
            text(language, "cost", "成本"),
            format_compact_cost(diagnostics.loop_cost.total_loop_cost),
            text(language, "retries", "重试"),
            diagnostics.loop_cost.retry_events,
            text(language, "groups", "组数"),
            diagnostics.loop_cost.loop_groups,
            diagnostics.loop_cost.loop_type,
            diagnostics.loop_cost.turns
        ));
    }
    for warning in &session.tool_warnings {
        lines.push(format!(
            "{}: {}",
            text(language, "Tool warning", "工具警告"),
            localized_tool_warning(&warning.detail, language)
        ));
    }
    for latency in diagnostics
        .tool_latencies
        .iter()
        .filter(|item| item.p95_sec > 30.0 || item.timeouts > 0)
        .take(3)
    {
        lines.push(format!(
            "{}: {} min={:.1}s p95={:.1}s {}={:.1}s {}={}",
            text(language, "Tool latency", "工具延迟"),
            latency.tool_name,
            latency.min_sec,
            latency.p95_sec,
            text(language, "max", "最大"),
            latency.max_sec,
            text(language, "timeouts", "超时"),
            latency.timeouts
        ));
    }
    lines.push(format!(
        "{}: {:.1}% {}={} {}={}",
        text(language, "Context utilization", "上下文利用率"),
        diagnostics.context_utilization.utilization_pct,
        text(language, "risk", "风险"),
        localized_level(&diagnostics.context_utilization.risk_level, language),
        text(language, "available", "可用"),
        format_tokens(diagnostics.context_utilization.available_for_task as i64)
    ));
    if !diagnostics.context_utilization.suggestion.is_empty() {
        lines.push(format!(
            "{}: {}",
            text(language, "Context suggestion", "上下文建议"),
            diagnostics.context_utilization.suggestion
        ));
    }
    for item in diagnostics.large_params.iter().take(3) {
        lines.push(format!(
            "{}: {} {} {} {}={}",
            text(language, "Large parameter", "大参数"),
            item.tool_name,
            item.size,
            text(language, "bytes", "字节"),
            text(language, "risk", "风险"),
            localized_level(&item.risk, language)
        ));
        lines.push(format!("  {} {}", item.timestamp, item.detail));
    }
    for item in diagnostics.unused_tools.iter().take(3) {
        lines.push(format!(
            "{}: {} {} {} {}",
            text(language, "Rare tool", "低频工具"),
            item.tool_name,
            text(language, "called", "调用"),
            item.call_count,
            text(language, "time(s)", "次")
        ));
        lines.push(format!("  [{}] {}", item.level, item.detail));
    }
    for item in diagnostics.stuck_patterns.iter().take(3) {
        lines.push(format!(
            "{}: {}",
            text(language, "Stuck pattern", "卡住模式"),
            localized_stuck_pattern(&item.description, language)
        ));
    }
    for fix in fix_suggestions(session).into_iter().take(3) {
        lines.push(format!(
            "{} [{}]: {} — {}",
            text(language, "Fix", "修复建议"),
            localized_level(&fix.severity, language),
            fix.category,
            localized_fix_action(&fix.action, language)
        ));
        lines.push(format!("  {}", fix.description));
    }
    lines.join("\n")
}

pub(super) fn step_lines(session: &Session, language: Language, limit: usize) -> Vec<String> {
    let mut lines = vec![text(language, "Steps (metadata only)", "步骤（仅元数据）").to_string()];
    if session.diagnostics.steps.is_empty() {
        lines.push(
            text(
                language,
                "- unavailable for this source",
                "- 当前来源不提供步骤详情",
            )
            .to_string(),
        );
        return lines;
    }
    lines.extend(session.diagnostics.steps.iter().take(limit).map(|step| {
        format!(
            "- {} {}  {}  {}",
            step.kind,
            short(&step.name, 28),
            step.status,
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
            text(language, "more", "更多")
        ));
    }
    lines
}

pub(super) fn diagnostic_actions(session: &Session, language: Language) -> Vec<String> {
    let mut actions = Vec::new();
    actions.push(selected_next_action(session, language));
    if session.health < 50 {
        actions.push(
            text(
                language,
                "check failed tool calls and high-severity anomalies before cost tuning",
                "先检查失败工具调用和高严重度异常，再看成本优化",
            )
            .to_string(),
        );
    }
    if session.metrics.tool_calls_fail > 0 {
        actions.push(
            text(
                language,
                "filter by failed tools or inspect raw diagnostics for tool errors",
                "按失败工具筛选，或查看原始诊断里的工具错误",
            )
            .to_string(),
        );
    }
    if !session.anomalies.is_empty() {
        actions.push(
            text(
                language,
                "review anomaly details and compare against nearby healthy sessions",
                "查看异常详情，并和相近的健康会话对比",
            )
            .to_string(),
        );
    }
    if session.metrics.cost_estimated >= 1.0 {
        actions.push(
            text(
                language,
                "open Diff to compare cost drivers against cheaper sessions",
                "打开对比视图，和更低成本会话比较成本来源",
            )
            .to_string(),
        );
    }
    if actions.len() == 1 {
        actions.push(
            text(
                language,
                "use Raw report when you need the full narrative",
                "需要完整叙事时查看原始报告",
            )
            .to_string(),
        );
    }
    actions
}

pub(super) fn localized_level(value: &str, language: Language) -> String {
    if language == Language::En {
        return value.to_string();
    }
    match value {
        "critical" => "严重",
        "warning" => "警告",
        "high" => "高",
        "medium" => "中",
        "good" => "良好",
        "low" => "低",
        "info" => "提示",
        _ => value,
    }
    .to_string()
}

pub(super) fn localized_anomaly(kind: &str, language: Language) -> String {
    if language == Language::En {
        return kind.replace('_', " ");
    }
    match kind {
        "hanging" => "卡顿",
        "latency" => "延迟",
        "tool_failures" => "工具失败",
        "shallow_thinking" => "推理过浅",
        "redaction" | "redacted" => "推理脱敏",
        "no_tools" => "未使用工具",
        _ => kind,
    }
    .to_string()
}

pub(super) fn anomaly_detail_for_tui(
    anomaly: &agenttrace_core::Anomaly,
    language: Language,
) -> String {
    if language == Language::En {
        return empty_as_unknown(&anomaly.detail).to_string();
    }
    match anomaly.kind.as_str() {
        "no_tools" => "无工具调用，仅包含对话".to_string(),
        "hanging" => anomaly
            .detail
            .strip_suffix('s')
            .map(|value| value.replace(" gap(s) >60s, max=", " 个间隔超过 60 秒，最长=") + "秒")
            .unwrap_or_else(|| anomaly.detail.clone()),
        "latency" => anomaly
            .detail
            .strip_prefix("p95 latency = ")
            .and_then(|value| value.strip_suffix('s'))
            .map(|value| format!("P95 延迟 = {value} 秒"))
            .unwrap_or_else(|| anomaly.detail.clone()),
        "tool_failures" => anomaly.detail.replace(" failed", " 次失败"),
        "redaction" | "redacted" => anomaly.detail.replace(" block(s) redacted", " 个块已脱敏"),
        "shallow_thinking" => anomaly
            .detail
            .replace("avg reasoning = ", "平均推理 = ")
            .replace(" chars", " 字符")
            .replace("very shallow", "非常浅"),
        _ => anomaly.detail.clone(),
    }
}

pub(super) fn localized_stuck_pattern(value: &str, language: Language) -> String {
    if language == Language::En {
        return value.to_string();
    }
    value
        .replace(" gaps exceed 120s", " 个间隔超过 120 秒")
        .replace("Repeated assistant response ", "助手重复响应 ")
        .replace(" times", " 次")
        .replace(" tool calls have no result", " 个工具调用没有结果")
}

pub(super) fn localized_fix_action(value: &str, language: Language) -> String {
    if language == Language::En {
        return value.to_string();
    }
    match value {
        "Add cancellation and a bounded timeout to long-running tools." => {
            "为长时间运行的工具增加取消机制和有限超时。"
        }
        "Inspect failed arguments and stop retrying unchanged calls." => {
            "检查失败参数，并停止重试未变化的调用。"
        }
        "Plan and verify the risky steps before execution." => "执行前规划并验证高风险步骤。",
        "Check whether missing reasoning hides the failure boundary." => {
            "检查缺失的推理是否掩盖了失败边界。"
        }
        "Inspect concrete artifacts instead of relying on chat-only reasoning." => {
            "检查具体产物，不要只依赖对话推理。"
        }
        _ => value,
    }
    .to_string()
}

pub(super) fn localized_tool_warning(value: &str, language: Language) -> String {
    if language == Language::En {
        return value.to_string();
    }
    value
        .replace("tool ", "工具 ")
        .replace("uses broad authority", "使用了宽泛权限")
        .replace("sensitive path", "敏感路径")
}

pub(super) fn cost_alert_message(value: &str, language: Language) -> String {
    if language == Language::En {
        return value.to_string();
    }
    value
        .replace("Cost/turn is ", "每轮成本是基线的 ")
        .replace("x the session baseline.", " 倍。")
        .replace("Loop waste is ", "循环浪费占会话成本的 ")
        .replace("% of session cost.", "% 。")
        .replace("No comparable cost history.", "没有可比较的成本历史。")
}

pub(super) fn signal_lines(session: &Session, language: Language) -> Vec<String> {
    let metrics = &session.metrics;
    let mut lines = Vec::new();
    if let Some(line) = top_usage_line(
        text(language, "Top tool", "最高频工具"),
        &metrics.tool_usage,
        42,
    ) {
        lines.push(line);
    }
    if let Some(line) = top_usage_line(
        text(language, "Top file", "最高频文件"),
        &metrics.file_usage,
        42,
    ) {
        lines.push(line);
    }
    if let Some(line) = top_usage_line(
        text(language, "Top arg", "最高频参数"),
        &metrics.tool_arg_usage,
        42,
    ) {
        lines.push(line);
    }
    if let Some(line) = top_usage_line(
        text(language, "Authority", "权限"),
        &metrics.tool_authority,
        42,
    ) {
        lines.push(line);
    }
    if !metrics.highest_authority.is_empty() && metrics.highest_authority != "unknown_authority" {
        lines.push(format!(
            "{}: {}",
            text(language, "Highest authority", "最高权限"),
            metrics.highest_authority
        ));
    }
    if metrics.reasoning_blocks > 0 {
        lines.push(format!(
            "{}: {}={} {}={} {}={}",
            text(language, "Reasoning", "推理"),
            text(language, "blocks", "块"),
            format_count(metrics.reasoning_blocks as i64),
            text(language, "chars", "字符"),
            format_count(metrics.reasoning_chars as i64),
            text(language, "redacted", "已脱敏"),
            format_count(metrics.reasoning_redact as i64)
        ));
    }
    if lines.is_empty() {
        lines.push(
            text(
                language,
                "Signals: no tool/file hotspots recorded",
                "信号：未记录工具/文件热点",
            )
            .to_string(),
        );
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
        text(language, "Anomalies", "异常").to_string(),
        "---------".to_string(),
    ];
    if session.anomalies.is_empty() {
        lines.push(format!("- {}", text(language, "none", "无")));
        return lines;
    }
    for anomaly in session.anomalies.iter().take(limit) {
        lines.push(format!(
            "- {} {}: {}",
            localized_level(empty_as_unknown(&anomaly.severity), language),
            localized_anomaly(empty_as_unknown(&anomaly.kind), language),
            anomaly_detail_for_tui(anomaly, language)
        ));
    }
    if session.anomalies.len() > limit {
        lines.push(format!(
            "- ... {} {}",
            format_count((session.anomalies.len() - limit) as i64),
            text(language, "more", "更多")
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
        let filters = active_filter_summary(app);
        let filter_hint = if filters.is_empty() {
            format!(
                "{}: {}",
                app.t("Active filters", "当前筛选"),
                app.t("none", "无")
            )
        } else {
            format!("{}: {filters}", app.t("Active filters", "当前筛选"))
        };
        return format!(
            "{context}\n\n{}\n{filter_hint}\n{}",
            app.t(
                "Need at least two visible sessions for diff.",
                "至少需要两个可见会话才能对比。"
            ),
            app.t(
                "Press Esc or run :clear/:reset to broaden the comparison set.",
                "按 Esc 或运行 :clear/:reset 扩大对比范围。"
            )
        );
    }
    let (left, right) = diff_pair(sessions.len(), app.selected);
    format!(
        "{context}\n\n{}",
        report_compare_with_language(
            &[sessions[left].clone(), sessions[right].clone()],
            "default",
            app.language.report(),
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
    let filters = active_filter_summary(app);
    let filter_text = if filters.is_empty() {
        app.t("none", "无").to_string()
    } else {
        filters
    };
    let top_source = app
        .derived
        .top_source
        .as_ref()
        .map(|item| format!("{}:{}", item.label, format_count(item.sessions as i64)))
        .unwrap_or_else(|| app.t("none", "无").to_string());
    format!(
        "{}: {}={} {}={} {}={} {} {}={}",
        app.t("Context", "上下文"),
        app.t("visible", "可见"),
        format_count(visible_count as i64),
        app.t("filter", "筛选"),
        filter_text,
        app.t("sort", "排序"),
        sort_key_label(app.sort_key, app.language),
        if app.sort_desc {
            app.t("desc", "降序")
        } else {
            app.t("asc", "升序")
        },
        app.t("top source", "主要来源"),
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
        (View::Help, Language::En) => ["Current view: Help", "  ? or Esc returns", "", ""],
        (View::Help, Language::Zh) => ["当前视图：帮助", "  ? 或 Esc 返回", "", ""],
    };
    let common = match language {
        Language::En => [
            "Triage workflow",
            "  Start on Overview. Inspect First ranks the sessions most worth opening.",
            "  enter on Overview opens the top Inspect First item.",
            "  l switches language between English and Chinese.",
            "  ! critical sessions, $ costly sessions, f cycles health filters",
            "  R cycles Today/7d/30d/All ranges",
            "  enter outside Overview opens detail, 3 opens diagnostics for the selected session",
            "",
            "Navigation",
            "  0 overview, 1 list, 2 detail, 3 diagnostics, 4 diff, tab next view",
            "  j/k or arrows move selection; Ctrl+d/u moves half a page; G jumps to the end",
            "",
            "Filters and sorting",
            "  / text, Esc clear, s selected source, h health sort, c cost sort",
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
            "  默认从概览开始。优先检查会把最值得打开的会话排在前面。",
            "  在概览按 enter 会打开优先检查的第一项。",
            "  按 l 在英文和中文之间切换。",
            "  ! 筛严重会话，$ 筛高成本会话，f 循环健康度筛选。",
            "  R 在今天/7天/30天/全部之间切换。",
            "  在其它视图按 enter 打开详情，按 3 打开选中会话的诊断。",
            "",
            "导航",
            "  0 概览，1 列表，2 详情，3 诊断，4 对比，tab 下一个视图。",
            "  j/k 或方向键移动选择；Ctrl+d/u 半页移动；G 跳到末尾。",
            "",
            "筛选和排序",
            "  / 文本，Esc 清除，s 选中来源，h 健康度排序，c 成本排序。",
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
        app.t("force reload", "强制重载")
    } else {
        app.t("normal load", "正常加载")
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
        format!("{}={}", app.t("sources", "来源"), app.t("none", "无"))
    } else {
        format!(
            "{}={}",
            app.t("sources", "来源"),
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
            app.t("from", "来自"),
            short(&display_source_label(&state.source), 36)
        )),
        Line::from(format!(
            "{} {}/{} {}, {} {}, {}",
            app.t("loaded", "已加载"),
            format_count(processed as i64),
            format_count(state.discovered as i64),
            app.t("files processed", "个文件已处理"),
            format_count(state.cache_hits as i64),
            app.t("cache hits", "缓存命中"),
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
            app.t("sessions parsed", "已解析会话"),
            format_count(state.parsed as i64),
            app.t("confidence", "可信度"),
            localized_level(&health.confidence, app.language),
            app.t("skipped", "跳过"),
            format_count(state.skipped as i64),
            app.t("pricing fallback", "价格回退"),
            format_count(health.fallback_pricing as i64),
            app.t("latest", "最新"),
            if health.latest_session_at.is_empty() {
                app.t("unknown", "未知").to_string()
            } else {
                short(&health.latest_session_at, 20)
            }
        )),
    ]
}

pub(super) fn load_summary_line(app: &App) -> String {
    let state = &app.load_state;
    match state.phase {
        LoadPhase::Idle => app.t("idle", "空闲").to_string(),
        LoadPhase::Discovering => format!(
            "{} {} {}",
            app.t("discovering", "发现中"),
            format_count(state.discovered as i64),
            app.t("files", "个文件")
        ),
        LoadPhase::Parsing => format!(
            "{} {} {}, {} {}",
            app.t("loading", "加载中"),
            format_count(state.discovered as i64),
            app.t("files", "个文件"),
            format_count(state.cache_hits as i64),
            app.t("cache hits", "缓存命中")
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
                .unwrap_or_else(|| app.t("none", "无").to_string());
            format!(
                "{} {} {}, {} {}, {source}",
                app.t("loaded", "已加载"),
                format_count(state.parsed as i64),
                app.t("sessions", "个会话"),
                format_count(state.cache_hits as i64),
                app.t("cache hits", "缓存命中")
            )
        }
        LoadPhase::Failed => app.t("load failed", "加载失败").to_string(),
    }
}

pub(super) fn load_phase_label(phase: LoadPhase, language: Language) -> &'static str {
    match phase {
        LoadPhase::Idle => text(language, "Idle", "空闲"),
        LoadPhase::Discovering => text(language, "Discovering", "发现中"),
        LoadPhase::Parsing => text(language, "Loading", "加载中"),
        LoadPhase::Ready => text(language, "Ready", "就绪"),
        LoadPhase::Failed => text(language, "Failed", "失败"),
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
            app.t("top model", "最高频模型"),
            short(model, 24),
            app.t("sessions", "会话"),
            format_count(group.sessions as i64),
            app.t("cost", "成本"),
            format_compact_cost(group.cost)
        )
    } else {
        format!(
            "{} {}",
            app.t("top model", "最高频模型"),
            app.t("none", "无")
        )
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
        format!("{health} {}", text(language, "ok", "良好"))
    } else if health >= 50 {
        format!("{health} {}", text(language, "warn", "警告"))
    } else {
        format!("{health} {}", text(language, "crit", "严重"))
    }
}

pub(super) fn session_table_title(app: &App, active_filters: &str) -> String {
    if active_filters.is_empty() {
        format!(
            "{} - {} {} - {} {} {}",
            app.t("Sessions", "会话"),
            app.filtered.len(),
            app.t("visible", "可见"),
            app.t("sort", "排序"),
            sort_key_label(app.sort_key, app.language),
            if app.sort_desc {
                app.t("desc", "降序")
            } else {
                app.t("asc", "升序")
            }
        )
    } else {
        format!(
            "{} - {} {} - {} {} - {} {} {}",
            app.t("Sessions", "会话"),
            app.filtered.len(),
            app.t("visible", "可见"),
            app.t("filters", "筛选"),
            active_filters,
            app.t("sort", "排序"),
            sort_key_label(app.sort_key, app.language),
            if app.sort_desc {
                app.t("desc", "降序")
            } else {
                app.t("asc", "升序")
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
    let mut lines = vec![Line::from(app.t(
        "rank  target                  open          why",
        "排名  目标                    打开          原因",
    ))];
    let items = &app.derived.inspect_first;
    if items.is_empty() {
        lines.push(Line::from(
            app.t("no priority sessions", "没有需要优先检查的会话"),
        ));
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
            Span::raw(format!(
                "{:<name_width$} ",
                short(&session.name, name_width),
                name_width = name_width
            )),
            Span::styled(
                format!("{:<12}", inspect_open_label(item.label, app.language)),
                Style::default().fg(inspect_label_color(item.label)),
            ),
            Span::raw(short(&triage_reason(session, app.language), 28)),
        ]));
        lines.push(Line::from(vec![
            Span::raw(format!("      {}=", app.t("health", "健康度"))),
            Span::styled(
                format!("{:<3} ", session.health),
                Style::default().fg(health_color(session.health)),
            ),
            Span::raw(format!(
                "{:<8} {}: {}",
                format_compact_cost(session.metrics.cost_estimated),
                app.t("action", "动作"),
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
    let now = chrono::Utc::now();
    let indices = app
        .sessions
        .iter()
        .enumerate()
        .filter(|(_, session)| {
            matches_source_filter(session, &app.source_filter)
                && matches_text_filter(&session.metrics.model_used, &app.model_filter)
                && matches_text_filter(&project_name(session), &app.project_filter)
                && session_matches_time_range(session, app.range_filter, now)
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
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
        View::Detail => text(language, "Detail", "详情"),
        View::Diagnostics => text(language, "Diagnostics", "诊断"),
        _ => text(language, "Open", "打开"),
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
        return format!("{label:<7} {}", text(language, "none", "无"));
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
        text(language, "fail", "失败"),
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
        return Line::from(format!("{kind:<7} {}", text(language, "none", "无")));
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
        return text(language, "critical health", "健康度严重").to_string();
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
            text(language, "anomaly", "异常")
        );
    }
    if session.metrics.tool_calls_fail > 0 {
        return format!(
            "{} {}",
            session.metrics.tool_calls_fail,
            text(language, "failed tools", "个失败工具")
        );
    }
    if session.metrics.cost_estimated >= 1.0 {
        return text(language, "high cost", "高成本").to_string();
    }
    text(language, "healthy", "健康").to_string()
}

pub(super) fn selected_next_action(session: &Session, language: Language) -> String {
    if session.health < 50 {
        return text(
            language,
            "open diagnostics for critical health",
            "打开诊断查看严重健康问题",
        )
        .to_string();
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
        return text(
            language,
            "inspect failed tool results",
            "检查失败的工具结果",
        )
        .to_string();
    }
    if session.metrics.cost_estimated >= 1.0 {
        return text(
            language,
            "compare cost drivers in diff",
            "在对比视图比较成本来源",
        )
        .to_string();
    }
    text(
        language,
        "open detail for full report",
        "打开详情查看完整报告",
    )
    .to_string()
}

pub(super) fn next_action(app: &App) -> String {
    if app.sessions.is_empty() {
        if app.pending_load.is_some() {
            return app.t("wait for loader", "等待加载完成").to_string();
        }
        return app.t("load sessions", "加载会话").to_string();
    }
    if matches!(app.view, View::Detail | View::Diagnostics) {
        if let Some(session) = app.selected_session() {
            return selected_next_action(session, app.language);
        }
    }
    if app.overview.critical > 0 {
        return app.t("open critical sessions", "打开严重会话").to_string();
    }
    if app
        .sessions
        .iter()
        .any(|session| !session.anomalies.is_empty())
    {
        return app.t("review anomalies", "查看异常").to_string();
    }
    if app
        .sessions
        .iter()
        .any(|session| session.metrics.tool_calls_fail > 0)
    {
        return app.t("inspect failed tools", "检查失败工具").to_string();
    }
    app.t("watch cost and latency", "关注成本和延迟")
        .to_string()
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
