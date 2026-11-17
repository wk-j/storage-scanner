# Building and screenshots

## Install with Homebrew

The formula is in [wk-j/homebrew-tap](https://github.com/wk-j/homebrew-tap). It builds the latest release (v0.2.0) from source with Homebrew's Rust:

```sh
brew install wk-j/tap/storage-scanner           # latest tagged release
brew install --HEAD wk-j/tap/storage-scanner    # straight from main
storage-scanner                                 # opens the window
```

`brew upgrade storage-scanner` picks up a newer release once the tap's formula points at it.

## macOS desktop application


Build a Finder-launchable app on macOS with Rust/Cargo and the Xcode Command Line
Tools (`xcode-select --install`). Use a current stable Rust toolchain: edition 2024
alone does not guarantee that all pinned dependencies support Rust 1.85.

```sh
./scripts/package-macos.sh
open "target/macos/Storage Scanner.app"
```

Drag the resulting **Storage Scanner.app** into `/Applications` to install it.
No repository files or Terminal are required at runtime; fonts are embedded in the
executable. Launching normally scans your home folder. For a safe demo:

```sh
open -n "target/macos/Storage Scanner.app" --args --demo
```

The script builds the Rust host architecture (Apple Silicon or Intel, not a
Universal binary), uses `Cargo.lock`, validates the bundle and checks that linked
libraries come only from macOS system locations. The bundle identifier is
`app.storage-scanner.desktop`; the version comes from `Cargo.toml`. Existing
bundles are retained; subsequent builds print a new timestamped output path.

The minimum macOS version is conservatively set to the **build machine's OS
version**, both in the executable and `Info.plist`. The initial verified build is
Apple Silicon on macOS 27.2 with SDK 27.2 and Rust 1.99.1. Older macOS versions and
Intel have not been verified. This is not a claim that dependencies require 27.2;
a lower deployment target needs separate compatibility testing.

The app is **ad-hoc signed**, not Developer ID signed or notarized. This is a local
build workflow, not a public distribution release. Gatekeeper may block downloaded
copies; do not disable Gatekeeper. Public distribution needs a stable publisher
bundle ID, Developer ID signing and Apple notarization as a separate step.

No sandbox entitlement or automatic Full Disk Access grant is added. macOS privacy
controls still apply, and Terminal permissions do not necessarily carry over to
Finder-launched apps. The scanner currently skips unreadable folders, so category
sizes may be incomplete without a detailed permissions warning. Grant access in
System Settings only if you trust the app and need it. Empty Trash still requires
confirmation and permanently removes only children of `~/.Trash`; Caches remain
display-only. Never use your real Trash to test deletion.

The original icon source is `assets/macos/generate-icon.swift`. Regenerate the
committed icon with:

```sh
./scripts/package-macos.sh --icon-only
```

Build logs and intermediate artifacts may be kept under `target/macos/`; no
packaging output is written into the tracked screenshot directory `out/`.

## Build and run

Rust 1.85 or newer (edition 2024). The window uses Metal on macOS and Vulkan or DX12 elsewhere. A machine with no GPU can render through Lavapipe:

```sh
sudo apt install mesa-vulkan-drivers
export VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json
```

```sh
cargo run --release
cargo run --release -- --demo
```

On Linux the default is the demo: a fixed 512 GB disk (357.8 GB used) and no deletes. `--real` walks the home directory instead. On macOS and Windows the real walk is the default; `--demo` forces the designed dataset.

## Screenshots

`scripts/shots.sh` records the demo with Rune's headless renderer (the same one as the window):

| | |
| --- | --- |
| Scanning | `out/scanning.png` |
| Idle, Apps hovered | `out/idle-apps.png` |
| Emptying Trash | `out/clearing-caches.png` |
| After emptying the Trash | `out/after-trash.png` |
| Developer tools, Rust hovered | `out/dev-tools.png` |
| Developer tools, a Rust item selected | `out/dev-tools-command.png` |

## App icon

`assets/icon.png` is the Dock and app-switcher icon: the dot-matrix bar on a dark tile. The app sets it when it starts (`rune::set_app_icon`), because Homebrew installs a bare binary with no app bundle to carry one. `python3 scripts/icon.py` draws it again; it needs only the Python standard library.
