use super::*;

fn row(group: Group, key: &str, bytes: u64) -> Row {
    Row {
        key: key.into(),
        section: Section::Tools,
        group,
        name: key.into(),
        label: key.into(),
        path: key.into(),
        bytes,
        command: String::new(),
        note: String::new(),
    }
}

#[test]
fn a_snapshot_totals_every_family_folds_small_items_and_sorts_by_size() {
    let all = vec![
        row(Group::Rust, "r-big", 9 * MIN_ROW),
        row(Group::Node, "n-small", MIN_ROW - 1),
        row(Group::Node, "n-mid", 2 * MIN_ROW),
        row(Group::Docker, "d-big", 5 * MIN_ROW),
        row(Group::Rust, "r-small", 10),
        row(Group::Java, "j-empty", 0),
    ];
    let snap = Snap::from_rows(&all, String::new(), true);
    let keys: Vec<&str> = snap.rows.iter().map(|r| r.key.as_str()).collect();
    assert_eq!(keys, ["r-big", "d-big", "n-mid"]);
    assert_eq!(snap.groups[Group::Rust.ix()], 9 * MIN_ROW + 10, "small items still count toward their family");
    assert_eq!(snap.groups[Group::Node.ix()], 3 * MIN_ROW - 1);
    assert_eq!(snap.small[Group::Node.ix()], Small { count: 1, bytes: MIN_ROW - 1 });
    assert_eq!(snap.small[Group::Rust.ix()], Small { count: 1, bytes: 10 });
    assert_eq!(snap.small[Group::Java.ix()], Small::default(), "an empty folder is no item");
    assert_eq!(snap.total, snap.groups.iter().sum::<u64>());
    assert_eq!(snap.group_rows(Group::Node).count(), 1);
}

#[test]
fn project_labels_name_the_project_not_the_build_folder() {
    assert_eq!(project_label(Path::new("/Users/ada/Project/e-saraban/web")), "e-saraban/web");
    assert_eq!(toolchain_short("stable-aarch64-apple-darwin"), "stable");
    assert_eq!(toolchain_short("1.90.0-x86_64-unknown-linux-gnu"), "1.90.0");
    assert_eq!(toolchain_short("nightly"), "nightly");
}

#[test]
fn commands_quote_paths_for_every_shell() {
    assert_eq!(quote(Path::new("/Users/ada/Library/Application Support/x")), "'/Users/ada/Library/Application Support/x'");
    assert_eq!(quote(Path::new("/tmp/it's")), "'/tmp/it'\\''s'");
    let paths = [PathBuf::from("/a/bin"), PathBuf::from("/a/obj")];
    assert_eq!(remove_command(&paths, false), "rm -rf '/a/bin' '/a/obj'");
    assert_eq!(remove_command(&paths[..1], true), "sudo rm -rf '/a/bin'");
}

#[test]
fn sizes_read_in_megabytes_below_a_gigabyte() {
    assert_eq!(fmt_size(50_000_000), "50 MB");
    assert_eq!(fmt_size(999_400_000), "999 MB");
    assert_eq!(fmt_size(999_600_000), "1.0 GB");
    assert_eq!(fmt_size(21_500_000_000), "21.5 GB");
}

#[test]
fn cargo_config_target_dir_is_read_from_the_usual_line() {
    assert_eq!(config_target_dir("[build]\ntarget-dir = \"/Users/ada/.cargo-target\"\n").as_deref(), Some("/Users/ada/.cargo-target"));
    assert_eq!(config_target_dir("[build]\ntarget-dir='~/t'").as_deref(), Some("~/t"));
    assert_eq!(config_target_dir("[build]\njobs = 4\n"), None);
}

#[test]
fn build_folders_need_their_project_marker() {
    let dir = Path::new("/p");
    let names = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let kinds = |m: &Markers, subdirs: &[&str]| build_dirs(dir, m, &names(subdirs)).into_iter().map(|(k, p)| (k, p.len())).collect::<Vec<_>>();

    // A `target`, `bin` or `build` folder alone is source, not build output.
    let none = Markers::default();
    assert_eq!(kinds(&none, &["target", "bin", "obj", "build", ".next", "src"]), []);
    // node_modules needs no marker.
    assert_eq!(kinds(&none, &["node_modules"]), [(Kind::NodeModules, 1)]);

    let mut rust = Markers::default();
    rust.note("Cargo.toml");
    assert_eq!(kinds(&rust, &["target", "src"]), [(Kind::RustTarget, 1)]);

    let mut maven = Markers::default();
    maven.note("pom.xml");
    assert_eq!(kinds(&maven, &["target"]), [(Kind::MavenTarget, 1)]);

    let mut dotnet = Markers::default();
    dotnet.note("App.csproj");
    assert_eq!(kinds(&dotnet, &["bin", "obj", "Properties"]), [(Kind::Dotnet, 2)], "bin and obj are one row");

    let mut next = Markers::default();
    next.note("package.json");
    assert_eq!(kinds(&next, &["node_modules", ".next"]), [(Kind::NodeModules, 1), (Kind::NextBuild, 1)]);
}

#[test]
fn the_walk_skips_library_media_hidden_folders_and_bundles() {
    let home = Path::new("/Users/ada");
    let skip: HashSet<PathBuf> = [home.join("go/pkg")].into_iter().collect();
    let at = |p: &str| {
        let path = home.join(p);
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        skip_walk(&path, &name, home, &skip)
    };
    assert!(at("Library"));
    assert!(at("Pictures"));
    assert!(at(".cache"));
    assert!(at("go/pkg"));
    assert!(at("code/Tool.app"));
    assert!(!at("code"));
    assert!(!at("code/Library"), "only the home Library is skipped");
}

fn write(path: &Path, bytes: usize) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, vec![7u8; bytes]).unwrap();
}

fn scan(home: &Path) -> Snap {
    let shared = Arc::new(Shared::new());
    shared.epoch.store(1, Ordering::Relaxed);
    run(home, false, shared.clone(), Arc::new(|| {}), 1);
    let snap = shared.snap.lock().unwrap().clone();
    assert!(snap.done);
    snap
}

/// Every measured row, small ones too, for assertions on a tiny fixture.
fn all_rows(home: &Path) -> Vec<Row> {
    let shared = Arc::new(Shared::new());
    shared.epoch.store(1, Ordering::Relaxed);
    let items = catalog(home, false);
    let mut rows: Vec<Row> = items.iter().map(|i| i.row.clone()).collect();
    let mut cx = Ctx {
        shared,
        waker: Arc::new(|| {}),
        epoch: 1,
        home: home.to_path_buf(),
        seen: HashSet::new(),
        rows: Vec::new(),
        current: home.to_path_buf(),
        last: Instant::now(),
    };
    let skip: HashSet<PathBuf> = items.iter().flat_map(|i| i.paths.iter().cloned()).collect();
    discover(home, &skip, &mut cx);
    rows.extend(cx.rows);
    rows
}

#[test]
fn scanning_a_home_finds_tools_and_build_folders_and_deletes_nothing() {
    let root = std::env::temp_dir().join(format!("storage-scanner-dev-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let home = root.join("home");
    write(&home.join(".npm/_cacache/content/a"), 4096);
    write(&home.join("Library/Caches/pip/http/b"), 4096);
    write(&home.join("code/app/Cargo.toml"), 10);
    write(&home.join("code/app/src/main.rs"), 10);
    write(&home.join("code/app/target/debug/app"), 8192);
    write(&home.join("code/web/package.json"), 10);
    write(&home.join("code/web/node_modules/left/index.js"), 4096);
    write(&home.join("code/web/node_modules/left/node_modules/deep/x.js"), 4096);
    write(&home.join("code/tool/bin/run.sh"), 10);
    write(&home.join("code/desk/App.csproj"), 10);
    write(&home.join("code/desk/bin/Debug/App.dll"), 4096);
    write(&home.join("code/desk/obj/project.assets.json"), 4096);
    write(&home.join("code/api/.venv/pyvenv.cfg"), 10);
    write(&home.join("code/api/.venv/lib/site.py"), 4096);
    write(&home.join("Library/Application Support/Code/User/settings.json"), 10);

    let rows = all_rows(&home);
    let find = |key: &str| rows.iter().find(|r| r.key == key).unwrap_or_else(|| panic!("{key} in {:?}", rows.iter().map(|r| &r.key).collect::<Vec<_>>()));

    assert_eq!(find("npm-cache").command, "npm cache clean --force");
    assert_eq!(find("pip-cache").command, "python3 -m pip cache purge");

    let target = find(&format!("proj:{}", home.join("code/app/target").display()));
    assert_eq!(target.command, format!("rm -rf '{}'", home.join("code/app/target").display()));
    assert!(target.bytes >= 8192);

    // node_modules is one row; the nested one is inside it, not a second row.
    let modules: Vec<&Row> = rows.iter().filter(|r| r.name == "node_modules").collect();
    assert_eq!(modules.len(), 1);
    assert!(modules[0].bytes >= 8192);

    let desk = find(&format!("proj:{}", home.join("code/desk/bin").display()));
    assert_eq!(desk.command, format!("rm -rf '{}' '{}'", home.join("code/desk/bin").display(), home.join("code/desk/obj").display()));
    find(&format!("proj:{}", home.join("code/api/.venv").display()));

    // A bin folder without a project file is someone's scripts.
    assert!(!rows.iter().any(|r| r.command.contains("code/tool")));
    // Library outside the catalog is never walked for projects.
    assert!(!rows.iter().any(|r| r.path.contains("Application Support")));
    // No command removes a project folder itself or home.
    for r in &rows {
        assert!(!r.command.ends_with(&format!("'{}'", home.join("code/app").display())), "{}", r.command);
        assert!(!r.command.contains(&format!("'{}'", home.display())), "{}", r.command);
    }

    // The full job finishes, totals every row, and the files are all still there.
    let snap = scan(&home);
    assert!(snap.total >= 8192 * 4);
    assert!(home.join("code/app/target/debug/app").is_file());
    assert!(home.join("code/web/node_modules/left/index.js").is_file());
    assert!(home.join(".npm/_cacache/content/a").is_file());

    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn hard_links_count_once() {
    let root = std::env::temp_dir().join(format!("storage-scanner-links-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    write(&root.join("store/pkg.js"), 64 * 1024);
    fs::create_dir_all(root.join("project/node_modules")).unwrap();
    fs::hard_link(root.join("store/pkg.js"), root.join("project/node_modules/pkg.js")).unwrap();

    let shared = Arc::new(Shared::new());
    let mut cx = Ctx {
        shared,
        waker: Arc::new(|| {}),
        epoch: 0,
        home: root.clone(),
        seen: HashSet::new(),
        rows: Vec::new(),
        current: root.clone(),
        last: Instant::now(),
    };
    let store = measure(&root.join("store"), &mut cx);
    let modules = measure(&root.join("project/node_modules"), &mut cx);
    assert!(store >= 64 * 1024);
    assert!(modules < 64 * 1024, "the linked file was already counted: {modules}");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn the_demo_lands_every_row_then_stops() {
    let start = demo::snap(0.0);
    assert!(!start.done);
    assert!(start.rows.is_empty());
    let mid = demo::snap(demo::DURATION * 0.5);
    assert!(!mid.done);
    assert!(mid.total > 0 && !mid.path.is_empty());
    let end = demo::snap(demo::DURATION);
    assert!(end.done);
    assert!(end.path.is_empty());
    assert!(end.total > mid.total);
    assert!(end.small.iter().filter(|s| s.count > 0).count() >= 2, "some families show a smaller-items cell");
    assert!(Group::ALL.iter().all(|g| end.groups[g.ix()] > 0), "the demo fills every family");
    for r in &end.rows {
        assert!(!r.command.is_empty(), "{}", r.key);
        assert!(r.bytes >= MIN_ROW);
    }
}
