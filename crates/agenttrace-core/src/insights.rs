use crate::{parse_ts, pricing, total_tokens, Session};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct SessionComparison {
    pub outcome: &'static str,
    pub reasons: Vec<&'static str>,
}

pub fn compare_session_outcome(current: &Session, previous: &Session) -> SessionComparison {
    let outcome = match (
        current.metrics.duration_sec <= previous.metrics.duration_sec,
        current.metrics.cost_estimated <= previous.metrics.cost_estimated,
    ) {
        (true, true) => "faster_cheaper",
        (true, false) => "faster_costlier",
        (false, true) => "slower_cheaper",
        (false, false) => "slower_costlier",
    };
    let mut reasons = Vec::new();
    if current.metrics.tool_calls_fail < previous.metrics.tool_calls_fail {
        reasons.push("fewer_failures");
    }
    if current.diagnostics.loop_cost.total_loop_cost
        < previous.diagnostics.loop_cost.total_loop_cost
    {
        reasons.push("less_repeated_work");
    }
    if reasons.is_empty() {
        reasons.push("review_metric_changes");
    }
    SessionComparison { outcome, reasons }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TimeRange {
    Today,
    Days7,
    Days30,
    #[default]
    All,
}

impl TimeRange {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "today" | "day" | "1d" => Some(Self::Today),
            "7d" | "week" | "weekly" => Some(Self::Days7),
            "30d" | "month" | "monthly" => Some(Self::Days30),
            "all" | "" => Some(Self::All),
            _ => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Today => "today",
            Self::Days7 => "7d",
            Self::Days30 => "30d",
            Self::All => "all",
        }
    }
    pub fn since(self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        match self {
            Self::Today => now
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .map(|value| value.and_utc()),
            Self::Days7 => Some(now - chrono::Duration::days(7)),
            Self::Days30 => Some(now - chrono::Duration::days(30)),
            Self::All => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectIdentity {
    pub id: String,
    pub display_name: String,
    pub root: String,
    pub resolution: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ReportScope {
    pub generated_at: String,
    pub range: String,
    pub earliest_session_at: String,
    pub latest_session_at: String,
    pub sessions_in_scope: usize,
    pub sources: Vec<SourceScope>,
    pub includes_sqlite_derived_sessions: bool,
    pub includes_preserved_history: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct SourceScope {
    pub source: String,
    pub sessions: usize,
    pub tokens: i64,
    pub estimated_cost: f64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DataHealth {
    pub discovered: usize,
    pub parsed: usize,
    pub skipped: usize,
    pub cache_hits: usize,
    pub unknown_sources: usize,
    pub unknown_models: usize,
    pub fallback_pricing: usize,
    pub latest_session_at: String,
    pub confidence: String,
    pub with_tokens: usize,
    pub with_duration: usize,
    pub with_tools: usize,
    pub with_event_timing: usize,
    pub with_diagnostics: usize,
}

pub fn session_capability(session: &Session) -> &'static str {
    let m = &session.metrics;
    if !m.gaps_sec.is_empty()
        || !session.diagnostics.tool_latencies.is_empty()
        || session.diagnostics.context_utilization.estimated_total > 0
    {
        "detailed"
    } else if m.duration_sec > 0.0
        || m.tokens_input + m.tokens_output > 0
        || m.tool_calls_total > 0
        || !matches!(m.model_used.as_str(), "" | "default" | "unknown")
    {
        "aggregate"
    } else {
        "limited"
    }
}

pub fn resolve_project(session: &Session) -> ProjectIdentity {
    let decoded;
    let raw = if !session.cwd.trim().is_empty() {
        session.cwd.trim()
    } else if let Some(path) = decode_agent_project_dir(&session.path) {
        decoded = path;
        decoded.as_str()
    } else {
        ""
    };
    if raw.is_empty() || raw.starts_with("history:") {
        return ProjectIdentity {
            id: "unknown".to_string(),
            display_name: "unknown".to_string(),
            root: String::new(),
            resolution: "unattributed".to_string(),
        };
    }
    let path = lexical_normalize(Path::new(raw));
    let (root, resolution) = git_root(&path)
        .map(|root| (root, "git_root".to_string()))
        .unwrap_or_else(|| (path.clone(), "cwd".to_string()));
    let display_name = root
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("unknown")
        .to_string();
    let root = root.to_string_lossy().to_string();
    ProjectIdentity {
        id: root.clone(),
        display_name,
        root,
        resolution,
    }
}

// Claude Code and Cursor name project folders after the workspace path with separators
// replaced by '-'; recover the real directory only when that path still exists.
fn decode_agent_project_dir(session_path: &str) -> Option<String> {
    let path = Path::new(session_path);
    let dir = path.ancestors().find_map(|ancestor| {
        let parent = ancestor.parent()?;
        matches!(parent.file_name()?.to_str()?, "projects")
            .then(|| ancestor.file_name()?.to_str())
            .flatten()
    })?;
    let encoded = dir.trim_start_matches('-');
    if encoded.is_empty() {
        return None;
    }
    let mut current = PathBuf::from("/");
    let mut pending = String::new();
    for part in encoded.split('-') {
        pending = if pending.is_empty() {
            part.to_string()
        } else {
            format!("{pending}-{part}")
        };
        for candidate in [
            pending.clone(),
            pending.replace('-', "."),
            pending.replace('-', "_"),
        ] {
            let next = current.join(&candidate);
            if next.is_dir() {
                current = next;
                pending.clear();
                break;
            }
        }
    }
    (pending.is_empty() && current != Path::new("/")).then(|| current.to_string_lossy().to_string())
}

pub fn project_name(session: &Session) -> String {
    resolve_project(session).display_name
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::RootDir => out.push(Path::new("/")),
            Component::Prefix(prefix) => out.push(prefix.as_os_str()),
            Component::Normal(value) => out.push(value),
        }
    }
    out
}

// Project views resolve thousands of sessions per frame; the filesystem walk is memoized per cwd.
fn git_root(path: &Path) -> Option<PathBuf> {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Option<PathBuf>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(hit) = cache.lock().ok().and_then(|map| map.get(path).cloned()) {
        return hit;
    }
    let found = find_git_root(path);
    if let Ok(mut map) = cache.lock() {
        map.insert(path.to_path_buf(), found.clone());
    }
    found
}

fn find_git_root(path: &Path) -> Option<PathBuf> {
    let mut current = if path.is_dir() { path } else { path.parent()? };
    loop {
        let marker = current.join(".git");
        if marker.is_dir() {
            return Some(current.to_path_buf());
        }
        if marker.is_file() {
            return Some(worktree_main_root(&marker).unwrap_or_else(|| current.to_path_buf()));
        }
        current = current.parent()?;
    }
}

// Linked worktrees carry a `.git` file ("gitdir: <repo>/.git/worktrees/<name>"); group them
// under the main checkout so every agent worktree counts as the same project.
fn worktree_main_root(marker: &Path) -> Option<PathBuf> {
    let content = std::fs::read_to_string(marker).ok()?;
    let gitdir = PathBuf::from(content.strip_prefix("gitdir:")?.trim());
    let git = gitdir.parent()?.parent()?;
    (git.file_name()? == ".git").then(|| git.parent().map(Path::to_path_buf))?
}

pub fn filter_sessions(
    sessions: &[Session],
    range: TimeRange,
    project: &str,
    source: &str,
    model: &str,
    now: DateTime<Utc>,
) -> Vec<Session> {
    sessions
        .iter()
        .filter(|session| {
            session_matches_time_range(session, range, now)
                && project_matches(session, project)
                && contains(&session.metrics.source_tool, source)
                && contains(&session.metrics.model_used, model)
        })
        .cloned()
        .collect()
}
pub fn session_matches_time_range(session: &Session, range: TimeRange, now: DateTime<Utc>) -> bool {
    range.since(now).map_or(true, |since| {
        parse_ts(&session.metrics.session_start).is_some_and(|time| time >= since)
    })
}

pub fn report_scope(
    sessions: &[Session],
    range: TimeRange,
    includes_preserved_history: bool,
) -> ReportScope {
    let mut sources: BTreeMap<String, SourceScope> = BTreeMap::new();
    let mut earliest = None;
    let mut latest = None;
    for session in sessions {
        if let Some(time) = parse_ts(&session.metrics.session_start) {
            earliest = Some(earliest.map_or(time, |current: DateTime<Utc>| current.min(time)));
            latest = Some(latest.map_or(time, |current: DateTime<Utc>| current.max(time)));
        }
        let entry = sources
            .entry(session.metrics.source_tool.clone())
            .or_insert_with(|| SourceScope {
                source: session.metrics.source_tool.clone(),
                ..SourceScope::default()
            });
        entry.sessions += 1;
        entry.tokens += total_tokens(session);
        entry.estimated_cost += session.metrics.cost_estimated;
    }
    ReportScope {
        generated_at: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        range: range.label().to_string(),
        earliest_session_at: earliest
            .map(|time: DateTime<Utc>| time.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
            .unwrap_or_default(),
        latest_session_at: latest
            .map(|time: DateTime<Utc>| time.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
            .unwrap_or_default(),
        sessions_in_scope: sessions.len(),
        sources: sources.into_values().collect(),
        includes_sqlite_derived_sessions: sessions.iter().any(|session| {
            matches!(
                session.metrics.source_tool.as_str(),
                "hermes_db" | "opencode_db"
            )
        }),
        includes_preserved_history,
    }
}

pub fn data_health(sessions: &[Session], discovered: usize, cache_hits: usize) -> DataHealth {
    let parsed = sessions.len();
    let unknown_sources = sessions
        .iter()
        .filter(|s| matches!(s.metrics.source_tool.as_str(), "" | "generic" | "unknown"))
        .count();
    let unknown_models = sessions
        .iter()
        .filter(|s| matches!(s.metrics.model_used.as_str(), "" | "default" | "unknown"))
        .count();
    let fallback_pricing = sessions
        .iter()
        .filter(|s| !pricing::has_specific_price(&s.metrics.model_used))
        .count();
    let skipped = discovered.saturating_sub(parsed);
    DataHealth {
        discovered,
        parsed,
        skipped,
        cache_hits,
        unknown_sources,
        unknown_models,
        fallback_pricing,
        latest_session_at: sessions
            .iter()
            .filter_map(|s| parse_ts(&s.metrics.session_start))
            .max()
            .map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
            .unwrap_or_default(),
        confidence: if parsed == 0 || skipped > 0 || unknown_sources > 0 || unknown_models > 0 {
            "low"
        } else if fallback_pricing > 0 {
            "medium"
        } else {
            "high"
        }
        .to_string(),
        with_tokens: sessions
            .iter()
            .filter(|s| s.metrics.tokens_input + s.metrics.tokens_output > 0)
            .count(),
        with_duration: sessions
            .iter()
            .filter(|s| s.metrics.duration_sec > 0.0)
            .count(),
        with_tools: sessions
            .iter()
            .filter(|s| s.metrics.tool_calls_total > 0)
            .count(),
        with_event_timing: sessions
            .iter()
            .filter(|s| !s.metrics.gaps_sec.is_empty())
            .count(),
        with_diagnostics: sessions
            .iter()
            .filter(|s| session_capability(s) == "detailed")
            .count(),
    }
}

fn project_matches(session: &Session, filter: &str) -> bool {
    if filter.trim().is_empty() {
        return true;
    }
    let project = resolve_project(session);
    contains(&project.id, filter)
        || contains(&project.display_name, filter)
        || contains(&project.root, filter)
}

fn contains(value: &str, filter: &str) -> bool {
    filter.trim().is_empty()
        || value
            .to_ascii_lowercase()
            .contains(&filter.trim().to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn session_at(cwd: &str, path: &str) -> Session {
        Session {
            name: "s".to_string(),
            path: path.to_string(),
            cwd: cwd.to_string(),
            metrics: crate::Metrics::default(),
            anomalies: Vec::new(),
            health: 100,
            tool_warnings: Vec::new(),
            diagnostics: crate::Diagnostics::default(),
        }
    }

    #[test]
    fn projects_group_worktrees_and_decode_agent_dirs() {
        let root = std::env::temp_dir().join(format!("agenttrace-project-{}", std::process::id()));
        let repo = root.join("src").join("my-repo");
        let worktree = root.join("worktrees").join("a1b2").join("my-repo");
        fs::create_dir_all(repo.join(".git").join("worktrees").join("wt")).unwrap();
        fs::create_dir_all(&worktree).unwrap();
        fs::write(
            worktree.join(".git"),
            format!("gitdir: {}\n", repo.join(".git/worktrees/wt").display()),
        )
        .unwrap();

        let main = resolve_project(&session_at(&repo.to_string_lossy(), "/x.jsonl"));
        let linked = resolve_project(&session_at(&worktree.to_string_lossy(), "/x.jsonl"));
        assert_eq!(linked.id, main.id);
        assert_eq!(linked.display_name, "my-repo");

        // Agent project-dir encoding is defined for '/'-separated absolute paths.
        if cfg!(unix) {
            let encoded = repo.to_string_lossy().replace('/', "-");
            let transcript = root.join("projects").join(&encoded).join("session.jsonl");
            let decoded = resolve_project(&session_at("", &transcript.to_string_lossy()));
            assert_eq!(decoded.id, main.id);
        }

        let missing = resolve_project(&session_at("", "/nowhere/projects/-gone-dir/s.jsonl"));
        assert_eq!(missing.display_name, "unknown");
        let _ = fs::remove_dir_all(root);
    }
}
