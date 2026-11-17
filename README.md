# Storage Scanner

A native disk map, written in Rust with [Rune](https://github.com/anomalyco/rune). The window is the card. Used space is a row of glowing cells, one run per category; free space is the dim grid beside them. Emptying the Trash burns its cells off the bar. Caches are displayed but cannot be cleared.

Design reference: [Storage scanner by @kshvbgde](https://x.com/bestsaasdesigns/status/2121105777415090259). The frames it was matched against are in `docs/design/`.

## Clear with fire

[![Fire effect while emptying Trash](docs/fire-clear.gif)](docs/fire-clear.mp4)

The fire effect from UI Lab burns the Trash area after confirmation. Cancel leaves it untouched. This clip uses demo mode, so no real files are deleted.

[Watch the 60 fps MP4](docs/fire-clear.mp4).

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

**Empty Trash** asks first. In the demo it only plays the animation. In real mode it permanently deletes children of `~/.Trash` and nothing else. Cache deletion is blocked in both the UI and backend; `~/Library/Caches` is scanned for display only. A symlinked Trash root is refused, and symlinks inside are unlinked without following them.

The fire clip above uses the demo disk and is recorded with the headless renderer (`--demo --script scripts/design-qa.json --video docs/fire-clear.mp4`).

## Screenshots

`scripts/shots.sh` records the demo with Rune's headless renderer (the same one as the window):

| | |
| --- | --- |
| Scanning | `out/scanning.png` |
| Idle, Apps hovered | `out/idle-apps.png` |
| Emptying Trash | `out/clearing-caches.png` |
| After emptying the Trash | `out/after-trash.png` |

## How files are counted

The real scanner walks the home directory (and `/Applications` when it is there) and buckets file bytes:

- **Trash** — `~/.Trash`
- **Caches** — `~/Library/Caches`
- **Apps** — `~/Applications`, `/Applications`, and `*.app` bundles
- **Developer** — `~/Library/Developer`, plus `node_modules`, `target`, `.cargo`, and the other build directories in `src/scan.rs`
- **Photos, Movies, Music, Documents** — those folders, and files with a matching extension
- **System data** — disk used, minus the categories above

## Fonts

Inter (Regular, Medium, SemiBold, Bold) is bundled under the SIL Open Font License, `assets/Inter-LICENSE.txt`. Paths use IBM Plex Mono, also OFL, `assets/IBMPlexMono-LICENSE.txt`.
