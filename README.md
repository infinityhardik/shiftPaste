# Shift Paste

> Clipboard history and snippet collections, one keystroke away. Fast, private, and light on memory.

Press **Ctrl+Shift+V** (**⌘⇧V** on macOS) anywhere. A popup opens next to your mouse with everything you've copied plus your saved snippets. Type a few letters, press **Enter**, and Shift Paste pastes it into the app you were in.

**Version 2.0** is a rewrite in Rust: a fraction of 1.x's memory, and it runs on Windows, macOS and Linux. The Python 1.x code is no longer in this repository; it's in the git history (commit `fa2d57c` and earlier).

---

## What it does

| | |
|---|---|
| **Clipboard history** | Every copy is saved, de-duplicated, and kept up to your limits (500 items / 90 days by default). Rich text is optional. |
| **Collections** | Your own snippet lists (addresses, IDs, templates), with title, tags and private notes. Favorites (★) appear first. |
| **Search that forgives typos** | Letters match in order, so `1884` finds “18 mm 8 x 4 LL Pro”. Type `@work` to search one collection or `#bank` for a tag. Accents and capitals don't matter. |
| **Excel & CSV** | One-click import (each sheet becomes a collection) or a wizard with column mapping and a preview. Export one collection, all of them (a backup), or your history. |
| **Paste your way** | `Shift+Enter` pastes plain text, `Alt+Enter` changes case or joins several items (tab-separated fills Excel cells), `Ctrl+Space` picks several items. |
| **Privacy** | 100% offline. Skips copies marked secret by password managers. App lists decide which apps are recorded and where the shortcut works. Pause anytime from the tray. |
| **Everywhere** | Windows, macOS and Linux (X11 fully; Wayland with some limits). Light and dark themes, keyboard-first, screen-reader friendly. |

## Build and run (2.0)

You need [Rust](https://rustup.rs) 1.92 or newer.

```bash
cargo run --release -p shiftpaste
```

On **Linux**, first install the system libraries (Debian/Ubuntu names):

```bash
sudo apt install libgtk-3-dev libayatana-appindicator3-dev libxkbcommon-dev libxkbcommon-x11-dev libfontconfig1-dev libxcb1-dev libxcb-randr0-dev libxcb-xfixes0-dev libxcb-shape0-dev
```

On **macOS**, allow Shift Paste under *System Settings → Privacy & Security → Accessibility* so it can paste for you. Without this permission it still copies the item, and you press ⌘V yourself.

Useful commands while it runs:

```bash
shiftpaste --toggle    # open/close the popup (bind this to a key on Wayland)
shiftpaste --editor    # open the editor
shiftpaste --quit      # quit
```

Run the checks the CI runs:

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

## Releases and installers

Bump `version` in `Cargo.toml`, add notes in `.github/release-notes/<version>.md`, then push a tag such as `v2.0.1` (or run the **Release** workflow by hand). It builds a draft GitHub Release for you to review and publish; a manual run can publish directly. A version that doesn't match `Cargo.toml` is refused. The release contains:

| Platform | Download |
|---|---|
| Windows | `ShiftPaste-<version>-Setup.exe` (asks for admin, installs for all users) and `ShiftPaste-<version>-portable.zip` |
| macOS | `ShiftPaste-<version>-macOS-universal.dmg` (Apple Silicon + Intel; not notarized, so right-click → Open the first time) |
| Linux | `ShiftPaste-<version>-x86_64.AppImage` |

To build locally, use the scripts in [`packaging/`](packaging/) (`makensis` for Windows, `build-dmg.sh` on a Mac, `build-appimage.sh` on Linux).

## Project layout

```
crates/core       No UI, no OS code: database, search, library + undo/redo,
                  settings, app presets, Excel/CSV import & export (fully tested)
crates/platform   OS integration: clipboard, hotkey, tray, paste keystrokes,
                  foreground app, launch at login, dialogs, single instance
crates/app        Slint UI (crates/app/ui/*.slint) and the window controllers
packaging/        Windows installer (NSIS), macOS bundle/dmg, Linux AppImage
docs/             Design spec and decisions (RUST_REWRITE_SPEC.md), 1.x report
```

Design decisions live in [`docs/RUST_REWRITE_SPEC.md`](docs/RUST_REWRITE_SPEC.md).

## Where your data lives

| OS | Folder |
|---|---|
| Windows | `%LOCALAPPDATA%\infinityhardik\ShiftPaste\data` (settings in `%APPDATA%\infinityhardik\ShiftPaste\config`) |
| macOS | `~/Library/Application Support/com.infinityhardik.ShiftPaste` |
| Linux | `~/.local/share/shiftpaste` (settings in `~/.config/shiftpaste`) |

**Portable mode:** create a folder named `ShiftPaste-data` next to the executable, and everything is stored there instead.

**Uninstalling on Windows** keeps this data unless you tick *Also delete my clipboard history, collections and settings*. Ticked, nothing of Shift Paste is left behind, for any account on the PC. For scripted removal, `uninstall.exe /S` keeps the data and `uninstall.exe /S /DELETEDATA` removes it.

## License

MIT. See [LICENSE](LICENSE). The UI is built with [Slint](https://slint.dev) under its royalty-free license.

**Developed by:** [InfinityHardik](https://github.com/infinityhardik) + Claude
