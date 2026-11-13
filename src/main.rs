//! Storage Scanner. A disk map in the shape of a glowing cell bar.
//!
//! `cargo run --release` opens the window. On Linux that is the deterministic demo (nothing is
//! deleted). On macOS and Windows it scans the home folder; pass `--demo` for the same fake disk.
//! `--real` scans for real on Linux too, and still asks before deleting `~/Library/Caches` or
//! `~/.Trash`.
//!
//! Headless, same renderer:
//! `cargo run --release -- --demo --snap out/idle-apps.png --scene idle --at 0.4`

mod bar;
mod demo;
mod icons;
mod model;
mod scan;
mod ui;

use model::{State, CARD_H, CARD_W};
use rune::AppSpec;

fn main() {
    rune::run(AppSpec {
        scenes: &[
            ("scanning", State::scene_scanning),
            ("idle", State::scene_idle),
            ("clearing", State::scene_clearing),
            ("emptied", State::scene_emptied),
        ],
        fonts: &[
            include_bytes!("../assets/Inter-Regular.ttf"),
            include_bytes!("../assets/Inter-Medium.ttf"),
            include_bytes!("../assets/Inter-SemiBold.ttf"),
            include_bytes!("../assets/Inter-Bold.ttf"),
            include_bytes!("../assets/IBMPlexMono-Regular.ttf"),
        ],
        families: ("Inter", "IBM Plex Mono"),
        min_size: Some((CARD_W, CARD_H)),
        setup: Some(model::setup),
        poll: Some(model::poll),
        ..AppSpec::new("Storage Scanner", (CARD_W, CARD_H), State::new, ui::view)
    });
}
