use std::path::PathBuf;
use std::process::Command;

fn generated_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/generated")
        .join(name)
}

#[test]
fn cli_entrypoint_reads_generated_fixture() {
    let output = Command::new(env!("CARGO_BIN_EXE_agenttrace"))
        .args([
            "--sessions",
            "--limit",
            "1",
            generated_fixture("detailed-tool-steps.jsonl")
                .to_str()
                .expect("fixture path is valid UTF-8"),
        ])
        .output()
        .expect("run agenttrace CLI");

    assert!(output.status.success(), "CLI failed: {:?}", output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("SESSION\tHEALTH\tDATA"));
}

#[test]
fn lang_flag_localizes_reports_and_rejects_unknown_values() {
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_agenttrace"))
            .args(args)
            .output()
            .expect("run agenttrace CLI")
    };

    let zh = run(&["--demo", "--overview", "--lang", "zh"]);
    assert!(zh.status.success(), "CLI failed: {zh:?}");
    let stdout = String::from_utf8_lossy(&zh.stdout);
    assert!(stdout.contains("全局概览"), "{stdout}");
    assert!(stdout.contains("事件时间线"), "{stdout}");
    assert!(!stdout.contains("Incident timeline"), "{stdout}");

    let en = run(&["--demo", "--overview"]);
    assert!(String::from_utf8_lossy(&en.stdout).contains("Incident timeline"));

    let invalid = run(&["--demo", "--overview", "--lang", "fr"]);
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("unsupported --lang value: fr"));

    let error = run(&["--demo", "--sessions", "--order", "up", "--lang", "zh"]);
    assert!(!error.status.success());
    assert!(String::from_utf8_lossy(&error.stderr).contains("--order 必须是 asc 或 desc"));

    let json = run(&["--demo", "--recommend", "-f", "json", "--lang", "zh"]);
    let value: serde_json::Value =
        serde_json::from_slice(&json.stdout).expect("recommendations JSON");
    assert_eq!(value[0]["id"], "tool-failures", "codes stay English");
    assert_eq!(value[0]["title"], "减少失败的工具调用");
}

#[test]
fn usage_reports_split_by_timezone_and_emit_blocks() {
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/claude-code-preamble.jsonl");
    let fixture = fixture.to_str().expect("fixture path is valid UTF-8");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_agenttrace"))
            .args(args)
            .arg(fixture)
            .output()
            .expect("run agenttrace CLI")
    };

    // Fixture usage is stamped 2026-05-03T10:00Z: still May 3 in UTC, already
    // May 4 at +14:00.
    let utc = run(&["--daily", "--tz", "utc", "-f", "json"]);
    assert!(utc.status.success(), "CLI failed: {utc:?}");
    let doc: serde_json::Value = serde_json::from_slice(&utc.stdout).expect("daily json");
    assert_eq!(doc["timezone"], "+00:00");
    assert_eq!(doc["buckets"][0]["period"], "2026-05-03");
    assert!(doc["buckets"][0]["tokens"].as_i64().unwrap_or(0) > 0);

    let east = run(&["--daily", "--tz", "+14:00", "-f", "json"]);
    let doc: serde_json::Value = serde_json::from_slice(&east.stdout).expect("daily json");
    assert_eq!(doc["buckets"][0]["period"], "2026-05-04");

    let blocks = run(&["--blocks", "--tz", "utc", "-f", "json"]);
    assert!(blocks.status.success(), "CLI failed: {blocks:?}");
    let doc: serde_json::Value = serde_json::from_slice(&blocks.stdout).expect("blocks json");
    assert_eq!(doc["estimated"], true);
    assert_eq!(doc["blocks"][0]["start"], "2026-05-03T10:00:00+00:00");
    assert_eq!(doc["blocks"][0]["active"], false);

    let bad = run(&["--weekly", "--tz", "Mars/Olympus"]);
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("--tz"));

    let both = run(&["--daily", "--blocks"]);
    assert!(!both.status.success(), "multiple actions must be rejected");
}
