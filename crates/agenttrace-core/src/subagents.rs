//! Links Claude Code subagent transcripts (`<session>/subagents/agent-*.jsonl`)
//! to the session that spawned them.

use crate::{total_tokens, Session};
use std::collections::HashMap;
use std::path::Path;

/// Returns the parent transcript path for a Claude Code subagent transcript.
fn parent_transcript_path(path: &str) -> Option<String> {
    let path = Path::new(path);
    let name = path.file_name()?.to_str()?;
    if !(name.starts_with("agent-") && name.ends_with(".jsonl")) {
        return None;
    }
    let subagents_dir = path.parent()?;
    if subagents_dir.file_name()?.to_str()? != "subagents" {
        return None;
    }
    let session_dir = subagents_dir.parent()?;
    session_dir.file_name()?;
    // Append to the original string rather than `join`, so the separator style
    // matches the parent's own path on every platform.
    Some(format!("{}.jsonl", session_dir.to_string_lossy()))
}

/// Rolls subagent cost and tokens up into their parent session.
///
/// Subagent sessions stay in the list (so totals still count them once) and
/// record their parent; the parent gains `subagent_*` rollups that are kept
/// separate from its own metrics.
pub fn attribute_subagents(sessions: &mut [Session]) {
    let index: HashMap<String, usize> = sessions
        .iter()
        .enumerate()
        .map(|(i, session)| (session.path.clone(), i))
        .collect();
    let links = sessions
        .iter()
        .enumerate()
        .filter_map(|(child, session)| {
            let parent = *index.get(&parent_transcript_path(&session.path)?)?;
            Some((child, parent))
        })
        .collect::<Vec<_>>();
    for session in sessions.iter_mut() {
        session.metrics.subagent_count = 0;
        session.metrics.subagent_cost = 0.0;
        session.metrics.subagent_tokens = 0;
        session.metrics.parent_session.clear();
    }
    for (child, parent) in links {
        let (cost, tokens) = (
            sessions[child].metrics.cost_estimated,
            total_tokens(&sessions[child]),
        );
        sessions[child].metrics.parent_session = sessions[parent].path.clone();
        let metrics = &mut sessions[parent].metrics;
        metrics.subagent_count += 1;
        metrics.subagent_cost += cost;
        metrics.subagent_tokens += tokens;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Metrics;

    fn session(path: &str, cost: f64, input: i64) -> Session {
        Session {
            name: path.to_string(),
            path: path.to_string(),
            cwd: String::new(),
            metrics: Metrics {
                cost_estimated: cost,
                tokens_input: input,
                ..Metrics::default()
            },
            anomalies: Vec::new(),
            health: 100,
            tool_warnings: Vec::new(),
            diagnostics: crate::Diagnostics::default(),
        }
    }

    #[test]
    fn resolves_parent_transcript_path() {
        assert_eq!(
            parent_transcript_path("/p/proj/abc/subagents/agent-a1.jsonl").as_deref(),
            Some("/p/proj/abc.jsonl")
        );
        assert_eq!(parent_transcript_path("/p/proj/abc.jsonl"), None);
        #[cfg(windows)]
        assert_eq!(
            parent_transcript_path(r"C:\p\abc\subagents\agent-a1.jsonl").as_deref(),
            Some(r"C:\p\abc.jsonl")
        );
        assert_eq!(
            parent_transcript_path("/p/proj/abc/other/agent-a1.jsonl"),
            None
        );
        assert_eq!(
            parent_transcript_path("/p/proj/abc/subagents/agent-a1.meta.json"),
            None
        );
    }

    #[test]
    fn rolls_subagents_into_parent_without_changing_own_metrics() {
        let mut sessions = vec![
            session("/p/proj/abc.jsonl", 1.0, 100),
            session("/p/proj/abc/subagents/agent-a1.jsonl", 0.5, 40),
            session("/p/proj/abc/subagents/agent-a2.jsonl", 0.25, 10),
            session("/p/proj/zzz/subagents/agent-orphan.jsonl", 9.0, 900),
        ];
        attribute_subagents(&mut sessions);
        let parent = &sessions[0].metrics;
        assert_eq!(parent.cost_estimated, 1.0);
        assert_eq!(parent.subagent_count, 2);
        assert!((parent.subagent_cost - 0.75).abs() < 1e-9);
        assert_eq!(parent.subagent_tokens, 50);
        assert_eq!(sessions[1].metrics.parent_session, "/p/proj/abc.jsonl");
        assert!(sessions[3].metrics.parent_session.is_empty());

        // Idempotent when re-run on already attributed sessions.
        attribute_subagents(&mut sessions);
        assert_eq!(sessions[0].metrics.subagent_count, 2);

        // Dropping the parent clears stale child links on the next pass.
        let mut orphans = sessions[1..].to_vec();
        attribute_subagents(&mut orphans);
        assert!(orphans[0].metrics.parent_session.is_empty());
    }
}
