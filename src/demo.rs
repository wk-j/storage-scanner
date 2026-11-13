//! The design's disk, revealed on a fixed clock. Nothing here touches the filesystem.
//!
//! Totals match the reference frames: 512 GB disk, 357.8 GB used, split across the nine categories.

use crate::scan::{Cat, N};

pub const GB: f64 = 1_000_000_000.0;

/// Apps, Developer, Photos, Movies, Documents, Music, System data, Caches, Trash.
pub const TOTALS: [f64; N] = [
    52.6 * GB,
    36.3 * GB,
    58.6 * GB,
    44.1 * GB,
    41.7 * GB,
    12.1 * GB,
    34.2 * GB,
    46.8 * GB,
    31.4 * GB,
];

pub const CAPACITY: f64 = 512.0 * GB;
pub const SCAN_END: f64 = 7.2;
pub const CLEAR_DUR: f64 = 1.45;

/// `(start, end)` of each category's reveal, in seconds. Later categories finish later, so the
/// bar grows from the left the way the reference scan does.
const WINDOWS: [(f64, f64); N] = [
    (0.15, 2.50),
    (0.70, 3.50),
    (1.15, 4.15),
    (1.55, 4.55),
    (1.95, 4.95),
    (2.30, 4.70),
    (2.10, 5.35),
    (2.70, 5.75),
    (3.50, 7.20),
];

const SCAN_PATHS: [&[&str]; N] = [
    &[
        "~/Applications/Penrose Studio.app",
        "~/Applications/Figma.app",
        "~/Applications/Slack.app",
        "/Applications/Xcode.app",
    ],
    &[
        "~/code/web/node_modules/react",
        "~/Library/Developer/Xcode/DerivedData",
        "~/.cargo/registry",
        "~/proj/storage/target/release",
    ],
    &["~/Pictures/Photos Library.photoslibrary", "~/Pictures/2026/IMG_4402.heic"],
    &["~/Movies/Trip/iceland.mp4", "~/Movies/Screen recording 2026-08-14.mov"],
    &["~/Documents/Archive/taxes-2025.pdf", "~/Documents/Notes"],
    &["~/Music/Library.musiclibrary", "~/Music/Albums"],
    &["~/Library/Application Support", "~/Library/Containers"],
    &["~/Library/Caches/pnpm", "~/Library/Caches/com.apple.Safari", "~/Library/Caches/Homebrew"],
    &[
        "~/.Trash/Screen recording 2026-08-14.mov",
        "~/.Trash/atlas-backup.zip",
        "~/.Trash/Old Downloads",
    ],
];

const CLEAR_PATHS: &[&str] = &[
    "~/Library/Caches/pnpm",
    "~/Library/Caches/com.apple.Safari",
    "~/Library/Caches/Homebrew",
    "~/Library/Caches/node-gyp",
];

const TRASH_PATHS: &[&str] = &[
    "~/.Trash/atlas-backup.zip",
    "~/.Trash/Screen recording 2026-08-14.mov",
    "~/.Trash/Old Downloads",
];

pub fn ease(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn reveal(t: f64, start: f64, end: f64) -> f64 {
    if t <= start {
        0.0
    } else if t >= end {
        1.0
    } else {
        ease((t - start) / (end - start))
    }
}

/// Category bytes at `scan_t`, with optional clear / empty animations and a settled end state.
pub fn bytes(scan_t: f64, clear_age: Option<f64>, trash_age: Option<f64>, settled: bool) -> [f64; N] {
    let mut out = [0.0; N];
    for i in 0..N {
        let (a, b) = WINDOWS[i];
        out[i] = TOTALS[i] * reveal(scan_t, a, b);
    }
    if let Some(age) = clear_age {
        out[Cat::Caches.ix()] = TOTALS[Cat::Caches.ix()] * (1.0 - ease(age / CLEAR_DUR));
    }
    if let Some(age) = trash_age {
        out[Cat::Trash.ix()] = TOTALS[Cat::Trash.ix()] * (1.0 - ease(age / CLEAR_DUR));
    }
    if settled {
        out[Cat::Caches.ix()] = 0.0;
        out[Cat::Trash.ix()] = 0.0;
    }
    out
}

pub fn scanning(scan_t: f64) -> bool {
    scan_t < SCAN_END
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activity {
    Calculating,
    Clearing,
    Emptying,
}

pub fn activity(scan_t: f64, clear_age: Option<f64>, trash_age: Option<f64>) -> Option<(Activity, &'static str)> {
    if let Some(age) = clear_age.filter(|a| *a < CLEAR_DUR) {
        return Some((Activity::Clearing, cycle(CLEAR_PATHS, age)));
    }
    if let Some(age) = trash_age.filter(|a| *a < CLEAR_DUR) {
        return Some((Activity::Emptying, cycle(TRASH_PATHS, age)));
    }
    if scanning(scan_t) {
        let cat = active_cat(scan_t);
        return Some((Activity::Calculating, cycle(SCAN_PATHS[cat], scan_t)));
    }
    None
}

fn cycle(paths: &[&'static str], t: f64) -> &'static str {
    paths[((t * 2.4) as usize) % paths.len()]
}

fn active_cat(t: f64) -> usize {
    let mut best = 0;
    for i in 0..N {
        if t + 0.05 >= WINDOWS[i].0 {
            best = i;
        }
    }
    best
}

pub fn blurb(cat: Cat) -> &'static str {
    match cat {
        Cat::Apps => "142 apps. Penrose Studio is the largest at 12.4 GB.",
        Cat::Developer => "Dependencies and build output. node_modules is the largest share.",
        Cat::Photos => "14,870 photos and 402 videos, all kept on this device.",
        Cat::Movies => "86 videos. Screen recordings take the most room.",
        Cat::Documents => "2,416 files across documents, archives and PDFs.",
        Cat::Music => "1,902 songs, kept on this device.",
        Cat::System => "macOS and support files this view doesn't split further.",
        Cat::Caches => "Temporary files. Apps rebuild them when they need them.",
        Cat::Trash => "Things you've already deleted, still taking space.",
    }
}

/// One decimal gigabyte, matching the reference ("52.6", "0.0").
pub fn fmt_gb(bytes: f64) -> String {
    let tenths = (bytes / (GB / 10.0)).round() as i64;
    let tenths = tenths.max(0);
    format!("{}.{}", tenths / 10, tenths % 10)
}

pub fn fmt_cap(bytes: f64) -> String {
    let gb = bytes / GB;
    if (gb - gb.round()).abs() < 0.05 {
        format!("{:.0}", gb.round())
    } else {
        fmt_gb(bytes)
    }
}

pub fn sum(bytes: &[f64; N]) -> f64 {
    bytes.iter().sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn design_totals_are_357_8_of_512() {
        let all: f64 = TOTALS.iter().sum();
        assert_eq!(fmt_gb(all), "357.8");
        assert_eq!(fmt_cap(CAPACITY), "512");
        for (i, v) in TOTALS.iter().enumerate() {
            assert_eq!(fmt_gb(*v).chars().next().is_some(), true, "{i}");
        }
        assert_eq!(fmt_gb(TOTALS[Cat::Apps.ix()]), "52.6");
        assert_eq!(fmt_gb(TOTALS[Cat::Photos.ix()]), "58.6");
        assert_eq!(fmt_gb(TOTALS[Cat::Caches.ix()]), "46.8");
        assert_eq!(fmt_gb(TOTALS[Cat::Trash.ix()]), "31.4");
        assert_eq!(fmt_gb(TOTALS[Cat::System.ix()]), "34.2");
    }

    #[test]
    fn the_scan_grows_and_lands_on_the_design_totals() {
        let start = bytes(0.0, None, None, false);
        assert!(start.iter().all(|b| *b == 0.0));
        let mut prev = 0.0;
        for step in 1..80 {
            let t = step as f64 * 0.1;
            let now = sum(&bytes(t, None, None, false));
            assert!(now + 1.0 >= prev, "usage should not go backwards at {t}");
            prev = now;
        }
        let done = bytes(SCAN_END, None, None, false);
        for (i, (got, want)) in done.iter().zip(TOTALS).enumerate() {
            assert!((got - want).abs() < 1.0, "cat {i}: {got} != {want}");
        }
        let mid = bytes(5.5, None, None, false);
        assert!(mid[Cat::Trash.ix()] < TOTALS[Cat::Trash.ix()] * 0.85);
        assert!(mid[Cat::Apps.ix()] > TOTALS[Cat::Apps.ix()] * 0.99);
        let clearing = bytes(SCAN_END, Some(CLEAR_DUR * 0.5), None, false);
        assert!(clearing[Cat::Caches.ix()] < TOTALS[Cat::Caches.ix()] * 0.6);
        let settled = bytes(SCAN_END, None, None, true);
        assert_eq!(settled[Cat::Caches.ix()], 0.0);
        assert_eq!(settled[Cat::Trash.ix()], 0.0);
        assert_eq!(fmt_gb(sum(&settled)), "279.6");
    }

    #[test]
    fn status_paths_match_the_reference_copy() {
        let (kind, path) = activity(0.4, None, None).unwrap();
        assert_eq!(kind, Activity::Calculating);
        assert_eq!(path, "~/Applications/Penrose Studio.app");
        let (kind, path) = activity(5.6, None, None).unwrap();
        assert_eq!(kind, Activity::Calculating);
        assert!(path.contains(".Trash"), "{path}");
        let (kind, path) = activity(SCAN_END, Some(0.4), None).unwrap();
        assert_eq!(kind, Activity::Clearing);
        assert!(path.contains("Library/Caches"), "{path}");
        let (kind, path) = activity(SCAN_END, Some(CLEAR_DUR), Some(0.3)).unwrap();
        assert_eq!(kind, Activity::Emptying);
        assert!(path.contains(".Trash"), "{path}");
        assert!(activity(SCAN_END, None, None).is_none());
    }
}
