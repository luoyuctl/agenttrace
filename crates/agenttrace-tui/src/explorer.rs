use super::*;
use ratatui::widgets::Clear;
use std::fs;

const VIEW_CHOICES: [ExplorerView; 8] = [
    ExplorerView::Attention,
    ExplorerView::Recent,
    ExplorerView::All,
    ExplorerView::Projects,
    ExplorerView::Context,
    ExplorerView::Storage,
    ExplorerView::Cost,
    ExplorerView::Tools,
];

const DETAIL_SECTIONS: [DetailSection; 4] = [
    DetailSection::Summary,
    DetailSection::Timeline,
    DetailSection::Context,
    DetailSection::Files,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExplorerLayout {
    Compact,
    Standard,
    Wide,
}

impl ExplorerLayout {
    fn for_area(area: Rect) -> Self {
        if area.width >= 150 && area.height >= 28 {
            Self::Wide
        } else if area.width >= 100 && area.height >= 22 {
            Self::Standard
        } else {
            Self::Compact
        }
    }
}

impl App {
    pub(super) fn handle_explorer_event(&mut self, event: Event) -> anyhow::Result<bool> {
        if let Event::Paste(text) = event {
            if self.mode == InputMode::Search {
                self.input.push_str(&text.replace(['\r', '\n'], " "));
                self.apply_search_input();
                self.explorer_selected = 0;
            } else if self.explorer_overlay == ExplorerOverlay::Command {
                self.input.push_str(&text.replace(['\r', '\n'], " "));
                self.overlay_selected = 0;
            }
            return Ok(false);
        }
        let Event::Key(key) = event else {
            return Ok(false);
        };
        if key.kind != KeyEventKind::Press {
            return Ok(false);
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Ok(true);
        }
        if self.mode == InputMode::Search {
            let quit = self.handle_search_key(key);
            self.explorer_selected = 0;
            return Ok(quit);
        }
        if self.explorer_overlay != ExplorerOverlay::None {
            return self.handle_explorer_overlay(key);
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('k') {
            self.open_explorer_overlay(ExplorerOverlay::Command);
            return Ok(false);
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('r') {
            self.reload(true)?;
            return Ok(false);
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('d') {
            self.move_explorer(8);
            return Ok(false);
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('u') {
            self.move_explorer(-8);
            return Ok(false);
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(true),
            KeyCode::Char(' ') => self.toggle_compare_anchor(),
            KeyCode::Char('d') if self.compare_anchor.is_some() => {
                self.compare_open = true;
                self.scroll = 0;
            }
            KeyCode::Char('D') => self.compare_with_previous_project_session(),
            KeyCode::Char(':') => self.open_explorer_overlay(ExplorerOverlay::Command),
            KeyCode::Char('/') => {
                self.capture_search_snapshot();
                self.mode = InputMode::Search;
                self.input.clone_from(&self.query);
                self.input_original.clone_from(&self.query);
            }
            KeyCode::Char('v') => self.open_explorer_overlay(ExplorerOverlay::ViewPicker),
            KeyCode::Char('f') => self.open_explorer_overlay(ExplorerOverlay::Filter),
            KeyCode::Char('?') => self.open_explorer_overlay(ExplorerOverlay::Help),
            KeyCode::Char('r') => self.reload(false)?,
            KeyCode::Char('L') => self.toggle_language(),
            KeyCode::Char('l') if self.explorer_detail.is_none() => self.toggle_language(),
            KeyCode::Char('!') => {
                self.filter_critical_sessions();
                self.explorer_selected = 0;
            }
            KeyCode::Char('$') => {
                self.filter_costly_sessions();
                self.explorer_selected = 0;
            }
            KeyCode::Char('s') if self.explorer_view == ExplorerView::Projects => {
                if let Some(project) = self.explorer_session().map(resolve_project) {
                    self.project_filter.clear();
                    self.project_id_filter = project.id;
                    self.refresh_filtered();
                    self.explorer_view = ExplorerView::All;
                    self.explorer_selected = 0;
                    self.status = format!(
                        "{}: {}",
                        self.t("project filter", "项目筛选"),
                        project.display_name
                    );
                }
            }
            KeyCode::Char('s') => {
                self.filter_selected_source();
                self.explorer_selected = 0;
            }
            KeyCode::Char('S') => {
                self.filter_top_driver(DriverKind::Source);
                self.explorer_selected = 0;
            }
            KeyCode::Char('M') => {
                self.filter_top_driver(DriverKind::Model);
                self.explorer_selected = 0;
            }
            KeyCode::Char('R') => self.cycle_range(),
            KeyCode::Char('G') => {
                let count = self.explorer_indices().len();
                if count > 0 {
                    self.explorer_selected = count - 1;
                    self.selected = self.explorer_selected;
                }
            }
            KeyCode::Esc if self.compare_open => {
                self.compare_open = false;
                self.scroll = 0;
            }
            KeyCode::Esc => {
                if self.explorer_detail.take().is_none() && self.has_filters() {
                    self.clear_filters();
                    self.refresh_filtered();
                    self.explorer_selected = 0;
                    self.status = self.t("filter cleared", "已清除筛选").to_string();
                }
            }
            KeyCode::Enter => {
                if self.explorer_detail.is_none() && self.explorer_session().is_some() {
                    self.explorer_detail = Some(DetailSection::Summary);
                    self.scroll = 0;
                }
            }
            KeyCode::Char('j') if self.explorer_detail.is_some() => self.move_detail_session(1),
            KeyCode::Char('k') if self.explorer_detail.is_some() => self.move_detail_session(-1),
            KeyCode::Down if self.explorer_detail.is_some() => {
                self.scroll = self.scroll.saturating_add(1)
            }
            KeyCode::Up if self.explorer_detail.is_some() => {
                self.scroll = self.scroll.saturating_sub(1)
            }
            KeyCode::Down | KeyCode::Char('j') => self.move_explorer(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_explorer(-1),
            KeyCode::PageDown => self.scroll = self.scroll.saturating_add(8),
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(8),
            KeyCode::Right if self.explorer_detail.is_some() => self.move_detail_section(1),
            KeyCode::Left | KeyCode::Char('h') if self.explorer_detail.is_some() => {
                self.move_detail_section(-1)
            }
            KeyCode::Char('l') if self.explorer_detail.is_some() => self.move_detail_section(1),
            _ => {}
        }
        Ok(false)
    }

    fn session_key(session: &Session) -> String {
        format!("{}\n{}", session.path, session.metrics.session_start)
    }

    fn toggle_compare_anchor(&mut self) {
        let Some(session) = self.explorer_session() else {
            return;
        };
        let key = Self::session_key(session);
        if self.compare_anchor.as_deref() == Some(key.as_str()) {
            self.compare_anchor = None;
            self.compare_open = false;
            self.status = self.t("comparison cleared", "已取消对比").to_string();
        } else {
            self.compare_anchor = Some(key);
            self.compare_open = false;
            self.status = self
                .t(
                    "comparison start selected; move to another session and press d",
                    "已选中对比起点；移到另一个会话后按 d",
                )
                .to_string();
        }
    }

    fn compare_with_previous_project_session(&mut self) {
        let Some(current) = self.explorer_session() else {
            return;
        };
        let current_key = Self::session_key(current);
        let Some(current_start) = (!current.metrics.session_start.is_empty())
            .then_some(current.metrics.session_start.as_str())
        else {
            self.status = self
                .t(
                    "This session has no timestamp for a previous-run comparison.",
                    "当前会话没有时间戳，无法寻找上一次会话。",
                )
                .to_string();
            return;
        };
        let project = resolve_project(current).id;
        let anchor = self
            .sessions
            .iter()
            .filter(|session| {
                resolve_project(session).id == project
                    && Self::session_key(session) != current_key
                    && !session.metrics.session_start.is_empty()
                    && session.metrics.session_start.as_str() < current_start
            })
            .max_by(|left, right| {
                left.metrics
                    .session_start
                    .cmp(&right.metrics.session_start)
                    .then_with(|| Self::session_key(left).cmp(&Self::session_key(right)))
            })
            .map(Self::session_key);
        if let Some(anchor) = anchor {
            self.compare_anchor = Some(anchor);
            self.compare_open = true;
            self.scroll = 0;
        } else {
            self.status = self
                .t(
                    "This is the earliest session from this project.",
                    "当前就是这个项目最早的会话。",
                )
                .to_string();
        }
    }

    fn move_detail_session(&mut self, delta: isize) {
        let count = self.explorer_indices().len();
        if count == 0 {
            return;
        }
        self.explorer_selected = self
            .explorer_selected
            .saturating_add_signed(delta)
            .min(count - 1);
        self.selected = self.explorer_selected;
        self.scroll = 0;
    }

    pub(super) fn compare_sessions(&self) -> Option<[Session; 2]> {
        let anchor = self.compare_anchor.as_deref()?;
        let left = self
            .sessions
            .iter()
            .find(|session| Self::session_key(session) == anchor)?
            .clone();
        let right = self.explorer_session()?.clone();
        (Self::session_key(&left) != Self::session_key(&right)).then_some([left, right])
    }

    fn open_explorer_overlay(&mut self, overlay: ExplorerOverlay) {
        self.explorer_overlay = overlay;
        self.overlay_selected = match overlay {
            ExplorerOverlay::ViewPicker => VIEW_CHOICES
                .iter()
                .position(|view| *view == self.explorer_view)
                .unwrap_or(0),
            _ => 0,
        };
        self.input.clear();
    }

    fn handle_explorer_overlay(&mut self, key: KeyEvent) -> anyhow::Result<bool> {
        match key.code {
            KeyCode::Esc => {
                self.explorer_overlay = ExplorerOverlay::None;
                self.input.clear();
            }
            KeyCode::Backspace if self.explorer_overlay == ExplorerOverlay::Command => {
                self.input.pop();
                self.overlay_selected = 0;
            }
            KeyCode::Char(c) if self.explorer_overlay == ExplorerOverlay::Command => {
                self.input.push(c);
                self.overlay_selected = 0;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max = self.overlay_item_count().saturating_sub(1);
                self.overlay_selected = (self.overlay_selected + 1).min(max);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.overlay_selected = self.overlay_selected.saturating_sub(1);
            }
            KeyCode::Char('x') if self.explorer_overlay == ExplorerOverlay::Filter => {
                self.clear_filters();
                self.refresh_filtered();
                self.explorer_selected = 0;
            }
            KeyCode::Enter => self.activate_explorer_overlay()?,
            _ => {}
        }
        Ok(false)
    }

    fn overlay_item_count(&self) -> usize {
        match self.explorer_overlay {
            ExplorerOverlay::ViewPicker => VIEW_CHOICES.len(),
            ExplorerOverlay::Filter => 6,
            ExplorerOverlay::Command => self.command_choices().len(),
            ExplorerOverlay::Help | ExplorerOverlay::None => 1,
        }
    }

    fn activate_explorer_overlay(&mut self) -> anyhow::Result<()> {
        match self.explorer_overlay {
            ExplorerOverlay::ViewPicker => {
                if let Some(view) = VIEW_CHOICES.get(self.overlay_selected) {
                    self.explorer_view = *view;
                    self.explorer_detail = None;
                    self.explorer_selected = 0;
                }
                self.explorer_overlay = ExplorerOverlay::None;
            }
            ExplorerOverlay::Filter => {
                match self.overlay_selected {
                    0 => self.cycle_health_filter(),
                    1 => self.cycle_explorer_source(),
                    2 => self.cycle_explorer_project(),
                    3 => self.cycle_range(),
                    4 => {
                        self.issue_filter = if self.issue_filter == "context" {
                            String::new()
                        } else {
                            "context".to_string()
                        };
                        self.refresh_filtered();
                    }
                    5 => {
                        self.clear_filters();
                        self.refresh_filtered();
                    }
                    _ => {}
                }
                self.explorer_selected = 0;
                self.explorer_overlay = ExplorerOverlay::None;
            }
            ExplorerOverlay::Command => {
                let choices = self.command_choices();
                if let Some((_, command)) = choices.get(self.overlay_selected) {
                    match *command {
                        "view:attention" => self.explorer_view = ExplorerView::Attention,
                        "view:context" => self.explorer_view = ExplorerView::Context,
                        "view:projects" => self.explorer_view = ExplorerView::Projects,
                        "view:storage" => self.explorer_view = ExplorerView::Storage,
                        "view:cost" => self.explorer_view = ExplorerView::Cost,
                        "view:tools" => self.explorer_view = ExplorerView::Tools,
                        "filter:context" => {
                            self.issue_filter = "context".to_string();
                            self.refresh_filtered();
                        }
                        "clear" => {
                            self.clear_filters();
                            self.refresh_filtered();
                        }
                        "language" => self.toggle_language(),
                        "reload" => self.reload(false)?,
                        _ => {}
                    }
                }
                self.explorer_detail = None;
                self.explorer_selected = 0;
                self.explorer_overlay = ExplorerOverlay::None;
                self.input.clear();
            }
            ExplorerOverlay::Help | ExplorerOverlay::None => {
                self.explorer_overlay = ExplorerOverlay::None;
            }
        }
        Ok(())
    }

    fn command_choices(&self) -> Vec<(&'static str, &'static str)> {
        let query = self.input.to_ascii_lowercase();
        i18n::command_choices(self.language)
            .into_iter()
            .filter(|(label, _)| query.is_empty() || label.to_ascii_lowercase().contains(&query))
            .collect()
    }

    pub(super) fn move_explorer(&mut self, delta: isize) {
        if self.explorer_detail.is_some() {
            self.scroll = if delta > 0 {
                self.scroll.saturating_add(delta as u16)
            } else {
                self.scroll.saturating_sub((-delta) as u16)
            };
            return;
        }
        let count = self.explorer_indices().len();
        if count == 0 {
            self.explorer_selected = 0;
        } else {
            self.explorer_selected = self
                .explorer_selected
                .saturating_add_signed(delta)
                .min(count - 1);
            self.selected = self.explorer_selected;
        }
    }

    fn move_detail_section(&mut self, delta: isize) {
        let current = self
            .explorer_detail
            .and_then(|section| DETAIL_SECTIONS.iter().position(|item| *item == section))
            .unwrap_or(0);
        let next = current
            .saturating_add_signed(delta)
            .min(DETAIL_SECTIONS.len() - 1);
        self.explorer_detail = Some(DETAIL_SECTIONS[next]);
        self.scroll = 0;
    }

    pub(super) fn explorer_indices(&self) -> Vec<usize> {
        let mut indices = self.filtered.clone();
        if self.explorer_view == ExplorerView::Storage {
            let mut paths = std::collections::HashSet::new();
            indices.retain(|index| paths.insert(self.sessions[*index].path.clone()));
        }
        match self.explorer_view {
            ExplorerView::Attention => {
                indices.retain(|index| needs_attention(&self.sessions[*index]));
                indices.sort_by_key(|index| attention_rank(&self.sessions[*index]));
            }
            ExplorerView::Projects => {
                indices.sort_by(|a, b| {
                    resolve_project(&self.sessions[*a])
                        .id
                        .cmp(&resolve_project(&self.sessions[*b]).id)
                        .then_with(|| {
                            self.sessions[*b]
                                .metrics
                                .cost_estimated
                                .total_cmp(&self.sessions[*a].metrics.cost_estimated)
                        })
                });
                let mut projects = std::collections::HashSet::new();
                indices.retain(|index| projects.insert(resolve_project(&self.sessions[*index]).id));
            }

            ExplorerView::Context => indices.sort_by(|a, b| {
                self.sessions[*b]
                    .diagnostics
                    .context_utilization
                    .utilization_pct
                    .partial_cmp(
                        &self.sessions[*a]
                            .diagnostics
                            .context_utilization
                            .utilization_pct,
                    )
                    .unwrap_or(Ordering::Equal)
            }),
            ExplorerView::Storage => indices
                .sort_by_key(|index| std::cmp::Reverse(session_file_size(&self.sessions[*index]))),
            ExplorerView::Cost => indices.sort_by(|a, b| {
                self.sessions[*b]
                    .metrics
                    .cost_estimated
                    .partial_cmp(&self.sessions[*a].metrics.cost_estimated)
                    .unwrap_or(Ordering::Equal)
            }),
            ExplorerView::Tools => indices.sort_by_key(|index| {
                std::cmp::Reverse(self.sessions[*index].metrics.tool_calls_fail)
            }),
            ExplorerView::Recent | ExplorerView::All => {}
        }
        if self.explorer_view == ExplorerView::Recent {
            indices.truncate(25);
        }
        indices
    }

    fn cycle_explorer_source(&mut self) {
        let mut values = self
            .sessions
            .iter()
            .map(display_session_source)
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        values.sort();
        values.dedup();
        self.source_filter = cycle_value(&values, &self.source_filter);
        self.refresh_filtered();
    }

    fn cycle_explorer_project(&mut self) {
        let mut values = self
            .sessions
            .iter()
            .map(project_name)
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        values.sort();
        values.dedup();
        self.project_id_filter.clear();
        self.project_filter = cycle_value(&values, &self.project_filter);
        self.refresh_filtered();
    }

    pub(super) fn explorer_session(&self) -> Option<&Session> {
        self.explorer_indices()
            .get(self.explorer_selected)
            .and_then(|index| self.sessions.get(*index))
    }
}

pub(super) fn render_explorer(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();
    if area.width < 64 || area.height < 18 {
        frame.render_widget(
            Paragraph::new(app.t(
                "Window is too small. Make it at least 64x18.",
                "窗口太小了，请调到至少 64×18。",
            )),
            area,
        );
        return;
    }
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(8),
        Constraint::Length(3),
    ])
    .split(area);
    render_explorer_header(frame, app, rows[0]);
    if app.pending_load.is_some() && app.sessions.is_empty() {
        render_loading_status(frame, app, rows[1]);
    } else if app.compare_open {
        render_compare(frame, app, rows[1]);
    } else if let Some(section) = app.explorer_detail {
        render_explorer_detail(frame, app, section, rows[1]);
    } else {
        render_explorer_master(frame, app, rows[1]);
    }
    render_explorer_footer(frame, app, rows[2]);
    render_explorer_overlay(frame, app, area);
}

fn render_compare(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let Some([left, right]) = app.compare_sessions() else {
        frame.render_widget(
            Paragraph::new(app.t(
                "Choose a different session to compare.",
                "请选择另一个会话进行对比。",
            )),
            area,
        );
        return;
    };
    let cost_delta = right.metrics.cost_estimated - left.metrics.cost_estimated;
    let duration_delta = right.metrics.duration_sec - left.metrics.duration_sec;
    let token_delta = total_tokens(&right) - total_tokens(&left);
    let fail_delta = right.metrics.tool_calls_fail as i64 - left.metrics.tool_calls_fail as i64;
    let verdict = if cost_delta > 0.0 && duration_delta > 0.0 {
        app.t(
            "The second session was slower and cost more.",
            "第二个会话更慢，也更贵。",
        )
    } else if cost_delta > 0.0 {
        app.t("The second session cost more.", "第二个会话更贵。")
    } else if duration_delta > 0.0 {
        app.t("The second session was slower.", "第二个会话更慢。")
    } else {
        app.t(
            "The second session was faster or cheaper.",
            "第二个会话更快或更省。",
        )
    };
    let text = format!(
        "{}\n\n{}\n→ {}\n\n{}\n{} {:+.4}\n{} {:+}\n{} {:+}\n{} {:+.1}s\n{} {:+}\n\n{}",
        app.t("Compare sessions", "对比会话"),
        left.name,
        right.name,
        verdict,
        app.t("Cost", "花费"),
        cost_delta,
        app.t("Tokens", "Token"),
        token_delta,
        app.t("Tool failures", "工具失败"),
        fail_delta,
        app.t("Time", "耗时"),
        duration_delta,
        app.t("Health", "健康度"),
        right.health - left.health,
        app.t(
            "Positive numbers mean the second session used more.",
            "正数表示第二个会话用得更多。",
        )
    );
    frame.render_widget(
        Paragraph::new(text)
            .scroll((app.scroll, 0))
            .wrap(Wrap { trim: false }),
        area.inner(ratatui::layout::Margin {
            horizontal: 2,
            vertical: 1,
        }),
    );
}

fn render_explorer_header(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let title = i18n::explorer_view_label(app.explorer_view, app.language);
    let line = Line::from(vec![
        Span::styled(
            "AgentTrace",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("   │   "),
        Span::styled(title, Style::default().fg(Color::Cyan)),
        Span::raw(format!(
            "   │   {}   │   {}",
            range_summary(app),
            load_summary_line(app)
        )),
        Span::styled(
            app.t("   │   / search", "   │   / 搜索"),
            Style::default().fg(Color::Gray),
        ),
    ]);
    frame.render_widget(Paragraph::new(line).block(bottom_rule()), area);
}

fn range_summary(app: &App) -> String {
    let visible = app.visible_sessions();
    let tokens = total_tokens_all(&visible);
    let cost: f64 = visible
        .iter()
        .map(|session| session.metrics.cost_estimated)
        .sum();
    let attention = visible
        .iter()
        .filter(|session| needs_attention(session))
        .count();
    format!(
        "{} · {} {} · {} · {} · {} {}",
        range_label(app.range_filter, app.language),
        visible.len(),
        app.t("sessions", "个会话"),
        format_tokens(tokens),
        format_compact_cost(cost),
        attention,
        app.t("need attention", "个需处理")
    )
}

fn render_explorer_master(frame: &mut Frame<'_>, app: &App, area: Rect) {
    match ExplorerLayout::for_area(area) {
        ExplorerLayout::Compact => render_explorer_list(frame, app, area),
        ExplorerLayout::Standard => {
            let columns =
                Layout::horizontal([Constraint::Percentage(42), Constraint::Percentage(58)])
                    .split(area);
            render_explorer_list(frame, app, columns[0]);
            render_explorer_preview(frame, app, columns[1]);
        }
        ExplorerLayout::Wide => {
            let list_width = (area.width / 3).clamp(48, 68);
            let columns = Layout::horizontal([Constraint::Length(list_width), Constraint::Min(72)])
                .split(area);
            render_explorer_list(frame, app, columns[0]);
            render_explorer_preview(frame, app, columns[1]);
        }
    }
}

fn render_explorer_list(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let indices = app.explorer_indices();
    let visible = area.height.saturating_sub(4) as usize;
    let start = app.explorer_selected.saturating_sub(visible / 2);
    let mut lines = vec![Line::styled(
        explorer_list_title(app),
        Style::default().add_modifier(Modifier::BOLD),
    )];
    lines.push(Line::raw(""));
    for (position, index) in indices.iter().enumerate().skip(start).take(visible) {
        let session = &app.sessions[*index];
        let selected = position == app.explorer_selected;
        let marker = if selected { "›" } else { " " };
        let style = if selected {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        lines.push(Line::styled(
            explorer_row(app, session, marker, area.width),
            style,
        ));
    }
    if indices.is_empty() {
        let message = if app.explorer_view == ExplorerView::Attention && !app.filtered.is_empty() {
            app.t(
                "Nothing needs attention right now.",
                "目前没有需要处理的问题。",
            )
        } else {
            app.t(
                "No sessions match the current filter.",
                "没有会话匹配当前筛选。",
            )
        };
        lines.push(Line::styled(message, Style::default().fg(Color::Gray)));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .block(right_rule())
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_explorer_preview(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let Some(session) = app.explorer_session() else {
        frame.render_widget(
            Paragraph::new(app.t("Nothing selected.", "还没选会话。")),
            area,
        );
        return;
    };
    let inner = area.inner(ratatui::layout::Margin {
        horizontal: 3,
        vertical: 1,
    });
    if app.explorer_view != ExplorerView::Attention
        && app.explorer_view != ExplorerView::Recent
        && app.explorer_view != ExplorerView::All
        && app.explorer_view != ExplorerView::Projects
    {
        let text = match app.explorer_view {
            ExplorerView::Context => context_preview(session, app.language),
            ExplorerView::Storage => storage_preview(session, app.language),
            ExplorerView::Cost => cost_preview(session, app.language),
            ExplorerView::Tools => tools_preview(session, app.language),
            ExplorerView::Attention
            | ExplorerView::Recent
            | ExplorerView::All
            | ExplorerView::Projects => unreachable!(),
        };
        frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), inner);
        return;
    }
    if app.explorer_view == ExplorerView::Projects {
        frame.render_widget(
            Paragraph::new(project_preview(app, session)).wrap(Wrap { trim: false }),
            inner,
        );
        return;
    }
    let metrics = &session.metrics;
    let context = &session.diagnostics.context_utilization;
    let mut lines = vec![
        Line::styled(
            app.t("Why look here", "为什么先看这个"),
            Style::default().fg(Color::Cyan),
        ),
        Line::styled(
            short(&session.name, 56),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::raw(format!(
            "{}  {}    {}  {}",
            app.t("Agent", "来源"),
            display_session_source(session),
            app.t("Project", "项目"),
            project_name(session)
        )),
        Line::raw(format!(
            "{}  {}    {}  {}",
            app.t("Model", "模型"),
            short(&metrics.model_used, 24),
            app.t("Session file", "会话文件"),
            short(&session.path, 28)
        )),
        Line::raw(""),
        Line::from(vec![
            metric_span(
                app.t("Health", "健康"),
                session.health.to_string(),
                health_color(session.health),
            ),
            Span::raw("     "),
            metric_span(
                app.t("Context", "上下文"),
                format!("{:.0}%", context.utilization_pct),
                risk_color(&context.risk_level),
            ),
            Span::raw("     "),
            metric_span(
                app.t("Cost", "花费"),
                format_compact_cost(metrics.cost_estimated),
                Color::White,
            ),
            Span::raw("     "),
            metric_span(
                app.t("Time", "耗时"),
                format_duration(metrics.duration_sec),
                Color::White,
            ),
        ]),
        Line::raw(""),
        Line::styled(
            app.t("What's going on", "现在的问题"),
            Style::default().fg(Color::Cyan),
        ),
        Line::raw(primary_finding(session, app.language)),
        Line::raw(""),
        Line::styled(
            app.t("What we saw", "我们看到了什么"),
            Style::default().fg(Color::Cyan),
        ),
    ];
    for evidence in explorer_evidence(session, app.language).into_iter().take(5) {
        lines.push(Line::raw(format!("• {evidence}")));
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        app.t("What to do", "建议怎么做"),
        Style::default().fg(Color::Cyan),
    ));
    lines.push(Line::raw(explorer_recommendation(session, app.language)));
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn project_preview(app: &App, session: &Session) -> String {
    let identity = resolve_project(session);
    let project = identity.display_name.clone();
    let sessions = app
        .visible_sessions()
        .into_iter()
        .filter(|item| resolve_project(item).id == identity.id)
        .collect::<Vec<_>>();
    let cost: f64 = sessions
        .iter()
        .map(|item| item.metrics.cost_estimated)
        .sum();
    let tokens = total_tokens_all(&sessions);
    let attention = sessions.iter().filter(|item| needs_attention(item)).count();
    let average = if sessions.is_empty() {
        0.0
    } else {
        sessions.iter().map(|item| item.health as f64).sum::<f64>() / sessions.len() as f64
    };
    format!(
        "{}\n{}\n\n{}  {}\n{}  {}\n{}  {}\n{}  {:.0}\n{}  {}\n\n{}",
        app.t("Project summary", "项目概况"),
        project,
        app.t("Sessions", "会话数"),
        sessions.len(),
        app.t("Estimated spend", "估算花费"),
        format_compact_cost(cost),
        app.t("Tokens", "Token"),
        format_tokens(tokens),
        app.t("Average health", "平均健康度"),
        average,
        app.t("Need attention", "需处理"),
        attention,
        app.t(
            "Press s to show only this project's sessions.",
            "按 s 只看这个项目的会话。",
        )
    )
}

fn context_preview(session: &Session, language: Language) -> String {
    let value = &session.diagnostics.context_utilization;
    let params = session
        .diagnostics
        .large_params
        .iter()
        .take(5)
        .map(|item| format!("• {}", item.tool_name))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "{}\n{}\n\n{}         {:.1}%\n{}                {}\n{}     {}\n{}        {}\n{}       {}\n{}    {}\n{}           {}\n\n{}\n{}\n\n{}\n{}",
        text(language, "Context filling up", "上下文快满了"),
        session.name,
        text(language, "used", "已用"),
        value.utilization_pct,
        text(language, "risk", "风险"),
        i18n::risk_label(&value.risk_level, language),
        text(language, "estimated total", "估算总量"),
        format_tokens(value.estimated_total as i64),
        text(language, "conversation", "对话内容"),
        format_tokens(value.conversation_history as i64),
        text(language, "system prompt", "系统提示"),
        format_tokens(value.system_prompt as i64),
        text(language, "tool definitions", "工具定义"),
        format_tokens(value.tool_definitions as i64),
        text(language, "room left", "还能用"),
        format_tokens(value.available_for_task as i64),
        text(language, "What's taking space", "什么在占空间"),
        if params.is_empty() {
            text(language, "No oversized tool arguments showed up.", "没看到特别大的工具参数。")
        } else {
            &params
        },
        text(language, "Did it compact?", "有没有压缩"),
        text(
            language,
            "We didn't see a compaction event from this agent.",
            "这个来源没有记录压缩事件。"
        )
    )
}

fn storage_preview(session: &Session, language: Language) -> String {
    let metadata = fs::metadata(&session.path).ok();
    let size = metadata.as_ref().map(|value| value.len()).unwrap_or(0);
    let modified = metadata
        .and_then(|value| value.modified().ok())
        .map(|value| format!("{value:?}"))
        .unwrap_or_else(|| text(language, "unknown", "未知").to_string());
    format!(
        "{}\n{}\n\n{}  {}\n{}  {}\n\n{}\n{}\n\n{}\n{}\n\n{}\n{}",
        text(language, "On this machine", "在这台电脑上"),
        session.name,
        text(language, "Size", "大小"),
        format_bytes(size),
        text(language, "Last changed", "上次改动"),
        modified,
        text(language, "Session file", "会话文件"),
        session.path,
        text(language, "Workspace", "工作区"),
        session.cwd,
        text(language, "Safe to know", "可以放心"),
        text(
            language,
            "Look or archive it yourself. AgentTrace never deletes these files.",
            "你可以自己查看或归档。AgentTrace 不会删这些文件。"
        )
    )
}

fn cost_preview(session: &Session, language: Language) -> String {
    let audit = session_cost_audit(session);
    let unavailable = text(language, "not available", "不可用");
    let current_cost = audit
        .estimated_cost_usd
        .map(format_compact_cost)
        .unwrap_or_else(|| unavailable.to_string());
    let difference = audit
        .estimated_cost_usd
        .map(|cost| format_compact_cost(cost - audit.stored_estimated_cost_usd))
        .unwrap_or_else(|| unavailable.to_string());
    let component_costs = audit
        .component_cost_usd
        .as_ref()
        .map(|cost| {
            format!(
                "{} {}  {} {}  {} {}  {} {}",
                text(language, "in", "输入"),
                format_compact_cost(cost.input),
                text(language, "out", "输出"),
                format_compact_cost(cost.output),
                text(language, "cache write", "写缓存"),
                format_compact_cost(cost.cache_write),
                text(language, "cache read", "读缓存"),
                format_compact_cost(cost.cache_read)
            )
        })
        .unwrap_or_else(|| unavailable.to_string());
    let rates = audit
        .rates_per_million_usd
        .as_ref()
        .map(|rate| {
            format!(
                "{}  {} ${:.2}  {} ${:.2}  {} ${:.2}  {} ${:.2}",
                text(language, "Price per 1M tokens", "每百万 token 价格"),
                text(language, "in", "输入"),
                rate.input,
                text(language, "out", "输出"),
                rate.output,
                text(language, "cache write", "写缓存"),
                rate.cache_write,
                text(language, "cache read", "读缓存"),
                rate.cache_read
            )
        })
        .unwrap_or_else(|| unavailable.to_string());
    [
        text(language, "Estimated spend", "估算花费").to_string(),
        session.name.clone(),
        String::new(),
        audit
            .estimated_cost_usd
            .map(format_compact_cost)
            .unwrap_or_else(|| format_compact_cost(audit.stored_estimated_cost_usd)),
        String::new(),
        format!(
            "{}       {}",
            text(language, "input tokens", "输入 token"),
            format_tokens(audit.tokens.input)
        ),
        format!(
            "{}      {}",
            text(language, "output tokens", "输出 token"),
            format_tokens(audit.tokens.output)
        ),
        format!(
            "{}        {}",
            text(language, "cache write", "写入缓存"),
            format_tokens(audit.tokens.cache_write)
        ),
        format!(
            "{}         {}",
            text(language, "cache read", "读取缓存"),
            format_tokens(audit.tokens.cache_read)
        ),
        format!(
            "{}     {}",
            text(language, "total counted", "统计总量"),
            format_tokens(audit.tokens.total)
        ),
        String::new(),
        text(language, "Price sources", "价格来源").to_string(),
        format!(
            "{}  {}",
            text(language, "Current rates", "当前价格"),
            audit.pricing_source
        ),
        format!(
            "{}  {}",
            text(language, "Stored estimate", "历史估算"),
            audit.stored_pricing_source
        ),
        format!(
            "{}  {}",
            text(language, "Price status", "价格状态"),
            i18n::pricing_status_label(&audit.pricing_status, language)
        ),
        format!(
            "{}  {}",
            text(language, "How complete the data is", "数据全不全"),
            i18n::capability_label(audit.capability, language)
        ),
        format!(
            "{}  {} / {}",
            text(language, "Source / model", "来源 / 模型"),
            audit.provider,
            audit.model
        ),
        String::new(),
        text(language, "Split by token type", "按 token 类型拆开").to_string(),
        component_costs,
        rates,
        format!(
            "{}  {}",
            text(language, "Note", "说明"),
            text(
                language,
                match audit.pricing_status.as_str() {
                    "catalog_estimate" => "This uses the matching model in our price list.",
                    "fallback_estimate" => "No exact model match, so this is a fallback estimate.",
                    "aggregate_estimate" =>
                        "SQLite combined multiple models, so no single-model exact price applies.",
                    _ => "We don't have a reliable price for this model yet.",
                },
                match audit.pricing_status.as_str() {
                    "catalog_estimate" => "这是按价目表里对上的模型估算的。",
                    "fallback_estimate" => "没有对上确切模型，所以这是兜底估算。",
                    "aggregate_estimate" => "SQLite 聚合了多个模型，不能按单一模型精确计价。",
                    _ => "这个模型暂时没有靠谱的价格。",
                }
            )
        ),
        format!(
            "{}  {}",
            text(language, "Historical stored estimate", "缓存中的历史估算"),
            format_compact_cost(audit.stored_estimated_cost_usd)
        ),
        format!(
            "{}  {}",
            text(language, "Current-rate estimate", "按当前价格重算"),
            current_cost
        ),
        format!("{}  {}", text(language, "Difference", "差异"), difference),
        format!(
            "{}  {}",
            text(language, "Why they differ", "差异原因"),
            audit.pricing_note.clone()
        ),
        String::new(),
        text(
            language,
            "This is a local estimate from token counts, not a bill.",
            "这是按 token 数量做的本地估算，不是账单。",
        )
        .to_string(),
    ]
    .join("\n")
}

fn tools_preview(session: &Session, language: Language) -> String {
    let mut usage = session.metrics.tool_usage.iter().collect::<Vec<_>>();
    usage.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
    let usage = usage
        .into_iter()
        .take(6)
        .map(|(name, count)| format!("{count:>5}  {name}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut latency = session
        .diagnostics
        .tool_latencies
        .iter()
        .collect::<Vec<_>>();
    latency.sort_by(|a, b| b.p95_sec.partial_cmp(&a.p95_sec).unwrap_or(Ordering::Equal));
    let latency = latency
        .into_iter()
        .take(6)
        .map(|item| {
            format!(
                "{:>7}  {}{}",
                format_duration(item.p95_sec),
                item.tool_name,
                if item.timeouts > 0 { "  timeout" } else { "" }
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "{}\n{}\n\n{}       {}\n{}   {}\n{}      {}\n{} {}\n\n{}\n{}\n\n{}\n{}",
        text(language, "Tool trouble", "工具出问题"),
        session.name,
        text(language, "calls", "调用次数"),
        session.metrics.tool_calls_total,
        text(language, "succeeded", "成功"),
        session.metrics.tool_calls_ok,
        text(language, "failed", "失败"),
        session.metrics.tool_calls_fail,
        text(language, "repeat loops", "反复调用"),
        session.diagnostics.loop_cost.loop_groups,
        text(language, "Most used tools", "用得最多的工具"),
        if usage.is_empty() {
            text(language, "No tool calls showed up.", "没看到工具调用。")
        } else {
            &usage
        },
        text(language, "Slowest tools", "最慢的工具"),
        if latency.is_empty() {
            text(language, "No timing samples showed up.", "没看到耗时样本。")
        } else {
            &latency
        }
    )
}

fn render_explorer_detail(frame: &mut Frame<'_>, app: &App, section: DetailSection, area: Rect) {
    let Some(session) = app.explorer_session() else {
        return;
    };
    let header_height = if area.height < 24 { 3 } else { 5 };
    let rows =
        Layout::vertical([Constraint::Length(header_height), Constraint::Min(4)]).split(area);
    let tabs = DETAIL_SECTIONS
        .iter()
        .map(|item| {
            let label = i18n::detail_section_label(*item, app.language);
            if *item == section {
                Span::styled(
                    format!("  {label}  "),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled(format!("  {label}  "), Style::default().fg(Color::Gray))
            }
        })
        .collect::<Vec<_>>();
    let name_width = rows[0].width.saturating_sub(6) as usize;
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(
                format!("←  {}", short(&session.name, name_width)),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Line::from(tabs),
        ])
        .block(bottom_rule()),
        rows[0],
    );

    let content = rows[1].inner(ratatui::layout::Margin {
        horizontal: if rows[1].width < 90 { 1 } else { 2 },
        vertical: 1,
    });
    if ExplorerLayout::for_area(content) == ExplorerLayout::Wide {
        let columns = Layout::horizontal([Constraint::Percentage(68), Constraint::Percentage(32)])
            .split(content);
        render_detail_section(frame, app, session, section, columns[0]);
        render_detail_sidebar(frame, app, session, columns[1]);
    } else {
        render_detail_section(frame, app, session, section, content);
    }
}

fn render_detail_section(
    frame: &mut Frame<'_>,
    app: &App,
    session: &Session,
    section: DetailSection,
    area: Rect,
) {
    if section == DetailSection::Timeline {
        render_timeline_table(frame, app, session, area);
        return;
    }
    let text = match section {
        DetailSection::Summary => detail_summary(session, app.language),
        DetailSection::Context => detail_context(session, app.language),
        DetailSection::Files => detail_files(session, app.language),
        DetailSection::Timeline => unreachable!(),
    };
    frame.render_widget(
        Paragraph::new(text)
            .scroll((app.scroll, 0))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_detail_sidebar(frame: &mut Frame<'_>, app: &App, session: &Session, area: Rect) {
    let audit = session_cost_audit(session);
    let text = vec![
        Line::styled(
            app.t("Session at a glance", "会话概况"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::raw(format!(
            "{}  {}",
            app.t("Source", "来源"),
            display_session_source(session)
        )),
        Line::raw(format!(
            "{}  {}",
            app.t("Model", "模型"),
            session.metrics.model_used
        )),
        Line::raw(format!("{}  {}", app.t("Health", "健康"), session.health)),
        Line::raw(format!(
            "{}  {:.0}% ({})",
            app.t("Context", "上下文"),
            session.diagnostics.context_utilization.utilization_pct,
            i18n::risk_label(
                &session.diagnostics.context_utilization.risk_level,
                app.language,
            )
        )),
        Line::raw(format!(
            "{}  {}",
            app.t("Spend", "花费"),
            format_compact_cost(session.metrics.cost_estimated)
        )),
        Line::raw(format!(
            "{}  {}",
            app.t("Time", "耗时"),
            format_duration(session.metrics.duration_sec)
        )),
        Line::raw(""),
        Line::styled(
            app.t("Data quality", "数据质量"),
            Style::default().fg(Color::Cyan),
        ),
        Line::raw(i18n::capability_label(audit.capability, app.language)),
        Line::raw(i18n::pricing_status_label(
            &audit.pricing_status,
            app.language,
        )),
        Line::raw(format!(
            "{}: {}",
            app.t("Tokens", "Token"),
            i18n::provenance_label(&session.metrics.provenance.tokens, app.language)
        )),
        Line::raw(format!(
            "{}: {}",
            app.t("Time", "耗时"),
            i18n::provenance_label(&session.metrics.provenance.duration, app.language)
        )),
        Line::raw(format!(
            "{}: {}",
            app.t("Tool results", "工具结果"),
            i18n::provenance_label(&session.metrics.provenance.tool_results, app.language)
        )),
        Line::raw(""),
        Line::styled(
            app.t("Workspace", "工作区"),
            Style::default().fg(Color::Cyan),
        ),
        Line::raw(session.cwd.clone()),
        Line::raw(""),
        Line::styled(
            app.t("Session file", "会话文件"),
            Style::default().fg(Color::Cyan),
        ),
        Line::raw(session.path.clone()),
    ];
    frame.render_widget(
        Paragraph::new(text)
            .block(left_rule())
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_explorer_footer(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let text = if app.compare_open {
        app.t(
            "↑/↓ Scroll   Esc Back   Space Clear comparison   ? Help",
            "↑/↓ 滚动   Esc 返回   Space 取消对比   ? 帮助",
        )
        .to_string()
    } else if app.mode == InputMode::Search {
        format!(
            "/ {}   {}/{}",
            app.input,
            app.filtered.len(),
            app.sessions.len()
        )
    } else if app.explorer_detail.is_some() {
        app.t(
            "j/k Session   ←/→ Section   ↑/↓ Scroll   Esc Back   / Search   L Language",
            "j/k 切会话   ←/→ 换分区   ↑/↓ 滚动   Esc 返回   / 搜索   L 切换语言",
        )
        .to_string()
    } else {
        app.t(
            "↑/↓ Select   Enter Open   Space Mark   d Compare   D Previous run   / Search   l Language",
            "↑/↓ 选择   Enter 打开   Space 标记   d 对比   D 同项目上次   / 搜索   l 切换语言",
        )
        .to_string()
    };
    frame.render_widget(Paragraph::new(text).block(top_rule()), area);
}

fn render_explorer_overlay(frame: &mut Frame<'_>, app: &App, area: Rect) {
    if app.explorer_overlay == ExplorerOverlay::None {
        return;
    }
    let rect = centered_rect(
        area,
        64.min(area.width.saturating_sub(4)),
        match app.explorer_overlay {
            ExplorerOverlay::ViewPicker => 20,
            ExplorerOverlay::Filter => 17,
            ExplorerOverlay::Command => 20,
            ExplorerOverlay::Help => 18,
            ExplorerOverlay::None => 0,
        }
        .min(area.height.saturating_sub(4)),
    );
    frame.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    match app.explorer_overlay {
        ExplorerOverlay::ViewPicker => render_view_picker(frame, app, inner),
        ExplorerOverlay::Filter => render_filter_overlay(frame, app, inner),
        ExplorerOverlay::Command => render_command_overlay(frame, app, inner),
        ExplorerOverlay::Help => render_help_overlay(frame, app, inner),
        ExplorerOverlay::None => {}
    }
}

fn render_view_picker(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let mut lines = vec![
        Line::styled(
            app.t("Switch view", "换个视图"),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
    ];
    for (index, view) in VIEW_CHOICES.iter().enumerate() {
        lines.push(overlay_row(
            index == app.overlay_selected,
            i18n::explorer_view_label(*view, app.language),
            i18n::explorer_view_description(*view, app.language),
        ));
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        app.t(
            "↑↓ Select   Enter Open   Esc Close",
            "↑↓ 选择   Enter 打开   Esc 关闭",
        ),
        Style::default().fg(Color::Gray),
    ));
    frame.render_widget(Paragraph::new(lines), area);
}

fn render_filter_overlay(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let any = app.t("Any", "不限");
    let health = if app.health_filter.is_empty() {
        any.to_string()
    } else {
        app.health_filter.clone()
    };
    let rows = [
        (app.t("Health", "健康"), health),
        (
            app.t("Source", "来源"),
            if app.source_filter.is_empty() {
                any.to_string()
            } else {
                app.source_filter.clone()
            },
        ),
        (
            app.t("Project", "项目"),
            if active_project_filter_label(app).is_empty() {
                any.to_string()
            } else {
                active_project_filter_label(app)
            },
        ),
        (
            app.t("When", "时间"),
            range_label(app.range_filter, app.language).to_string(),
        ),
        (
            app.t("Context risk", "上下文风险"),
            if app.issue_filter == "context" {
                app.t("warning and critical", "警告和严重").to_string()
            } else {
                any.to_string()
            },
        ),
        (
            app.t("Reset all", "全部重置"),
            active_filter_summary(app, app.language),
        ),
    ];
    let mut lines = vec![
        Line::styled(
            app.t("Filter sessions", "筛选会话"),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
    ];
    for (index, (label, value)) in rows.iter().enumerate() {
        lines.push(overlay_row(index == app.overlay_selected, label, value));
    }
    lines.push(Line::raw(""));
    lines.push(Line::raw(format!(
        "{} {}",
        app.filtered.len(),
        app.t("matching sessions", "个匹配会话")
    )));
    lines.push(Line::styled(
        app.t(
            "↑↓ Field   Enter Change   x Clear   Esc Close",
            "↑↓ 字段   Enter 修改   x 清除   Esc 关闭",
        ),
        Style::default().fg(Color::Gray),
    ));
    frame.render_widget(Paragraph::new(lines), area);
}

fn render_command_overlay(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let mut lines = vec![
        Line::styled(
            format!("> {}▏", app.input),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
    ];
    for (index, (label, _)) in app.command_choices().iter().enumerate() {
        lines.push(overlay_row(index == app.overlay_selected, label, ""));
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        app.t(
            "↑↓ Select   Enter Run   Esc Close",
            "↑↓ 选择   Enter 执行   Esc 关闭",
        ),
        Style::default().fg(Color::Gray),
    ));
    frame.render_widget(Paragraph::new(lines), area);
}

fn render_help_overlay(frame: &mut Frame<'_>, app: &App, area: Rect) {
    frame.render_widget(
        Paragraph::new(app.t(
            "Keys\n\n↑/↓   Move or scroll\nEnter Open this session\nEsc   Go back or close\n/     Search\nf     Filter\nv     Switch view\nCtrl+K Commands\nr     Reload\nl     Switch language\nq     Quit\n\nPress Esc to close",
            "快捷键\n\n↑/↓   移动或滚动\nEnter 打开这个会话\nEsc   返回或关闭\n/     搜索\nf     筛选\nv     换视图\nCtrl+K 命令\nr     重新加载\nl     切换语言\nq     退出\n\n按 Esc 关闭",
        )),
        area,
    );
}

fn explorer_row(app: &App, session: &Session, marker: &str, width: u16) -> String {
    let name_width = width.saturating_sub(24).clamp(18, 42) as usize;
    let value = match app.explorer_view {
        ExplorerView::Attention => format!(
            "{} P{} {:>3}",
            i18n::inspect_reason_label(inspect_reason(session), app.language),
            attention_priority(session),
            session.health
        ),
        ExplorerView::Projects => project_name(session),
        ExplorerView::Context => format!(
            "{:>5.0}%",
            session.diagnostics.context_utilization.utilization_pct
        ),
        ExplorerView::Storage => format_bytes(session_file_size(session)),
        ExplorerView::Cost => format_compact_cost(session.metrics.cost_estimated),
        ExplorerView::Tools => format!(
            "{} {}",
            session.metrics.tool_calls_fail,
            app.t("failed", "次失败")
        ),
        ExplorerView::Recent | ExplorerView::All => display_session_source(session),
    };
    format!(
        "{marker} {}  {:>10}",
        pad_display_width(&short(&session.name, name_width), name_width),
        value
    )
}

fn explorer_list_title(app: &App) -> String {
    if app.explorer_view == ExplorerView::Attention {
        let indices = app.explorer_indices();
        let urgent = indices
            .iter()
            .filter(|index| attention_priority(&app.sessions[**index]) == 1)
            .count();
        let slow = indices
            .iter()
            .filter(|index| inspect_reason(&app.sessions[**index]) == "latency")
            .count();
        let costly = indices
            .iter()
            .filter(|index| inspect_reason(&app.sessions[**index]) == "cost")
            .count();
        return format!(
            "{} ({}) · {} {} · {} {} · {} {}",
            i18n::explorer_list_title(app.explorer_view, app.language),
            indices.len(),
            urgent,
            app.t("urgent", "紧急"),
            slow,
            app.t("slow", "偏慢"),
            costly,
            app.t("costly", "偏贵")
        );
    }
    format!(
        "{} ({})",
        i18n::explorer_list_title(app.explorer_view, app.language),
        app.explorer_indices().len()
    )
}

fn detail_summary(session: &Session, language: Language) -> String {
    let evidence = explorer_evidence(session, language)
        .into_iter()
        .map(|item| format!("• {item}"))
        .collect::<Vec<_>>()
        .join("\n");
    let audit = session_cost_audit(session);
    let completeness = format!(
        "{} · {} · {}\n{}: {} · {}: {} · {}: {} · {}: {}",
        i18n::capability_label(session_capability(session), language),
        i18n::inspect_reason_label(inspect_reason(session), language),
        i18n::pricing_status_label(&audit.pricing_status, language),
        text(language, "tokens", "Token"),
        i18n::provenance_label(&session.metrics.provenance.tokens, language),
        text(language, "time", "耗时"),
        i18n::provenance_label(&session.metrics.provenance.duration, language),
        text(language, "tool results", "工具结果"),
        i18n::provenance_label(&session.metrics.provenance.tool_results, language),
        text(language, "cost", "成本"),
        i18n::provenance_label(&session.metrics.provenance.cost, language)
    );
    format!(
        "{}\n{}\n\n{}\n{}={}  {}={:.0}%  {}={}  {}={}\n{}\n\n{}\n{}\n\n{}\n{}\n\n{}\n{}",
        text(language, "What's going on", "现在的问题"),
        primary_finding(session, language),
        text(language, "Numbers", "数字"),
        text(language, "health", "健康"),
        session.health,
        text(language, "context", "上下文"),
        session.diagnostics.context_utilization.utilization_pct,
        text(language, "cost", "花费"),
        format_compact_cost(session.metrics.cost_estimated),
        text(language, "time", "耗时"),
        format_duration(session.metrics.duration_sec),
        health_explanation(session, language),
        text(language, "What we saw", "我们看到了什么"),
        evidence,
        text(language, "What to do", "建议怎么做"),
        explorer_recommendation(session, language),
        text(language, "How complete this is", "信息全不全"),
        completeness
    )
}

fn render_timeline_table(frame: &mut Frame<'_>, app: &App, session: &Session, area: Rect) {
    if session.diagnostics.steps.is_empty() {
        frame.render_widget(
            Paragraph::new(detail_timeline_empty(session, app.language))
                .scroll((app.scroll, 0))
                .wrap(Wrap { trim: false }),
            area,
        );
        return;
    }

    let compact = area.width < 90;
    let time_width = if area.width >= 130 { 25 } else { 19 };
    let kind_width = if compact { 10 } else { 14 };
    let status_width = if compact { 8 } else { 12 };
    let fixed = time_width + kind_width + status_width + 12;
    let name_width = area.width.saturating_sub(fixed).max(18) as usize;
    let constraints = if compact {
        vec![
            Constraint::Length(kind_width),
            Constraint::Min(18),
            Constraint::Length(8),
            Constraint::Length(status_width),
        ]
    } else {
        vec![
            Constraint::Length(time_width),
            Constraint::Length(kind_width),
            Constraint::Min(18),
            Constraint::Length(8),
            Constraint::Length(status_width),
        ]
    };
    let rows = session
        .diagnostics
        .steps
        .iter()
        .skip(app.scroll as usize)
        .take(area.height.saturating_sub(4) as usize)
        .map(|step| {
            let name = short(&step.name, name_width);
            if compact {
                Row::new(vec![
                    Cell::from(step.kind.clone()),
                    Cell::from(name),
                    Cell::from(format_duration(step.duration_sec)),
                    Cell::from(step.status.clone()),
                ])
            } else {
                Row::new(vec![
                    Cell::from(short(&step.started_at, time_width as usize)),
                    Cell::from(step.kind.clone()),
                    Cell::from(name),
                    Cell::from(format_duration(step.duration_sec)),
                    Cell::from(step.status.clone()),
                ])
            }
        });
    let header = if compact {
        Row::new(vec![
            app.t("Type", "类型"),
            app.t("Step", "步骤"),
            app.t("Time", "耗时"),
            app.t("Result", "结果"),
        ])
    } else {
        Row::new(vec![
            app.t("Started", "开始时间"),
            app.t("Type", "类型"),
            app.t("Step", "步骤"),
            app.t("Time", "耗时"),
            app.t("Result", "结果"),
        ])
    }
    .style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )
    .bottom_margin(1);
    let title = format!(
        "{} · {} {}",
        app.t("What happened", "发生了什么"),
        session.diagnostics.steps.len(),
        app.t("steps", "个步骤")
    );
    frame.render_widget(
        Table::new(rows, constraints)
            .header(header)
            .column_spacing(2)
            .block(Block::default().title(title).borders(Borders::BOTTOM)),
        area,
    );
}

fn detail_timeline_empty(session: &Session, language: Language) -> String {
    let mut lines = vec![
        text(language, "What happened", "发生了什么").to_string(),
        String::new(),
        text(
            language,
            "This session didn't record a step-by-step timeline.",
            "这个会话没有记下逐步时间线。",
        )
        .to_string(),
    ];
    for anomaly in &session.anomalies {
        lines.push(format!("• [{}] {}", anomaly.severity, anomaly.detail));
    }
    lines.push(String::new());
    lines.push(
        text(
            language,
            "We didn't see a compaction event from this agent.",
            "这个来源没有记录压缩事件。",
        )
        .to_string(),
    );
    lines.join("\n")
}

fn detail_context(session: &Session, language: Language) -> String {
    let value = &session.diagnostics.context_utilization;
    let params = session
        .diagnostics
        .large_params
        .iter()
        .take(10)
        .map(|item| format!("• {}", item.tool_name))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "{}\n\n{}       {}\n{}  {}\n{}         {}\n{}      {}\n{}    {}\n{}           {:.1}%\n{}                  {}\n\n{}\n{}\n\n{}\n{}\n\n{}",
        text(language, "Context", "上下文"),
        text(language, "estimated total", "估算总量"),
        format_tokens(value.estimated_total as i64),
        text(language, "conversation history", "对话内容"),
        format_tokens(value.conversation_history as i64),
        text(language, "system prompt", "系统提示"),
        format_tokens(value.system_prompt as i64),
        text(language, "tool definitions", "工具定义"),
        format_tokens(value.tool_definitions as i64),
        text(language, "room left", "还能用"),
        format_tokens(value.available_for_task as i64),
        text(language, "used", "已用"),
        value.utilization_pct,
        text(language, "risk", "风险"),
        i18n::risk_label(&value.risk_level, language),
        text(language, "What's taking space", "什么在占空间"),
        if params.is_empty() {
            text(language, "none observed", "没看到").to_string()
        } else {
            params
        },
        text(language, "Did it compact?", "有没有压缩"),
        text(
            language,
            "We didn't see a compaction event from this agent.",
            "这个来源没有记录压缩事件。",
        ),
        value.suggestion
    )
}

fn health_explanation(session: &Session, language: Language) -> String {
    let mut parts = Vec::new();
    if session.metrics.tool_calls_fail > 0 {
        parts.push(format!(
            "{} {}",
            session.metrics.tool_calls_fail,
            text(language, "tool failures", "次工具失败")
        ));
    }
    if matches!(
        session.diagnostics.context_utilization.risk_level.as_str(),
        "warning" | "critical"
    ) {
        parts.push(format!(
            "{} {:.0}%",
            text(language, "context", "上下文"),
            session.diagnostics.context_utilization.utilization_pct
        ));
    }
    if session.diagnostics.loop_cost.loop_groups > 0 {
        parts.push(format!(
            "{} {}",
            session.diagnostics.loop_cost.loop_groups,
            text(language, "repeat loops", "组反复调用")
        ));
    }
    if !session.anomalies.is_empty() {
        parts.push(format!(
            "{} {}",
            session.anomalies.len(),
            text(language, "unusual signals", "个异常信号")
        ));
    }
    if parts.is_empty() {
        text(
            language,
            "Health: no clear penalty found",
            "健康度：没有明显扣分项",
        )
        .to_string()
    } else {
        format!(
            "{}: {}",
            text(language, "Health is affected by", "健康度受这些因素影响"),
            parts.join(", ")
        )
    }
}

fn detail_files(session: &Session, language: Language) -> String {
    let size = session_file_size(session);
    let metadata = fs::metadata(&session.path).ok();
    let modified = metadata
        .and_then(|value| value.modified().ok())
        .map(|value| format!("{value:?}"))
        .unwrap_or_else(|| text(language, "unknown", "未知").to_string());
    let mut files = session.metrics.file_usage.iter().collect::<Vec<_>>();
    files.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
    let accessed = files
        .into_iter()
        .take(30)
        .map(|(path, count)| format!("{count:>4}  {path}"))
        .collect::<Vec<_>>()
        .join("\n");
    let repeated = session
        .metrics
        .file_usage
        .iter()
        .filter(|(_, count)| **count >= 3)
        .take(10)
        .map(|(path, count)| format!("{count:>4}  {path}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "{}\n\n{}\n{}\n\n{}  {}\n{}  {}\n\n{}\n{}\n\n{}\n{}\n\n{}",
        text(language, "Session file", "会话文件"),
        session.path,
        session.cwd,
        text(language, "Size", "大小"),
        format_bytes(size),
        text(language, "Last changed", "上次改动"),
        modified,
        text(language, "Files it touched", "碰过的文件"),
        if accessed.is_empty() {
            text(language, "none observed", "没看到").to_string()
        } else {
            accessed
        },
        text(
            language,
            "Possible repeated reads",
            "可能重复读取的文件",
        ),
        if repeated.is_empty() {
            text(
                language,
                "No file appeared 3 or more times.",
                "没有文件出现 3 次以上。",
            )
            .to_string()
        } else {
            repeated
        },
        text(
            language,
            "This count comes from file paths in tool arguments; it may include reads, writes, or searches.",
            "这里统计的是工具参数里的文件路径，可能包含读取、写入或搜索。"
        )
    )
}

fn primary_finding(session: &Session, language: Language) -> String {
    match inspect_reason(session) {
        "critical" => format!(
            "{} ({})",
            text(
                language,
                "This session looks unhealthy",
                "这个会话看起来不健康"
            ),
            session.health
        ),
        "anomaly" => session
            .anomalies
            .first()
            .map(|anomaly| anomaly.detail.clone())
            .unwrap_or_else(|| {
                text(language, "Something unusual showed up", "出现了异常").to_string()
            }),
        "failures" => format!(
            "{}: {}",
            text(language, "Tools failed", "工具失败了"),
            session.metrics.tool_calls_fail
        ),
        "context" => format!(
            "{} ({:.0}%)",
            text(language, "Context is nearly full", "上下文快满了"),
            session.diagnostics.context_utilization.utilization_pct
        ),
        "loops" => format!(
            "{}: {}",
            text(language, "Repeated tool loop", "工具反复调用"),
            session.diagnostics.loop_cost.loop_groups
        ),
        "latency" => format!(
            "{} {}",
            text(language, "This run was slow", "这次跑得偏慢"),
            format_duration(session.metrics.duration_sec)
        ),
        "cost" => format!(
            "{} {}",
            text(
                language,
                "This was the most expensive session",
                "这是最贵的会话"
            ),
            format_compact_cost(session.metrics.cost_estimated)
        ),
        "warning" => text(
            language,
            "This session needs a closer look",
            "这个会话需要进一步看看",
        )
        .to_string(),
        _ => text(
            language,
            "Nothing urgent jumped out.",
            "没有特别紧急的问题。",
        )
        .to_string(),
    }
}

fn explorer_evidence(session: &Session, language: Language) -> Vec<String> {
    let mut evidence = Vec::new();
    let context = &session.diagnostics.context_utilization;
    if context.utilization_pct > 0.0 {
        evidence.push(format!(
            "{} {:.1}% ({})",
            text(language, "Context", "上下文"),
            context.utilization_pct,
            i18n::risk_label(&context.risk_level, language)
        ));
    }
    if session.metrics.tool_calls_fail > 0 {
        evidence.push(format!(
            "{} / {} {}",
            session.metrics.tool_calls_fail,
            session.metrics.tool_calls_total,
            text(language, "tool calls failed", "次工具调用失败")
        ));
    }
    if session.diagnostics.loop_cost.loop_groups > 0 {
        evidence.push(format!(
            "{} {}",
            session.diagnostics.loop_cost.loop_groups,
            text(language, "repeat loops", "组反复调用")
        ));
    }
    for anomaly in session.anomalies.iter().take(3) {
        evidence.push(anomaly.detail.clone());
    }
    if evidence.is_empty() {
        evidence.push(
            text(
                language,
                "There's not much extra detail for this session.",
                "这个会话没有更多细节。",
            )
            .to_string(),
        );
    }
    evidence
}

fn explorer_recommendation(session: &Session, language: Language) -> String {
    let context = &session.diagnostics.context_utilization;
    if !context.suggestion.trim().is_empty() {
        return context.suggestion.clone();
    }
    if session.metrics.tool_calls_fail > 0 {
        return text(
            language,
            "Check the failed tool calls before trying again.",
            "再试之前，先看看失败的工具调用。",
        )
        .to_string();
    }
    text(
        language,
        "Open What happened and check the recorded steps.",
        "打开「发生了什么」，核对记下的步骤。",
    )
    .to_string()
}

fn session_file_size(session: &Session) -> u64 {
    fs::metadata(&session.path)
        .map(|value| value.len())
        .unwrap_or(0)
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let value = bytes as f64;
    if value >= GB {
        format!("{:.1} GB", value / GB)
    } else if value >= MB {
        format!("{:.1} MB", value / MB)
    } else if value >= KB {
        format!("{:.1} KB", value / KB)
    } else {
        format!("{bytes} B")
    }
}

fn metric_span(label: &str, value: String, color: Color) -> Span<'static> {
    Span::styled(
        format!("{label} {value}"),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )
}

fn risk_color(risk: &str) -> Color {
    match risk {
        "critical" => Color::LightRed,
        "warning" => Color::Yellow,
        _ => Color::Cyan,
    }
}

fn overlay_row(selected: bool, label: &str, description: &str) -> Line<'static> {
    let style = if selected {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    Line::styled(
        format!(
            "{} {:<20}  {}",
            if selected { "›" } else { " " },
            label,
            description
        ),
        style,
    )
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    )
}

fn cycle_value(values: &[String], current: &str) -> String {
    values
        .iter()
        .position(|value| value.eq_ignore_ascii_case(current))
        .and_then(|index| values.get(index + 1).cloned())
        .or_else(|| values.first().cloned().filter(|_| current.is_empty()))
        .unwrap_or_default()
}

fn bottom_rule() -> Block<'static> {
    Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Color::DarkGray))
}

fn top_rule() -> Block<'static> {
    Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::DarkGray))
}

fn right_rule() -> Block<'static> {
    Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(Color::DarkGray))
}

fn left_rule() -> Block<'static> {
    Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(Color::DarkGray))
}
