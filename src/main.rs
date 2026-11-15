//! Storage Scanner. A disk map in the shape of a glowing cell bar.
//!
//! `cargo run --release` opens the window. On Linux that is the deterministic demo (nothing is
//! deleted). On macOS and Windows it scans the home folder; pass `--demo` for the same fake disk.
//! `--real` scans for real on Linux too, and still asks before emptying `~/.Trash`, the only
//! thing it deletes. The Developer tools tab only measures and shows commands to run by hand.
//!
//! Headless, same renderer:
//! `cargo run --release -- --demo --snap out/idle-apps.png --scene idle --at 0.4`

mod bar;
mod demo;
mod devtools;
mod fire;
#[cfg(test)]
mod host_tests;
mod icons;
mod model;
mod scan;
mod ui;

use model::{State, CARD_H, CARD_W};
use rune::AppSpec;

/// The Dock and app-switcher icon, drawn by `scripts/icon.py`. Homebrew installs a bare binary
/// with no app bundle, so the app sets its own icon while it runs.
const ICON: &[u8] = include_bytes!("../assets/icon.png");

fn main() {
    if let Err(e) = rune::set_app_icon(ICON) {
        eprintln!("storage-scanner: {e}");
    }
    rune::App::new(AppSpec {
        scenes: &[
            ("scanning", State::scene_scanning),
            ("idle", State::scene_idle),
            ("clearing", State::scene_clearing),
            ("emptied", State::scene_emptied),
            ("developer", State::scene_developer),
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
    }).compositor(fire::Gfx::default()).run();
}

#[cfg(test)]
mod tests {
    /// The icon is a square 1024 px RGBA PNG, the size macOS draws Dock icons from.
    #[test]
    fn the_icon_is_a_square_png() {
        let icon = super::ICON;
        assert_eq!(&icon[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(&icon[12..16], b"IHDR");
        let width = u32::from_be_bytes(icon[16..20].try_into().unwrap());
        let height = u32::from_be_bytes(icon[20..24].try_into().unwrap());
        assert_eq!((width, height), (1024, 1024));
        assert_eq!(icon[25], 6, "RGBA, so the tile's corners stay transparent");
    }
}
