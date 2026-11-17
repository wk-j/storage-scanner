//! The developer tools tab's state, drawn the way the overview is: a dot-matrix bar with one
//! segment per tool family, a 3×3 legend, a status line that follows the pointer, and a footer.
//!
//! Clicking a family opens it: the legend lists that family's items. Clicking an item puts its
//! cleanup command in the footer, with a Copy button. Nothing here deletes files.
//!
//! Families can be opened only once a measurement has finished, so the items never re-rank
//! under the pointer. While measuring, the bar grows on the disk's scale like the overview's
//! scan; when it finishes, the bar eases to the developer total so the families fill it.

use std::sync::Arc;

use rune::{Pixels, Waker};

use super::{demo, fmt_size, spawn, Group, Shared, Snap, GROUPS};
use crate::bar;
use crate::model::{ease_out, smooth, Crossfade, HOVER_GRACE, HOVER_SECS};

/// How long a Copy button reads "Copied".
pub const COPIED_SECS: f64 = 1.6;
/// Time constant of the bar's zoom from the disk's scale to the developer total.
const ZOOM_SECS: f64 = 0.25;

/// What the pointer is over: a family (bar segment or legend cell) or one item in an open family.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hit {
    Group(Group),
    Item(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StatusId {
    Quiet,
    Measuring,
    Group(Group),
    Item(String),
}

/// The line under the total, like the overview's: icon, title, size, and a sentence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    pub id: StatusId,
    pub group: Option<Group>,
    pub title: String,
    pub detail: String,
    pub blurb: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FootId {
    Measuring,
    Groups,
    Group(Group),
    Item(String),
}

/// The footer under the divider. An item's footer carries its note and command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Foot {
    pub id: FootId,
    pub group: Option<Group>,
    pub message: String,
    pub command: String,
    pub key: String,
    /// The Copy button reads "Copied". Changes in place: the footer's crossfade keys on `id`.
    pub copied: bool,
}

pub struct DevTab {
    demo: bool,
    /// The latest measurement read from the background job (or the demo clock).
    pub snap: Snap,
    shared: Arc<Shared>,
    pub waker: Option<Waker>,
    /// Clock time the current measurement started.
    origin: Option<f64>,
    /// A press asked for a measurement; the next frame starts it at that frame's clock.
    request: bool,
    /// Family sizes the bar, the legend and the total draw, eased toward the measurement.
    pub live: [f64; GROUPS],
    /// The bar's whole, eased: the disk while measuring, the developer total once done.
    pub cap: f64,
    pub pixels: Pixels,
    sig: u64,
    hit: Option<Hit>,
    /// What the status line and the bar treat as hovered, held for a short grace after a leave.
    pub hover: Option<Hit>,
    leaving: bool,
    leave_elapsed: f32,
    /// Smoothed 0..1 emphasis per family, as the overview's hover.
    pub amt: [f32; GROUPS],
    from: [f32; GROUPS],
    goal: [f32; GROUPS],
    elapsed: [f32; GROUPS],
    /// The open family, whose items the legend lists.
    pub group: Option<Group>,
    /// The item whose command the footer shows.
    pub selected: Option<String>,
    pub copied: Option<String>,
    copied_at: Option<f64>,
    pub status_fade: Crossfade<Status>,
    pub footer_fade: Crossfade<Foot>,
}

impl DevTab {
    pub fn new(demo: bool) -> Self {
        DevTab {
            demo,
            snap: Snap::default(),
            shared: Arc::new(Shared::new()),
            waker: None,
            origin: None,
            request: false,
            live: [0.0; GROUPS],
            cap: 0.0,
            pixels: Pixels::new(),
            sig: 0,
            hit: None,
            hover: None,
            leaving: false,
            leave_elapsed: 0.0,
            amt: [0.0; GROUPS],
            from: [0.0; GROUPS],
            goal: [0.0; GROUPS],
            elapsed: [0.0; GROUPS],
            group: None,
            selected: None,
            copied: None,
            copied_at: None,
            status_fade: Crossfade::new(Status::quiet()),
            footer_fade: Crossfade::new(Foot::measuring()),
        }
    }

    /// The tab became visible. The first time, measure; afterwards keep what was measured.
    pub fn open(&mut self) {
        if self.origin.is_none() {
            self.request = true;
        }
    }

    /// Measure again from empty, as the overview's Rescan does, and close any open family.
    pub fn rescan(&mut self) {
        self.request = true;
        self.live = [0.0; GROUPS];
        self.group = None;
        self.selected = None;
        self.copied = None;
        self.copied_at = None;
        self.hit = None;
        self.hover = None;
    }

    /// A demo measurement that finished long ago, for headless scenes.
    pub fn finished_demo(&mut self) {
        self.origin = Some(-demo::DURATION - 1.0);
    }

    pub fn measuring(&self) -> bool {
        self.origin.is_some() && !self.snap.done
    }

    pub fn total(&self) -> f64 {
        self.live.iter().sum()
    }

    pub fn note_hit(&mut self, hit: Hit, inside: bool) {
        if inside {
            self.hit = Some(hit);
        } else if self.hit.as_ref() == Some(&hit) {
            self.hit = None;
        }
    }

    /// Open a family. Only a finished measurement can be opened, so its items hold still.
    pub fn press_group(&mut self, group: Group) {
        if !self.snap.done || self.group == Some(group) {
            return;
        }
        self.group = Some(group);
        self.selected = None;
        self.copied = None;
        self.copied_at = None;
        self.hit = None;
    }

    pub fn press_item(&mut self, key: &str) {
        if self.selected.as_deref() == Some(key) || self.snap.row(key).is_none() {
            return;
        }
        self.selected = Some(key.to_string());
        self.copied = None;
        self.copied_at = None;
    }

    /// Back to every family.
    pub fn press_back(&mut self) {
        self.group = None;
        self.selected = None;
        self.copied = None;
        self.copied_at = None;
        self.hit = None;
    }

    /// A Copy button put the command on the clipboard. Nothing runs; the label confirms it.
    pub fn note_copied(&mut self, key: &str) {
        self.copied = Some(key.to_string());
        self.copied_at = None;
    }

    /// One frame. `dt` is wall time since the last frame, `visual_dt` the clamped step fades use.
    /// Returns whether anything visible changed or is still moving.
    pub fn step(&mut self, t: f64, dt: f64, visual_dt: f32, disk: f64) -> bool {
        let mut moving = false;
        if self.request {
            self.request = false;
            self.origin = Some(t);
            if !self.demo {
                if let Some(waker) = self.waker.clone() {
                    spawn(self.shared.clone(), waker);
                }
            }
        }
        if let Some(t0) = self.origin {
            let was_done = self.snap.done;
            self.snap = if self.demo { demo::snap(t - t0) } else { self.shared.snap.lock().expect("dev lock").clone() };
            // The frame a measurement finishes changes what's drawn: report it.
            moving |= !self.snap.done || !was_done;
            moving |= self.ease_sizes(dt, disk);
        }
        if self.group.is_some() && !self.snap.done {
            self.group = None;
            self.selected = None;
        }
        moving |= self.settle_hover(visual_dt);
        moving |= self.step_emphasis(visual_dt);
        let scanning = self.measuring();
        bar::render(
            &self.pixels,
            &mut self.sig,
            &bar::Input { bytes: &self.live, capacity: self.cap.max(1.0), hover: &self.amt, scanning },
        );
        let status = self.status_snap();
        let foot = self.foot_snap();
        let finishing = self.status_fade.incoming.is_some() || self.footer_fade.incoming.is_some();
        moving |= self.status_fade.step(status, |s| s.id.clone(), visual_dt);
        moving |= self.footer_fade.step(foot, |f| f.id.clone(), visual_dt);
        moving |= finishing;
        if self.copied.is_some() {
            // Stamp on the first frame after the press: a press after a long idle carries a stale clock.
            let at = *self.copied_at.get_or_insert(t);
            if t - at >= COPIED_SECS {
                self.copied = None;
                self.copied_at = None;
            }
            moving = true;
        }
        moving
    }

    /// Ease the family sizes and the bar's whole toward the measurement. A finished measurement
    /// seen from empty (a scene, or a tab opened after it finished) lands at once.
    fn ease_sizes(&mut self, dt: f64, disk: f64) -> bool {
        let total = self.snap.total as f64;
        let cap_goal = if self.snap.done { total.max(1.0) } else { disk.max(total).max(1.0) };
        let mut target = [0.0; GROUPS];
        for (i, b) in self.snap.groups.iter().enumerate() {
            target[i] = *b as f64;
        }
        if self.snap.done && self.live.iter().all(|v| *v == 0.0) {
            self.live = target;
            self.cap = cap_goal;
            return true;
        }
        if self.cap <= 0.0 {
            self.cap = cap_goal;
        }
        let mut drift = false;
        for i in 0..GROUPS {
            self.live[i] = smooth(self.live[i], target[i], dt);
            drift |= (self.live[i] - target[i]).abs() > 2.0e7;
        }
        // The zoom to the developer total is quicker than the sizes' ease, and lands exactly.
        if dt > 0.0 {
            self.cap += (cap_goal - self.cap) * (1.0 - (-dt / ZOOM_SECS).exp());
        }
        if (self.cap - cap_goal).abs() <= cap_goal * 0.002 {
            self.cap = cap_goal;
        }
        drift || self.cap != cap_goal
    }

    /// Keep the hovered thing across a short gap between cells, as the overview does.
    fn settle_hover(&mut self, dt: f32) -> bool {
        if let Some(hit) = self.hit.clone() {
            let changed = self.hover.as_ref() != Some(&hit);
            self.hover = Some(hit);
            self.leaving = false;
            self.leave_elapsed = 0.0;
            return changed;
        }
        if self.hover.is_none() {
            self.leaving = false;
            return false;
        }
        if !self.leaving {
            self.leaving = true;
            self.leave_elapsed = 0.0;
        }
        self.leave_elapsed += dt.max(0.0);
        if self.leave_elapsed >= HOVER_GRACE {
            self.hover = None;
            self.leaving = false;
        }
        true
    }

    /// The family the bar and the legend bring forward: the hovered one, else the open one.
    pub fn emphasis(&self) -> Option<Group> {
        match &self.hover {
            Some(Hit::Group(g)) => Some(*g),
            Some(Hit::Item(_)) => self.group,
            None => self.group,
        }
    }

    fn step_emphasis(&mut self, dt: f32) -> bool {
        let on = if self.measuring() { None } else { self.emphasis() };
        let mut moving = false;
        for i in 0..GROUPS {
            let goal = if on.map(Group::ix) == Some(i) { 1.0 } else { 0.0 };
            if (goal - self.goal[i]).abs() > 0.001 {
                self.from[i] = self.amt[i];
                self.elapsed[i] = 0.0;
                self.goal[i] = goal;
            }
            if (self.amt[i] - self.goal[i]).abs() <= 0.001 {
                self.amt[i] = self.goal[i];
                continue;
            }
            self.elapsed[i] += dt;
            let u = ease_out((self.elapsed[i] / HOVER_SECS).clamp(0.0, 1.0));
            self.amt[i] = self.from[i] + (self.goal[i] - self.from[i]) * u;
            if self.elapsed[i] >= HOVER_SECS {
                self.amt[i] = self.goal[i];
            }
            moving |= (self.amt[i] - self.goal[i]).abs() > 0.001;
        }
        moving
    }

    fn status_snap(&self) -> Status {
        if self.measuring() {
            let path = if self.snap.path.is_empty() { "Starting…".to_string() } else { self.snap.path.clone() };
            return Status { id: StatusId::Measuring, group: None, title: "Measuring".into(), detail: path, blurb: String::new() };
        }
        let hovered = self.hover.clone();
        let item = match &hovered {
            Some(Hit::Item(k)) => Some(k.clone()),
            Some(Hit::Group(_)) => None,
            None => self.selected.clone(),
        };
        if let Some(row) = item.as_deref().and_then(|k| self.snap.row(k)) {
            let blurb = if row.label == row.name { row.path.clone() } else { format!("{} · {}", row.name, row.path) };
            return Status {
                id: StatusId::Item(row.key.clone()),
                group: Some(row.group),
                title: row.label.clone(),
                detail: fmt_size(row.bytes),
                blurb,
            };
        }
        let group = match &hovered {
            Some(Hit::Group(g)) => Some(*g),
            _ => self.group,
        };
        if let Some(g) = group {
            return Status {
                id: StatusId::Group(g),
                group: Some(g),
                title: g.name().into(),
                detail: fmt_size(self.snap.groups[g.ix()]),
                blurb: self.group_blurb(g),
            };
        }
        Status::quiet()
    }

    fn group_blurb(&self, g: Group) -> String {
        let rows: Vec<_> = self.snap.group_rows(g).collect();
        let small = self.snap.small[g.ix()];
        let count = rows.len() as u64 + small.count;
        let noun = if count == 1 { "item" } else { "items" };
        match rows.first() {
            Some(top) => format!("{count} {noun}. {} is the largest at {}.", top.label, fmt_size(top.bytes)),
            None if count > 0 => format!("{count} small {noun}."),
            None => "Nothing found for this family.".into(),
        }
    }

    fn foot_snap(&self) -> Foot {
        if self.origin.is_none() || self.measuring() {
            return Foot::measuring();
        }
        let Some(g) = self.group else {
            return Foot {
                id: FootId::Groups,
                group: None,
                message: "Nothing here deletes files. Click a tool to see what you can clean up.".into(),
                command: String::new(),
                key: String::new(),
                copied: false,
            };
        };
        if let Some(row) = self.selected.as_deref().and_then(|k| self.snap.row(k)) {
            return Foot {
                id: FootId::Item(row.key.clone()),
                group: Some(g),
                message: row.note.clone(),
                command: row.command.clone(),
                key: row.key.clone(),
                copied: self.copied.as_deref() == Some(row.key.as_str()),
            };
        }
        Foot {
            id: FootId::Group(g),
            group: Some(g),
            message: "Click an item to get the command that cleans it up.".into(),
            command: String::new(),
            key: String::new(),
            copied: false,
        }
    }
}

impl Status {
    fn quiet() -> Self {
        Status {
            id: StatusId::Quiet,
            group: None,
            title: "Hover a tool to see what's using space.".into(),
            detail: String::new(),
            blurb: String::new(),
        }
    }
}

impl Foot {
    fn measuring() -> Self {
        Foot {
            id: FootId::Measuring,
            group: None,
            message: "Looking for caches, toolchains and build folders".into(),
            command: String::new(),
            key: String::new(),
            copied: false,
        }
    }
}

#[cfg(test)]
#[path = "tab_tests.rs"]
mod tests;
