//! Calendar (daily/weekly/monthly) and 5-hour block rollups over timestamped
//! usage records.
//!
//! Each usage record is attributed to the bucket its own timestamp falls in, so
//! a session that spans midnight is split across days. Sessions without
//! per-turn usage (text estimates, aggregate SQLite sources) contribute one
//! record at their start time.

use crate::{round4, total_tokens, Session, UsagePoint};
use chrono::{
    DateTime, Datelike, Duration, FixedOffset, Local, NaiveDate, TimeZone, Timelike, Utc,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// Length of a Claude-style rolling usage window.
const BLOCK_SECS: i64 = 5 * 3600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsagePeriod {
    Day,
    Week,
    Month,
}

/// Timezone used to assign usage records to calendar buckets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UsageTz {
    #[default]
    Local,
    Fixed(FixedOffset),
}

impl UsageTz {
    /// Parses `local`, `utc`, or an offset such as `+08:00`, `-0500`, `+8`.
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        match value.to_ascii_lowercase().as_str() {
            "" | "local" => return Some(Self::Local),
            "utc" | "z" => return FixedOffset::east_opt(0).map(Self::Fixed),
            _ => {}
        }
        let (sign, rest) = match value.as_bytes().first()? {
            b'+' => (1, &value[1..]),
            b'-' => (-1, &value[1..]),
            _ => return None,
        };
        let (hours, minutes) = match rest.split_once(':') {
            Some((h, m)) => (h, m),
            None if rest.len() == 4 => rest.split_at(2),
            None => (rest, "0"),
        };
        let hours: i32 = hours.parse().ok()?;
        let minutes: i32 = minutes.parse().ok()?;
        if hours > 14 || minutes > 59 {
            return None;
        }
        FixedOffset::east_opt(sign * (hours * 3600 + minutes * 60)).map(Self::Fixed)
    }

    fn local_date(self, ts: i64) -> Option<NaiveDate> {
        let utc = DateTime::<Utc>::from_timestamp(ts, 0)?;
        Some(match self {
            Self::Local => utc.with_timezone(&Local).date_naive(),
            Self::Fixed(offset) => utc.with_timezone(&offset).date_naive(),
        })
    }

    fn format(self, ts: i64) -> String {
        let Some(utc) = DateTime::<Utc>::from_timestamp(ts, 0) else {
            return String::new();
        };
        match self {
            Self::Local => utc.with_timezone(&Local).to_rfc3339(),
            Self::Fixed(offset) => utc.with_timezone(&offset).to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct UsageBucket {
    /// `YYYY-MM-DD` (day, or the Monday of an ISO week) or `YYYY-MM`.
    pub period: String,
    pub sessions: usize,
    pub tokens: i64,
    pub cost: f64,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct UsageBlock {
    pub start: String,
    pub end: String,
    pub first_activity: String,
    pub last_activity: String,
    pub active: bool,
    pub sessions: usize,
    pub tokens: i64,
    pub cost: f64,
    /// Tokens per minute between the first and last record in the block.
    pub tokens_per_minute: f64,
    /// Estimated USD per hour between the first and last record in the block.
    pub cost_per_hour: f64,
    /// For the active block: cost if the current burn rate holds until `end`.
    pub projected_cost: f64,
    /// For the active block: tokens if the current burn rate holds until `end`.
    pub projected_tokens: i64,
}

fn session_points(session: &Session) -> Vec<UsagePoint> {
    if !session.metrics.usage_points.is_empty() {
        return session.metrics.usage_points.clone();
    }
    let Ok(start) = DateTime::parse_from_rfc3339(&session.metrics.session_start) else {
        return Vec::new();
    };
    vec![UsagePoint {
        ts: start.timestamp(),
        tokens: total_tokens(session),
        cost: session.metrics.cost_estimated,
    }]
}

fn bucket_key(date: NaiveDate, period: UsagePeriod) -> String {
    match period {
        UsagePeriod::Day => date.format("%Y-%m-%d").to_string(),
        UsagePeriod::Week => {
            let monday = date - Duration::days(date.weekday().num_days_from_monday() as i64);
            monday.format("%Y-%m-%d").to_string()
        }
        UsagePeriod::Month => date.format("%Y-%m").to_string(),
    }
}

/// Groups usage into calendar buckets, newest first.
pub fn usage_by_period(sessions: &[Session], period: UsagePeriod, tz: UsageTz) -> Vec<UsageBucket> {
    let mut buckets: BTreeMap<String, (BTreeSet<usize>, i64, f64)> = BTreeMap::new();
    for (index, session) in sessions.iter().enumerate() {
        for point in session_points(session) {
            let Some(date) = tz.local_date(point.ts) else {
                continue;
            };
            let entry = buckets.entry(bucket_key(date, period)).or_default();
            entry.0.insert(index);
            entry.1 += point.tokens;
            entry.2 += point.cost;
        }
    }
    buckets
        .into_iter()
        .rev()
        .map(|(period, (sessions, tokens, cost))| UsageBucket {
            period,
            sessions: sessions.len(),
            tokens,
            cost: round4(cost),
        })
        .collect()
}

/// Splits usage into 5-hour blocks, newest first.
///
/// A block starts at the top of the hour of its first record and lasts five
/// hours; the next record at or after the block end (or after a gap of five
/// hours) opens a new block. This mirrors how Claude subscription windows are
/// commonly approximated from local logs; it is an estimate, not the
/// provider's server-side accounting.
pub fn usage_blocks(sessions: &[Session], now: DateTime<Utc>, tz: UsageTz) -> Vec<UsageBlock> {
    let mut points = sessions
        .iter()
        .enumerate()
        .flat_map(|(index, session)| {
            session_points(session)
                .into_iter()
                .map(move |point| (point, index))
        })
        .collect::<Vec<_>>();
    points.sort_by_key(|(point, _)| point.ts);

    struct Acc {
        start: i64,
        first: i64,
        last: i64,
        sessions: BTreeSet<usize>,
        tokens: i64,
        cost: f64,
    }
    let mut blocks: Vec<Acc> = Vec::new();
    for (point, index) in points {
        let open_new = blocks.last().map_or(true, |block| {
            point.ts >= block.start + BLOCK_SECS || point.ts - block.last >= BLOCK_SECS
        });
        if open_new {
            let start = DateTime::<Utc>::from_timestamp(point.ts, 0)
                .and_then(|ts| ts.with_minute(0)?.with_second(0))
                .map_or(point.ts, |ts| ts.timestamp());
            blocks.push(Acc {
                start,
                first: point.ts,
                last: point.ts,
                sessions: BTreeSet::new(),
                tokens: 0,
                cost: 0.0,
            });
        }
        let block = blocks.last_mut().expect("block opened above");
        block.last = point.ts;
        block.sessions.insert(index);
        block.tokens += point.tokens;
        block.cost += point.cost;
    }

    let now = now.timestamp();
    blocks
        .into_iter()
        .rev()
        .map(|block| {
            let end = block.start + BLOCK_SECS;
            let active = now >= block.start && now < end;
            let minutes = ((block.last - block.first) as f64 / 60.0).max(1.0);
            let tokens_per_minute = block.tokens as f64 / minutes;
            let cost_per_minute = block.cost / minutes;
            let remaining = if active {
                (end - now) as f64 / 60.0
            } else {
                0.0
            };
            UsageBlock {
                start: tz.format(block.start),
                end: tz.format(end),
                first_activity: tz.format(block.first),
                last_activity: tz.format(block.last),
                active,
                sessions: block.sessions.len(),
                tokens: block.tokens,
                cost: round4(block.cost),
                tokens_per_minute: (tokens_per_minute * 10.0).round() / 10.0,
                cost_per_hour: round4(cost_per_minute * 60.0),
                projected_cost: round4(block.cost + cost_per_minute * remaining),
                projected_tokens: block.tokens + (tokens_per_minute * remaining) as i64,
            }
        })
        .collect()
}

/// Returns the timezone offset label used in report headers.
pub fn usage_tz_label(tz: UsageTz, now: DateTime<Utc>) -> String {
    match tz {
        UsageTz::Local => Local
            .from_utc_datetime(&now.naive_utc())
            .format("%:z")
            .to_string(),
        UsageTz::Fixed(offset) => offset.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Metrics;

    fn ts(value: &str) -> i64 {
        DateTime::parse_from_rfc3339(value).unwrap().timestamp()
    }

    fn session(points: &[(&str, i64, f64)]) -> Session {
        Session {
            name: "s".to_string(),
            path: "s".to_string(),
            cwd: String::new(),
            metrics: Metrics {
                usage_points: points
                    .iter()
                    .map(|(at, tokens, cost)| UsagePoint {
                        ts: ts(at),
                        tokens: *tokens,
                        cost: *cost,
                    })
                    .collect(),
                ..Metrics::default()
            },
            anomalies: Vec::new(),
            health: 100,
            tool_warnings: Vec::new(),
            diagnostics: crate::Diagnostics::default(),
        }
    }

    #[test]
    fn parses_timezones() {
        assert_eq!(UsageTz::parse("local"), Some(UsageTz::Local));
        let utc = FixedOffset::east_opt(0).unwrap();
        assert_eq!(UsageTz::parse("UTC"), Some(UsageTz::Fixed(utc)));
        let cst = FixedOffset::east_opt(8 * 3600).unwrap();
        for value in ["+08:00", "+0800", "+8"] {
            assert_eq!(UsageTz::parse(value), Some(UsageTz::Fixed(cst)), "{value}");
        }
        let est = FixedOffset::west_opt(5 * 3600 + 1800).unwrap();
        assert_eq!(UsageTz::parse("-05:30"), Some(UsageTz::Fixed(est)));
        assert_eq!(UsageTz::parse("Asia/Shanghai"), None);
        assert_eq!(UsageTz::parse("+25"), None);
    }

    #[test]
    fn splits_a_session_across_midnight_in_the_requested_timezone() {
        let sessions = [session(&[
            ("2026-10-01T15:30:00Z", 100, 1.0),
            ("2026-10-01T16:30:00Z", 50, 0.5),
        ])];
        let utc = usage_by_period(&sessions, UsagePeriod::Day, UsageTz::parse("utc").unwrap());
        assert_eq!(utc.len(), 1);
        assert_eq!(utc[0].tokens, 150);

        // 16:30Z is 00:30 the next day in +08:00.
        let cst = usage_by_period(
            &sessions,
            UsagePeriod::Day,
            UsageTz::parse("+08:00").unwrap(),
        );
        assert_eq!(
            cst.iter()
                .map(|b| (b.period.as_str(), b.tokens))
                .collect::<Vec<_>>(),
            [("2026-10-02", 50), ("2026-10-01", 100)]
        );
        assert!(cst.iter().all(|bucket| bucket.sessions == 1));
    }

    #[test]
    fn groups_weeks_by_monday_and_months() {
        let sessions = [
            session(&[("2026-09-28T10:00:00Z", 1, 0.0)]), // Monday
            session(&[("2026-10-04T10:00:00Z", 2, 0.0)]), // Sunday, same ISO week
            session(&[("2026-10-05T10:00:00Z", 4, 0.0)]), // next Monday
        ];
        let utc = UsageTz::parse("utc").unwrap();
        let weeks = usage_by_period(&sessions, UsagePeriod::Week, utc);
        assert_eq!(
            weeks
                .iter()
                .map(|b| (b.period.as_str(), b.tokens, b.sessions))
                .collect::<Vec<_>>(),
            [("2026-10-05", 4, 1), ("2026-09-28", 3, 2)]
        );
        let months = usage_by_period(&sessions, UsagePeriod::Month, utc);
        assert_eq!(
            months
                .iter()
                .map(|b| (b.period.as_str(), b.tokens))
                .collect::<Vec<_>>(),
            [("2026-10", 6), ("2026-09", 1)]
        );
    }

    #[test]
    fn builds_five_hour_blocks_and_projects_the_active_one() {
        let sessions = [
            session(&[
                ("2026-10-04T08:10:00Z", 1000, 1.0),
                ("2026-10-04T09:10:00Z", 1000, 1.0),
            ]),
            session(&[
                ("2026-10-04T13:05:00Z", 600, 0.6), // past 08:00 + 5h -> new block
                ("2026-10-04T13:35:00Z", 600, 0.6),
            ]),
        ];
        let utc = UsageTz::parse("utc").unwrap();
        let now = DateTime::parse_from_rfc3339("2026-10-04T14:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let blocks = usage_blocks(&sessions, now, utc);
        assert_eq!(blocks.len(), 2);

        let active = &blocks[0];
        assert!(active.active);
        assert_eq!(active.start, "2026-10-04T13:00:00+00:00");
        assert_eq!(active.end, "2026-10-04T18:00:00+00:00");
        assert_eq!(active.tokens, 1200);
        assert_eq!(active.tokens_per_minute, 40.0); // 1200 tokens over 30 minutes
        assert_eq!(active.cost_per_hour, 2.4);
        // 4h remaining at $0.04/min.
        assert_eq!(active.projected_cost, 10.8);
        assert_eq!(active.projected_tokens, 1200 + 40 * 240);

        let done = &blocks[1];
        assert!(!done.active);
        assert_eq!(done.start, "2026-10-04T08:00:00+00:00");
        assert_eq!(done.tokens, 2000);
        assert_eq!(done.projected_cost, done.cost);
    }

    #[test]
    fn falls_back_to_session_start_without_usage_points() {
        let mut aggregate = session(&[]);
        aggregate.metrics.session_start = "2026-10-03T23:00:00Z".to_string();
        aggregate.metrics.tokens_input = 70;
        aggregate.metrics.cost_estimated = 0.7;
        let days = usage_by_period(
            &[aggregate],
            UsagePeriod::Day,
            UsageTz::parse("utc").unwrap(),
        );
        assert_eq!(days.len(), 1);
        assert_eq!(
            (days[0].period.as_str(), days[0].tokens, days[0].cost),
            ("2026-10-03", 70, 0.7)
        );
    }
}
