use crate::{project_name, Anomaly, Diagnostics, Metrics, Session};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DerivedSession {
    id: String,
    project: String,
    source: String,
    model: String,
    start: String,
    end: String,
    duration: f64,
    input: i64,
    output: i64,
    cache_write: i64,
    cache_read: i64,
    cost: f64,
    health: i32,
    anomalies: Vec<(String, String)>,
}

pub fn history_path() -> PathBuf {
    if let Some(dir) = std::env::var_os("AGENTTRACE_HISTORY_DIR").map(PathBuf::from) {
        return dir.join("history.json");
    }
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("agenttrace").join("history.json")
}

pub fn preserve_derived_history(sessions: &[Session]) -> anyhow::Result<()> {
    let mut records = load_records();
    for session in sessions {
        let record = DerivedSession::from_session(session);
        records.insert(record.id.clone(), record);
    }
    let path = history_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(&records)?)?;
    Ok(())
}

pub fn merge_preserved_history(live: &mut Vec<Session>) {
    let mut seen = live
        .iter()
        .map(session_id)
        .collect::<std::collections::BTreeSet<_>>();
    for record in load_records().into_values() {
        if seen.insert(record.id.clone()) {
            live.push(record.into_session());
        }
    }
}

fn load_records() -> BTreeMap<String, DerivedSession> {
    std::fs::read(history_path())
        .ok()
        .map(|raw| decode_records(&raw))
        .unwrap_or_default()
}

fn decode_records(raw: &[u8]) -> BTreeMap<String, DerivedSession> {
    serde_json::from_slice::<BTreeMap<String, DerivedSession>>(raw)
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, record)| record.id.chars().count() >= 8)
        .collect()
}

fn session_id(session: &Session) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    session.path.hash(&mut hasher);
    session.metrics.session_start.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

impl DerivedSession {
    fn from_session(session: &Session) -> Self {
        Self {
            id: session_id(session),
            project: project_name(session),
            source: session.metrics.source_tool.clone(),
            model: session.metrics.model_used.clone(),
            start: session.metrics.session_start.clone(),
            end: session.metrics.session_end.clone(),
            duration: session.metrics.duration_sec,
            input: session.metrics.tokens_input,
            output: session.metrics.tokens_output,
            cache_write: session.metrics.tokens_cache_w,
            cache_read: session.metrics.tokens_cache_r,
            cost: session.metrics.cost_estimated,
            health: session.health,
            anomalies: session
                .anomalies
                .iter()
                .map(|item| (item.kind.clone(), item.severity.clone()))
                .collect(),
        }
    }

    fn into_session(self) -> Session {
        let short_id = self.id.chars().take(8).collect::<String>();
        Session {
            name: format!("history-{short_id}"),
            path: format!("history:{}", self.id),
            cwd: self.project,
            metrics: Metrics {
                source_tool: self.source,
                model_used: self.model,
                session_start: self.start,
                session_end: self.end,
                duration_sec: self.duration,
                tokens_input: self.input,
                tokens_output: self.output,
                tokens_cache_w: self.cache_write,
                tokens_cache_r: self.cache_read,
                cost_estimated: self.cost,
                ..Metrics::default()
            },
            anomalies: self
                .anomalies
                .into_iter()
                .map(|(kind, severity)| Anomaly {
                    kind,
                    severity,
                    detail: "preserved derived history".to_string(),
                })
                .collect(),
            health: self.health,
            tool_warnings: Vec::new(),
            diagnostics: Diagnostics::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserved_history_contains_only_derived_fields() {
        let session = Session {
            name: "secret task".to_string(),
            path: "/tmp/private/session.jsonl".to_string(),
            cwd: "/work/project".to_string(),
            metrics: Metrics {
                source_tool: "codex_cli".to_string(),
                model_used: "gpt-5".to_string(),
                session_start: "2026-07-19T00:00:00Z".to_string(),
                tokens_input: 10,
                cost_estimated: 0.01,
                ..Metrics::default()
            },
            anomalies: Vec::new(),
            health: 95,
            tool_warnings: Vec::new(),
            diagnostics: Diagnostics::default(),
        };
        let json = serde_json::to_string(&DerivedSession::from_session(&session)).unwrap();
        assert!(!json.contains("secret task"));
        assert!(!json.contains("/tmp/private/session.jsonl"));
        assert!(json.contains("project"));
        assert!(json.contains("gpt-5"));

        let mut valid = DerivedSession::from_session(&session);
        let mut short = valid.clone();
        short.id = "x".to_string();
        valid.id = "12345678".to_string();
        let raw = serde_json::to_vec(&BTreeMap::from([
            ("valid".to_string(), valid),
            ("short".to_string(), short.clone()),
        ]))
        .unwrap();
        assert_eq!(decode_records(&raw).len(), 1);
        assert_eq!(short.into_session().name, "history-x");
    }
}
