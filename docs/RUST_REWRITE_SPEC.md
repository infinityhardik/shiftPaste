# Shift Paste 2.0 — Rust Rewrite Spec

> The agreed plan for rewriting Shift Paste in Rust: every decision made with the owner, plus
> the defaults chosen where nothing was specified (marked **[default]**, open to change).
> Background on the current Python app: [`CODEBASE_REPORT.md`](CODEBASE_REPORT.md).

**Goals:**
- Much lower memory than the Python/PySide6 app.
- Runs on **Windows, macOS and Linux**.
- **In-app editor** for snippet collections.
- **Excel/CSV import and export.**
- Every setting in the UI actually works (today 7 of 11 don't).

---

## 0. Quality bar (applies to every feature)

Each item is a concrete rule the build is checked against, not a slogan.

| Principle | Rule |
|---|---|
| **Simplicity** | Works out of the box with zero setup. Every setting visibly does something (no dead settings like 1.x). Nothing ever needs a restart. Fewer settings beat more settings: if a good default exists, there's no setting. |
| **Ease of use** | Keyboard-first: every popup and editor action has a shortcut, shown in tooltips/menus. Undo replaces "Are you sure?"; confirmations only for big destructive actions (clear history, delete a collection). Clear empty states, e.g. "No matches. Ctrl+S saves the selected item to a collection". |
| **First run** | A small welcome card shows the hotkey, how to import Excel/CSV, and (macOS) why Accessibility permission is needed, with a button that opens the right settings page. |
| **Responsiveness** | The UI thread never waits on disk, clipboard or Excel. Search runs on **every keystroke** with no debounce delay (fast enough in Rust), so 1.x's "search delay" setting is gone. Popup opens ≤ 100 ms after the hotkey. |
| **Performance & memory** | Event-driven clipboard watching (no 400 ms polling). In-memory search index with pre-normalised text. List views only draw visible rows. Full text loads lazily for the preview pane. SQLite in WAL mode with cached prepared statements. Targets in §6. |
| **Robustness** | **Never lose data:** every change is a SQLite transaction; `settings.toml` is written to a temp file then renamed. A corrupt database is backed up and recreated with a notice, never silently wiped. Clipboard "busy" errors retry with backoff. Worker-thread panics are caught and logged, and the tray app keeps running. If the hotkey is already taken, a clear message offers to record a different one. One instance only. |
| **UX appropriateness** | Looks native on each OS (Fluent on Windows, Cupertino on macOS). Uses platform conventions (Cmd on macOS, menu-bar app with no Dock icon). Follows OS light/dark mode and per-monitor DPI scaling. |
| **Accessibility** | Screen-reader labels via AccessKit (built into Slint), full keyboard navigation, visible focus, readable contrast in both themes. |
| **Paste reliability** | Pastes into the window you came from, even when opened from the tray. Uses the right paste keystroke per app (§2.3). Never re-records its own paste as a new copy. |

---

## 1. Decision log

| # | Topic | Decision |
|---|---|---|
| 1 | Where collections live | **The app's own database.** Excel/CSV are only for import/export. No live-linked files. |
| 2 | Snippet fields | **Content** (what gets pasted), **Title**, **Notes**, **Tags** |
| 3 | Editor layout | **List + detail**: collections, then snippets, then a large editor pane |
| 4 | Editor extras | **All four:** save a history item to a collection · edit/delete individual history items · reorder & move/copy snippets · undo/redo |
| 5 | Clipboard content | **Text + rich text (HTML/RTF)**, with rich text **optional via a setting**, as today |
| 6 | Import flow | **Both:** one-click import, plus an "Advanced…" wizard |
| 7 | Export | **All four:** collection → CSV · collection → .xlsx · all collections → one .xlsx · history → CSV/.xlsx |
| 8 | History limits | **Both count and age limits**; whichever is hit first wins |
| 9 | Matching | **Characters in order, better scoring** (nucleo). Spaces ignored, as today. Unicode-correct |
| 10 | Ranking | **Match quality first, recency breaks ties** |
| 11 | Result layout | **One merged list with source badges**, plus `@collection` filter prefix |
| 12 | Popup extras | **Preview pane** · **Paste as plain text** (Shift+Enter) · **Item actions** (save / edit / delete). No Ctrl+1…9 quick paste |
| 13 | UI toolkit | **Slint** |
| 14 | Linux | **Full support on X11; best-effort on Wayland** |
| 15 | Hotkey | **Standard OS hotkey API** (no low-level keyboard hook) |
| 16 | Privacy | **App lists for capture** · **pause recording** · **"all except" or "only these apps" modes** |
| 16b | Password-manager "don't record" flags | **Honoured, as a setting** (on by default **[default]**) |
| 16c | What the app lists control | **Capture and hotkey separately**, each with its own mode and list |
| 17 | Migration from 1.x | **Fresh start**, no automatic or manual migration |
| 18 | Windows | **Installer with admin (UAC) prompt**, installing for all users into Program Files, **+ portable .exe** |
| 18b | macOS | **Unsigned .dmg** (Intel + Apple Silicon) |
| 18c | Linux | **AppImage only** |
| 19 | Repo layout | **Rust replaces Python** in this repo. Python moves to `legacy/` until done, then is deleted. Committed .exe files move to GitHub Releases |
| 20 | Release plan | **One big release** (v2.0.0) when everything here is done |
| 21 | Clicking a popup row | **Single click pastes**, as today |
| 22 | Language | **Rust**, confirmed after a side-by-side memory measurement (§6.1) |

Not chosen (so out of scope for 2.0):
- image/file capture
- database encryption
- signed macOS builds
- `.deb`/`.rpm`
- auto-update
- quick paste by number
- importing data from 1.x

---

## 2. Features in detail

### 2.1 Clipboard capture
- **Text is always captured.** HTML and RTF are captured only when **"Keep formatting"** is on (off by default, as today).
- **Change detection** is event-driven where the OS allows it, instead of today's 400 ms polling (see §4).
- **Deduplication** is by hash of the plain text. Copying something again bumps its `last_copied_at` and `copy_count`.
- **Pasting an item from Shift Paste also bumps its recency.** This fixes today's lost-copy bug: we record the hash of what *we* put on the clipboard and ignore exactly that, instead of a one-shot "ignore next" flag.
- **Size cap:** text above **1 MB** is truncated, and formatted payloads above **2 MB** are dropped. **[default]**
- **Skipped when any of these apply:**
  1. Recording is paused (tray toggle).
  2. The password-manager flag setting is on and the copy carries a "don't record" marker (§4.3).
  3. The app the copy came from is blocked by the **capture app list** (§2.6).

### 2.2 History limits
- **Count limit:** 100 / **500** / 1,000 / 5,000 / Unlimited. **[default: 500]**
- **Age limit:** 7 / 30 / **90** days / Never. **[default: 90]**
- Enforced after every insert, at startup, and when settings change. Collections are never auto-deleted.
- "Clear history" is in the tray menu and the editor's History tab, and asks for confirmation.

### 2.3 Popup
- Opens at the **mouse cursor, on the monitor the cursor is on** **[default]**, flipping above the cursor near the bottom edge.
- It is frameless and always on top. It closes on `Esc` or when it loses focus.
- **Empty query:** shows the most recent history items.
- **Typing:** one **merged, ranked list** of history and collection snippets. Each row shows a badge: `📋 History · 2m ago` or `📁 Work · Bank IFSC`.
- **Filters:**
  - `@work` limits results to the Work collection. `@work` with nothing else lists that collection in its manual order.
  - `#bank` filters by tag. **[default — natural extension of tags]**
- **Keys:**

  | Key | Action |
  |---|---|
  | `↑`/`↓` | move selection |
  | `Enter` | paste (with formatting if it was captured) |
  | `Shift+Enter` | paste as plain text |
  | `Tab` | toggle the **preview pane** (full text of the selected item) |
  | `Ctrl+S` | save the selected history item to a collection (asks which) |
  | `Ctrl+E` | open the item in the editor |
  | `Delete` | remove the selected history item |

  On macOS, Cmd replaces Ctrl.
- **Mouse:** hovering highlights a row; a **single click pastes it**, as today.
- **Paste keystroke per app:**
  - `Ctrl+V` is the default (`Cmd+V` on macOS).
  - Linux terminals (GNOME Terminal, Konsole, Alacritty, kitty, xterm, …) get `Ctrl+Shift+V`, because `Ctrl+V` doesn't paste there.
  - There's a built-in list of known terminals, plus a per-app override in Settings.
  - While Shift Paste sends the keystroke, its own hotkey is briefly switched off so it can't trigger itself.

### 2.4 Search and ranking
- **Matcher:** [`nucleo-matcher`](https://crates.io/crates/nucleo-matcher) (from the Helix editor).
  - The query's characters must appear in order (`1884` finds `18 mm 8 x 4 LL Pro`), and spaces in the query are ignored, the same rule as today.
  - It scores consecutive characters, word starts and case properly.
  - **Unicode case-folding and accent normalisation** fix today's bug where `ÉCOLE` could never be found.
- **Searched fields:** Content and Title for snippets; Content for history.
- **Order:** sort by match score. Among results whose scores are within ~10% of each other, the most recently used or copied comes first.
- **Speed target:** under 10 ms for 5,000 history items plus 20,000 snippets. Searching an in-memory index is fine at this scale.
- **No debounce:** results update on every keystroke. If a search is still running when the next key arrives, the stale one is dropped.

### 2.5 Collections editor (main window)
List + detail layout:

```
┌ Collections ───┬ Snippets ───────────┬ Edit ───────────────────────┐
│ 📋 History     │ 🔎 filter…          │ Title: Bank IFSC             │
│ ▸ Work    (42) │ Bank IFSC           │ Tags : [work ✕] [bank ✕] [+] │
│   Personal (8) │ Home address        │ ┌ Content ─────────────────┐ │
│   Pinned   (5) │ Email signature     │ │ HDFC0001234              │ │
│ + New          │ + Add snippet       │ └──────────────────────────┘ │
│ Import Export  │                     │ Notes: main branch           │
└────────────────┴─────────────────────┴──────────────────────────────┘
```

- **Collections:** create, rename, delete (with confirmation), drag to reorder.
- **Snippets:**
  - add and delete;
  - drag to reorder within a collection;
  - move or copy to another collection (drag onto a collection, or use the context menu);
  - multi-select for bulk delete, move or export.
- **Detail pane:**
  - multi-line Content, Title, Tags as chips (with autocomplete from existing tags), Notes;
  - **autosaves** (debounced).
- **History tab:** the same list view over clipboard history, where you can:
  - edit an item's text,
  - delete items,
  - **"Save to collection…"**,
  - clear all.
- **Undo/redo:** `Ctrl+Z` / `Ctrl+Y` (`Cmd+Z` / `Cmd+Shift+Z` on macOS) for every editor change, including deletes and moves. The undo stack lasts for the session.

### 2.6 App lists (capture and hotkey)
There are two independent settings, each with a **mode** and a **list**:

| Setting | Mode "All apps except…" | Mode "Only these apps…" |
|---|---|---|
| **Clipboard capture** | Record copies from every app but those listed | Record only copies made while a listed app is in front |
| **Popup hotkey** | Hotkey opens the popup everywhere except listed apps | Hotkey opens the popup only in listed apps |

- **App identity:**
  - Windows: executable name (`KeePassXC.exe`)
  - macOS: bundle ID (`com.apple.Safari`), shown with the app name
  - Linux X11: `WM_CLASS` / process name
- **"Add app…"** picks from the apps currently running, and names can also be typed in.
- **Wayland:** the foreground app usually can't be detected.
  - In "All except" mode the lists are not applied.
  - In "Only these" mode, capture and the hotkey stay off for unknown apps (fail safe).
  - Settings shows a note explaining this.
- **Hotkey caveat:** the standard hotkey API reserves the key combination system-wide. Excluded apps don't open the popup, but they also don't receive the keystroke (same as today, by decision 15).

### 2.7 Import (Excel and CSV)
- **Formats:** `.xlsx`, `.xlsm`, `.xls`, `.xlsb`, `.ods` (via `calamine`), and `.csv` / `.tsv` / `.txt` (via `csv`).
  - Encoding comes from the BOM, falling back to UTF-8 and then Windows-1252.
  - The delimiter (`,` `;` tab) is sniffed.
- **One-click import (the default button):**
  - Every non-empty sheet becomes a **new collection** named after the sheet; for CSV, after the file name.
  - Duplicate collection names get a ` (2)` suffix.
  - **Header detection:** if row 1 contains a known column name, row 1 is a header and columns are matched by name. Known names (case-insensitive):
    - `Content` (also `Text`, `Snippet`, `Value`)
    - `Title` (also `Name`)
    - `Tags`
    - `Notes`

    Otherwise there is no header and **column A = Content**.
  - Tags are split on `,` or `;`.
- **"Advanced…" wizard:**
  - sheet picker, "first row is a header" checkbox, a dropdown per column (Content / Title / Tags / Notes / Ignore), and a live preview of the first rows;
  - target: a new collection or an existing one;
  - mode: **Add, skipping duplicates** (same Content in that collection) / **Add all** / **Replace collection**.
- Large files import on a background thread with progress. The import is one DB transaction and fully undoable.

### 2.8 Export
| Export | Output |
|---|---|
| Collection → CSV | UTF-8 **with BOM** (so Excel shows accents, Hindi, etc. correctly), comma-separated, header `Title,Content,Tags,Notes` |
| Collection → .xlsx | One sheet with the same columns, header row in bold, sensible column widths |
| All collections → one .xlsx | One sheet per collection. Sheet names are sanitised (max 31 chars, no `[]:*?/\`). **Re-importing it with one-click restores every collection** |
| History → CSV / .xlsx | `Content, First copied, Last copied, Copy count, Source app` |

### 2.9 Tray menu
**Open Shift Paste** · **Open Editor** · **Pause recording** (checkbox; the tray icon changes while paused) · **Settings** · **Clear history…** · **Quit**.

Left-click opens the popup. Before the popup opens, the app remembers the *previous* foreground app, not the tray or taskbar, so a later paste goes to the right window.

### 2.10 Settings (every one is wired up)
| Group | Setting | Default |
|---|---|---|
| General | Launch at login | On |
| | Theme: System / Light / Dark | System **[default]** |
| Hotkey | Shortcut (key-recorder field, shows a conflict error if registration fails) | `Ctrl+Shift+V` / `Cmd+Shift+V` |
| | Hotkey app list (mode + list) | All except, empty |
| History | Count limit · Age limit | 500 · 90 days |
| | Keep formatting (HTML/RTF) | Off |
| Privacy | Honour password-manager "don't record" flags | On |
| | Capture app list (mode + list) | All except, empty |
| | Pause recording | Off |
| Data | Data folder location (read-only) with an "Open folder" button | — |

- Settings are stored in `settings.toml` in the config folder. **[default]** It is human-readable and easy to back up.
- Changes apply immediately: there is no Save/Cancel mismatch like today.

---

## 3. Architecture

### 3.1 Workspace layout
```
Cargo.toml                    workspace
crates/
  core/        no UI, no OS calls — fully unit-tested
    db.rs          rusqlite + migrations (PRAGMA user_version)
    model.rs       Collection, Snippet, HistoryItem, Tag
    search.rs      nucleo index, filters (@collection, #tag), ranking
    import.rs      calamine + csv, header detection, column mapping
    export.rs      rust_xlsxwriter + csv
    settings.rs    serde + TOML
    undo.rs        command stack for the editor
  platform/    one module per OS behind a common trait
    clipboard.rs   read/write text+HTML+RTF, change events, don't-record flags
    foreground.rs  "which app is in front?"
    paste.rs       restore focus + simulate Ctrl/Cmd+V
    hotkey.rs      global-hotkey; Wayland portal fallback
    autostart.rs   auto-launch
  app/         binary `shiftpaste`
    main.rs        single instance, CLI (--toggle, --editor), wiring
    tray.rs        tray-icon + muda menu
    ui/*.slint     popup, editor, settings, import wizard, about
legacy/        Python 1.x — removed when 2.0 is done
.github/workflows/
  ci.yml       fmt + clippy + tests on windows/macos/ubuntu
  release.yml  on tag: NSIS installer + portable .exe, universal .dmg, AppImage → GitHub Release
```

### 3.2 Crates (candidates, to confirm while building)
| Need | Crate |
|---|---|
| UI | `slint` (winit backend; pick the femtovg or software renderer by measured memory) |
| Tray + menu | `tray-icon`, `muda` |
| Global hotkey | `global-hotkey` (Win/macOS/X11) + `ashpd` for the Wayland GlobalShortcuts portal |
| Clipboard | `clipboard-rs` or `arboard`, plus small per-OS code for change events and flags (§4) |
| Paste keystroke | `enigo` |
| Database | `rusqlite` (bundled SQLite), `rusqlite_migration` |
| Search | `nucleo-matcher` |
| Excel/CSV | `calamine` (read), `rust_xlsxwriter` (write), `csv`, `encoding_rs` |
| Autostart | `auto-launch` |
| Single instance + `--toggle` IPC | `interprocess` (local socket) |
| Paths | `directories` |
| Logging | `tracing` + `tracing-appender` (rolling log file in the data folder; fixes today's "no logs in release builds") |
| OS APIs | `windows` (Win32), `objc2` / `objc2-app-kit` (macOS), `x11rb` (X11) |

### 3.3 Data model (SQLite)
```
collections (id, name UNIQUE, sort_order, created_at, updated_at)
snippets    (id, collection_id → collections ON DELETE CASCADE,
             title, content, notes, sort_order,
             use_count, last_used_at, created_at, updated_at)
tags        (id, name UNIQUE COLLATE NOCASE)
snippet_tags(snippet_id → snippets CASCADE, tag_id → tags CASCADE, PRIMARY KEY both)
history     (id, content, content_hash UNIQUE, html NULL, rtf NULL,
             source_app NULL, copy_count, first_copied_at, last_copied_at)
```
- All timestamps are **UTC Unix milliseconds** (today's are local-time strings).
- The schema version lives in `PRAGMA user_version`, with ordered migrations.

### 3.4 Data locations (new; doesn't touch 1.x data)
`directories::ProjectDirs("com", "infinityhardik", "ShiftPaste")`:
- Windows: `%APPDATA%\infinityhardik\ShiftPaste\` (config) and `%LOCALAPPDATA%\infinityhardik\ShiftPaste\` (data, logs)
- macOS: `~/Library/Application Support/com.infinityhardik.ShiftPaste/`
- Linux: `~/.config/shiftpaste/` and `~/.local/share/shiftpaste/`

### 3.5 Process model
- **One process, one UI thread.** The Slint event loop is the main thread.
- **A clipboard-watcher thread** sends events over a channel.
- **Hotkey events** arrive on the event loop (or the portal thread on Wayland).
- **Import/export** run on a worker thread with progress events.
- On Linux, `tray-icon` needs GTK, so the tray runs its own GTK loop on a separate thread.
- **Single instance:** a second launch (or `shiftpaste --toggle` / `--editor`) messages the running instance over a local socket and exits. `--toggle` is also what Wayland users bind as a system shortcut.

---

## 4. Platform notes

### 4.1 Support matrix
| Capability | Windows | macOS | Linux X11 | Linux Wayland |
|---|---|---|---|---|
| Clipboard change events | `AddClipboardFormatListener` | poll `NSPasteboard.changeCount` (a cheap integer, every ~250 ms) | XFixes selection events | `wlr`/`ext-data-control` where the compositor supports it (KDE, wlroots, recent GNOME); otherwise only when our window is focused |
| Global hotkey | ✅ | ✅ | ✅ | GlobalShortcuts portal (KDE, GNOME 48+, Hyprland) or a system shortcut running `shiftpaste --toggle` |
| Foreground app (for app lists) | ✅ | ✅ | ✅ | ❌ usually unavailable (§2.6) |
| Restore focus + auto-paste | ✅ | ✅ needs Accessibility permission | ✅ | ⚠️ where allowed; otherwise copies and shows "Press Ctrl+V" |
| Tray icon | ✅ | ✅ menu-bar icon | ✅ needs AppIndicator support | same; vanilla GNOME needs the AppIndicator extension (Ubuntu ships it) |
| Launch at login | Registry Run key (HKCU) | LaunchAgent | `~/.config/autostart` | same |

### 4.2 macOS specifics
- Runs as a menu-bar app (`LSUIElement`, no Dock icon).
- First run shows an onboarding sheet asking for **Accessibility** permission, which auto-paste needs. Without it, items are copied and the user presses Cmd+V.

### 4.3 Password-manager "don't record" markers (decision 16b)
| OS | Markers honoured |
|---|---|
| Windows | `ExcludeClipboardContentFromMonitorProcessing`, `Clipboard Viewer Ignore`, `CanIncludeInClipboardHistory` = 0 |
| macOS | `org.nspasteboard.ConcealedType`, `org.nspasteboard.TransientType`, `org.nspasteboard.AutoGeneratedType` |
| Linux | `x-kde-passwordManagerHint` = `secret` |

---

## 5. Build, packaging and release

- **CI** (every push), on Windows, macOS and Ubuntu: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`.
- **Release** (on tag `v*`), built by GitHub Actions and attached to a GitHub Release:
  - **Windows:** an **NSIS installer that asks for admin (UAC)** and installs for all users into `Program Files\Shift Paste`, with a Start-menu entry, optional desktop shortcut and uninstaller. Plus a **portable `.exe`** (zip).
    - Before installing or uninstalling, the installer closes a running Shift Paste.
    - Uninstalling keeps the user's data unless they tick "Also delete my data".
    - The "Launch Shift Paste" checkbox at the end starts the app **as the normal user, not as admin**. A program started from an admin installer would otherwise run with admin rights, and its data could land in the wrong user profile.
    - "Launch at login" stays per user (HKCU) and is controlled from the app's Settings.
  - **macOS:** **unsigned universal `.dmg`** (Intel + Apple Silicon, ad-hoc signed as Apple Silicon requires).
  - **Linux:** **AppImage** (x86_64).
- **Tool:** `cargo-packager`, or plain per-OS scripts if it falls short.
- **Release profile:** `opt-level = "s"` or `3` (whichever measures better), `lto = "thin"`, `codegen-units = 1`, `strip = true`.
  - Panics are left on the default "unwind" setting so a bug in a worker thread is caught and logged without killing the always-running app. This is a robustness-over-size choice.
- **Housekeeping:**
  - Delete `dist/*.exe` from the repo; binaries live in Releases.
  - Remove `shiftpaste.spec` and `setup_script.iss` along with `legacy/` at the end.
  - Rewrite README and HOW_TO_BUILD for 2.0.
- **Credit:** Slint's free license asks for a credit. The About dialog uses Slint's built-in `AboutSlint` widget.

---

## 6. Targets

| Metric | Target |
|---|---|
| Idle memory (tray, popup hidden) | **≤ 40 MB** working set / RSS |
| Memory with popup or editor open | ≤ 80 MB |
| Hotkey → popup visible | ≤ 100 ms |
| Keystroke → results updated | ≤ 16 ms (5k history + 20k snippets) |
| Cold start to tray | ≤ 500 ms |
| Import 10,000-row .xlsx | ≤ 2 s |

These are measured on each OS before release. If Slint's GPU renderer misses the idle target, try the software renderer and/or destroying the popup window while hidden.

### 6.1 Measured baseline (2026-11-16)
The real 1.x app and a minimal Rust + Slint popup spike (frameless 420×480, search box, 20-row list, Slint software renderer, release build) were run on the same Linux virtual display (Xvfb, software rendering). Memory was read from `/proc/self/status`:

| Phase | Python 1.x (PySide6) RSS / private | Rust + Slint spike RSS / private |
|---|---|---|
| Started, popup hidden | 74–75 MB / 28–29 MB | **14 MB / 1 MB** |
| Popup shown | 77–78 MB / 29 MB | **16 MB / 1 MB** |
| Popup hidden again | 77–78 MB / 29 MB | **16 MB / 1 MB** |
| On-disk size | 54 MB .exe (bundled Python + 142 MB of Qt libraries before compression) | **15 MB** single binary (Linux, stripped) |

Caveats:
- The spike has no database, tray, hotkey or clipboard watcher yet. Expect the full app to land around **25–40 MB**. On Linux the tray's GTK dependency adds the most; on Windows the tray is native and nearly free.
- Windows and macOS numbers will differ, so re-measure there (§6 targets).
- The difference comes mostly from dropping the Python interpreter and Qt, not from "Rust being fast".

---

## 7. Known risks and limitations (accepted)

1. **Wayland:**
   - The global hotkey depends on the desktop (portal or a user-bound `--toggle` shortcut).
   - Auto-paste and app detection may be unavailable.
   - Clipboard watching needs data-control support.
2. **Unsigned macOS app:**
   - Gatekeeper warning on first open (right-click → Open).
   - **The Accessibility permission resets after every update**, because the code signature changes. Users must re-tick Shift Paste in System Settings. Signing later fixes this.
3. **Unsigned Windows binaries:** SmartScreen may warn ("More info → Run anyway") until the app builds reputation.
4. **AppImage** needs FUSE 2 (`libfuse2`) on some newer distros.
5. **The hotkey is reserved system-wide** (decision 15). Excluded apps don't receive `Ctrl+Shift+V`, which is "paste as plain text" in browsers and Slack, and "paste" in Linux terminals. **Open question:** whether to add "pass-through", where Shift Paste re-sends the keystroke to excluded apps.
6. **No migration from 1.x** (decision 17). Users re-import their Excel files with the new importer.

---

## 8. Build order (internal; released once as 2.0.0)

1. Workspace and CI skeleton, `core` crate: DB, models, settings, search, with unit tests.
2. `core` import/export (Excel + CSV, header detection, wizard back-end), round-trip tests.
3. `platform` on Windows: clipboard events + flags, foreground app, paste, hotkey, autostart.
4. Slint popup + tray + single instance, end-to-end on Windows.
5. Slint editor (list + detail, history tab, undo/redo, drag reorder), import wizard, settings.
6. `platform` on macOS and Linux X11, then Wayland best-effort.
7. Packaging workflows, memory/speed measurement on all three OSes, docs, remove `legacy/` and `dist/`, tag **v2.0.0**.
