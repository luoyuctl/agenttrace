use crate::{Anomaly, Diagnostics, Metrics, Session, ToolWarning};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) const SESSION_CACHE_SCHEMA_VERSION: i64 = 16;
const SQLITE_SNAPSHOT_SCHEMA_VERSION: i64 = 2;

#[derive(Debug, Clone, Default)]
pub struct SessionCache {
    path: PathBuf,
    entries: BTreeMap<String, CacheEntry>,
    raw_entries: BTreeMap<String, Value>,
    dirs: BTreeMap<String, DirCacheEntry>,
    dirty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    mod_time: i64,
    size: i64,
    session: GoSession,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntryHeader {
    mod_time: i64,
    size: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct FileFingerprint {
    mod_time: i64,
    size: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SqliteSnapshot {
    schema_version: i64,
    database: FileFingerprint,
    wal: Option<FileFingerprint>,
    sessions: Vec<GoSession>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DirCacheEntry {
    mod_time: i64,
    files: Vec<String>,
    dirs: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct CachedDirListing {
    pub files: Vec<PathBuf>,
    pub dirs: Vec<PathBuf>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct GoSession {
    #[serde(default, rename = "Name")]
    name: String,
    #[serde(default, rename = "Path")]
    path: String,
    #[serde(default, rename = "CWD")]
    cwd: String,
    #[serde(default, rename = "Metrics")]
    metrics: GoMetrics,
    #[serde(default, rename = "Anomalies")]
    anomalies: Vec<GoAnomaly>,
    #[serde(default, rename = "Health")]
    health: i32,
    #[serde(default, rename = "ToolWarnings")]
    tool_warnings: Vec<GoToolWarning>,
    #[serde(default, rename = "Diagnostics")]
    diagnostics: Diagnostics,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct GoMetrics {
    #[serde(default, rename = "EventsTotal")]
    events_total: usize,
    #[serde(default, rename = "UserMessages")]
    user_messages: usize,
    #[serde(default, rename = "AssistantTurns")]
    assistant_turns: usize,
    #[serde(default, rename = "ToolResults")]
    tool_results: usize,
    #[serde(default, rename = "ToolCallsTotal")]
    tool_calls_total: usize,
    #[serde(default, rename = "ToolCallsOK")]
    tool_calls_ok: usize,
    #[serde(default, rename = "ToolCallsFail")]
    tool_calls_fail: usize,
    #[serde(default, rename = "ToolUsage")]
    tool_usage: BTreeMap<String, usize>,
    #[serde(default, rename = "FileUsage")]
    file_usage: BTreeMap<String, usize>,
    #[serde(default, rename = "ToolArgUsage")]
    tool_arg_usage: BTreeMap<String, usize>,
    #[serde(default, rename = "ToolAuthority")]
    tool_authority: BTreeMap<String, usize>,
    #[serde(default, rename = "HighestAuthority")]
    highest_authority: String,
    #[serde(default, rename = "ReasoningBlocks")]
    reasoning_blocks: usize,
    #[serde(default, rename = "ReasoningChars")]
    reasoning_chars: usize,
    #[serde(default, rename = "ReasoningLens")]
    reasoning_lens: Vec<usize>,
    #[serde(default, rename = "ReasoningRedact")]
    reasoning_redact: usize,
    #[serde(default, rename = "TokensInput")]
    tokens_input: i64,
    #[serde(default, rename = "TokensOutput")]
    tokens_output: i64,
    #[serde(default, rename = "TokensCacheW")]
    tokens_cache_w: i64,
    #[serde(default, rename = "TokensCacheR")]
    tokens_cache_r: i64,
    #[serde(default, rename = "GapsSec")]
    gaps_sec: Vec<f64>,
    #[serde(default, rename = "ModelUsed")]
    model_used: String,
    #[serde(default, rename = "SourceTool")]
    source_tool: String,
    #[serde(default, rename = "SessionStart")]
    session_start: String,
    #[serde(default, rename = "SessionEnd")]
    session_end: String,
    #[serde(default, rename = "DurationSec")]
    duration_sec: f64,
    #[serde(default, rename = "CostEstimated")]
    cost_estimated: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct GoAnomaly {
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    severity: String,
    #[serde(default)]
    emoji: String,
    #[serde(default)]
    detail: String,
}

pub fn session_cache_path() -> PathBuf {
    if let Some(dir) = std::env::var_os("AGENTTRACE_SESSION_CACHE_DIR").map(PathBuf::from) {
        if !dir.as_os_str().is_empty() {
            return dir.join("sessions.json");
        }
    }
    user_cache_dir().join("agenttrace").join("sessions.json")
}

pub fn clear_session_cache() -> anyhow::Result<()> {
    let cache = session_cache_path();
    for path in [
        cache.clone(),
        cache.with_file_name("hermes-sqlite.json"),
        cache.with_file_name("opencode-sqlite.json"),
    ] {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }
    }
    Ok(())
}

pub(crate) fn load_sqlite_snapshot(database: &Path, name: &str) -> Option<Vec<Session>> {
    load_sqlite_snapshot_from(database, &sqlite_snapshot_path(name))
}

fn load_sqlite_snapshot_from(database: &Path, snapshot_path: &Path) -> Option<Vec<Session>> {
    let raw = fs::read(snapshot_path).ok()?;
    let snapshot = serde_json::from_slice::<SqliteSnapshot>(&raw).ok()?;
    if snapshot.schema_version != SQLITE_SNAPSHOT_SCHEMA_VERSION
        || snapshot.database != file_fingerprint(database)?
        || snapshot.wal != file_fingerprint(&sqlite_wal_path(database))
    {
        return None;
    }
    Some(
        snapshot
            .sessions
            .into_iter()
            .map(|session| session.into_session(&database.to_string_lossy()))
            .collect(),
    )
}

pub(crate) fn store_sqlite_snapshot(
    database: &Path,
    name: &str,
    sessions: &[Session],
) -> anyhow::Result<()> {
    store_sqlite_snapshot_at(database, &sqlite_snapshot_path(name), sessions)
}

fn store_sqlite_snapshot_at(
    database: &Path,
    path: &Path,
    sessions: &[Session],
) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let snapshot = SqliteSnapshot {
        schema_version: SQLITE_SNAPSHOT_SCHEMA_VERSION,
        database: file_fingerprint(database).ok_or_else(|| anyhow::anyhow!("database missing"))?,
        wal: file_fingerprint(&sqlite_wal_path(database)),
        sessions: sessions.iter().map(GoSession::from_session).collect(),
    };
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec(&snapshot)?)?;
    fs::rename(tmp, path)?;
    Ok(())
}

fn sqlite_snapshot_path(name: &str) -> PathBuf {
    session_cache_path().with_file_name(format!("{name}-sqlite.json"))
}

fn sqlite_wal_path(database: &Path) -> PathBuf {
    PathBuf::from(format!("{}-wal", database.to_string_lossy()))
}

fn file_fingerprint(path: &Path) -> Option<FileFingerprint> {
    let metadata = fs::metadata(path).ok()?;
    Some(FileFingerprint {
        mod_time: file_mod_time_nanos(&metadata),
        size: metadata.len() as i64,
    })
}

pub fn load_session_cache() -> SessionCache {
    let path = session_cache_path();
    let Ok(raw) = fs::read_to_string(&path) else {
        return SessionCache {
            path,
            ..SessionCache::default()
        };
    };
    let Ok(Value::Object(doc)) = serde_json::from_str::<Value>(&raw) else {
        return SessionCache {
            path,
            ..SessionCache::default()
        };
    };
    if doc.get("schema_version").and_then(Value::as_i64) != Some(SESSION_CACHE_SCHEMA_VERSION) {
        return SessionCache {
            path,
            dirty: true,
            ..SessionCache::default()
        };
    }
    let raw_entries = doc
        .get("entries")
        .and_then(Value::as_object)
        .map(|entries| {
            entries
                .iter()
                .filter(|(_, value)| decode_cache_entry_header(value).is_some())
                .map(|(path, value)| (path.clone(), value.clone()))
                .collect()
        })
        .unwrap_or_default();
    let dirs = doc
        .get("dirs")
        .and_then(Value::as_object)
        .map(|dirs| {
            dirs.iter()
                .filter_map(|(path, value)| {
                    serde_json::from_value::<DirCacheEntry>(value.clone())
                        .ok()
                        .map(|entry| (path.clone(), entry))
                })
                .collect()
        })
        .unwrap_or_default();
    SessionCache {
        path,
        raw_entries,
        dirs,
        ..SessionCache::default()
    }
}

pub fn load_cached_sessions(dir: Option<&Path>) -> Vec<Session> {
    let mut cache = load_session_cache();
    load_cached_sessions_from_cache(dir, &mut cache)
}

pub fn load_cached_sessions_from_cache(
    dir: Option<&Path>,
    cache: &mut SessionCache,
) -> Vec<Session> {
    let paths = cache
        .raw_entries
        .keys()
        .chain(cache.entries.keys())
        .map(PathBuf::from)
        .collect::<BTreeSet<_>>();
    let sessions = paths
        .into_iter()
        .filter(|path| dir.map_or(true, |dir| path.starts_with(dir)))
        .filter_map(|path| cached_session(&path, cache))
        .collect();
    if cache.is_dirty() {
        let _ = save_session_cache(cache);
    }
    sessions
}

impl SessionCache {
    pub fn entry_count(&self) -> usize {
        let mut count = self.raw_entries.len();
        for path in self.entries.keys() {
            if !self.raw_entries.contains_key(path) {
                count += 1;
            }
        }
        count
    }

    pub fn dir_count(&self) -> usize {
        self.dirs.len()
    }

    pub(crate) fn is_dirty(&self) -> bool {
        self.dirty
    }
}

pub(crate) fn cached_dir_listing(dir: &Path, cache: &mut SessionCache) -> Option<CachedDirListing> {
    let key = cache_key(dir);
    let metadata = fs::metadata(dir).ok()?;
    if !metadata.is_dir() {
        return None;
    }
    let entry = cache.dirs.get(&key)?;
    if entry.mod_time != file_mod_time_nanos(&metadata) {
        cache.dirs.remove(&key);
        cache.dirty = true;
        return None;
    }
    Some(CachedDirListing {
        files: entry.files.iter().map(PathBuf::from).collect(),
        dirs: entry.dirs.iter().map(PathBuf::from).collect(),
    })
}

pub(crate) fn store_dir_listing(
    dir: &Path,
    files: &[PathBuf],
    dirs: &[PathBuf],
    cache: &mut SessionCache,
) -> anyhow::Result<()> {
    let metadata = fs::metadata(dir)?;
    cache.dirs.insert(
        cache_key(dir),
        DirCacheEntry {
            mod_time: file_mod_time_nanos(&metadata),
            files: files.iter().map(|path| cache_key(path)).collect(),
            dirs: dirs.iter().map(|path| cache_key(path)).collect(),
        },
    );
    cache.dirty = true;
    Ok(())
}

pub(crate) fn cached_file_mod_time_if_fresh(
    path: &Path,
    metadata: &fs::Metadata,
    cache: &mut SessionCache,
) -> Option<i64> {
    let key = cache_key(path);
    let header = cached_entry_header(&key, cache)?;
    if header.size == metadata.len() as i64 && header.mod_time == file_mod_time_nanos(metadata) {
        return Some(header.mod_time);
    }
    delete_cached_session_key(&key, cache);
    None
}

fn decode_cache_entry_header(value: &Value) -> Option<CacheEntryHeader> {
    Some(CacheEntryHeader {
        mod_time: value.get("mod_time")?.as_i64()?,
        size: value.get("size")?.as_i64()?,
    })
}

fn cached_entry_header(path: &str, cache: &mut SessionCache) -> Option<CacheEntryHeader> {
    if let Some(entry) = cache.entries.get(path) {
        return Some(CacheEntryHeader {
            mod_time: entry.mod_time,
            size: entry.size,
        });
    }
    cache
        .raw_entries
        .get(path)
        .and_then(decode_cache_entry_header)
}

fn cached_entry(path: &str, cache: &mut SessionCache) -> Option<CacheEntry> {
    if let Some(entry) = cache.entries.get(path) {
        return Some(entry.clone());
    }
    let raw = cache.raw_entries.get(path)?.clone();
    let Ok(entry) = serde_json::from_value::<CacheEntry>(raw) else {
        delete_cached_session_key(path, cache);
        return None;
    };
    cache.entries.insert(path.to_string(), entry.clone());
    Some(entry)
}

fn cached_entry_missing_tool_warnings(path: &str, cache: &SessionCache) -> bool {
    cache
        .raw_entries
        .get(path)
        .and_then(|entry| entry.get("session").or_else(|| entry.get("Session")))
        .and_then(Value::as_object)
        .is_some_and(|session| {
            !session.contains_key("ToolWarnings") && !session.contains_key("tool_warnings")
        })
}

fn cached_entry_missing_tool_arg_usage(path: &str, cache: &SessionCache) -> bool {
    cache
        .raw_entries
        .get(path)
        .and_then(|entry| entry.get("session").or_else(|| entry.get("Session")))
        .and_then(|session| session.get("Metrics").or_else(|| session.get("metrics")))
        .and_then(Value::as_object)
        .is_some_and(|metrics| {
            !metrics.contains_key("ToolArgUsage") && !metrics.contains_key("tool_arg_usage")
        })
}

fn cached_entry_empty_source_tool(path: &str, cache: &SessionCache) -> bool {
    cache
        .raw_entries
        .get(path)
        .and_then(|entry| entry.get("session").or_else(|| entry.get("Session")))
        .and_then(|session| session.get("Metrics").or_else(|| session.get("metrics")))
        .and_then(Value::as_object)
        .is_some_and(|metrics| {
            metrics
                .get("SourceTool")
                .or_else(|| metrics.get("source_tool"))
                .and_then(Value::as_str)
                .is_some_and(str::is_empty)
        })
}

pub(crate) fn delete_cached_session(path: &Path, cache: &mut SessionCache) {
    delete_cached_session_key(&cache_key(path), cache);
}

fn delete_cached_session_key(path: &str, cache: &mut SessionCache) {
    if cache.entries.remove(path).is_some() || cache.raw_entries.remove(path).is_some() {
        cache.dirty = true;
    }
}

pub fn save_session_cache(cache: &SessionCache) -> anyhow::Result<()> {
    if let Some(parent) = cache.path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut doc = Map::new();
    doc.insert(
        "schema_version".to_string(),
        Value::Number(SESSION_CACHE_SCHEMA_VERSION.into()),
    );
    let mut entries = Map::new();
    for (path, value) in &cache.raw_entries {
        entries.insert(path.clone(), value.clone());
    }
    for (path, entry) in &cache.entries {
        entries.insert(
            path.clone(),
            serde_json::to_value(entry).expect("cache entry serialize"),
        );
    }
    doc.insert("entries".to_string(), Value::Object(entries));
    if !cache.dirs.is_empty() {
        let dirs = cache
            .dirs
            .iter()
            .map(|(path, entry)| {
                (
                    path.clone(),
                    serde_json::to_value(entry).expect("dir cache entry serialize"),
                )
            })
            .collect();
        doc.insert("dirs".to_string(), Value::Object(dirs));
    }
    let tmp = cache.path.with_file_name(format!(
        "{}.tmp",
        cache
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("sessions.json")
    ));
    fs::write(&tmp, serde_json::to_vec(&Value::Object(doc))?)?;
    fs::rename(tmp, &cache.path)?;
    Ok(())
}

pub fn cached_session(path: &Path, cache: &mut SessionCache) -> Option<Session> {
    let key = cache_key(path);
    let header = cached_entry_header(&key, cache)?;
    if !is_fresh(path, &header) {
        delete_cached_session_key(&key, cache);
        return None;
    }
    if cached_entry_missing_tool_warnings(&key, cache) {
        delete_cached_session_key(&key, cache);
        return None;
    }
    if cached_entry_missing_tool_arg_usage(&key, cache) {
        delete_cached_session_key(&key, cache);
        return None;
    }
    if cached_entry_empty_source_tool(&key, cache) {
        delete_cached_session_key(&key, cache);
        return None;
    }
    let entry = cached_entry(&key, cache)?;
    Some(entry.session.clone().into_session(&key))
}

pub fn store_session(
    path: &Path,
    session: &Session,
    cache: &mut SessionCache,
) -> anyhow::Result<()> {
    let metadata = fs::metadata(path)?;
    let key = cache_key(path);
    cache.entries.insert(
        key.clone(),
        CacheEntry {
            mod_time: file_mod_time_nanos(&metadata),
            size: metadata.len() as i64,
            session: GoSession::from_session(session),
        },
    );
    cache.raw_entries.remove(&key);
    cache.dirty = true;
    Ok(())
}

impl GoSession {
    fn from_session(session: &Session) -> Self {
        Self {
            name: session.name.clone(),
            path: session.path.clone(),
            cwd: session.cwd.clone(),
            metrics: GoMetrics::from_metrics(&session.metrics),
            anomalies: session
                .anomalies
                .iter()
                .map(GoAnomaly::from_anomaly)
                .collect(),
            health: session.health,
            tool_warnings: session
                .tool_warnings
                .iter()
                .map(GoToolWarning::from_tool_warning)
                .collect(),
            diagnostics: session.diagnostics.clone(),
        }
    }

    fn into_session(self, fallback_path: &str) -> Session {
        Session {
            name: self.name,
            path: if self.path.is_empty() {
                fallback_path.to_string()
            } else {
                self.path
            },
            cwd: self.cwd,
            metrics: self.metrics.into_metrics(),
            anomalies: self
                .anomalies
                .into_iter()
                .map(GoAnomaly::into_anomaly)
                .collect(),
            health: self.health,
            tool_warnings: self
                .tool_warnings
                .into_iter()
                .map(GoToolWarning::into_tool_warning)
                .collect(),
            diagnostics: self.diagnostics,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct GoToolWarning {
    #[serde(default, rename = "ToolName")]
    tool_name: String,
    #[serde(default, rename = "Pattern")]
    pattern: String,
    #[serde(default, rename = "Count")]
    count: usize,
    #[serde(default, rename = "Detail")]
    detail: String,
    #[serde(default, rename = "Severity")]
    severity: String,
}

impl GoToolWarning {
    fn from_tool_warning(warning: &ToolWarning) -> Self {
        Self {
            tool_name: warning.tool_name.clone(),
            pattern: warning.pattern.clone(),
            count: warning.count,
            detail: warning.detail.clone(),
            severity: warning.severity.clone(),
        }
    }

    fn into_tool_warning(self) -> ToolWarning {
        ToolWarning {
            tool_name: self.tool_name,
            pattern: self.pattern,
            count: self.count,
            detail: self.detail,
            severity: self.severity,
        }
    }
}

impl GoMetrics {
    fn from_metrics(metrics: &Metrics) -> Self {
        Self {
            events_total: metrics.events_total,
            user_messages: metrics.user_messages,
            assistant_turns: metrics.assistant_turns,
            tool_results: metrics.tool_results,
            tool_calls_total: metrics.tool_calls_total,
            tool_calls_ok: metrics.tool_calls_ok,
            tool_calls_fail: metrics.tool_calls_fail,
            tool_usage: metrics.tool_usage.clone(),
            file_usage: metrics.file_usage.clone(),
            tool_arg_usage: metrics.tool_arg_usage.clone(),
            tool_authority: metrics.tool_authority.clone(),
            highest_authority: metrics.highest_authority.clone(),
            reasoning_blocks: metrics.reasoning_blocks,
            reasoning_chars: metrics.reasoning_chars,
            reasoning_lens: metrics.reasoning_lens.clone(),
            reasoning_redact: metrics.reasoning_redact,
            tokens_input: metrics.tokens_input,
            tokens_output: metrics.tokens_output,
            tokens_cache_w: metrics.tokens_cache_w,
            tokens_cache_r: metrics.tokens_cache_r,
            gaps_sec: metrics.gaps_sec.clone(),
            model_used: metrics.model_used.clone(),
            source_tool: metrics.source_tool.clone(),
            session_start: metrics.session_start.clone(),
            session_end: metrics.session_end.clone(),
            duration_sec: metrics.duration_sec,
            cost_estimated: metrics.cost_estimated,
        }
    }

    fn into_metrics(self) -> Metrics {
        Metrics {
            events_total: self.events_total,
            user_messages: self.user_messages,
            assistant_turns: self.assistant_turns,
            tool_results: self.tool_results,
            tool_calls_total: self.tool_calls_total,
            tool_calls_ok: self.tool_calls_ok,
            tool_calls_fail: self.tool_calls_fail,
            tool_usage: self.tool_usage,
            file_usage: self.file_usage,
            tool_arg_usage: self.tool_arg_usage,
            tool_authority: self.tool_authority,
            highest_authority: self.highest_authority,
            reasoning_blocks: self.reasoning_blocks,
            reasoning_chars: self.reasoning_chars,
            reasoning_lens: self.reasoning_lens,
            reasoning_redact: self.reasoning_redact,
            tokens_input: self.tokens_input,
            tokens_output: self.tokens_output,
            tokens_cache_w: self.tokens_cache_w,
            tokens_cache_r: self.tokens_cache_r,
            timestamps: Vec::new(),
            gaps_sec: self.gaps_sec,
            model_used: self.model_used,
            source_tool: self.source_tool,
            session_start: self.session_start,
            session_end: self.session_end,
            duration_sec: self.duration_sec,
            cost_estimated: self.cost_estimated,
        }
    }
}

impl GoAnomaly {
    fn from_anomaly(anomaly: &Anomaly) -> Self {
        Self {
            kind: anomaly.kind.clone(),
            severity: anomaly.severity.clone(),
            emoji: anomaly_emoji(&anomaly.severity).to_string(),
            detail: anomaly.detail.clone(),
        }
    }

    fn into_anomaly(self) -> Anomaly {
        Anomaly {
            kind: self.kind,
            severity: self.severity,
            detail: self.detail,
        }
    }
}

fn user_cache_dir() -> PathBuf {
    if cfg!(target_os = "macos") {
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            return home.join("Library").join("Caches");
        }
    }
    if let Some(cache) = std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from) {
        if !cache.as_os_str().is_empty() {
            return cache;
        }
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        return home.join(".cache");
    }
    std::env::temp_dir()
}

fn cache_key(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn is_fresh(path: &Path, entry: &CacheEntryHeader) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    entry.size == metadata.len() as i64 && entry.mod_time == file_mod_time_nanos(&metadata)
}

#[cfg(unix)]
fn file_mod_time_nanos(metadata: &fs::Metadata) -> i64 {
    use std::os::unix::fs::MetadataExt;
    metadata.mtime() * 1_000_000_000 + metadata.mtime_nsec()
}

#[cfg(not(unix))]
fn file_mod_time_nanos(metadata: &fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

fn anomaly_emoji(severity: &str) -> &'static str {
    match severity {
        "high" => "🔴",
        "medium" => "🟡",
        "low" => "🟢",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_snapshot_is_invalidated_by_database_or_wal_changes() {
        let root = std::env::temp_dir().join(format!(
            "agenttrace-sqlite-cache-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let database = root.join("state.db");
        let snapshot = root.join("snapshot.json");
        fs::create_dir_all(&root).expect("create temp dir");
        fs::write(&database, b"db").expect("write database");
        let session = Session {
            name: "cached".to_string(),
            path: database.to_string_lossy().to_string(),
            cwd: String::new(),
            metrics: Metrics::default(),
            anomalies: Vec::new(),
            health: 100,
            tool_warnings: Vec::new(),
            diagnostics: Diagnostics::default(),
        };

        store_sqlite_snapshot_at(&database, &snapshot, &[session]).expect("store snapshot");
        assert_eq!(
            load_sqlite_snapshot_from(&database, &snapshot)
                .expect("cache hit")
                .len(),
            1
        );

        fs::write(sqlite_wal_path(&database), b"wal").expect("write wal");
        assert!(load_sqlite_snapshot_from(&database, &snapshot).is_none());
        let _ = fs::remove_dir_all(root);
    }
}
