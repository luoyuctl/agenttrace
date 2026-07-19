use super::*;
use agenttrace_core::{Anomaly, Metrics};
use crossterm::event::KeyModifiers;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::fs;

#[test]
fn filters_sessions_by_metadata_and_tool_usage() {
    let mut app = App::new(
        vec![
            session("billing", "claude_code", "claude-sonnet-4", 70, 0.02, "rg"),
            session("docs", "codex_cli", "gpt-5", 95, 0.01, "read_file"),
        ],
        "test",
        None,
    );

    app.query = "billing".to_string();
    app.refresh_filtered();
    assert_eq!(app.filtered.len(), 1);
    assert_eq!(
        app.selected_session().map(|s| s.name.as_str()),
        Some("billing")
    );

    app.query = "read_file".to_string();
    app.refresh_filtered();
    assert_eq!(
        app.selected_session().map(|s| s.name.as_str()),
        Some("docs")
    );

    app.query = "Claude Code".to_string();
    app.refresh_filtered();
    assert_eq!(
        app.selected_session().map(|s| s.name.as_str()),
        Some("billing")
    );
}

#[test]
fn commands_switch_views_and_apply_search() {
    let mut app = App::new(
        vec![session("billing", "claude_code", "m", 80, 0.0, "rg")],
        "test",
        None,
    );

    assert!(!app.run_command("search billing").unwrap());
    assert_eq!(app.view, View::List);
    assert_eq!(app.filtered.len(), 1);

    assert!(!app.run_command("reset").unwrap());
    assert_eq!(app.view, View::List);
    assert_eq!(app.status, "filter cleared");
    assert_eq!(app.filtered.len(), 1);
    assert!(app.query.is_empty());

    assert!(!app.run_command("diagnostics").unwrap());
    assert_eq!(app.view, View::Diagnostics);

    assert!(app.run_command("quit").unwrap());
}

#[test]
fn inspect_command_selects_ranked_session_and_clears_filters() {
    let mut anomalous = session("anomalous", "pi", "m", 70, 0.05, "rg");
    anomalous.cwd = "/tmp/in-scope".to_string();
    anomalous.anomalies.push(Anomaly {
        kind: "latency".to_string(),
        severity: "high".to_string(),
        detail: "p95 gap".to_string(),
    });
    let mut critical = session("critical", "claude_code", "m", 40, 0.20, "bash");
    critical.cwd = "/tmp/in-scope".to_string();
    critical.metrics.tool_calls_fail = 2;
    critical.metrics.tool_calls_total = 3;
    let mut docs = session("docs", "codex_cli", "gpt-5", 95, 0.01, "read_file");
    docs.cwd = "/tmp/out-of-scope".to_string();
    let recent = chrono::Utc::now().to_rfc3339();
    anomalous.metrics.session_start = recent.clone();
    critical.metrics.session_start = recent.clone();
    docs.metrics.session_start = recent;
    let mut app = App::new(vec![critical, anomalous, docs], "test", None);

    app.project_filter = "in-scope".to_string();
    app.range_filter = TimeRange::Days30;
    app.query = "critical".to_string();
    app.refresh_filtered();
    assert_eq!(
        app.selected_session().map(|session| session.name.as_str()),
        Some("critical")
    );

    assert!(!app.run_command("inspect 2").unwrap());
    assert!(app.query.is_empty());
    assert_eq!(app.project_filter, "in-scope");
    assert_eq!(app.range_filter, TimeRange::Days30);
    assert_eq!(app.view, View::Diagnostics);
    assert_eq!(
        app.selected_session().map(|session| session.name.as_str()),
        Some("anomalous")
    );
    assert!(app.status.contains("inspect anomaly #2"));
}

#[test]
fn overview_enter_opens_first_inspect_item() {
    let mut app = App::new(
        vec![
            session("healthy", "codex_cli", "gpt-5", 95, 0.01, "read_file"),
            session("critical", "claude_code", "m", 35, 0.20, "bash"),
        ],
        "test",
        None,
    );
    app.view = View::Overview;

    app.handle_normal_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .expect("overview enter");

    assert_eq!(app.view, View::Diagnostics);
    assert_eq!(
        app.selected_session().map(|session| session.name.as_str()),
        Some("critical")
    );
    assert!(app.status.contains("inspect critical #1"));
}

#[test]
fn language_defaults_to_english_and_l_toggles_chinese() {
    let mut app = App::new(
        vec![session("critical", "claude_code", "m", 35, 0.20, "bash")],
        "test",
        None,
    );
    assert_eq!(app.language, Language::En);
    assert!(help_text(View::Overview, app.language).contains("Triage workflow"));

    app.handle_normal_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE))
        .expect("toggle language");
    assert_eq!(app.language, Language::Zh);
    assert_eq!(app.status, "语言：中文");
    assert!(help_text(View::Overview, app.language).contains("分诊流程"));

    let backend = TestBackend::new(100, 24);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| render(frame, &mut app))
        .expect("render zh overview");
    let overview = format!("{:?}", terminal.backend().buffer());
    assert!(overview.contains("概览"));
    assert!(overview.contains("优先检查"));
    assert!(overview.contains("健康度严重"));
    assert!(overview.contains("打开诊断查看严重健康问题"));
    assert!(!detail_text(&app).contains("AI 智能体会话性能报告"));
    app.raw_report_expanded = true;
    assert!(detail_text(&app).contains("AI 智能体会话性能报告"));
    app.raw_report_expanded = false;
    let diagnostics = diagnostics_text(&app);
    assert!(diagnostics.contains("浪费分析"));
    assert!(!diagnostics.contains("Waste Analysis"));

    app.help_context = View::Overview;
    app.view = View::Help;
    terminal
        .draw(|frame| render(frame, &mut app))
        .expect("render zh help");
    let help = format!("{:?}", terminal.backend().buffer());
    assert!(help.contains("分诊流程"));
    assert!(help.contains("当前视图：概览"));

    app.handle_normal_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE))
        .expect("toggle language back");
    assert_eq!(app.language, Language::En);
    assert_eq!(app.status, "language: English");
}

#[test]
fn unknown_commands_fall_back_to_text_filter_like_go_tui() {
    let mut app = App::new(
        vec![
            session("billing", "claude_code", "m", 80, 0.0, "rg"),
            session("docs", "codex_cli", "m", 95, 0.0, "read_file"),
        ],
        "test",
        None,
    );

    assert!(!app.run_command("billing").unwrap());
    assert_eq!(app.view, View::List);
    assert_eq!(app.query, "billing");
    assert_eq!(app.status, "filter: billing");
    assert_eq!(app.filtered.len(), 1);
    assert_eq!(
        app.selected_session().map(|session| session.name.as_str()),
        Some("billing")
    );
}

#[test]
fn commands_apply_go_style_triage_filters() {
    let mut costly = session("costly", "claude_code", "claude-sonnet-4", 45, 1.20, "rg");
    costly.anomalies.push(Anomaly {
        kind: "latency".to_string(),
        severity: "high".to_string(),
        detail: "p95 gap".to_string(),
    });
    let mut app = App::new(
        vec![
            costly,
            session("docs", "codex_cli", "gpt-5", 95, 0.01, "read_file"),
            session("mid", "pi", "gpt-5-mini", 70, 0.08, "grep"),
        ],
        "test",
        None,
    );

    assert!(!app.run_command("health crit").unwrap());
    assert_eq!(app.filtered.len(), 1);
    assert_eq!(
        app.selected_session().map(|s| s.name.as_str()),
        Some("costly")
    );

    assert!(!app.run_command("clear").unwrap());
    assert!(!app.run_command("source codex").unwrap());
    assert_eq!(
        app.selected_session().map(|s| s.name.as_str()),
        Some("docs")
    );

    assert!(!app.run_command("source Claude Code").unwrap());
    assert_eq!(
        app.selected_session().map(|s| s.name.as_str()),
        Some("costly")
    );

    assert!(!app.run_command("model mini").unwrap());
    assert_eq!(app.filtered.len(), 0);

    assert!(!app.run_command("clear").unwrap());
    assert!(!app.run_command("cost >0.10").unwrap());
    assert_eq!(
        app.selected_session().map(|s| s.name.as_str()),
        Some("costly")
    );

    assert!(!app.run_command("anomaly latency").unwrap());
    assert_eq!(
        app.selected_session().map(|s| s.name.as_str()),
        Some("costly")
    );

    assert!(!app.run_command("critical").unwrap());
    assert_eq!(app.health_filter, "crit");
}

#[test]
fn filters_by_data_capability_and_actionable_issue() {
    let mut detailed = session("detailed", "codex_cli", "gpt-5", 90, 0.1, "rg");
    detailed.metrics.tool_calls_fail = 1;
    let mut aggregate = session("aggregate", "hermes_db", "gpt-5", 90, 0.1, "rg");
    aggregate.metrics.gaps_sec.clear();
    let mut app = App::new(vec![detailed, aggregate], "test", None);

    assert!(!app.run_command("capability aggregate").unwrap());
    assert_eq!(app.filtered.len(), 1);
    assert_eq!(app.selected_session().unwrap().name, "aggregate");

    assert!(!app.run_command("clear").unwrap());
    assert!(!app.run_command("issues failures").unwrap());
    assert_eq!(app.filtered.len(), 1);
    assert_eq!(app.selected_session().unwrap().name, "detailed");
    assert!(active_filter_summary(&app).contains("issues=failures"));
}

#[test]
fn commands_apply_explicit_sort_direction_and_top_alias() {
    let mut app = App::new(
        vec![
            session("cheap", "pi", "m", 90, 0.01, "rg"),
            session("expensive", "codex_cli", "m", 90, 0.30, "rg"),
        ],
        "test",
        None,
    );

    assert!(!app.run_command("sort cost asc").unwrap());
    assert_eq!(
        app.selected_session().map(|s| s.name.as_str()),
        Some("cheap")
    );
    assert!(!app.run_command("top cost").unwrap());
    assert_eq!(
        app.selected_session().map(|s| s.name.as_str()),
        Some("expensive")
    );
    assert_eq!(app.sort_key, SortKey::Cost);
    assert!(app.sort_desc);

    assert!(!app.run_command("sort source asc").unwrap());
    assert_eq!(
        app.selected_session().map(|s| s.name.as_str()),
        Some("expensive")
    );
    assert_eq!(app.sort_key, SortKey::Source);
    assert!(!app.sort_desc);
}

#[test]
fn quick_filter_keys_match_go_keymap_semantics() {
    let mut app = App::new(
        vec![
            session("healthy", "codex_cli", "m", 95, 0.00, "read_file"),
            session("warning", "pi", "m", 70, 0.02, "rg"),
            session("critical", "claude_code", "m", 40, 0.50, "bash"),
        ],
        "test",
        None,
    );

    app.handle_normal_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE))
        .expect("health filter key");
    assert_eq!(app.health_filter, "good");
    assert_eq!(
        app.selected_session().map(|s| s.name.as_str()),
        Some("healthy")
    );

    app.handle_normal_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE))
        .expect("health filter key");
    assert_eq!(app.health_filter, "warn");
    assert_eq!(
        app.selected_session().map(|s| s.name.as_str()),
        Some("warning")
    );

    app.handle_normal_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        .expect("clear filters");
    let selected_source = app
        .selected_session()
        .map(|session| session.metrics.source_tool.clone())
        .expect("selected session source");
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE))
        .expect("source filter key");
    assert_eq!(app.source_filter, selected_source);

    app.handle_normal_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        .expect("clear filters");
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('$'), KeyModifiers::NONE))
        .expect("cost filter key");
    assert_eq!(app.filtered.len(), 2);
    assert!(app.status.contains("quick cost filter"));

    app.handle_normal_key(KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE))
        .expect("critical filter key");
    assert_eq!(app.health_filter, "crit");
    assert_eq!(
        app.selected_session().map(|s| s.name.as_str()),
        Some("critical")
    );
}

#[test]
fn go_navigation_keys_and_pairwise_diff_stay_compatible() {
    let mut app = App::new(
        vec![
            session("a", "codex_cli", "m1", 95, 0.01, "read_file"),
            session("b", "pi", "m2", 80, 0.02, "rg"),
            session("c", "pi", "m3", 40, 0.03, "bash"),
        ],
        "test",
        None,
    );

    assert!(app
        .handle_event(Event::Key(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        )))
        .unwrap());
    app.view = View::Overview;
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('`'), KeyModifiers::NONE))
        .unwrap();
    assert_eq!(app.view, View::List);
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE))
        .unwrap();
    assert_eq!(app.view, View::Help);
    assert!(help_text(app.help_context, app.language).contains("Current view: List"));
    app.handle_normal_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        .unwrap();
    assert_eq!(app.view, View::List);

    app.selected = 1;
    app.view = View::Diff;
    let visible = app.visible_sessions();
    let (left, right) = diff_pair(visible.len(), app.selected);
    let diff = diff_text(&app);
    assert!(diff.contains(&visible[left].name));
    assert!(diff.contains(&visible[right].name));
    let omitted = (0..visible.len())
        .find(|index| *index != left && *index != right)
        .unwrap();
    assert!(!diff.contains(&format!("{}          ", visible[omitted].name)));

    app.view = View::List;
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('S'), KeyModifiers::NONE))
        .unwrap();
    assert!(!app.source_filter.is_empty());
    app.clear_filters();
    app.refresh_filtered();
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('M'), KeyModifiers::NONE))
        .unwrap();
    assert!(!app.model_filter.is_empty());
}

#[test]
fn vim_page_and_end_navigation_clamp_selection_and_scroll_details() {
    let mut app = App::new(
        (0..12)
            .map(|index| {
                session(
                    &format!("session-{index}"),
                    "codex_cli",
                    "m",
                    90,
                    0.01,
                    "rg",
                )
            })
            .collect(),
        "test",
        None,
    );
    app.view = View::List;

    app.handle_normal_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL))
        .unwrap();
    assert_eq!(app.selected, 8);

    app.handle_normal_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL))
        .unwrap();
    assert_eq!(app.selected, 0);

    app.handle_normal_key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE))
        .unwrap();
    assert_eq!(app.selected, app.filtered.len() - 1);

    app.view = View::Detail;
    app.scroll = 0;
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL))
        .unwrap();
    assert_eq!(app.scroll, 8);
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL))
        .unwrap();
    assert_eq!(app.scroll, 0);
}

#[test]
fn sort_cost_descending_then_toggle() {
    let mut app = App::new(
        vec![
            session("cheap", "codex_cli", "m", 90, 0.01, "rg"),
            session("expensive", "codex_cli", "m", 90, 0.30, "rg"),
        ],
        "test",
        None,
    );

    app.set_sort(SortKey::Cost);
    assert_eq!(
        app.selected_session().map(|s| s.name.as_str()),
        Some("expensive")
    );
    app.set_sort(SortKey::Cost);
    assert_eq!(
        app.selected_session().map(|s| s.name.as_str()),
        Some("cheap")
    );
}

#[test]
fn overview_inspect_first_prioritizes_distinct_triage_entries() {
    let mut critical = session("critical", "codex_cli", "m", 40, 0.20, "bash");
    critical.metrics.tokens_input = 200;
    let mut anomalous = session("anomalous", "pi", "m", 70, 0.05, "rg");
    anomalous.anomalies.push(Anomaly {
        kind: "latency".to_string(),
        severity: "medium".to_string(),
        detail: "p95 gap".to_string(),
    });
    let mut failed = session("failed", "claude_code", "m", 85, 0.03, "read_file");
    failed.metrics.tool_calls_fail = 3;
    failed.metrics.tool_calls_total = 4;
    let mut costly = session("costly", "codex_cli", "m", 95, 1.40, "rg");
    costly.metrics.tokens_input = 3_000;
    let mut slow = session("slow", "codex_cli", "m", 95, 0.02, "rg");
    slow.metrics.duration_sec = 700.0;
    slow.metrics.gaps_sec = vec![5.0, 120.0, 240.0];
    let mut app = App::new(
        vec![critical, anomalous, failed, costly, slow],
        "testdata",
        None,
    );

    let items = inspect_first_items(&app.sessions);
    assert_eq!(items[0].label, "critical");
    assert_eq!(app.sessions[items[0].index].name, "critical");
    assert!(items.iter().any(|item| item.label == "anomaly"));
    assert!(items.iter().any(|item| item.label == "failures"));
    assert!(items.iter().any(|item| item.label == "cost"));
    assert!(items.iter().any(|item| item.label == "latency"));

    let backend = TestBackend::new(120, 42);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| render(frame, &mut app))
        .expect("render overview");
    let overview = format!("{:?}", terminal.backend().buffer());
    assert!(overview.contains("Inspect First"));
    assert!(overview.contains("critical"));
    assert!(overview.contains("critical health"));
    assert!(overview.contains("action: open diagnostics for critical"));

    app.view = View::Overview;
    app.handle_normal_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .expect("open top inspect item");
    assert_eq!(app.view, View::Diagnostics);
    assert_eq!(
        app.selected_session().map(|session| session.name.as_str()),
        Some("critical")
    );
}

#[test]
fn renders_overview_and_list_with_test_backend() {
    let mut billing = session("billing", "claude_code", "claude-sonnet-4", 80, 0.02, "rg");
    billing.cwd = "/work/storefront".to_string();
    billing.anomalies.push(Anomaly {
        kind: "latency".to_string(),
        severity: "medium".to_string(),
        detail: "p95 gap".to_string(),
    });
    let mut app = App::new(
        vec![
            billing,
            session("docs", "codex_cli", "gpt-5", 95, 0.01, "read_file"),
        ],
        "testdata",
        None,
    );
    let backend = TestBackend::new(100, 38);
    let mut terminal = Terminal::new(backend).expect("test terminal");

    terminal
        .draw(|frame| render(frame, &mut app))
        .expect("render overview");
    let overview = format!("{:?}", terminal.backend().buffer());
    assert!(overview.contains("AGENTTRACE"));
    assert!(overview.contains("idle"));
    assert!(overview.contains("Scoreboard"));
    assert!(overview.contains("Loading Status"));
    assert!(overview.contains("normal load"));
    assert!(overview.contains("0 cache hits"));
    assert!(overview.contains("tokens"));
    assert!(overview.contains("health"));
    assert!(overview.contains("p95"));
    assert!(overview.contains("Health Distribution"));
    assert!(overview.contains("Driver Distribution"));
    assert!(overview.contains("latency"));
    assert!(overview.contains("Inspect First"));
    assert!(overview.contains("Recent Sessions"));
    assert!(overview.contains("latency anomaly"));

    let wide_backend = TestBackend::new(196, 44);
    let mut wide_terminal = Terminal::new(wide_backend).expect("wide test terminal");
    wide_terminal
        .draw(|frame| render(frame, &mut app))
        .expect("render wide overview");
    let wide = format!("{:?}", wide_terminal.backend().buffer());
    assert!(wide.contains("Source  Claude Code"));
    assert!(wide.contains("Health Distribution"));
    assert!(wide.contains("Driver Distribution"));

    app.view = View::List;
    terminal
        .draw(|frame| render(frame, &mut app))
        .expect("render list");
    let list = format!("{:?}", terminal.backend().buffer());
    assert!(list.contains("Driver Summary"));
    assert!(list.contains("List Status"));
    assert!(list.contains("2/2 visible"));
    assert!(list.contains("filters: none"));
    assert!(list.contains("Enter detail"));
    assert!(list.contains("Loading Status"));
    assert!(list.contains("Idle"));
    assert!(list.contains("Source"));
    assert!(list.contains("Claude Code"));
    assert!(list.contains("Model"));
    assert!(list.contains("fail0"));
    assert!(list.contains("Selected Triage"));
    assert!(list.contains("selected: billing"));
    assert!(list.contains("ok=100%"));
    assert!(list.contains("reason=latency anomaly"));
    assert!(list.contains("p95 latency"));
    assert!(list.contains("Sessions"));
    assert!(list.contains("sort Recent desc"));
    assert!(list.contains("ok%"));
    assert!(list.contains("anom"));
    assert!(list.contains("reason"));
    assert!(list.contains("billing"));
    assert!(!list.contains("claude_code"));
    assert!(list.contains("q quit"));
    assert!(list.contains("filters"));
    assert!(list.contains("? help"));

    let long_name = "Investigate checkout latency";
    app.sessions[0].name = long_name.to_string();
    app.refresh_filtered();
    let wide_backend = TestBackend::new(196, 44);
    let mut wide_terminal = Terminal::new(wide_backend).expect("wide list terminal");
    wide_terminal
        .draw(|frame| render(frame, &mut app))
        .expect("render responsive list");
    let wide_list = format!("{:?}", wide_terminal.backend().buffer());
    assert!(wide_list.contains(long_name));
    assert!(wide_list.contains("Selected Detail"));
    app.sessions[0].name = "billing".to_string();
    app.refresh_filtered();

    app.view = View::Detail;
    terminal
        .draw(|frame| render(frame, &mut app))
        .expect("render detail");
    let detail = format!("{:?}", terminal.backend().buffer());
    assert!(detail.contains("Detail - billing"));
    assert!(detail.contains("reason=latency anomaly"));
    assert!(detail.contains("Session Summary"));
    assert!(detail.contains("Workspace: /work/storefront"));
    assert!(detail.contains("Session file:"));
    assert!(detail.contains("Context:"));
    assert!(detail.contains("health=80"));
    assert!(detail.contains("cost=$0.0200"));
    assert!(detail.contains("fail=0"));
    assert!(detail.contains("anom=1"));
    assert!(detail.contains("source=Claude Code"));
    assert!(detail.contains("Next Action"));
    assert!(!detail.contains("Raw report"));
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE))
        .expect("expand raw report");
    assert!(app.raw_report_expanded);
    assert!(detail_text(&app).contains("Raw report"));
    app.raw_report_expanded = false;

    let wide_backend = TestBackend::new(140, 38);
    let mut wide_terminal = Terminal::new(wide_backend).expect("wide detail terminal");
    wide_terminal
        .draw(|frame| render(frame, &mut app))
        .expect("render wide detail");
    let wide_detail = format!("{:?}", wide_terminal.backend().buffer());
    assert!(wide_detail.contains("Session Overview"));
    assert!(wide_detail.contains("Diagnosis"));
    assert!(wide_detail.contains("Press v to view the raw report"));

    app.view = View::Diagnostics;
    terminal
        .draw(|frame| render(frame, &mut app))
        .expect("render diagnostics");
    let diagnostics = format!("{:?}", terminal.backend().buffer());
    assert!(diagnostics.contains("Diagnostics - billing"));
    assert!(diagnostics.contains("reason=latency anomaly"));
    assert!(diagnostics.contains("Problem"));
    assert!(diagnostics.contains("Next"));
    assert!(diagnostics.contains("Evidence"));
    assert!(diagnostics.contains("Raw Signals"));
    assert!(diagnostics.contains("Context:"));
    assert!(diagnostics.contains("health=80"));
    assert!(diagnostics.contains("cost=$0.0200"));
    assert!(diagnostics.contains("fail=0"));
    assert!(diagnostics.contains("anom=1"));
    assert!(diagnostics.contains("source=Claude Code"));
    assert!(diagnostics_text(&app).contains("Raw diagnostics"));

    app.view = View::Diff;
    terminal
        .draw(|frame| render(frame, &mut app))
        .expect("render diff");
    let diff = format!("{:?}", terminal.backend().buffer());
    assert!(diff.contains("Diff - 2 visible"));
    assert!(diff.contains("sort Recent desc"));
    assert!(diff.contains("Context:"));
    assert!(diff.contains("visible=2"));
    assert!(diff.contains("filter=none"));
    assert!(diff_context_line(&app, 2).contains("top source=Claude Code:1"));

    app.language = Language::Zh;
    assert!(diff_context_line(&app, 2).contains("主要来源=Claude Code:1"));
    assert!(detail_native_text(app.selected_session().unwrap(), app.language).contains("Token"));
    assert!(!detail_native_text(app.selected_session().unwrap(), app.language).contains("令牌"));
    app.language = Language::En;

    app.help_context = View::Overview;
    app.view = View::Help;
    app.scroll = 0;
    terminal
        .draw(|frame| render(frame, &mut app))
        .expect("render help");
    let help = format!("{:?}", terminal.backend().buffer());
    assert!(help.contains("Triage workflow"));
    assert!(help.contains("enter on Overview"));
    assert!(help.contains("f cycles health"));
    assert!(help.contains("s selected source"));
    assert!(help.contains("$ costly sessions"));
    assert!(help.contains("! critical"));
    assert!(help.contains(":inspect [rank]"));
    assert!(help.contains(":health good|warn|crit|<80"));
    assert!(help.contains(":source <name>"));
    assert!(help.contains(":clear/:reset"));
    assert!(help.contains(":sort <field> [asc|desc]"));
}

#[test]
fn diff_empty_state_explains_active_filters() {
    let mut app = App::new(
        vec![session(
            "billing",
            "claude_code",
            "claude-sonnet-4",
            80,
            0.02,
            "rg",
        )],
        "testdata",
        None,
    );
    app.model_filter = "definitely-no-match".to_string();
    app.refresh_filtered();
    app.view = View::Diff;

    let backend = TestBackend::new(100, 18);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| render(frame, &mut app))
        .expect("render empty diff");
    let diff = format!("{:?}", terminal.backend().buffer());
    assert!(diff.contains("Context:"));
    assert!(diff.contains("visible=0"));
    assert!(diff.contains("filter=model=definitely-no-match"));
    assert!(diff.contains("Need at least two visible sessions for diff."));
    assert!(diff.contains("Active filters: model=definitely-no-match"));
    assert!(diff.contains("Press Esc or run :clear/:reset"));
}

#[test]
fn renders_no_visible_sessions_state_for_empty_filter_result() {
    let mut app = App::new(
        vec![session(
            "billing",
            "claude_code",
            "claude-sonnet-4",
            80,
            0.02,
            "rg",
        )],
        "testdata",
        None,
    );
    app.model_filter = "definitely-no-match".to_string();
    app.refresh_filtered();
    app.view = View::List;

    let backend = TestBackend::new(100, 18);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| render(frame, &mut app))
        .expect("render empty filter list");
    let list = format!("{:?}", terminal.backend().buffer());
    assert!(list.contains("No visible sessions match the active filters."));
    assert!(list.contains("Active filters: model=definitely-no-match"));
    assert!(list.contains("Press Esc or run :clear"));
}

#[test]
fn ctrl_r_force_reload_clears_session_cache_before_loading() {
    let root = std::env::temp_dir().join(format!(
        "agenttrace-rust-tui-force-reload-{}",
        std::process::id()
    ));
    let sessions_dir = root.join("sessions");
    let cache_dir = root.join("cache");
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");
    fs::create_dir_all(&cache_dir).expect("create cache dir");
    let session_path = sessions_dir.join("session.jsonl");
    fs::write(
            &session_path,
            r#"{"role":"user","content":"fresh","timestamp":"2026-05-02T10:00:00Z","SourceTool":"generic"}
{"role":"assistant","content":"fresh answer","timestamp":"2026-05-02T10:00:01Z","SourceTool":"generic"}
"#,
        )
        .expect("write session");
    let metadata = fs::metadata(&session_path).expect("session metadata");
    let cache_path = cache_dir.join("sessions.json");
    fs::write(
            &cache_path,
            format!(
                r#"{{"schema_version":16,"entries":{{"{}":{{"mod_time":{},"size":{},"session":{{"Name":"cached","Path":"{}","Metrics":{{"SourceTool":"hermes_jsonl","ModelUsed":"cached-model","SessionStart":"2026-05-02T09:00:00Z","ToolArgUsage":{{}}}},"Health":91,"ToolWarnings":[],"Diagnostics":{{}}}}}}}}}}"#,
                session_path.to_string_lossy(),
                file_mod_time_nanos_for_test(&metadata),
                metadata.len(),
                session_path.to_string_lossy()
            ),
        )
        .expect("write cache");

    with_session_cache_dir_for_test(&cache_dir, || {
        let mut app = App::new_loading("test", sessions_dir.to_string_lossy().to_string());
        assert_eq!(app.sessions.len(), 1);
        assert_eq!(app.sessions[0].name, "cached");
        assert!(app.load_state.showing_cached);
        assert_eq!(app.load_state.phase, LoadPhase::Discovering);
        assert_eq!(app.load_state.discovered, 0);
        assert_eq!(app.load_state.cache_hits, 0);
        assert!(app.status.contains("discovering session files"));
        let loading = loading_status_lines(&app)
            .iter()
            .map(|line| format!("{line:?}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(loading.contains("Discovering"));
        assert!(loading.contains("normal load"));
        assert!(loading.contains("loaded 0/0 files processed"));
        assert!(loading.contains("0 cache hits"));
        assert!(loading.contains("0%"));
        wait_for_progress(&mut app);
        assert!(matches!(
            app.load_state.phase,
            LoadPhase::Parsing | LoadPhase::Ready
        ));
        assert_eq!(app.load_state.processed, 1);
        assert_eq!(app.load_state.parsed, 1);
        assert_eq!(app.load_state.cache_hits, 1);
        assert_eq!(app.sessions[0].name, "cached");
        let progressed = loading_status_lines(&app)
            .iter()
            .map(|line| format!("{line:?}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(progressed.contains("100%"));
        wait_for_pending_load(&mut app);
        assert_eq!(app.sessions.len(), 1);
        assert_eq!(app.sessions[0].name, "cached");
        assert_eq!(app.load_state.phase, LoadPhase::Ready);
        assert_eq!(app.load_state.parsed, 1);
        assert!(load_summary_line(&app).contains("loaded 1 sessions"));
        assert!(load_summary_line(&app).contains("1 cache hits"));
        assert!(load_summary_line(&app).contains("hermes_jsonl:1"));
        assert!(cache_path.is_file());

        app.handle_normal_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL))
            .expect("force reload");
        assert!(app
            .status
            .contains("force reload: discovering session files"));
        assert_eq!(app.load_state.phase, LoadPhase::Discovering);
        assert_eq!(app.load_state.cache_hits, 0);
        let force_loading = loading_status_lines(&app)
            .iter()
            .map(|line| format!("{line:?}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(force_loading.contains("force reload"));
        assert!(force_loading.contains("0 cache hits"));
        assert!(force_loading.contains("cache bypass"));
        wait_for_progress(&mut app);
        assert_eq!(app.load_state.processed, 1);
        assert_eq!(app.load_state.cache_hits, 0);
        wait_for_pending_load(&mut app);
        assert_eq!(app.sessions.len(), 1);
        assert_eq!(app.sessions[0].name, "fresh");
        let refreshed_cache = fs::read_to_string(&cache_path).expect("read refreshed cache");
        assert!(refreshed_cache.contains(r#""Name":"fresh""#));
        assert!(!refreshed_cache.contains(r#""Name":"cached""#));
        assert!(app.status.starts_with("force reloaded 1 sessions"));
    });

    let _ = fs::remove_dir_all(root);
}

#[test]
fn startup_uses_cache_immediately_but_waits_without_cache() {
    let mut cached = App::new(
        vec![session("cached", "pi", "m", 90, 0.0, "rg")],
        "test",
        None,
    );
    cached.load_state.phase = LoadPhase::Parsing;
    cached.load_state.showing_cached = true;
    let backend = TestBackend::new(100, 24);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| render(frame, &mut cached))
        .expect("render cached");
    assert!(format!("{:?}", terminal.backend().buffer()).contains("Scoreboard"));

    let mut empty = App::new(Vec::new(), "test", None);
    empty.load_state.phase = LoadPhase::Parsing;
    let (_tx, rx) = mpsc::channel();
    empty.pending_load = Some(rx);
    terminal
        .draw(|frame| render(frame, &mut empty))
        .expect("render empty");
    let rendered = format!("{:?}", terminal.backend().buffer());
    assert!(rendered.contains("Loading Status"));
    assert!(!rendered.contains("Scoreboard"));
}

fn wait_for_pending_load(app: &mut App) {
    for _ in 0..50 {
        app.poll_pending_load();
        if app.pending_load.is_none() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("pending TUI load did not finish");
}

fn wait_for_progress(app: &mut App) {
    for _ in 0..50 {
        app.poll_pending_load();
        if app.load_state.processed > 0 {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("pending TUI load did not report progress");
}

fn session(name: &str, source: &str, model: &str, health: i32, cost: f64, tool: &str) -> Session {
    let mut metrics = Metrics {
        source_tool: source.to_string(),
        model_used: model.to_string(),
        session_start: format!("2026-05-02T10:00:0{}Z", name.len() % 10),
        assistant_turns: 2,
        tool_calls_total: 1,
        tool_calls_fail: if health < 80 { 1 } else { 0 },
        cost_estimated: cost,
        tokens_input: 10,
        tokens_output: 5,
        duration_sec: 125.0,
        gaps_sec: vec![2.0, 12.0, 40.0],
        ..Metrics::default()
    };
    metrics.tool_usage.insert(tool.to_string(), 1);
    Session {
        name: name.to_string(),
        path: format!("/tmp/{name}.jsonl"),
        cwd: "/tmp".to_string(),
        metrics,
        anomalies: Vec::new(),
        health,
        tool_warnings: Vec::new(),
        diagnostics: agenttrace_core::Diagnostics::default(),
    }
}

fn with_session_cache_dir_for_test(cache_dir: &std::path::Path, run: impl FnOnce()) {
    let previous = std::env::var_os("AGENTTRACE_SESSION_CACHE_DIR");
    std::env::set_var("AGENTTRACE_SESSION_CACHE_DIR", cache_dir);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run));
    match previous {
        Some(value) => std::env::set_var("AGENTTRACE_SESSION_CACHE_DIR", value),
        None => std::env::remove_var("AGENTTRACE_SESSION_CACHE_DIR"),
    }
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}

#[cfg(unix)]
fn file_mod_time_nanos_for_test(metadata: &fs::Metadata) -> i64 {
    use std::os::unix::fs::MetadataExt;
    metadata.mtime() * 1_000_000_000 + metadata.mtime_nsec()
}

#[cfg(not(unix))]
fn file_mod_time_nanos_for_test(metadata: &fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}
