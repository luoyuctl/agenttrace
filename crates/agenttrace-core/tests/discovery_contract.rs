use agenttrace_core::{
    build_doctor_report, find_session_files, load_sessions_from_dir, load_sessions_with_progress,
    parse_file, render_waste_report, search_sessions, session_cache_path, session_capability,
    LoadOptions,
};
use rusqlite::Connection;
use serde_json::Value;
use std::fs;
use std::sync::{Mutex, OnceLock};

const SAMPLE_JSONL: &str = r#"{"role":"session_meta","timestamp":"2026-05-02T10:00:00Z","ModelUsed":"claude-sonnet-4"}
{"role":"meta","ModelUsed":"claude-sonnet-4","Usage":{"input_tokens":1000,"output_tokens":500}}
{"role":"user","content":"Inspect billing export.","timestamp":"2026-05-02T10:00:00Z","ModelUsed":"claude-sonnet-4"}
{"role":"assistant","content":"I will inspect the route.","timestamp":"2026-05-02T10:00:01Z","reasoning":"Find the route and keep the change small.","tool_calls":[{"id":"t1","name":"rg","args":"billing export"}],"ModelUsed":"claude-sonnet-4"}
{"role":"tool","content":"{\"success\":true}","tool_call_id":"t1","timestamp":"2026-05-02T10:00:02Z"}
"#;

fn generated_fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("testdata/generated")
        .join(name)
}

#[test]
fn generated_capability_and_step_fixtures_cover_degradation() {
    let detailed = parse_file(&generated_fixture("detailed-tool-steps.jsonl")).unwrap();
    assert_eq!(session_capability(&detailed), "detailed");
    assert_eq!(detailed.diagnostics.steps.len(), 3);
    assert_eq!(detailed.diagnostics.steps[0].status, "ok");
    assert_eq!(detailed.diagnostics.steps[1].status, "error");
    assert_eq!(detailed.diagnostics.steps[2].status, "missing");
}

#[test]
fn generated_sql_builds_an_aggregate_only_session() {
    let root = temp_root("agenttrace-generated-aggregate");
    let home = root.join("home");
    let db_path = home.join(".hermes/state.db");
    fs::create_dir_all(db_path.parent().unwrap()).unwrap();
    let db = Connection::open(&db_path).unwrap();
    db.execute_batch(&fs::read_to_string(generated_fixture("aggregate-session.sql")).unwrap())
        .unwrap();
    drop(db);
    with_home(&home, || {
        let sessions = agenttrace_core::load_sqlite_backed_sessions();
        let aggregate = sessions
            .iter()
            .find(|session| session.name == "aggregate")
            .unwrap();
        assert_eq!(session_capability(aggregate), "aggregate");
        assert!(aggregate.diagnostics.steps.is_empty());
        let limited = sessions
            .iter()
            .find(|session| session.name == "limited")
            .unwrap();
        assert_eq!(session_capability(limited), "limited");
        assert!(limited.diagnostics.steps.is_empty());
    });
    let _ = fs::remove_dir_all(root);
}

#[test]
fn generated_steps_never_serialize_content_or_arguments() {
    let session = parse_file(&generated_fixture("tool-step-redaction.jsonl")).unwrap();
    let json = serde_json::to_string(&session.diagnostics.steps).unwrap();
    assert!(!json.contains("SHOULD_NOT_LEAK"));
}

#[test]
fn generated_provider_fixtures_stay_parseable() {
    for (name, source) in [
        ("workbuddy.jsonl", "workbuddy"),
        ("antigravity.jsonl", "antigravity_cli"),
        ("copilot-session.jsonl", "copilot_cli"),
        ("kimi-wire.jsonl", "kimi_cli"),
        ("openclaw-wrapper.json", "openclaw"),
        ("qwen-stream.jsonl", "qwen_code"),
    ] {
        let parsed =
            parse_file(&generated_fixture(name)).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(parsed.metrics.source_tool, source, "{name}");
    }

    let root = temp_root("agenttrace-generated-pi");
    let path = root.join(".pi/agent/sessions/pi-session.jsonl");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::copy(generated_fixture("pi-session.jsonl"), &path).unwrap();
    assert_eq!(parse_file(&path).unwrap().metrics.source_tool, "pi");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_discovers_and_loads_real_jsonl_files() {
    let root =
        std::env::temp_dir().join(format!("agenttrace-rust-discovery-{}", std::process::id()));
    let nested = root.join("nested");
    fs::create_dir_all(&nested).expect("create test dir");
    let session_path = nested.join("session.jsonl");
    let ignored_path = nested.join("sessions.json");
    fs::write(&session_path, SAMPLE_JSONL).expect("write session");
    fs::write(&ignored_path, SAMPLE_JSONL).expect("write ignored cache");

    let mut loaded_health = None;
    with_session_cache(&root.join("cache"), || {
        let files = find_session_files(Some(&root));
        assert_eq!(files, vec![session_path.clone()]);

        let sessions = load_sessions_from_dir(Some(&root));
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].name, "Inspect billing export.");
        assert_eq!(sessions[0].metrics.model_used, "claude-sonnet-4");
        assert_eq!(sessions[0].metrics.source_tool, "hermes_jsonl");
        assert_eq!(sessions[0].metrics.tool_calls_total, 1);
        loaded_health = Some(sessions[0].health);
    });

    let parsed = parse_file(&session_path).expect("parse single file");
    assert_eq!(parsed.health, loaded_health.expect("loaded session health"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_parses_workbuddy_messages_tools_usage_and_millis() {
    let root = temp_root("agenttrace-rust-workbuddy");
    fs::create_dir_all(&root).expect("create workbuddy temp dir");
    let session_path = root.join("session.jsonl");
    fs::write(
        &session_path,
        r#"{"type":"message","role":"user","content":[{"type":"input_text","text":"inspect"}],"timestamp":1783777800000,"sessionId":"s1","cwd":"/tmp/project","providerData":{"agent":"cli"}}
{"type":"reasoning","content":[],"rawContent":[{"type":"reasoning_text","text":"check first"}],"timestamp":1783777801000,"sessionId":"s1","cwd":"/tmp/project","providerData":{"model":"glm-5.2","agent":"cli"}}
{"type":"function_call","name":"Read","callId":"c1","arguments":"{\"path\":\"a.rs\"}","timestamp":1783777802000,"sessionId":"s1","cwd":"/tmp/project","message":{"usage":{"input_tokens":100,"output_tokens":20,"cache_read_input_tokens":60}},"providerData":{"model":"glm-5.2","agent":"cli"}}
{"type":"function_call_result","name":"Read","callId":"c1","status":"completed","output":{"type":"text","text":"ok"},"timestamp":1783777803000,"sessionId":"s1","cwd":"/tmp/project","providerData":{"model":"glm-5.2","agent":"cli"}}
{"type":"message","role":"assistant","content":[{"type":"output_text","text":"done"}],"timestamp":1783777804000,"sessionId":"s1","cwd":"/tmp/project","message":{"usage":{"input_tokens":120,"output_tokens":30,"cache_read_input_tokens":80}},"providerData":{"model":"glm-5.2","agent":"cli"}}
"#,
    )
    .expect("write workbuddy session");

    let parsed = parse_file(&session_path).expect("parse workbuddy session");
    assert_eq!(parsed.cwd, "/tmp/project");
    assert_eq!(parsed.metrics.source_tool, "workbuddy");
    assert_eq!(parsed.metrics.model_used, "glm-5.2");
    assert_eq!(parsed.metrics.tool_calls_total, 1);
    assert_eq!(parsed.metrics.tool_calls_ok, 1);
    assert_eq!(parsed.metrics.reasoning_blocks, 1);
    assert_eq!(parsed.metrics.tokens_input, 40);
    assert_eq!(parsed.metrics.tokens_output, 30);
    assert_eq!(parsed.metrics.tokens_cache_r, 80);
    assert_eq!(parsed.metrics.session_start, "2026-07-11T13:50:00Z");
    assert_eq!(parsed.metrics.session_end, "2026-07-11T13:50:04Z");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_parses_antigravity_cli_transcript() {
    let root = temp_root("agenttrace-rust-antigravity");
    fs::create_dir_all(&root).expect("create antigravity temp dir");
    let path = root.join("transcript.jsonl");
    fs::write(
        &path,
        r#"{"step_index":0,"source":"USER_EXPLICIT","type":"USER_INPUT","status":"DONE","created_at":"2026-05-19T19:33:40Z","content":"inspect"}
{"step_index":1,"source":"MODEL","type":"PLANNER_RESPONSE","status":"DONE","created_at":"2026-05-19T19:33:41Z","thinking":"check","tool_calls":[{"name":"view_file","args":{"AbsolutePath":"a.rs"}}]}
{"step_index":2,"source":"MODEL","type":"VIEW_FILE","status":"DONE","created_at":"2026-05-19T19:33:42Z","content":"ok"}
"#,
    )
    .expect("write antigravity transcript");

    let parsed = parse_file(&path).expect("parse antigravity transcript");
    assert_eq!(parsed.metrics.source_tool, "antigravity_cli");
    assert_eq!(parsed.metrics.tool_calls_total, 1);
    assert_eq!(parsed.metrics.tool_calls_ok, 1);
    assert_eq!(parsed.metrics.reasoning_blocks, 1);
    assert_eq!(parsed.metrics.session_start, "2026-05-19T19:33:40Z");
    assert_eq!(parsed.metrics.session_end, "2026-05-19T19:33:42Z");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_parses_cursor_agent_transcript() {
    let root = temp_root("agenttrace-rust-cursor-transcript");
    fs::create_dir_all(&root).expect("create cursor temp dir");
    let path = root.join("session.jsonl");
    fs::write(
        &path,
        r#"{"role":"user","message":{"content":[{"type":"text","text":"inspect"}]}}
{"role":"assistant","message":{"content":[{"type":"text","text":"checking"},{"type":"tool_use","name":"Read","input":{"path":"a.rs"}}]}}
"#,
    )
    .expect("write cursor transcript");

    let parsed = parse_file(&path).expect("parse cursor transcript");
    assert_eq!(parsed.metrics.source_tool, "cursor");
    assert_eq!(parsed.metrics.tool_calls_total, 1);
    assert_eq!(parsed.metrics.user_messages, 1);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_parses_claude_flat_transcript() {
    let root = temp_root("agenttrace-rust-claude-transcript");
    fs::create_dir_all(&root).expect("create claude temp dir");
    let path = root.join("session.jsonl");
    fs::write(
        &path,
        r#"{"type":"user","timestamp":"2026-03-19T11:21:41Z","content":"inspect"}
{"type":"tool_use","timestamp":"2026-03-19T11:21:42Z","tool_name":"read","tool_input":{"path":"a.rs"}}
{"type":"tool_result","timestamp":"2026-03-19T11:21:43Z","tool_name":"read","tool_output":"ok"}
"#,
    )
    .expect("write claude transcript");

    let parsed = parse_file(&path).expect("parse claude transcript");
    assert_eq!(parsed.metrics.source_tool, "claude_code");
    assert_eq!(parsed.metrics.tool_calls_total, 1);
    assert_eq!(parsed.metrics.tool_calls_ok, 1);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_parses_copilot_session_state() {
    let root = temp_root("agenttrace-rust-copilot-session");
    fs::create_dir_all(&root).expect("create copilot temp dir");
    let path = root.join("events.jsonl");
    fs::write(
        &path,
        r#"{"type":"session.start","timestamp":"2026-05-07T10:00:00Z","data":{"context":{"cwd":"/tmp/copilot"}}}
{"type":"user.message","timestamp":"2026-05-07T10:00:01Z","data":{"content":"inspect"}}
{"type":"tool.execution_start","timestamp":"2026-05-07T10:00:02Z","data":{"toolName":"Read","toolCallId":"c1","arguments":{"path":"a.rs"}}}
{"type":"tool.execution_complete","timestamp":"2026-05-07T10:00:03Z","data":{"toolCallId":"c1","success":true}}
{"type":"session.shutdown","timestamp":"2026-05-07T10:00:04Z","data":{"modelMetrics":{"gpt-5.4":{"usage":{"inputTokens":100,"outputTokens":20,"cacheReadTokens":40}}}}}
"#,
    )
    .expect("write copilot session");

    let parsed = parse_file(&path).expect("parse copilot session");
    assert_eq!(parsed.cwd, "/tmp/copilot");
    assert_eq!(parsed.metrics.source_tool, "copilot_cli");
    assert_eq!(parsed.metrics.model_used, "gpt-5.4");
    assert_eq!(parsed.metrics.tool_calls_total, 1);
    assert_eq!(parsed.metrics.tool_calls_ok, 1);
    assert_eq!(parsed.metrics.tokens_input, 100);
    assert_eq!(parsed.metrics.tokens_cache_r, 40);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_parses_kimi_wire_session() {
    let root = temp_root("agenttrace-rust-kimi-wire");
    fs::create_dir_all(&root).expect("create kimi temp dir");
    let path = root.join("wire.jsonl");
    fs::write(
        &path,
        r#"{"type":"metadata","protocol_version":"2"}
{"timestamp":1770000000.0,"message":{"type":"TurnBegin","payload":{"user_input":"inspect"}}}
{"timestamp":1770000001.0,"message":{"type":"ThinkPart","payload":{"text":"check"}}}
{"timestamp":1770000002.0,"message":{"type":"ToolCall","payload":{"id":"c1","name":"Read","arguments":{"path":"a.rs"}}}}
{"timestamp":1770000003.0,"message":{"type":"ToolResult","payload":{"id":"c1","result":"ok"}}}
{"timestamp":1770000004.0,"message":{"type":"StatusUpdate","payload":{"token_usage":{"inputTokens":100,"outputTokens":20,"cacheReadInputTokens":40}}}}
"#,
    )
    .expect("write kimi wire session");

    let parsed = parse_file(&path).expect("parse kimi wire session");
    assert_eq!(parsed.metrics.source_tool, "kimi_cli");
    assert_eq!(parsed.metrics.tool_calls_total, 1);
    assert_eq!(parsed.metrics.tool_calls_ok, 1);
    assert_eq!(parsed.metrics.reasoning_blocks, 1);
    assert_eq!(parsed.metrics.tokens_input, 100);
    assert_eq!(parsed.metrics.tokens_cache_r, 40);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_fallback_jsonl_matches_go_hermes_source_labeling() {
    let root = temp_root("agenttrace-rust-hermes-jsonl-source");
    fs::create_dir_all(&root).expect("create source-label temp dir");
    let session_path = root.join("session.jsonl");
    fs::write(
        &session_path,
        r#"{"role":"assistant","content":"I will inspect the file.","timestamp":"2026-05-02T10:40:00Z","tool_calls":[{"id":"bad-args-1","name":"read_file","args":"{\"path\":"}],"ModelUsed":"gpt-4.1","SourceTool":"generic"}
{"role":"tool","tool_call_id":"bad-args-1","content":"ok","timestamp":"2026-05-02T10:40:01Z","is_error":false,"SourceTool":"generic"}
"#,
    )
    .expect("write invalid args session");

    let parsed = parse_file(&session_path).expect("parse invalid args fixture");
    assert_eq!(parsed.metrics.source_tool, "hermes_jsonl");
    assert_eq!(parsed.tool_warnings.len(), 1);
    assert_eq!(parsed.tool_warnings[0].tool_name, "read_file");
    assert_eq!(parsed.tool_warnings[0].pattern, "invalid_args");

    let results = search_sessions(&[parsed], "malformed", 20);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].source_tool, "hermes_jsonl");
    assert!(results[0].matches.contains(
        &"tool warning: Tool 'read_file' had 1 call(s) with malformed arguments".to_string()
    ));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_generic_jsonl_accepts_go_style_event_field_names() {
    let root = temp_root("agenttrace-rust-go-style-jsonl");
    fs::create_dir_all(&root).expect("create go-style temp dir");
    let session_path = root.join("session.jsonl");
    fs::write(
        &session_path,
        r#"{"Role":"meta","Timestamp":"2026-05-02T10:00:00Z","ModelUsed":"claude-sonnet-4","Usage":{"input_tokens":100,"output_tokens":20}}
{"Role":"assistant","Content":"I will run the tool.","Timestamp":"2026-05-02T10:00:01Z","ToolCalls":[{"id":"t1","function":{"name":"run","arguments":"echo ok"},"type":"function"}],"ModelUsed":"claude-sonnet-4"}
{"Role":"tool","Content":"ok","ToolCallID":"t1","Timestamp":"2026-05-02T10:00:02Z","IsError":false}
"#,
    )
    .expect("write go-style jsonl session");

    let parsed = parse_file(&session_path).expect("parse go-style jsonl session");
    assert_eq!(parsed.metrics.source_tool, "hermes_jsonl");
    assert_eq!(parsed.metrics.model_used, "claude-sonnet-4");
    assert_eq!(parsed.metrics.tokens_input, 100);
    assert_eq!(parsed.metrics.tokens_output, 20);
    assert_eq!(parsed.metrics.tool_calls_total, 1);
    assert_eq!(parsed.metrics.tool_calls_ok, 1);
    assert_eq!(parsed.metrics.tool_usage.get("run"), Some(&1));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_single_codex_session_meta_json_object_falls_back_to_generic() {
    let root = temp_root("agenttrace-rust-single-codex-meta-generic");
    fs::create_dir_all(&root).expect("create codex meta temp dir");
    let session_path = root.join("meta-only.jsonl");
    fs::write(
        &session_path,
        r#"{"timestamp":"2026-05-03T10:00:00Z","type":"session_meta","payload":{"id":"s1","model_provider":"openai","source":"cli"}}"#,
    )
    .expect("write single meta session");

    let parsed = parse_file(&session_path).expect("parse single meta session");
    assert_eq!(parsed.metrics.source_tool, "generic");
    assert_eq!(parsed.metrics.events_total, 1);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_generic_jsonl_accepts_legacy_type_events_without_role() {
    let root = temp_root("agenttrace-rust-legacy-generic-jsonl");
    fs::create_dir_all(&root).expect("create legacy generic temp dir");
    let session_path = root.join("legacy.jsonl");
    fs::write(
        &session_path,
        r#"{"kind":"session","sessionId":"s1","projectHash":"p","startTime":"2026-05-03T10:00:00Z","lastUpdated":"2026-05-03T10:00:01Z"}
{"type":"user","id":"u1","timestamp":"2026-05-03T10:00:00Z","content":["hello"]}
{"type":"assistant","id":"a1","timestamp":"2026-05-03T10:00:01Z","content":"hi"}
"#,
    )
    .expect("write legacy generic session");

    let parsed = parse_file(&session_path).expect("parse legacy generic session");
    assert_eq!(parsed.metrics.source_tool, "generic");
    assert_eq!(parsed.metrics.events_total, 2);
    assert_eq!(parsed.metrics.user_messages, 0);
    assert_eq!(parsed.metrics.assistant_turns, 0);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_messages_json_without_roles_falls_back_to_codex_cli() {
    let root = temp_root("agenttrace-rust-messages-without-roles");
    fs::create_dir_all(&root).expect("create messages temp dir");
    let session_path = root.join("messages.json");
    fs::write(
        &session_path,
        r#"{"kind":"session","sessionId":"s1","projectHash":"p","startTime":"2026-05-03T10:00:00Z","lastUpdated":"2026-05-03T10:00:01Z","messages":[{"type":"user","id":"u1","timestamp":"2026-05-03T10:00:00Z","content":"hello"},{"type":"assistant","id":"a1","timestamp":"2026-05-03T10:00:01Z","content":"hi"}]}"#,
    )
    .expect("write role-less messages session");

    let parsed = parse_file(&session_path).expect("parse role-less messages session");
    assert_eq!(parsed.metrics.source_tool, "codex_cli");
    assert_eq!(parsed.metrics.events_total, 2);
    assert_eq!(parsed.metrics.user_messages, 0);
    assert_eq!(parsed.metrics.assistant_turns, 0);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_writes_and_reuses_go_compatible_session_cache() {
    let root = temp_root("agenttrace-rust-session-cache");
    let home = root.join("home");
    let sessions_dir = home.join("sessions");
    let cache_dir = home.join("cache");
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");
    let session_path = sessions_dir.join("session.jsonl");
    fs::write(&session_path, SAMPLE_JSONL).expect("write session");

    with_home_and_cache(&home, &cache_dir, || {
        let sessions = load_sessions_from_dir(Some(&sessions_dir));
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].metrics.source_tool, "hermes_jsonl");

        let cache_path = session_cache_path();
        assert_eq!(cache_path, cache_dir.join("sessions.json"));
        let raw = fs::read_to_string(&cache_path).expect("read written cache");
        let doc: Value = serde_json::from_str(&raw).expect("cache json");
        assert_eq!(
            doc.pointer("/schema_version").and_then(Value::as_i64),
            Some(16)
        );
        let entry = doc
            .pointer(&format!("/entries/{}", escape_json_pointer(&session_path)))
            .expect("cache entry");
        assert!(entry.get("mod_time").and_then(Value::as_i64).is_some());
        assert_eq!(
            entry.pointer("/session/Name").and_then(Value::as_str),
            Some("Inspect billing export.")
        );
        assert_eq!(
            entry
                .pointer("/session/Metrics/SourceTool")
                .and_then(Value::as_str),
            Some("hermes_jsonl")
        );
        assert_eq!(
            entry
                .pointer("/session/Metrics/ToolUsage/rg")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            entry
                .pointer("/session/Metrics/ToolArgUsage/billing export")
                .and_then(Value::as_u64),
            Some(1)
        );

        let doctor = build_doctor_report(Some(&sessions_dir), false);
        assert_eq!(doctor.cache_entries, 1);
        assert_eq!(doctor.cached_valid, 1);

        fs::write(&session_path, "not a session\n").expect("invalidate session cache entry");
        let after_stale = load_sessions_from_dir(Some(&sessions_dir));
        assert!(after_stale.is_empty());
        let doctor = build_doctor_report(Some(&sessions_dir), false);
        assert_eq!(doctor.cached_valid, 0);
    });

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_reports_monotonic_load_progress_and_real_cache_hits() {
    let root = temp_root("agenttrace-rust-load-progress");
    let sessions_dir = root.join("sessions");
    let cache_dir = root.join("cache");
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");
    fs::write(sessions_dir.join("a.jsonl"), SAMPLE_JSONL).expect("write session a");
    fs::write(sessions_dir.join("b.jsonl"), SAMPLE_JSONL).expect("write session b");
    fs::write(sessions_dir.join("bad.jsonl"), "not a session\n").expect("write bad session");

    with_session_cache(&cache_dir, || {
        let mut first = Vec::new();
        let report =
            load_sessions_with_progress(Some(&sessions_dir), &LoadOptions::default(), |progress| {
                first.push(progress)
            });
        assert_eq!(report.discovered, 3);
        assert_eq!(report.parsed, 2);
        assert_eq!(report.skipped, 1);
        assert_eq!(report.cache_hits, 0);
        assert_eq!(
            first.iter().map(|item| item.processed).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(first.last().map(|item| item.skipped), Some(1));

        let mut second = Vec::new();
        let report =
            load_sessions_with_progress(Some(&sessions_dir), &LoadOptions::default(), |progress| {
                second.push(progress)
            });
        assert_eq!(report.cache_hits, 2);
        assert_eq!(second.last().map(|item| item.cache_hits), Some(2));
        assert_eq!(
            second.iter().filter(|item| item.session.is_some()).count(),
            2
        );
    });

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_skips_claude_workflow_definitions() {
    let root = temp_root("agenttrace-claude-workflows");
    let project = root.join(".claude/projects/demo/session");
    let workflows = project.join("workflows");
    fs::create_dir_all(&workflows).expect("create workflow directory");
    fs::write(project.join("session.jsonl"), SAMPLE_JSONL).expect("write session");
    fs::write(
        workflows.join("wf_demo.json"),
        r#"{"runId":"wf_demo","script":"x"}"#,
    )
    .expect("write workflow");

    let files = find_session_files(Some(&root.join(".claude/projects")));
    assert_eq!(files, vec![project.join("session.jsonl")]);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_refreshes_cache_entries_from_old_schema_version() {
    let root = temp_root("agenttrace-rust-session-cache-old-schema");
    let home = root.join("home");
    let sessions_dir = home.join("sessions");
    let cache_dir = home.join("cache");
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");
    fs::create_dir_all(&cache_dir).expect("create cache dir");
    let session_path = sessions_dir.join("session.jsonl");
    fs::write(&session_path, SAMPLE_JSONL).expect("write session");
    let metadata = fs::metadata(&session_path).expect("session metadata");
    let cache_path = cache_dir.join("sessions.json");
    fs::write(
        &cache_path,
        format!(
            r#"{{"schema_version":3,"entries":{{"{}":{{"mod_time":{},"size":{},"session":{{"Name":"cached","Path":"{}","Metrics":{{"SourceTool":"stale","ModelUsed":"cached-model","SessionStart":"2026-05-02T09:00:00Z","ToolArgUsage":{{}}}},"Health":91,"ToolWarnings":[]}}}}}}}}"#,
            session_path.to_string_lossy(),
            file_mod_time_nanos_for_test(&metadata),
            metadata.len(),
            session_path.to_string_lossy()
        ),
    )
    .expect("write old schema cache");

    with_home_and_cache(&home, &cache_dir, || {
        let sessions = load_sessions_from_dir(Some(&sessions_dir));
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].name, "Inspect billing export.");
        assert_eq!(sessions[0].metrics.source_tool, "hermes_jsonl");

        let raw = fs::read_to_string(session_cache_path()).expect("read refreshed cache");
        let doc: Value = serde_json::from_str(&raw).expect("cache json");
        assert_eq!(
            doc.pointer("/schema_version").and_then(Value::as_i64),
            Some(16)
        );
        let entry = doc
            .pointer(&format!("/entries/{}", escape_json_pointer(&session_path)))
            .expect("cache entry");
        assert_eq!(
            entry
                .pointer("/session/Metrics/SourceTool")
                .and_then(Value::as_str),
            Some("hermes_jsonl")
        );
    });

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_refreshes_cache_entries_missing_tool_arg_usage() {
    let root = temp_root("agenttrace-rust-session-cache-tool-args");
    let home = root.join("home");
    let sessions_dir = home.join("sessions");
    let cache_dir = home.join("cache");
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");
    fs::create_dir_all(&cache_dir).expect("create cache dir");
    let session_path = sessions_dir.join("session.jsonl");
    fs::write(&session_path, SAMPLE_JSONL).expect("write session");
    let metadata = fs::metadata(&session_path).expect("session metadata");
    let cache_path = cache_dir.join("sessions.json");
    fs::write(
        &cache_path,
        format!(
            r#"{{"entries":{{"{}":{{"mod_time":{},"size":{},"session":{{"Name":"cached","Path":"{}","Metrics":{{"SourceTool":"hermes_jsonl","ModelUsed":"cached-model","SessionStart":"2026-05-02T09:00:00Z","ToolUsage":{{"rg":1}},"FileUsage":{{"go test ./...":1}}}},"Health":91,"ToolWarnings":[]}}}}}}}}"#,
            session_path.to_string_lossy(),
            file_mod_time_nanos_for_test(&metadata),
            metadata.len(),
            session_path.to_string_lossy()
        ),
    )
    .expect("write old cache");

    with_home_and_cache(&home, &cache_dir, || {
        let sessions = load_sessions_from_dir(Some(&sessions_dir));
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].name, "Inspect billing export.");
        assert_eq!(
            sessions[0].metrics.tool_arg_usage.get("billing export"),
            Some(&1)
        );
        assert!(sessions[0].metrics.file_usage.is_empty());

        let raw = fs::read_to_string(session_cache_path()).expect("read refreshed cache");
        let doc: Value = serde_json::from_str(&raw).expect("cache json");
        let entry = doc
            .pointer(&format!("/entries/{}", escape_json_pointer(&session_path)))
            .expect("cache entry");
        assert_eq!(
            entry
                .pointer("/session/Metrics/ToolArgUsage/billing export")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert!(entry
            .pointer("/session/Metrics/FileUsage/go test ./...")
            .is_none());
    });

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_refreshes_cache_entries_with_empty_source_tool() {
    let root = temp_root("agenttrace-rust-session-cache-empty-source");
    let home = root.join("home");
    let sessions_dir = home.join("sessions");
    let cache_dir = home.join("cache");
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");
    fs::create_dir_all(&cache_dir).expect("create cache dir");
    let session_path = sessions_dir.join("meta-only.jsonl");
    fs::write(
        &session_path,
        r#"{"timestamp":"2026-05-03T10:00:00Z","type":"session_meta","payload":{"id":"s1","model_provider":"openai","source":"cli"}}"#,
    )
    .expect("write single meta session");
    let metadata = fs::metadata(&session_path).expect("session metadata");
    let cache_path = cache_dir.join("sessions.json");
    fs::write(
        &cache_path,
        format!(
            r#"{{"entries":{{"{}":{{"mod_time":{},"size":{},"session":{{"Name":"cached","Path":"{}","Metrics":{{"SourceTool":"","ModelUsed":"cached-model","SessionStart":"2026-05-03T09:00:00Z","ToolArgUsage":{{}}}},"Health":91,"ToolWarnings":[]}}}}}}}}"#,
            session_path.to_string_lossy(),
            file_mod_time_nanos_for_test(&metadata),
            metadata.len(),
            session_path.to_string_lossy()
        ),
    )
    .expect("write old empty-source cache");

    with_home_and_cache(&home, &cache_dir, || {
        let sessions = load_sessions_from_dir(Some(&sessions_dir));
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].name, "meta-only");
        assert_eq!(sessions[0].metrics.source_tool, "generic");

        let raw = fs::read_to_string(session_cache_path()).expect("read refreshed cache");
        let doc: Value = serde_json::from_str(&raw).expect("cache json");
        let entry = doc
            .pointer(&format!("/entries/{}", escape_json_pointer(&session_path)))
            .expect("cache entry");
        assert_eq!(
            entry
                .pointer("/session/Metrics/SourceTool")
                .and_then(Value::as_str),
            Some("generic")
        );
    });

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_writes_and_refreshes_go_compatible_directory_cache() {
    let root = temp_root("agenttrace-rust-dir-cache");
    let home = root.join("home");
    let sessions_dir = home.join("sessions");
    let nested = sessions_dir.join("nested");
    let cache_dir = home.join("cache");
    fs::create_dir_all(&nested).expect("create nested session dir");
    let first_path = nested.join("first.jsonl");
    fs::write(&first_path, SAMPLE_JSONL).expect("write first session");

    with_home_and_cache(&home, &cache_dir, || {
        let sessions = load_sessions_from_dir(Some(&sessions_dir));
        assert_eq!(sessions.len(), 1);

        let cache_path = session_cache_path();
        let raw = fs::read_to_string(&cache_path).expect("read written cache");
        let doc: Value = serde_json::from_str(&raw).expect("cache json");
        let root_dir = doc
            .pointer(&format!("/dirs/{}", escape_json_pointer(&sessions_dir)))
            .expect("root dir cache entry");
        let nested_dir = doc
            .pointer(&format!("/dirs/{}", escape_json_pointer(&nested)))
            .expect("nested dir cache entry");
        assert!(root_dir.get("mod_time").and_then(Value::as_i64).is_some());
        assert_eq!(
            root_dir.pointer("/dirs/0").and_then(Value::as_str),
            Some(nested.to_string_lossy().as_ref())
        );
        assert_eq!(
            nested_dir.pointer("/files/0").and_then(Value::as_str),
            Some(first_path.to_string_lossy().as_ref())
        );

        let doctor = build_doctor_report(Some(&sessions_dir), false);
        assert_eq!(doctor.cache_dirs, 2);

        let second_path = nested.join("second.jsonl");
        fs::write(&second_path, SAMPLE_JSONL).expect("write second session");
        bump_dir_mtime(&nested);
        bump_dir_mtime(&sessions_dir);
        let sessions = load_sessions_from_dir(Some(&sessions_dir));
        assert_eq!(sessions.len(), 2);
        assert!(sessions
            .iter()
            .any(|session| session.path == second_path.to_string_lossy()));

        fs::remove_file(&first_path).expect("remove stale cached session file");
        bump_dir_mtime(&nested);
        bump_dir_mtime(&sessions_dir);
        let sessions = load_sessions_from_dir(Some(&sessions_dir));
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].path, second_path.to_string_lossy());
    });

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_parses_opencode_storage_session() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root");
    let session_path =
        repo_root.join("testdata/opencode/storage/session/project_alpha/ses_abc.json");

    let parsed = parse_file(&session_path).expect("parse OpenCode storage session");
    assert_eq!(parsed.metrics.source_tool, "opencode");
    assert_eq!(parsed.metrics.model_used, "claude-sonnet-4");
    assert_eq!(parsed.metrics.user_messages, 1);
    assert_eq!(parsed.metrics.assistant_turns, 2);
    assert_eq!(parsed.metrics.tool_calls_total, 1);
    assert_eq!(parsed.metrics.tool_calls_ok, 1);
    assert_eq!(parsed.metrics.tokens_input, 42);
    assert_eq!(parsed.metrics.tokens_output, 17);
    assert_eq!(parsed.metrics.tokens_cache_r, 3);
    assert_eq!(parsed.metrics.tokens_cache_w, 2);
}

#[test]
fn rust_discovers_only_opencode_storage_session_files_like_go() {
    let root = temp_root("agenttrace-rust-opencode-discovery");
    let home = root.join("home");
    let storage = home
        .join(".local")
        .join("share")
        .join("opencode")
        .join("storage");
    let session_dir = storage.join("session").join("project_alpha");
    let message_dir = storage.join("message").join("ses_abc");
    let part_dir = storage.join("part").join("msg_user");
    fs::create_dir_all(&session_dir).expect("create opencode session dir");
    fs::create_dir_all(&message_dir).expect("create opencode message dir");
    fs::create_dir_all(&part_dir).expect("create opencode part dir");
    let session_path = session_dir.join("ses_abc.json");
    let message_path = message_dir.join("msg_user.json");
    let part_path = part_dir.join("part_text.json");
    let raw = r#"{"id":"ses_abc","projectID":"project_alpha","time":{"created":1764750000000}}"#;
    fs::write(&session_path, raw).expect("write opencode session file");
    fs::write(&message_path, raw).expect("write opencode message file");
    fs::write(&part_path, raw).expect("write opencode part file");

    with_home(&home, || {
        let files = find_session_files(None);
        assert!(
            files.contains(&session_path),
            "missing session file: {files:?}"
        );
        assert!(
            !files.contains(&message_path) && !files.contains(&part_path),
            "message/part files should be skipped: {files:?}"
        );

        let files = find_session_files(Some(&storage));
        assert!(
            files.contains(&session_path),
            "missing session file: {files:?}"
        );
        assert!(
            !files.contains(&message_path) && !files.contains(&part_path),
            "message/part files should be skipped for custom storage: {files:?}"
        );
    });

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_parses_fixture_formats_used_by_compare_gate() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root");
    let cases = [
        (
            "testdata/codex-rollout-with-aider-text.jsonl",
            "codex_cli",
            "gpt-5.4",
            1,
            1,
            0,
            45,
        ),
        (
            "testdata/claude-code-preamble.jsonl",
            "claude_code",
            "claude-sonnet-4",
            1,
            1,
            1,
            181,
        ),
        (
            "testdata/copilot-attrs-map.jsonl",
            "copilot_cli",
            "gpt-4.1",
            0,
            2,
            1,
            168,
        ),
        (
            "testdata/kimi-tool-args.json",
            "kimi_cli",
            "kimi-k2.6",
            1,
            2,
            1,
            160,
        ),
    ];

    for (rel, source, model, users, turns, tools, tokens) in cases {
        let parsed = parse_file(&repo_root.join(rel)).unwrap_or_else(|err| panic!("{rel}: {err}"));
        assert_eq!(parsed.metrics.source_tool, source, "{rel}");
        assert_eq!(parsed.metrics.model_used, model, "{rel}");
        assert_eq!(parsed.metrics.user_messages, users, "{rel}");
        assert_eq!(parsed.metrics.assistant_turns, turns, "{rel}");
        assert_eq!(parsed.metrics.tool_calls_total, tools, "{rel}");
        assert_eq!(
            parsed.metrics.tokens_input
                + parsed.metrics.tokens_output
                + parsed.metrics.tokens_cache_r
                + parsed.metrics.tokens_cache_w,
            tokens,
            "{rel}"
        );
    }
}

#[test]
fn rust_codex_rollout_token_counts_use_turn_context_model() {
    let root = temp_root("agenttrace-rust-codex-token-counts");
    fs::create_dir_all(&root).expect("create codex temp dir");
    let session_path = root.join("rollout.jsonl");
    fs::write(
        &session_path,
        r#"{"timestamp":"2026-05-03T10:00:00Z","type":"session_meta","payload":{"model_provider":"openai"}}
{"timestamp":"2026-05-03T10:00:01Z","type":"turn_context","payload":{"model":"gpt-5.4"}}
{"timestamp":"2026-05-03T10:00:02Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":1000,"cached_input_tokens":400,"output_tokens":100,"reasoning_output_tokens":20,"total_tokens":1100},"last_token_usage":{"input_tokens":1000,"cached_input_tokens":400,"output_tokens":100,"reasoning_output_tokens":20,"total_tokens":1100}}}}
{"timestamp":"2026-05-03T10:00:03Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":1000,"cached_input_tokens":400,"output_tokens":100,"reasoning_output_tokens":20,"total_tokens":1100},"last_token_usage":{"input_tokens":1000,"cached_input_tokens":400,"output_tokens":100,"reasoning_output_tokens":20,"total_tokens":1100}}}}
{"timestamp":"2026-05-03T10:00:04Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":1700,"cached_input_tokens":900,"output_tokens":160,"reasoning_output_tokens":30,"total_tokens":1860},"last_token_usage":{"input_tokens":700,"cached_input_tokens":500,"output_tokens":60,"reasoning_output_tokens":10,"total_tokens":760}}}}
{"timestamp":"2026-05-03T10:00:05Z","type":"response_item","payload":{"type":"function_call","call_id":"call_1","name":"shell","arguments":"{}"}}
{"timestamp":"2026-05-03T10:00:06Z","type":"response_item","payload":{"type":"function_call_output","call_id":"call_1","output":"ok"}}
"#,
    )
    .expect("write codex rollout");

    let parsed = parse_file(&session_path).expect("parse codex rollout");
    let metrics = &parsed.metrics;
    assert_eq!(metrics.model_used, "gpt-5.4");
    assert_eq!(metrics.source_tool, "codex_cli");
    assert_eq!(metrics.tokens_input, 800);
    assert_eq!(metrics.tokens_cache_r, 900);
    assert_eq!(metrics.tokens_output, 190);
    assert_eq!(metrics.tool_calls_total, 1);
    assert_eq!(metrics.tool_calls_ok, 1);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_codex_rollout_prefers_cached_input_tokens_like_go() {
    let root = temp_root("agenttrace-rust-codex-cache-read-priority");
    fs::create_dir_all(&root).expect("create codex temp dir");
    let session_path = root.join("rollout.jsonl");
    fs::write(
        &session_path,
        r#"{"timestamp":"2026-05-03T10:00:00Z","type":"session_meta","payload":{"model":"gpt-5.5"}}
{"timestamp":"2026-05-03T10:00:01Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":1000,"cached_input_tokens":100,"cache_read_input_tokens":900,"output_tokens":10}}}}
{"timestamp":"2026-05-03T10:00:02Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"done"}]}}
"#,
    )
    .expect("write codex rollout");

    let parsed = parse_file(&session_path).expect("parse codex rollout");
    let metrics = &parsed.metrics;
    assert_eq!(metrics.source_tool, "codex_cli");
    assert_eq!(metrics.tokens_input, 900);
    assert_eq!(metrics.tokens_cache_r, 100);
    assert_eq!(metrics.tokens_output, 10);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_codex_rollout_clamps_negative_uncached_input_like_go() {
    let root = temp_root("agenttrace-rust-codex-negative-input");
    fs::create_dir_all(&root).expect("create codex temp dir");
    let session_path = root.join("rollout.jsonl");
    fs::write(
        &session_path,
        r#"{"timestamp":"2026-05-03T10:00:00Z","type":"session_meta","payload":{"model":"gpt-5.5"}}
{"timestamp":"2026-05-03T10:00:01Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":1000,"cached_input_tokens":100,"output_tokens":10}}}}
{"timestamp":"2026-05-03T10:00:02Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":1100,"cached_input_tokens":500,"output_tokens":20}}}}
{"timestamp":"2026-05-03T10:00:03Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"done"}]}}
"#,
    )
    .expect("write codex rollout");

    let parsed = parse_file(&session_path).expect("parse codex rollout");
    let metrics = &parsed.metrics;
    assert_eq!(metrics.source_tool, "codex_cli");
    assert_eq!(metrics.tokens_input, 900);
    assert_eq!(metrics.tokens_cache_r, 500);
    assert_eq!(metrics.tokens_output, 20);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_claude_code_jsonl_deduplicates_assistant_usage_snapshots() {
    let root = temp_root("agenttrace-rust-claude-usage-dedupe");
    fs::create_dir_all(&root).expect("create claude temp dir");
    let session_path = root.join("claude.jsonl");
    fs::write(
        &session_path,
        r#"{"type":"assistant","timestamp":"2026-05-03T10:00:00Z","message":{"id":"msg_1","role":"assistant","model":"claude-sonnet-4-6","usage":{"input_tokens":100,"output_tokens":10,"cache_read_input_tokens":7,"cache_creation_input_tokens":3},"content":[{"type":"text","text":"hello"}]}}
{"type":"assistant","timestamp":"2026-05-03T10:00:01Z","message":{"id":"msg_1","role":"assistant","model":"claude-sonnet-4-6","usage":{"input_tokens":100,"output_tokens":10,"cache_read_input_tokens":7,"cache_creation_input_tokens":3},"content":[{"type":"tool_use","id":"tool_1","name":"Read","input":{}}]}}
"#,
    )
    .expect("write claude jsonl");

    let parsed = parse_file(&session_path).expect("parse claude jsonl");
    let metrics = &parsed.metrics;
    assert_eq!(metrics.source_tool, "claude_code");
    assert_eq!(metrics.tokens_input, 100);
    assert_eq!(metrics.tokens_output, 10);
    assert_eq!(metrics.tokens_cache_r, 7);
    assert_eq!(metrics.tokens_cache_w, 3);
    assert_eq!(metrics.tool_calls_total, 1);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_claude_code_jsonl_uses_body_cwd_like_go() {
    let root = temp_root("agenttrace-rust-claude-cwd");
    fs::create_dir_all(&root).expect("create claude temp dir");
    let session_path = root.join("claude.jsonl");
    fs::write(
        &session_path,
        r#"{"type":"user","sessionId":"session-abc","cwd":"/real/worktree/alpha","timestamp":"2026-05-20T10:00:00Z","message":{"role":"user","content":"inspect cwd provenance"}}
{"type":"assistant","sessionId":"session-abc","timestamp":"2026-05-20T10:00:01Z","message":{"role":"assistant","model":"claude-sonnet-4","content":[{"type":"text","text":"ok"}]}}
"#,
    )
    .expect("write claude jsonl");

    let parsed = parse_file(&session_path).expect("parse claude jsonl");
    assert_eq!(parsed.cwd, "/real/worktree/alpha");
    assert_eq!(parsed.metrics.source_tool, "claude_code");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_claude_code_jsonl_keeps_first_non_unknown_model_like_go() {
    let root = temp_root("agenttrace-rust-claude-model-order");
    fs::create_dir_all(&root).expect("create claude temp dir");
    let session_path = root.join("claude.jsonl");
    fs::write(
        &session_path,
        r#"{"type":"assistant","timestamp":"2026-05-03T10:00:00Z","message":{"id":"msg_1","role":"assistant","model":"glm-5.1","usage":{"input_tokens":100,"output_tokens":10},"content":[{"type":"text","text":"hello"}]}}
{"type":"assistant","timestamp":"2026-05-03T10:00:01Z","message":{"id":"msg_2","role":"assistant","model":"qwen3.7-max","usage":{"input_tokens":20,"output_tokens":5},"content":[{"type":"text","text":"again"}]}}
"#,
    )
    .expect("write claude jsonl");

    let parsed = parse_file(&session_path).expect("parse claude jsonl");
    assert_eq!(parsed.metrics.source_tool, "claude_code");
    assert_eq!(parsed.metrics.model_used, "glm-5.1");
    assert_eq!(parsed.metrics.tokens_input, 120);
    assert_eq!(parsed.metrics.tokens_output, 15);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_parses_openclaw_anthropic_wrapper() {
    let root = temp_root("agenttrace-rust-openclaw");
    fs::create_dir_all(&root).expect("create openclaw temp dir");
    let session_path = root.join("openclaw.json");
    fs::write(
        &session_path,
        r#"{"provider":"openclaw","model":"claude-sonnet-4","usage":{"input_tokens":33,"output_tokens":12},"messages":[{"role":"user","content":"Inspect the parser.","timestamp":"2026-05-03T10:00:00Z"},{"role":"assistant","timestamp":"2026-05-03T10:00:01Z","content":[{"type":"thinking","thinking":"Check provider detection first."},{"type":"text","text":"I will inspect it."},{"type":"tool_use","id":"tc1","name":"read_file","input":{"path":"src/main.rs"}}]},{"role":"tool","tool_call_id":"tc1","content":"done","timestamp":"2026-05-03T10:00:02Z","is_error":false}]}"#,
    )
    .expect("write openclaw session");

    let parsed = parse_file(&session_path).expect("parse openclaw session");
    let metrics = &parsed.metrics;
    assert_eq!(metrics.source_tool, "openclaw");
    assert_eq!(metrics.model_used, "claude-sonnet-4");
    assert_eq!(metrics.user_messages, 1);
    assert_eq!(metrics.assistant_turns, 3);
    assert_eq!(metrics.tool_calls_total, 1);
    assert_eq!(metrics.tool_calls_ok, 1);
    assert_eq!(metrics.tokens_input, 33);
    assert_eq!(metrics.tokens_output, 12);
    assert_eq!(metrics.reasoning_blocks, 1);
    assert_eq!(metrics.tool_usage.get("read_file"), Some(&1));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_parses_hermes_json_session() {
    let root = temp_root("agenttrace-rust-hermes-json");
    fs::create_dir_all(&root).expect("create hermes json temp dir");
    let session_path = root.join("hermes.json");
    fs::write(
        &session_path,
        r#"{"session_id":"s1","platform":"darwin","model":"claude-sonnet-4","usage":{"input_tokens":100,"output_tokens":200},"messages":[{"role":"user","content":"hello","timestamp":"2026-01-01T00:00:00Z"},{"role":"assistant","content":"","timestamp":"2026-01-01T00:00:01Z","tool_calls":[{"id":"tc1","function":{"name":"read_file","arguments":{"path":"README.md"}}}]},{"role":"tool","content":"ok","tool_call_id":"tc1","timestamp":"2026-01-01T00:00:02Z","is_error":false}]}"#,
    )
    .expect("write hermes json session");

    let parsed = parse_file(&session_path).expect("parse hermes json session");
    let metrics = &parsed.metrics;
    assert_eq!(metrics.source_tool, "hermes_json");
    assert_eq!(metrics.model_used, "claude-sonnet-4");
    assert_eq!(metrics.user_messages, 1);
    assert_eq!(metrics.assistant_turns, 1);
    assert_eq!(metrics.tool_calls_total, 1);
    assert_eq!(metrics.tool_calls_ok, 1);
    assert_eq!(metrics.tokens_input, 100);
    assert_eq!(metrics.tokens_output, 200);
    assert_eq!(metrics.session_start, "2026-01-01T00:00:00Z");
    assert_eq!(metrics.session_end, "2026-01-01T00:00:02Z");
    assert_eq!(metrics.tool_usage.get("read_file"), Some(&1));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_parses_hermes_json_session_timestamps() {
    let root = temp_root("agenttrace-rust-hermes-json-session-ts");
    fs::create_dir_all(&root).expect("create hermes json temp dir");
    let session_path = root.join("hermes-session-times.json");
    fs::write(
        &session_path,
        r#"{"session_id":"s2","model":"claude-sonnet-4","session_start":"2026-01-02T00:00:00Z","last_updated":"2026-01-02T00:00:05Z","messages":[{"role":"user","content":"hello"},{"role":"assistant","content":"done"}]}"#,
    )
    .expect("write hermes json session");

    let parsed = parse_file(&session_path).expect("parse hermes json session times");
    let metrics = &parsed.metrics;
    assert_eq!(metrics.source_tool, "hermes_json");
    assert_eq!(metrics.model_used, "claude-sonnet-4");
    assert_eq!(metrics.session_start, "2026-01-02T00:00:00Z");
    assert_eq!(metrics.session_end, "2026-01-02T00:00:05Z");
    assert_eq!(metrics.duration_sec, 5.0);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_parses_aider_chat_history() {
    let root = temp_root("agenttrace-rust-aider-history");
    fs::create_dir_all(&root).expect("create aider temp dir");
    let session_path = root.join(".aider.chat.history.md");
    fs::write(
        &session_path,
        r#"# aider chat started at 2026-05-02 10:00:00

> aider --model gpt-5.4

#### Fix parser detection

I will keep embedded Aider text from stealing JSONL formats.

> Tokens: 1.2k sent, 300 cache write, 400 cache hit, 345 received

#### Continue with tests

Added focused parser tests.
"#,
    )
    .expect("write aider history");

    let files = find_session_files(Some(&root));
    assert_eq!(files, vec![session_path.clone()]);

    let parsed = parse_file(&session_path).expect("parse aider history");
    let metrics = &parsed.metrics;
    assert_eq!(metrics.source_tool, "aider");
    assert_eq!(metrics.model_used, "gpt-5.4");
    assert_eq!(metrics.user_messages, 2);
    assert_eq!(metrics.assistant_turns, 2);
    assert_eq!(metrics.tokens_input, 1200);
    assert_eq!(metrics.tokens_output, 345);
    assert_eq!(metrics.tokens_cache_w, 300);
    assert_eq!(metrics.tokens_cache_r, 400);
    assert!(!metrics.session_start.is_empty());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_rejects_empty_aider_chat_history() {
    let root = temp_root("agenttrace-rust-aider-empty");
    fs::create_dir_all(&root).expect("create aider temp dir");
    let session_path = root.join(".aider.chat.history.md");
    fs::write(&session_path, "").expect("write empty aider history");

    let err = parse_file(&session_path).expect_err("empty aider history should fail");
    assert!(err
        .to_string()
        .contains("aider chat history: no parseable events"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_parses_oh_my_pi_session_jsonl() {
    let root = temp_root("agenttrace-rust-oh-my-pi");
    fs::create_dir_all(&root).expect("create oh-my-pi temp dir");
    let session_path = root.join("session.jsonl");
    fs::write(
        &session_path,
        r#"{"type":"session","version":3,"id":"1f9d2a6b9c0d1234","timestamp":"2026-02-16T10:20:30.000Z","cwd":"/work/pi"}
{"type":"message","id":"u1","parentId":null,"timestamp":"2026-02-16T10:21:00.000Z","message":{"role":"user","content":[{"type":"text","text":"Inspect the failing test"}],"timestamp":1771237260000}}
{"type":"message","id":"a1","parentId":"u1","timestamp":"2026-02-16T10:21:10.000Z","message":{"role":"assistant","provider":"anthropic","model":"claude-sonnet-4-5","content":[{"type":"thinking","thinking":"Need to inspect logs first."},{"type":"text","text":"I will inspect the failure."},{"type":"toolCall","id":"tc1","name":"read","arguments":{"path":"go.mod"}},{"type":"toolCall","id":"tc2","name":"read","arguments":{"path":"README.md"}}],"usage":{"input":100,"output":20,"cacheRead":7,"cacheWrite":3},"timestamp":1771237270000}}
{"type":"message","id":"t1","parentId":"a1","timestamp":"2026-02-16T10:21:12.000Z","message":{"role":"toolResult","toolCallId":"tc1","toolName":"read","content":[{"type":"text","text":"module github.com/luoyuctl/agenttrace"}],"isError":false,"timestamp":1771237272000}}
{"type":"message","id":"t2","parentId":"a1","timestamp":"2026-02-16T10:21:13.000Z","message":{"role":"toolResult","toolCallId":"tc2","toolName":"read","content":[{"type":"text","text":"lenient surrogate \ud83c result"}],"isError":false,"timestamp":1771237273000}}
"#,
    )
    .expect("write oh-my-pi session");

    let parsed = parse_file(&session_path).expect("parse oh-my-pi session");
    let metrics = &parsed.metrics;
    assert_eq!(metrics.source_tool, "oh_my_pi");
    assert_eq!(metrics.model_used, "claude-sonnet-4-5");
    assert_eq!(metrics.user_messages, 1);
    assert_eq!(metrics.assistant_turns, 1);
    assert_eq!(metrics.tool_calls_total, 2);
    assert_eq!(metrics.tool_calls_ok, 2);
    assert_eq!(metrics.tokens_input, 100);
    assert_eq!(metrics.tokens_output, 20);
    assert_eq!(metrics.tokens_cache_r, 7);
    assert_eq!(metrics.tokens_cache_w, 3);
    assert_eq!(metrics.reasoning_blocks, 1);
    assert_eq!(metrics.tool_usage.get("read"), Some(&2));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_uses_pi_source_for_pi_session_path() {
    let root = temp_root("agenttrace-rust-pi-source");
    let session_dir = root.join(".pi").join("agent").join("sessions");
    fs::create_dir_all(&session_dir).expect("create pi session dir");
    let session_path = session_dir.join("session.jsonl");
    fs::write(
        &session_path,
        r#"{"type":"session","version":3,"id":"pi-session","cwd":"/work/pi"}
{"type":"message","message":{"role":"user","content":"hello from pi"}}
"#,
    )
    .expect("write pi session");

    let parsed = parse_file(&session_path).expect("parse pi session");
    assert_eq!(parsed.metrics.source_tool, "pi");
    assert_eq!(parsed.metrics.user_messages, 1);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_rejects_oh_my_pi_session_with_invalid_header() {
    let root = temp_root("agenttrace-rust-oh-my-pi-invalid");
    fs::create_dir_all(&root).expect("create oh-my-pi temp dir");
    let session_path = root.join("broken.jsonl");
    fs::write(
        &session_path,
        r#"{"type":"session","version":3,"cwd":"/work/pi"}
{"type":"message","message":{"role":"user","content":"hello"}}
"#,
    )
    .expect("write invalid oh-my-pi session");

    let err = parse_file(&session_path).expect_err("invalid oh-my-pi header should fail");
    assert!(err.to_string().contains("oh_my_pi: invalid session header"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_discovers_pi_session_files() {
    let root = temp_root("agenttrace-rust-pi-discovery");
    let home = root.join("home");
    let session_dir = home.join(".pi").join("agent").join("sessions");
    let cache_dir = home.join("cache");
    fs::create_dir_all(&session_dir).expect("create pi session dir");
    let session_path = session_dir.join("session.jsonl");
    fs::write(
        &session_path,
        r#"{"type":"session","version":3,"id":"pi-session","cwd":"/work/pi"}
{"type":"message","message":{"role":"user","content":"hello from pi"}}
"#,
    )
    .expect("write pi session");

    with_home_and_cache(&home, &cache_dir, || {
        let files = find_session_files(None);
        assert_eq!(files, vec![session_path.clone()]);

        let sessions = load_sessions_from_dir(None);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].metrics.source_tool, "pi");
        let raw = fs::read_to_string(session_cache_path()).expect("read cache");
        let doc: Value = serde_json::from_str(&raw).expect("cache json");
        assert!(doc
            .pointer(&format!("/dirs/{}", escape_json_pointer(&session_dir)))
            .is_some());
    });

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_parses_qwen_code_stream_jsonl() {
    let root = temp_root("agenttrace-rust-qwen-stream");
    fs::create_dir_all(&root).expect("create qwen temp dir");
    let session_path = root.join("qwen-stream.jsonl");
    fs::write(
        &session_path,
        r#"{"type":"system","subtype":"session_start","uuid":"sys-1","session_id":"session-1","model":"qwen3-coder-plus","timestamp":"2026-05-03T10:00:00Z"}
{"type":"assistant","uuid":"assistant-1","session_id":"session-1","timestamp":"2026-05-03T10:00:02Z","message":{"id":"msg-1","type":"message","role":"assistant","model":"qwen3-coder-plus","content":[{"type":"reasoning","text":"Need to inspect package files."},{"type":"text","text":"I'll inspect the package."},{"type":"tool_use","id":"tool-1","name":"read_file","input":{"path":"package.json"}}],"usage":{"input_tokens":120,"output_tokens":45,"cache_read_input_tokens":10,"cache_creation_input_tokens":5}}}
{"type":"user","uuid":"user-1","session_id":"session-1","timestamp":"2026-05-03T10:00:03Z","message":{"role":"user","content":"Please continue after inspecting it."}}
{"type":"user","uuid":"tool-result-1","session_id":"session-1","timestamp":"2026-05-03T10:00:04Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tool-1","content":"package metadata","is_error":false}]}}
{"type":"result","subtype":"success","uuid":"result-1","session_id":"session-1","is_error":false,"duration_ms":1234,"result":"I'll inspect the package.","usage":{"input_tokens":120,"output_tokens":45}}
"#,
    )
    .expect("write qwen stream");

    let parsed = parse_file(&session_path).expect("parse qwen stream");
    let metrics = &parsed.metrics;
    assert_eq!(metrics.source_tool, "qwen_code");
    assert_eq!(metrics.model_used, "qwen3-coder-plus");
    assert_eq!(metrics.user_messages, 1);
    assert_eq!(metrics.assistant_turns, 1);
    assert_eq!(metrics.tool_calls_total, 1);
    assert_eq!(metrics.tool_calls_ok, 1);
    assert_eq!(metrics.tokens_input, 120);
    assert_eq!(metrics.tokens_output, 45);
    assert_eq!(metrics.tokens_cache_r, 10);
    assert_eq!(metrics.tokens_cache_w, 5);
    assert_eq!(metrics.reasoning_blocks, 1);
    assert_eq!(metrics.tool_usage.get("read_file"), Some(&1));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_parses_qwen_code_json_output_array() {
    let root = temp_root("agenttrace-rust-qwen-array");
    fs::create_dir_all(&root).expect("create qwen temp dir");
    let session_path = root.join("qwen-output.json");
    fs::write(
        &session_path,
        r#"[{"type":"system","subtype":"session_start","uuid":"sys-1","session_id":"session-1","model":"qwen3-coder-plus"},{"type":"result","subtype":"success","uuid":"result-1","session_id":"session-1","is_error":false,"result":"The capital of France is Paris.","stats":{"models":{"qwen3-coder-plus":{"tokens":{"input":20,"output":7}}}}}]"#,
    )
    .expect("write qwen array");

    let parsed = parse_file(&session_path).expect("parse qwen array");
    assert_eq!(parsed.metrics.source_tool, "qwen_code");
    assert_eq!(parsed.metrics.assistant_turns, 1);
    assert_eq!(parsed.metrics.tokens_input, 20);
    assert_eq!(parsed.metrics.tokens_output, 7);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_parses_qwen_code_json_object_output() {
    let root = temp_root("agenttrace-rust-qwen-object");
    fs::create_dir_all(&root).expect("create qwen temp dir");
    let session_path = root.join("qwen-result.json");
    fs::write(
        &session_path,
        r#"{"response":"Done.","stats":{"models":{"qwen3-coder-plus":{"tokens":{"input":31,"output":9,"cacheRead":4}}}}}"#,
    )
    .expect("write qwen object");

    let parsed = parse_file(&session_path).expect("parse qwen object");
    assert_eq!(parsed.metrics.source_tool, "qwen_code");
    assert_eq!(parsed.metrics.assistant_turns, 1);
    assert_eq!(parsed.metrics.tokens_input, 31);
    assert_eq!(parsed.metrics.tokens_output, 9);
    assert_eq!(parsed.metrics.tokens_cache_r, 4);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_rejects_qwen_code_stream_without_messages() {
    let root = temp_root("agenttrace-rust-qwen-empty");
    fs::create_dir_all(&root).expect("create qwen temp dir");
    let session_path = root.join("empty-qwen.jsonl");
    fs::write(
        &session_path,
        r#"{"type":"system","subtype":"session_start","uuid":"sys-1","session_id":"session-1","model":"qwen3-coder-plus"}
"#,
    )
    .expect("write empty qwen stream");

    let err = parse_file(&session_path).expect_err("empty qwen stream should fail");
    assert!(err.to_string().contains("qwen_code: no parseable events"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_discovers_qwen_project_chat_files() {
    let root = temp_root("agenttrace-rust-qwen-discovery");
    let home = root.join("home");
    let chat_dir = home
        .join(".qwen")
        .join("projects")
        .join("repo")
        .join("chats");
    let cache_dir = home.join("cache");
    fs::create_dir_all(&chat_dir).expect("create qwen chat dir");
    let session_path = chat_dir.join("chat.jsonl");
    fs::write(
        &session_path,
        r#"{"type":"result","subtype":"success","uuid":"result-1","session_id":"session-1","result":"Done.","usage":{"input_tokens":2,"output_tokens":1}}
"#,
    )
    .expect("write qwen chat file");

    with_home_and_cache(&home, &cache_dir, || {
        let files = find_session_files(None);
        assert_eq!(files, vec![session_path.clone()]);

        let sessions = load_sessions_from_dir(None);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].metrics.source_tool, "qwen_code");
        let raw = fs::read_to_string(session_cache_path()).expect("read cache");
        let doc: Value = serde_json::from_str(&raw).expect("cache json");
        assert!(doc
            .pointer(&format!("/dirs/{}", escape_json_pointer(&chat_dir)))
            .is_some());
    });

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rust_renders_waste_report_for_testdata_latest_slice() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root");
    let cache_root = temp_root("agenttrace-rust-report-cache");
    with_session_cache(&cache_root.join("cache"), || {
        let sessions = load_sessions_from_dir(Some(&repo_root.join("testdata")));
        let latest = sessions
            .iter()
            .max_by(|a, b| {
                a.metrics
                    .session_start
                    .cmp(&b.metrics.session_start)
                    .then_with(|| a.name.cmp(&b.name))
            })
            .expect("latest session");

        let report = render_waste_report(latest);
        assert!(report.contains("AGENTTRACE v"));
        assert!(report.contains("Score: 22/100"));
        assert!(report.contains("minor waste - cache 0% hit"));
        assert!(report.contains("caching not enabled"));
    });
    let _ = fs::remove_dir_all(cache_root);
}

#[test]
fn default_discovery_uses_hermes_sqlite_when_present() {
    let root = temp_root("agenttrace-rust-sqlite-hermes");
    let home = root.join("home");
    let legacy_dir = home.join(".hermes").join("sessions");
    fs::create_dir_all(&legacy_dir).expect("create legacy dir");
    fs::write(legacy_dir.join("legacy.jsonl"), SAMPLE_JSONL).expect("write legacy session");
    write_hermes_state_db(&home.join(".hermes").join("state.db"));

    with_home(&home, || {
        let files = find_session_files(None);
        assert_eq!(files, vec![legacy_dir.join("legacy.jsonl")]);

        let sessions = load_sessions_from_dir(None);
        assert_eq!(sessions.len(), 1);
        let metrics = &sessions[0].metrics;
        assert_eq!(metrics.source_tool, "hermes_db");
        assert_eq!(metrics.model_used, "gpt-5.1");
        assert_eq!(metrics.user_messages, 1);
        assert_eq!(metrics.assistant_turns, 1);
        assert_eq!(metrics.tool_calls_total, 1);
        assert_eq!(metrics.tokens_input, 1000);
        assert_eq!(metrics.tokens_output, 200);
        assert_eq!(metrics.tokens_cache_r, 50);
        assert_eq!(metrics.tokens_cache_w, 25);
    });

    let _ = fs::remove_dir_all(root);
}

#[test]
fn default_discovery_uses_opencode_sqlite_when_present() {
    let root = temp_root("agenttrace-rust-sqlite-opencode");
    let home = root.join("home");
    let storage = home
        .join(".local")
        .join("share")
        .join("opencode")
        .join("storage");
    let session_dir = storage.join("session").join("project_alpha");
    fs::create_dir_all(&session_dir).expect("create storage session dir");
    let legacy_session = session_dir.join("ses_abc.json");
    fs::write(
        &legacy_session,
        r#"{"id":"ses_abc","projectID":"project_alpha"}"#,
    )
    .expect("write legacy opencode storage session");
    write_opencode_db(
        &home
            .join(".local")
            .join("share")
            .join("opencode")
            .join("opencode.db"),
    );

    with_home(&home, || {
        let files = find_session_files(None);
        assert_eq!(files, vec![legacy_session]);

        let sessions = load_sessions_from_dir(None);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].name, "Parser DB");
        let metrics = &sessions[0].metrics;
        assert_eq!(metrics.source_tool, "opencode_db");
        assert_eq!(metrics.model_used, "claude-sonnet-4");
        assert_eq!(metrics.user_messages, 1);
        assert_eq!(metrics.assistant_turns, 1);
        assert_eq!(metrics.tool_calls_total, 1);
        assert_eq!(metrics.tool_calls_ok, 1);
        assert_eq!(metrics.tokens_input, 42);
        assert_eq!(metrics.tokens_output, 22);
        assert_eq!(metrics.tokens_cache_r, 3);
        assert_eq!(metrics.tokens_cache_w, 2);
    });

    let _ = fs::remove_dir_all(root);
}

fn temp_root(prefix: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("{prefix}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    root
}

fn with_home(home: &std::path::Path, f: impl FnOnce()) {
    with_home_and_cache(home, &home.join("cache"), f);
}

fn with_session_cache(cache: &std::path::Path, f: impl FnOnce()) {
    let _guard = env_lock().lock().expect("env lock");
    let previous_session_cache = std::env::var_os("AGENTTRACE_SESSION_CACHE_DIR");
    std::env::set_var("AGENTTRACE_SESSION_CACHE_DIR", cache);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    restore_env("AGENTTRACE_SESSION_CACHE_DIR", previous_session_cache);
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}

fn with_home_and_cache(home: &std::path::Path, cache: &std::path::Path, f: impl FnOnce()) {
    let _guard = env_lock().lock().expect("env lock");
    let previous_home = std::env::var_os("HOME");
    let previous_xdg_config = std::env::var_os("XDG_CONFIG_HOME");
    let previous_xdg_cache = std::env::var_os("XDG_CACHE_HOME");
    let previous_session_cache = std::env::var_os("AGENTTRACE_SESSION_CACHE_DIR");
    std::env::set_var("HOME", home);
    std::env::set_var("XDG_CONFIG_HOME", home.join(".config"));
    std::env::set_var("XDG_CACHE_HOME", home.join(".cache"));
    std::env::set_var("AGENTTRACE_SESSION_CACHE_DIR", cache);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    restore_env("HOME", previous_home);
    restore_env("XDG_CONFIG_HOME", previous_xdg_config);
    restore_env("XDG_CACHE_HOME", previous_xdg_cache);
    restore_env("AGENTTRACE_SESSION_CACHE_DIR", previous_session_cache);
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}

fn escape_json_pointer(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('~', "~0").replace('/', "~1")
}

fn restore_env(key: &str, previous: Option<std::ffi::OsString>) {
    if let Some(value) = previous {
        std::env::set_var(key, value);
    } else {
        std::env::remove_var(key);
    }
}

fn bump_dir_mtime(path: &std::path::Path) {
    let marker = path.join(format!(".agenttrace-mtime-{}", std::process::id()));
    fs::write(&marker, b"x").expect("write mtime marker");
    fs::remove_file(marker).expect("remove mtime marker");
}

fn file_mod_time_nanos_for_test(metadata: &fs::Metadata) -> i64 {
    metadata
        .modified()
        .expect("file modified time")
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .expect("modified time after unix epoch")
        .as_nanos() as i64
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn write_hermes_state_db(path: &std::path::Path) {
    fs::create_dir_all(path.parent().expect("db parent")).expect("create db parent");
    let db = Connection::open(path).expect("open hermes db");
    db.execute_batch(
        r#"
        create table sessions (
            id text primary key,
            model text,
            started_at real,
            ended_at real,
            message_count integer,
            tool_call_count integer,
            input_tokens integer,
            output_tokens integer,
            cache_read_tokens integer,
            cache_write_tokens integer
        );
        create table messages (session_id text, role text);
        insert into sessions values ('db-session', 'gpt-5.1', 1760000000, 1760000060, 2, 1, 1000, 200, 50, 25);
        insert into messages values ('db-session', 'user'), ('db-session', 'assistant');
        "#,
    )
    .expect("seed hermes db");
}

fn write_opencode_db(path: &std::path::Path) {
    fs::create_dir_all(path.parent().expect("db parent")).expect("create db parent");
    let db = Connection::open(path).expect("open opencode db");
    db.execute_batch(
        r#"
        create table session (
            id text primary key,
            title text,
            time_created integer,
            time_updated integer
        );
        create table message (session_id text, data text);
        create table part (session_id text, data text);
        insert into session values ('ses_abc', 'Parser DB', 1764750000000, 1764750004000);
        insert into message values ('ses_abc', '{"id":"msg_user","role":"user"}');
        insert into message values ('ses_abc', '{"id":"msg_assistant","role":"assistant","modelID":"claude-sonnet-4","tokens":{"input":42,"output":17,"reasoning":5,"cache":{"read":3,"write":2}}}');
        insert into part values ('ses_abc', '{"type":"tool","state":{"status":"completed"}}');
        "#,
    )
    .expect("seed opencode db");
}
