use super::*;
use crate::demo::CAPACITY;

const FRAME: f64 = 1.0 / 60.0;

/// Run frames until `until` holds or `limit` seconds pass; returns the clock it stopped at.
fn run(tab: &mut DevTab, mut t: f64, limit: f64, until: impl Fn(&DevTab) -> bool) -> f64 {
    let end = t + limit;
    while t < end && !until(tab) {
        t += FRAME;
        tab.step(t, FRAME, FRAME as f32, CAPACITY);
    }
    t
}

fn finished() -> (DevTab, f64) {
    let mut tab = DevTab::new(true);
    tab.open();
    tab.step(0.0, 0.0, 0.0, CAPACITY);
    let t = run(&mut tab, 0.0, 10.0, |d| d.snap.done);
    // Let the sizes and the bar's whole settle.
    let t = run(&mut tab, t, 5.0, |d| d.cap == d.snap.total as f64);
    (tab, t)
}

#[test]
fn opening_measures_once_and_the_bar_zooms_to_the_developer_total_when_done() {
    let mut tab = DevTab::new(true);
    assert!(!tab.step(0.0, 0.0, 0.0, CAPACITY) || tab.origin.is_none(), "nothing is measured before the tab opens");
    assert!(tab.origin.is_none());

    tab.open();
    assert!(tab.step(5.0, FRAME, FRAME as f32, CAPACITY), "measuring keeps frames coming");
    assert_eq!(tab.origin, Some(5.0));
    assert!(tab.measuring());
    let t = run(&mut tab, 5.0, 0.8, |_| false);
    assert!(tab.total() > 0.0, "the total counts up while measuring");
    assert!((tab.cap - CAPACITY).abs() < CAPACITY * 0.01, "while measuring the bar is on the disk's scale");

    let t = run(&mut tab, t, 10.0, |d| d.snap.done);
    let done_at = t;
    let t = run(&mut tab, t, 5.0, |d| d.cap == d.snap.total as f64);
    assert_eq!(tab.cap, tab.snap.total as f64, "done: the families fill the bar");
    assert!(t - done_at < 2.0, "the zoom takes {:.2}s", t - done_at);
    let mut rest = false;
    for k in 1..300 {
        rest = !tab.step(t + k as f64 * FRAME, FRAME, FRAME as f32, CAPACITY);
        if rest {
            break;
        }
    }
    assert!(rest, "a finished, settled tab stops asking for frames");
    for g in Group::ALL {
        assert!((tab.live[g.ix()] - tab.snap.groups[g.ix()] as f64).abs() < 2.0e7, "{g:?} settles on its measured size");
    }

    tab.open();
    assert!(!tab.request, "coming back keeps the measurement");
}

#[test]
fn the_frame_that_finishes_redraws() {
    let mut tab = DevTab::new(true);
    tab.open();
    let mut t = 0.0;
    tab.step(t, 0.0, 0.0, CAPACITY);
    let mut finishing = None;
    while t < 10.0 {
        t += FRAME;
        let was = tab.snap.done;
        let redraw = tab.step(t, FRAME, FRAME as f32, CAPACITY);
        if tab.snap.done && !was {
            finishing = Some(redraw);
            break;
        }
    }
    assert_eq!(finishing, Some(true), "the finishing frame must redraw, or the last measuring frame stays on screen");
}

#[test]
fn families_open_only_after_measuring_and_items_hold_still() {
    let mut tab = DevTab::new(true);
    tab.open();
    tab.step(0.0, 0.0, 0.0, CAPACITY);
    run(&mut tab, 0.0, 0.5, |_| false);
    tab.press_group(Group::Rust);
    assert_eq!(tab.group, None, "a family can't be opened while items are still arriving");

    let (mut tab, t) = finished();
    tab.press_group(Group::Rust);
    assert_eq!(tab.group, Some(Group::Rust));
    let before: Vec<String> = tab.snap.group_rows(Group::Rust).map(|r| r.key.clone()).collect();
    assert!(!before.is_empty());
    run(&mut tab, t, 1.0, |_| false);
    let after: Vec<String> = tab.snap.group_rows(Group::Rust).map(|r| r.key.clone()).collect();
    assert_eq!(before, after);
    let sizes: Vec<u64> = tab.snap.group_rows(Group::Rust).map(|r| r.bytes).collect();
    assert!(sizes.windows(2).all(|w| w[0] >= w[1]), "largest first: {sizes:?}");
}

#[test]
fn an_item_shows_its_command_in_the_footer_and_back_closes_the_family() {
    let (mut tab, t) = finished();
    tab.press_group(Group::Node);
    let key = tab.snap.group_rows(Group::Node).next().unwrap().key.clone();
    tab.press_item("tool:not-here");
    assert_eq!(tab.selected, None, "only a measured item can be selected");
    tab.press_item(&key);
    let t = run(&mut tab, t, 0.5, |_| false);
    let foot = &tab.footer_fade.base;
    assert_eq!(foot.id, FootId::Item(key.clone()));
    let row = tab.snap.row(&key).unwrap();
    assert_eq!(foot.command, row.command);
    assert_eq!(foot.message, row.note);
    assert_eq!(tab.status_fade.base.id, StatusId::Item(key.clone()));

    tab.note_copied(&key);
    tab.step(t + 30.0, FRAME, FRAME as f32, CAPACITY);
    assert!(tab.footer_fade.base.copied, "a stale clock doesn't skip the Copied label");
    assert!(tab.footer_fade.incoming.is_none(), "Copied changes in place, without a crossfade");
    tab.step(t + 30.0 + COPIED_SECS, FRAME, FRAME as f32, CAPACITY);
    assert!(tab.copied.is_none());

    tab.note_copied(&key);
    let other = tab.snap.group_rows(Group::Node).nth(1).unwrap().key.clone();
    tab.press_item(&other);
    assert!(tab.copied.is_none(), "a new item shows Copy, not Copied");

    tab.press_back();
    assert_eq!((tab.group, tab.selected.clone()), (None, None));
    run(&mut tab, t + 40.0, 0.5, |_| false);
    assert_eq!(tab.footer_fade.base.id, FootId::Groups);
}

#[test]
fn hover_holds_across_a_short_gap_and_brings_its_family_forward() {
    let (mut tab, t) = finished();
    tab.note_hit(Hit::Group(Group::Docker), true);
    let t = run(&mut tab, t, 0.4, |_| false);
    assert_eq!(tab.emphasis(), Some(Group::Docker));
    assert!((tab.amt[Group::Docker.ix()] - 1.0).abs() < 1e-3);
    assert_eq!(tab.status_fade.base.id, StatusId::Group(Group::Docker));

    // Leave and enter the next cell within the grace: no quiet line in between.
    tab.note_hit(Hit::Group(Group::Docker), false);
    tab.step(t + FRAME, FRAME, FRAME as f32, CAPACITY);
    assert!(tab.hover.is_some());
    tab.note_hit(Hit::Group(Group::Node), true);
    let t = run(&mut tab, t + FRAME, 0.4, |_| false);
    assert_eq!(tab.status_fade.base.id, StatusId::Group(Group::Node));

    tab.note_hit(Hit::Group(Group::Node), false);
    run(&mut tab, t, 0.5, |_| false);
    assert_eq!(tab.hover, None);
    assert_eq!(tab.status_fade.base.id, StatusId::Quiet);
    assert!(tab.amt.iter().all(|a| *a < 1e-3));
}

#[test]
fn rescan_empties_the_bar_and_closes_the_family() {
    let (mut tab, t) = finished();
    tab.press_group(Group::Java);
    let key = tab.snap.group_rows(Group::Java).next().unwrap().key.clone();
    tab.press_item(&key);
    tab.rescan();
    assert_eq!((tab.group, tab.selected.clone()), (None, None));
    assert_eq!(tab.total(), 0.0);
    tab.step(t + 1.0, FRAME, FRAME as f32, CAPACITY);
    assert!(tab.measuring());
    assert_eq!(tab.origin, Some(t + 1.0));
}

