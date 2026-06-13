use agenttrace_core::{
    add_baseline_comparison, average_health, compute_overview, data_health, demo_sessions,
    evaluate_overview_gate, filter_sessions, find_session_files, fix_suggestions, inspect_first,
    load_sessions_with_options, parse_file, predict_cost_anomaly, pricing_cache_path,
    render_doctor_report, render_model_pricing_list, render_test_match,
    render_waste_report_with_language, report_compare_json, report_json_with_language,
    report_overview_html, report_overview_json_with_health, report_overview_markdown,
    report_overview_text, report_search_json, report_search_text, report_text_with_language,
    search_sessions, session_capability, tool_fail_rate, total_tokens, update_pricing,
    BaselineThresholds, LoadOptions, LoadReport, ReportLanguage, Session, TimeRange, VERSION,
};
use anyhow::{bail, Context};
use chrono::{DateTime, Utc};
use clap::Parser;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Parser)]
#[command(name = "agenttrace")]
#[command(about = "TUI observability for AI coding agent sessions")]
struct Args {
    path: Option<String>,
    #[arg(short = 'f', long = "format", default_value = "text")]
    format: String,
    #[arg(short = 'd')]
    dir: Option<String>,
    #[arg(long)]
    compare: bool,
    #[arg(long)]
    overview: bool,
    #[arg(long)]
    sessions: bool,
    #[arg(long)]
    diagnostics: bool,
    #[arg(long)]
    inspect: Option<usize>,
    #[arg(short = 'm', default_value = "default")]
    model: String,
    #[arg(short = 'o')]
    output: Option<PathBuf>,
    #[arg(long)]
    latest: bool,
    #[arg(long)]
    waste: bool,
    #[arg(long = "list-models")]
    list_models: bool,
    #[arg(long = "update-pricing")]
    update_pricing: bool,
    #[arg(long = "test-match")]
    test_match: bool,
    #[arg(long)]
    version: bool,
    #[arg(long)]
    demo: bool,
    #[arg(long)]
    doctor: bool,
    #[arg(long)]
    search: Option<String>,
    #[arg(long = "search-limit", default_value_t = 20)]
    search_limit: usize,
    #[arg(long = "fail-under-health", default_value_t = 0)]
    fail_under_health: i32,
    #[arg(long = "fail-on-critical")]
    fail_on_critical: bool,
    #[arg(long = "max-tool-fail-rate")]
    max_tool_fail_rate: Option<f64>,
    #[arg(long)]
    baseline: Option<String>,
    #[arg(long = "baseline-max-duration-delta-pct", default_value_t = 0.0)]
    baseline_max_duration_delta_pct: f64,
    #[arg(long = "baseline-max-cost-delta-pct", default_value_t = 0.0)]
    baseline_max_cost_delta_pct: f64,
    #[arg(long = "baseline-max-token-delta-pct", default_value_t = 0.0)]
    baseline_max_token_delta_pct: f64,
    #[arg(long = "lang", default_value = "en")]
    lang: String,
    #[arg(long, default_value = "all")]
    range: String,
    #[arg(long, default_value = "")]
    project: String,
    #[arg(long, default_value = "")]
    source: String,
    #[arg(long = "model-filter", default_value = "")]
    model_filter: String,
    #[arg(long, default_value = "")]
    query: String,
    #[arg(long, default_value = "")]
    health: String,
    #[arg(long, default_value = "")]
    cost: String,
    #[arg(long, default_value = "")]
    anomaly: String,
    #[arg(long, default_value = "recent")]
    sort: String,
    #[arg(long, default_value = "desc")]
    order: String,
    #[arg(long, default_value_t = 20)]
    limit: usize,
    #[arg(long = "clear-cache")]
    clear_cache: bool,
    #[arg(long = "preserve-history")]
    preserve_history: bool,
    #[arg(long = "include-history")]
    include_history: bool,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let args = Args::parse_from(go_flag_compatible_args(std::env::args_os()));
    let language = report_language(&args.lang);

    if args.version {
        println!("agenttrace v{}", VERSION);
        return Ok(());
    }

    if args.clear_cache {
        agenttrace_core::clear_session_cache()?;
        println!("Session cache cleared.");
        if !has_session_action(&args) {
            return Ok(());
        }
    }

    if args.update_pricing {
        println!("Downloading pricing from LiteLLM...");
        let count = update_pricing()?;
        println!("Loaded {} model prices", count);
        println!("Cache saved: {}", pricing_cache_path().display());
        if !has_post_pricing_action(&args) {
            return Ok(());
        }
    }

    if args.test_match {
        print!("{}", render_test_match());
        return Ok(());
    }

    if args.doctor {
        let doctor_dir = args.dir.as_deref().map(PathBuf::from);
        let out = render_doctor_report(doctor_dir.as_deref(), args.demo, &args.format)?;
        write_output(&args.output, &out)?;
        print!("{out}");
        return Ok(());
    }

    if args.list_models {
        print!("{}", render_model_pricing_list());
        return Ok(());
    }

    if !has_session_action(&args) {
        if args.demo {
            let sessions = demo_sessions()?;
            return agenttrace_tui::run_with_sessions(sessions, "demo");
        }
        return agenttrace_tui::run(args.dir.as_deref().unwrap_or(""));
    }
    if args.baseline.is_some() && !args.overview {
        bail!("--baseline requires --overview -f json");
    }

    if args.compare {
        let sessions = load_sessions_for_compare(&args)?;
        if sessions.is_empty() {
            bail!(
                "No session files found in {}",
                args.dir.as_deref().unwrap_or("")
            );
        }
        let out = if args.format == "json" {
            report_compare_json(&sessions)
        } else {
            agenttrace_core::report_compare_with_language(&sessions, &args.model, language)
        };
        write_output(&args.output, &(out.clone() + "\n"))?;
        print!("{out}");
        return Ok(());
    }

    if args.waste {
        let session = load_latest_session(&args)?;
        let out = render_waste_report_with_language(&session, language);
        write_output(&args.output, &(out.clone() + "\n"))?;
        print!("{out}");
        return Ok(());
    }

    if (args.latest || args.path.is_some())
        && !args.sessions
        && !args.diagnostics
        && args.inspect.is_none()
    {
        let session = load_latest_session(&args)?;
        let out = match args.format.as_str() {
            "json" => report_json_with_language(&session, language),
            _ => report_text_with_language(&session, language),
        };
        write_output(&args.output, &(out.clone() + "\n"))?;
        print!("{out}");
        return Ok(());
    }

    let (sessions, load_report) = load_sessions_report(&args)?;

    if args.sessions || args.diagnostics || args.inspect.is_some() {
        let sessions = prepare_cli_view(sessions, &args)?;
        if sessions.is_empty() {
            bail!("No sessions match the requested filters");
        }
        if args.sessions {
            let out = render_session_list(&sessions, &args.format, args.limit);
            write_output(&args.output, &(out.clone() + "\n"))?;
            print!("{out}");
            return Ok(());
        }
        let session = if let Some(rank) = args.inspect {
            if rank == 0 {
                bail!("--inspect rank starts at 1");
            }
            let item = inspect_first(&sessions)
                .get(rank - 1)
                .cloned()
                .context("inspect rank exceeds available priority sessions")?;
            &sessions[item.index]
        } else if args.latest {
            latest_session(&sessions).expect("sessions checked non-empty")
        } else {
            sessions.first().expect("sessions checked non-empty")
        };
        let out = render_diagnostics(session, &sessions, &args.format, language)?;
        write_output(&args.output, &(out.clone() + "\n"))?;
        print!("{out}");
        return Ok(());
    }

    if let Some(query) = args.search.as_deref() {
        let results = search_sessions(&sessions, query, args.search_limit);
        let out = if args.format == "json" {
            report_search_json(&results)
        } else {
            report_search_text(&results, query)
        };
        write_output(&args.output, &(out.clone() + "\n"))?;
        print!("{out}");
        return Ok(());
    }

    if args.overview {
        let overview = compute_overview(&sessions);
        let mut out = match args.format.as_str() {
            "json" => {
                let health = data_health(
                    &sessions,
                    sessions.len() + load_report.as_ref().map(|item| item.skipped).unwrap_or(0),
                    load_report
                        .as_ref()
                        .map(|item| item.cache_hits)
                        .unwrap_or(0),
                );
                report_overview_json_with_health(&overview, &sessions, Some(&health))
            }
            "markdown" | "md" => report_overview_markdown(&overview, &sessions),
            "html" => report_overview_html(&overview, &sessions),
            _ => report_overview_text(&overview, &sessions),
        };
        if let Some(baseline) = args.baseline.as_deref() {
            if args.format != "json" {
                bail!("--baseline requires --overview -f json");
            }
            out = add_baseline_comparison(
                &out,
                baseline,
                BaselineThresholds {
                    max_duration_delta_pct: args.baseline_max_duration_delta_pct,
                    max_cost_delta_pct: args.baseline_max_cost_delta_pct,
                    max_token_delta_pct: args.baseline_max_token_delta_pct,
                },
            )?;
        }
        write_output(&args.output, &(out.clone() + "\n"))?;
        print!("{out}");
        let failures = evaluate_overview_gate(
            &overview,
            &sessions,
            args.fail_under_health,
            args.fail_on_critical,
            args.max_tool_fail_rate,
        );
        if !failures.is_empty() {
            for failure in failures {
                eprintln!("Gate failed: {failure}");
            }
            eprintln!("Local evidence:");
            eprintln!("- avg health: {:.1}", average_health(&sessions));
            eprintln!("- critical sessions: {}", overview.critical);
            eprintln!("- tool fail rate: {:.1}%", tool_fail_rate(&sessions));
            if let Some(session) = sessions.iter().min_by(|left, right| {
                left.health
                    .cmp(&right.health)
                    .then_with(|| left.path.cmp(&right.path))
                    .then_with(|| left.name.cmp(&right.name))
            }) {
                eprintln!("- lowest-health session: {}", session.path);
            }
            let inspect = if args.demo {
                "agenttrace --demo --overview -f json".to_string()
            } else if let Some(dir) = args.dir.as_deref() {
                format!("agenttrace -d {:?} --overview -f json", dir)
            } else {
                "agenttrace --overview -f json".to_string()
            };
            eprintln!("- inspect: `{inspect}`");
            std::process::exit(2);
        }
        return Ok(());
    }

    bail!("no report action selected")
}

fn go_flag_compatible_args<I>(args: I) -> Vec<OsString>
where
    I: IntoIterator<Item = OsString>,
{
    let mut args = args.into_iter();
    let mut out = Vec::new();
    if let Some(program) = args.next() {
        out.push(program);
    }

    let mut expecting_value = false;
    while let Some(arg) = args.next() {
        if expecting_value {
            out.push(arg);
            expecting_value = false;
            continue;
        }
        if arg == "--" {
            out.push(arg);
            if let Some(path) = args.next() {
                out.push(path);
            }
            break;
        }
        if is_go_flag_positional(&arg) {
            out.push(arg);
            break;
        }
        expecting_value = flag_takes_value(&arg);
        out.push(arg);
    }

    out
}

fn is_go_flag_positional(arg: &OsString) -> bool {
    let text = arg.to_string_lossy();
    text == "-" || !text.starts_with('-')
}

fn flag_takes_value(arg: &OsString) -> bool {
    let text = arg.to_string_lossy();
    if text.contains('=') {
        return false;
    }
    matches!(
        text.as_ref(),
        "-f" | "--format"
            | "-d"
            | "-m"
            | "-o"
            | "--search"
            | "--search-limit"
            | "--fail-under-health"
            | "--max-tool-fail-rate"
            | "--baseline"
            | "--baseline-max-duration-delta-pct"
            | "--baseline-max-cost-delta-pct"
            | "--baseline-max-token-delta-pct"
            | "--lang"
            | "--range"
            | "--project"
            | "--source"
            | "--model-filter"
            | "--query"
            | "--health"
            | "--cost"
            | "--anomaly"
            | "--sort"
            | "--order"
            | "--limit"
            | "--inspect"
    )
}

fn latest_session(sessions: &[Session]) -> Option<&Session> {
    sessions.iter().max_by(|a, b| newer_session_order(a, b))
}

fn newer_session_order(a: &Session, b: &Session) -> std::cmp::Ordering {
    let a_has_session_time = !a.metrics.session_start.is_empty();
    let b_has_session_time = !b.metrics.session_start.is_empty();
    a_has_session_time
        .cmp(&b_has_session_time)
        .then_with(|| {
            if a_has_session_time && b_has_session_time {
                a.metrics.session_start.cmp(&b.metrics.session_start)
            } else {
                session_mod_time(a).cmp(&session_mod_time(b))
            }
        })
        .then_with(|| a.path.cmp(&b.path))
}

fn session_mod_time(session: &Session) -> SystemTime {
    fs::metadata(&session.path)
        .and_then(|metadata| metadata.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

fn report_language(value: &str) -> ReportLanguage {
    match value.to_ascii_lowercase().as_str() {
        "zh" | "zh-cn" | "zh_cn" | "chinese" => ReportLanguage::Zh,
        _ => ReportLanguage::En,
    }
}

fn load_sessions(args: &Args) -> anyhow::Result<Vec<Session>> {
    load_sessions_report(args).map(|(sessions, _)| sessions)
}

fn load_sessions_report(args: &Args) -> anyhow::Result<(Vec<Session>, Option<LoadReport>)> {
    if args.demo {
        return Ok((prepare_explicit_sessions(demo_sessions()?, args)?, None));
    }
    if let Some(path) = args.path.as_deref() {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok((
                prepare_explicit_sessions(vec![parse_file(&path)?], args)?,
                None,
            ));
        }
        if path.is_dir() {
            if is_cline_task_dir(&path) {
                return Ok((
                    prepare_explicit_sessions(vec![parse_file(&path)?], args)?,
                    None,
                ));
            }
            bail!(
                "Error loading {}: positional path must be a session file",
                path.display()
            );
        }
        bail!("session path does not exist: {}", path.display());
    }
    let dir = args.dir.as_deref().map(PathBuf::from);
    let (sessions, report) = if dir.is_none() {
        let range = parse_range(args)?;
        let report = load_sessions_with_options(
            None,
            &LoadOptions {
                since: range.since(Utc::now()),
                project: args.project.clone(),
                source: args.source.clone(),
                model: args.model_filter.clone(),
                include_history: args.include_history,
                preserve_history: args.preserve_history,
            },
        );
        (report.sessions.clone(), Some(report))
    } else {
        (
            prepare_explicit_sessions(load_parseable_sessions_from_files(dir.as_deref())?, args)?,
            None,
        )
    };
    if sessions.is_empty() {
        bail!(
            "No session files found in {}",
            args.dir.as_deref().unwrap_or("")
        );
    }
    Ok((sessions, report))
}

fn parse_range(args: &Args) -> anyhow::Result<TimeRange> {
    TimeRange::parse(&args.range).context("range must be today, 7d, 30d, or all")
}

fn filter_cli_sessions(sessions: Vec<Session>, args: &Args) -> anyhow::Result<Vec<Session>> {
    Ok(filter_sessions(
        &sessions,
        parse_range(args)?,
        &args.project,
        &args.source,
        &args.model_filter,
        Utc::now(),
    ))
}

fn prepare_explicit_sessions(
    mut sessions: Vec<Session>,
    args: &Args,
) -> anyhow::Result<Vec<Session>> {
    if args.preserve_history {
        agenttrace_core::preserve_derived_history(&sessions)?;
    }
    if args.include_history {
        agenttrace_core::merge_preserved_history(&mut sessions);
    }
    filter_cli_sessions(sessions, args)
}

fn load_latest_session(args: &Args) -> anyhow::Result<Session> {
    if args.demo {
        let sessions = demo_sessions()?;
        return latest_session(&sessions)
            .cloned()
            .context("no demo sessions");
    }
    if let Some(path) = args.path.as_deref() {
        let path = PathBuf::from(path);
        if path.is_file() || is_cline_task_dir(&path) {
            return parse_file(&path);
        }
        if path.is_dir() {
            bail!(
                "Error loading {}: positional path must be a session file",
                path.display()
            );
        }
        bail!("session path does not exist: {}", path.display());
    }
    let dir = args.dir.as_deref().map(PathBuf::from);
    let files = find_session_files(dir.as_deref());
    if files.is_empty() {
        bail!(
            "No session files found in {}",
            args.dir.as_deref().unwrap_or("")
        );
    }
    let target = latest_session_file(&files).context("No session files found.")?;
    parse_file(target)
}

fn load_parseable_sessions_from_files(
    dir: Option<&std::path::Path>,
) -> anyhow::Result<Vec<Session>> {
    let files = find_session_files(dir);
    if files.is_empty() {
        return Ok(Vec::new());
    }
    Ok(files
        .iter()
        .filter_map(|path| parse_file(path).ok())
        .collect())
}

fn latest_session_file(files: &[PathBuf]) -> Option<&std::path::Path> {
    files
        .iter()
        .filter_map(|path| latest_session_candidate(path).map(|candidate| (path, candidate)))
        .max_by(|(_, a), (_, b)| newer_session_candidate_order(a, b))
        .map(|(path, _)| path.as_path())
}

struct LatestCandidate {
    session_time: Option<DateTime<Utc>>,
    mod_time: SystemTime,
    path: String,
}

fn latest_session_candidate(path: &std::path::Path) -> Option<LatestCandidate> {
    let mod_time = fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()?;
    let session_time = parse_file(path)
        .ok()
        .and_then(|session| parse_session_time(&session.metrics.session_start));
    Some(LatestCandidate {
        session_time,
        mod_time,
        path: path.to_string_lossy().to_string(),
    })
}

fn parse_session_time(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|time| time.with_timezone(&Utc))
}

fn newer_session_candidate_order(a: &LatestCandidate, b: &LatestCandidate) -> std::cmp::Ordering {
    a.session_time
        .is_some()
        .cmp(&b.session_time.is_some())
        .then_with(|| match (a.session_time, b.session_time) {
            (Some(a_time), Some(b_time)) => a_time.cmp(&b_time),
            _ => a.mod_time.cmp(&b.mod_time),
        })
        .then_with(|| a.path.cmp(&b.path))
}

fn load_sessions_for_compare(args: &Args) -> anyhow::Result<Vec<Session>> {
    if args.demo {
        return load_sessions(args);
    }
    let dir = args.dir.as_deref().map(PathBuf::from);
    let mut files = find_session_files(dir.as_deref());
    if files.len() > 15 {
        eprintln!(
            "Found {} session files, showing the most recent 15. Use -d <dir> or remove old sessions to compare all.",
            files.len()
        );
        files.truncate(15);
    }
    Ok(files
        .iter()
        .filter_map(|path| parse_file(path).ok())
        .collect())
}

fn is_cline_task_dir(path: &std::path::Path) -> bool {
    path.join("api_conversation_history.json").is_file()
        || path.join("ui_messages.json").is_file()
        || path.join("task_metadata.json").is_file()
}

fn write_output(path: &Option<PathBuf>, content: &str) -> anyhow::Result<()> {
    if let Some(path) = path {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, content)?;
        eprintln!("Saved: {}", path.display());
    }
    Ok(())
}

fn prepare_cli_view(mut sessions: Vec<Session>, args: &Args) -> anyhow::Result<Vec<Session>> {
    validate_view_filters(args)?;
    sessions.retain(|session| {
        matches_text(session, &args.query)
            && matches_health(session.health, &args.health)
            && matches_number(session.metrics.cost_estimated, &args.cost)
            && (args.anomaly.is_empty()
                || session.anomalies.iter().any(|item| {
                    args.anomaly.eq_ignore_ascii_case("any")
                        || item
                            .kind
                            .to_ascii_lowercase()
                            .contains(&args.anomaly.to_ascii_lowercase())
                        || item
                            .detail
                            .to_ascii_lowercase()
                            .contains(&args.anomaly.to_ascii_lowercase())
                }))
    });
    let descending = match args.order.as_str() {
        "asc" => false,
        "desc" => true,
        _ => bail!("--order must be asc or desc"),
    };
    sessions.sort_by(|left, right| {
        let ordering = match args.sort.as_str() {
            "recent" | "time" => left.metrics.session_start.cmp(&right.metrics.session_start),
            "health" => left.health.cmp(&right.health),
            "cost" => left
                .metrics
                .cost_estimated
                .total_cmp(&right.metrics.cost_estimated),
            "turns" => left
                .metrics
                .assistant_turns
                .cmp(&right.metrics.assistant_turns),
            "failures" => left
                .metrics
                .tool_calls_fail
                .cmp(&right.metrics.tool_calls_fail),
            "source" => left.metrics.source_tool.cmp(&right.metrics.source_tool),
            "name" => left.name.cmp(&right.name),
            "anomalies" => left.anomalies.len().cmp(&right.anomalies.len()),
            _ => std::cmp::Ordering::Equal,
        };
        let ordering = if descending {
            ordering.reverse()
        } else {
            ordering
        };
        ordering.then_with(|| left.path.cmp(&right.path))
    });
    if !matches!(
        args.sort.as_str(),
        "recent"
            | "time"
            | "health"
            | "cost"
            | "turns"
            | "failures"
            | "source"
            | "name"
            | "anomalies"
    ) {
        bail!("unsupported --sort value: {}", args.sort);
    }
    Ok(sessions)
}

fn matches_text(session: &Session, query: &str) -> bool {
    let query = query.trim().to_ascii_lowercase();
    query.is_empty()
        || [
            session.name.as_str(),
            session.path.as_str(),
            session.cwd.as_str(),
            session.metrics.source_tool.as_str(),
            session.metrics.model_used.as_str(),
        ]
        .iter()
        .any(|value| value.to_ascii_lowercase().contains(&query))
        || session
            .metrics
            .tool_usage
            .keys()
            .chain(session.metrics.file_usage.keys())
            .any(|value| value.to_ascii_lowercase().contains(&query))
        || session.anomalies.iter().any(|item| {
            item.kind.to_ascii_lowercase().contains(&query)
                || item.detail.to_ascii_lowercase().contains(&query)
        })
}

fn matches_health(health: i32, filter: &str) -> bool {
    match filter.trim().to_ascii_lowercase().as_str() {
        "" => true,
        "good" | "healthy" => health >= 80,
        "warn" | "warning" => (50..80).contains(&health),
        "crit" | "critical" => health < 50,
        filter => matches_number(health as f64, filter),
    }
}

fn validate_view_filters(args: &Args) -> anyhow::Result<()> {
    if !args.health.is_empty()
        && !matches!(
            args.health.to_ascii_lowercase().as_str(),
            "good" | "healthy" | "warn" | "warning" | "crit" | "critical"
        )
        && !valid_number_filter(&args.health)
    {
        bail!("invalid --health filter: {}", args.health);
    }
    if !args.cost.is_empty() && !valid_number_filter(&args.cost) {
        bail!("invalid --cost filter: {}", args.cost);
    }
    Ok(())
}

fn valid_number_filter(filter: &str) -> bool {
    [">=", "<=", ">", "<", "="]
        .iter()
        .find_map(|prefix| filter.strip_prefix(prefix))
        .is_some_and(|value| value.parse::<f64>().is_ok())
}

fn matches_number(value: f64, filter: &str) -> bool {
    let filter = filter.trim();
    if filter.is_empty() {
        return true;
    }
    for (prefix, compare) in [
        (
            ">=",
            std::cmp::Ordering::is_ge as fn(std::cmp::Ordering) -> bool,
        ),
        ("<=", std::cmp::Ordering::is_le),
        (">", std::cmp::Ordering::is_gt),
        ("<", std::cmp::Ordering::is_lt),
        ("=", std::cmp::Ordering::is_eq),
    ] {
        if let Some(raw) = filter.strip_prefix(prefix) {
            return raw
                .parse::<f64>()
                .ok()
                .is_some_and(|target| compare(value.total_cmp(&target)));
        }
    }
    false
}

fn render_session_list(sessions: &[Session], format: &str, limit: usize) -> String {
    let sessions = sessions.iter().take(limit).collect::<Vec<_>>();
    if format == "json" {
        return serde_json::to_string_pretty(&sessions).expect("sessions serialize");
    }
    let mut lines =
        vec!["SESSION\tHEALTH\tDATA\tSOURCE\tMODEL\tCOST\tTOKENS\tFAIL\tANOMALIES".to_string()];
    lines.extend(sessions.into_iter().map(|session| {
        format!(
            "{}\t{}\t{}\t{}\t{}\t{:.4}\t{}\t{}\t{}",
            session.name,
            session.health,
            session_capability(session),
            session.metrics.source_tool,
            session.metrics.model_used,
            session.metrics.cost_estimated,
            total_tokens(session),
            session.metrics.tool_calls_fail,
            session.anomalies.len()
        )
    }));
    lines.join("\n")
}

fn render_diagnostics(
    session: &Session,
    history: &[Session],
    format: &str,
    language: ReportLanguage,
) -> anyhow::Result<String> {
    let alert = predict_cost_anomaly(history, session);
    let fixes = fix_suggestions(session);
    if format == "json" {
        return Ok(serde_json::to_string_pretty(&serde_json::json!({
            "session": session,
            "cost_alert": alert,
            "fix_suggestions": fixes,
        }))?);
    }
    let mut out = report_text_with_language(session, language);
    out.push_str("\nDiagnostics\n-----------\n");
    out.push_str(&serde_json::to_string_pretty(&session.diagnostics)?);
    if alert.triggered {
        out.push_str(&format!(
            "\nCost alert [{}]: {}",
            alert.level, alert.message
        ));
    }
    for fix in fixes {
        out.push_str(&format!("\nFix [{}]: {}", fix.severity, fix.action));
    }
    Ok(out)
}

fn has_post_pricing_action(args: &Args) -> bool {
    args.path.is_some()
        || args.list_models
        || args.test_match
        || args.doctor
        || args.latest
        || args.compare
        || args.overview
        || args.sessions
        || args.diagnostics
        || args.inspect.is_some()
        || args.waste
        || args
            .search
            .as_deref()
            .map(|query| !query.trim().is_empty())
            .unwrap_or(false)
}

fn has_session_action(args: &Args) -> bool {
    args.path.is_some()
        || args.latest
        || args.compare
        || args.overview
        || args.sessions
        || args.diagnostics
        || args.inspect.is_some()
        || args.waste
        || args.baseline.is_some()
        || args
            .search
            .as_deref()
            .map(|query| !query.trim().is_empty())
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use agenttrace_core::Metrics;
    use std::io::Write;

    #[test]
    fn latest_session_prefers_session_timestamp_over_mod_time() {
        let newer_file = temp_session_file("agenttrace-newer-mtime", "newer-mtime");
        let older_file = temp_session_file("agenttrace-older-mtime", "older-mtime");
        let older_mtime_session = session("older-time", &newer_file, "2026-01-01T00:00:00Z");
        let newer_session_time = session("newer-time", &older_file, "2026-01-02T00:00:00Z");

        assert_eq!(
            latest_session(&[older_mtime_session, newer_session_time])
                .map(|session| session.name.as_str()),
            Some("newer-time")
        );

        let _ = fs::remove_file(newer_file);
        let _ = fs::remove_file(older_file);
    }

    #[test]
    fn latest_session_uses_mod_time_when_timestamps_are_missing() {
        let older = temp_session_file("agenttrace-older-modtime", "older");
        std::thread::sleep(std::time::Duration::from_millis(5));
        let newer = temp_session_file("agenttrace-newer-modtime", "newer");
        let older_session = session("older", &older, "");
        let newer_session = session("newer", &newer, "");

        assert_eq!(
            latest_session(&[newer_session, older_session]).map(|session| session.name.as_str()),
            Some("newer")
        );

        let _ = fs::remove_file(older);
        let _ = fs::remove_file(newer);
    }

    #[test]
    fn latest_session_breaks_mod_time_ties_by_path() {
        let alpha = session_with_missing_file("alpha", "/tmp/agenttrace-alpha.jsonl");
        let omega = session_with_missing_file("omega", "/tmp/agenttrace-omega.jsonl");

        assert_eq!(
            latest_session(&[alpha, omega]).map(|session| session.name.as_str()),
            Some("omega")
        );
    }

    #[test]
    fn cli_view_filters_sorts_and_renders_diagnostics_json() {
        let mut args = compare_args(None);
        args.sessions = true;
        args.health = "crit".to_string();
        args.sort = "cost".to_string();
        args.order = "desc".to_string();
        let mut critical = session("critical", "/tmp/critical", "2026-01-01T00:00:00Z");
        critical.health = 40;
        critical.metrics.cost_estimated = 2.0;
        critical.metrics.tool_calls_fail = 2;
        let mut healthy = session("healthy", "/tmp/healthy", "2026-01-02T00:00:00Z");
        healthy.metrics.cost_estimated = 3.0;

        let filtered = prepare_cli_view(vec![healthy, critical], &args).expect("filter");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "critical");
        assert!(render_session_list(&filtered, "json", 20).contains("\"critical\""));
        let diagnostics = render_diagnostics(&filtered[0], &filtered, "json", ReportLanguage::En)
            .expect("diagnostics");
        assert!(diagnostics.contains("\"diagnostics\""));
        assert!(diagnostics.contains("\"cost_alert\""));

        args.cost = "expensive".to_string();
        assert!(prepare_cli_view(filtered, &args).is_err());
    }

    #[test]
    fn go_flag_compatible_args_ignore_flags_after_positional_path() {
        let args = go_flag_compatible_args([
            OsString::from("agenttrace"),
            OsString::from("session.jsonl"),
            OsString::from("-f"),
            OsString::from("json"),
        ]);

        assert_eq!(
            args,
            vec![
                OsString::from("agenttrace"),
                OsString::from("session.jsonl")
            ]
        );
    }

    #[test]
    fn go_flag_compatible_args_keep_flags_before_positional_path() {
        let args = go_flag_compatible_args([
            OsString::from("agenttrace"),
            OsString::from("-f"),
            OsString::from("json"),
            OsString::from("session.jsonl"),
        ]);

        assert_eq!(
            args,
            vec![
                OsString::from("agenttrace"),
                OsString::from("-f"),
                OsString::from("json"),
                OsString::from("session.jsonl"),
            ]
        );
    }

    #[test]
    fn compare_loader_uses_go_file_cap() {
        let root =
            std::env::temp_dir().join(format!("agenttrace-compare-cap-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create compare temp dir");
        for idx in 0..16 {
            write_compare_session(&root.join(format!("{idx:02}.jsonl")), idx);
        }

        let args = compare_args(Some(root.to_string_lossy().to_string()));
        let sessions = load_sessions_for_compare(&args).expect("load compare sessions");
        assert_eq!(sessions.len(), 15);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn load_sessions_reports_empty_after_parse_failures_like_go_overview() {
        let root = std::env::temp_dir().join(format!(
            "agenttrace-empty-after-parse-failure-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create parse failure dir");
        fs::write(root.join("storage.json"), r#"{"not":"#).expect("write bad json");

        let mut args = compare_args(Some(root.to_string_lossy().to_string()));
        args.compare = false;
        args.overview = true;
        let err = load_sessions(&args).expect_err("empty parseable sessions should fail");
        assert!(err.to_string().contains("No session files found in"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn latest_session_file_can_select_unparseable_newest_file_like_go() {
        let root =
            std::env::temp_dir().join(format!("agenttrace-latest-bad-file-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create latest bad file dir");
        let older = root.join("older.jsonl");
        let newer = root.join("newer.json");
        fs::write(
            &older,
            r#"{"role":"user","content":"older no timestamp","SourceTool":"generic"}
{"role":"assistant","content":"older answer","SourceTool":"generic"}
"#,
        )
        .expect("write older session");
        std::thread::sleep(std::time::Duration::from_millis(5));
        fs::write(&newer, r#"{"not":"#).expect("write bad latest");

        let files = find_session_files(Some(&root));
        let latest = latest_session_file(&files).expect("latest file");
        assert_eq!(latest, newer.as_path());

        let mut args = compare_args(Some(root.to_string_lossy().to_string()));
        args.compare = false;
        args.latest = true;
        load_latest_session(&args).expect_err("bad latest should fail");

        let _ = fs::remove_dir_all(root);
    }

    fn temp_session_file(prefix: &str, content: &str) -> String {
        let path = std::env::temp_dir().join(format!("{prefix}-{}.jsonl", std::process::id()));
        let mut file = fs::File::create(&path).expect("create temp session");
        writeln!(file, "{content}").expect("write temp session");
        path.to_string_lossy().to_string()
    }

    fn session(name: &str, path: &str, session_start: &str) -> Session {
        Session {
            name: name.to_string(),
            path: path.to_string(),
            cwd: String::new(),
            metrics: Metrics {
                session_start: session_start.to_string(),
                ..Metrics::default()
            },
            anomalies: Vec::new(),
            health: 100,
            tool_warnings: Vec::new(),
            diagnostics: agenttrace_core::Diagnostics::default(),
        }
    }

    fn session_with_missing_file(name: &str, path: &str) -> Session {
        session(name, path, "")
    }

    fn compare_args(dir: Option<String>) -> Args {
        Args {
            path: None,
            format: "json".to_string(),
            dir,
            compare: true,
            overview: false,
            sessions: false,
            diagnostics: false,
            inspect: None,
            model: "default".to_string(),
            output: None,
            latest: false,
            waste: false,
            list_models: false,
            update_pricing: false,
            test_match: false,
            version: false,
            demo: false,
            doctor: false,
            search: None,
            search_limit: 20,
            fail_under_health: 0,
            fail_on_critical: false,
            max_tool_fail_rate: None,
            baseline: None,
            baseline_max_duration_delta_pct: 0.0,
            baseline_max_cost_delta_pct: 0.0,
            baseline_max_token_delta_pct: 0.0,
            lang: "en".to_string(),
            range: "all".to_string(),
            project: String::new(),
            source: String::new(),
            model_filter: String::new(),
            query: String::new(),
            health: String::new(),
            cost: String::new(),
            anomaly: String::new(),
            sort: "recent".to_string(),
            order: "desc".to_string(),
            limit: 20,
            clear_cache: false,
            preserve_history: false,
            include_history: false,
        }
    }

    fn write_compare_session(path: &std::path::Path, idx: usize) {
        let mut file = fs::File::create(path).expect("create compare session");
        writeln!(
            file,
            r#"{{"role":"user","content":"compare {idx}","timestamp":"2026-05-02T10:00:{idx:02}Z","ModelUsed":"gpt-4.1"}}"#
        )
        .expect("write compare user");
        writeln!(
            file,
            r#"{{"role":"assistant","content":"done","timestamp":"2026-05-02T10:01:{idx:02}Z","ModelUsed":"gpt-4.1"}}"#
        )
        .expect("write compare assistant");
    }
}
