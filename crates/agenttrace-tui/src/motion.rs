//! Frame-paced motion for the explorer: transitions, count-ups and ambient
//! loading effects. Effects are continuous functions of time (sweeping edges,
//! eased offsets) so each extra frame shows a distinct state up to 120 FPS.
//! Every effect only reuses the existing palette and is applied to the
//! rendered buffer, so disabling motion yields the exact static frame.

use super::*;
use ratatui::buffer::Buffer;
use std::cell::{Cell, RefCell};
use std::hash::{Hash, Hasher};

pub(super) const MAX_FPS: u32 = 120;
const AMBIENT_INTERVAL: Duration = Duration::from_millis(33);
const STARTUP_DURATION: Duration = Duration::from_millis(260);
const VIEW_DURATION: Duration = Duration::from_millis(240);
const REVEAL_DURATION: Duration = Duration::from_millis(140);
const OVERLAY_DURATION: Duration = Duration::from_millis(170);
const SELECTION_DURATION: Duration = Duration::from_millis(140);
// Held arrow keys repeat faster than this; animating each step would only lag behind.
const RAPID_SELECTION: Duration = Duration::from_millis(110);
const NOTICE_IN_DURATION: Duration = Duration::from_millis(220);
const NOTICE_OUT_DURATION: Duration = Duration::from_millis(400);
const SPARK_DURATION: Duration = Duration::from_millis(420);
const SPARK_STAGGER: Duration = Duration::from_millis(14);
pub(super) const COUNTER_DURATION: Duration = Duration::from_millis(450);
pub(super) const PREVIEW_COUNTER_DURATION: Duration = Duration::from_millis(280);
const SHIMMER_PERIOD: Duration = Duration::from_millis(1400);
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Frame-rate tiers the pacer moves between; each divides 120 evenly so
/// frames stay on a regular cadence after a switch.
const FPS_TIERS: [u32; 3] = [120, 60, 30];
/// Frames sampled before the pacer decides to change tier.
const PACER_WINDOW: usize = 8;
/// Step down when the median frame cost uses more than this share of the budget.
const PACER_DOWN_LOAD: f64 = 0.8;
/// Step up only when the median cost fits this share of the faster tier's budget.
const PACER_UP_LOAD: f64 = 0.35;
/// Minimum time after a change before stepping down again.
const PACER_COOLDOWN: Duration = Duration::from_millis(1000);
/// Stepping back up waits longer: terminal buffers can hide backpressure for a
/// moment, so a quick recovery would just bounce between tiers.
const PACER_UP_COOLDOWN: Duration = Duration::from_millis(4000);

/// Adapts the frame rate to what this machine and terminal can sustain.
///
/// A terminal app can't see the display refresh rate (and over SSH there is no
/// local display at all), so the pacer measures what actually matters: how long
/// each frame takes to render, diff and write to the terminal. Writes block when
/// the terminal can't parse output fast enough, so a slow terminal shows up in
/// the same number as a slow render.
#[derive(Debug, Clone)]
pub(super) struct Pacer {
    cap: u32,
    tier: usize,
    samples: std::collections::VecDeque<Duration>,
    last_change: Option<Instant>,
}

impl Pacer {
    pub(super) fn new(cap: u32) -> Self {
        let cap = cap.clamp(1, MAX_FPS);
        Self {
            cap,
            tier: 0,
            samples: std::collections::VecDeque::with_capacity(PACER_WINDOW),
            last_change: None,
        }
    }

    /// Current target, never above the user's cap.
    pub(super) fn fps(&self) -> u32 {
        FPS_TIERS[self.tier].min(self.cap)
    }

    fn budget(fps: u32) -> Duration {
        Duration::from_secs_f64(1.0 / f64::from(fps))
    }

    /// Records one frame's render+write cost; returns true if the tier changed.
    pub(super) fn record(&mut self, cost: Duration, now: Instant) -> bool {
        if self.samples.len() == PACER_WINDOW {
            self.samples.pop_front();
        }
        self.samples.push_back(cost);
        let since_change = self
            .last_change
            .map(|last| now.saturating_duration_since(last));
        if self.samples.len() < PACER_WINDOW
            || since_change.is_some_and(|elapsed| elapsed < PACER_COOLDOWN)
        {
            return false;
        }
        // Median, so a single full repaint (resize, first frame) doesn't trigger a drop.
        let mut sorted = self.samples.iter().copied().collect::<Vec<_>>();
        sorted.sort();
        let median = sorted[sorted.len() / 2].as_secs_f64();
        let current = Self::budget(self.fps()).as_secs_f64();
        let slower = self.tier + 1 < FPS_TIERS.len() && FPS_TIERS[self.tier + 1] < self.fps();
        if slower && median > current * PACER_DOWN_LOAD {
            self.tier += 1;
        } else if self.tier > 0
            && !since_change.is_some_and(|elapsed| elapsed < PACER_UP_COOLDOWN)
            && FPS_TIERS[self.tier] < self.cap
            && median
                < Self::budget(FPS_TIERS[self.tier - 1].min(self.cap)).as_secs_f64() * PACER_UP_LOAD
        {
            self.tier -= 1;
        } else {
            return false;
        }
        self.samples.clear();
        self.last_change = Some(now);
        true
    }
}

/// What the user currently sees; changes between frames start transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Scene {
    pub(super) view: ExplorerView,
    pub(super) detail: Option<DetailSection>,
    pub(super) compare: bool,
    pub(super) expanded: bool,
    pub(super) loading: bool,
    pub(super) overlay: ExplorerOverlay,
    pub(super) selected: usize,
    pub(super) generation: u64,
    pub(super) notice: Option<u64>,
    pub(super) notice_expiry: Option<Instant>,
}

impl Scene {
    fn depth(&self) -> u8 {
        u8::from(self.detail.is_some() || self.compare) + u8::from(self.expanded)
    }

    fn screen(&self) -> (ExplorerView, Option<DetailSection>, bool, bool) {
        (self.view, self.detail, self.compare, self.expanded)
    }
}

pub(super) fn notice_key(message: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    message.hash(&mut hasher);
    hasher.finish()
}

#[derive(Debug, Clone, Copy)]
struct Tween {
    start: Instant,
    duration: Duration,
}

impl Tween {
    fn new(start: Instant, duration: Duration) -> Self {
        Self { start, duration }
    }

    fn progress(&self, now: Instant) -> f64 {
        if self.duration.is_zero() {
            return 1.0;
        }
        (now.saturating_duration_since(self.start).as_secs_f64() / self.duration.as_secs_f64())
            .clamp(0.0, 1.0)
    }

    fn active(&self, now: Instant) -> bool {
        self.progress(now) < 1.0
    }
}

#[derive(Debug, Clone, Copy)]
struct Counter {
    from: f64,
    to: f64,
    tween: Tween,
}

#[derive(Debug, Clone, Copy)]
struct ViewTransition {
    tween: Tween,
    direction: i8,
    offset: u16,
}

pub(super) struct Motion {
    enabled: bool,
    pacer: Pacer,
    show_fps: bool,
    presented: std::collections::VecDeque<Instant>,
    last_cost: Duration,
    epoch: Instant,
    now: Instant,
    scene: Option<Scene>,
    view: Option<ViewTransition>,
    reveal: Option<Tween>,
    overlay: Option<Tween>,
    selection: Option<Tween>,
    last_selection_change: Option<Instant>,
    notice: Option<Tween>,
    notice_expiry: Option<Instant>,
    spark: Option<Tween>,
    ambient: bool,
    counters: RefCell<BTreeMap<&'static str, Counter>>,
    pub(super) selected_row: Cell<Option<Rect>>,
    pub(super) notice_row: Cell<Option<Rect>>,
    pub(super) trend_cache: RefCell<Option<TrendCache>>,
}

#[derive(Debug, Clone)]
pub(super) struct TrendCache {
    pub(super) generation: u64,
    pub(super) day: chrono::NaiveDate,
    pub(super) buckets: Vec<f64>,
}

impl Motion {
    pub(super) fn new(fps: Option<u32>) -> Self {
        let now = Instant::now();
        let fps = fps.map(|fps| fps.clamp(1, MAX_FPS));
        Self {
            enabled: fps.is_some(),
            pacer: Pacer::new(fps.unwrap_or(MAX_FPS)),
            show_fps: false,
            presented: std::collections::VecDeque::new(),
            last_cost: Duration::ZERO,
            epoch: now,
            now,
            scene: None,
            view: None,
            reveal: None,
            overlay: None,
            selection: None,
            last_selection_change: None,
            notice: None,
            notice_expiry: None,
            spark: None,
            ambient: false,
            counters: RefCell::new(BTreeMap::new()),
            selected_row: Cell::new(None),
            notice_row: Cell::new(None),
            trend_cache: RefCell::new(None),
        }
    }

    pub(super) fn disabled() -> Self {
        Self::new(None)
    }

    /// `AGENTTRACE_FPS` caps the frame rate (1-120, default 120; `0`/`off`
    /// disables motion); below the cap the pacer adapts automatically.
    /// `AGENTTRACE_REDUCED_MOTION=1` also disables motion, and
    /// `AGENTTRACE_SHOW_FPS=1` prints the live frame rate in the top-right corner.
    #[cfg(not(test))]
    pub(super) fn from_env() -> Self {
        if env_flag("AGENTTRACE_REDUCED_MOTION") {
            return Self::disabled();
        }
        let mut motion = Self::new(parse_fps(std::env::var("AGENTTRACE_FPS").ok().as_deref()));
        motion.show_fps = env_flag("AGENTTRACE_SHOW_FPS");
        motion
    }

    #[cfg(test)]
    pub(super) fn from_env() -> Self {
        Self::disabled()
    }

    pub(super) fn enabled(&self) -> bool {
        self.enabled
    }

    pub(super) fn frame_interval(&self) -> Duration {
        Pacer::budget(self.pacer.fps())
    }

    pub(super) fn target_fps(&self) -> u32 {
        self.pacer.fps()
    }

    /// Feeds the pacer with how long the last frame took to render and write.
    pub(super) fn record_frame(&mut self, started: Instant, cost: Duration) {
        self.last_cost = cost;
        if !self.enabled {
            return;
        }
        self.pacer.record(cost, started);
        self.presented.push_back(started);
        while self
            .presented
            .front()
            .is_some_and(|first| started.saturating_duration_since(*first) > Duration::from_secs(1))
        {
            self.presented.pop_front();
        }
    }

    /// Frames actually presented during the last second, while animating.
    pub(super) fn measured_fps(&self) -> usize {
        let Some(last) = self.presented.back() else {
            return 0;
        };
        if self.now.saturating_duration_since(*last) > Duration::from_millis(250) {
            return 0;
        }
        self.presented.len()
    }

    /// Optional corner readout: presented fps / target fps · last frame cost.
    pub(super) fn render_fps(&self, buf: &mut Buffer, area: Rect) {
        if !self.show_fps || area.width < 24 {
            return;
        }
        let label = format!(
            " {}/{} fps · {:.1}ms ",
            self.measured_fps(),
            self.target_fps(),
            self.last_cost.as_secs_f64() * 1000.0
        );
        let width = unicode_width::UnicodeWidthStr::width(label.as_str()) as u16;
        let x = area.right().saturating_sub(width);
        buf.set_string(x, area.top(), label, Style::default().fg(Color::DarkGray));
    }

    /// Diffs the new scene against the previous frame and starts transitions.
    pub(super) fn begin_frame(&mut self, scene: Scene, ambient: bool, now: Instant) {
        self.now = now;
        self.ambient = ambient;
        self.selected_row.set(None);
        self.notice_row.set(None);
        if !self.enabled {
            self.scene = Some(scene);
            return;
        }
        let Some(previous) = self.scene.replace(scene) else {
            self.reveal = Some(Tween::new(now, STARTUP_DURATION));
            self.spark = Some(Tween::new(now, SPARK_DURATION));
            self.notice_expiry = scene.notice_expiry;
            return;
        };
        if previous.screen() != scene.screen() || previous.loading != scene.loading {
            let (direction, offset) = match scene.depth().cmp(&previous.depth()) {
                Ordering::Greater => (1, 16),
                Ordering::Less => (-1, 16),
                Ordering::Equal if previous.loading != scene.loading => (0, 0),
                Ordering::Equal => (1, 8),
            };
            self.view = Some(ViewTransition {
                tween: Tween::new(now, VIEW_DURATION),
                direction,
                offset,
            });
            self.selection = None;
            self.spark = Some(Tween::new(now, SPARK_DURATION));
        } else if previous.generation != scene.generation {
            self.spark = Some(Tween::new(now, SPARK_DURATION));
        }
        if previous.overlay != scene.overlay {
            if scene.overlay == ExplorerOverlay::None {
                self.overlay = None;
                self.reveal = Some(Tween::new(now, REVEAL_DURATION));
            } else {
                self.overlay = Some(Tween::new(now, OVERLAY_DURATION));
            }
        }
        if previous.selected != scene.selected
            && previous.screen() == scene.screen()
            && scene.overlay == ExplorerOverlay::None
        {
            let rapid = self
                .last_selection_change
                .is_some_and(|last| now.saturating_duration_since(last) < RAPID_SELECTION);
            self.selection = (!rapid).then(|| Tween::new(now, SELECTION_DURATION));
            self.last_selection_change = Some(now);
        }
        if scene.notice.is_some() && previous.notice != scene.notice {
            self.notice = Some(Tween::new(now, NOTICE_IN_DURATION));
        }
        self.notice_expiry = scene.notice_expiry;
    }

    fn transitioning(&self, now: Instant) -> bool {
        let tweens = [
            self.reveal,
            self.overlay,
            self.selection,
            self.notice,
            self.spark,
        ];
        tweens.iter().flatten().any(|tween| tween.active(now))
            || self.view.is_some_and(|view| view.tween.active(now))
            || self
                .counters
                .borrow()
                .values()
                .any(|counter| counter.tween.active(now))
    }

    /// Delay until the next frame should be drawn without new input.
    pub(super) fn next_frame_delay(&self, now: Instant) -> Option<Duration> {
        if !self.enabled {
            return None;
        }
        if self.transitioning(now) {
            return Some(self.frame_interval());
        }
        let notice = self.notice_expiry.and_then(|expiry| {
            let fade_start = expiry.checked_sub(NOTICE_OUT_DURATION)?;
            (expiry > now).then(|| {
                fade_start
                    .saturating_duration_since(now)
                    .max(self.frame_interval())
            })
        });
        let ambient = self
            .ambient
            .then_some(AMBIENT_INTERVAL.max(self.frame_interval()));
        match (notice, ambient) {
            (Some(left), Some(right)) => Some(left.min(right)),
            (left, right) => left.or(right),
        }
    }

    /// Eased value that rolls from the previously shown value to `target`.
    pub(super) fn counter(&self, key: &'static str, target: f64, duration: Duration) -> f64 {
        if !self.enabled || !target.is_finite() {
            return target;
        }
        let now = self.now;
        let mut counters = self.counters.borrow_mut();
        let counter = counters.entry(key).or_insert(Counter {
            from: 0.0,
            to: target,
            tween: Tween::new(now, duration),
        });
        if (counter.to - target).abs() > f64::EPSILON {
            let shown = interpolate(counter, now);
            *counter = Counter {
                from: shown,
                to: target,
                tween: Tween::new(now, duration),
            };
        }
        interpolate(counter, now)
    }

    pub(super) fn spinner(&self) -> &'static str {
        let elapsed = self.now.saturating_duration_since(self.epoch).as_millis();
        SPINNER[(elapsed / 80) as usize % SPINNER.len()]
    }

    /// Column (relative to the bar start) of the moving highlight band.
    pub(super) fn shimmer(&self, width: u16) -> Option<u16> {
        if !self.enabled || width < 4 {
            return None;
        }
        let elapsed = self.now.saturating_duration_since(self.epoch).as_secs_f64();
        let phase = (elapsed / SHIMMER_PERIOD.as_secs_f64()).fract();
        Some((phase * f64::from(width + 8)) as u16)
    }

    /// Growth factor (0..=1) for sparkline bar `index`, staggered left to right.
    pub(super) fn spark_growth(&self, index: usize) -> f64 {
        let Some(tween) = self.spark.filter(|_| self.enabled) else {
            return 1.0;
        };
        let delay = SPARK_STAGGER * index as u32;
        let shifted = Tween::new(tween.start + delay, tween.duration);
        ease_out_cubic(shifted.progress(self.now))
    }

    pub(super) fn overlay_rect(&self, rect: Rect) -> Rect {
        let Some(tween) = self.overlay.filter(|_| self.enabled) else {
            return rect;
        };
        let eased = ease_out_back(tween.progress(self.now));
        let width = scaled(rect.width, 0.6, eased).min(rect.width);
        let height = scaled(rect.height, 0.3, eased).clamp(3.min(rect.height), rect.height);
        Rect::new(
            rect.x + (rect.width - width) / 2,
            rect.y + (rect.height - height) / 2,
            width,
            height,
        )
    }

    /// True while the overlay backdrop is still easing in.
    pub(super) fn backdrop_partial(&self) -> bool {
        self.overlay
            .filter(|_| self.enabled)
            .is_some_and(|tween| tween.progress(self.now) < 0.35)
    }

    pub(super) fn apply_overlay(&self, buf: &mut Buffer, area: Rect) {
        if let Some(tween) = self.overlay.filter(|_| self.enabled) {
            sweep_rows(buf, area, ease_out_cubic(tween.progress(self.now)));
        }
    }

    /// Fade/slide the main content after a screen change.
    pub(super) fn apply_content(&self, buf: &mut Buffer, area: Rect) {
        if !self.enabled {
            return;
        }
        if let Some(view) = self.view {
            let progress = view.tween.progress(self.now);
            if progress < 1.0 {
                let eased = ease_out_cubic(progress);
                let travel = view.offset.min(area.width / 5);
                let offset = (f64::from(travel) * (1.0 - eased)).round() as u16;
                shift_columns(buf, area, view.direction, offset);
                sweep_columns(buf, area, view.direction, eased);
            }
        }
        if let Some(reveal) = self.selection {
            let progress = reveal.progress(self.now);
            if let Some(row) = self.selected_row.get().filter(|_| progress < 1.0) {
                wipe_selection(buf, row, ease_out_cubic(progress));
            }
        }
    }

    /// Fade the whole frame after startup or an overlay closes.
    pub(super) fn apply_reveal(&self, buf: &mut Buffer, area: Rect) {
        if let Some(tween) = self.reveal.filter(|_| self.enabled) {
            sweep_rows(buf, area, ease_out_cubic(tween.progress(self.now)));
        }
    }

    pub(super) fn apply_notice(&self, buf: &mut Buffer) {
        if !self.enabled {
            return;
        }
        let Some(row) = self.notice_row.get() else {
            return;
        };
        if let Some(tween) = self.notice {
            let progress = tween.progress(self.now);
            if progress < 1.0 {
                type_in(buf, row, ease_out_cubic(progress));
            }
        }
        if let Some(expiry) = self.notice_expiry {
            let remaining = expiry.saturating_duration_since(self.now);
            if remaining < NOTICE_OUT_DURATION {
                let left = remaining.as_secs_f64() / NOTICE_OUT_DURATION.as_secs_f64();
                type_in(buf, row, ease_out_cubic(left));
            }
        }
    }
}

#[cfg(not(test))]
fn env_flag(name: &str) -> bool {
    std::env::var(name).is_ok_and(|value| !matches!(value.trim(), "" | "0" | "false" | "off"))
}

pub(super) fn parse_fps(value: Option<&str>) -> Option<u32> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Some(MAX_FPS);
    };
    if matches!(
        value.to_ascii_lowercase().as_str(),
        "0" | "off" | "false" | "none"
    ) {
        return None;
    }
    Some(
        value
            .parse::<u32>()
            .map_or(MAX_FPS, |fps| fps.clamp(1, MAX_FPS)),
    )
}

fn interpolate(counter: &Counter, now: Instant) -> f64 {
    let eased = ease_out_cubic(counter.tween.progress(now));
    counter.from + (counter.to - counter.from) * eased
}

fn scaled(value: u16, floor: f64, eased: f64) -> u16 {
    (f64::from(value) * (floor + (1.0 - floor) * eased)).round() as u16
}

pub(super) fn ease_out_cubic(t: f64) -> f64 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

fn ease_out_back(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    let c1 = 1.10158;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

/// Width (cells) of the soft edge that leads a sweep.
const SWEEP_BAND: f64 = 6.0;

/// Palette-only per-cell dimming: 0 = untouched, 1 = DIM, 2 = DarkGray.
fn dim_cell(cell: &mut ratatui::buffer::Cell, level: u8) {
    match level {
        0 => {}
        1 => cell.modifier.insert(Modifier::DIM),
        _ => {
            if cell.bg == Color::Reset {
                cell.fg = Color::DarkGray;
            } else {
                cell.bg = Color::DarkGray;
            }
            cell.modifier.remove(Modifier::BOLD);
        }
    }
}

/// Level for a cell `distance` cells past the sweep edge.
fn sweep_level(distance: f64) -> u8 {
    if distance <= 0.0 {
        0
    } else if distance <= SWEEP_BAND {
        1
    } else {
        2
    }
}

/// A soft edge travels across the columns; every frame moves it by sub-cell
/// steps, so the transition stays smooth at high frame rates.
fn sweep_columns(buf: &mut Buffer, area: Rect, direction: i8, eased: f64) {
    let area = area.intersection(buf.area);
    let span = f64::from(area.width) + SWEEP_BAND * 2.0;
    let edge = eased * span - SWEEP_BAND;
    for x in area.left()..area.right() {
        let column = f64::from(x - area.left());
        let column = if direction < 0 {
            f64::from(area.width) - 1.0 - column
        } else {
            column
        };
        let level = sweep_level(column - edge);
        if level == 0 {
            continue;
        }
        for y in area.top()..area.bottom() {
            dim_cell(&mut buf[(x, y)], level);
        }
    }
}

/// Same as [`sweep_columns`] but top to bottom; rows are ~2x taller than
/// columns are wide, so the band is narrower.
fn sweep_rows(buf: &mut Buffer, area: Rect, eased: f64) {
    let area = area.intersection(buf.area);
    let band = SWEEP_BAND / 2.0;
    let span = f64::from(area.height) + band * 2.0;
    let edge = eased * span - band;
    for y in area.top()..area.bottom() {
        let distance = f64::from(y - area.top()) - edge;
        let level = if distance <= 0.0 {
            0
        } else if distance <= band {
            1
        } else {
            2
        };
        if level == 0 {
            continue;
        }
        for x in area.left()..area.right() {
            dim_cell(&mut buf[(x, y)], level);
        }
    }
}

/// Shift each row horizontally; `direction > 0` means the content enters from the right.
fn shift_columns(buf: &mut Buffer, area: Rect, direction: i8, offset: u16) {
    let area = area.intersection(buf.area);
    let offset = offset.min(area.width);
    if offset == 0 || direction == 0 {
        return;
    }
    for y in area.top()..area.bottom() {
        if direction > 0 {
            for x in (area.left() + offset..area.right()).rev() {
                buf[(x, y)] = buf[(x - offset, y)].clone();
            }
            for x in area.left()..area.left() + offset {
                buf[(x, y)].reset();
            }
            // A wide glyph cut by the right edge would leave half a character.
            let last = area.right() - 1;
            if unicode_width::UnicodeWidthStr::width(buf[(last, y)].symbol()) > 1 {
                buf[(last, y)].set_symbol(" ");
            }
        } else {
            for x in area.left()..area.right() - offset {
                buf[(x, y)] = buf[(x + offset, y)].clone();
            }
            for x in area.right() - offset..area.right() {
                buf[(x, y)].reset();
            }
            if buf[(area.left(), y)].symbol().is_empty() {
                buf[(area.left(), y)].set_symbol(" ");
            }
        }
    }
}

/// The selection bar sweeps from the inactive color to the active one.
fn wipe_selection(buf: &mut Buffer, row: Rect, eased: f64) {
    let row = row.intersection(buf.area);
    let edge = row.left() + scaled(row.width, 0.0, eased);
    for x in edge..row.right() {
        let cell = &mut buf[(x, row.top())];
        if cell.bg == Color::Cyan {
            cell.bg = Color::DarkGray;
        }
    }
}

/// Reveal the text in `row` left to right with a bold leading edge.
fn type_in(buf: &mut Buffer, row: Rect, eased: f64) {
    let row = row.intersection(buf.area);
    let Some(end) = (row.left()..row.right())
        .rev()
        .find(|x| !buf[(*x, row.top())].symbol().trim().is_empty())
    else {
        return;
    };
    let span = end + 1 - row.left();
    let edge = row.left() + scaled(span, 0.0, eased);
    for x in edge..=end {
        buf[(x, row.top())].set_symbol(" ");
    }
    if edge > row.left() {
        buf[(edge - 1, row.top())].modifier.insert(Modifier::BOLD);
    }
}

#[cfg(test)]
mod motion_tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn scene() -> Scene {
        Scene {
            view: ExplorerView::Attention,
            detail: None,
            compare: false,
            expanded: false,
            loading: false,
            overlay: ExplorerOverlay::None,
            selected: 0,
            generation: 0,
            notice: None,
            notice_expiry: None,
        }
    }

    #[test]
    fn fps_is_capped_at_120_and_can_be_disabled() {
        assert_eq!(parse_fps(None), Some(120));
        assert_eq!(parse_fps(Some("240")), Some(120));
        assert_eq!(parse_fps(Some("60")), Some(60));
        assert_eq!(parse_fps(Some("off")), None);
        assert_eq!(parse_fps(Some("0")), None);
        let motion = Motion::new(Some(500));
        assert!(motion.frame_interval() >= Duration::from_secs_f64(1.0 / 120.0));
    }

    #[test]
    fn pacer_steps_down_when_frames_miss_the_budget() {
        let mut pacer = Pacer::new(120);
        let start = Instant::now();
        for frame in 0..PACER_WINDOW {
            pacer.record(
                Duration::from_millis(9),
                start + Duration::from_millis(frame as u64),
            );
        }
        assert_eq!(pacer.fps(), 60);
        let later = start + PACER_COOLDOWN * 2;
        for frame in 0..PACER_WINDOW {
            pacer.record(
                Duration::from_millis(15),
                later + Duration::from_millis(frame as u64),
            );
        }
        assert_eq!(pacer.fps(), 30);
        let floor = later + PACER_COOLDOWN * 2;
        for frame in 0..PACER_WINDOW * 2 {
            pacer.record(
                Duration::from_millis(50),
                floor + Duration::from_millis(frame as u64),
            );
        }
        assert_eq!(pacer.fps(), 30, "30 is the floor");
    }

    #[test]
    fn pacer_recovers_after_cooldown_when_frames_are_cheap() {
        let mut pacer = Pacer::new(120);
        let start = Instant::now();
        for _ in 0..PACER_WINDOW {
            pacer.record(Duration::from_millis(12), start);
        }
        assert_eq!(pacer.fps(), 60);
        for _ in 0..PACER_WINDOW {
            pacer.record(
                Duration::from_micros(500),
                start + Duration::from_millis(200),
            );
        }
        assert_eq!(pacer.fps(), 60, "cooldown blocks an immediate bounce");
        for _ in 0..PACER_WINDOW {
            pacer.record(Duration::from_micros(500), start + PACER_COOLDOWN * 2);
        }
        assert_eq!(pacer.fps(), 60, "recovery waits longer than a drop");
        for _ in 0..PACER_WINDOW {
            pacer.record(Duration::from_micros(500), start + PACER_UP_COOLDOWN * 2);
        }
        assert_eq!(pacer.fps(), 120);
    }

    #[test]
    fn pacer_ignores_a_single_slow_frame_and_respects_the_cap() {
        let mut pacer = Pacer::new(120);
        let start = Instant::now();
        pacer.record(Duration::from_millis(40), start);
        for _ in 1..PACER_WINDOW {
            pacer.record(Duration::from_micros(600), start);
        }
        assert_eq!(pacer.fps(), 120);

        let mut capped = Pacer::new(60);
        for _ in 0..PACER_WINDOW {
            capped.record(Duration::from_micros(100), start + PACER_COOLDOWN * 3);
        }
        assert_eq!(capped.fps(), 60, "never exceeds AGENTTRACE_FPS");
    }

    #[test]
    fn transitions_run_at_frame_rate_then_go_idle() {
        let mut motion = Motion::new(Some(120));
        let start = Instant::now();
        motion.begin_frame(scene(), false, start);
        assert_eq!(
            motion.next_frame_delay(start),
            Some(motion.frame_interval())
        );
        let mut next = scene();
        next.detail = Some(DetailSection::Summary);
        motion.begin_frame(next, false, start);
        let later = start + Duration::from_secs(2);
        motion.begin_frame(next, false, later);
        assert_eq!(motion.next_frame_delay(later), None);
    }

    #[test]
    fn disabled_motion_never_schedules_frames_or_changes_values() {
        let mut motion = Motion::disabled();
        let now = Instant::now();
        motion.begin_frame(scene(), true, now);
        assert_eq!(motion.next_frame_delay(now), None);
        assert_eq!(motion.counter("x", 42.0, COUNTER_DURATION), 42.0);
        assert_eq!(motion.spark_growth(3), 1.0);
    }

    #[test]
    fn counters_roll_toward_target() {
        let mut motion = Motion::new(Some(120));
        let start = Instant::now();
        motion.begin_frame(scene(), false, start);
        assert_eq!(motion.counter("cost", 10.0, COUNTER_DURATION), 0.0);
        motion.begin_frame(scene(), false, start + COUNTER_DURATION / 2);
        let mid = motion.counter("cost", 10.0, COUNTER_DURATION);
        assert!(mid > 0.0 && mid < 10.0, "{mid}");
        motion.begin_frame(scene(), false, start + COUNTER_DURATION);
        assert_eq!(motion.counter("cost", 10.0, COUNTER_DURATION), 10.0);
    }

    #[test]
    fn notice_fade_wakes_before_expiry() {
        let mut motion = Motion::new(Some(120));
        let now = Instant::now();
        motion.begin_frame(scene(), false, now);
        let mut next = scene();
        next.notice = Some(1);
        next.notice_expiry = Some(now + Duration::from_secs(2));
        let later = now + Duration::from_secs(1);
        motion.begin_frame(next, false, later);
        let delay = motion.next_frame_delay(later + NOTICE_IN_DURATION).unwrap();
        assert!(delay <= Duration::from_secs(1), "{delay:?}");
    }

    #[test]
    fn effects_only_use_the_existing_palette() {
        let mut terminal = Terminal::new(TestBackend::new(20, 3)).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                let buf = frame.buffer_mut();
                for x in 0..area.width {
                    buf[(x, 0)].set_symbol("x").set_fg(Color::Cyan);
                    buf[(x, 1)].set_symbol("y").set_bg(Color::Cyan);
                }
                sweep_columns(buf, area, 1, 0.1);
                sweep_rows(buf, area, 0.1);
                shift_columns(buf, area, 1, 3);
                wipe_selection(buf, Rect::new(0, 1, 20, 1), 0.5);
            })
            .unwrap();
        let allowed = [Color::Reset, Color::DarkGray, Color::Cyan];
        for cell in terminal.backend().buffer().content() {
            assert!(allowed.contains(&cell.fg), "{:?}", cell.fg);
            assert!(allowed.contains(&cell.bg), "{:?}", cell.bg);
        }
    }

    #[test]
    fn animated_explorer_render_keeps_layout_intact() {
        let mut app = App::new(Vec::new(), "test", None);
        app.motion = Motion::new(Some(120));
        let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
        for _ in 0..3 {
            terminal
                .draw(|frame| render_explorer(frame, &mut app))
                .unwrap();
        }
        app.explorer_overlay = ExplorerOverlay::Help;
        terminal
            .draw(|frame| render_explorer(frame, &mut app))
            .unwrap();
        assert!(format!("{:?}", terminal.backend().buffer()).contains("AgentTrace"));
    }
}
