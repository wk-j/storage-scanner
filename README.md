# Storage Scanner

A native disk map, written in Rust with [Rune](https://github.com/anomalyco/rune). Used space is a row of glowing cells, one run per category; free space is the dim grid beside them. A second tab, **Developer tools**, shows what package managers, toolchains and build folders hold, with the command that cleans each one up.

[![Fire effect while emptying Trash](docs/fire-clear.gif)](docs/fire-clear.mp4)

## Install

```sh
brew install wk-j/tap/storage-scanner
storage-scanner
```

From source, with Rust 1.85 or newer:

```sh
cargo run --release              # scans your home folder (the demo on Linux)
cargo run --release -- --demo    # the designed demo disk, nothing is deleted
```

## What it deletes

Only **Empty Trash** deletes, after you confirm, and only the contents of `~/.Trash`. Caches are shown but never cleared. The Developer tools tab shows commands for you to run; the app never runs them.

## Docs

- [Overview tab](docs/overview.md): Empty Trash, the fire effect, how files are counted
- [Developer tools tab](docs/developer-tools.md): tool families, what is measured, the cleanup commands
- [Building and screenshots](docs/building.md): Homebrew, requirements, demo and real modes, the headless renderer

## Credits

Design reference: [Storage scanner by @kshvbgde](https://x.com/bestsaasdesigns/status/2121105777415090259); the frames it was matched against are in `docs/design/`. Inter (Regular, Medium, SemiBold, Bold) and IBM Plex Mono, used for paths, are bundled under the SIL Open Font License (`assets/Inter-LICENSE.txt`, `assets/IBMPlexMono-LICENSE.txt`).
