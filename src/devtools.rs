//! Developer tools tab: what package managers, toolchains and project build output hold on disk,
//! and the command that cleans each one up.
//!
//! Nothing here deletes files. The app measures and shows commands; the person copies one and
//! runs it in a terminal. Commands use absolute, single-quoted paths so they paste the same way
//! into sh, zsh, bash and fish.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::scan::{home_dir, pretty};

mod catalog;
pub mod demo;
pub mod tab;
pub mod view;

use catalog::{Cmd, TOOLS};

/// Items smaller than this fold into their group's "smaller items" cell.
pub const MIN_ROW: u64 = 50_000_000;

/// Tool families, one per bar segment and legend cell. The order is the bar's left-to-right
/// order and each slot takes that slot's colour from the bar (Docker blue, Node green, Rust
/// orange, Java red, .NET purple and so on), so a family keeps its colour across scans.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Group {
    Docker = 0,
    Node = 1,
    Apple = 2,
    Dotnet = 3,
    Python = 4,
    Homebrew = 5,
    Other = 6,
    Rust = 7,
    Java = 8,
}

pub const GROUPS: usize = 9;

impl Group {
    pub const ALL: [Group; GROUPS] = [
        Group::Docker,
        Group::Node,
        Group::Apple,
        Group::Dotnet,
        Group::Python,
        Group::Homebrew,
        Group::Other,
        Group::Rust,
        Group::Java,
    ];

    pub fn ix(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            Group::Docker => "Docker",
            Group::Node => "Node",
            Group::Apple => "Xcode",
            Group::Dotnet => ".NET",
            Group::Python => "Python",
            Group::Homebrew => "Homebrew",
            Group::Other => "Other",
            Group::Rust => "Rust",
            Group::Java => "Java",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Group::Docker => "docker",
            Group::Node => "node",
            Group::Apple => "xcode",
            Group::Dotnet => "dotnet",
            Group::Python => "python",
            Group::Homebrew => "homebrew",
            Group::Other => "other",
            Group::Rust => "rust",
            Group::Java => "java",
        }
    }

    /// Icon and legend colour: the overview's colour for the same bar slot.
    pub fn ui(self) -> u32 {
        crate::scan::Cat::ALL[self.ix()].ui()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    /// Global caches, stores and toolchains.
    Tools,
    /// Build output and dependencies inside project folders.
    Projects,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// Stable element key: the tool id, or the folder for project rows.
    pub key: String,
    pub section: Section,
    pub group: Group,
    /// What it is: "npm cache", "node_modules", "Rust target".
    pub name: String,
    /// Short enough for a legend cell: the tool's name, or the project for build output.
    pub label: String,
    /// Where it lives, `~`-relative for display.
    pub path: String,
    pub bytes: u64,
    /// What to paste into a terminal. Never run by the app.
    pub command: String,
    /// What happens after the command runs, and anything to do first.
    pub note: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Small {
    pub count: u64,
    pub bytes: u64,
}

#[derive(Clone, Debug, Default)]
pub struct Snap {
    /// Items at or above [`MIN_ROW`], largest first.
    pub rows: Vec<Row>,
    /// Every item's bytes, small ones too, per [`Group`].
    pub groups: [u64; GROUPS],
    /// Count and bytes of the items below [`MIN_ROW`], per [`Group`].
    pub small: [Small; GROUPS],
    pub total: u64,
    /// What is being measured now, `~`-relative.
    pub path: String,
    pub done: bool,
}

impl Snap {
    /// Build a snapshot from every measured item.
    pub fn from_rows(all: &[Row], path: String, done: bool) -> Snap {
        let mut groups = [0u64; GROUPS];
        let mut small = [Small::default(); GROUPS];
        let mut rows: Vec<Row> = Vec::new();
        for row in all {
            groups[row.group.ix()] += row.bytes;
            if row.bytes >= MIN_ROW {
                rows.push(row.clone());
            } else if row.bytes > 0 {
                let s = &mut small[row.group.ix()];
                s.count += 1;
                s.bytes += row.bytes;
            }
        }
        rows.sort_by(|a, b| b.bytes.cmp(&a.bytes).then(a.key.cmp(&b.key)));
        let total = groups.iter().sum();
        Snap { rows, groups, small, total, path, done }
    }

    pub fn group_rows(&self, group: Group) -> impl Iterator<Item = &Row> {
        self.rows.iter().filter(move |r| r.group == group)
    }

    pub fn row(&self, key: &str) -> Option<&Row> {
        self.rows.iter().find(|r| r.key == key)
    }
}

/// A legend label for build output: the project folder's last two names, "e-saraban/web".
pub fn project_label(dir: &Path) -> String {
    let names: Vec<&str> = dir.components().rev().take(2).filter_map(|c| c.as_os_str().to_str()).collect();
    names.into_iter().rev().collect::<Vec<_>>().join("/")
}

pub struct Shared {
    epoch: AtomicU64,
    pub snap: Mutex<Snap>,
}

impl Shared {
    pub fn new() -> Self {
        Shared { epoch: AtomicU64::new(0), snap: Mutex::new(Snap::default()) }
    }
}

/// Start (or restart) the measurement on a background thread. An earlier run stops at its next check.
pub fn spawn(shared: Arc<Shared>, waker: rune::Waker) {
    let epoch = shared.epoch.fetch_add(1, Ordering::Relaxed) + 1;
    *shared.snap.lock().expect("dev lock") = Snap { path: "Starting…".into(), ..Snap::default() };
    waker();
    let _ = std::thread::Builder::new().name("dev-tools-scan".into()).spawn(move || job(shared, waker, epoch));
}

/// "21.5 GB" from a gigabyte up, whole megabytes below it.
pub fn fmt_size(bytes: u64) -> String {
    // 999.6 MB reads as "1.0 GB", not "1000 MB".
    if bytes >= 999_500_000 {
        format!("{} GB", crate::demo::fmt_gb(bytes as f64))
    } else {
        format!("{} MB", (bytes as f64 / 1_000_000.0).round() as u64)
    }
}

/// Single-quote a path for any common shell. A `'` inside closes the quote, adds an escaped one, and reopens.
pub fn quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "'\\''"))
}

pub fn remove_command(paths: &[PathBuf], sudo: bool) -> String {
    let list: Vec<String> = paths.iter().map(|p| quote(p)).collect();
    format!("{}rm -rf {}", if sudo { "sudo " } else { "" }, list.join(" "))
}

// ---------------------------------------------------------------------------------------------
// Catalog: the tool list is in `catalog.rs`; this adds what depends on the machine.

/// One measurable item: what to sum, and the row it becomes.
pub struct Item {
    pub paths: Vec<PathBuf>,
    pub row: Row,
}

fn resolve(home: &Path, p: &str) -> PathBuf {
    match p.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None => PathBuf::from(p),
    }
}

/// Every tool-level item that exists on this machine. `system` adds what lives outside `home`
/// or comes from the environment (Homebrew, system .NET SDKs, `CARGO_TARGET_DIR`,
/// `RUSTUP_HOME`); tests turn it off so a fixture home is all they measure.
pub fn catalog(home: &Path, system: bool) -> Vec<Item> {
    let mut out = Vec::new();
    if let Some(dir) = cargo_target_dir(home, system) {
        let paths = vec![dir.clone()];
        out.push(Item {
            row: Row {
                key: "cargo-target-dir".into(),
                section: Section::Tools,
                group: Group::Rust,
                name: "Cargo shared target folder".into(),
                label: "Cargo shared target".into(),
                path: pretty(&dir, home),
                bytes: 0,
                command: remove_command(&paths, false),
                note: "Build output for every Rust project that builds here. Each rebuilds from scratch on its next cargo build.".into(),
            },
            paths,
        });
    }
    for tool in TOOLS {
        let paths: Vec<PathBuf> = tool.paths.iter()
            .filter(|p| system || p.starts_with("~/"))
            .map(|p| resolve(home, p))
            .filter(|p| exists_dir(p))
            .collect();
        if paths.is_empty() {
            continue;
        }
        let command = match tool.cmd {
            Cmd::Run(c) => c.to_string(),
            Cmd::Remove => remove_command(&paths, false),
        };
        let shown: Vec<String> = paths.iter().map(|p| pretty(p, home)).collect();
        out.push(Item {
            row: Row {
                key: tool.key.into(),
                section: Section::Tools,
                group: tool.group,
                name: tool.name.into(),
                label: tool.name.into(),
                path: shown.join(", "),
                bytes: 0,
                command,
                note: tool.note.into(),
            },
            paths,
        });
    }
    out.extend(rustup_toolchains(home, system));
    out.extend(dotnet_sdks(home, system));
    out
}

fn exists_dir(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok_and(|m| m.is_dir())
}

/// `CARGO_TARGET_DIR`, or `build.target-dir` in `~/.cargo/config.toml`: one folder every project shares.
fn cargo_target_dir(home: &Path, system: bool) -> Option<PathBuf> {
    let from_env = std::env::var_os("CARGO_TARGET_DIR").filter(|_| system).map(PathBuf::from);
    let from_config = || {
        let text = fs::read_to_string(home.join(".cargo/config.toml")).ok()?;
        config_target_dir(&text).map(|v| resolve(home, &v))
    };
    let dir = from_env.or_else(from_config)?;
    // A relative target-dir is per project, not shared. Never offer to remove home itself.
    (dir.is_absolute() && dir != home && exists_dir(&dir)).then_some(dir)
}

/// `target-dir = "…"` from a cargo config. Good enough for the one-line form people write.
pub fn config_target_dir(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let line = line.trim();
        let rest = line.strip_prefix("target-dir")?.trim_start().strip_prefix('=')?.trim();
        let v = rest.trim_matches('"').trim_matches('\'');
        (!v.is_empty()).then(|| v.to_string())
    })
}

fn rustup_toolchains(home: &Path, system: bool) -> Vec<Item> {
    let root = std::env::var_os("RUSTUP_HOME").filter(|_| system).map(PathBuf::from).unwrap_or_else(|| home.join(".rustup"));
    let default = fs::read_to_string(root.join("settings.toml")).ok().and_then(|t| {
        t.lines().find_map(|l| l.trim().strip_prefix("default_toolchain").map(|r| r.trim_start_matches([' ', '=']).trim_matches('"').to_string()))
    });
    let Ok(rd) = fs::read_dir(root.join("toolchains")) else { return Vec::new() };
    let mut out = Vec::new();
    for ent in rd.flatten() {
        let path = ent.path();
        if !exists_dir(&path) {
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()).map(str::to_string) else { continue };
        let is_default = default.as_deref() == Some(name.as_str());
        let note = if is_default {
            "Your default toolchain. Removing it leaves cargo without a compiler until you install another.".to_string()
        } else {
            "An installed Rust toolchain. rustup toolchain list shows which one is the default.".to_string()
        };
        out.push(Item {
            row: Row {
                key: format!("rustup:{name}"),
                section: Section::Tools,
                group: Group::Rust,
                name: format!("Rust toolchain {name}"),
                label: format!("Toolchain {}", toolchain_short(&name)),
                path: pretty(&path, home),
                bytes: 0,
                command: format!("rustup toolchain uninstall {name}"),
                note,
            },
            paths: vec![path],
        });
    }
    out
}

/// "stable" from "stable-aarch64-apple-darwin": the host triple is the same for every toolchain.
pub(crate) fn toolchain_short(name: &str) -> &str {
    ["-aarch64-", "-x86_64-", "-i686-", "-arm"].iter().filter_map(|t| name.find(t)).min().map_or(name, |i| &name[..i])
}

fn dotnet_sdks(home: &Path, system: bool) -> Vec<Item> {
    let mut out = Vec::new();
    let mut roots = vec![(home.join(".dotnet"), false)];
    if system {
        roots.insert(0, (PathBuf::from("/usr/local/share/dotnet"), true));
    }
    for (root, sudo) in roots {
        let Ok(rd) = fs::read_dir(root.join("sdk")) else { continue };
        for ent in rd.flatten() {
            let path = ent.path();
            let Some(version) = path.file_name().and_then(|s| s.to_str()).map(str::to_string) else { continue };
            // Version folders start with a digit; NuGetFallbackFolder and the like are shared.
            if !exists_dir(&path) || !version.starts_with(|c: char| c.is_ascii_digit()) {
                continue;
            }
            let paths = vec![path.clone()];
            out.push(Item {
                row: Row {
                    key: format!("dotnet-sdk:{}", path.display()),
                    section: Section::Tools,
                    group: Group::Dotnet,
                    name: format!(".NET SDK {version}"),
                    label: format!(".NET SDK {version}"),
                    path: pretty(&path, home),
                    bytes: 0,
                    command: remove_command(&paths, sudo),
                    note: "Check dotnet --list-sdks first: a global.json may pin this version.".into(),
                },
                paths,
            });
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Project build output

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    NodeModules,
    RustTarget,
    MavenTarget,
    Venv,
    NextBuild,
    Dotnet,
    Gradle,
    Pods,
    SwiftBuild,
}

impl Kind {
    fn group(self) -> Group {
        match self {
            Kind::NodeModules | Kind::NextBuild => Group::Node,
            Kind::RustTarget => Group::Rust,
            Kind::MavenTarget | Kind::Gradle => Group::Java,
            Kind::Venv => Group::Python,
            Kind::Dotnet => Group::Dotnet,
            Kind::Pods | Kind::SwiftBuild => Group::Apple,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Kind::NodeModules => "node_modules",
            Kind::RustTarget => "Rust target",
            Kind::MavenTarget => "Maven target",
            Kind::Venv => "Python virtualenv",
            Kind::NextBuild => "Next.js build",
            Kind::Dotnet => ".NET bin and obj",
            Kind::Gradle => "Gradle build",
            Kind::Pods => "CocoaPods Pods",
            Kind::SwiftBuild => "SwiftPM build",
        }
    }

    fn note(self) -> &'static str {
        match self {
            Kind::NodeModules => "Dependencies. The project's install command (npm, pnpm, yarn or bun) restores them.",
            // Not `cargo clean`: with CARGO_TARGET_DIR set it would clean the shared folder instead.
            Kind::RustTarget => "Build output. cargo build recreates it.",
            Kind::MavenTarget => "Build output. mvn package recreates it.",
            Kind::Venv => "Recreate it with python3 -m venv (or uv sync) and reinstall packages.",
            Kind::NextBuild => "Build cache. next build recreates it.",
            Kind::Dotnet => "Build output. dotnet build recreates it.",
            Kind::Gradle => "Build output and project cache. gradle build recreates them.",
            Kind::Pods => "Installed pods. pod install restores them.",
            Kind::SwiftBuild => "Build output. swift build recreates it.",
        }
    }
}

/// Marker files in one folder, read from the same listing the walk already has.
#[derive(Default)]
struct Markers {
    cargo: bool,
    pom: bool,
    package_json: bool,
    dotnet: bool,
    gradle: bool,
    podfile: bool,
    swift: bool,
}

impl Markers {
    fn note(&mut self, name: &str) {
        match name {
            "Cargo.toml" => self.cargo = true,
            "pom.xml" => self.pom = true,
            "package.json" => self.package_json = true,
            "build.gradle" | "build.gradle.kts" | "settings.gradle" | "settings.gradle.kts" => self.gradle = true,
            "Podfile" => self.podfile = true,
            "Package.swift" => self.swift = true,
            _ => {
                if [".csproj", ".fsproj", ".vbproj"].iter().any(|ext| name.ends_with(ext)) {
                    self.dotnet = true;
                }
            }
        }
    }
}

/// Which build folders a project folder holds, given its marker files and child folder names.
/// Several folders of one kind (`bin` and `obj`) come back as one item.
fn build_dirs(dir: &Path, markers: &Markers, subdirs: &[String]) -> Vec<(Kind, Vec<PathBuf>)> {
    let has = |n: &str| subdirs.iter().any(|s| s == n);
    let mut out: Vec<(Kind, Vec<PathBuf>)> = Vec::new();
    let mut add = |kind: Kind, names: &[&str]| {
        let paths: Vec<PathBuf> = names.iter().filter(|n| has(n)).map(|n| dir.join(n)).collect();
        if !paths.is_empty() {
            out.push((kind, paths));
        }
    };
    add(Kind::NodeModules, &["node_modules"]);
    if markers.cargo {
        add(Kind::RustTarget, &["target"]);
    } else if markers.pom {
        add(Kind::MavenTarget, &["target"]);
    }
    for venv in [".venv", "venv"] {
        if has(venv) && dir.join(venv).join("pyvenv.cfg").is_file() {
            add(Kind::Venv, &[venv]);
        }
    }
    if markers.package_json {
        add(Kind::NextBuild, &[".next"]);
    }
    if markers.dotnet {
        add(Kind::Dotnet, &["bin", "obj"]);
    }
    if markers.gradle {
        add(Kind::Gradle, &["build", ".gradle"]);
    }
    if markers.podfile {
        add(Kind::Pods, &["Pods"]);
    }
    if markers.swift {
        add(Kind::SwiftBuild, &[".build"]);
    }
    out
}

/// Folders the project walk never enters: the home Library, media libraries, app bundles,
/// hidden folders (tool caches are measured from the catalog instead), and catalog roots.
fn skip_walk(path: &Path, name: &str, home: &Path, skip: &HashSet<PathBuf>) -> bool {
    if name.starts_with('.') || skip.contains(path) {
        return true;
    }
    if path.parent() == Some(home) && matches!(name, "Library" | "Applications" | "Pictures" | "Movies" | "Music") {
        return true;
    }
    [".app", ".photoslibrary", ".musiclibrary", ".xcarchive"].iter().any(|ext| name.ends_with(ext))
}

// ---------------------------------------------------------------------------------------------
// The job

struct Ctx {
    shared: Arc<Shared>,
    waker: rune::Waker,
    epoch: u64,
    home: PathBuf,
    /// Hard-linked files count once, as `du` does. pnpm links its store into every node_modules.
    seen: HashSet<(u64, u64)>,
    rows: Vec<Row>,
    current: PathBuf,
    last: Instant,
}

impl Ctx {
    fn alive(&self) -> bool {
        self.shared.epoch.load(Ordering::Relaxed) == self.epoch
    }

    fn publish(&mut self, done: bool) {
        if !self.alive() {
            return;
        }
        let path = if done { String::new() } else { pretty(&self.current, &self.home) };
        *self.shared.snap.lock().expect("dev lock") = Snap::from_rows(&self.rows, path, done);
        self.last = Instant::now();
        (self.waker)();
    }

    fn tick(&mut self, at: &Path) {
        if self.last.elapsed() > Duration::from_millis(100) {
            self.current = at.to_path_buf();
            self.publish(false);
        }
    }
}

fn job(shared: Arc<Shared>, waker: rune::Waker, epoch: u64) {
    let Some(home) = home_dir() else {
        *shared.snap.lock().expect("dev lock") = Snap { done: true, ..Snap::default() };
        waker();
        return;
    };
    run(&home, true, shared, waker, epoch);
}

/// Measure the catalog, then walk `home` for project build folders, publishing as rows land.
fn run(home: &Path, system: bool, shared: Arc<Shared>, waker: rune::Waker, epoch: u64) {
    let home = home.to_path_buf();
    let mut cx = Ctx {
        shared,
        waker,
        epoch,
        home: home.clone(),
        seen: HashSet::new(),
        rows: Vec::new(),
        current: home.clone(),
        last: Instant::now(),
    };
    let items = catalog(&home, system);
    let skip: HashSet<PathBuf> = items.iter().flat_map(|i| i.paths.iter().cloned()).chain([home.join("go").join("pkg")]).collect();
    for item in items {
        if !cx.alive() {
            return;
        }
        cx.current = item.paths[0].clone();
        let bytes = item.paths.iter().map(|p| measure(p, &mut cx)).sum();
        cx.rows.push(Row { bytes, ..item.row });
        cx.publish(false);
    }
    discover(&home, &skip, &mut cx);
    cx.publish(true);
}

fn discover(dir: &Path, skip: &HashSet<PathBuf>, cx: &mut Ctx) {
    if !cx.alive() {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut markers = Markers::default();
    let mut subdirs: Vec<String> = Vec::new();
    for ent in rd.flatten() {
        let Ok(ft) = ent.file_type() else { continue };
        let Some(name) = ent.file_name().to_str().map(str::to_string) else { continue };
        if ft.is_dir() {
            subdirs.push(name);
        } else if ft.is_file() {
            markers.note(&name);
        }
    }
    cx.tick(dir);
    let mut taken: HashSet<PathBuf> = HashSet::new();
    for (kind, paths) in build_dirs(dir, &markers, &subdirs) {
        let bytes = paths.iter().map(|p| measure(p, cx)).sum();
        taken.extend(paths.iter().cloned());
        cx.rows.push(project_row(kind, dir, &paths, bytes, &cx.home));
        cx.publish(false);
    }
    subdirs.sort();
    for name in subdirs {
        let path = dir.join(&name);
        if taken.contains(&path) || name == "node_modules" || skip_walk(&path, &name, &cx.home, skip) {
            continue;
        }
        discover(&path, skip, cx);
    }
}

fn project_row(kind: Kind, dir: &Path, paths: &[PathBuf], bytes: u64, home: &Path) -> Row {
    let path = if paths.len() == 1 {
        pretty(&paths[0], home)
    } else {
        let names: Vec<String> = paths.iter().filter_map(|p| p.file_name().and_then(|s| s.to_str()).map(str::to_string)).collect();
        format!("{} ({})", pretty(dir, home), names.join(", "))
    };
    Row {
        key: format!("proj:{}", paths[0].display()),
        section: Section::Projects,
        group: kind.group(),
        name: kind.name().into(),
        label: project_label(dir),
        path,
        bytes,
        command: remove_command(paths, false),
        note: kind.note().into(),
    }
}

/// Bytes a folder holds on disk, like `du`: allocated blocks, symlinks not followed, each
/// hard-linked file once per scan. Unreadable folders count as empty.
fn measure(root: &Path, cx: &mut Ctx) -> u64 {
    let mut total = 0u64;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if !cx.alive() {
            return total;
        }
        let Ok(meta) = fs::symlink_metadata(&dir) else { continue };
        total = total.saturating_add(on_disk(&meta, &mut cx.seen));
        if !meta.is_dir() {
            continue;
        }
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for ent in rd.flatten() {
            let Ok(ft) = ent.file_type() else { continue };
            if ft.is_dir() {
                stack.push(ent.path());
            } else if let Ok(m) = ent.metadata() {
                total = total.saturating_add(on_disk(&m, &mut cx.seen));
            }
        }
        cx.tick(&dir);
    }
    total
}

#[cfg(unix)]
fn on_disk(meta: &fs::Metadata, seen: &mut HashSet<(u64, u64)>) -> u64 {
    use std::os::unix::fs::MetadataExt;
    if meta.is_file() && meta.nlink() > 1 && !seen.insert((meta.dev(), meta.ino())) {
        return 0;
    }
    meta.blocks().saturating_mul(512)
}

#[cfg(not(unix))]
fn on_disk(meta: &fs::Metadata, _seen: &mut HashSet<(u64, u64)>) -> u64 {
    meta.len()
}

#[cfg(test)]
mod tests;
