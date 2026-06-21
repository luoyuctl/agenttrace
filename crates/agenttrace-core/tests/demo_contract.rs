use agenttrace_core::{
    add_baseline_comparison, compute_overview, demo_sessions, evaluate_overview_gate,
    report_json_with_language, report_overview_json, search_sessions, BaselineThresholds, Metrics,
    ReportLanguage, Session, VERSION,
};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;

#[test]
fn demo_overview_exposes_ci_contract_fields() {
    let sessions = demo_sessions().expect("demo sessions parse");
    let overview = compute_overview(&sessions);
    let report: Value =
        serde_json::from_str(&report_overview_json(&overview, &sessions)).expect("valid json");

    assert_eq!(report["version"], VERSION);
    assert_eq!(report["summary"]["total_sessions"], 3);
    assert_eq!(
        report["summary"]["tool_authority"]["highest"],
        "test_or_build"
    );
    assert!(report["summary"]["total_duration_seconds"]
        .as_f64()
        .is_some());
    assert!(report["summary"]["total_cost"].as_f64().unwrap() > 0.0);
    assert!(report["recent_sessions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|session| session["possible_cost_driver"]
            .as_str()
            .unwrap_or("")
            .contains("possible driver")));
    assert!(report["surfaces"]["authority_categories"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item == "test_or_build"));
    assert!(report["by_project"].is_array());
}

#[test]
fn range_and_project_filters_share_one_session_scope() {
    let mut sessions = demo_sessions().expect("demo sessions parse");
    sessions[0].cwd = "/work/alpha".to_string();
    sessions[1].cwd = "/work/beta".to_string();
    sessions[2].cwd = "/work/alpha".to_string();
    let now = chrono::DateTime::parse_from_rfc3339("2026-05-03T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let filtered = agenttrace_core::filter_sessions(
        &sessions,
        agenttrace_core::TimeRange::Days7,
        "alpha",
        "",
        "",
        now,
    );
    assert!(filtered
        .iter()
        .all(|session| session.cwd.ends_with("alpha")));
}

#[test]
fn demo_search_returns_metadata_evidence() {
    let sessions = demo_sessions().expect("demo sessions parse");
    let results = search_sessions(&sessions, "internal/ws", 20);
    assert_eq!(results.len(), 1);
    assert!(results[0]
        .matches
        .iter()
        .any(|item| item.contains("internal/ws")));
}

#[test]
fn overview_high_authority_tools_follow_go_classifier() {
    let metrics = Metrics {
        tool_usage: BTreeMap::from([
            ("bash".to_string(), 1),
            ("read_file".to_string(), 1),
            ("terminal".to_string(), 1),
            ("write_file".to_string(), 1),
        ]),
        tool_authority: BTreeMap::from([
            ("read_only_files".to_string(), 1),
            ("shell_exec".to_string(), 1),
            ("write_files".to_string(), 1),
        ]),
        highest_authority: "shell_exec".to_string(),
        ..Metrics::default()
    };

    let sessions = vec![Session {
        name: "authority".to_string(),
        path: "/tmp/authority.jsonl".to_string(),
        cwd: String::new(),
        metrics,
        anomalies: Vec::new(),
        health: 100,
        tool_warnings: Vec::new(),
        diagnostics: agenttrace_core::Diagnostics::default(),
    }];
    let overview = compute_overview(&sessions);
    let report: Value =
        serde_json::from_str(&report_overview_json(&overview, &sessions)).expect("valid json");
    let tools = report["surfaces"]["high_authority_tools"]
        .as_array()
        .expect("high authority tools");

    assert!(tools.iter().any(|item| item == "bash"));
    assert!(tools.iter().any(|item| item == "terminal"));
    assert!(tools.iter().any(|item| item == "write_file"));
    assert!(!tools.iter().any(|item| item == "read_file"));
}

#[test]
fn demo_latest_json_supports_zh_language_slice() {
    let sessions = demo_sessions().expect("demo sessions parse");
    let latest = sessions
        .iter()
        .max_by(|a, b| a.metrics.session_start.cmp(&b.metrics.session_start))
        .expect("latest demo session");
    let report: Value =
        serde_json::from_str(&report_json_with_language(latest, ReportLanguage::Zh))
            .expect("valid zh json");

    assert_eq!(report["session"]["duration_human"], "40秒");
    assert_eq!(
        report["anomalies"][0]["detail"],
        "平均推理 = 11 字符 (极浅)"
    );
    assert_eq!(report["anomalies"][1]["detail"], "无工具调用 — 纯对话会话");
}

#[test]
fn demo_baseline_comparison_is_stable_for_identical_report() {
    let sessions = demo_sessions().expect("demo sessions parse");
    let overview = compute_overview(&sessions);
    let report = report_overview_json(&overview, &sessions);
    let path = std::env::temp_dir().join(format!(
        "agenttrace-rust-baseline-{}.json",
        std::process::id()
    ));
    fs::write(&path, &report).expect("write baseline");
    let compared: Value = serde_json::from_str(
        &add_baseline_comparison(
            &report,
            path.to_str().unwrap(),
            BaselineThresholds {
                max_duration_delta_pct: 1.5,
                max_cost_delta_pct: 2.5,
                max_token_delta_pct: 3.5,
            },
        )
        .expect("baseline compare"),
    )
    .expect("valid compared json");
    let _ = fs::remove_file(path);

    assert_eq!(
        compared["baseline_comparison"]["thresholds"]["max_duration_delta_pct"],
        1.5
    );
    assert_eq!(compared["baseline_comparison"]["cost_delta_pct"], 0.0);
    assert_eq!(compared["baseline_comparison"]["token_delta_pct"], 0.0);
    assert_eq!(
        compared["baseline_comparison"]["cost_above_threshold"],
        false
    );
    assert_eq!(
        compared["baseline_comparison"]["tokens_above_threshold"],
        false
    );
    assert!(compared["baseline_comparison"]["current"]["tools"].is_array());
    assert!(compared["baseline_comparison"]["new_tools"].is_array());
    assert!(compared["baseline_comparison"]["new_high_authority_tool_use"].is_array());
    assert_eq!(
        compared["baseline_comparison"]["slower_than_baseline"],
        false
    );
}

#[test]
fn demo_gate_fails_like_current_contract() {
    let sessions = demo_sessions().expect("demo sessions parse");
    let overview = compute_overview(&sessions);
    let failures = evaluate_overview_gate(&overview, &sessions, 80, true, Some(15.0));
    assert!(!failures.is_empty());
}
