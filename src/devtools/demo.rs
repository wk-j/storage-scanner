//! The developer tools tab on the demo disk: fixed rows revealed on a short clock.
//! Nothing here touches the filesystem; the commands name a made-up home folder.

use std::path::PathBuf;

use super::{project_label, remove_command, toolchain_short, Group, Row, Section, Snap};
use crate::demo::GB;

/// Seconds the demo measurement takes from the first frame of the tab.
pub const DURATION: f64 = 1.6;

const HOME: &str = "/Users/ada";

struct Demo {
    section: Section,
    group: Group,
    name: &'static str,
    /// `~`-relative, as the tab shows it.
    path: &'static str,
    gb: f64,
    /// `None`: `rm -rf` the path, or `remove` when the row holds several folders.
    run: Option<&'static str>,
    remove: &'static [&'static str],
    note: &'static str,
}

const ROWS: &[Demo] = &[
    Demo { section: Section::Tools, group: Group::Docker, name: "Docker Desktop disk", path: "~/Library/Containers/com.docker.docker", gb: 21.5, run: Some("docker system prune -a"), remove: &[], note: "Start Docker first. Removes stopped containers, unused networks and images no container uses. Volumes stay." },
    Demo { section: Section::Tools, group: Group::Homebrew, name: "Homebrew packages", path: "/opt/homebrew/Cellar", gb: 18.6, run: Some("brew autoremove"), remove: &[], note: "Removes dependencies nothing installed needs. brew leaves lists what you installed; brew uninstall <name> removes one." },
    Demo { section: Section::Tools, group: Group::Apple, name: "Xcode DerivedData", path: "~/Library/Developer/Xcode/DerivedData", gb: 6.4, run: None, remove: &[], note: "Xcode build output and indexes. Rebuilt on the next build." },
    Demo { section: Section::Tools, group: Group::Apple, name: "iOS simulators", path: "~/Library/Developer/CoreSimulator/Devices", gb: 4.8, run: Some("xcrun simctl delete unavailable"), remove: &[], note: "Removes simulators whose runtime is no longer installed." },
    Demo { section: Section::Tools, group: Group::Node, name: "pnpm store", path: "~/Library/pnpm/store", gb: 3.2, run: Some("pnpm store prune"), remove: &[], note: "Removes packages no project links to. Installed projects keep working." },
    Demo { section: Section::Tools, group: Group::Java, name: "Maven repository", path: "~/.m2/repository", gb: 2.1, run: None, remove: &[], note: "Downloaded dependencies. Maven fetches them again on the next build." },
    Demo { section: Section::Tools, group: Group::Rust, name: "Cargo registry", path: "~/.cargo/registry", gb: 1.9, run: None, remove: &[], note: "Downloaded crates and git dependencies. Cargo fetches them again on the next build." },
    Demo { section: Section::Tools, group: Group::Node, name: "npm cache", path: "~/.npm/_cacache", gb: 1.8, run: Some("npm cache clean --force"), remove: &[], note: "Downloaded packages. npm fetches them again when a project installs." },
    Demo { section: Section::Tools, group: Group::Other, name: "Go build cache", path: "~/Library/Caches/go-build", gb: 1.4, run: Some("go clean -cache"), remove: &[], note: "Compiled packages. go build rebuilds them." },
    Demo { section: Section::Tools, group: Group::Rust, name: "Rust toolchain stable-aarch64-apple-darwin", path: "~/.rustup/toolchains/stable-aarch64-apple-darwin", gb: 1.3, run: Some("rustup toolchain uninstall stable-aarch64-apple-darwin"), remove: &[], note: "Your default toolchain. Removing it leaves cargo without a compiler until you install another." },
    Demo { section: Section::Tools, group: Group::Java, name: "Gradle caches", path: "~/.gradle/caches", gb: 0.7, run: None, remove: &[], note: "Run gradle --stop first. Gradle downloads what the next build needs." },
    Demo { section: Section::Tools, group: Group::Homebrew, name: "Homebrew downloads", path: "~/Library/Caches/Homebrew", gb: 0.5, run: Some("brew cleanup --prune=all"), remove: &[], note: "Cached downloads and outdated versions. Installed packages stay." },
    Demo { section: Section::Tools, group: Group::Python, name: "pip cache", path: "~/Library/Caches/pip", gb: 0.03, run: Some("python3 -m pip cache purge"), remove: &[], note: "Downloaded wheels. pip fetches them again on the next install." },
    Demo { section: Section::Tools, group: Group::Other, name: "Composer cache", path: "~/Library/Caches/composer", gb: 0.02, run: Some("composer clear-cache"), remove: &[], note: "Downloaded PHP packages. Composer fetches them again." },
    Demo { section: Section::Projects, group: Group::Rust, name: "Rust target", path: "~/proj/storage/target", gb: 8.9, run: None, remove: &[], note: "Build output. cargo build recreates it." },
    Demo { section: Section::Projects, group: Group::Node, name: "node_modules", path: "~/code/web/node_modules", gb: 2.1, run: None, remove: &[], note: "Dependencies. The project's install command (npm, pnpm, yarn or bun) restores them." },
    Demo { section: Section::Projects, group: Group::Dotnet, name: ".NET bin and obj", path: "~/code/desktop/App (bin, obj)", gb: 0.8, run: None, remove: &["~/code/desktop/App/bin", "~/code/desktop/App/obj"], note: "Build output. dotnet build recreates it." },
    Demo { section: Section::Projects, group: Group::Python, name: "Python virtualenv", path: "~/code/api/.venv", gb: 0.7, run: None, remove: &[], note: "Recreate it with python3 -m venv (or uv sync) and reinstall packages." },
    Demo { section: Section::Projects, group: Group::Node, name: "Next.js build", path: "~/code/web/.next", gb: 0.6, run: None, remove: &[], note: "Build cache. next build recreates it." },
    Demo { section: Section::Projects, group: Group::Node, name: "node_modules", path: "~/code/docs/node_modules", gb: 0.04, run: None, remove: &[], note: "Dependencies. The project's install command (npm, pnpm, yarn or bun) restores them." },
    Demo { section: Section::Projects, group: Group::Dotnet, name: ".NET bin and obj", path: "~/code/desktop/Tests (bin, obj)", gb: 0.03, run: None, remove: &["~/code/desktop/Tests/bin", "~/code/desktop/Tests/obj"], note: "Build output. dotnet build recreates it." },
    Demo { section: Section::Projects, group: Group::Python, name: "Python virtualenv", path: "~/code/scripts/.venv", gb: 0.02, run: None, remove: &[], note: "Recreate it with python3 -m venv (or uv sync) and reinstall packages." },
];

fn abs(path: &str) -> PathBuf {
    match path.strip_prefix("~/") {
        Some(rest) => PathBuf::from(HOME).join(rest),
        None => PathBuf::from(path),
    }
}

fn row(d: &Demo) -> Row {
    let command = match d.run {
        Some(c) => c.to_string(),
        None if d.remove.is_empty() => remove_command(&[abs(d.path)], false),
        None => remove_command(&d.remove.iter().map(|p| abs(p)).collect::<Vec<_>>(), false),
    };
    // Build output is labelled by its project: the folder above the first one it removes.
    let label = match d.section {
        // As the real catalog labels them: "Toolchain stable".
        Section::Tools => match d.name.strip_prefix("Rust toolchain ") {
            Some(name) => format!("Toolchain {}", toolchain_short(name)),
            None => d.name.to_string(),
        },
        Section::Projects => {
            let first = d.remove.first().copied().unwrap_or(d.path);
            project_label(abs(first).parent().unwrap_or(&abs(first)))
        }
    };
    Row {
        key: format!("{}:{}", if d.section == Section::Tools { "tool" } else { "proj" }, d.path),
        section: d.section,
        group: d.group,
        name: d.name.into(),
        label,
        path: d.path.into(),
        bytes: (d.gb * GB) as u64,
        command,
        note: d.note.into(),
    }
}

/// The tab `age` seconds after it started measuring: rows land one by one, then it is done.
pub fn snap(age: f64) -> Snap {
    let progress = (age / DURATION).clamp(0.0, 1.0);
    let landed = ((ROWS.len() as f64) * progress).floor() as usize;
    let all: Vec<Row> = ROWS[..landed.min(ROWS.len())].iter().map(row).collect();
    let done = progress >= 1.0;
    let path = if done { String::new() } else { ROWS[landed.min(ROWS.len() - 1)].path.to_string() };
    Snap::from_rows(&all, path, done)
}
