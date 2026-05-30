use crate::reports::{json_float, json_string};
use crate::{
    canonical_sessions, format_cost, format_tokens, highest_authority_for_metrics, round4,
    total_tokens, SearchResult, Session, VERSION,
};
use std::collections::BTreeSet;

pub fn search_sessions(sessions: &[Session], query: &str, limit: usize) -> Vec<SearchResult> {
    let query = query.trim().to_ascii_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    let limit = if limit == 0 { 20 } else { limit };
    let mut results = Vec::new();

    for session in canonical_sessions(sessions) {
        let matches = search_session_evidence(&session, &query);
        if matches.is_empty() {
            continue;
        }
        results.push(SearchResult {
            name: session.name.clone(),
            path: session.path.clone(),
            cwd: session.cwd.clone(),
            source_tool: session.metrics.source_tool.clone(),
            model: session.metrics.model_used.clone(),
            health: session.health,
            cost: round4(session.metrics.cost_estimated),
            tokens: total_tokens(&session),
            matches,
        });
        if results.len() >= limit {
            break;
        }
    }

    results
}

pub fn report_search_json(results: &[SearchResult]) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"count\": {},\n", results.len()));
    out.push_str("  \"results\": ");
    if results.is_empty() {
        out.push_str("[],\n");
    } else {
        write_search_results_json(&mut out, results);
        out.push_str(",\n");
    }
    out.push_str(&format!(
        "  \"version\": {}\n",
        serde_json::to_string(VERSION).expect("version serializes")
    ));
    out.push('}');
    out
}

fn write_search_results_json(out: &mut String, results: &[SearchResult]) {
    out.push_str("[\n");
    for (index, result) in results.iter().enumerate() {
        out.push_str("    {\n");
        out.push_str(&format!("      \"name\": {},\n", json_string(&result.name)));
        out.push_str(&format!("      \"path\": {},\n", json_string(&result.path)));
        if !result.cwd.is_empty() {
            out.push_str(&format!("      \"cwd\": {},\n", json_string(&result.cwd)));
        }
        out.push_str(&format!(
            "      \"source_tool\": {},\n",
            json_string(&result.source_tool)
        ));
        out.push_str(&format!(
            "      \"model\": {},\n",
            json_string(&result.model)
        ));
        out.push_str(&format!("      \"health\": {},\n", result.health));
        out.push_str(&format!("      \"cost\": {},\n", json_float(result.cost)));
        out.push_str(&format!("      \"tokens\": {},\n", result.tokens));
        out.push_str("      \"matches\": ");
        write_string_array_json(out, &result.matches, 3);
        out.push('\n');
        out.push_str("    }");
        if index + 1 < results.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("  ]");
}

fn write_string_array_json(out: &mut String, values: &[String], base_indent: usize) {
    if values.is_empty() {
        out.push_str("[]");
        return;
    }
    let item_indent = "  ".repeat(base_indent + 1);
    let close_indent = "  ".repeat(base_indent);
    out.push_str("[\n");
    for (index, value) in values.iter().enumerate() {
        out.push_str(&item_indent);
        out.push_str(&json_string(value));
        if index + 1 < values.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&close_indent);
    out.push(']');
}

pub fn report_search_text(results: &[SearchResult], query: &str) -> String {
    let mut out = format!("Search results: {:?} ({})\n", query, results.len());
    if results.is_empty() {
        out.push_str("No matching session metadata found.\n");
        return out;
    }
    for result in results {
        out.push_str(&format!(
            "\n{}  {}  {}  health={}  {}  {} TOKENS\n",
            result.name,
            result.source_tool,
            result.model,
            result.health,
            format_cost(result.cost),
            format_tokens(result.tokens)
        ));
        if !result.cwd.is_empty() {
            out.push_str(&format!("  cwd: {}\n", result.cwd));
        }
        if !result.path.is_empty() {
            out.push_str(&format!("  path: {}\n", result.path));
        }
        for item in &result.matches {
            out.push_str(&format!("  - {}\n", item));
        }
    }
    out
}

fn search_session_evidence(session: &Session, query: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut matches = Vec::new();
    add_match(&mut matches, &mut seen, "name", &session.name, query);
    add_match(&mut matches, &mut seen, "path", &session.path, query);
    add_match(&mut matches, &mut seen, "cwd", &session.cwd, query);
    add_match(
        &mut matches,
        &mut seen,
        "source",
        &session.metrics.source_tool,
        query,
    );
    add_match(
        &mut matches,
        &mut seen,
        "model",
        &session.metrics.model_used,
        query,
    );
    add_match(
        &mut matches,
        &mut seen,
        "authority",
        &highest_authority_for_metrics(&session.metrics),
        query,
    );

    for tool in session.metrics.tool_usage.keys() {
        add_match(&mut matches, &mut seen, "tool", tool, query);
    }
    for arg in session.metrics.tool_arg_usage.keys() {
        add_match(&mut matches, &mut seen, "tool argument", arg, query);
    }
    for file in session.metrics.file_usage.keys() {
        add_match(&mut matches, &mut seen, "file", file, query);
    }
    for anomaly in &session.anomalies {
        add_match(&mut matches, &mut seen, "anomaly", &anomaly.kind, query);
        add_match(&mut matches, &mut seen, "anomaly", &anomaly.detail, query);
    }
    for warning in &session.tool_warnings {
        add_match(
            &mut matches,
            &mut seen,
            "tool warning",
            &warning.tool_name,
            query,
        );
        add_match(
            &mut matches,
            &mut seen,
            "tool warning",
            &warning.pattern,
            query,
        );
        add_match(
            &mut matches,
            &mut seen,
            "tool warning",
            &warning.detail,
            query,
        );
    }
    matches.truncate(8);
    matches
}

fn add_match(
    matches: &mut Vec<String>,
    seen: &mut BTreeSet<String>,
    label: &str,
    value: &str,
    query: &str,
) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    if value.to_ascii_lowercase().contains(query) {
        let item = format!("{}: {}", label, value);
        if seen.insert(item.clone()) {
            matches.push(item);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Metrics, Session};

    #[test]
    fn search_evidence_uses_go_labels_dedupes_and_caps_matches() {
        let mut metrics = Metrics {
            source_tool: "codex_cli".to_string(),
            model_used: "gpt-5".to_string(),
            ..Metrics::default()
        };
        metrics
            .tool_arg_usage
            .insert("go test ./internal/billing".to_string(), 2);
        metrics
            .tool_arg_usage
            .insert("go test ./internal/billing".to_string(), 1);
        for idx in 0..12 {
            metrics
                .file_usage
                .insert(format!("internal/billing/file-{idx}.go"), 1);
        }
        let session = Session {
            name: "billing".to_string(),
            path: "/tmp/billing.jsonl".to_string(),
            cwd: String::new(),
            metrics,
            anomalies: Vec::new(),
            health: 100,
            tool_warnings: Vec::new(),
            diagnostics: crate::Diagnostics::default(),
        };

        let matches = search_session_evidence(&session, "billing");
        assert!(matches.contains(&"tool argument: go test ./internal/billing".to_string()));
        assert!(matches.len() <= 8);
        assert!(!matches.iter().any(|item| item.starts_with("tool_arg:")));
    }

    #[test]
    fn search_json_formats_zero_cost_like_go() {
        let result = SearchResult {
            name: "session".to_string(),
            path: "/tmp/session.jsonl".to_string(),
            cwd: String::new(),
            source_tool: "pi".to_string(),
            model: "mimo-v2.5-pro".to_string(),
            health: 88,
            cost: 0.0,
            tokens: 42,
            matches: vec!["tool argument: error".to_string()],
        };

        let report = report_search_json(&[result]);
        assert!(report.contains("\"cost\": 0,"));
        assert!(!report.contains("\"cost\": 0.0,"));
    }
}
