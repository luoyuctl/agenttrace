use crate::{detect_anomalies, health_score, token_cost, Metrics, Session};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
struct RoleCounts {
    user: usize,
    assistant: usize,
    tool: usize,
}

#[derive(Debug, Default)]
struct SqliteSessionAgg {
    id: String,
    title: String,
    model: String,
    models: BTreeSet<String>,
    start_unix: f64,
    end_unix: f64,
    events: usize,
    user_messages: usize,
    assistant_turns: usize,
    tool_results: usize,
    tool_calls_total: usize,
    tool_calls_ok: usize,
    tool_calls_fail: usize,
    input_tokens: i64,
    output_tokens: i64,
    cache_read_tokens: i64,
    cache_write_tokens: i64,
    message_tokens: usize,
    usage_cost: f64,
    usage_cost_set: bool,
    source_tool: String,
    path: String,
    cwd: String,
}

pub fn load_sqlite_backed_sessions() -> Vec<Session> {
    load_sqlite_backed_sessions_since(None)
}

pub(crate) fn load_sqlite_backed_sessions_since(since: Option<DateTime<Utc>>) -> Vec<Session> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };
    let mut sessions = Vec::new();
    for path in hermes_state_db_paths(&home) {
        sessions.extend(load_hermes_sqlite_sessions(&path, since));
    }
    for path in opencode_db_paths(&home) {
        sessions.extend(load_opencode_sqlite_sessions(&path, since));
    }
    sessions
}

pub fn skip_sqlite_backed_file_dir(dir: &Path) -> bool {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return false;
    };
    if hermes_state_db_paths(&home)
        .iter()
        .any(|path| sqlite_file_exists(path))
        && clean_path(dir) == clean_path(&home.join(".hermes").join("sessions"))
    {
        return true;
    }
    if opencode_db_paths(&home)
        .iter()
        .any(|path| sqlite_file_exists(path))
        && is_opencode_storage_root(dir)
    {
        return true;
    }
    false
}

fn hermes_state_db_path(home: &Path) -> PathBuf {
    home.join(".hermes").join("state.db")
}

fn hermes_state_db_paths(home: &Path) -> Vec<PathBuf> {
    let mut paths = vec![hermes_state_db_path(home)];
    if let Ok(entries) = std::fs::read_dir(home.join(".hermes").join("profiles")) {
        paths.extend(
            entries
                .flatten()
                .map(|entry| entry.path().join("state.db"))
                .filter(|path| path.is_file()),
        );
    }
    paths
}

fn opencode_db_path(home: &Path) -> PathBuf {
    home.join(".local")
        .join("share")
        .join("opencode")
        .join("opencode.db")
}

fn opencode_db_paths(home: &Path) -> Vec<PathBuf> {
    let primary = opencode_db_path(home);
    let Some(dir) = primary.parent() else {
        return vec![primary];
    };
    let mut paths = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("opencode") && name.ends_with(".db"))
        })
        .collect::<Vec<_>>();
    if paths.is_empty() {
        paths.push(primary);
    }
    paths.sort();
    paths
}

fn sqlite_file_exists(path: &Path) -> bool {
    path.is_file()
}

fn open_sqlite_read_only(path: &Path) -> rusqlite::Result<Connection> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
}

fn load_hermes_sqlite_sessions(path: &Path, since: Option<DateTime<Utc>>) -> Vec<Session> {
    if !sqlite_file_exists(path) {
        return Vec::new();
    }
    if let Some(sessions) = crate::session_cache::load_sqlite_snapshot(path, "hermes") {
        return filter_since(sessions, since);
    }
    let sessions = query_hermes_sqlite_sessions(path, None);
    let _ = crate::session_cache::store_sqlite_snapshot(path, "hermes", &sessions);
    filter_since(sessions, since)
}

fn query_hermes_sqlite_sessions(path: &Path, since: Option<DateTime<Utc>>) -> Vec<Session> {
    let Ok(db) = open_sqlite_read_only(path) else {
        return Vec::new();
    };
    let roles = sqlite_role_counts(&db, "messages", "session_id", "role");
    let cwd = if sqlite_has_column(&db, "sessions", "cwd") {
        "cwd"
    } else {
        "''"
    };
    let sql = format!(
        "select id, model, started_at, ended_at, message_count, tool_call_count, \
         input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, {cwd} from sessions \
         where (?1 is null or started_at >= ?1)"
    );
    let Ok(mut stmt) = db.prepare(&sql) else {
        return Vec::new();
    };
    let since_unix = since.map(|value| value.timestamp() as f64);
    let Ok(rows) = stmt.query_map([since_unix], |row| {
        Ok(SqliteSessionAgg {
            id: row.get::<_, String>(0)?,
            model: string_or(row.get::<_, Option<String>>(1)?, "default"),
            start_unix: row.get::<_, Option<f64>>(2)?.unwrap_or(0.0),
            end_unix: row.get::<_, Option<f64>>(3)?.unwrap_or(0.0),
            events: row.get::<_, Option<i64>>(4)?.unwrap_or(0).max(0) as usize,
            tool_calls_total: row.get::<_, Option<i64>>(5)?.unwrap_or(0).max(0) as usize,
            tool_calls_ok: row.get::<_, Option<i64>>(5)?.unwrap_or(0).max(0) as usize,
            input_tokens: row.get::<_, Option<i64>>(6)?.unwrap_or(0),
            output_tokens: row.get::<_, Option<i64>>(7)?.unwrap_or(0),
            cache_read_tokens: row.get::<_, Option<i64>>(8)?.unwrap_or(0),
            cache_write_tokens: row.get::<_, Option<i64>>(9)?.unwrap_or(0),
            cwd: row.get::<_, Option<String>>(10)?.unwrap_or_default(),
            source_tool: "hermes_db".to_string(),
            path: path.to_string_lossy().to_string(),
            ..SqliteSessionAgg::default()
        })
    }) else {
        return Vec::new();
    };

    rows.filter_map(Result::ok)
        .map(|mut agg| {
            if !agg.model.is_empty() {
                agg.models.insert(agg.model.clone());
            }
            if let Some(counts) = roles.get(&agg.id) {
                agg.user_messages = counts.user;
                agg.assistant_turns = counts.assistant;
                agg.tool_results = counts.tool;
            }
            session_from_sqlite_agg(agg)
        })
        .collect()
}

fn load_opencode_sqlite_sessions(path: &Path, since: Option<DateTime<Utc>>) -> Vec<Session> {
    if !sqlite_file_exists(path) {
        return Vec::new();
    }
    if let Some(sessions) = crate::session_cache::load_sqlite_snapshot(path, "opencode") {
        return filter_since(sessions, since);
    }
    let sessions = query_opencode_sqlite_sessions(path, None);
    let _ = crate::session_cache::store_sqlite_snapshot(path, "opencode", &sessions);
    filter_since(sessions, since)
}

fn query_opencode_sqlite_sessions(path: &Path, since: Option<DateTime<Utc>>) -> Vec<Session> {
    let Ok(db) = open_sqlite_read_only(path) else {
        return Vec::new();
    };
    let mut aggs = opencode_sqlite_session_rows(&db, path, since);
    if aggs.is_empty() {
        return Vec::new();
    }
    add_opencode_sqlite_messages(&db, &mut aggs);
    add_opencode_sqlite_parts(&db, &mut aggs);

    aggs.into_values()
        .map(|mut agg| {
            if agg.model.is_empty() {
                agg.model = "default".to_string();
            }
            if agg.events == 0 {
                agg.events = agg.user_messages + agg.assistant_turns + agg.tool_calls_total;
            }
            session_from_sqlite_agg(agg)
        })
        .collect()
}

fn filter_since(sessions: Vec<Session>, since: Option<DateTime<Utc>>) -> Vec<Session> {
    sessions
        .into_iter()
        .filter(|session| {
            since.map_or(true, |since| {
                DateTime::parse_from_rfc3339(&session.metrics.session_start)
                    .ok()
                    .is_some_and(|time| time.with_timezone(&Utc) >= since)
            })
        })
        .collect()
}

fn opencode_sqlite_session_rows(
    db: &Connection,
    path: &Path,
    since: Option<DateTime<Utc>>,
) -> HashMap<String, SqliteSessionAgg> {
    let directory = if sqlite_has_column(db, "session", "directory") {
        "directory"
    } else {
        "''"
    };
    let sql = format!(
        "select id, title, time_created, time_updated, {directory} from session \
         where (?1 is null or time_created >= ?1)"
    );
    let Ok(mut stmt) = db.prepare(&sql) else {
        return HashMap::new();
    };
    let since_millis = since.map(|value| value.timestamp_millis());
    let Ok(rows) = stmt.query_map([since_millis], |row| {
        let id = row.get::<_, String>(0)?;
        Ok((
            id.clone(),
            SqliteSessionAgg {
                id,
                title: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                start_unix: row.get::<_, Option<i64>>(2)?.unwrap_or(0) as f64 / 1000.0,
                end_unix: row.get::<_, Option<i64>>(3)?.unwrap_or(0) as f64 / 1000.0,
                cwd: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                source_tool: "opencode_db".to_string(),
                path: path.to_string_lossy().to_string(),
                ..SqliteSessionAgg::default()
            },
        ))
    }) else {
        return HashMap::new();
    };
    rows.filter_map(Result::ok).collect()
}

fn add_opencode_sqlite_messages(db: &Connection, aggs: &mut HashMap<String, SqliteSessionAgg>) {
    let Ok(mut stmt) = db.prepare("select session_id, data from message") else {
        return;
    };
    let Ok(rows) = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) else {
        return;
    };
    for (session_id, raw) in rows.filter_map(Result::ok) {
        let Some(agg) = aggs.get_mut(&session_id) else {
            continue;
        };
        let Ok(Value::Object(doc)) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        agg.events += 1;
        match string(doc.get("role")) {
            "user" => agg.user_messages += 1,
            "assistant" => agg.assistant_turns += 1,
            "tool" => agg.tool_results += 1,
            _ => {}
        }
        let model = opencode_sqlite_message_model(&doc);
        if !model.is_empty() {
            agg.models.insert(model.clone());
            agg.model = model;
        }
        if add_opencode_sqlite_message_tokens(agg, &doc) {
            agg.message_tokens += 1;
        }
    }
}

fn add_opencode_sqlite_parts(db: &Connection, aggs: &mut HashMap<String, SqliteSessionAgg>) {
    let Ok(mut stmt) = db.prepare("select session_id, data from part") else {
        return;
    };
    let Ok(rows) = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) else {
        return;
    };
    for (session_id, raw) in rows.filter_map(Result::ok) {
        let Some(agg) = aggs.get_mut(&session_id) else {
            continue;
        };
        let Ok(Value::Object(doc)) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        match string(doc.get("type")) {
            "step-finish" => {
                if agg.message_tokens == 0 {
                    add_opencode_step_finish_tokens(agg, &doc);
                }
            }
            "tool" => {
                agg.tool_calls_total += 1;
                if opencode_tool_failed(&doc) {
                    agg.tool_calls_fail += 1;
                } else {
                    agg.tool_calls_ok += 1;
                }
            }
            _ => {}
        }
    }
}

fn add_opencode_sqlite_message_tokens(
    agg: &mut SqliteSessionAgg,
    doc: &serde_json::Map<String, Value>,
) -> bool {
    let Some(tokens) = doc.get("tokens").and_then(Value::as_object) else {
        return false;
    };
    let (input, output, cache_read, cache_write) = add_opencode_tokens_from_map(agg, tokens);
    let mut model = opencode_sqlite_message_model(doc);
    if model.is_empty() {
        model = agg.model.clone();
    }
    if !model.is_empty() {
        agg.usage_cost += token_cost_raw(input, output, cache_write, cache_read, &model);
        agg.usage_cost_set = true;
    }
    true
}

fn add_opencode_step_finish_tokens(
    agg: &mut SqliteSessionAgg,
    doc: &serde_json::Map<String, Value>,
) {
    let Some(tokens) = doc.get("tokens").and_then(Value::as_object) else {
        return;
    };
    let (input, output, cache_read, cache_write) = add_opencode_tokens_from_map(agg, tokens);
    if !agg.model.is_empty() {
        agg.usage_cost += token_cost_raw(input, output, cache_write, cache_read, &agg.model);
        agg.usage_cost_set = true;
    }
}

fn add_opencode_tokens_from_map(
    agg: &mut SqliteSessionAgg,
    tokens: &serde_json::Map<String, Value>,
) -> (i64, i64, i64, i64) {
    let cache = tokens.get("cache").and_then(Value::as_object);
    let input = number_as_i64(tokens.get("input"));
    let output = number_as_i64(tokens.get("output")) + number_as_i64(tokens.get("reasoning"));
    let cache_read = cache
        .map(|cache| number_as_i64(cache.get("read")))
        .unwrap_or(0);
    let cache_write = cache
        .map(|cache| number_as_i64(cache.get("write")))
        .unwrap_or(0);
    agg.input_tokens += input;
    agg.output_tokens += output;
    agg.cache_read_tokens += cache_read;
    agg.cache_write_tokens += cache_write;
    (input, output, cache_read, cache_write)
}

fn opencode_tool_failed(doc: &serde_json::Map<String, Value>) -> bool {
    doc.get("state")
        .and_then(Value::as_object)
        .map(|state| string(state.get("status")).to_ascii_lowercase())
        .map(|status| matches!(status.as_str(), "error" | "failed" | "cancelled"))
        .unwrap_or(false)
}

fn opencode_sqlite_message_model(doc: &serde_json::Map<String, Value>) -> String {
    let direct = string(doc.get("modelID"));
    if !direct.is_empty() {
        return direct.to_string();
    }
    doc.get("model")
        .and_then(Value::as_object)
        .map(|model| string(model.get("modelID")).to_string())
        .unwrap_or_default()
}

fn sqlite_role_counts(
    db: &Connection,
    table: &str,
    session_column: &str,
    role_column: &str,
) -> HashMap<String, RoleCounts> {
    let sql = format!(
        "select {session_column}, {role_column}, count(*) from {table} group by {session_column}, {role_column}"
    );
    let Ok(mut stmt) = db.prepare(&sql) else {
        return HashMap::new();
    };
    let Ok(rows) = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
        ))
    }) else {
        return HashMap::new();
    };
    let mut out = HashMap::new();
    for (session_id, role, count) in rows.filter_map(Result::ok) {
        let entry = out.entry(session_id).or_insert_with(RoleCounts::default);
        match role.as_str() {
            "user" => entry.user = count.max(0) as usize,
            "assistant" => entry.assistant = count.max(0) as usize,
            "tool" => entry.tool = count.max(0) as usize,
            _ => {}
        }
    }
    out
}

fn session_from_sqlite_agg(agg: SqliteSessionAgg) -> Session {
    let mut models = agg.models;
    if models.is_empty() && !agg.model.is_empty() {
        models.insert(agg.model.clone());
    }
    let multiple_models = models.len() > 1;
    let model = if multiple_models {
        "multiple".to_string()
    } else if agg.model.is_empty() {
        "default".to_string()
    } else {
        agg.model
    };
    let mut cost_estimated = token_cost(
        agg.input_tokens,
        agg.output_tokens,
        agg.cache_write_tokens,
        agg.cache_read_tokens,
        &model,
    );
    if agg.usage_cost_set {
        cost_estimated = crate::round4(agg.usage_cost);
    }
    let pricing_source = if multiple_models {
        "SQLite aggregate: multiple models".to_string()
    } else {
        crate::pricing::pricing_source_for(&model)
    };
    let mut metrics = Metrics {
        events_total: agg.events,
        user_messages: agg.user_messages,
        assistant_turns: agg.assistant_turns,
        tool_results: agg.tool_results,
        tool_calls_total: agg.tool_calls_total,
        tool_calls_ok: agg.tool_calls_ok,
        tool_calls_fail: agg.tool_calls_fail,
        tokens_input: agg.input_tokens,
        tokens_output: agg.output_tokens,
        tokens_cache_w: agg.cache_write_tokens,
        tokens_cache_r: agg.cache_read_tokens,
        model_used: model,
        source_tool: agg.source_tool,
        session_start: unix_seconds_rfc3339(agg.start_unix),
        session_end: unix_seconds_rfc3339(agg.end_unix),
        cost_estimated,
        provenance: crate::MetricProvenance {
            tokens: "reported_by_agent".to_string(),
            duration: "unavailable".to_string(),
            tool_results: if agg.tool_results > 0 {
                "reported_by_agent".to_string()
            } else {
                "unavailable".to_string()
            },
            files: "unavailable".to_string(),
            cost: if agg.usage_cost_set {
                "calculated_per_message_tokens".to_string()
            } else {
                "calculated_from_tokens".to_string()
            },
            pricing_source,
        },
        ..Metrics::default()
    };
    if agg.end_unix > agg.start_unix {
        metrics.duration_sec = agg.end_unix - agg.start_unix;
        metrics.provenance.duration = "timestamp_span".to_string();
    }
    let anomalies = detect_anomalies(&metrics);
    let name = if agg.title.is_empty() {
        agg.id
    } else {
        agg.title
    };
    let health = health_score(&anomalies);
    Session {
        name,
        path: agg.path,
        cwd: agg.cwd,
        metrics,
        anomalies,
        health,
        tool_warnings: Vec::new(),
        diagnostics: crate::Diagnostics::default(),
    }
}

fn token_cost_raw(input: i64, output: i64, cache_write: i64, cache_read: i64, model: &str) -> f64 {
    token_cost(input, output, cache_write, cache_read, model)
}

fn unix_seconds_rfc3339(value: f64) -> String {
    if value <= 0.0 {
        return String::new();
    }
    let secs = value as i64;
    let nsecs = ((value - secs as f64) * 1e9) as u32;
    chrono::DateTime::<chrono::Utc>::from_timestamp(secs, nsecs)
        .map(|ts| ts.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
        .unwrap_or_default()
}

fn string_or(value: Option<String>, fallback: &str) -> String {
    value
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

fn string(value: Option<&Value>) -> &str {
    value.and_then(Value::as_str).unwrap_or("")
}

fn sqlite_has_column(db: &Connection, table: &str, column: &str) -> bool {
    db.prepare(&format!("pragma table_info({table})"))
        .and_then(|mut stmt| {
            stmt.query_map([], |row| row.get::<_, String>(1))
                .map(|rows| rows.filter_map(Result::ok).any(|name| name == column))
        })
        .unwrap_or(false)
}

fn number_as_i64(value: Option<&Value>) -> i64 {
    match value {
        Some(Value::Number(number)) => number
            .as_i64()
            .or_else(|| number.as_u64().map(|n| n as i64))
            .or_else(|| number.as_f64().map(|n| n as i64))
            .unwrap_or(0),
        _ => 0,
    }
}

fn is_opencode_storage_root(path: &Path) -> bool {
    path.to_string_lossy()
        .replace('\\', "/")
        .ends_with("/opencode/storage")
}

fn clean_path(path: &Path) -> PathBuf {
    path.components().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_session_preserves_workspace() {
        let session = session_from_sqlite_agg(SqliteSessionAgg {
            id: "session".to_string(),
            cwd: "/work/sqlite".to_string(),
            ..SqliteSessionAgg::default()
        });
        assert_eq!(session.cwd, "/work/sqlite");
    }

    #[test]
    fn sqlite_multi_model_aggregate_is_not_exactly_priced_as_one_model() {
        let session = session_from_sqlite_agg(SqliteSessionAgg {
            model: "gpt-5".to_string(),
            models: BTreeSet::from(["gpt-5".to_string(), "claude-sonnet-4".to_string()]),
            usage_cost: 1.25,
            usage_cost_set: true,
            ..SqliteSessionAgg::default()
        });
        assert_eq!(session.metrics.model_used, "multiple");
        assert_eq!(
            session.metrics.provenance.pricing_source,
            "SQLite aggregate: multiple models"
        );
        assert_eq!(session.metrics.cost_estimated, 1.25);
    }
}
