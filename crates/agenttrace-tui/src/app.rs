use agenttrace_core::{
    average_health, canonical_sessions, clear_session_cache, compute_overview,
    compute_overview_iter, context_trends, cost_audit, data_health, delivery_evidence_with_git,
    fix_suggestions, format_cost, format_tokens, inspect_first, load_cached_sessions_from_cache,
    load_session_cache, load_sessions_with_progress, load_sessions_with_progress_from_cache_mode,
    mcp_governance, predict_cost_anomaly, project_name, recommendations,
    render_waste_report_with_language, report_compare_with_language, report_text_with_language,
    resolve_project, session_capability, session_matches_time_range, total_tokens, ContextTrend,
    CostAudit, DataHealth, DeliveryEvidence, LoadOptions, LoadProgress, LoadReport, McpGovernance,
    Overview, Recommendation, ReportLanguage, Session, SessionCache, TimeRange, VERSION,
};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Tabs, Wrap};
use ratatui::{DefaultTerminal, Frame};
use std::borrow::Borrow;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

const POLL_INTERVAL: Duration = Duration::from_millis(120);
const LOAD_BATCH_SIZE: usize = 8;
enum LoadMessage {
    Progress(Vec<LoadProgress>),
    Complete(anyhow::Result<(bool, LoadReport)>),
}

pub fn run(sessions_dir: &str) -> anyhow::Result<()> {
    run_with_language(sessions_dir, None)
}

pub fn run_with_language(sessions_dir: &str, language: Option<&str>) -> anyhow::Result<()> {
    let label = if sessions_dir.trim().is_empty() {
        "auto-discovery"
    } else {
        sessions_dir
    };
    let mut app = App::new_loading(label, sessions_dir.to_string());
    if let Some(language) = parse_language(language) {
        app.language = language;
    }
    run_with_app(app)
}

pub fn run_with_sessions(sessions: Vec<Session>, label: &str) -> anyhow::Result<()> {
    run_with_app(App::new(sessions, label, None))
}

fn parse_language(value: Option<&str>) -> Option<Language> {
    match value?.trim().to_ascii_lowercase().as_str() {
        "en" | "english" => Some(Language::En),
        "zh" | "zh-cn" | "zh_cn" | "chinese" => Some(Language::Zh),
        _ => None,
    }
}

fn run_with_app(app: App) -> anyhow::Result<()> {
    let mut terminal = ratatui::init();
    let result = run_app(&mut terminal, app);
    ratatui::restore();
    result
}

fn run_app(terminal: &mut DefaultTerminal, mut app: App) -> anyhow::Result<()> {
    let mut dirty = true;
    loop {
        dirty |= app.poll_pending_load();
        dirty |= app.poll_governance_delivery();
        if dirty {
            terminal.draw(|frame| render(frame, &mut app))?;
            dirty = false;
        }
        let timeout = if app.pending_load.is_some() || app.governance_delivery_pending() {
            POLL_INTERVAL
        } else {
            Duration::from_secs(60)
        };
        if event::poll(timeout)? {
            if app.handle_event(event::read()?)? {
                break;
            }
            dirty = true;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Overview,
    List,
    Detail,
    Diagnostics,
    Diff,
    Governance(GovernancePanel),
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GovernancePanel {
    ActionCenter,
    Efficiency,
    Delivery,
}

#[derive(Default)]
struct GovernanceSnapshot {
    audit: Option<CostAudit>,
    recommendations: Option<Vec<Recommendation>>,
    mcp: Option<McpGovernance>,
    context: Option<ContextTrend>,
    delivery: Option<DeliveryEvidence>,
    delivery_pending: Option<Receiver<DeliveryEvidence>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputMode {
    Normal,
    Search,
    Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortKey {
    Recent,
    Health,
    Cost,
    Turns,
    Failures,
    Source,
    Name,
    Anomalies,
}

#[derive(Debug, Clone, Copy)]
enum DriverKind {
    Source,
    Model,
    Anomaly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Language {
    En,
    Zh,
}

impl Language {
    fn toggle(self) -> Self {
        match self {
            Self::En => Self::Zh,
            Self::Zh => Self::En,
        }
    }

    fn report(self) -> ReportLanguage {
        match self {
            Self::En => ReportLanguage::En,
            Self::Zh => ReportLanguage::Zh,
        }
    }
}

struct App {
    sessions: Vec<Session>,
    overview: Overview,
    source_label: String,
    reload_dir: Option<String>,
    view: View,
    help_context: View,
    mode: InputMode,
    filtered: Vec<usize>,
    selected: usize,
    table_state: TableState,
    query: String,
    health_filter: String,
    source_filter: String,
    model_filter: String,
    project_filter: String,
    range_filter: TimeRange,
    cost_filter: Option<(CostOp, f64)>,
    anomaly_filter: Option<String>,
    capability_filter: String,
    issue_filter: String,
    input: String,
    input_original: String,
    status: String,
    sort_key: SortKey,
    sort_desc: bool,
    scroll: u16,
    pending_load: Option<Receiver<LoadMessage>>,
    load_state: LoadState,
    language: Language,
    derived: OverviewDerived,
    raw_report_expanded: bool,
    governance: Option<GovernanceSnapshot>,
    governance_dirty: bool,
}

#[derive(Debug, Clone, Default)]
struct OverviewDerived {
    health: DataHealth,
    average_health: f64,
    total_tokens: i64,
    total_duration: f64,
    p95_gap: f64,
    top_source: Option<DriverItem>,
    top_model: Option<DriverItem>,
    top_project: Option<DriverItem>,
    top_anomaly: Option<DriverItem>,
    inspect_first: Vec<InspectFirstItem>,
    tool_failure_sessions: usize,
    stuck_sessions: usize,
    context_risk_sessions: usize,
    loop_sessions: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CostOp {
    Gt,
    Gte,
    Lt,
    Lte,
    Eq,
}

#[derive(Debug, Clone, Default)]
struct LoadState {
    phase: LoadPhase,
    force: bool,
    source: String,
    discovered: usize,
    processed: usize,
    parsed: usize,
    skipped: usize,
    cache_hits: usize,
    cache_state: String,
    sources: Vec<(String, usize)>,
    showing_cached: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum LoadPhase {
    #[default]
    Idle,
    Discovering,
    Parsing,
    Ready,
    Failed,
}

impl App {
    fn new(sessions: Vec<Session>, source_label: &str, reload_dir: Option<String>) -> Self {
        let sessions = canonical_sessions(&sessions);
        let overview = compute_overview(&sessions);
        let mut app = Self {
            sessions,
            overview,
            source_label: source_label.to_string(),
            reload_dir,
            view: View::Overview,
            help_context: View::Overview,
            mode: InputMode::Normal,
            filtered: Vec::new(),
            selected: 0,
            table_state: TableState::default(),
            query: String::new(),
            health_filter: String::new(),
            source_filter: String::new(),
            model_filter: String::new(),
            project_filter: String::new(),
            range_filter: TimeRange::All,
            cost_filter: None,
            anomaly_filter: None,
            capability_filter: String::new(),
            issue_filter: String::new(),
            input: String::new(),
            input_original: String::new(),
            status: String::new(),
            sort_key: SortKey::Recent,
            sort_desc: true,
            scroll: 0,
            pending_load: None,
            load_state: LoadState::default(),
            language: Language::En,
            derived: OverviewDerived::default(),
            raw_report_expanded: false,
            governance: None,
            governance_dirty: true,
        };
        app.refresh_filtered();
        app
    }

    fn new_loading(source_label: &str, reload_dir: String) -> Self {
        let dir = (!reload_dir.trim().is_empty()).then(|| std::path::Path::new(&reload_dir));
        let mut cache = load_session_cache();
        let sessions = load_cached_sessions_from_cache(dir, &mut cache);
        let mut app = Self::new(sessions, source_label, Some(reload_dir));
        app.language = saved_language();
        app.start_reload_with_cache(false, Some(cache));
        app
    }

    fn handle_event(&mut self, event: Event) -> anyhow::Result<bool> {
        match event {
            Event::Paste(text) => self.handle_paste(text),
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    return Ok(true);
                }
                match self.mode {
                    InputMode::Search => Ok(self.handle_search_key(key)),
                    InputMode::Command => self.handle_command_key(key),
                    InputMode::Normal => self.handle_normal_key(key),
                }
            }
            _ => Ok(false),
        }
    }

    fn handle_paste(&mut self, text: String) -> anyhow::Result<bool> {
        match self.mode {
            InputMode::Search => {
                self.input.push_str(&text.replace(['\r', '\n'], " "));
                self.apply_search_input();
            }
            InputMode::Command => self.input.push_str(&text.replace(['\r', '\n'], " ")),
            InputMode::Normal => {}
        }
        Ok(false)
    }

    fn handle_search_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => {
                self.query.clone_from(&self.input_original);
                self.refresh_filtered();
                self.mode = InputMode::Normal;
                self.input.clear();
                self.input_original.clear();
                self.status = self.t("search cancelled", "已取消搜索").to_string();
            }
            KeyCode::Enter => {
                self.apply_search_input();
                self.mode = InputMode::Normal;
                self.input.clear();
                self.input_original.clear();
                self.status = if self.query.is_empty() {
                    self.t("filter cleared", "已清除筛选").to_string()
                } else {
                    format!("{}: {}", self.t("filter", "筛选"), self.query)
                };
            }
            KeyCode::Backspace => {
                self.input.pop();
                self.apply_search_input();
            }
            KeyCode::Char(c) => {
                self.input.push(c);
                self.apply_search_input();
            }
            _ => {}
        }
        false
    }

    fn apply_search_input(&mut self) {
        self.query = self.input.trim().to_string();
        self.refresh_filtered();
        self.view = View::List;
    }

    fn handle_command_key(&mut self, key: KeyEvent) -> anyhow::Result<bool> {
        match key.code {
            KeyCode::Esc => {
                self.mode = InputMode::Normal;
                self.input.clear();
            }
            KeyCode::Enter => {
                let command = self.input.trim().to_string();
                self.input.clear();
                self.mode = InputMode::Normal;
                return self.run_command(&command);
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) => self.input.push(c),
            _ => {}
        }
        Ok(false)
    }

    fn handle_normal_key(&mut self, key: KeyEvent) -> anyhow::Result<bool> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('r') {
            self.reload(true)?;
            return Ok(false);
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('d') {
            self.move_page(8);
            return Ok(false);
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('u') {
            self.move_page(-8);
            return Ok(false);
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(true),
            KeyCode::Char(':') => {
                self.mode = InputMode::Command;
                self.input.clear();
            }
            KeyCode::Char('/') => {
                self.mode = InputMode::Search;
                self.input.clone_from(&self.query);
                self.input_original.clone_from(&self.query);
                self.view = View::List;
            }
            KeyCode::Char('?') => {
                if self.view == View::Help {
                    self.view = self.help_context;
                } else {
                    self.help_context = self.view;
                    self.view = View::Help;
                }
                self.scroll = 0;
            }
            KeyCode::Tab | KeyCode::Char('`') => self.next_view(),
            KeyCode::Char('0') => self.view = View::Overview,
            KeyCode::Char('1') => self.view = View::List,
            KeyCode::Char('2') => {
                if self.selected_session().is_some() {
                    self.view = View::Detail;
                    self.scroll = 0;
                }
            }
            KeyCode::Enter => {
                if self.view == View::Overview {
                    self.open_inspect_first();
                } else if self.selected_session().is_some() {
                    self.view = View::Detail;
                    self.scroll = 0;
                }
            }
            KeyCode::Char('3') | KeyCode::Char('w') => {
                if self.selected_session().is_some() {
                    self.view = View::Diagnostics;
                    self.scroll = 0;
                }
            }
            KeyCode::Char('4') | KeyCode::Char('d') => {
                self.view = View::Diff;
                self.scroll = 0;
            }
            KeyCode::Char('5') => self.open_governance(GovernancePanel::ActionCenter),
            KeyCode::Char('6') | KeyCode::Char('8') => {
                self.open_governance(GovernancePanel::Efficiency)
            }
            KeyCode::Char('7') | KeyCode::Char('9') => {
                self.open_governance(GovernancePanel::Delivery)
            }
            KeyCode::Char('g') | KeyCode::Char('G') if matches!(self.view, View::Governance(_)) => {
                self.next_governance_panel();
            }
            KeyCode::Char('v') if self.view == View::Detail => {
                self.raw_report_expanded = !self.raw_report_expanded;
                self.scroll = 0;
            }
            KeyCode::Char('r') => self.reload(false)?,
            KeyCode::Char('f') => self.cycle_health_filter(),
            KeyCode::Char('s') => self.filter_selected_source(),
            KeyCode::Char('S') => self.filter_top_driver(DriverKind::Source),
            KeyCode::Char('M') => self.filter_top_driver(DriverKind::Model),
            KeyCode::Char('A') => self.filter_top_driver(DriverKind::Anomaly),
            KeyCode::Char('R') => self.cycle_range(),
            KeyCode::Char('$') => self.filter_costly_sessions(),
            KeyCode::Char('!') => self.filter_critical_sessions(),
            KeyCode::Char('c') => self.set_sort(SortKey::Cost),
            KeyCode::Char('e') => self.set_sort(SortKey::Failures),
            KeyCode::Char('h') => self.set_sort(SortKey::Health),
            KeyCode::Char('n') => self.set_sort(SortKey::Name),
            KeyCode::Char('t') => self.set_sort(SortKey::Turns),
            KeyCode::Char('a') => self.set_sort(SortKey::Anomalies),
            KeyCode::Char('l') | KeyCode::Char('L') => self.toggle_language(),
            KeyCode::Esc => {
                if matches!(
                    self.view,
                    View::Detail | View::Diagnostics | View::Diff | View::Governance(_)
                ) {
                    self.view = View::List;
                    self.scroll = 0;
                } else if self.view == View::Help {
                    self.view = self.help_context;
                    self.scroll = 0;
                } else if self.has_filters() {
                    self.clear_filters();
                    self.refresh_filtered();
                    self.status = self.t("filter cleared", "已清除筛选").to_string();
                } else {
                    self.view = View::Overview;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => self.move_next(),
            KeyCode::Up | KeyCode::Char('k') => self.move_previous(),
            KeyCode::Char('G') => {
                if !self.filtered.is_empty() {
                    self.selected = self.filtered.len() - 1;
                    self.clamp_selection();
                }
            }
            KeyCode::PageDown => self.scroll = self.scroll.saturating_add(8),
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(8),
            _ => {}
        }
        Ok(false)
    }

    fn t(&self, en: &'static str, zh: &'static str) -> &'static str {
        text(self.language, en, zh)
    }

    fn toggle_language(&mut self) {
        self.language = self.language.toggle();
        if let Err(error) = save_language(self.language) {
            self.status = format!("{}: {error}", UiText::LanguageSaveFailed.get(self.language));
            return;
        }
        self.status = match self.language {
            Language::En => "language: English (saved)".to_string(),
            Language::Zh => "语言：中文（已保存）".to_string(),
        };
    }

    fn run_command(&mut self, command: &str) -> anyhow::Result<bool> {
        let command = command.trim();
        let lower = command.to_ascii_lowercase();
        match lower.as_str() {
            "" => {}
            "q" | "quit" | "exit" => return Ok(true),
            "0" | "overview" => self.view = View::Overview,
            "1" | "list" => self.view = View::List,
            "2" | "detail" => {
                if self.selected_session().is_some() {
                    self.view = View::Detail;
                    self.scroll = 0;
                }
            }
            "3" | "diagnostics" | "waste" => {
                if self.selected_session().is_some() {
                    self.view = View::Diagnostics;
                    self.scroll = 0;
                }
            }
            "4" | "diff" => self.view = View::Diff,
            "5" | "governance" | "action" | "audit" | "recommend" | "recommendations" => {
                self.open_governance(GovernancePanel::ActionCenter)
            }
            "6" | "8" | "efficiency" | "mcp" | "mcp-governance" | "context" | "context-trends" => {
                self.open_governance(GovernancePanel::Efficiency)
            }
            "7" | "9" | "delivery" | "delivery-evidence" => {
                self.open_governance(GovernancePanel::Delivery)
            }
            "help" | "?" => {
                self.help_context = self.view;
                self.view = View::Help;
            }
            "first" | "inspect" => {
                self.select_inspect_item(0);
            }
            "clear" | "reset" => {
                self.clear_filters();
                self.refresh_filtered();
                self.view = View::List;
                self.status = self.t("filter cleared", "已清除筛选").to_string();
            }
            "reload" | "r" => self.reload(false)?,
            "critical" => {
                self.health_filter = "crit".to_string();
                self.refresh_filtered();
                self.view = View::List;
                self.status = self
                    .t("filter health: critical", "筛选健康度：严重")
                    .to_string();
            }
            "anomalies" | "anomaly" => {
                self.anomaly_filter = Some(String::new());
                self.refresh_filtered();
                self.view = View::List;
                self.status = self.t("filter anomalies", "筛选异常").to_string();
            }
            _ if lower.starts_with("first ") || lower.starts_with("inspect ") => {
                let fields = command.split_whitespace().collect::<Vec<_>>();
                if fields.len() == 2 {
                    match fields[1].parse::<usize>() {
                        Ok(rank) if rank > 0 => {
                            self.select_inspect_item(rank - 1);
                        }
                        _ => {
                            self.status = self
                                .t("usage: :inspect [1-5]", "用法：:inspect [1-5]")
                                .to_string()
                        }
                    }
                } else {
                    self.status = self
                        .t("usage: :inspect [1-5]", "用法：:inspect [1-5]")
                        .to_string();
                }
            }
            _ if lower.starts_with("search ") || lower.starts_with("filter ") => {
                let query = command
                    .split_once(' ')
                    .map(|(_, value)| value.trim())
                    .unwrap_or("");
                self.query = query.to_string();
                self.refresh_filtered();
                self.view = View::List;
                self.status = format!("{}: {}", self.t("filter", "筛选"), self.query);
            }
            _ if lower.starts_with("health ") => {
                let value = command_value(command);
                if parse_health_filter(&value).is_some() {
                    self.health_filter = value.to_ascii_lowercase();
                    self.refresh_filtered();
                    self.view = View::List;
                    self.status = format!(
                        "{}: {}",
                        self.t("filter health", "筛选健康度"),
                        self.health_filter
                    );
                } else {
                    self.status = self
                        .t(
                            "usage: :health good|warn|crit|<80|>=90",
                            "用法：:health good|warn|crit|<80|>=90",
                        )
                        .to_string();
                }
            }
            _ if lower.starts_with("source ") => {
                self.source_filter = command_value(command);
                self.refresh_filtered();
                self.view = View::List;
                self.status = format!(
                    "{}: {}",
                    self.t("filter source", "筛选来源"),
                    self.source_filter
                );
            }
            _ if lower.starts_with("model ") => {
                self.model_filter = command_value(command);
                self.refresh_filtered();
                self.view = View::List;
                self.status = format!(
                    "{}: {}",
                    self.t("filter model", "筛选模型"),
                    self.model_filter
                );
            }
            _ if lower.starts_with("project ") => {
                self.project_filter = command_value(command);
                self.refresh_filtered();
                self.view = View::List;
                self.status = format!(
                    "{}: {}",
                    self.t("filter project", "筛选项目"),
                    self.project_filter
                );
            }
            _ if lower.starts_with("range ") => {
                let value = command_value(command);
                if let Some(range) = TimeRange::parse(&value) {
                    self.range_filter = range;
                    self.refresh_filtered();
                    self.status = format!("{}: {}", self.t("range", "范围"), range.label());
                } else {
                    self.status = self
                        .t(
                            "usage: :range today|7d|30d|all",
                            "用法：:range today|7d|30d|all",
                        )
                        .to_string();
                }
            }
            _ if lower.starts_with("cost ") => {
                let value = command_value(command);
                if let Some(filter) = parse_cost_filter(&value) {
                    self.cost_filter = Some(filter);
                    self.refresh_filtered();
                    self.view = View::List;
                    self.status = format!("{}: {}", self.t("filter cost", "筛选成本"), value);
                } else {
                    self.status = self
                        .t(
                            "usage: :cost >0.10|>=1|<0.05|=0",
                            "用法：:cost >0.10|>=1|<0.05|=0",
                        )
                        .to_string();
                }
            }
            _ if lower.starts_with("anomaly ") || lower.starts_with("anomalies ") => {
                let value = command_value(command).to_ascii_lowercase();
                self.anomaly_filter = Some(value.clone());
                self.refresh_filtered();
                self.view = View::List;
                self.status = if value.is_empty() {
                    self.t("filter anomalies", "筛选异常").to_string()
                } else {
                    format!("{}: {value}", self.t("filter anomaly", "筛选异常"))
                };
            }
            _ if lower.starts_with("capability ") || lower.starts_with("data ") => {
                let value = command_value(command).to_ascii_lowercase();
                if matches!(value.as_str(), "detailed" | "aggregate" | "limited") {
                    self.capability_filter = value.clone();
                    self.refresh_filtered();
                    self.view = View::List;
                    self.status =
                        format!("{}: {value}", self.t("filter capability", "筛选数据能力"));
                } else {
                    self.status = self
                        .t(
                            "usage: :capability detailed|aggregate|limited",
                            "用法：:capability detailed|aggregate|limited",
                        )
                        .to_string();
                }
            }
            _ if lower.starts_with("issues ") || lower.starts_with("issue ") => {
                let value = command_value(command).to_ascii_lowercase();
                if matches!(value.as_str(), "failures" | "stuck" | "context" | "loops") {
                    self.issue_filter = value.clone();
                    self.refresh_filtered();
                    self.view = View::List;
                    self.status = format!("{}: {value}", self.t("filter issue", "筛选问题"));
                } else {
                    self.status = self
                        .t(
                            "usage: :issues failures|stuck|context|loops",
                            "用法：:issues failures|stuck|context|loops",
                        )
                        .to_string();
                }
            }
            _ if lower.starts_with("top ") => match parse_sort_key(&command_value(command)) {
                Some(key) => self.set_sort_desc(key, true),
                None => {
                    self.status = self
                        .t(
                            "usage: :top cost|turns|failures|health|source|anomalies",
                            "用法：:top cost|turns|failures|health|source|anomalies",
                        )
                        .to_string()
                }
            },
            _ if lower.starts_with("sort ") => {
                let fields = command.split_whitespace().collect::<Vec<_>>();
                if fields.len() < 2 || fields.len() > 3 {
                    self.status = self
                        .t(
                            "usage: :sort health|cost|turns|failures|source|name|anomalies [asc|desc]",
                            "用法：:sort health|cost|turns|failures|source|name|anomalies [asc|desc]",
                        )
                        .to_string();
                } else if let Some(key) = parse_sort_key(fields[1]) {
                    let desc = if fields.len() == 3 {
                        match fields[2].to_ascii_lowercase().as_str() {
                            "asc" => false,
                            "desc" => true,
                            _ => {
                                self.status = format!(
                                    "{}: {}",
                                    self.t("unknown sort direction", "未知排序方向"),
                                    fields[2]
                                );
                                return Ok(false);
                            }
                        }
                    } else {
                        key != SortKey::Name
                    };
                    self.set_sort_desc(key, desc);
                } else {
                    self.status = self
                        .t(
                            "usage: :sort health|cost|turns|failures|source|name|anomalies [asc|desc]",
                            "用法：:sort health|cost|turns|failures|source|name|anomalies [asc|desc]",
                        )
                        .to_string();
                }
            }
            _ => {
                self.query = command.to_string();
                self.refresh_filtered();
                self.view = View::List;
                self.status = format!("{}: {}", self.t("filter", "筛选"), self.query);
            }
        }
        Ok(false)
    }

    fn reload(&mut self, force: bool) -> anyhow::Result<()> {
        self.start_reload(force);
        Ok(())
    }

    fn start_reload(&mut self, force: bool) {
        self.start_reload_with_cache(force, None);
    }

    fn start_reload_with_cache(&mut self, force: bool, cache: Option<SessionCache>) {
        let Some(dir) = self.reload_dir.as_deref() else {
            self.status = self
                .t(
                    "reload unavailable for this session source",
                    "当前会话来源不支持重新加载",
                )
                .to_string();
            return;
        };
        let dir = dir.to_string();
        if force {
            self.sessions.clear();
            self.refresh_filtered();
        }
        let cache_state = if force {
            "cache bypass".to_string()
        } else {
            cache_state_label()
        };
        let (tx, rx) = mpsc::channel();
        self.pending_load = Some(rx);
        self.load_state = LoadState {
            phase: LoadPhase::Discovering,
            force,
            source: self.source_label.clone(),
            discovered: 0,
            processed: 0,
            parsed: 0,
            skipped: 0,
            cache_hits: 0,
            cache_state,
            sources: Vec::new(),
            showing_cached: !force && !self.sessions.is_empty(),
        };
        self.derived.health = data_health(&self.sessions, 0, 0);
        self.status = self
            .t(
                if force {
                    "force reload: discovering session files"
                } else {
                    "loading: discovering session files"
                },
                if force {
                    "强制重载：正在发现会话文件"
                } else {
                    "加载中：正在发现会话文件"
                },
            )
            .to_string();
        thread::spawn(move || {
            let mut batch = Vec::with_capacity(LOAD_BATCH_SIZE);
            let result = load_sessions_for_tui(&dir, force, cache, |progress| {
                batch.push(progress);
                if batch.len() == LOAD_BATCH_SIZE {
                    let _ = tx.send(LoadMessage::Progress(std::mem::take(&mut batch)));
                }
            });
            if !batch.is_empty() {
                let _ = tx.send(LoadMessage::Progress(batch));
            }
            let _ = tx.send(LoadMessage::Complete(result));
        });
    }

    fn poll_pending_load(&mut self) -> bool {
        let Some(rx) = self.pending_load.take() else {
            return false;
        };
        let mut changed = false;
        loop {
            match rx.try_recv() {
                Ok(LoadMessage::Progress(progress)) => {
                    for progress in progress {
                        self.apply_load_progress(progress);
                    }
                    changed = true;
                }
                Ok(LoadMessage::Complete(Ok((force, report)))) => {
                    self.apply_loaded_sessions(report, force);
                    return true;
                }
                Ok(LoadMessage::Complete(Err(err))) => {
                    self.load_state.phase = LoadPhase::Failed;
                    self.status = format!("{}: {err}", self.t("reload failed", "重新加载失败"));
                    return true;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.load_state.phase = LoadPhase::Failed;
                    self.status = self
                        .t(
                            "reload failed: loader disconnected",
                            "重新加载失败：加载器已断开",
                        )
                        .to_string();
                    return true;
                }
            }
        }
        if changed {
            self.finish_progress_batch();
            self.load_state.phase = LoadPhase::Parsing;
        }
        self.pending_load = Some(rx);
        changed
    }

    fn apply_load_progress(&mut self, progress: LoadProgress) {
        self.load_state.discovered = progress.discovered;
        self.load_state.processed = progress.processed;
        self.load_state.parsed = progress.parsed;
        self.load_state.skipped = progress.skipped;
        self.load_state.cache_hits = progress.cache_hits;
        if !self.load_state.showing_cached {
            if let Some(session) = progress.session {
                self.sessions.push(session);
            }
        }
    }

    fn finish_progress_batch(&mut self) {
        self.refresh_filtered();
        self.load_state.sources = source_counts(&self.sessions);
    }

    fn apply_loaded_sessions(&mut self, report: LoadReport, force: bool) {
        let selected = self.selected_session().cloned();
        self.load_state.discovered = report.discovered;
        self.load_state.processed = report.discovered;
        self.load_state.skipped = report.skipped;
        self.load_state.cache_hits = report.cache_hits;
        self.sessions = report.sessions;
        self.sessions
            .sort_by(|left, right| compare_sessions(left, right, SortKey::Recent, true));
        let selected_index = selected.as_ref().and_then(|selected| {
            self.sessions
                .iter()
                .position(|session| same_session(session, selected))
        });
        let selection_missing = selected.is_some() && selected_index.is_none();
        self.selected = 0;
        self.scroll = 0;
        self.refresh_filtered();
        if let Some(position) = selected_index.and_then(|index| {
            self.filtered
                .iter()
                .position(|candidate| *candidate == index)
        }) {
            self.selected = position;
            self.clamp_selection();
        } else if selection_missing {
            self.view = View::List;
        }
        self.load_state.phase = LoadPhase::Ready;
        self.load_state.showing_cached = false;
        self.load_state.force = force;
        self.load_state.parsed = self.sessions.len();
        self.load_state.sources = source_counts(&self.sessions);
        self.status = if selection_missing {
            self.t(
                "reloaded; selected session is no longer available",
                "已重新加载；原选中会话已不存在",
            )
            .to_string()
        } else if force {
            format!(
                "{} {} {} {} {}",
                self.t("force reloaded", "已强制重载"),
                format_count(self.sessions.len() as i64),
                self.t("sessions from", "个会话，来自"),
                format_count(self.load_state.discovered as i64),
                self.t("files", "个文件")
            )
        } else {
            format!(
                "{} {} {} {} {}",
                self.t("loaded", "已加载"),
                format_count(self.sessions.len() as i64),
                self.t("sessions from", "个会话，来自"),
                format_count(self.load_state.discovered as i64),
                self.t("files", "个文件")
            )
        };
    }

    fn next_view(&mut self) {
        self.view = match self.view {
            View::Overview => View::List,
            View::List => {
                if self.selected_session().is_some() {
                    View::Detail
                } else {
                    View::Overview
                }
            }
            View::Detail => View::Diagnostics,
            View::Diagnostics => View::Diff,
            View::Diff => View::Governance(GovernancePanel::ActionCenter),
            View::Governance(_) | View::Help => View::Overview,
        };
        self.scroll = 0;
    }

    fn set_sort(&mut self, key: SortKey) {
        let selected = self.filtered.get(self.selected).copied();
        if self.sort_key == key {
            self.sort_desc = !self.sort_desc;
        } else {
            self.sort_key = key;
            self.sort_desc = key != SortKey::Name;
        }
        self.refresh_filtered();
        if let Some(position) = selected.and_then(|index| {
            self.filtered
                .iter()
                .position(|candidate| *candidate == index)
        }) {
            self.selected = position;
            self.clamp_selection();
        }
        self.status = format!(
            "{} {}",
            self.t("sorted by", "排序："),
            sort_key_label(self.sort_key, self.language)
        );
        self.view = View::List;
    }

    fn set_sort_desc(&mut self, key: SortKey, desc: bool) {
        self.sort_key = key;
        self.sort_desc = desc;
        self.refresh_filtered();
        self.status = format!(
            "{} {} {}",
            self.t("sorted by", "排序："),
            sort_key_label(self.sort_key, self.language),
            if self.sort_desc {
                self.t("desc", "降序")
            } else {
                self.t("asc", "升序")
            }
        );
        self.view = View::List;
    }

    fn clear_filters(&mut self) {
        self.clear_triage_filters();
        self.source_filter.clear();
        self.model_filter.clear();
        self.project_filter.clear();
        self.range_filter = TimeRange::All;
        self.capability_filter.clear();
        self.issue_filter.clear();
    }

    fn clear_triage_filters(&mut self) {
        self.query.clear();
        self.health_filter.clear();
        self.cost_filter = None;
        self.anomaly_filter = None;
    }

    fn has_filters(&self) -> bool {
        !self.query.is_empty()
            || !self.health_filter.is_empty()
            || !self.source_filter.is_empty()
            || !self.model_filter.is_empty()
            || !self.project_filter.is_empty()
            || self.range_filter != TimeRange::All
            || self.cost_filter.is_some()
            || self.anomaly_filter.is_some()
            || !self.capability_filter.is_empty()
            || !self.issue_filter.is_empty()
    }

    fn cycle_health_filter(&mut self) {
        self.health_filter = match self.health_filter.as_str() {
            "" => "good".to_string(),
            "good" => "warn".to_string(),
            "warn" => "crit".to_string(),
            _ => String::new(),
        };
        self.refresh_filtered();
        self.view = View::List;
        self.status = if self.health_filter.is_empty() {
            self.t("quick health filter cleared", "已清除快捷健康度筛选")
                .to_string()
        } else {
            format!(
                "{}: {}",
                self.t("quick health filter", "快捷健康度筛选"),
                health_filter_label(&self.health_filter, self.language)
            )
        };
    }

    fn cycle_range(&mut self) {
        self.range_filter = match self.range_filter {
            TimeRange::All => TimeRange::Today,
            TimeRange::Today => TimeRange::Days7,
            TimeRange::Days7 => TimeRange::Days30,
            TimeRange::Days30 => TimeRange::All,
        };
        self.refresh_filtered();
        self.status = format!("{}: {}", self.t("range", "范围"), self.range_filter.label());
    }

    fn filter_selected_source(&mut self) {
        let Some(source) = self
            .selected_session()
            .map(|session| session.metrics.source_tool.clone())
            .filter(|source| !source.is_empty())
        else {
            self.status = UiText::CurrentSourceUnavailable
                .get(self.language)
                .to_string();
            return;
        };
        self.source_filter = source;
        self.refresh_filtered();
        self.view = View::List;
        self.status = format!(
            "{}: {}",
            self.t("quick source filter", "快捷来源筛选"),
            display_source_label(&self.source_filter)
        );
    }

    fn filter_top_driver(&mut self, kind: DriverKind) {
        let sessions = self.visible_sessions();
        let value = match kind {
            DriverKind::Source => top_driver(&sessions, driver_source).map(|item| item.label),
            DriverKind::Model => top_driver(&sessions, driver_model).map(|item| item.label),
            DriverKind::Anomaly => top_anomaly_driver(&sessions).map(|item| item.label),
        };
        let Some(value) = value else { return };
        self.clear_filters();
        match kind {
            DriverKind::Source => self.source_filter = value.clone(),
            DriverKind::Model => self.model_filter = value.clone(),
            DriverKind::Anomaly => self.anomaly_filter = Some(value.clone()),
        }
        self.refresh_filtered();
        self.view = View::List;
        self.status = format!("{}: {value}", self.t("top driver filter", "主要驱动筛选"));
    }

    fn filter_costly_sessions(&mut self) {
        self.cost_filter = Some((CostOp::Gt, 0.0));
        self.refresh_filtered();
        self.view = View::List;
        self.status = self
            .t("quick cost filter: >0", "快捷成本筛选：>0")
            .to_string();
    }

    fn filter_critical_sessions(&mut self) {
        self.health_filter = "crit".to_string();
        self.refresh_filtered();
        self.view = View::List;
        self.status = self.t("quick critical filter", "快捷严重筛选").to_string();
    }

    fn open_inspect_first(&mut self) {
        self.select_inspect_item(0);
    }

    fn select_inspect_item(&mut self, rank: usize) -> bool {
        let Some(item) = self.derived.inspect_first.get(rank).cloned() else {
            self.status = if self.sessions.is_empty() {
                self.t("no sessions loaded", "尚未加载会话").to_string()
            } else {
                format!(
                    "{} {} {}",
                    self.t("inspect rank", "检查排名"),
                    rank + 1,
                    self.t("unavailable", "不可用")
                )
            };
            return false;
        };
        let Some(session) = self.sessions.get(item.index) else {
            self.status = self
                .t("inspect target unavailable", "检查目标不可用")
                .to_string();
            return false;
        };
        let session_name = session.name.clone();
        let target_view = inspect_target_view(item.label);

        let Some(position) = self.filtered.iter().position(|idx| *idx == item.index) else {
            self.status = self
                .t("inspect target hidden", "检查目标已隐藏")
                .to_string();
            return false;
        };
        self.selected = position;
        self.clamp_selection();
        self.view = target_view;
        self.scroll = 0;
        self.status = format!(
            "{} {} #{}: {}",
            self.t("inspect", "检查"),
            inspect_label(item.label, self.language),
            rank + 1,
            short(&session_name, 36)
        );
        true
    }

    fn refresh_filtered(&mut self) {
        let query = self.query.trim().to_ascii_lowercase();
        let now = chrono::Utc::now();
        self.filtered = self
            .sessions
            .iter()
            .enumerate()
            .filter_map(|(idx, session)| {
                if self.session_visible(session, &query, now) {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect();
        let sort_key = self.sort_key;
        let sort_desc = self.sort_desc;
        self.filtered.sort_by(|a, b| {
            compare_sessions(&self.sessions[*a], &self.sessions[*b], sort_key, sort_desc)
        });
        self.clamp_selection();
        self.overview = compute_overview_iter(
            self.filtered
                .iter()
                .filter_map(|index| self.sessions.get(*index)),
        );
        self.governance_dirty = true;
        let visible = self.visible_sessions();
        let tool_failure_sessions = visible
            .iter()
            .filter(|session| session.metrics.tool_calls_fail > 0)
            .count();
        let stuck_sessions = visible
            .iter()
            .filter(|session| !session.diagnostics.stuck_patterns.is_empty())
            .count();
        let context_risk_sessions = visible
            .iter()
            .filter(|session| {
                matches!(
                    session.diagnostics.context_utilization.risk_level.as_str(),
                    "warning" | "critical"
                )
            })
            .count();
        let loop_sessions = visible
            .iter()
            .filter(|session| session.diagnostics.loop_cost.loop_groups > 0)
            .count();
        self.derived = OverviewDerived {
            health: data_health(
                &self.sessions,
                self.sessions.len() + self.load_state.skipped,
                self.load_state.cache_hits,
            ),
            average_health: average_health(&visible),
            total_tokens: total_tokens_all(&visible),
            total_duration: total_duration(&visible),
            p95_gap: p95_gap(&visible),
            top_source: top_driver(&visible, driver_source),
            top_model: top_driver(&visible, driver_model),
            top_project: top_driver(&visible, project_name),
            top_anomaly: top_anomaly_driver(&visible),
            inspect_first: inspect_first_items_for_app(self),
            tool_failure_sessions,
            stuck_sessions,
            context_risk_sessions,
            loop_sessions,
        };
    }

    fn session_visible(
        &self,
        session: &Session,
        query: &str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> bool {
        (query.is_empty() || session_matches(session, query))
            && matches_health_filter(session, &self.health_filter)
            && matches_source_filter(session, &self.source_filter)
            && matches_text_filter(&session.metrics.model_used, &self.model_filter)
            && matches_text_filter(&project_name(session), &self.project_filter)
            && session_matches_time_range(session, self.range_filter, now)
            && matches_cost_filter(session, self.cost_filter)
            && matches_anomaly_filter(session, self.anomaly_filter.as_deref())
            && (self.capability_filter.is_empty()
                || session_capability(session) == self.capability_filter)
            && matches_issue_filter(session, &self.issue_filter)
    }

    fn clamp_selection(&mut self) {
        if self.filtered.is_empty() {
            self.selected = 0;
            self.table_state.select(None);
            return;
        }
        if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len() - 1;
        }
        self.table_state.select(Some(self.selected));
    }

    fn move_next(&mut self) {
        if matches!(
            self.view,
            View::Detail | View::Diagnostics | View::Diff | View::Governance(_)
        ) {
            self.scroll = self.scroll.saturating_add(1);
            return;
        }
        if !self.filtered.is_empty() {
            self.selected = (self.selected + 1).min(self.filtered.len() - 1);
            self.clamp_selection();
        }
    }

    fn move_previous(&mut self) {
        if matches!(
            self.view,
            View::Detail | View::Diagnostics | View::Diff | View::Governance(_)
        ) {
            self.scroll = self.scroll.saturating_sub(1);
            return;
        }
        self.selected = self.selected.saturating_sub(1);
        self.clamp_selection();
    }

    fn move_page(&mut self, delta: i16) {
        if matches!(
            self.view,
            View::Detail | View::Diagnostics | View::Diff | View::Governance(_)
        ) {
            self.scroll = self.scroll.saturating_add_signed(delta);
            return;
        }
        self.selected = self.selected.saturating_add_signed(delta.into());
        self.clamp_selection();
    }

    fn selected_session(&self) -> Option<&Session> {
        self.filtered
            .get(self.selected)
            .and_then(|idx| self.sessions.get(*idx))
    }

    fn visible_sessions(&self) -> Vec<&Session> {
        self.filtered
            .iter()
            .filter_map(|idx| self.sessions.get(*idx))
            .collect()
    }

    fn open_governance(&mut self, panel: GovernancePanel) {
        self.view = View::Governance(panel);
        self.scroll = 0;
    }

    fn next_governance_panel(&mut self) {
        let panel = match self.view {
            View::Governance(GovernancePanel::ActionCenter) => GovernancePanel::Efficiency,
            View::Governance(GovernancePanel::Efficiency) => GovernancePanel::Delivery,
            View::Governance(GovernancePanel::Delivery) => GovernancePanel::ActionCenter,
            _ => GovernancePanel::ActionCenter,
        };
        self.open_governance(panel);
    }

    fn ensure_governance(&mut self, panel: GovernancePanel) {
        if self.governance_dirty {
            self.governance = Some(GovernanceSnapshot::default());
            self.governance_dirty = false;
        }
        let snapshot = self
            .governance
            .get_or_insert_with(GovernanceSnapshot::default);
        let missing = match panel {
            GovernancePanel::ActionCenter => {
                snapshot.audit.is_none() || snapshot.recommendations.is_none()
            }
            GovernancePanel::Efficiency => snapshot.mcp.is_none() || snapshot.context.is_none(),
            GovernancePanel::Delivery => {
                snapshot.delivery.is_none() && snapshot.delivery_pending.is_none()
            }
        };
        if !missing {
            return;
        }
        let sessions = self.visible_sessions_cloned();
        match panel {
            GovernancePanel::ActionCenter => {
                let snapshot = self.governance.as_mut().expect("governance initialized");
                if snapshot.audit.is_none() {
                    snapshot.audit = Some(cost_audit(&sessions));
                }
                if snapshot.recommendations.is_none() {
                    snapshot.recommendations = Some(recommendations(&sessions));
                }
            }
            GovernancePanel::Efficiency => {
                let snapshot = self.governance.as_mut().expect("governance initialized");
                if snapshot.mcp.is_none() {
                    snapshot.mcp = Some(mcp_governance(&sessions));
                }
                if snapshot.context.is_none() {
                    snapshot.context = Some(context_trends(&sessions));
                }
            }
            GovernancePanel::Delivery => {
                let (tx, rx) = mpsc::channel();
                self.governance
                    .as_mut()
                    .expect("governance initialized")
                    .delivery_pending = Some(rx);
                thread::spawn(move || {
                    let _ = tx.send(delivery_evidence_with_git(&sessions));
                });
            }
        }
    }

    fn visible_sessions_cloned(&self) -> Vec<Session> {
        self.visible_sessions().into_iter().cloned().collect()
    }

    fn governance_delivery_pending(&self) -> bool {
        self.governance
            .as_ref()
            .is_some_and(|snapshot| snapshot.delivery_pending.is_some())
    }

    fn poll_governance_delivery(&mut self) -> bool {
        let Some(snapshot) = self.governance.as_mut() else {
            return false;
        };
        let Some(receiver) = snapshot.delivery_pending.take() else {
            return false;
        };
        match receiver.try_recv() {
            Ok(delivery) => {
                snapshot.delivery = Some(delivery);
                true
            }
            Err(mpsc::TryRecvError::Empty) => {
                snapshot.delivery_pending = Some(receiver);
                false
            }
            Err(mpsc::TryRecvError::Disconnected) => true,
        }
    }
}

fn language_preference_path() -> Option<std::path::PathBuf> {
    #[cfg(test)]
    let base = std::env::temp_dir().join(format!("agenttrace-tui-test-{}", std::process::id()));
    #[cfg(not(test))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".config"))
        })?;
    Some(base.join("agenttrace").join("tui-language"))
}

fn saved_language() -> Language {
    language_preference_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map(|value| match value.trim() {
            "zh" => Language::Zh,
            _ => Language::En,
        })
        .unwrap_or(Language::En)
}

fn save_language(language: Language) -> std::io::Result<()> {
    let Some(path) = language_preference_path() else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        path,
        match language {
            Language::En => "en\n",
            Language::Zh => "zh\n",
        },
    )
}

fn same_session(left: &Session, right: &Session) -> bool {
    left.path == right.path
        && left.name == right.name
        && left.metrics.session_start == right.metrics.session_start
}

fn text(language: Language, en: &'static str, zh: &'static str) -> &'static str {
    match language {
        Language::En => en,
        Language::Zh => zh,
    }
}

fn context_view_label(view: View, language: Language) -> &'static str {
    match view {
        View::Overview => text(language, "Overview", "概览"),
        View::List => text(language, "List", "列表"),
        View::Detail => text(language, "Detail", "详情"),
        View::Diagnostics => text(language, "Diagnostics", "诊断"),
        View::Diff => text(language, "Diff", "对比"),
        View::Governance(panel) => governance_panel_label(panel, language),
        View::Help => text(language, "Help", "帮助"),
    }
}

fn governance_panel_label(panel: GovernancePanel, language: Language) -> &'static str {
    match panel {
        GovernancePanel::ActionCenter => text(language, "Action Center", "行动中心"),
        GovernancePanel::Efficiency => text(language, "Efficiency", "效率"),
        GovernancePanel::Delivery => text(language, "Delivery", "交付"),
    }
}

fn sort_key_label(key: SortKey, language: Language) -> &'static str {
    match key {
        SortKey::Recent => text(language, "Recent", "最近"),
        SortKey::Health => text(language, "Health", "健康度"),
        SortKey::Cost => text(language, "Cost", "成本"),
        SortKey::Turns => text(language, "Turns", "轮次"),
        SortKey::Failures => text(language, "Failures", "失败"),
        SortKey::Source => text(language, "Source", "来源"),
        SortKey::Name => text(language, "Name", "名称"),
        SortKey::Anomalies => text(language, "Anomalies", "异常"),
    }
}

fn health_filter_label(filter: &str, language: Language) -> String {
    match filter {
        "good" | "healthy" => text(language, "good", "良好").to_string(),
        "warn" | "warning" => text(language, "warning", "警告").to_string(),
        "crit" | "critical" => text(language, "critical", "严重").to_string(),
        _ => filter.to_string(),
    }
}

fn range_label(range: TimeRange, language: Language) -> &'static str {
    match range {
        TimeRange::Today => text(language, "today", "今天"),
        TimeRange::Days7 => text(language, "7d", "7天"),
        TimeRange::Days30 => text(language, "30d", "30天"),
        TimeRange::All => text(language, "all", "全部"),
    }
}

fn cache_state_for_language(state: &str, language: Language) -> &'static str {
    match state {
        "cache warm" => text(language, "cache warm", "缓存已预热"),
        "cache bypass" => text(language, "cache bypass", "绕过缓存"),
        _ => text(language, "cache empty", "缓存为空"),
    }
}

fn inspect_label(label: &str, language: Language) -> &'static str {
    match label {
        "critical" => text(language, "critical", "严重"),
        "anomaly" => text(language, "anomaly", "异常"),
        "failures" => text(language, "failures", "失败"),
        "cost" => text(language, "cost", "成本"),
        "latency" => text(language, "latency", "延迟"),
        _ => text(language, "session", "会话"),
    }
}

fn load_sessions_for_tui(
    dir: &str,
    force: bool,
    cache: Option<SessionCache>,
    on_progress: impl FnMut(LoadProgress),
) -> anyhow::Result<(bool, LoadReport)> {
    if force {
        clear_session_cache()?;
    }
    let load_dir = if dir.trim().is_empty() {
        None
    } else {
        Some(std::path::Path::new(dir))
    };
    let report = if let Some(mut cache) = cache {
        load_sessions_with_progress_from_cache_mode(
            load_dir,
            &LoadOptions::default(),
            &mut cache,
            false,
            on_progress,
        )
    } else {
        load_sessions_with_progress(load_dir, &LoadOptions::default(), on_progress)
    };
    Ok((force, report))
}

#[path = "filters.rs"]
mod filters;
#[path = "i18n.rs"]
mod i18n;
#[path = "presentation.rs"]
mod presentation;

use filters::*;
use i18n::UiText;
use presentation::*;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
