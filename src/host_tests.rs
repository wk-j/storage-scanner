//! The app driven the way a native window drives it: a frame runs after input, and after that
//! only while Rune's own motion runs or a wake is due. Nothing else draws, so whatever `poll`
//! animates (crossfades, hover fades, the scan) must keep asking for frames, or a resting window
//! freezes it part way, both footer layers showing at once.

use rune::Env;
use rune::runtime::Runtime;

use crate::devtools::Group;
use crate::devtools::tab::{DevTab, FootId};
use crate::model::{self, State, CARD_H, CARD_W};

const FRAME: f64 = 1.0 / 60.0;

fn developer_window() -> Runtime<State> {
    let mut s = State::new();
    s.demo = true;
    s.dev = DevTab::new(true);
    s.scene_developer();
    let mut rt = Runtime::new(s, crate::ui::view, (CARD_W, CARD_H), Env::new(true, 2.0));
    rt.poll = Some(model::poll);
    rt
}

/// Draws frames until a native window would stop: nothing animating and no wake due by the
/// next display frame. Returns how many it drew.
fn until_rest(rt: &mut Runtime<State>) -> usize {
    let mut frames = 0;
    loop {
        rt.frame(FRAME);
        frames += 1;
        let due = rt.wake_deadline().is_some_and(|at| at <= rt.clock + FRAME);
        if !rt.animating && !due {
            return frames;
        }
        assert!(frames < 600, "the window never rests");
    }
}

/// A click with the pointer left where it pressed: no input follows it.
fn click(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("nothing keyed {key} on screen"));
    rt.pointer_move(x, y);
    until_rest(rt);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    until_rest(rt);
}

#[test]
fn a_clicked_item_shows_its_command_without_moving_the_pointer() {
    let mut rt = developer_window();
    until_rest(&mut rt);
    click(&mut rt, "dev-group-rust");
    assert_eq!(rt.state.dev.footer_fade.base.id, FootId::Group(Group::Rust));
    assert!(rt.state.dev.footer_fade.incoming.is_none(), "the family footer froze part way");

    let item = rt.state.dev.snap.group_rows(Group::Rust).next().expect("a Rust item").key.clone();
    click(&mut rt, &format!("dev-item-{item}"));
    let fade = &rt.state.dev.footer_fade;
    assert!(fade.incoming.is_none(), "the footer froze part way, both commands showing");
    assert_eq!(fade.base.id, FootId::Item(item.clone()));
    assert_eq!(rt.state.dev.status_fade.incoming.as_ref().map(|s| s.id.clone()), None, "the status line froze part way");
}

#[test]
fn switching_tabs_starts_measuring_without_moving_the_pointer() {
    let mut s = State::new();
    s.demo = true;
    s.dev = DevTab::new(true);
    s.scene_idle();
    let mut rt = Runtime::new(s, crate::ui::view, (CARD_W, CARD_H), Env::new(true, 2.0));
    rt.poll = Some(model::poll);
    until_rest(&mut rt);
    click(&mut rt, "tab-developer");
    assert!(rt.state.dev.snap.done, "the demo measurement ran to the end on its own");
    assert!(rt.state.dev.footer_fade.incoming.is_none());
    assert_eq!(rt.state.dev.footer_fade.base.id, FootId::Groups);
}
