# Shift Paste

> Clipboard history and snippet collections, one keystroke away. Fast, private, and light on memory.

Press **Ctrl+Shift+V** (**⌘⇧V** on macOS) anywhere. A popup opens next to your mouse with everything you've copied plus your saved snippets. Type a few letters, press **Enter**, and Shift Paste pastes it into the app you were in.

**Status:** version 2.0 is being rewritten in Rust (this branch). The Python 1.x app is kept in [`legacy/`](legacy/) until 2.0 ships, and its downloads are still in [`dist/`](dist/).

---

## What it does

| | |
|---|---|
| **Clipboard history** | Every copy is saved, de-duplicated, and kept up to your limits (500 items / 90 days by default). Rich text is optional. |
| **Collections** | Your own snippet lists (addresses, IDs, templates), with title, tags and private notes. Favorites (★) appear first. |
| **Search that forgives typos** | Letters match in order, so `1884` finds “18 mm 8 x 4 LL Pro”. Type `@work` to search one collection or `#bank` for a tag. Accents and capitals don't matter. |
| **Excel & CSV** | One-click import (each sheet becomes a collection) or a wizard with column mapping and a preview. Export one collection, all of them (a backup), or your history. |
| **Paste your way** | `⇧ Enter` pastes plain text, `Alt Enter` changes case or joins several items (tab-separated fills Excel cells), `Ctrl+Space` picks several items. |
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

## Project layout

```
crates/core       No UI, no OS code: database, search, library + undo/redo,
                  settings, app presets, Excel/CSV import & export (fully tested)
crates/platform   OS integration: clipboard, hotkey, tray, paste keystrokes,
                  foreground app, launch at login, dialogs, single instance
crates/app        Slint UI (crates/app/ui/*.slint) and the window controllers
docs/             Design spec and decisions (RUST_REWRITE_SPEC.md), 1.x report
legacy/           The Python 1.x app (to be removed when 2.0 ships)
```

Design decisions live in [`docs/RUST_REWRITE_SPEC.md`](docs/RUST_REWRITE_SPEC.md).

## Where your data lives

| OS | Folder |
|---|---|
| Windows | `%LOCALAPPDATA%\infinityhardik\ShiftPaste\data` (settings in `%APPDATA%\infinityhardik\ShiftPaste\config`) |
| macOS | `~/Library/Application Support/com.infinityhardik.ShiftPaste` |
| Linux | `~/.local/share/shiftpaste` (settings in `~/.config/shiftpaste`) |

**Portable mode:** create a folder named `ShiftPaste-data` next to the executable, and everything is stored there instead.

## License

MIT. See [LICENSE](LICENSE). The UI is built with [Slint](https://slint.dev) under its royalty-free license.

**Developed by:** [InfinityHardik](https://github.com/infinityhardik) + AI assistants (Gemini, Claude)
