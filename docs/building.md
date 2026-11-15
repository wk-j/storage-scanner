# Building and screenshots

## Install with Homebrew

The formula is in [wk-j/homebrew-tap](https://github.com/wk-j/homebrew-tap). It builds the latest release (v0.1.0) from source with Homebrew's Rust:

```sh
brew install wk-j/tap/storage-scanner           # latest tagged release
brew install --HEAD wk-j/tap/storage-scanner    # straight from main
storage-scanner                                 # opens the window
```

`brew upgrade storage-scanner` picks up a newer release once the tap's formula points at it.

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
