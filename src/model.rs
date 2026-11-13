//! App state. Demo numbers are a function of the clock; a real scan arrives from a background walk.
//! Clearing in demo mode only changes those numbers. The delete path is never called unless the
//! user confirms, and then only for `~/Library/Caches` or `~/.Trash`.

use std::sync::Arc;

use rune::{Pixels, Tx, Waker};

use crate::bar::{self, Particle};
use crate::demo::{self, Activity};
use crate::scan::{self, Cat, N, Purge, Shared, Snap, Stat};

pub const CARD_W: f32 = 960.0;
pub const CARD_H: f32 = 564.0;
pub const PAD_X: f32 = 36.0;
pub const HEADER_Y: f32 = 32.0;
pub const STATUS_Y: f32 = 88.0;
pub const BAR_Y: f32 = 148.0;
pub const LEGEND_Y: f32 = 310.0;
pub const LEGEND_H: f32 = 132.0;
pub const DIVIDER_Y: f32 = 468.0;
/// Legend order, row by row. Hit cells tile this grid with no gaps.
pub const LEGEND: [[Cat; 3]; 3] = [
    [Cat::Apps, Cat::Developer, Cat::Photos],
    [Cat::Movies, Cat::Documents, Cat::Music],
    [Cat::System, Cat::Caches, Cat::Trash],
];
/// Hover fades in and out over this long, ease-out.
const HOVER_SECS: f32 = 0.18;
/// Keep the last category this long after the pointer leaves the legend and the bar.
/// A move into another category inside the window goes straight across, with no default line in between.
const HOVER_GRACE: f32 = 0.14;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Readout {
    Calculating(String),
    Clearing(String),
    Emptying(String),
    Hover,
    Quiet,
    Notice(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Footer {
    Scanning,
    Caches,
    Trash,
    Done,
}

/// What the line under the total is showing. Path and size text can change without a new fade.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StatusId {
    Quiet,
    Hover(Cat),
    Calculating,
    Clearing,
    Emptying,
    Notice,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusSnap {
    pub id: StatusId,
    pub cat: Option<Cat>,
    pub title: String,
    pub detail: String,
    pub blurb: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FooterSnap {
    pub kind: Footer,
    pub message: String,
    pub button: Option<&'static str>,
}

/// Two copies of a line, crossfading. `mix` is the incoming layer's opacity.
/// Retargeting never adds a third copy: the clearer of the two stays, at the opacity it already has.
#[derive(Clone, Debug)]
pub struct Crossfade<T: Clone> {
    pub base: T,
    pub incoming: Option<T>,
    pub mix: f32,
    from: f32,
    goal: f32,
    elapsed: f32,
    primed: bool,
}

const SLIDE: f32 = 3.0;

impl<T: Clone> Crossfade<T> {
    fn new(base: T) -> Self {
        Self { base, incoming: None, mix: 1.0, from: 0.0, goal: 1.0, elapsed: 0.0, primed: false }
    }

    /// Incoming layer's vertical offset, and the base layer's. Settled lines sit at 0.
    pub fn slide(&self) -> (f32, f32) {
        if self.incoming.is_none() {
            return (0.0, 0.0);
        }
        (SLIDE * (1.0 - self.mix), -SLIDE * self.mix)
    }

    fn step<K: PartialEq>(&mut self, next: T, key: impl Fn(&T) -> K, dt: f32) -> bool {
        if !self.primed {
            self.base = next;
            self.primed = true;
            self.incoming = None;
            self.mix = 1.0;
            return false;
        }
        let next_key = key(&next);
        if self.incoming.is_none() && key(&self.base) == next_key {
            self.base = next;
            return false;
        }
        if self.incoming.as_ref().is_some_and(|inc| key(inc) == next_key) {
            self.incoming = Some(next);
            if self.goal != 1.0 {
                self.from = self.mix;
                self.goal = 1.0;
                self.elapsed = 0.0;
            }
            return self.advance(dt);
        }
        if key(&self.base) == next_key && self.incoming.is_some() {
            self.base = next;
            if self.goal != 0.0 {
                self.from = self.mix;
                self.goal = 0.0;
                self.elapsed = 0.0;
            }
            return self.advance(dt);
        }
        self.begin(next);
        self.advance(dt)
    }

    fn begin(&mut self, next: T) {
        if self.incoming.is_some() {
            // The clearer layer becomes the one that fades out, still at its current opacity.
            // The new line takes the opacity the fainter layer already had, so nothing snaps.
            let visible = self.mix.clamp(0.0, 1.0);
            if visible >= 0.5 {
                if let Some(inc) = self.incoming.take() {
                    self.base = inc;
                }
                self.mix = 1.0 - visible;
            }
        } else {
            self.mix = 0.0;
        }
        self.incoming = Some(next);
        self.from = self.mix;
        self.goal = 1.0;
        self.elapsed = 0.0;
    }

    fn advance(&mut self, dt: f32) -> bool {
        let Some(_) = &self.incoming else { return false };
        self.elapsed += dt.max(0.0);
        let u = ease_out((self.elapsed / HOVER_SECS).clamp(0.0, 1.0));
        self.mix = self.from + (self.goal - self.from) * u;
        if self.elapsed < HOVER_SECS {
            return true;
        }
        self.mix = self.goal;
        if self.goal >= 1.0 {
            if let Some(inc) = self.incoming.take() {
                self.base = inc;
            }
        } else {
            self.incoming = None;
        }
        self.mix = 1.0;
        self.goal = 1.0;
        self.elapsed = 0.0;
        false
    }
}

pub struct State {
    pub t: f64,
    pub demo: bool,
    pub live: [f64; N],
    pub capacity: f64,
    pub hover: Option<Cat>,
    /// Category whose hit rect contains the pointer. Geometry in `settle_hover` decides what is shown.
    hit: Option<Cat>,
    /// The pointer has left every category and `hover` is held until `HOVER_GRACE` elapses.
    leaving: bool,
    leave_elapsed: f32,
    /// Window-point pointer, so headless frames can draw a cursor. The live window keeps the system pointer.
    pub pointer: Option<(f32, f32)>,
    /// Smoothed 0..1 hover for each category. The legend and the bar read this, not the raw pointer.
    pub hover_amt: [f32; N],
    hover_from: [f32; N],
    hover_goal: [f32; N],
    hover_elapsed: [f32; N],
    pub readout: Readout,
    pub footer: Footer,
    pub status_fade: Crossfade<StatusSnap>,
    pub footer_fade: Crossfade<FooterSnap>,
    pub confirm: Option<Purge>,
    pub notice: Option<String>,
    pub held_caches: f64,
    pub held_trash: f64,
    pub pixels: Pixels,
    pub stats: [Stat; N],
    pub scanning: bool,
    scan_epoch: f64,
    pin_scan: Option<f64>,
    clear_origin: Option<f64>,
    trash_origin: Option<f64>,
    settled: bool,
    real_done: bool,
    real_path: String,
    sig: u64,
    particles: Vec<Particle>,
    prev_edges: Option<[bar::Edge; N]>,
    shared: Arc<Shared>,
    waker: Option<Waker>,
}

impl State {
    pub fn new() -> Self {
        let demo = demo_mode();
        State {
            t: 0.0,
            demo,
            live: [0.0; N],
            capacity: if demo { demo::CAPACITY } else { 0.0 },
            hover: None,
            hit: None,
            leaving: false,
            leave_elapsed: 0.0,
            pointer: None,
            hover_amt: [0.0; N],
            hover_from: [0.0; N],
            hover_goal: [0.0; N],
            hover_elapsed: [0.0; N],
            readout: Readout::Quiet,
            footer: Footer::Scanning,
            status_fade: Crossfade::new(StatusSnap::quiet()),
            footer_fade: Crossfade::new(FooterSnap::scanning()),
            confirm: None,
            notice: None,
            held_caches: if demo { demo::TOTALS[Cat::Caches.ix()] } else { 0.0 },
            held_trash: if demo { demo::TOTALS[Cat::Trash.ix()] } else { 0.0 },
            pixels: Pixels::new(),
            stats: std::array::from_fn(|_| Stat::default()),
            scanning: true,
            scan_epoch: 0.0,
            pin_scan: None,
            clear_origin: None,
            trash_origin: None,
            settled: false,
            real_done: false,
            real_path: String::new(),
            sig: 0,
            particles: Vec::new(),
            prev_edges: None,
            shared: Arc::new(Shared::new()),
            waker: None,
        }
    }

    pub fn scene_scanning(&mut self) {
        self.pin_scan = Some(5.5);
    }

    pub fn scene_idle(&mut self) {
        self.pin_scan = Some(99.0);
        // The still is the rested hover, not the first frames of the fade.
        self.hover = Some(Cat::Apps);
        self.hover_amt[Cat::Apps.ix()] = 1.0;
        self.hover_from[Cat::Apps.ix()] = 1.0;
        self.hover_goal[Cat::Apps.ix()] = 1.0;
        self.hover_elapsed[Cat::Apps.ix()] = HOVER_SECS;
    }

    pub fn scene_clearing(&mut self) {
        self.pin_scan = Some(99.0);
        self.clear_origin = Some(0.0);
    }

    pub fn scene_emptied(&mut self) {
        self.pin_scan = Some(99.0);
        self.settled = true;
    }
}

pub fn demo_mode() -> bool {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--real") {
        return false;
    }
    args.iter().any(|a| a == "--demo") || cfg!(target_os = "linux")
}

pub fn setup(s: &mut State, waker: Waker) {
    s.waker = Some(waker.clone());
    if !s.demo {
        scan::spawn(s.shared.clone(), waker);
    }
}

pub fn set_pointer(s: &mut State, x: f32, y: f32) {
    s.pointer = Some((x, y));
}

/// What a legend or bar hit rect reported. The shown category follows the pointer geometry, so a
/// leave that arrives before the next enter cannot blank the line for a frame.
pub fn note_hit(s: &mut State, cat: Cat, inside: bool) {
    if inside {
        s.hit = Some(cat);
    } else if s.hit == Some(cat) {
        s.hit = None;
    }
}

/// Window rect of one legend cell. The nine cells tile the legend with shared edges and no gaps.
pub fn legend_hit_rect(row: usize, col: usize) -> (f32, f32, f32, f32) {
    let cw = bar::BAR_W / 3.0;
    let rh = LEGEND_H / 3.0;
    (PAD_X + col as f32 * cw, LEGEND_Y + row as f32 * rh, cw, rh)
}

/// Bar hit spans in window x, each abutting the next. The left inset belongs to the first
/// category, so a horizontal move never falls in a gap between segments.
pub fn bar_hit_spans(bytes: &[f64; N], capacity: f64) -> Vec<(Cat, f32, f32)> {
    let edges = bar::edges(bytes, capacity);
    let mut spans = Vec::new();
    let mut start = PAD_X;
    for cat in Cat::ALL {
        let edge = edges[cat.ix()];
        if edge.w < 0.5 {
            continue;
        }
        let end = PAD_X + edge.x + edge.w;
        if end > start + 0.5 {
            spans.push((cat, start, end));
            start = end;
        }
    }
    spans
}

/// Category under a window point. Legend wins; the bar's free space is not a category.
pub fn category_at(x: f32, y: f32, bytes: &[f64; N], capacity: f64) -> Option<Cat> {
    if (LEGEND_Y..LEGEND_Y + LEGEND_H).contains(&y) && (PAD_X..PAD_X + bar::BAR_W).contains(&x) {
        let col = ((x - PAD_X) / (bar::BAR_W / 3.0)).floor() as usize;
        let row = ((y - LEGEND_Y) / (LEGEND_H / 3.0)).floor() as usize;
        return Some(LEGEND[row.min(2)][col.min(2)]);
    }
    if (BAR_Y..BAR_Y + bar::BAR_H).contains(&y) {
        for (cat, x0, x1) in bar_hit_spans(bytes, capacity) {
            if x >= x0 && x < x1 {
                return Some(cat);
            }
        }
    }
    None
}

/// Own hover amount, and the strongest other category (what dims this row and its bar segment).
pub fn hover_pair(s: &State, cat: Cat) -> (f32, f32) {
    let mine = s.hover_amt[cat.ix()];
    let mut others = 0.0f32;
    for (i, v) in s.hover_amt.iter().enumerate() {
        if i != cat.ix() {
            others = others.max(*v);
        }
    }
    (mine, others)
}

pub fn press_rescan(s: &mut State) {
    s.scan_epoch = s.t;
    s.pin_scan = None;
    s.clear_origin = None;
    s.trash_origin = None;
    s.settled = false;
    s.hover = None;
    s.hit = None;
    s.leaving = false;
    s.leave_elapsed = 0.0;
    s.hover_amt = [0.0; N];
    s.hover_from = [0.0; N];
    s.hover_goal = [0.0; N];
    s.hover_elapsed = [0.0; N];
    s.confirm = None;
    s.notice = None;
    s.particles.clear();
    s.prev_edges = None;
    s.sig = 0;
    s.real_done = false;
    if s.demo {
        s.held_caches = demo::TOTALS[Cat::Caches.ix()];
        s.held_trash = demo::TOTALS[Cat::Trash.ix()];
    } else if let Some(waker) = s.waker.clone() {
        s.held_caches = 0.0;
        s.held_trash = 0.0;
        scan::spawn(s.shared.clone(), waker);
    }
}

pub fn press_clear(s: &mut State) {
    if s.scanning || s.clear_origin.is_some() || s.confirm.is_some() || s.footer != Footer::Caches {
        return;
    }
    // Demo confirms too, then only plays the animation. Real mode deletes after the same dialog.
    s.confirm = Some(Purge::Caches);
}

pub fn press_trash(s: &mut State) {
    if s.scanning || s.trash_origin.is_some() || s.confirm.is_some() || s.footer != Footer::Trash {
        return;
    }
    s.confirm = Some(Purge::Trash);
}

pub fn press_cancel(s: &mut State) {
    s.confirm = None;
}

/// Confirmed delete. Demo mode never reaches this; it animates without touching disk.
pub fn press_confirm(s: &mut State) {
    let Some(which) = s.confirm.take() else { return };
    if s.demo {
        begin_anim(s, which);
        return;
    }
    let Some(home) = scan::home_dir() else {
        s.notice = Some("No home directory, so nothing was deleted.".into());
        return;
    };
    match scan::purge(&home, which) {
        Ok(freed) => {
            scan::note_purged(&s.shared, which.cat(), freed);
            begin_anim(s, which);
        }
        Err(err) => s.notice = Some(err),
    }
}

fn begin_anim(s: &mut State, which: Purge) {
    match which {
        Purge::Caches => {
            s.held_caches = s.live[Cat::Caches.ix()].max(s.held_caches);
            s.clear_origin = Some(s.t);
        }
        Purge::Trash => {
            s.held_trash = s.live[Cat::Trash.ix()].max(s.held_trash);
            s.trash_origin = Some(s.t);
        }
    }
}

pub fn poll(s: &mut State, t: f64, tx: &mut Tx) -> bool {
    let dt = (t - s.t).max(0.0);
    s.t = t;
    // The bar and the figures are sampled from the clock. Layout springs would trail them.
    tx.instant();

    let scan_t = s.pin_scan.unwrap_or_else(|| (s.t - s.scan_epoch).max(0.0));
    let clear_age = s.clear_origin.map(|t0| (s.t - t0).max(0.0));
    let trash_age = s.trash_origin.map(|t0| (s.t - t0).max(0.0));

    let mut drift = false;
    if s.demo {
        s.live = demo::bytes(scan_t, clear_age, trash_age, s.settled);
        s.capacity = demo::CAPACITY;
        s.scanning = demo::scanning(scan_t) && !s.settled;
    } else {
        let snap = s.shared.snap.lock().expect("scan lock").clone();
        apply_real(s, &snap, clear_age, trash_age, dt, &mut drift);
    }

    if !s.scanning && s.clear_origin.is_none() {
        s.held_caches = s.live[Cat::Caches.ix()];
    }
    if !s.scanning && s.trash_origin.is_none() {
        s.held_trash = s.live[Cat::Trash.ix()];
    }

    s.footer = footer(s.scanning, &s.live, clear_age, trash_age, s.settled);
    // Decide the category before the detail line, so a gap never paints the default for a frame.
    let hover_holding = settle_hover(s, dt as f32);
    s.readout = readout(s, scan_t, clear_age, trash_age);

    let dissolving = if clear_age.is_some_and(|age| age < demo::CLEAR_DUR) {
        Some(Cat::Caches.ix())
    } else if trash_age.is_some_and(|age| age < demo::CLEAR_DUR) {
        Some(Cat::Trash.ix())
    } else {
        None
    };
    let edges = bar::edges(&s.live, s.capacity);
    bar::advance(&mut s.particles, &mut s.prev_edges, &edges, dissolving, dt as f32);
    let hover_moving = step_hover(s, dt as f32);
    let status_moving = s.status_fade.step(status_snap(s), |snap| snap.id.clone(), dt as f32);
    let footer_moving = s.footer_fade.step(footer_snap(s), |snap| snap.clone(), dt as f32);
    bar::render(
        &s.pixels,
        &mut s.sig,
        &bar::Input { bytes: &s.live, capacity: s.capacity, hover: &s.hover_amt, scanning: s.scanning, particles: &s.particles },
    );

    let clearing = clear_age.is_some_and(|age| age < demo::CLEAR_DUR + 0.15);
    let emptying = trash_age.is_some_and(|age| age < demo::CLEAR_DUR + 0.15);
    if std::env::var_os("HOVER_TRACE").is_some() {
        trace_hover(s);
    }
    s.scanning || clearing || emptying || !s.particles.is_empty() || drift || hover_holding || hover_moving || status_moving || footer_moving || (!s.demo && !s.real_done)
}

/// Hold the current category across a short gap. Entering another category inside the grace
/// replaces it directly. The default line waits until the pointer has been outside the legend
/// and the bar for the whole grace.
fn settle_hover(s: &mut State, dt: f32) -> bool {
    if s.confirm.is_some() {
        return false;
    }
    let under = match s.pointer {
        Some((x, y)) => category_at(x, y, &s.live, s.capacity),
        None => return false,
    };
    if let Some(cat) = under {
        let changed = s.hover != Some(cat);
        s.hover = Some(cat);
        s.leaving = false;
        s.leave_elapsed = 0.0;
        return changed;
    }
    if s.hover.is_none() {
        s.leaving = false;
        s.leave_elapsed = 0.0;
        return false;
    }
    if !s.leaving {
        s.leaving = true;
        s.leave_elapsed = 0.0;
    }
    s.leave_elapsed += dt.max(0.0);
    if s.leave_elapsed >= HOVER_GRACE {
        s.hover = None;
        s.leaving = false;
        s.leave_elapsed = 0.0;
        return true;
    }
    true
}

fn trace_hover(s: &State) {
    let geom = s.pointer.and_then(|(x, y)| category_at(x, y, &s.live, s.capacity));
    let base_op = if s.status_fade.incoming.is_some() { 1.0 - s.status_fade.mix } else { 1.0 };
    let (inc, inc_op) = match &s.status_fade.incoming {
        Some(v) => (status_label(&v.id), format!("{:.4}", s.status_fade.mix)),
        None => ("-".to_string(), "-".to_string()),
    };
    eprintln!(
        "HOVER t={:.4} geom={} hit={} shown={} base={} bop={:.4} inc={} iop={}",
        s.t,
        geom.map(Cat::key).unwrap_or("-"),
        s.hit.map(Cat::key).unwrap_or("-"),
        s.hover.map(Cat::key).unwrap_or("-"),
        status_label(&s.status_fade.base.id),
        base_op,
        inc,
        inc_op,
    );
}

fn status_label(id: &StatusId) -> String {
    match id {
        StatusId::Quiet => "quiet".into(),
        StatusId::Hover(cat) => format!("hover:{}", cat.key()),
        StatusId::Calculating => "calculating".into(),
        StatusId::Clearing => "clearing".into(),
        StatusId::Emptying => "emptying".into(),
        StatusId::Notice => "notice".into(),
    }
}

/// Ease each category toward 1 while the pointer is inside it and toward 0 after it leaves.
/// Retargeting mid-fade starts from the value already on screen. Returns whether any fade is still moving.
fn step_hover(s: &mut State, dt: f32) -> bool {
    let mut moving = false;
    for i in 0..N {
        let goal = if s.hover == Some(Cat::ALL[i]) { 1.0 } else { 0.0 };
        if (goal - s.hover_goal[i]).abs() > 0.001 {
            s.hover_from[i] = s.hover_amt[i];
            s.hover_elapsed[i] = 0.0;
            s.hover_goal[i] = goal;
        }
        if (s.hover_amt[i] - s.hover_goal[i]).abs() <= 0.001 {
            s.hover_amt[i] = s.hover_goal[i];
            continue;
        }
        s.hover_elapsed[i] += dt;
        let u = ease_out((s.hover_elapsed[i] / HOVER_SECS).clamp(0.0, 1.0));
        s.hover_amt[i] = s.hover_from[i] + (s.hover_goal[i] - s.hover_from[i]) * u;
        if s.hover_elapsed[i] >= HOVER_SECS {
            s.hover_amt[i] = s.hover_goal[i];
        }
        if (s.hover_amt[i] - s.hover_goal[i]).abs() > 0.001 {
            moving = true;
        }
    }
    moving
}

/// Ease-out quadratic: fast at the start, settled at the end.
fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t) * (1.0 - t)
}

fn apply_real(s: &mut State, snap: &Snap, clear_age: Option<f64>, trash_age: Option<f64>, dt: f64, drift: &mut bool) {
    if snap.capacity > 0 {
        s.capacity = snap.capacity as f64;
    }
    s.stats = snap.stats.clone();
    s.real_done = snap.done;
    s.real_path = snap.path.clone();
    if s.notice.is_none() {
        s.notice = snap.error.clone();
    }
    s.scanning = !snap.done;
    let mut target = [0.0; N];
    for i in 0..N {
        target[i] = snap.bytes[i] as f64;
    }
    if let Some(age) = clear_age.filter(|age| *age < demo::CLEAR_DUR) {
        target[Cat::Caches.ix()] = s.held_caches * (1.0 - demo::ease(age / demo::CLEAR_DUR));
    }
    if let Some(age) = trash_age.filter(|age| *age < demo::CLEAR_DUR) {
        target[Cat::Trash.ix()] = s.held_trash * (1.0 - demo::ease(age / demo::CLEAR_DUR));
    }
    for i in 0..N {
        let next = smooth(s.live[i], target[i], dt);
        if (next - target[i]).abs() > 2.0e7 {
            *drift = true;
        }
        s.live[i] = next;
    }
}

fn smooth(cur: f64, target: f64, dt: f64) -> f64 {
    if dt <= 0.0 {
        return cur;
    }
    let k = 1.0 - (-dt / 0.42).exp();
    cur + (target - cur) * k
}

fn footer(scanning: bool, live: &[f64; N], clear_age: Option<f64>, trash_age: Option<f64>, settled: bool) -> Footer {
    if scanning {
        return Footer::Scanning;
    }
    if settled {
        return Footer::Done;
    }
    if clear_age.is_some_and(|age| age < demo::CLEAR_DUR) {
        return Footer::Caches;
    }
    if trash_age.is_some_and(|age| age < demo::CLEAR_DUR) {
        return Footer::Trash;
    }
    if live[Cat::Caches.ix()] > 0.2 * demo::GB && clear_age.is_none() {
        return Footer::Caches;
    }
    if live[Cat::Trash.ix()] > 0.2 * demo::GB {
        return Footer::Trash;
    }
    Footer::Done
}

impl StatusSnap {
    fn quiet() -> Self {
        Self {
            id: StatusId::Quiet,
            cat: None,
            title: "Hover a category to see what's using space.".into(),
            detail: String::new(),
            blurb: String::new(),
        }
    }
}

impl FooterSnap {
    fn scanning() -> Self {
        Self { kind: Footer::Scanning, message: "Looking for things you can clear".into(), button: None }
    }
}

fn status_snap(s: &State) -> StatusSnap {
    match &s.readout {
        Readout::Quiet => StatusSnap::quiet(),
        Readout::Notice(msg) => StatusSnap { id: StatusId::Notice, cat: None, title: msg.clone(), detail: String::new(), blurb: String::new() },
        Readout::Calculating(path) => StatusSnap { id: StatusId::Calculating, cat: None, title: "Calculating".into(), detail: path.clone(), blurb: String::new() },
        Readout::Clearing(path) => StatusSnap { id: StatusId::Clearing, cat: None, title: "Clearing caches".into(), detail: path.clone(), blurb: String::new() },
        Readout::Emptying(path) => StatusSnap { id: StatusId::Emptying, cat: None, title: "Emptying the Trash".into(), detail: path.clone(), blurb: String::new() },
        Readout::Hover => {
            let cat = s.hover.unwrap_or(Cat::Apps);
            let blurb = if s.demo { demo::blurb(cat).to_string() } else { scan::blurb(cat, &s.stats[cat.ix()]) };
            StatusSnap {
                id: StatusId::Hover(cat),
                cat: Some(cat),
                title: cat.name().into(),
                detail: format!("{} GB", demo::fmt_gb(s.live[cat.ix()])),
                blurb,
            }
        }
    }
}

fn footer_snap(s: &State) -> FooterSnap {
    match s.footer {
        Footer::Scanning => FooterSnap::scanning(),
        Footer::Caches => FooterSnap {
            kind: Footer::Caches,
            message: format!("Caches are using {} GB. Apps rebuild them when they need them.", demo::fmt_gb(s.held_caches)),
            button: Some("Clear caches"),
        },
        Footer::Trash => FooterSnap {
            kind: Footer::Trash,
            message: format!("The Trash is holding {} GB of things you've already deleted.", demo::fmt_gb(s.held_trash)),
            button: Some("Empty Trash"),
        },
        Footer::Done => FooterSnap { kind: Footer::Done, message: "Caches and the Trash are clear.".into(), button: None },
    }
}

fn readout(s: &State, scan_t: f64, clear_age: Option<f64>, trash_age: Option<f64>) -> Readout {
    if let Some(msg) = &s.notice {
        return Readout::Notice(msg.clone());
    }
    if s.demo {
        return match demo::activity(scan_t, clear_age, trash_age) {
            Some((Activity::Calculating, path)) => Readout::Calculating(path.into()),
            Some((Activity::Clearing, path)) => Readout::Clearing(path.into()),
            Some((Activity::Emptying, path)) => Readout::Emptying(path.into()),
            None if s.hover.is_some() => Readout::Hover,
            None => Readout::Quiet,
        };
    }
    if s.scanning {
        let path = if s.real_path.is_empty() { "Starting…".into() } else { s.real_path.clone() };
        return Readout::Calculating(path);
    }
    if clear_age.is_some_and(|age| age < demo::CLEAR_DUR) {
        return Readout::Clearing("~/Library/Caches".into());
    }
    if trash_age.is_some_and(|age| age < demo::CLEAR_DUR) {
        return Readout::Emptying("~/.Trash".into());
    }
    if s.hover.is_some() { Readout::Hover } else { Readout::Quiet }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hover_eases_out_over_about_180ms_and_settles() {
        let mut s = State::new();
        s.hover = Some(Cat::Apps);
        assert!(step_hover(&mut s, 0.0), "a new hover keeps frames coming");
        assert!(s.hover_amt[Cat::Apps.ix()] < 0.01);

        assert!(step_hover(&mut s, 0.09));
        let mid = s.hover_amt[Cat::Apps.ix()];
        // Ease-out is ahead of a straight line (0.5) and not finished yet.
        assert!(mid > 0.65 && mid < 0.9, "mid fade {mid}");

        let mut moving = true;
        for _ in 0..4 {
            moving = step_hover(&mut s, 0.05);
        }
        assert!(!moving);
        assert!((s.hover_amt[Cat::Apps.ix()] - 1.0).abs() < 1e-3);
        assert!(s.hover_amt[Cat::Photos.ix()] < 1e-3);

        s.hover = None;
        assert!(step_hover(&mut s, 0.0));
        assert!((s.hover_amt[Cat::Apps.ix()] - 1.0).abs() < 1e-3, "leave starts from the current value");
        assert!(!step_hover(&mut s, HOVER_SECS));
        assert!(s.hover_amt[Cat::Apps.ix()] < 1e-3);

        s.hover = Some(Cat::Photos);
        let _ = step_hover(&mut s, 0.04);
        let photos = s.hover_amt[Cat::Photos.ix()];
        assert!(photos > 0.2 && photos < 1.0, "enter fades in, {photos}");
    }

    #[test]
    fn the_detail_line_crossfades_and_then_rests() {
        let mut fade = Crossfade::new(StatusSnap::quiet());
        let quiet = StatusSnap::quiet();
        assert!(!fade.step(quiet, |s| s.id.clone(), 0.0));
        let apps = StatusSnap {
            id: StatusId::Hover(Cat::Apps),
            cat: Some(Cat::Apps),
            title: "Apps".into(),
            detail: "52.6 GB".into(),
            blurb: "142 apps.".into(),
        };
        assert!(fade.step(apps.clone(), |s| s.id.clone(), 0.0));
        assert!(fade.step(apps.clone(), |s| s.id.clone(), 0.09));
        assert!(fade.mix > 0.65 && fade.mix < 0.9, "mix {}", fade.mix);
        assert!(!fade.step(apps.clone(), |s| s.id.clone(), 0.1));
        assert!(fade.incoming.is_none());
        assert_eq!(fade.base.id, StatusId::Hover(Cat::Apps));

        let quiet = StatusSnap::quiet();
        assert!(fade.step(quiet.clone(), |s| s.id.clone(), 0.0));
        assert!(!fade.step(quiet, |s| s.id.clone(), HOVER_SECS));
        assert!(fade.incoming.is_none());
        assert_eq!(fade.base.id, StatusId::Quiet);

        let mut path_a = apps.clone();
        path_a.id = StatusId::Calculating;
        path_a.detail = "~/a".into();
        assert!(!fade.step(path_a.clone(), |s| s.id.clone(), 0.2));
        path_a.detail = "~/b".into();
        assert!(!fade.step(path_a, |s| s.id.clone(), 0.0), "a new path in the same line does not restart the fade");
        assert!(fade.incoming.is_none());
        assert_eq!(fade.base.detail, "~/b");
    }

    #[test]
    fn retargeting_mid_fade_keeps_the_opacity_already_on_screen() {
        let mut fade = Crossfade::new(StatusSnap::quiet());
        assert!(!fade.step(StatusSnap::quiet(), |s| s.id.clone(), 0.0));
        let apps = hover_snap(Cat::Apps);
        let photos = hover_snap(Cat::Photos);
        let dev = hover_snap(Cat::Developer);

        assert!(fade.step(apps.clone(), |s| s.id.clone(), 0.09));
        let dominant = fade.mix;
        assert!(dominant > 0.5, "apps is the clearer layer, mix {dominant}");
        assert!(fade.step(photos.clone(), |s| s.id.clone(), 0.0));
        assert!((1.0 - fade.mix - dominant).abs() < 1e-4, "apps stays at {dominant}, mix {}", fade.mix);
        assert_eq!(fade.base.id, StatusId::Hover(Cat::Apps));
        assert_eq!(fade.incoming.as_ref().map(|s| s.id.clone()), Some(StatusId::Hover(Cat::Photos)));

        let mut fade = Crossfade::new(StatusSnap::quiet());
        assert!(!fade.step(StatusSnap::quiet(), |s| s.id.clone(), 0.0));
        assert!(fade.step(apps.clone(), |s| s.id.clone(), 0.02));
        let faint = fade.mix;
        assert!(faint < 0.5, "apps is still the fainter layer, mix {faint}");
        let base_op = 1.0 - faint;
        assert!(fade.step(dev, |s| s.id.clone(), 0.0));
        assert!((fade.mix - faint).abs() < 1e-4, "the new line takes the faint opacity");
        assert!((1.0 - fade.mix - base_op).abs() < 1e-4);
        assert_eq!(fade.base.id, StatusId::Quiet);
        assert!(fade.incoming.is_some());
    }

    #[test]
    fn legend_and_bar_hits_cover_the_gaps() {
        let (x, _, w, _) = legend_hit_rect(0, 0);
        let (x1, _, _, _) = legend_hit_rect(0, 1);
        assert!((x + w - x1).abs() < 1e-4, "legend columns meet");
        let (_, y0, _, h0) = legend_hit_rect(0, 0);
        let (_, y1, _, _) = legend_hit_rect(1, 0);
        assert!((y0 + h0 - y1).abs() < 1e-4, "legend rows meet");

        // The old layout left a 16px column gap and a 6px row gap. Both are inside a cell now.
        let gap_x = PAD_X + (bar::BAR_W - 32.0) / 3.0 + 8.0;
        let gap_y = LEGEND_Y + 40.0 + 3.0;
        assert_eq!(category_at(gap_x, LEGEND_Y + 20.0, &demo::TOTALS, demo::CAPACITY), Some(Cat::Apps));
        assert_eq!(category_at(175.0, gap_y, &demo::TOTALS, demo::CAPACITY), Some(Cat::Apps));
        assert_eq!(category_at(175.0, BAR_Y + bar::BAR_H + 10.0, &demo::TOTALS, demo::CAPACITY), None);

        let spans = bar_hit_spans(&demo::TOTALS, demo::CAPACITY);
        assert!(spans.len() >= 2);
        assert!((spans[0].1 - PAD_X).abs() < 1e-3, "the left inset belongs to the first segment");
        for pair in spans.windows(2) {
            assert!((pair[0].2 - pair[1].1).abs() < 1e-3, "bar segments meet");
            let boundary = pair[0].2;
            let y = BAR_Y + bar::BAR_H * 0.5;
            assert_eq!(category_at(boundary - 0.5, y, &demo::TOTALS, demo::CAPACITY), Some(pair[0].0));
            assert_eq!(category_at(boundary + 0.5, y, &demo::TOTALS, demo::CAPACITY), Some(pair[1].0));
        }
        assert_eq!(category_at(PAD_X + 2.0, BAR_Y + 40.0, &demo::TOTALS, demo::CAPACITY), Some(Cat::Apps));
        assert_eq!(category_at(PAD_X + bar::BAR_W - 4.0, BAR_Y + 40.0, &demo::TOTALS, demo::CAPACITY), None);
    }

    #[test]
    fn sweeping_across_gaps_does_not_dip_the_detail_line() {
        let mut s = idle_state();
        // Prime on the quiet line, then rest on Apps.
        let (x, y, w, h) = legend_hit_rect(0, 0);
        s.pointer = Some((x + w * 0.5, y + h * 0.5));
        for _ in 0..20 {
            drive(&mut s, 1.0 / 60.0);
        }
        assert!(s.status_fade.incoming.is_none());
        assert_eq!(s.status_fade.base.id, StatusId::Hover(Cat::Apps));

        let mut presence = Vec::new();
        let mut saw_quiet = false;
        // One step per frame, fast enough to cross the old 16px gap in well under the grace.
        let y = y + h * 0.5;
        let mut x = PAD_X + 8.0;
        while x < PAD_X + bar::BAR_W - 8.0 {
            s.pointer = Some((x, y));
            drive(&mut s, 1.0 / 60.0);
            presence.push(hover_presence(&s));
            saw_quiet |= layer_is_quiet(&s.status_fade.base) || s.status_fade.incoming.as_ref().is_some_and(layer_is_quiet);
            x += 8.0;
        }
        assert!(!saw_quiet, "the default line appears between categories");
        assert!(presence.iter().all(|p| *p > 0.98), "detail opacity dipped: {presence:?}");

        // Leave the legend for less than the grace, then enter Photos. Still no default line.
        s.pointer = Some((80.0, 40.0));
        for _ in 0..6 {
            drive(&mut s, 1.0 / 60.0);
        }
        assert!(s.hover.is_some(), "grace still holds the last category");
        assert!(!layer_is_quiet(&s.status_fade.base));
        let (px, py, pw, ph) = legend_hit_rect(0, 2);
        s.pointer = Some((px + pw * 0.5, py + ph * 0.5));
        for _ in 0..20 {
            drive(&mut s, 1.0 / 60.0);
            assert!(hover_presence(&s) > 0.98, "presence {}", hover_presence(&s));
            assert!(!layer_is_quiet(&s.status_fade.base));
            assert!(s.status_fade.incoming.as_ref().map(layer_is_quiet) != Some(true));
        }
        assert_eq!(s.hover, Some(Cat::Photos));

        // Staying outside past the grace is what reveals the default line.
        s.pointer = Some((80.0, 40.0));
        for _ in 0..12 {
            drive(&mut s, 1.0 / 60.0);
        }
        assert_eq!(s.hover, None);
        assert!(matches!(s.status_fade.base.id, StatusId::Hover(_)) || matches!(s.status_fade.incoming.as_ref().map(|i| i.id.clone()), Some(StatusId::Quiet)) || s.status_fade.base.id == StatusId::Quiet);
        let mut quiet = false;
        for _ in 0..12 {
            drive(&mut s, 1.0 / 60.0);
            quiet |= s.status_fade.base.id == StatusId::Quiet && s.status_fade.incoming.is_none();
        }
        assert!(quiet, "the default line settles only after the pointer has left");
    }

    fn hover_snap(cat: Cat) -> StatusSnap {
        StatusSnap { id: StatusId::Hover(cat), cat: Some(cat), title: cat.name().into(), detail: "1 GB".into(), blurb: "n".into() }
    }

    fn idle_state() -> State {
        let mut s = State::new();
        s.scanning = false;
        s.demo = true;
        s.live = demo::TOTALS;
        s.capacity = demo::CAPACITY;
        s.readout = Readout::Quiet;
        assert!(!s.status_fade.step(status_snap(&s), |snap| snap.id.clone(), 0.0));
        s
    }

    fn drive(s: &mut State, dt: f32) {
        s.t += dt as f64;
        let _ = settle_hover(s, dt);
        s.readout = if s.hover.is_some() { Readout::Hover } else { Readout::Quiet };
        let _ = step_hover(s, dt);
        let _ = s.status_fade.step(status_snap(s), |snap| snap.id.clone(), dt);
    }

    fn layer_is_quiet(snap: &StatusSnap) -> bool {
        snap.id == StatusId::Quiet
    }

    fn hover_presence(s: &State) -> f32 {
        let mut p = 0.0;
        let base_op = if s.status_fade.incoming.is_some() { 1.0 - s.status_fade.mix } else { 1.0 };
        if matches!(s.status_fade.base.id, StatusId::Hover(_)) {
            p += base_op;
        }
        if s.status_fade.incoming.as_ref().is_some_and(|i| matches!(i.id, StatusId::Hover(_))) {
            p += s.status_fade.mix;
        }
        p
    }
}
