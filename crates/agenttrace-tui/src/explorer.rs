use super::*;
use ratatui::widgets::Clear;
use std::fs;
use std::path::Path;

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
        } else if area.width >= 100 && area.height >= 16 {
            Self::Standard
        } else {
            Self::Compact
        }
    }
}

impl App {
    pub(super) fn expire_notice(&mut self) -> bool {
        if self
            .notice
            .as_ref()
            .and_then(|(_, expiry)| *expiry)
            .is_some_and(|expiry| Instant::now() >= expiry)
        {
            self.notice = None;
            return true;
        }
        false
    }

    pub(super) fn handle_explorer_event(&mut self, event: Event) -> anyhow::Result<bool> {
        if let Event::Paste(text) = event {
            if self.mode == InputMode::Search {
                self.input.push_str(&text.replace(['\r', '\n'], " "));
                self.apply_search_input();
                self.explorer_selected = 0;
            } else if self.searchable_overlay() {
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
            KeyCode::Char('e') if self.explorer_detail == Some(DetailSection::Summary) => {
                self.raw_report_expanded = !self.raw_report_expanded;
                self.scroll = 0;
            }
            KeyCode::Char('y') if self.explorer_detail.is_some() => {
                if let Some(session) = self.explorer_session() {
                    let summary = share_summary(session);
                    self.notice = Some(match copy_summary(&summary) {
                        Ok(()) => (
                            self.t("tui.summary_copied").to_string(),
                            Some(Instant::now() + Duration::from_secs(2)),
                        ),
                        Err(error) => (format!("{}: {error}", self.t("tui.copy_failed")), None),
                    });
                }
            }
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
                    self.status =
                        format!("{}: {}", self.t("tui.project_filter"), project.display_name);
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
            KeyCode::Esc if self.notice.is_some() => {
                self.notice = None;
            }
            KeyCode::Esc => {
                if self.explorer_detail.take().is_none() && self.has_filters() {
                    self.clear_filters();
                    self.refresh_filtered();
                    self.explorer_selected = 0;
                    self.status = self.t("tui.filter_cleared").to_string();
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
            self.status = self.t("tui.comparison_cleared").to_string();
        } else {
            self.compare_anchor = Some(key);
            self.compare_open = false;
            self.status = self
                .t("tui.comparison_start_selected_move_to_another_sessio")
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
                .t("tui.this_session_has_no_timestamp_for_a")
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
                .t("tui.this_is_the_earliest_session_from_this")
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

    fn searchable_overlay(&self) -> bool {
        matches!(
            self.explorer_overlay,
            ExplorerOverlay::Command
                | ExplorerOverlay::ProjectPicker
                | ExplorerOverlay::SourcePicker
        )
    }

    fn filter_choices(&self) -> Vec<(String, String)> {
        let mut values = BTreeMap::new();
        for session in &self.sessions {
            let (id, label) = if self.explorer_overlay == ExplorerOverlay::ProjectPicker {
                let project = resolve_project(session);
                (project.id, project.display_name)
            } else {
                let source = session.metrics.source_tool.clone();
                (source.clone(), source)
            };
            if !id.is_empty() {
                values.insert(id, label);
            }
        }
        let query = self.input.to_lowercase();
        std::iter::once((String::new(), self.t("tui.any").to_string()))
            .chain(values)
            .filter(|(id, label)| format!("{label} {id}").to_lowercase().contains(&query))
            .collect()
    }

    fn handle_explorer_overlay(&mut self, key: KeyEvent) -> anyhow::Result<bool> {
        match key.code {
            KeyCode::Esc => {
                self.explorer_overlay = if matches!(
                    self.explorer_overlay,
                    ExplorerOverlay::ProjectPicker | ExplorerOverlay::SourcePicker
                ) {
                    ExplorerOverlay::Filter
                } else {
                    ExplorerOverlay::None
                };
                self.input.clear();
                self.overlay_selected = 0;
            }
            KeyCode::Backspace if self.searchable_overlay() => {
                self.input.pop();
                self.overlay_selected = 0;
            }
            KeyCode::Char(c) if self.searchable_overlay() => {
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
            ExplorerOverlay::ProjectPicker | ExplorerOverlay::SourcePicker => {
                self.filter_choices().len()
            }
            ExplorerOverlay::Help | ExplorerOverlay::None => 1,
        }
    }

    fn activate_explorer_overlay(&mut self) -> anyhow::Result<()> {
        match self.explorer_overlay {
            ExplorerOverlay::ProjectPicker | ExplorerOverlay::SourcePicker => {
                if let Some((id, _)) = self.filter_choices().get(self.overlay_selected) {
                    if self.explorer_overlay == ExplorerOverlay::ProjectPicker {
                        self.project_filter.clear();
                        self.project_id_filter.clone_from(id);
                    } else {
                        self.source_filter.clone_from(id);
                    }
                    self.refresh_filtered();
                    self.explorer_selected = 0;
                    self.open_explorer_overlay(ExplorerOverlay::Filter);
                }
            }
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
                    1 => {
                        self.open_explorer_overlay(ExplorerOverlay::SourcePicker);
                        return Ok(());
                    }
                    2 => {
                        self.open_explorer_overlay(ExplorerOverlay::ProjectPicker);
                        return Ok(());
                    }
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

    pub(super) fn project_totals(&self) -> std::collections::HashMap<String, ProjectTotals> {
        let mut totals: std::collections::HashMap<String, ProjectTotals> =
            std::collections::HashMap::new();
        let mut names: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for index in &self.filtered {
            let session = &self.sessions[*index];
            let identity = resolve_project(session);
            let entry = totals.entry(identity.id.clone()).or_insert_with(|| {
                *names.entry(identity.display_name.clone()).or_insert(0) += 1;
                ProjectTotals {
                    label: identity.display_name.clone(),
                    root: identity.root.clone(),
                    ..ProjectTotals::default()
                }
            });
            entry.count += 1;
            entry.cost += session.metrics.cost_estimated;
        }
        for entry in totals.values_mut() {
            if names.get(&entry.label).copied().unwrap_or(0) > 1 {
                if let Some(parent) = Path::new(&entry.root)
                    .parent()
                    .and_then(Path::file_name)
                    .and_then(|name| name.to_str())
                {
                    entry.label = format!("{parent}/{}", entry.label);
                }
            }
        }
        totals
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
                let mut projects: std::collections::HashMap<String, (usize, f64)> =
                    std::collections::HashMap::new();
                for index in &indices {
                    let session = &self.sessions[*index];
                    let entry = projects
                        .entry(resolve_project(session).id)
                        .or_insert((*index, 0.0));
                    entry.1 += session.metrics.cost_estimated;
                    if session.metrics.session_start > self.sessions[entry.0].metrics.session_start
                    {
                        entry.0 = *index;
                    }
                }
                let mut grouped = projects.into_iter().collect::<Vec<_>>();
                grouped.sort_by(|(a_id, a), (b_id, b)| {
                    (a_id == "unknown")
                        .cmp(&(b_id == "unknown"))
                        .then_with(|| b.1.total_cmp(&a.1))
                        .then_with(|| a_id.cmp(b_id))
                });
                indices = grouped.into_iter().map(|(_, (index, _))| index).collect();
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
            Paragraph::new(app.t("tui.window_is_too_small_make_it_at")),
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
    if app.pending_load.is_some() && !app.load_state.showing_cached {
        shared::render_loading_status(frame, app, rows[1]);
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
            Paragraph::new(app.t("tui.choose_a_different_session_to_compare")),
            area,
        );
        return;
    };
    let cost_delta = right.metrics.cost_estimated - left.metrics.cost_estimated;
    let duration_delta = right.metrics.duration_sec - left.metrics.duration_sec;
    let token_delta = total_tokens(&right) - total_tokens(&left);
    let fail_delta = right.metrics.tool_calls_fail as i64 - left.metrics.tool_calls_fail as i64;
    let verdict = if cost_delta > 0.0 && duration_delta > 0.0 {
        app.t("tui.the_second_session_was_slower_and_cost")
    } else if cost_delta > 0.0 {
        app.t("tui.the_second_session_cost_more")
    } else if duration_delta > 0.0 {
        app.t("tui.the_second_session_was_slower")
    } else {
        app.t("tui.the_second_session_was_faster_or_cheaper")
    };
    let text = format!(
        "{}\n\n{}\n→ {}\n\n{}\n{} {:+.4}\n{} {:+}\n{} {:+}\n{} {:+.1}s\n{} {:+}\n\n{}",
        app.t("tui.compare_sessions"),
        left.name,
        right.name,
        verdict,
        app.t("tui.cost_4"),
        cost_delta,
        app.t("tui.tokens_2"),
        token_delta,
        app.t("tui.tool_failures_2"),
        fail_delta,
        app.t("tui.time_2"),
        duration_delta,
        app.t("tui.health"),
        right.health - left.health,
        app.t("tui.positive_numbers_mean_the_second_session_used",)
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
    let filters = if app.has_filters() {
        format!(
            "{}/{} · [{}] · {}",
            app.filtered.len(),
            app.sessions.len(),
            active_filter_summary(app, app.language),
            app.t("tui.f_edit")
        )
    } else {
        String::new()
    };
    let filter_line = Line::styled(
        short(&filters, area.width as usize),
        Style::default().fg(Color::Yellow),
    );
    if area.width < 140 {
        let visible = app.visible_sessions();
        let summary = format!(
            " · {} · {} {} · {}",
            range_label(app.range_filter, app.language),
            visible.len(),
            app.t("tui.sessions"),
            format_compact_cost(visible.iter().map(|item| item.metrics.cost_estimated).sum())
        );
        let used = 13 + unicode_width::UnicodeWidthStr::width(title);
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(vec![
                    Span::styled(
                        "AgentTrace",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" · "),
                    Span::styled(title, Style::default().fg(Color::Cyan)),
                    Span::styled(
                        short(&summary, (area.width as usize).saturating_sub(used)),
                        Style::default().fg(Color::Gray),
                    ),
                ]),
                filter_line,
            ])
            .block(bottom_rule()),
            area,
        );
        return;
    }
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
            shared::load_summary_line(app)
        )),
        Span::styled(app.t("tui.search"), Style::default().fg(Color::Gray)),
    ]);
    frame.render_widget(
        Paragraph::new(vec![line, filter_line]).block(bottom_rule()),
        area,
    );
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
        app.t("tui.sessions"),
        format_tokens(tokens),
        format_compact_cost(cost),
        attention,
        app.t("tui.need_attention")
    )
}

fn render_explorer_master(frame: &mut Frame<'_>, app: &App, area: Rect) {
    match ExplorerLayout::for_area(area) {
        ExplorerLayout::Compact => render_explorer_list(frame, app, area),
        ExplorerLayout::Standard => {
            let columns =
                Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
                    .split(area);
            render_explorer_list(frame, app, columns[0]);
            render_explorer_preview(frame, app, columns[1]);
        }
        ExplorerLayout::Wide => {
            let list_width = (area.width * 9 / 20).clamp(56, 96);
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
        short(
            &explorer_list_title(app),
            area.width.saturating_sub(2) as usize,
        ),
        Style::default().add_modifier(Modifier::BOLD),
    )];
    lines.push(Line::raw(""));
    let project_totals =
        (app.explorer_view == ExplorerView::Projects).then(|| app.project_totals());
    for (position, index) in indices.iter().enumerate().skip(start).take(visible) {
        let session = &app.sessions[*index];
        let selected = position == app.explorer_selected;
        let marker = if selected { "›" } else { " " };
        let style = if selected {
            Style::default()
                .fg(Color::Black)
                .bg(
                    if app.explorer_overlay == ExplorerOverlay::None
                        && app.mode == InputMode::Normal
                    {
                        Color::Cyan
                    } else {
                        Color::DarkGray
                    },
                )
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let row = explorer_row_spans(app, session, marker, area.width, project_totals.as_ref());
        lines.push(if selected {
            Line::styled(
                row.into_iter()
                    .map(|span| span.content.into_owned())
                    .collect::<String>(),
                style,
            )
        } else {
            Line::from(row)
        });
    }
    if indices.is_empty() {
        let message = if app.explorer_view == ExplorerView::Attention && !app.filtered.is_empty() {
            app.t("tui.nothing_needs_attention_right_now")
        } else {
            app.t("tui.no_sessions_match_the_current_filter")
        };
        lines.push(Line::styled(message, Style::default().fg(Color::Gray)));
    }
    let divider = if ExplorerLayout::for_area(frame.area()) == ExplorerLayout::Compact {
        Block::default()
    } else {
        right_rule()
    };
    frame.render_widget(
        Paragraph::new(lines).block(divider.border_style(Style::default().fg(
            if app.explorer_overlay == ExplorerOverlay::None && app.mode == InputMode::Normal {
                Color::Cyan
            } else {
                Color::DarkGray
            },
        ))),
        area,
    );
}

fn render_explorer_preview(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let Some(session) = app.explorer_session() else {
        frame.render_widget(Paragraph::new(app.t("tui.nothing_selected")), area);
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
        Line::styled(app.t("tui.why_look_here"), Style::default().fg(Color::Cyan)),
        Line::styled(
            short(&session.name, 56),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        preview_field(app.t("tui.agent"), display_session_source(session)),
        preview_field(app.t("tui.project"), project_name(session)),
        preview_field(app.t("tui.model"), metrics.model_used.clone()),
        Line::raw(""),
        Line::from(vec![
            metric_span(
                app.t("tui.health_3"),
                session.health.to_string(),
                health_color(session.health),
            ),
            Span::raw("     "),
            metric_span(
                app.t("tui.context_2"),
                format_context_pct(context.utilization_pct),
                risk_color(&context.risk_level),
            ),
            Span::raw("     "),
            metric_span(
                app.t("tui.cost_4"),
                format_compact_cost(metrics.cost_estimated),
                Color::White,
            ),
            Span::raw("     "),
            metric_span(
                app.t("tui.time_2"),
                format_duration(metrics.duration_sec),
                Color::White,
            ),
        ]),
        Line::raw(""),
        Line::styled(
            app.t("tui.what_s_going_on"),
            Style::default().fg(Color::Cyan),
        ),
        Line::raw(primary_finding(session, app.language)),
        Line::raw(""),
        Line::styled(app.t("tui.what_we_saw"), Style::default().fg(Color::Cyan)),
    ];
    for evidence in explorer_evidence(session, app.language).into_iter().take(5) {
        lines.push(Line::raw(format!("• {evidence}")));
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        app.t("tui.what_to_do"),
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
        app.t("tui.project_summary"),
        project,
        app.t("tui.sessions_2"),
        sessions.len(),
        app.t("tui.estimated_spend"),
        format_compact_cost(cost),
        app.t("tui.tokens_2"),
        format_tokens(tokens),
        app.t("tui.average_health"),
        average,
        app.t("tui.need_attention_2"),
        attention,
        app.t("tui.press_s_to_show_only_this_project",)
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
        "{}\n{}\n\n{}         {}\n{}                {}\n{}     {}\n{}        {}\n{}       {}\n{}    {}\n{}           {}\n\n{}\n{}\n\n{}\n{}",
        text(language, "tui.context_filling_up"),
        session.name,
        text(language, "tui.used"),
        format_context_pct(value.utilization_pct),
        text(language, "tui.risk"),
        i18n::risk_label(&value.risk_level, language),
        text(language, "tui.estimated_total"),
        format_tokens(value.estimated_total as i64),
        text(language, "tui.conversation"),
        format_tokens(value.conversation_history as i64),
        text(language, "tui.system_prompt"),
        format_tokens(value.system_prompt as i64),
        text(language, "tui.tool_definitions"),
        format_tokens(value.tool_definitions as i64),
        text(language, "tui.room_left"),
        format_tokens(value.available_for_task as i64),
        text(language, "tui.what_s_taking_space"),
        if params.is_empty() {
            text(language, "tui.no_oversized_tool_arguments_showed_up")
        } else {
            &params
        },
        text(language, "tui.did_it_compact"),
        text(
            language, "tui.we_didn_t_see_a_compaction_event")
    )
}

fn storage_preview(session: &Session, language: Language) -> String {
    let metadata = fs::metadata(&session.path).ok();
    let size = metadata.as_ref().map(|value| value.len()).unwrap_or(0);
    let modified = metadata
        .and_then(|value| value.modified().ok())
        .map(|value| format!("{value:?}"))
        .unwrap_or_else(|| text(language, "tui.unknown").to_string());
    format!(
        "{}\n{}\n\n{}  {}\n{}  {}\n\n{}\n{}\n\n{}\n{}\n\n{}\n{}",
        text(language, "tui.on_this_machine"),
        session.name,
        text(language, "tui.size"),
        format_bytes(size),
        text(language, "tui.last_changed"),
        modified,
        text(language, "tui.session_file"),
        session.path,
        text(language, "tui.workspace"),
        session.cwd,
        text(language, "tui.safe_to_know"),
        text(language, "tui.look_or_archive_it_yourself_agenttrace_never")
    )
}

fn cost_preview(session: &Session, language: Language) -> String {
    let audit = session_cost_audit(session);
    let unavailable = text(language, "tui.not_available");
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
                text(language, "tui.in"),
                format_compact_cost(cost.input),
                text(language, "tui.out"),
                format_compact_cost(cost.output),
                text(language, "tui.cache_write"),
                format_compact_cost(cost.cache_write),
                text(language, "tui.cache_read"),
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
                text(language, "tui.price_per_1m_tokens"),
                text(language, "tui.in"),
                rate.input,
                text(language, "tui.out"),
                rate.output,
                text(language, "tui.cache_write"),
                rate.cache_write,
                text(language, "tui.cache_read"),
                rate.cache_read
            )
        })
        .unwrap_or_else(|| unavailable.to_string());
    [
        text(language, "tui.estimated_spend").to_string(),
        session.name.clone(),
        String::new(),
        audit
            .estimated_cost_usd
            .map(format_compact_cost)
            .unwrap_or_else(|| format_compact_cost(audit.stored_estimated_cost_usd)),
        String::new(),
        format!(
            "{}       {}",
            text(language, "tui.input_tokens"),
            format_tokens(audit.tokens.input)
        ),
        format!(
            "{}      {}",
            text(language, "tui.output_tokens"),
            format_tokens(audit.tokens.output)
        ),
        format!(
            "{}        {}",
            text(language, "tui.cache_write_2"),
            format_tokens(audit.tokens.cache_write)
        ),
        format!(
            "{}         {}",
            text(language, "tui.cache_read_2"),
            format_tokens(audit.tokens.cache_read)
        ),
        format!(
            "{}     {}",
            text(language, "tui.total_counted"),
            format_tokens(audit.tokens.total)
        ),
        String::new(),
        text(language, "tui.price_sources").to_string(),
        format!(
            "{}  {}",
            text(language, "tui.current_rates"),
            audit.pricing_source
        ),
        format!(
            "{}  {}",
            text(language, "tui.stored_estimate"),
            audit.stored_pricing_source
        ),
        format!(
            "{}  {}",
            text(language, "tui.price_status"),
            i18n::pricing_status_label(&audit.pricing_status, language)
        ),
        format!(
            "{}  {}",
            text(language, "tui.how_complete_the_data_is"),
            i18n::capability_label(audit.capability, language)
        ),
        format!(
            "{}  {} / {}",
            text(language, "tui.source_model"),
            audit.provider,
            audit.model
        ),
        String::new(),
        text(language, "tui.split_by_token_type").to_string(),
        component_costs,
        rates,
        format!(
            "{}  {}",
            text(language, "tui.note"),
            text(
                language,
                match audit.pricing_status.as_str() {
                    "catalog_estimate" => "tui.pricing_note.catalog_estimate",
                    "fallback_estimate" => "tui.pricing_note.fallback_estimate",
                    "aggregate_estimate" => "tui.pricing_note.aggregate_estimate",
                    _ => "tui.pricing_note.unknown",
                }
            )
        ),
        format!(
            "{}  {}",
            text(language, "tui.historical_stored_estimate"),
            format_compact_cost(audit.stored_estimated_cost_usd)
        ),
        format!(
            "{}  {}",
            text(language, "tui.current_rate_estimate"),
            current_cost
        ),
        format!("{}  {}", text(language, "tui.difference"), difference),
        format!(
            "{}  {}",
            text(language, "tui.why_they_differ"),
            audit.pricing_note.clone()
        ),
        String::new(),
        text(language, "tui.this_is_a_local_estimate_from_token").to_string(),
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
                if item.timeouts > 0 {
                    text(language, "tui.timeout")
                } else if item.unmatched > 0 {
                    text(language, "tui.no_result")
                } else {
                    ""
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "{}\n{}\n\n{}       {}\n{}   {}\n{}      {}\n{} {}\n\n{}\n{}\n\n{}\n{}",
        text(language, "tui.tool_trouble"),
        session.name,
        text(language, "tui.calls"),
        session.metrics.tool_calls_total,
        text(language, "tui.succeeded"),
        session.metrics.tool_calls_ok,
        text(language, "tui.failed"),
        session.metrics.tool_calls_fail,
        text(language, "tui.repeat_loops"),
        session.diagnostics.loop_cost.loop_groups,
        text(language, "tui.most_used_tools"),
        if usage.is_empty() {
            text(language, "tui.no_tool_calls_showed_up")
        } else {
            &usage
        },
        text(language, "tui.slowest_tools"),
        if latency.is_empty() {
            text(language, "tui.no_timing_samples_showed_up")
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
        DetailSection::Summary => {
            let summary = detail_summary(session, app.language);
            if app.raw_report_expanded {
                summary
            } else {
                let evidence = explorer_evidence(session, app.language)
                    .into_iter()
                    .map(|item| format!("• {item}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!(
                    "{}\n{}\n\n{}\n{}\n\n{}\n{}\n\n{}",
                    app.t("tui.what_s_going_on"),
                    primary_finding(session, app.language),
                    app.t("tui.what_we_saw"),
                    evidence,
                    app.t("tui.what_to_do"),
                    explorer_recommendation(session, app.language),
                    app.t("tui.e_full_report_y_copy_safe_summary")
                )
            }
        }
        DetailSection::Context => explorer_detail_context(session, app.language),
        DetailSection::Files => detail_files(session, app.language),
        DetailSection::Timeline => unreachable!(),
    };
    let reason = inspect_reason(session);
    frame.render_widget(
        Paragraph::new(styled_detail_text(&text, reason))
            .scroll((app.scroll, 0))
            .wrap(Wrap { trim: false }),
        area,
    );
}

const DETAIL_HEADINGS: &[&str] = &[
    "What's going on",
    "现在的问题",
    "What we saw",
    "我们看到了什么",
    "What to do",
    "建议怎么做",
    "Numbers",
    "数字",
    "How complete this is",
    "信息全不全",
];

// Plain-text detail sections get heading/bullet styling without changing the copyable text.
fn styled_detail_text(text: &str, reason: &str) -> Vec<Line<'static>> {
    let mut after_problem_heading = false;
    text.lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if DETAIL_HEADINGS.contains(&trimmed) {
                after_problem_heading = matches!(trimmed, "What's going on" | "现在的问题");
                return Line::styled(
                    line.to_string(),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                );
            }
            if after_problem_heading && !trimmed.is_empty() {
                after_problem_heading = false;
                return Line::styled(
                    line.to_string(),
                    Style::default()
                        .fg(reason_color(reason))
                        .add_modifier(Modifier::BOLD),
                );
            }
            if let Some(rest) = trimmed.strip_prefix("• ") {
                return Line::from(vec![
                    Span::styled("• ", Style::default().fg(Color::DarkGray)),
                    Span::raw(rest.to_string()),
                ]);
            }
            if trimmed.starts_with("e ") && trimmed.contains(" · y ") {
                return Line::styled(line.to_string(), Style::default().fg(Color::DarkGray));
            }
            Line::raw(line.to_string())
        })
        .collect()
}

fn render_detail_sidebar(frame: &mut Frame<'_>, app: &App, session: &Session, area: Rect) {
    let audit = session_cost_audit(session);
    let text = vec![
        Line::styled(
            app.t("tui.session_at_a_glance"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        sidebar_field(
            app.t("tui.source"),
            display_session_source(session),
            Style::default(),
        ),
        sidebar_field(
            app.t("tui.model"),
            session.metrics.model_used.clone(),
            Style::default(),
        ),
        sidebar_field(
            app.t("tui.health_3"),
            session.health.to_string(),
            Style::default()
                .fg(health_color(session.health))
                .add_modifier(Modifier::BOLD),
        ),
        sidebar_field(
            app.t("tui.context_2"),
            format!(
                "{} ({})",
                format_context_pct(session.diagnostics.context_utilization.utilization_pct),
                i18n::risk_label(
                    &session.diagnostics.context_utilization.risk_level,
                    app.language,
                )
            ),
            Style::default().fg(risk_color(
                &session.diagnostics.context_utilization.risk_level,
            )),
        ),
        sidebar_field(
            app.t("tui.spend"),
            format_compact_cost(session.metrics.cost_estimated),
            Style::default().fg(cost_color(session.metrics.cost_estimated)),
        ),
        sidebar_field(
            app.t("tui.time_2"),
            format_duration(session.metrics.duration_sec),
            Style::default(),
        ),
        sidebar_field(
            app.t("tui.tools"),
            format!(
                "{} / {} {}",
                session.metrics.tool_calls_fail,
                session.metrics.tool_calls_total,
                app.t("tui.failed")
            ),
            if session.metrics.tool_calls_fail > 0 {
                Style::default().fg(Color::LightRed)
            } else {
                Style::default()
            },
        ),
        Line::raw(""),
        Line::styled(app.t("tui.data_quality"), Style::default().fg(Color::Cyan)),
        Line::raw(i18n::capability_label(audit.capability, app.language)),
        Line::raw(i18n::pricing_status_label(
            &audit.pricing_status,
            app.language,
        )),
        Line::raw(format!(
            "{}: {}",
            app.t("tui.tokens_2"),
            i18n::provenance_label(&session.metrics.provenance.tokens, app.language)
        )),
        Line::raw(format!(
            "{}: {}",
            app.t("tui.time_2"),
            i18n::provenance_label(&session.metrics.provenance.duration, app.language)
        )),
        Line::raw(format!(
            "{}: {}",
            app.t("tui.tool_results_2"),
            i18n::provenance_label(&session.metrics.provenance.tool_results, app.language)
        )),
        Line::raw(""),
        Line::styled(app.t("tui.workspace"), Style::default().fg(Color::Cyan)),
        Line::styled(
            if session.cwd.is_empty() {
                resolve_project(session).root
            } else {
                session.cwd.clone()
            },
            Style::default().fg(Color::Gray),
        ),
        Line::raw(""),
        Line::styled(app.t("tui.session_file"), Style::default().fg(Color::Cyan)),
        Line::styled(session.path.clone(), Style::default().fg(Color::Gray)),
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
        app.t("tui.scroll_esc_back_space_clear_comparison_help")
            .to_string()
    } else if app.mode == InputMode::Search {
        format!(
            "/ {}   {}/{}",
            app.input,
            app.filtered.len(),
            app.sessions.len()
        )
    } else if app.explorer_detail.is_some() {
        app.t("tui.help_esc_back_section_scroll_e_full").to_string()
    } else {
        app.t("tui.help_search_f_filter_v_views_enter").to_string()
    };
    frame.render_widget(
        Paragraph::new(vec![
            key_hint_line(&text, app.mode == InputMode::Search),
            Line::styled(
                short(
                    app.notice
                        .as_ref()
                        .map(|(message, _)| message.as_str())
                        .unwrap_or(&app.status),
                    area.width as usize,
                ),
                Style::default().fg(
                    if app
                        .notice
                        .as_ref()
                        .is_some_and(|(_, expiry)| expiry.is_none())
                    {
                        Color::Red
                    } else {
                        Color::Cyan
                    },
                ),
            ),
        ])
        .block(top_rule()),
        area,
    );
}

fn render_explorer_overlay(frame: &mut Frame<'_>, app: &App, area: Rect) {
    if app.explorer_overlay == ExplorerOverlay::None {
        return;
    }
    let buffer = frame.buffer_mut();
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = buffer.cell_mut((x, y)) {
                cell.set_style(Style::default().fg(Color::DarkGray).bg(Color::Reset));
            }
        }
    }
    let rect = centered_rect(
        area,
        76.min(area.width.saturating_sub(4)),
        match app.explorer_overlay {
            ExplorerOverlay::ViewPicker => 16,
            ExplorerOverlay::Filter => 15,
            ExplorerOverlay::Command => 20,
            ExplorerOverlay::ProjectPicker | ExplorerOverlay::SourcePicker => 20,
            ExplorerOverlay::Help => 14,
            ExplorerOverlay::None => 0,
        }
        .min(area.height.saturating_sub(4)),
    );
    // A double-width glyph starting one column left of the overlay would swallow its border.
    let guard = Rect::new(
        rect.x.saturating_sub(1),
        rect.y,
        rect.width + u16::from(rect.x > 0) + 1,
        rect.height,
    )
    .intersection(area);
    frame.render_widget(Clear, guard);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .padding(ratatui::widgets::Padding::horizontal(1));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    match app.explorer_overlay {
        ExplorerOverlay::ViewPicker => render_view_picker(frame, app, inner),
        ExplorerOverlay::Filter => render_filter_overlay(frame, app, inner),
        ExplorerOverlay::Command => render_command_overlay(frame, app, inner),
        ExplorerOverlay::Help => render_help_overlay(frame, app, inner),
        ExplorerOverlay::ProjectPicker | ExplorerOverlay::SourcePicker => {
            let choices = app.filter_choices();
            let visible = inner.height.saturating_sub(3) as usize;
            let start = app
                .overlay_selected
                .saturating_sub(visible.saturating_sub(1));
            let mut lines = vec![Line::raw(format!("/ {}▏", app.input))];
            for (index, (id, label)) in choices.iter().enumerate().skip(start).take(visible) {
                lines.push(overlay_row(index == app.overlay_selected, label, id));
            }
            if choices.is_empty() {
                lines.push(Line::raw(app.t("tui.no_matches")));
            }
            lines.push(Line::raw(app.t("tui.select_enter_apply_esc_back")));
            frame.render_widget(Paragraph::new(lines), inner);
        }
        ExplorerOverlay::None => {}
    }
}

fn render_view_picker(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let mut lines = vec![
        Line::styled(
            app.t("tui.switch_view"),
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
        app.t("tui.select_enter_open_esc_close"),
        Style::default().fg(Color::Gray),
    ));
    frame.render_widget(Paragraph::new(lines), area);
}

fn render_filter_overlay(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let any = app.t("tui.any");
    let health = if app.health_filter.is_empty() {
        any.to_string()
    } else {
        app.health_filter.clone()
    };
    let rows = [
        (app.t("tui.health_3"), health),
        (
            app.t("tui.source"),
            if app.source_filter.is_empty() {
                any.to_string()
            } else {
                app.source_filter.clone()
            },
        ),
        (
            app.t("tui.project"),
            if active_project_filter_label(app).is_empty() {
                any.to_string()
            } else {
                active_project_filter_label(app)
            },
        ),
        (
            app.t("tui.when"),
            range_label(app.range_filter, app.language).to_string(),
        ),
        (
            app.t("tui.context_risk"),
            if app.issue_filter == "context" {
                app.t("tui.warning_and_critical").to_string()
            } else {
                any.to_string()
            },
        ),
        (
            app.t("tui.reset_all"),
            active_filter_summary(app, app.language),
        ),
    ];
    let mut lines = vec![
        Line::styled(
            app.t("tui.filter_sessions"),
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
        app.t("tui.matching_sessions")
    )));
    lines.push(Line::styled(
        app.t("tui.field_enter_change_x_clear_esc_close"),
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
        app.t("tui.select_enter_run_esc_close"),
        Style::default().fg(Color::Gray),
    ));
    frame.render_widget(Paragraph::new(lines), area);
}

fn render_help_overlay(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let heading = |text: &str| {
        Line::styled(
            text.to_string(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
    };
    let row = |pairs: &[(&str, &str)]| {
        let mut spans = Vec::new();
        for (key, label) in pairs {
            spans.push(Span::styled(
                format!("{:>8} ", key),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw(pad_display_width(label, 22)));
        }
        Line::from(spans)
    };
    let mut lines = vec![heading(app.t("tui.keys")), Line::raw("")];
    if app.explorer_detail.is_some() {
        lines.push(row(&[
            ("j/k", app.t("tui.prev_next_session")),
            ("←/→", app.t("tui.sections")),
        ]));
        lines.push(row(&[
            ("↑/↓", app.t("tui.scroll")),
            ("e", app.t("tui.full_report")),
        ]));
        lines.push(row(&[
            ("y", app.t("tui.copy_safe_summary")),
            ("L", app.t("tui.language")),
        ]));
    } else {
        lines.push(row(&[
            ("↑/↓", app.t("tui.select")),
            ("Enter", app.t("tui.open")),
        ]));
        lines.push(row(&[
            ("Space", app.t("tui.mark_to_compare")),
            ("d", app.t("tui.compare")),
        ]));
        lines.push(row(&[
            ("D", app.t("tui.previous_run")),
            ("l", app.t("tui.language")),
        ]));
    }
    lines.push(Line::raw(""));
    lines.push(row(&[
        ("/", app.t("tui.search_2")),
        ("f", app.t("tui.filter_2")),
    ]));
    lines.push(row(&[
        ("v", app.t("tui.views")),
        ("Ctrl+K", app.t("tui.commands")),
    ]));
    lines.push(row(&[
        ("R", app.t("tui.time_range")),
        ("r", app.t("tui.reload")),
    ]));
    lines.push(row(&[
        ("Esc", app.t("tui.back_close")),
        ("q", app.t("tui.quit")),
    ]));
    frame.render_widget(Paragraph::new(lines), area);
}

// Deliberately exclude titles, paths, arguments and log excerpts from clipboard output.
fn share_summary(session: &Session) -> String {
    format!("AgentTrace\nHealth: {}\nEstimated cost: ${:.4}\nDuration: {:.1}s\nTool failures: {}\nAnomalies: {}\n", session.health, session.metrics.cost_estimated, session.metrics.duration_sec, session.metrics.tool_calls_fail, session.anomalies.len())
}

fn copy_summary(summary: &str) -> anyhow::Result<()> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let (program, args): (&str, &[&str]) = if cfg!(target_os = "macos") {
        ("pbcopy", &[])
    } else if cfg!(target_os = "windows") {
        ("clip.exe", &[])
    } else if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        ("wl-copy", &[])
    } else {
        ("xclip", &["-selection", "clipboard"])
    };
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let result = child
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("clipboard input unavailable"))?
        .write_all(summary.as_bytes());
    let status = child.wait()?;
    result?;
    anyhow::ensure!(status.success(), "{program} exited with {status}");
    Ok(())
}

const ROW_VALUE_WIDTH: usize = 12;

#[derive(Debug, Clone, Default)]
pub(super) struct ProjectTotals {
    label: String,
    root: String,
    count: usize,
    cost: f64,
}

fn explorer_row_spans(
    app: &App,
    session: &Session,
    marker: &str,
    width: u16,
    project_totals: Option<&std::collections::HashMap<String, ProjectTotals>>,
) -> Vec<Span<'static>> {
    let name_width = (width as usize)
        .saturating_sub(ROW_VALUE_WIDTH + 12)
        .max(10);
    let muted = Style::default().fg(Color::DarkGray);
    let (value, value_style) = match app.explorer_view {
        ExplorerView::Attention => {
            let reason = inspect_reason(session);
            (
                i18n::inspect_reason_label(reason, app.language).to_string(),
                Style::default().fg(reason_color(reason)),
            )
        }
        ExplorerView::Projects => {
            let totals = project_totals
                .and_then(|totals| totals.get(&resolve_project(session).id))
                .cloned()
                .unwrap_or_default();
            let (count, cost) = (totals.count, totals.cost);
            return vec![
                Span::raw(format!("{marker} ")),
                Span::styled(
                    pad_display_width(&totals.label, name_width.saturating_sub(10)),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{:>8}  ", format!("{count} {}", app.t("tui.runs"))),
                    muted,
                ),
                Span::styled(
                    pad_display_width(&format_compact_cost(cost), ROW_VALUE_WIDTH),
                    Style::default().fg(cost_color(cost)),
                ),
                Span::styled(
                    format!("{:>4}", session.health),
                    Style::default()
                        .fg(health_color(session.health))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
            ];
        }
        ExplorerView::Context => {
            let context = &session.diagnostics.context_utilization;
            (
                format_context_pct(context.utilization_pct),
                Style::default().fg(risk_color(&context.risk_level)),
            )
        }
        ExplorerView::Storage => (format_bytes(session_file_size(session)), muted),
        ExplorerView::Cost => (
            format_compact_cost(session.metrics.cost_estimated),
            Style::default().fg(cost_color(session.metrics.cost_estimated)),
        ),
        ExplorerView::Tools => (
            format!(
                "{} {}",
                session.metrics.tool_calls_fail,
                app.t("tui.failed_2")
            ),
            if session.metrics.tool_calls_fail > 0 {
                Style::default().fg(Color::LightRed)
            } else {
                muted
            },
        ),
        ExplorerView::Recent | ExplorerView::All => (display_session_source(session), muted),
    };
    vec![
        Span::raw(format!("{marker} ")),
        Span::raw(pad_display_width(&session.name, name_width)),
        Span::raw("  "),
        Span::styled(pad_display_width(&value, ROW_VALUE_WIDTH), value_style),
        Span::styled(
            format!("{:>4}", session.health),
            Style::default()
                .fg(health_color(session.health))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ]
}

// Footer hints are "key label" pairs separated by two spaces; keys render bold cyan.
fn key_hint_line(text: &str, raw: bool) -> Line<'static> {
    if raw {
        return Line::raw(text.to_string());
    }
    let mut spans = Vec::new();
    for (index, hint) in text
        .split("  ")
        .filter(|hint| !hint.trim().is_empty())
        .enumerate()
    {
        if index > 0 {
            spans.push(Span::styled("  ·  ", Style::default().fg(Color::DarkGray)));
        }
        let hint = hint.trim();
        match hint.split_once(' ') {
            Some((key, label)) => {
                spans.push(Span::styled(
                    key.to_string(),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    format!(" {label}"),
                    Style::default().fg(Color::Gray),
                ));
            }
            None => spans.push(Span::raw(hint.to_string())),
        }
    }
    Line::from(spans)
}

fn sidebar_field(label: &str, value: String, style: Style) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            pad_display_width(label, 9),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(value, style),
    ])
}

fn preview_field(label: &str, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            pad_display_width(label, 9),
            Style::default().fg(Color::DarkGray),
        ),
        Span::raw(value),
    ])
}

fn reason_color(reason: &str) -> Color {
    match reason {
        "critical" | "failures" => Color::LightRed,
        "anomaly" | "loops" => Color::Yellow,
        "cost" => Color::Magenta,
        "latency" => Color::LightBlue,
        _ => Color::DarkGray,
    }
}

fn cost_color(cost: f64) -> Color {
    if cost >= 50.0 {
        Color::LightRed
    } else if cost >= 10.0 {
        Color::Yellow
    } else if cost >= 1.0 {
        Color::White
    } else {
        Color::DarkGray
    }
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
            app.t("tui.urgent"),
            slow,
            app.t("tui.slow"),
            costly,
            app.t("tui.costly")
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
        text(language, "tui.tokens"),
        i18n::provenance_label(&session.metrics.provenance.tokens, language),
        text(language, "tui.time"),
        i18n::provenance_label(&session.metrics.provenance.duration, language),
        text(language, "tui.tool_results"),
        i18n::provenance_label(&session.metrics.provenance.tool_results, language),
        text(language, "tui.cost_2"),
        i18n::provenance_label(&session.metrics.provenance.cost, language)
    );
    format!(
        "{}\n{}\n\n{}\n{}={}  {}={}  {}={}  {}={}\n{}\n\n{}\n{}\n\n{}\n{}\n\n{}\n{}",
        text(language, "tui.what_s_going_on"),
        primary_finding(session, language),
        text(language, "tui.numbers"),
        text(language, "tui.health_2"),
        session.health,
        text(language, "tui.context"),
        format_context_pct(session.diagnostics.context_utilization.utilization_pct),
        text(language, "tui.cost_3"),
        format_compact_cost(session.metrics.cost_estimated),
        text(language, "tui.time"),
        format_duration(session.metrics.duration_sec),
        health_explanation(session, language),
        text(language, "tui.what_we_saw"),
        evidence,
        text(language, "tui.what_to_do"),
        explorer_recommendation(session, language),
        text(language, "tui.how_complete_this_is"),
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
    let time_width = 14;
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
    let steps = &session.diagnostics.steps;
    let start = (app.scroll as usize).min(steps.len());
    let mut previous_day = start
        .checked_sub(1)
        .and_then(|index| step_local_time(&steps[index].started_at))
        .map(|time| time.date_naive());
    let rows = steps
        .iter()
        .skip(start)
        .take(area.height.saturating_sub(4) as usize)
        .map(|step| {
            let name = short(&step.name, name_width);
            let status = Cell::from(localized_step_status(&step.status, app.language))
                .style(step_status_style(&step.status));
            let kind = localized_step_kind(&step.kind, app.language);
            if compact {
                Row::new(vec![
                    Cell::from(kind),
                    Cell::from(name),
                    Cell::from(format_duration(step.duration_sec)),
                    status,
                ])
            } else {
                let started = match step_local_time(&step.started_at) {
                    Some(time) => {
                        let day = time.date_naive();
                        let label = if previous_day == Some(day) {
                            time.format("%H:%M:%S").to_string()
                        } else {
                            time.format("%m-%d %H:%M:%S").to_string()
                        };
                        previous_day = Some(day);
                        label
                    }
                    None => short(&step.started_at, time_width as usize),
                };
                Row::new(vec![
                    Cell::from(started),
                    Cell::from(kind),
                    Cell::from(name),
                    Cell::from(format_duration(step.duration_sec)),
                    status,
                ])
            }
        })
        .collect::<Vec<_>>();
    let header = if compact {
        Row::new(vec![
            app.t("tui.type"),
            app.t("tui.step"),
            app.t("tui.time_2"),
            app.t("tui.result"),
        ])
    } else {
        Row::new(vec![
            app.t("tui.started"),
            app.t("tui.type"),
            app.t("tui.step"),
            app.t("tui.time_2"),
            app.t("tui.result"),
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
        app.t("tui.what_happened"),
        session.diagnostics.steps.len(),
        app.t("tui.steps")
    );
    frame.render_widget(
        Table::new(rows, constraints)
            .header(header)
            .column_spacing(2)
            .block(Block::default().title(title).borders(Borders::BOTTOM)),
        area,
    );
}

fn step_local_time(value: &str) -> Option<chrono::DateTime<chrono::Local>> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|time| time.with_timezone(&chrono::Local))
}

fn step_status_style(status: &str) -> Style {
    match status {
        "error" | "failed" | "fail" => Style::default().fg(Color::Red),
        "ok" | "success" => Style::default().fg(Color::Green),
        _ => Style::default(),
    }
}

pub(super) fn detail_timeline_empty(session: &Session, language: Language) -> String {
    let mut lines = vec![
        text(language, "tui.what_happened").to_string(),
        String::new(),
        text(language, "tui.this_session_didn_t_record_a_step").to_string(),
    ];
    for anomaly in &session.anomalies {
        lines.push(format!(
            "• [{}] {}",
            localized_level(&anomaly.severity, language),
            anomaly.detail_for(language)
        ));
    }
    lines.push(String::new());
    lines.push(text(language, "tui.we_didn_t_see_a_compaction_event").to_string());
    lines.join("\n")
}

pub(super) fn explorer_detail_context(session: &Session, language: Language) -> String {
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
        "{}\n\n{}       {}\n{}  {}\n{}         {}\n{}      {}\n{}    {}\n{}           {}\n{}                  {}\n\n{}\n{}\n\n{}\n{}\n\n{}",
        text(language, "tui.context_2"),
        text(language, "tui.estimated_total"),
        format_tokens(value.estimated_total as i64),
        text(language, "tui.conversation_history"),
        format_tokens(value.conversation_history as i64),
        text(language, "tui.system_prompt"),
        format_tokens(value.system_prompt as i64),
        text(language, "tui.tool_definitions"),
        format_tokens(value.tool_definitions as i64),
        text(language, "tui.room_left"),
        format_tokens(value.available_for_task as i64),
        text(language, "tui.used"),
        format_context_pct(value.utilization_pct),
        text(language, "tui.risk"),
        i18n::risk_label(&value.risk_level, language),
        text(language, "tui.what_s_taking_space"),
        if params.is_empty() {
            text(language, "tui.none_observed").to_string()
        } else {
            params
        },
        text(language, "tui.did_it_compact"),
        text(
            language, "tui.we_didn_t_see_a_compaction_event",
        ),
        value.suggestion_for(language)
    )
}

fn health_explanation(session: &Session, language: Language) -> String {
    let mut parts = Vec::new();
    if session.metrics.tool_calls_fail > 0 {
        parts.push(format!(
            "{} {}",
            session.metrics.tool_calls_fail,
            text(language, "tui.tool_failures")
        ));
    }
    if matches!(
        session.diagnostics.context_utilization.risk_level.as_str(),
        "warning" | "critical"
    ) {
        parts.push(format!(
            "{} {}",
            text(language, "tui.context"),
            format_context_pct(session.diagnostics.context_utilization.utilization_pct)
        ));
    }
    if session.diagnostics.loop_cost.loop_groups > 0 {
        parts.push(format!(
            "{} {}",
            session.diagnostics.loop_cost.loop_groups,
            text(language, "tui.repeat_loops_2")
        ));
    }
    if !session.anomalies.is_empty() {
        parts.push(format!(
            "{} {}",
            session.anomalies.len(),
            text(language, "tui.unusual_signals")
        ));
    }
    if parts.is_empty() {
        text(language, "tui.health_no_clear_penalty_found").to_string()
    } else {
        format!(
            "{}: {}",
            text(language, "tui.health_is_affected_by"),
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
        .unwrap_or_else(|| text(language, "tui.unknown").to_string());
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
        text(language, "tui.session_file"),
        session.path,
        session.cwd,
        text(language, "tui.size"),
        format_bytes(size),
        text(language, "tui.last_changed"),
        modified,
        text(language, "tui.files_it_touched"),
        if accessed.is_empty() {
            text(language, "tui.none_observed").to_string()
        } else {
            accessed
        },
        text(language, "tui.possible_repeated_reads",),
        if repeated.is_empty() {
            text(language, "tui.no_file_appeared_3_or_more_times").to_string()
        } else {
            repeated
        },
        text(language, "tui.this_count_comes_from_file_paths_in")
    )
}

fn primary_finding(session: &Session, language: Language) -> String {
    match inspect_reason(session) {
        "critical" => format!(
            "{} ({})",
            text(language, "tui.this_session_looks_unhealthy"),
            session.health
        ),
        "anomaly" => session
            .anomalies
            .first()
            .map(|anomaly| anomaly.detail_for(language))
            .unwrap_or_else(|| text(language, "tui.something_unusual_showed_up").to_string()),
        "failures" => format!(
            "{}: {}",
            text(language, "tui.tools_failed"),
            session.metrics.tool_calls_fail
        ),
        "context" => format!(
            "{} ({})",
            text(language, "tui.context_is_nearly_full"),
            format_context_pct(session.diagnostics.context_utilization.utilization_pct)
        ),
        "loops" => format!(
            "{}: {}",
            text(language, "tui.repeated_tool_loop"),
            session.diagnostics.loop_cost.loop_groups
        ),
        "latency" => format!(
            "{} {}",
            text(language, "tui.this_run_was_slow"),
            format_duration(session.metrics.duration_sec)
        ),
        "cost" => format!(
            "{} {}",
            text(language, "tui.this_was_the_most_expensive_session"),
            format_compact_cost(session.metrics.cost_estimated)
        ),
        "warning" => text(language, "tui.this_session_needs_a_closer_look").to_string(),
        _ => text(language, "tui.nothing_urgent_jumped_out").to_string(),
    }
}

fn explorer_evidence(session: &Session, language: Language) -> Vec<String> {
    let mut evidence = Vec::new();
    let context = &session.diagnostics.context_utilization;
    if session.metrics.tool_calls_fail > 0 {
        let rate = session.metrics.tool_calls_fail as f64
            / session.metrics.tool_calls_total.max(1) as f64
            * 100.0;
        evidence.push(format!(
            "{} / {} {} ({rate:.0}%)",
            session.metrics.tool_calls_fail,
            session.metrics.tool_calls_total,
            text(language, "tui.tool_calls_failed")
        ));
    }
    if context.utilization_pct > 0.0 && context.risk_level != "good" {
        evidence.push(format!(
            "{} {} ({})",
            text(language, "tui.context_2"),
            format_context_pct(context.utilization_pct),
            i18n::risk_label(&context.risk_level, language)
        ));
    }
    if session.diagnostics.loop_cost.loop_groups > 0 {
        evidence.push(format!(
            "{} {}",
            session.diagnostics.loop_cost.loop_groups,
            text(language, "tui.repeat_loops_2")
        ));
    }
    for anomaly in session
        .anomalies
        .iter()
        .filter(|anomaly| anomaly.kind != "tool_failures")
        .take(3)
    {
        evidence.push(anomaly.detail_for(language));
    }
    if evidence.is_empty() {
        evidence.push(text(language, "tui.there_s_not_much_extra_detail_for").to_string());
    }
    evidence
}

// Context size is a rough character-based estimate, shown as a floor rather than a precise overflow.
pub(super) fn format_context_pct(pct: f64) -> String {
    if pct > 100.0 {
        ">100%".to_string()
    } else {
        format!("{pct:.0}%")
    }
}

fn explorer_recommendation(session: &Session, language: Language) -> String {
    let context = &session.diagnostics.context_utilization;
    match inspect_reason(session) {
        "loops" => {
            return text(language, "tui.the_agent_repeated_the_same_call_check").to_string();
        }
        "cost" => {
            return text(language, "tui.check_the_spend_view_to_see_which").to_string();
        }
        "latency" => {
            return text(language, "tui.look_for_the_longest_gaps_in_what").to_string();
        }
        _ => {}
    }
    if session.metrics.tool_calls_fail > 0 {
        return text(language, "tui.check_the_failed_tool_calls_before_trying").to_string();
    }
    if !context.suggestion.trim().is_empty() && context.risk_level != "good" {
        return context.suggestion_for(language);
    }
    text(language, "tui.open_what_happened_and_check_the_recorded").to_string()
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
    let (label_style, description_style) = if selected {
        (
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            Style::default().fg(Color::Black).bg(Color::Cyan),
        )
    } else {
        (Style::default(), Style::default().fg(Color::DarkGray))
    };
    Line::from(vec![
        Span::styled(
            format!(
                "{} {}",
                if selected { "›" } else { " " },
                pad_display_width(label, 20)
            ),
            label_style,
        ),
        Span::styled(format!("  {description} "), description_style),
    ])
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    )
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

#[cfg(test)]
mod interaction_tests {
    use super::*;

    #[test]
    fn initial_progress_remains_visible_after_first_batch() {
        use ratatui::{backend::TestBackend, Terminal};
        let mut app = App::new(agenttrace_core::demo_sessions().unwrap(), "demo", None);
        let (_tx, rx) = mpsc::channel();
        app.pending_load = Some(rx);
        app.load_state.showing_cached = false;
        app.load_state.discovered = 20;
        app.load_state.processed = 8;
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| render_explorer(frame, &mut app))
            .unwrap();
        assert!(format!("{:?}", terminal.backend().buffer()).contains("8/20 · 40%"));
        app.load_state.processed = 20;
        terminal
            .draw(|frame| render_explorer(frame, &mut app))
            .unwrap();
        assert!(format!("{:?}", terminal.backend().buffer()).contains("loading databases"));
    }

    #[test]
    fn notices_expire_without_busy_polling_and_filters_fit_narrow_header() {
        use ratatui::{backend::TestBackend, Terminal};
        let mut app = App::new(agenttrace_core::demo_sessions().unwrap(), "demo", None);
        app.notice = Some((
            "Copied".into(),
            Some(Instant::now() + Duration::from_secs(2)),
        ));
        assert!(event_poll_timeout(&app) <= Duration::from_secs(2));
        assert!(!app.expire_notice());
        app.notice.as_mut().unwrap().1 = Some(Instant::now() - Duration::from_secs(1));
        assert!(app.expire_notice());
        assert_eq!(event_poll_timeout(&app), Duration::from_secs(60));
        app.notice = Some(("Copy failed".into(), None));
        assert!(!app.expire_notice());
        app.handle_explorer_event(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)))
            .unwrap();
        assert!(app.notice.is_none());
        app.source_filter = "hermes".into();
        app.refresh_filtered();
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| render_explorer(frame, &mut app))
            .unwrap();
        let screen = format!("{:?}", terminal.backend().buffer());
        assert!(screen.contains("source: hermes"));
        assert!(screen.contains("? Help"));
        let (_tx, rx) = mpsc::channel();
        app.pending_load = Some(rx);
        assert!(shared::load_summary_line(&app).contains("Refreshing"));
        assert!(event_poll_timeout(&app) <= POLL_INTERVAL);
    }

    #[test]
    fn searchable_projects_preserve_identity_and_refresh_reading_position() {
        let mut sessions = agenttrace_core::demo_sessions().unwrap();
        sessions[0].cwd = "/alpha/same".into();
        sessions[1].cwd = "/beta/same".into();
        let mut app = App::new(sessions.clone(), "demo", None);
        app.open_explorer_overlay(ExplorerOverlay::ProjectPicker);
        app.input = "beta".into();
        assert_eq!(app.filter_choices().len(), 1);
        app.activate_explorer_overlay().unwrap();
        assert_eq!(app.project_id_filter, "/beta/same");
        assert_eq!(app.filtered.len(), 1);
        app.explorer_detail = Some(DetailSection::Summary);
        app.scroll = 7;
        app.apply_loaded_sessions(
            LoadReport {
                sessions,
                discovered: 3,
                ..Default::default()
            },
            false,
        );
        assert_eq!(app.scroll, 7);
        assert_eq!(app.explorer_detail, Some(DetailSection::Summary));
        assert_eq!(
            ExplorerLayout::for_area(Rect::new(0, 0, 120, 18)),
            ExplorerLayout::Standard
        );
        let summary = share_summary(app.explorer_session().unwrap());
        assert!(!summary.contains("beta"));
        assert!(!summary.contains(&app.explorer_session().unwrap().name));
    }
}
