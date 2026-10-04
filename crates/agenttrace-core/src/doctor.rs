use crate::i18n::{tr, Language, Message};
use crate::{
    cached_session, find_session_files, known_session_dirs, load_session_cache,
    load_sqlite_backed_sessions, parse_file, skip_sqlite_backed_file_dir, Session, VERSION,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct DoctorReport {
    pub version: String,
    pub mode: String,
    pub cache_path: String,
    pub cache_entries: usize,
    pub cache_dirs: usize,
    pub cached_valid: usize,
    pub sessions: usize,
    pub session_files: usize,
    pub directories: Vec<DoctorDirReport>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorDirReport {
    pub name: String,
    pub path: String,
    pub exists: bool,
    pub files: usize,
    pub parsed: usize,
    pub failed: usize,
    pub cache_hits: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub failure_samples: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct SessionCacheReport {
    path: PathBuf,
    entries: BTreeMap<String, CacheEntryHeader>,
    dirs: usize,
}

#[derive(Debug, Clone, Default)]
struct CacheEntryHeader {
    mod_time: i64,
    size: i64,
}

pub fn render_doctor_report(
    dir: Option<&Path>,
    demo: bool,
    format: &str,
) -> anyhow::Result<String> {
    render_doctor_report_with_language(dir, demo, format, Language::En)
}

pub fn render_doctor_report_with_language(
    dir: Option<&Path>,
    demo: bool,
    format: &str,
    language: Language,
) -> anyhow::Result<String> {
    let report = build_doctor_report(dir, demo);
    if format == "json" {
        Ok(serde_json::to_string_pretty(&report)? + "\n")
    } else {
        Ok(doctor_report_text(&report, dir, demo, language))
    }
}

pub fn build_doctor_report(dir: Option<&Path>, demo: bool) -> DoctorReport {
    let cache = load_session_cache_report();
    let files = if dir.is_none() {
        find_reportable_session_files(None)
    } else {
        find_session_files(dir)
    };
    let sqlite_sessions = if dir.is_none() && !demo {
        load_sqlite_backed_sessions()
    } else {
        Vec::new()
    };
    let cached_valid = valid_cached_session_count(&files, &cache);
    let mode = if demo {
        "demo sessions"
    } else if dir.is_some() {
        "custom directory"
    } else {
        "auto-discovery"
    };
    let mut report = DoctorReport {
        version: VERSION.to_string(),
        mode: mode.to_string(),
        cache_path: cache.path.to_string_lossy().to_string(),
        cache_entries: cache.entries.len(),
        cache_dirs: cache.dirs,
        cached_valid,
        sessions: files.len() + sqlite_sessions.len(),
        session_files: files.len(),
        directories: doctor_directories(dir, &files, &sqlite_sessions),
        recommendations: Vec::new(),
    };
    report.recommendations = doctor_recommendations(&report, dir, demo);
    report
}

fn find_reportable_session_files(dir: Option<&Path>) -> Vec<PathBuf> {
    if dir.is_some() {
        return find_session_files(dir);
    }
    let mut out = Vec::new();
    for candidate in crate::discover_session_dirs() {
        if skip_sqlite_backed_file_dir(&candidate) {
            continue;
        }
        out.extend(crate::collect_session_files(&candidate));
    }
    out
}

fn doctor_directories(
    dir: Option<&Path>,
    files: &[PathBuf],
    sqlite_sessions: &[Session],
) -> Vec<DoctorDirReport> {
    let mut cache = load_session_cache();
    if let Some(dir) = dir {
        let abs = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
        return vec![doctor_dir_report("custom", &abs, files, &mut cache)];
    }

    let mut count_by_root = BTreeMap::new();
    for candidate in known_session_dirs() {
        count_by_root.insert(candidate.path, 0usize);
    }
    for file in files {
        for (root, count) in &mut count_by_root {
            if is_under(file, root) {
                *count += 1;
            }
        }
    }

    let mut dirs = Vec::new();
    for candidate in known_session_dirs() {
        let matching = files
            .iter()
            .filter(|file| is_under(file, &candidate.path))
            .cloned()
            .collect::<Vec<_>>();
        dirs.push(doctor_dir_report(
            &candidate.name,
            &candidate.path,
            &matching,
            &mut cache,
        ));
    }
    dirs.extend(doctor_sqlite_directories(sqlite_sessions));
    dirs
}

fn doctor_dir_report(
    name: &str,
    path: &Path,
    files: &[PathBuf],
    cache: &mut crate::SessionCache,
) -> DoctorDirReport {
    let mut parsed = 0;
    let mut cache_hits = 0;
    let mut failure_samples = Vec::new();
    for file in files {
        if cached_session(file, cache).is_some() {
            cache_hits += 1;
            parsed += 1;
        } else if parse_file(file).is_ok() {
            parsed += 1;
        } else if failure_samples.len() < 3 {
            failure_samples.push(file.to_string_lossy().to_string());
        }
    }
    DoctorDirReport {
        name: name.to_string(),
        path: path.to_string_lossy().to_string(),
        exists: path.is_dir(),
        files: files.len(),
        parsed,
        failed: files.len().saturating_sub(parsed),
        cache_hits,
        failure_samples,
    }
}

fn doctor_sqlite_directories(sessions: &[Session]) -> Vec<DoctorDirReport> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };
    let mut count_by_tool = BTreeMap::new();
    for session in sessions {
        *count_by_tool
            .entry(session.metrics.source_tool.clone())
            .or_insert(0usize) += 1;
    }
    let candidates = [
        (
            "hermes_db",
            "Hermes Agent (DB)",
            home.join(".hermes").join("state.db"),
        ),
        (
            "opencode_db",
            "OpenCode (DB)",
            home.join(".local")
                .join("share")
                .join("opencode")
                .join("opencode.db"),
        ),
    ];
    let mut dirs = Vec::new();
    for (tool, name, path) in candidates {
        let files = count_by_tool.get(tool).copied().unwrap_or(0);
        let exists = path.is_file();
        if !exists && files == 0 {
            continue;
        }
        dirs.push(DoctorDirReport {
            name: name.to_string(),
            path: path.to_string_lossy().to_string(),
            exists,
            files,
            parsed: files,
            failed: 0,
            cache_hits: 0,
            failure_samples: Vec::new(),
        });
    }
    dirs
}

fn doctor_recommendation_keys(
    report: &DoctorReport,
    dir: Option<&Path>,
    demo: bool,
) -> Vec<&'static str> {
    if report.sessions == 0 {
        if dir.is_some() {
            return vec!["doctor.rec.none_in_dir"];
        }
        return vec!["doctor.rec.none"];
    }
    let mut keys = vec!["doctor.rec.ready"];
    if demo {
        keys.push("doctor.rec.demo_cache");
        return keys;
    }
    if report.cached_valid == 0 {
        keys.push("doctor.rec.no_reusable");
    }
    keys
}

fn doctor_recommendations(report: &DoctorReport, dir: Option<&Path>, demo: bool) -> Vec<String> {
    doctor_recommendation_keys(report, dir, demo)
        .into_iter()
        .map(|key| tr(Language::En, key).to_string())
        .collect()
}

fn doctor_report_text(
    report: &DoctorReport,
    dir: Option<&Path>,
    demo: bool,
    language: Language,
) -> String {
    let t = |key| tr(language, key);
    let mode = match report.mode.as_str() {
        "demo sessions" => t("doctor.mode.demo"),
        "custom directory" => t("doctor.mode.custom"),
        "auto-discovery" => t("doctor.mode.auto"),
        other => other,
    };
    let mut out = String::new();
    out.push_str(&format!("{}\n", t("doctor.title")));
    out.push_str(&format!("{}: {}\n", t("doctor.version"), report.version));
    out.push_str(&format!("{}: {}\n", t("doctor.mode_label"), mode));
    out.push_str(&format!(
        "{}: {}\n",
        t("doctor.session_files"),
        report.sessions
    ));
    out.push_str(&format!("{}: {}\n", t("doctor.cache"), report.cache_path));
    out.push_str(&format!(
        "  {}\n",
        Message::new("doctor.cache_summary")
            .arg("entries", report.cache_entries)
            .arg("reusable", report.cached_valid)
            .arg("dirs", report.cache_dirs)
            .render_or(language, "")
    ));
    out.push_str(&format!("\n{}:\n", t("doctor.providers")));
    for dir in &report.directories {
        let status = if dir.exists {
            t("doctor.found")
        } else {
            t("doctor.missing")
        };
        out.push_str(&format!(
            "  {:20} {:7} found={:<5} parsed={:<5} failed={:<5} cache={:<5} {}\n",
            dir.name, status, dir.files, dir.parsed, dir.failed, dir.cache_hits, dir.path
        ));
        for sample in &dir.failure_samples {
            out.push_str(&format!("    {}: {sample}\n", t("doctor.failed")));
        }
    }
    out.push_str(&format!("\n{}:\n", t("doctor.recommendations")));
    for key in doctor_recommendation_keys(report, dir, demo) {
        out.push_str(&format!("  - {}\n", t(key)));
    }
    out
}

fn load_session_cache_report() -> SessionCacheReport {
    let path = session_cache_path();
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return SessionCacheReport {
            path,
            ..SessionCacheReport::default()
        };
    };
    let Ok(Value::Object(doc)) = serde_json::from_str::<Value>(&raw) else {
        return SessionCacheReport {
            path,
            ..SessionCacheReport::default()
        };
    };
    let mut entries = BTreeMap::new();
    if let Some(raw_entries) = doc.get("entries").and_then(Value::as_object) {
        for (path, entry) in raw_entries {
            if let Some(header) = decode_cache_entry_header(entry) {
                entries.insert(path.clone(), header);
            }
        }
    }
    let dirs = doc
        .get("dirs")
        .and_then(Value::as_object)
        .map(|dirs| {
            dirs.values()
                .filter(|entry| entry.get("mod_time").and_then(Value::as_i64).is_some())
                .count()
        })
        .unwrap_or(0);
    SessionCacheReport {
        path,
        entries,
        dirs,
    }
}

fn session_cache_path() -> PathBuf {
    if let Some(dir) = std::env::var_os("AGENTTRACE_SESSION_CACHE_DIR").map(PathBuf::from) {
        if !dir.as_os_str().is_empty() {
            return dir.join("sessions.json");
        }
    }
    user_cache_dir().join("agenttrace").join("sessions.json")
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

fn decode_cache_entry_header(value: &Value) -> Option<CacheEntryHeader> {
    Some(CacheEntryHeader {
        mod_time: value.get("mod_time")?.as_i64()?,
        size: value.get("size")?.as_i64()?,
    })
}

fn valid_cached_session_count(paths: &[PathBuf], cache: &SessionCacheReport) -> usize {
    let mut valid = 0;
    let mut seen = HashSet::new();
    for path in paths {
        let key = path.to_string_lossy().to_string();
        if !seen.insert(key.clone()) {
            continue;
        }
        let Some(entry) = cache.entries.get(&key) else {
            continue;
        };
        let Ok(metadata) = path.metadata() else {
            continue;
        };
        if entry.size != metadata.len() as i64 {
            continue;
        }
        if file_mod_time_nanos(&metadata) == Some(entry.mod_time) {
            valid += 1;
        }
    }
    valid
}

#[cfg(unix)]
fn file_mod_time_nanos(metadata: &std::fs::Metadata) -> Option<i64> {
    use std::os::unix::fs::MetadataExt;
    Some(metadata.mtime() * 1_000_000_000 + metadata.mtime_nsec())
}

#[cfg(not(unix))]
fn file_mod_time_nanos(metadata: &std::fs::Metadata) -> Option<i64> {
    let modified = metadata.modified().ok()?;
    let duration = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(duration.as_nanos() as i64)
}

fn is_under(path: &Path, root: &Path) -> bool {
    path == root || path.starts_with(root)
}
