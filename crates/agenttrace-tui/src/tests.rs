use super::*;
use agenttrace_core::{Anomaly, Metrics};
use crossterm::event::KeyModifiers;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::fs;

#[test]
fn explorer_navigation_reaches_views_details_and_overlays() {
    let mut app = App::new(
        vec![session("billing", "claude_code", "gpt-5", 45, 0.2, "bash")],
        "test",
        None,
    );
    assert_eq!(app.explorer_view, ExplorerView::Attention);

    app.handle_explorer_event(Event::Key(KeyEvent::new(
        KeyCode::Char('v'),
        KeyModifiers::NONE,
    )))
    .unwrap();
    assert_eq!(app.explorer_overlay, ExplorerOverlay::ViewPicker);
    for _ in 0..4 {
        app.handle_explorer_event(Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)))
            .unwrap();
    }
    app.handle_explorer_event(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )))
    .unwrap();
    assert_eq!(app.explorer_view, ExplorerView::Context);

    app.handle_explorer_event(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )))
    .unwrap();
    assert_eq!(app.explorer_detail, Some(DetailSection::Summary));
    app.handle_explorer_event(Event::Key(KeyEvent::new(
        KeyCode::Right,
        KeyModifiers::NONE,
    )))
    .unwrap();
    assert_eq!(app.explorer_detail, Some(DetailSection::Timeline));
    app.handle_explorer_event(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)))
        .unwrap();
    assert_eq!(app.explorer_detail, None);

    app.handle_explorer_event(Event::Key(KeyEvent::new(
        KeyCode::Char('k'),
        KeyModifiers::CONTROL,
    )))
    .unwrap();
    assert_eq!(app.explorer_overlay, ExplorerOverlay::Command);
}

#[test]
fn explorer_footer_shows_language_shortcut_in_list_and_detail() {
    let mut app = App::new(
        vec![session("language", "codex_cli", "gpt-5", 90, 0.1, "rg")],
        "test",
        None,
    );
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).expect("terminal");
    terminal
        .draw(|frame| render_explorer(frame, &mut app))
        .expect("render list footer");
    let list = format!("{:?}", terminal.backend().buffer());
    assert!(list.contains("l Language"));

    app.explorer_detail = Some(DetailSection::Summary);
    terminal
        .draw(|frame| render_explorer(frame, &mut app))
        .expect("render detail footer");
    let detail = format!("{:?}", terminal.backend().buffer());
    assert!(detail.contains("L Language"));

    app.handle_explorer_event(Event::Key(KeyEvent::new(
        KeyCode::Char('L'),
        KeyModifiers::NONE,
    )))
    .expect("switch language in detail");
    assert_eq!(app.language, Language::Zh);
    terminal
        .draw(|frame| render_explorer(frame, &mut app))
        .expect("render chinese footer");
    assert!(format!("{:?}", terminal.backend().buffer()).contains("L 切换语言"));
}

#[test]
fn explorer_renders_specialized_previews_and_real_file_size() {
    let path = std::env::temp_dir().join(format!(
        "agenttrace-explorer-{}-{}.jsonl",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    fs::write(&path, vec![b'x'; 2_048]).expect("write session file");
    let mut item = session("local", "codex_cli", "gpt-5", 70, 0.42, "rg");
    item.path = path.to_string_lossy().into_owned();
    item.metrics.tokens_input = 12_000;
    item.metrics.tool_calls_total = 3;
    item.metrics.tool_calls_ok = 2;
    item.metrics.tool_calls_fail = 1;
    item.diagnostics.context_utilization.utilization_pct = 88.0;
    item.diagnostics.context_utilization.risk_level = "warning".to_string();
    let mut app = App::new(vec![item], "test", None);
    let backend = TestBackend::new(140, 38);
    let mut terminal = Terminal::new(backend).expect("test terminal");

    for (view, expected) in [
        (ExplorerView::Context, "Context filling up"),
        (ExplorerView::Storage, "2.0 KB"),
        (ExplorerView::Cost, "Estimated spend"),
        (ExplorerView::Tools, "Tool trouble"),
    ] {
        app.explorer_view = view;
        terminal
            .draw(|frame| render_explorer(frame, &mut app))
            .expect("render explorer");
        let rendered = format!("{:?}", terminal.backend().buffer());
        assert!(
            rendered.contains(expected),
            "missing {expected}: {rendered}"
        );
        if view == ExplorerView::Cost {
            assert!(rendered.contains("Historical stored estimate"));
            assert!(rendered.contains("Current-rate estimate"));
            assert!(rendered.contains("Difference"));
        }
    }

    app.explorer_detail = Some(DetailSection::Timeline);
    terminal
        .draw(|frame| render_explorer(frame, &mut app))
        .expect("render timeline");
    assert!(format!("{:?}", terminal.backend().buffer())
        .contains("We didn't see a compaction event from this agent."));
    fs::remove_file(path).expect("remove session file");
}

#[test]
fn explorer_layout_uses_compact_standard_and_wide_space() {
    let mut item = session("responsive", "codex_cli", "gpt-5", 72, 0.42, "exec_command");
    item.diagnostics.steps.push(agenttrace_core::TraceStep {
        kind: "tool".to_string(),
        name: "exec_command with a long but useful step name".to_string(),
        started_at: "2026-05-20T06:10:45Z".to_string(),
        ended_at: "2026-05-20T06:10:47Z".to_string(),
        duration_sec: 2.0,
        status: "ok".to_string(),
        tokens: 0,
        call_id: "call-1".to_string(),
        parent_id: String::new(),
    });
    let mut app = App::new(vec![item], "test", None);

    let mut compact = Terminal::new(TestBackend::new(80, 24)).expect("compact terminal");
    compact
        .draw(|frame| render_explorer(frame, &mut app))
        .expect("render compact explorer");
    let compact_text = format!("{:?}", compact.backend().buffer());
    assert!(compact_text.contains("Look here first"));
    assert!(!compact_text.contains("Why look here"));

    let mut standard = Terminal::new(TestBackend::new(120, 30)).expect("standard terminal");
    standard
        .draw(|frame| render_explorer(frame, &mut app))
        .expect("render standard explorer");
    let standard_text = format!("{:?}", standard.backend().buffer());
    assert!(standard_text.contains("Why look here"));

    app.explorer_detail = Some(DetailSection::Timeline);
    let mut wide = Terminal::new(TestBackend::new(200, 50)).expect("wide terminal");
    wide.draw(|frame| render_explorer(frame, &mut app))
        .expect("render wide detail");
    let wide_text = format!("{:?}", wide.backend().buffer());
    assert!(wide_text.contains("Session at a glance"));
    assert!(wide_text.contains("2026-05-20T06:10:45Z"));
    assert!(wide_text.contains("exec_command with a long but useful step name"));
}

#[test]
fn explorer_daily_workflow_supports_attention_detail_compare_range_and_projects() {
    let mut healthy = session("healthy", "codex_cli", "gpt-5", 95, 0.05, "read_file");
    healthy.cwd = "/work/project-a".to_string();
    healthy.metrics.duration_sec = 20.0;
    healthy.metrics.gaps_sec.clear();
    let mut critical = session("critical", "claude_code", "gpt-5", 40, 1.2, "read_file");
    critical.cwd = "/work/project-a".to_string();
    critical
        .metrics
        .file_usage
        .insert("src/app.rs".to_string(), 4);
    let mut slow = session("slow", "pi", "gpt-5", 85, 0.2, "rg");
    slow.cwd = "/work/project-b".to_string();
    slow.metrics.duration_sec = 600.0;
    let mut app = App::new(vec![healthy, critical, slow], "test", None);

    let attention = app.explorer_indices();
    assert_eq!(attention.len(), 2);
    assert!(attention
        .iter()
        .all(|index| app.sessions[*index].name != "healthy"));

    app.handle_explorer_event(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )))
    .expect("open detail");
    let first = app.selected_session().expect("first detail").name.clone();
    app.scroll = 7;
    app.handle_explorer_event(Event::Key(KeyEvent::new(
        KeyCode::Char('j'),
        KeyModifiers::NONE,
    )))
    .expect("next detail session");
    assert_ne!(app.selected_session().expect("next detail").name, first);
    assert_eq!(app.scroll, 0);

    app.handle_explorer_event(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)))
        .expect("back to list");
    app.handle_explorer_event(Event::Key(KeyEvent::new(
        KeyCode::Char(' '),
        KeyModifiers::NONE,
    )))
    .expect("mark compare start");
    app.handle_explorer_event(Event::Key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)))
        .expect("pick compare target");
    app.handle_explorer_event(Event::Key(KeyEvent::new(
        KeyCode::Char('d'),
        KeyModifiers::NONE,
    )))
    .expect("open compare");
    assert!(app.compare_open);
    assert!(app.compare_sessions().is_some());

    app.compare_open = false;
    app.compare_anchor = None;
    app.explorer_view = ExplorerView::All;
    app.explorer_selected = app
        .explorer_indices()
        .iter()
        .position(|index| app.sessions[*index].name == "critical")
        .expect("critical position");
    app.handle_explorer_event(Event::Key(KeyEvent::new(
        KeyCode::Char('D'),
        KeyModifiers::NONE,
    )))
    .expect("compare with previous project run");
    assert!(app.compare_open);
    assert!(app.compare_sessions().is_some());

    app.compare_open = false;
    app.explorer_view = ExplorerView::Projects;
    let mut terminal = Terminal::new(TestBackend::new(160, 40)).expect("terminal");
    terminal
        .draw(|frame| render_explorer(frame, &mut app))
        .expect("render project view");
    let project = format!("{:?}", terminal.backend().buffer());
    assert!(project.contains("Project summary"));
    assert!(project.contains("sessions"));
    assert!(project.contains("need attention"));

    app.explorer_view = ExplorerView::All;
    app.explorer_selected = app
        .explorer_indices()
        .iter()
        .position(|index| app.sessions[*index].name == "critical")
        .expect("critical position");
    app.explorer_detail = Some(DetailSection::Summary);
    terminal
        .draw(|frame| render_explorer(frame, &mut app))
        .expect("render health explanation");
    assert!(format!("{:?}", terminal.backend().buffer()).contains("Health is affected by"));
    app.explorer_detail = Some(DetailSection::Files);
    terminal
        .draw(|frame| render_explorer(frame, &mut app))
        .expect("render repeated reads");
    assert!(format!("{:?}", terminal.backend().buffer()).contains("Possible repeated reads"));
}

#[test]
fn explorer_search_accepts_paste_and_filter_covers_context_risk() {
    let mut risky = session("risky", "codex_cli", "gpt-5", 70, 0.1, "rg");
    risky.diagnostics.context_utilization.risk_level = "critical".to_string();
    let mut app = App::new(
        vec![
            risky,
            session("healthy", "pi", "gpt-5", 95, 0.01, "read_file"),
        ],
        "test",
        None,
    );
    app.handle_explorer_event(Event::Key(KeyEvent::new(
        KeyCode::Char('/'),
        KeyModifiers::NONE,
    )))
    .unwrap();
    app.handle_explorer_event(Event::Paste("risky\n".to_string()))
        .unwrap();
    assert_eq!(app.query, "risky");
    assert_eq!(app.filtered.len(), 1);

    app.handle_explorer_event(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )))
    .unwrap();
    app.handle_explorer_event(Event::Key(KeyEvent::new(
        KeyCode::Char('k'),
        KeyModifiers::CONTROL,
    )))
    .unwrap();
    app.handle_explorer_event(Event::Paste("context risk".to_string()))
        .unwrap();
    app.handle_explorer_event(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )))
    .unwrap();
    assert_eq!(app.issue_filter, "context");
}

#[test]
fn explorer_search_understands_plain_structured_filters() {
    let mut costly = session("costly", "codex_cli", "gpt-5", 60, 2.0, "rg");
    costly.metrics.tool_calls_fail = 3;
    costly.diagnostics.context_utilization.utilization_pct = 90.0;
    let mut cheap = session("cheap", "pi", "gpt-5", 95, 0.1, "read_file");
    cheap.metrics.tool_calls_fail = 0;
    cheap.diagnostics.context_utilization.utilization_pct = 20.0;
    let mut app = App::new(vec![costly, cheap], "test", None);

    for (query, expected) in [
        ("cost > 1", "costly"),
        ("health < 70", "costly"),
        ("failed > 0", "costly"),
        ("context > 80", "costly"),
        ("source codex", "costly"),
    ] {
        app.clear_filters();
        app.input = query.to_string();
        app.apply_search_input();
        assert!(
            app.query.is_empty(),
            "structured query leaked into text search"
        );
        assert_eq!(app.filtered.len(), 1, "query={query}");
        assert_eq!(
            app.sessions[app.filtered[0]].name, expected,
            "query={query}"
        );
    }
}

#[test]
fn explorer_storage_lists_each_physical_file_once() {
    let mut first = session("first", "hermes_db", "gpt-5", 90, 0.1, "rg");
    let mut second = session("second", "hermes_db", "gpt-5", 90, 0.1, "rg");
    first.path = "/tmp/shared-state.db".to_string();
    second.path = first.path.clone();
    let mut app = App::new(vec![first, second], "test", None);
    app.explorer_view = ExplorerView::Storage;
    let backend = TestBackend::new(140, 38);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| render_explorer(frame, &mut app))
        .expect("render storage");
    let rendered = format!("{:?}", terminal.backend().buffer());
    assert_ne!(rendered.contains("first"), rendered.contains("second"));
}

#[test]
fn explorer_attention_uses_core_inspect_reason_and_cost_shows_provenance() {
    let mut critical = session("critical", "claude_code", "unknown", 20, 0.2, "bash");
    critical.metrics.tool_calls_fail = 0;
    let mut failing = session("failing", "codex_cli", "gpt-5", 80, 9.0, "rg");
    failing.metrics.tool_calls_fail = 4;
    let mut app = App::new(vec![failing, critical], "test", None);
    let backend = TestBackend::new(140, 38);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| render_explorer(frame, &mut app))
        .expect("render attention");
    let attention = format!("{:?}", terminal.backend().buffer());
    let critical_at = attention.find("critical").expect("critical session");
    let failing_at = attention.find("failing").expect("failing session");
    assert!(
        critical_at < failing_at,
        "inspect-first rank should put critical before failing: {attention}"
    );
    assert!(attention.contains("Why look here"));
    assert!(attention.contains("This session looks unhealthy"));

    app.explorer_view = ExplorerView::Cost;
    app.explorer_selected = 0;
    terminal
        .draw(|frame| render_explorer(frame, &mut app))
        .expect("render cost");
    let cost = format!("{:?}", terminal.backend().buffer());
    assert!(cost.contains("Estimated spend"));
    assert!(cost.contains("Price sources"));
    assert!(cost.contains("Current rates"));
    assert!(cost.contains("Stored estimate"));
    assert!(
        cost.contains("can't price this model yet") || cost.contains("priced from our model list"),
        "missing pricing status: {cost}"
    );
}

#[test]
fn chinese_workspace_copy_is_plain_language_and_width_aware() {
    let mut item = agenttrace_core::Recommendation {
        id: "tool-failures".to_string(),
        priority: "P1".to_string(),
        severity: "high".to_string(),
        category: "tool_failure".to_string(),
        title: "Reduce failing tool calls".to_string(),
        rationale: String::new(),
        evidence: Vec::new(),
        estimated_savings_usd: 0.0,
        estimated_savings_tokens: 0,
        confidence: "high".to_string(),
        action: "Inspect arguments and results".to_string(),
        validation_command: String::new(),
    };
    assert_eq!(
        recommendation_title(&item, Language::Zh),
        "减少失败的工具调用"
    );
    assert!(recommendation_action(&item, Language::Zh).contains("不要原样重试"));
    assert_eq!(short("中文宽度", 4), "...");
    assert_eq!(short("中文宽度", 5), "中...");
    item.id = "slow-tool".to_string();
    assert_eq!(
        recommendation_title(&item, Language::Zh),
        "给慢工具设定时间上限"
    );
    let mut app = App::new(Vec::new(), "test", None);
    app.model_filter = "gpt-5".to_string();
    app.issue_filter = "failures".to_string();
    assert_eq!(
        active_filter_summary(&app, Language::Zh),
        "模型: gpt-5 · 问题: 工具失败"
    );
}

#[test]
fn workspaces_render_filtered_reports_and_project_resolution() {
    let mut governed = session(
        "governed",
        "claude_code",
        "claude-sonnet-4",
        45,
        1.2,
        "mcp__demo__lookup",
    );
    governed.cwd = env!("CARGO_MANIFEST_DIR").to_string();
    governed.metrics.tokens_cache_r = 250;
    governed.metrics.tokens_output = 100;
    governed.metrics.tool_calls_fail = 1;
    governed.metrics.tool_calls_total = 2;
    governed
        .metrics
        .tool_authority
        .insert("write_files".to_string(), 1);
    governed.diagnostics.context_utilization.risk_level = "critical".to_string();
    governed.diagnostics.context_utilization.utilization_pct = 95.0;
    let mut app = App::new(vec![governed], "test", None);
    let backend = TestBackend::new(140, 44);
    let mut terminal = Terminal::new(backend).expect("test terminal");

    for (command, expected) in [("action", "Action Center"), ("efficiency", "Efficiency")] {
        app.run_command(command).expect("open governance view");
        assert!(matches!(app.view, View::Governance(_)));
        terminal
            .draw(|frame| render(frame, &mut app))
            .expect("render governance view");
        let rendered = format!("{:?}", terminal.backend().buffer());
        assert!(rendered.contains("Workspace"));
        assert!(rendered.contains(expected));
        assert!(rendered.contains("Projects"));
        assert!(rendered.contains("git_root"));
    }

    app.run_command("delivery")
        .expect("open delivery workspace");
    terminal
        .draw(|frame| render(frame, &mut app))
        .expect("start delivery workspace");
    let pending = format!("{:?}", terminal.backend().buffer());
    assert!(pending.contains("Correlating local Git commits"));
    for _ in 0..50 {
        app.poll_governance_delivery();
        if !app.governance_delivery_pending() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    terminal
        .draw(|frame| render(frame, &mut app))
        .expect("render delivery workspace");
    let delivery = format!("{:?}", terminal.backend().buffer());
    assert!(delivery.contains("Delivery evidence"));

    app.next_governance_panel();
    assert!(matches!(
        app.view,
        View::Governance(GovernancePanel::ActionCenter)
    ));
    app.view = View::List;
    app.explorer_detail = None;
    assert_eq!(app.view, View::List);
}

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
fn simplified_navigation_uses_three_areas_and_command_palette_key() {
    let mut app = App::new(
        vec![session("billing", "claude_code", "m", 70, 0.0, "rg")],
        "test",
        None,
    );
    assert_eq!(app.explorer_view, ExplorerView::Attention);

    app.handle_normal_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .expect("open session summary");
    assert_eq!(app.explorer_detail, Some(DetailSection::Summary));
    app.handle_normal_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE))
        .expect("open timeline");
    assert_eq!(app.explorer_detail, Some(DetailSection::Timeline));
    app.handle_normal_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        .expect("leave detail");
    assert_eq!(app.explorer_detail, None);

    app.handle_normal_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL))
        .expect("open command palette");
    assert_eq!(app.explorer_overlay, ExplorerOverlay::Command);
    for character in "project jk".chars() {
        app.handle_normal_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE))
            .expect("type command query");
    }
    assert_eq!(app.input, "project jk");
}

#[test]
fn live_search_paste_and_escape_restore_the_previous_filter() {
    let mut app = App::new(
        vec![
            session("billing", "claude_code", "claude-sonnet-4", 70, 0.02, "rg"),
            session("docs", "codex_cli", "gpt-5", 95, 0.01, "read_file"),
        ],
        "test",
        None,
    );
    app.query = "docs".to_string();
    app.refresh_filtered();

    app.handle_normal_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE))
        .expect("start search");
    assert_eq!(app.input, "docs");
    app.handle_event(Event::Paste("billing\n".to_string()))
        .expect("paste search");
    assert_eq!(app.query, "docsbilling");
    assert!(app.filtered.is_empty());

    app.handle_event(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)))
        .expect("cancel search");
    assert_eq!(app.query, "docs");
    assert_eq!(app.filtered.len(), 1);
    assert_eq!(
        app.selected_session().map(|session| session.name.as_str()),
        Some("docs")
    );
}

#[test]
fn live_search_escape_restores_structured_and_project_filters() {
    let mut first = session("first", "codex_cli", "gpt-5", 90, 2.0, "rg");
    first.cwd = "/tmp/foo/project".to_string();
    let mut second = session("second", "pi", "gpt-5", 90, 0.1, "rg");
    second.cwd = "/tmp/bar/project".to_string();
    let mut app = App::new(vec![first, second], "test", None);

    app.run_command("cost >1").expect("cost filter");
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE))
        .expect("start cost search");
    app.handle_normal_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        .expect("cancel cost search");
    assert_eq!(app.cost_filter, Some((CostOp::Gt, 1.0)));

    app.run_command("project foo").expect("project filter");
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE))
        .expect("start project search");
    app.handle_normal_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        .expect("cancel project search");
    assert_eq!(app.project_filter, "foo");

    app.project_id_filter = "/tmp/foo/project".to_string();
    app.refresh_filtered();
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE))
        .expect("start project id search");
    app.handle_event(Event::Paste("anything".to_string()))
        .expect("type project id search");
    app.handle_normal_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        .expect("cancel project id search");
    assert_eq!(app.project_id_filter, "/tmp/foo/project");

    app.clear_filters();
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE))
        .expect("start invalidation search");
    app.handle_event(Event::Paste("cost >1".to_string()))
        .expect("type valid cost expression");
    app.handle_normal_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE))
        .expect("make cost expression invalid");
    assert!(app.cost_filter.is_none());
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE))
        .expect("repair cost expression");
    app.handle_normal_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .expect("submit search");
    assert!(app.cost_filter.is_some());
    assert!(app.search_snapshot.is_none());
}

#[test]
fn project_view_filters_by_canonical_id_and_keeps_same_names_separate() {
    let mut left = session("left", "codex_cli", "gpt-5", 90, 0.1, "rg");
    left.cwd = "/tmp/alpha/project".to_string();
    let mut right = session("right", "codex_cli", "gpt-5", 90, 0.1, "rg");
    right.cwd = "/tmp/beta/project".to_string();
    let mut app = App::new(vec![left, right], "test", None);
    app.explorer_view = ExplorerView::Projects;
    let projects = app.explorer_indices();
    assert_eq!(projects.len(), 2);
    assert_ne!(
        resolve_project(&app.sessions[projects[0]]).id,
        resolve_project(&app.sessions[projects[1]]).id
    );
    app.explorer_selected = projects
        .iter()
        .position(|index| app.sessions[*index].name == "left")
        .expect("left project");
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE))
        .expect("filter project");
    assert_eq!(app.project_id_filter, "/tmp/alpha/project");
    assert_eq!(app.filtered.len(), 1);
    assert_eq!(app.sessions[app.filtered[0]].name, "left");
    assert_eq!(app.status, "project filter: project");
}

#[test]
fn previous_project_session_requires_time_and_uses_deterministic_ties() {
    let mut current = session("current", "codex_cli", "m", 90, 0.1, "rg");
    current.cwd = "/tmp/same-project".to_string();
    current.metrics.session_start = "2026-05-03T10:00:00Z".to_string();
    let mut tied_a = session("tie-a", "codex_cli", "m", 90, 0.1, "rg");
    tied_a.cwd = current.cwd.clone();
    tied_a.metrics.session_start = "2026-05-02T10:00:00Z".to_string();
    let mut tied_b = session("tie-b", "codex_cli", "m", 90, 0.1, "rg");
    tied_b.cwd = current.cwd.clone();
    tied_b.metrics.session_start = tied_a.metrics.session_start.clone();
    let mut no_time = session("no-time", "codex_cli", "m", 90, 0.1, "rg");
    no_time.cwd = current.cwd.clone();
    no_time.metrics.session_start.clear();
    let mut app = App::new(vec![current, tied_a, tied_b, no_time], "test", None);
    app.explorer_view = ExplorerView::All;
    app.explorer_selected = app
        .explorer_indices()
        .iter()
        .position(|index| app.sessions[*index].name == "current")
        .expect("current session");
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('D'), KeyModifiers::NONE))
        .expect("find previous session");
    assert_eq!(
        app.compare_sessions().expect("previous session")[0].name,
        "tie-b"
    );

    app.compare_open = false;
    app.compare_anchor = None;
    let current_index = app
        .sessions
        .iter()
        .position(|session| session.name == "current")
        .expect("current index");
    app.sessions[current_index].metrics.session_start = "2026-05-01T10:00:00Z".to_string();
    app.refresh_filtered();
    app.explorer_selected = app
        .explorer_indices()
        .iter()
        .position(|index| app.sessions[*index].name == "current")
        .expect("earliest current session");
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('D'), KeyModifiers::NONE))
        .expect("report earliest session");
    assert!(app.status.contains("earliest"));

    app.sessions[current_index].metrics.session_start.clear();
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('D'), KeyModifiers::NONE))
        .expect("report missing timestamp");
    assert!(app.status.contains("no timestamp"));
}

#[test]
fn auto_refresh_waits_ten_seconds_and_never_starts_concurrently() {
    let root = std::env::temp_dir().join(format!("agenttrace-auto-refresh-{}", std::process::id()));
    fs::create_dir_all(&root).expect("create reload dir");
    let mut app = App::new(
        vec![session("cached", "pi", "m", 90, 0.0, "rg")],
        "test",
        Some(root.to_string_lossy().into_owned()),
    );
    app.last_auto_refresh = Instant::now() - AUTO_REFRESH_INTERVAL - Duration::from_millis(1);
    assert!(app.poll_auto_refresh().expect("auto refresh"));
    assert!(app.pending_load.is_some());
    assert!(!app.poll_auto_refresh().expect("no concurrent auto refresh"));
    let refreshed_at = app.last_auto_refresh;
    app.start_reload(false);
    assert_eq!(app.last_auto_refresh, refreshed_at);

    let mut no_reload = App::new(vec![], "test", None);
    no_reload.last_auto_refresh = Instant::now() - AUTO_REFRESH_INTERVAL - Duration::from_secs(1);
    assert!(!no_reload.poll_auto_refresh().expect("no reload source"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn inspect_command_respects_active_filters() {
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

    assert!(!app.run_command("inspect 1").unwrap());
    assert_eq!(app.query, "critical");
    assert_eq!(app.project_filter, "in-scope");
    assert_eq!(app.range_filter, TimeRange::Days30);
    assert_eq!(app.view, View::Diagnostics);
    assert_eq!(
        app.selected_session().map(|session| session.name.as_str()),
        Some("critical")
    );
    assert!(app.status.contains("inspect"));
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
    app.handle_normal_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .expect("open top attention item");

    assert_eq!(app.explorer_detail, Some(DetailSection::Summary));
    assert_eq!(
        app.selected_session().map(|session| session.name.as_str()),
        Some("critical")
    );
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
    assert_eq!(app.status, "语言：中文（已保存）");
    assert!(help_text(View::Overview, app.language).contains("分诊流程"));

    let backend = TestBackend::new(100, 24);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    app.view = View::Overview;
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
    assert_eq!(app.status, "language: English (saved)");
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
    assert!(active_filter_summary(&app, Language::En).contains("issue: tool failures"));
    assert!(active_filter_summary(&app, Language::Zh).contains("问题: 工具失败"));
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
        .expect("open filter overlay");
    assert_eq!(app.explorer_overlay, ExplorerOverlay::Filter);
    app.handle_normal_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .expect("apply health filter");
    assert_eq!(app.health_filter, "good");
    assert_eq!(
        app.selected_session().map(|s| s.name.as_str()),
        Some("healthy")
    );

    app.handle_normal_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE))
        .expect("open filter overlay");
    app.handle_normal_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .expect("cycle health filter");
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
    assert_eq!(
        app.selected_session()
            .map(|session| session.metrics.source_tool.as_str()),
        Some(selected_source.as_str())
    );

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
fn reload_missing_selection_returns_to_list() {
    let selected = session("selected", "codex_cli", "m", 95, 0.1, "rg");
    let remaining = session("remaining", "pi", "m", 80, 0.1, "rg");
    let mut app = App::new(vec![selected, remaining.clone()], "test", None);
    app.explorer_view = ExplorerView::All;
    app.explorer_selected = app
        .filtered
        .iter()
        .position(|index| app.sessions[*index].name == "selected")
        .unwrap();
    app.selected = app.explorer_selected;
    app.explorer_detail = Some(DetailSection::Summary);
    app.view = View::Detail;
    app.apply_loaded_sessions(
        LoadReport {
            sessions: vec![remaining],
            discovered: 1,
            parsed: 1,
            ..LoadReport::default()
        },
        false,
    );
    assert_eq!(app.view, View::List);
    assert!(app.status.contains("no longer available"));
    assert_eq!(app.derived.health.discovered, app.derived.health.parsed);
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
    app.handle_normal_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE))
        .unwrap();
    assert_eq!(app.explorer_overlay, ExplorerOverlay::Help);
    app.handle_normal_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        .unwrap();
    assert_eq!(app.explorer_overlay, ExplorerOverlay::None);

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
    app.explorer_view = ExplorerView::All;

    app.handle_normal_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL))
        .unwrap();
    assert_eq!(app.explorer_selected, 8);

    app.handle_normal_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL))
        .unwrap();
    assert_eq!(app.explorer_selected, 0);

    app.handle_normal_key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE))
        .unwrap();
    assert_eq!(app.explorer_selected, app.explorer_indices().len() - 1);

    app.explorer_detail = Some(DetailSection::Summary);
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

    app.move_next();
    let selected = app.selected_session().unwrap().name.clone();
    app.set_sort(SortKey::Cost);
    assert_eq!(
        app.selected_session().map(|s| s.name.as_str()),
        Some(selected.as_str())
    );
    app.set_sort(SortKey::Cost);
    assert_eq!(
        app.selected_session().map(|s| s.name.as_str()),
        Some(selected.as_str())
    );
}

#[test]
fn inspect_first_uses_all_active_filters() {
    let mut app = App::new(
        vec![session("critical", "codex_cli", "m", 35, 0.20, "bash")],
        "test",
        None,
    );
    app.query = "no-match".to_string();
    app.refresh_filtered();
    assert!(app.derived.inspect_first.is_empty());
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

    let backend = TestBackend::new(140, 38);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| render_explorer(frame, &mut app))
        .expect("render attention");
    let overview = format!("{:?}", terminal.backend().buffer());
    assert!(overview.contains("Look here first"));
    assert!(overview.contains("critical"));
    assert!(overview.contains("Why look here"));
    assert!(overview.contains("This session looks unhealthy"));

    app.handle_normal_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .expect("open top inspect item");
    assert_eq!(app.explorer_detail, Some(DetailSection::Summary));
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
    app.view = View::Overview;

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
    assert!(wide.contains("List Status"));
    assert!(wide.contains("Sessions - 2 visible"));
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
    assert!(wide_list.contains("List Status"));
    assert!(wide_list.contains("Selected Triage"));
    assert!(wide_list.contains("Driver Summary"));
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
    app.raw_report_expanded = true;
    assert!(app.raw_report_expanded);
    assert!(detail_text(&app).contains("Raw report"));
    app.raw_report_expanded = false;

    let wide_backend = TestBackend::new(140, 38);
    let mut wide_terminal = Terminal::new(wide_backend).expect("wide detail terminal");
    wide_terminal
        .draw(|frame| render(frame, &mut app))
        .expect("render wide detail");
    let wide_detail = format!("{:?}", wide_terminal.backend().buffer());
    assert!(wide_detail.contains("Sessions - 2 visible"));
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
    assert!(help.contains("Start in Sessions"));
    assert!(help.contains("Tab switches Sessions"));
    assert!(help.contains("Ctrl+K"));
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
    assert!(diff.contains("filter=model: definitely-no-match"));
    assert!(diff.contains("Need at least two visible sessions for diff."));
    assert!(diff.contains("Active filters: model: definitely-no-match"));
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
    assert!(list.contains("Active filters: model: definitely-no-match"));
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
    let session_path_json = format!(
        "\"{}\"",
        session_path
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('\"', "\\\"")
    );
    fs::write(
            &cache_path,
            format!(
                r#"{{"schema_version":17,"entries":{{{0}:{{"mod_time":{1},"size":{2},"session":{{"Name":"cached","Path":{0},"Metrics":{{"SourceTool":"hermes_jsonl","ModelUsed":"cached-model","SessionStart":"2026-05-02T09:00:00Z","ToolArgUsage":{{}}}},"Health":91,"ToolWarnings":[],"Diagnostics":{{}}}}}}}}}}"#,
                session_path_json,
                file_mod_time_nanos_for_test(&metadata),
                metadata.len()
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
    cached.view = View::Overview;
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
