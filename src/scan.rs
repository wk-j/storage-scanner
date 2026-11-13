//! Categories, the home-directory walk, disk capacity, and the Trash-only deletion policy.
//!
//! Clearing is limited to the real directory `~/.Trash`. A symlink at
//! that root is refused. Children that are symlinks are unlinked without following them, and a
//! directory whose canonical path escapes the root is left in place.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cat {
    Apps = 0,
    Developer = 1,
    Photos = 2,
    Movies = 3,
    Documents = 4,
    Music = 5,
    System = 6,
    Caches = 7,
    Trash = 8,
}

pub const N: usize = 9;

impl Cat {
    pub const ALL: [Cat; N] = [
        Cat::Apps,
        Cat::Developer,
        Cat::Photos,
        Cat::Movies,
        Cat::Documents,
        Cat::Music,
        Cat::System,
        Cat::Caches,
        Cat::Trash,
    ];

    pub fn ix(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            Cat::Apps => "Apps",
            Cat::Developer => "Developer",
            Cat::Photos => "Photos",
            Cat::Movies => "Movies",
            Cat::Documents => "Documents",
            Cat::Music => "Music",
            Cat::System => "System data",
            Cat::Caches => "Caches",
            Cat::Trash => "Trash",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Cat::Apps => "apps",
            Cat::Developer => "developer",
            Cat::Photos => "photos",
            Cat::Movies => "movies",
            Cat::Documents => "documents",
            Cat::Music => "music",
            Cat::System => "system",
            Cat::Caches => "caches",
            Cat::Trash => "trash",
        }
    }

    /// Legend / icon colour, sRGB hex.
    pub fn ui(self) -> u32 {
        match self {
            Cat::Apps => 0x4c8dff,
            Cat::Developer => 0x3ddc84,
            Cat::Photos => 0xff4fae,
            Cat::Movies => 0xb46bff,
            Cat::Documents => 0x2ec8d2,
            Cat::Music => 0xf0c43a,
            Cat::System => 0xc4c8d0,
            Cat::Caches => 0xf08830,
            Cat::Trash => 0xff4d57,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Purge {
    Caches,
    Trash,
}

impl Purge {
    pub fn cat(self) -> Cat {
        match self {
            Purge::Caches => Cat::Caches,
            Purge::Trash => Cat::Trash,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Stat {
    pub files: u64,
    pub photos: u64,
    pub videos: u64,
    pub largest_name: String,
    pub largest: u64,
}

impl Default for Stat {
    fn default() -> Self {
        Stat { files: 0, photos: 0, videos: 0, largest_name: String::new(), largest: 0 }
    }
}

#[derive(Clone, Debug)]
pub struct Snap {
    pub bytes: [u64; N],
    pub capacity: u64,
    pub used: u64,
    pub path: String,
    pub done: bool,
    pub stats: [Stat; N],
    pub error: Option<String>,
}

impl Default for Snap {
    fn default() -> Self {
        Snap {
            bytes: [0; N],
            capacity: 0,
            used: 0,
            path: String::new(),
            done: false,
            stats: std::array::from_fn(|_| Stat::default()),
            error: None,
        }
    }
}

pub struct Shared {
    pub epoch: AtomicU64,
    pub snap: Mutex<Snap>,
}

impl Shared {
    pub fn new() -> Self {
        Shared { epoch: AtomicU64::new(0), snap: Mutex::new(Snap::default()) }
    }
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from)
}

/// `(total, used)` bytes for the filesystem that holds `path`.
pub fn disk_usage(path: &Path) -> Option<(u64, u64)> {
    #[cfg(unix)]
    {
        let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
        use std::os::unix::ffi::OsStrExt;
        let c = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
        if unsafe { libc::statvfs(c.as_ptr(), &mut st) } != 0 {
            return None;
        }
        let fr = st.f_frsize as u64;
        let total = st.f_blocks as u64 * fr;
        let avail = st.f_bavail as u64 * fr;
        Some((total, total.saturating_sub(avail)))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

pub fn spawn(shared: Arc<Shared>, waker: rune::Waker) {
    let epoch = shared.epoch.fetch_add(1, Ordering::Relaxed) + 1;
    {
        let mut snap = shared.snap.lock().expect("scan lock");
        let capacity = snap.capacity;
        let used = snap.used;
        *snap = Snap::default();
        snap.capacity = capacity;
        snap.used = used;
        snap.path = "Starting…".into();
    }
    waker();
    let _ = std::thread::Builder::new().name("storage-scan".into()).spawn(move || scan_job(shared, waker, epoch));
}

/// Record a successful purge in the live snapshot and fold System data back out of the disk total.
pub fn note_purged(shared: &Shared, cat: Cat, freed: u64) {
    let mut snap = shared.snap.lock().expect("scan lock");
    snap.bytes[cat.ix()] = 0;
    snap.stats[cat.ix()] = Stat::default();
    snap.used = snap.used.saturating_sub(freed);
    recompute_system(&mut snap);
}

fn recompute_system(snap: &mut Snap) {
    let named: u64 = snap.bytes.iter().enumerate().filter(|(i, _)| *i != Cat::System.ix()).map(|(_, b)| *b).sum();
    snap.bytes[Cat::System.ix()] = snap.used.saturating_sub(named);
}

fn scan_job(shared: Arc<Shared>, waker: rune::Waker, epoch: u64) {
    let Some(home) = home_dir() else {
        finish_err(&shared, &waker, epoch, "No home directory");
        return;
    };
    if let Some((total, used)) = disk_usage(&home) {
        if !alive(&shared, epoch) {
            return;
        }
        let mut snap = shared.snap.lock().expect("scan lock");
        snap.capacity = total;
        snap.used = used;
    }
    waker();

    let mut acc = Acc::default();
    let mut last = Instant::now();
    walk(&home, &home, &shared, epoch, &mut acc, &mut last, &waker);
    if alive(&shared, epoch) {
        let apps = Path::new("/Applications");
        if apps.is_dir() {
            walk(apps, &home, &shared, epoch, &mut acc, &mut last, &waker);
        }
    }
    if !alive(&shared, epoch) {
        return;
    }
    publish(&shared, &acc, &home, true);
    {
        let mut snap = shared.snap.lock().expect("scan lock");
        recompute_system(&mut snap);
        snap.done = true;
        snap.path.clear();
    }
    waker();
}

fn finish_err(shared: &Shared, waker: &rune::Waker, epoch: u64, msg: &str) {
    if !alive(shared, epoch) {
        return;
    }
    let mut snap = shared.snap.lock().expect("scan lock");
    snap.error = Some(msg.into());
    snap.done = true;
    waker();
}

fn alive(shared: &Shared, epoch: u64) -> bool {
    shared.epoch.load(Ordering::Relaxed) == epoch
}

struct Acc {
    bytes: [u64; N],
    stats: [Stat; N],
    apps: HashMap<String, u64>,
    seen: u64,
    current: PathBuf,
}

impl Default for Acc {
    fn default() -> Self {
        Acc {
            bytes: [0; N],
            stats: std::array::from_fn(|_| Stat::default()),
            apps: HashMap::new(),
            seen: 0,
            current: PathBuf::new(),
        }
    }
}

fn publish(shared: &Shared, acc: &Acc, home: &Path, force: bool) {
    if !force && acc.seen % 200 != 0 {
        return;
    }
    let mut snap = shared.snap.lock().expect("scan lock");
    snap.bytes = acc.bytes;
    snap.stats = acc.stats.clone();
    snap.path = pretty(&acc.current, home);
    // System data is the rest of the disk, filled in when the walk finishes.
    snap.bytes[Cat::System.ix()] = 0;
}

fn walk(dir: &Path, home: &Path, shared: &Shared, epoch: u64, acc: &mut Acc, last: &mut Instant, waker: &rune::Waker) {
    if !alive(shared, epoch) {
        return;
    }
    if dir == home.join("Library") {
        for name in ["Caches", "Developer"] {
            let sub = dir.join(name);
            if sub.is_dir() {
                walk(&sub, home, shared, epoch, acc, last, waker);
            }
        }
        return;
    }
    let rd = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };
    for ent in rd.flatten() {
        if !alive(shared, epoch) {
            return;
        }
        let path = ent.path();
        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            if skip_dir(&path) {
                continue;
            }
            walk(&path, home, shared, epoch, acc, last, waker);
            continue;
        }
        if !meta.is_file() {
            continue;
        }
        let len = meta.len();
        acc.seen += 1;
        acc.current = path.clone();
        if let Some(cat) = classify(&path, home) {
            acc.bytes[cat.ix()] = acc.bytes[cat.ix()].saturating_add(len);
            note_file(acc, cat, &path, len);
        }
        if acc.seen % 200 == 0 || last.elapsed() > Duration::from_millis(90) {
            publish(shared, acc, home, true);
            *last = Instant::now();
            waker();
        }
    }
}

fn skip_dir(path: &Path) -> bool {
    matches!(path.file_name().and_then(|s| s.to_str()), Some(".git" | ".svn" | ".hg"))
}

fn note_file(acc: &mut Acc, cat: Cat, path: &Path, len: u64) {
    let st = &mut acc.stats[cat.ix()];
    if cat == Cat::Apps {
        if let Some(name) = bundle_name(path) {
            let new_bundle = !acc.apps.contains_key(&name);
            let total = acc.apps.entry(name.clone()).or_insert(0);
            *total = total.saturating_add(len);
            if new_bundle {
                st.files += 1;
            }
            if *total >= st.largest {
                st.largest = *total;
                st.largest_name = name;
            }
            return;
        }
    }
    st.files += 1;
    if cat == Cat::Photos {
        if is_video(path) {
            st.videos += 1;
        } else {
            st.photos += 1;
        }
    }
    if len >= st.largest {
        st.largest = len;
        st.largest_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("file").to_string();
    }
}

fn bundle_name(path: &Path) -> Option<String> {
    path.components().rev().find_map(|c| {
        let s = c.as_os_str().to_str()?;
        s.strip_suffix(".app").map(|n| n.to_string())
    })
}

pub fn blurb(cat: Cat, st: &Stat) -> String {
    if st.files == 0 && st.largest == 0 {
        return "Nothing in this category.".into();
    }
    match cat {
        Cat::Apps => {
            let n = st.files.max(1);
            if st.largest_name.is_empty() {
                format!("{n} apps.")
            } else {
                format!("{n} apps. {} is the largest at {}.", st.largest_name, fmt_bytes(st.largest))
            }
        }
        Cat::Photos => format!(
            "{} photos and {} videos, all kept on this device.",
            grouped(st.photos),
            grouped(st.videos)
        ),
        Cat::Movies => format!("{} videos.", grouped(st.files)),
        Cat::Music => format!("{} audio files, kept on this device.", grouped(st.files)),
        Cat::Documents => format!("{} files.", grouped(st.files)),
        Cat::Developer => "Projects, dependencies and build output.".into(),
        Cat::Caches => "Temporary files. Apps rebuild them when they need them.".into(),
        Cat::Trash => "Things you've already deleted, still taking space.".into(),
        Cat::System => "Everything else on the disk that isn't one of the categories above.".into(),
    }
}

fn grouped(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out.chars().rev().collect()
}

fn fmt_bytes(bytes: u64) -> String {
    let tenths = ((bytes as f64) / 100_000_000.0).round().max(0.0) as u64;
    format!("{}.{} GB", tenths / 10, tenths % 10)
}

/// Bucket a file. `None` means it stays in the disk-level System data residual.
pub fn classify(path: &Path, home: &Path) -> Option<Cat> {
    if under(path, &home.join(".Trash")) {
        return Some(Cat::Trash);
    }
    if under(path, &home.join("Library").join("Caches")) {
        return Some(Cat::Caches);
    }
    if under(path, &home.join("Applications")) || under(path, Path::new("/Applications")) || has_app(path) {
        return Some(Cat::Apps);
    }
    if under(path, &home.join("Library").join("Developer")) || dev_component(path) {
        return Some(Cat::Developer);
    }
    if under(path, &home.join("Pictures")) || under(path, &home.join("Photos")) {
        return Some(Cat::Photos);
    }
    if under(path, &home.join("Movies")) || is_video(path) {
        return Some(Cat::Movies);
    }
    if under(path, &home.join("Music")) || is_audio(path) {
        return Some(Cat::Music);
    }
    if is_photo(path) {
        return Some(Cat::Photos);
    }
    if under(path, &home.join("Documents")) || is_doc(path) {
        return Some(Cat::Documents);
    }
    None
}

fn under(path: &Path, root: &Path) -> bool {
    path.starts_with(root)
}

fn has_app(path: &Path) -> bool {
    path.components().any(|c| c.as_os_str().to_str().is_some_and(|s| s.ends_with(".app")))
}

fn dev_component(path: &Path) -> bool {
    const NAMES: &[&str] = &[
        "node_modules",
        "target",
        ".cargo",
        ".gradle",
        ".venv",
        "venv",
        "Pods",
        ".swiftpm",
        "__pycache__",
        ".tox",
        ".turbo",
        ".next",
        ".nuxt",
        "Carthage",
        "DerivedData",
        ".yarn",
        ".pnpm-store",
        ".npm",
        "vendor",
    ];
    path.components().any(|c| c.as_os_str().to_str().is_some_and(|s| NAMES.contains(&s)))
}

fn ext_is(path: &Path, set: &[&str]) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        let e = e.to_ascii_lowercase();
        set.iter().any(|s| *s == e)
    })
}

fn is_photo(path: &Path) -> bool {
    ext_is(path, &["jpg", "jpeg", "png", "gif", "heic", "heif", "tif", "tiff", "bmp", "webp", "dng", "raw", "cr2", "nef", "arw", "orf", "rw2", "raf"])
}

fn is_video(path: &Path) -> bool {
    ext_is(path, &["mov", "mp4", "m4v", "mkv", "avi", "webm", "wmv", "mpg", "mpeg", "flv"])
}

fn is_audio(path: &Path) -> bool {
    ext_is(path, &["mp3", "m4a", "aac", "flac", "wav", "aiff", "aif", "ogg", "opus", "wma", "alac"])
}

fn is_doc(path: &Path) -> bool {
    ext_is(path, &["pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "txt", "rtf", "md", "pages", "numbers", "key", "csv", "epub", "odt", "ods", "odp"])
}

pub fn pretty(path: &Path, home: &Path) -> String {
    let shown = if let Ok(rest) = path.strip_prefix(home) {
        format!("~/{}", rest.display())
    } else {
        path.display().to_string()
    };
    if shown.chars().count() > 72 {
        let tail: String = shown.chars().rev().take(68).collect::<String>().chars().rev().collect();
        format!("…{tail}")
    } else {
        shown
    }
}

fn root_for(home: &Path, which: Purge) -> PathBuf {
    match which {
        Purge::Caches => home.join("Library").join("Caches"),
        Purge::Trash => home.join(".Trash"),
    }
}

/// Delete the children of `~/.Trash` only and nothing else.
/// Returns the number of bytes removed.
pub fn purge(home: &Path, which: Purge) -> Result<u64, String> {
    if which != Purge::Trash {
        return Err("Cache deletion is disabled. Only Trash can be emptied.".into());
    }
    let root = root_for(home, which);
    if !root.exists() {
        return Ok(0);
    }
    let meta = fs::symlink_metadata(&root).map_err(|e| e.to_string())?;
    if meta.file_type().is_symlink() {
        return Err(format!("refusing to delete a symlinked {}", root.display()));
    }
    if !meta.is_dir() {
        return Err(format!("refusing to delete {}: not a directory", root.display()));
    }
    let home_c = home.canonicalize().map_err(|e| e.to_string())?;
    let root_c = root.canonicalize().map_err(|e| e.to_string())?;
    let expected = match which {
        Purge::Caches => home_c.join("Library").join("Caches"),
        Purge::Trash => home_c.join(".Trash"),
    };
    let expected_c = expected.canonicalize().map_err(|e| e.to_string())?;
    if root_c != expected_c {
        return Err(format!("refusing to delete {}: outside the allowed root", root.display()));
    }
    // The allowed roots are exactly these two suffixes of the home directory.
    let ok_suffix = match which {
        Purge::Caches => root_c.ends_with(Path::new("Library/Caches")) || root_c.ends_with(Path::new("Library\\Caches")),
        Purge::Trash => root_c.ends_with(".Trash"),
    };
    if !ok_suffix || !root_c.starts_with(&home_c) {
        return Err("refusing to delete outside ~/Library/Caches and ~/.Trash".into());
    }

    let mut freed = 0u64;
    let mut failed = 0u64;
    let entries = fs::read_dir(&root_c).map_err(|e| e.to_string())?;
    for ent in entries {
        let ent = match ent {
            Ok(ent) => ent,
            Err(_) => {
                failed += 1;
                continue;
            }
        };
        match remove_child(&ent.path(), &root_c) {
            Ok(n) => freed = freed.saturating_add(n),
            Err(_) => failed += 1,
        }
    }
    if failed > 0 {
        Err(format!("{failed} items in {} could not be removed", display_root(which)))
    } else {
        Ok(freed)
    }
}

fn display_root(which: Purge) -> &'static str {
    match which {
        Purge::Caches => "~/Library/Caches",
        Purge::Trash => "~/.Trash",
    }
}

fn remove_child(path: &Path, root_c: &Path) -> io::Result<u64> {
    let meta = fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() {
        fs::remove_file(path)?;
        return Ok(0);
    }
    if meta.is_dir() {
        let canon = path.canonicalize()?;
        if !canon.starts_with(root_c) {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "escapes root"));
        }
        let mut n = 0u64;
        for ent in fs::read_dir(path)? {
            n = n.saturating_add(remove_child(&ent?.path(), root_c)?);
        }
        fs::remove_dir(path)?;
        Ok(n)
    } else {
        let n = meta.len();
        fs::remove_file(path)?;
        Ok(n)
    }
}

/// Used by the safety test to plant a file without going through the scanner.
#[cfg(test)]
fn write_file(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, bytes).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home() -> PathBuf {
        PathBuf::from("/home/ada")
    }

    #[test]
    fn buckets_follow_path_and_extension_priority() {
        let h = home();
        assert_eq!(classify(Path::new("/home/ada/Library/Caches/pnpm/a"), &h), Some(Cat::Caches));
        assert_eq!(classify(Path::new("/home/ada/.Trash/atlas-backup.zip"), &h), Some(Cat::Trash));
        assert_eq!(classify(Path::new("/home/ada/Applications/Penrose Studio.app/Contents/MacOS/Penrose"), &h), Some(Cat::Apps));
        assert_eq!(classify(Path::new("/Applications/Xcode.app/Contents/Info.plist"), &h), Some(Cat::Apps));
        assert_eq!(classify(Path::new("/home/ada/code/web/node_modules/leftpad/index.js"), &h), Some(Cat::Developer));
        assert_eq!(classify(Path::new("/home/ada/proj/target/release/app"), &h), Some(Cat::Developer));
        assert_eq!(classify(Path::new("/home/ada/.cargo/registry/cache/x"), &h), Some(Cat::Developer));
        assert_eq!(classify(Path::new("/home/ada/Library/Developer/Xcode/DerivedData/a/b"), &h), Some(Cat::Developer));
        assert_eq!(classify(Path::new("/home/ada/Pictures/trip/a.jpg"), &h), Some(Cat::Photos));
        assert_eq!(classify(Path::new("/home/ada/Pictures/clip.mov"), &h), Some(Cat::Photos));
        assert_eq!(classify(Path::new("/home/ada/Movies/iceland.mp4"), &h), Some(Cat::Movies));
        assert_eq!(classify(Path::new("/home/ada/Downloads/talk.mkv"), &h), Some(Cat::Movies));
        assert_eq!(classify(Path::new("/home/ada/Music/song.mp3"), &h), Some(Cat::Music));
        assert_eq!(classify(Path::new("/home/ada/Documents/taxes.pdf"), &h), Some(Cat::Documents));
        assert_eq!(classify(Path::new("/home/ada/Desktop/notes.txt"), &h), Some(Cat::Documents));
        assert_eq!(classify(Path::new("/home/ada/Library/Application Support/app/data"), &h), None);
        assert_eq!(classify(Path::new("/home/ada/misc.bin"), &h), None);
        // Caches wins over a photo extension, Trash wins over a movie.
        assert_eq!(classify(Path::new("/home/ada/Library/Caches/thumb.jpg"), &h), Some(Cat::Caches));
        assert_eq!(classify(Path::new("/home/ada/.Trash/clip.mov"), &h), Some(Cat::Trash));
    }

    #[test]
    fn purge_only_removes_trash_and_refuses_caches() {
        let root = std::env::temp_dir().join(format!("storage-scanner-purge-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let home = root.join("home");
        write_file(&home.join("Library/Caches/a.txt"), b"cache-a");
        write_file(&home.join("Library/Caches/sub/b.txt"), b"cache-b");
        write_file(&home.join("Library/Important/keep.txt"), b"keep-me");
        write_file(&home.join("Documents/keep.txt"), b"doc");
        write_file(&home.join(".Trash/old.txt"), b"trash");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(home.join("Documents/keep.txt"), home.join("Library/Caches/link")).unwrap();
        }

        let err = purge(&home, Purge::Caches).unwrap_err();
        assert!(err.contains("disabled"));
        assert_eq!(fs::read(home.join("Library/Caches/a.txt")).unwrap(), b"cache-a");
        assert_eq!(fs::read(home.join("Library/Caches/sub/b.txt")).unwrap(), b"cache-b");
        assert_eq!(fs::read(home.join("Documents/keep.txt")).unwrap(), b"doc");
        #[cfg(unix)]
        assert!(fs::symlink_metadata(home.join("Library/Caches/link")).unwrap().file_type().is_symlink());

        let freed_trash = purge(&home, Purge::Trash).unwrap();
        assert!(freed_trash >= "trash".len() as u64);
        assert!(!home.join(".Trash/old.txt").exists());
        assert!(home.join(".Trash").is_dir());
        assert_eq!(fs::read(home.join("Documents/keep.txt")).unwrap(), b"doc");

        // A symlinked Trash root is refused, and the target is untouched.
        let elsewhere = root.join("elsewhere");
        write_file(&elsewhere.join("secret.txt"), b"secret");
        let linked_home = root.join("linked");
        fs::create_dir_all(linked_home.join("Library")).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&elsewhere, linked_home.join(".Trash")).unwrap();
            let err = purge(&linked_home, Purge::Trash).unwrap_err();
            assert!(err.contains("refusing"), "{err}");
            assert_eq!(fs::read(elsewhere.join("secret.txt")).unwrap(), b"secret");
        }

        let _ = fs::remove_dir_all(&root);
    }
}
