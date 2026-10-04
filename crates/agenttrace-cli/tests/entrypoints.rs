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
