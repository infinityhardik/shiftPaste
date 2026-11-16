//! App runtime: shared state, start-up, clipboard capture, hotkey, tray and
//! the popup controller. Everything here runs on the UI thread; background
//! threads hand events over with [`post`].

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use shiftpaste_core::apps::{paste_keys_for, AppIdentity, PasteKeys};
use shiftpaste_core::library::Library;
use shiftpaste_core::model::{Id, NewClip};
use shiftpaste_core::paths::AppPaths;
use shiftpaste_core::presets::Platform;
use shiftpaste_core::search::{Hit, Kind, Mode, Results};
use shiftpaste_core::settings::{Settings, Theme as ThemeSetting};
use shiftpaste_core::time::{now_ms, time_ago};
use shiftpaste_core::transform::{Joiner, Transform};
use shiftpaste_platform as platform;
use slint::platform::Key;
use slint::{ComponentHandle, ModelRc, SharedString, Timer, VecModel};

use crate::rows::{self, RowTarget};
use crate::ui::{MenuEntry, PopupWindow, Theme};
use crate::{accent_hex, key_labels};

/// Rows fetched per search (the list shows them all; more is just noise).
const RESULT_LIMIT: usize = 60;

thread_local! {
    static APP: RefCell<Option<Rc<App>>> = const { RefCell::new(None) };
}

/// Run `f` with the app (UI thread only).
pub fn with_app(f: impl FnOnce(&Rc<App>)) {
    let app = APP.with(|a| a.borrow().clone());
    if let Some(app) = app {
        f(&app);
    }
}

/// Run `f` on the UI thread from any thread.
pub fn post(f: impl FnOnce(&Rc<App>) + Send + 'static) {
    let _ = slint::invoke_from_event_loop(move || with_app(f));
}

#[derive(Default)]
pub struct PopupState {
    results: Option<Results>,
    targets: Vec<RowTarget>,
    current: Option<usize>,
    picks: Vec<(Kind, Id)>,
    preview: bool,
    menu: Option<PopupMenu>,
    all_favorites: bool,
    focus: Option<platform::foreground::FocusTarget>,
    front_app: Option<AppIdentity>,
    toast_undo: bool,
    toast_timer: Option<Timer>,
    /// macOS manual resize: (edge, cursor at start, size at start), physical px.
    resizing: Option<(i32, (i32, i32), (u32, u32))>,
}

struct PopupMenu {
    title: String,
    entries: Vec<(MenuEntry, Option<MenuAction>)>,
    current: usize,
}

#[derive(Clone, Copy)]
enum MenuAction {
    Transform(Transform),
    Join(Joiner),
    SaveTo(Id),
    SaveToNew,
}

pub struct App {
    pub paths: AppPaths,
    pub lib: RefCell<Library>,
    pub settings: RefCell<Settings>,
    pub popup: PopupWindow,
    pub popup_state: RefCell<PopupState>,
    pub editor: RefCell<Option<crate::editor_ctl::EditorCtl>>,
    pub prefs: RefCell<Option<crate::prefs_ctl::PrefsCtl>>,
    pub importer: RefCell<Option<crate::import_ctl::ImportCtl>>,
    pub hotkeys: RefCell<Option<platform::hotkey::Hotkeys>>,
    pub hotkey_error: RefCell<String>,
    pub tray: RefCell<Option<platform::tray::Tray>>,
    pub want_html: Arc<AtomicBool>,
    /// Text Shift Paste itself just put on the clipboard (not a new copy).
    self_copy: RefCell<Option<String>>,
    watcher: RefCell<Option<platform::clipboard::Watcher>>,
    /// Shown once in the editor (e.g. "your settings file had an error").
    pub notice: RefCell<Option<String>>,
}

impl App {
    /// Build the app and install it as the UI thread's app.
    pub fn start(
        paths: AppPaths,
        lib: Library,
        settings: Settings,
        notice: Option<String>,
    ) -> Rc<App> {
        crate::CREATING_POPUP.store(true, Ordering::SeqCst);
        let popup = PopupWindow::new().expect("popup window");
        crate::CREATING_POPUP.store(false, Ordering::SeqCst);
        let app = Rc::new(App {
            paths,
            lib: RefCell::new(lib),
            want_html: Arc::new(AtomicBool::new(settings.history.keep_formatting)),
            settings: RefCell::new(settings),
            popup,
            popup_state: RefCell::new(PopupState::default()),
            editor: RefCell::new(None),
            prefs: RefCell::new(None),
            importer: RefCell::new(None),
            hotkeys: RefCell::new(None),
            hotkey_error: RefCell::new(String::new()),
            tray: RefCell::new(None),
            self_copy: RefCell::new(None),
            watcher: RefCell::new(None),
            notice: RefCell::new(notice),
        });
        APP.with(|a| *a.borrow_mut() = Some(app.clone()));
        app.wire_popup();
        app.apply_theme();
        app.start_hotkey();
        app.start_tray();
        app.start_clipboard();
        app.apply_launch_at_login();
        let limits = app.settings.borrow().history.limits();
        let _ = app.lib.borrow_mut().enforce_limits(limits);
        // Re-apply the age limit hourly so old items expire while the app runs.
        let hourly = Timer::default();
        hourly.start(
            slint::TimerMode::Repeated,
            Duration::from_secs(3600),
            || {
                with_app(|app| {
                    let limits = app.settings.borrow().history.limits();
                    let _ = app.lib.borrow_mut().enforce_limits(limits);
                })
            },
        );
        std::mem::forget(hourly);
        app
    }

    // ------------------------------------------------------------- settings

    pub fn save_settings(&self) {
        let path = self.paths.settings_file();
        if let Err(e) = self.settings.borrow().save(&path) {
            tracing::error!("couldn't save settings: {e}");
        }
    }

    pub fn is_dark(&self) -> bool {
        match self.settings.borrow().general.theme {
            ThemeSetting::Light => false,
            ThemeSetting::Dark => true,
            ThemeSetting::System => matches!(dark_light::detect(), Ok(dark_light::Mode::Dark)),
        }
    }

    /// Push the light/dark choice to every open window.
    pub fn apply_theme(&self) {
        let dark = self.is_dark();
        self.popup.global::<Theme>().set_dark(dark);
        if let Some(e) = self.editor.borrow().as_ref() {
            e.window.global::<Theme>().set_dark(dark);
        }
        if let Some(p) = self.prefs.borrow().as_ref() {
            p.window.global::<Theme>().set_dark(dark);
        }
        if let Some(i) = self.importer.borrow().as_ref() {
            i.window.global::<Theme>().set_dark(dark);
        }
    }

    pub fn apply_launch_at_login(&self) {
        let on = self.settings.borrow().general.launch_at_login;
        if let Err(e) = platform::autostart::set(on) {
            tracing::warn!("launch at login: {e}");
        }
    }

    pub fn set_paused(&self, paused: bool) {
        self.settings.borrow_mut().privacy.recording_paused = paused;
        self.save_settings();
        if let Some(t) = self.tray.borrow().as_ref() {
            t.set_paused(paused);
        }
        self.popup.set_paused(paused);
        if let Some(p) = self.prefs.borrow().as_ref() {
            p.window.set_recording_paused(paused);
        }
    }

    // ---------------------------------------------------------------- hotkey

    fn start_hotkey(self: &Rc<Self>) {
        match platform::hotkey::Hotkeys::new() {
            Ok(h) => *self.hotkeys.borrow_mut() = Some(h),
            Err(e) => {
                tracing::error!("global hotkeys unavailable: {e}");
                *self.hotkey_error.borrow_mut() =
                    format!("Global shortcuts aren't available here ({e}).");
                return;
            }
        }
        let shortcut = self.settings.borrow().hotkey.shortcut.clone();
        self.register_shortcut(&shortcut);
        platform::hotkey::on_pressed(|id| post(move |app| app.on_hotkey(id)));
        // Wayland: apps can't grab keys themselves; ask the desktop portal.
        #[cfg(target_os = "linux")]
        if platform::is_wayland() {
            platform::wayland_shortcut::bind(&shortcut, || {
                post(|app| {
                    if app.popup.window().is_visible() {
                        app.hide_popup();
                    } else {
                        app.open_popup_for(None);
                    }
                })
            });
        }
    }

    /// Register a shortcut; errors are kept for the Settings window.
    pub fn register_shortcut(&self, shortcut: &str) -> bool {
        let result = match self.hotkeys.borrow_mut().as_mut() {
            Some(h) => h.set(shortcut).map_err(|e| e.to_string()),
            None => Err("Global shortcuts aren't available on this system.".into()),
        };
        let ok = result.is_ok();
        *self.hotkey_error.borrow_mut() = result.err().unwrap_or_default();
        ok
    }

    fn on_hotkey(self: &Rc<Self>, id: u32) {
        if self.hotkeys.borrow().as_ref().and_then(|h| h.id()) != Some(id) {
            return;
        }
        if self.popup.window().is_visible() {
            self.hide_popup();
            return;
        }
        let front = platform::foreground::current_app().map(to_identity);
        let allowed = self.settings.borrow().hotkey.apps.allows(front.as_ref());
        if allowed {
            self.open_popup_for(front);
        } else if let Some(h) = self.hotkeys.borrow_mut().as_mut() {
            // Give the app in front its own shortcut back.
            h.pass_through();
        }
    }

    // ----------------------------------------------------------------- tray

    fn start_tray(&self) {
        let paused = self.settings.borrow().privacy.recording_paused;
        let tray = platform::tray::Tray::create(paused, |cmd| {
            post(move |app| {
                use platform::tray::TrayCommand as T;
                match cmd {
                    T::Open => {
                        // The taskbar is in front right now; use the app the
                        // user came from (Windows skips shell windows).
                        let front = platform::foreground::remember()
                            .and_then(platform::foreground::app_of)
                            .or_else(platform::foreground::current_app)
                            .map(to_identity);
                        app.open_popup_for(front);
                    }
                    T::Editor => crate::editor_ctl::open_editor(app, None),
                    T::TogglePause => {
                        let paused = !app.settings.borrow().privacy.recording_paused;
                        app.set_paused(paused);
                    }
                    T::Settings => crate::prefs_ctl::open_settings(app, None),
                    T::ClearHistory => crate::editor_ctl::confirm_clear_history(app),
                    T::Quit => app.quit(),
                }
            })
        });
        match tray {
            Ok(t) => *self.tray.borrow_mut() = Some(t),
            Err(e) => tracing::warn!("tray icon unavailable: {e}"),
        }
    }

    pub fn quit(&self) {
        self.watcher.borrow_mut().take();
        let _ = slint::quit_event_loop();
    }

    // ------------------------------------------------------------ clipboard

    fn start_clipboard(&self) {
        let watcher = platform::clipboard::Watcher::spawn(self.want_html.clone(), |change| {
            post(move |app| app.on_clipboard(change));
        });
        *self.watcher.borrow_mut() = Some(watcher);
    }

    fn on_clipboard(self: &Rc<Self>, change: platform::clipboard::Change) {
        // Our own paste: it was already recorded as "used", not a new copy.
        if self.self_copy.borrow().as_deref() == Some(change.text.as_str()) {
            self.self_copy.borrow_mut().take();
            return;
        }
        let (paused, honour, capture, limits, keep_html) = {
            let s = self.settings.borrow();
            (
                s.privacy.recording_paused,
                s.privacy.honour_password_flags,
                s.privacy.capture.clone(),
                s.history.limits(),
                s.history.keep_formatting,
            )
        };
        if paused || (change.concealed && honour) {
            return;
        }
        let source = change.source.map(to_identity);
        if !capture.allows(source.as_ref()) {
            return;
        }
        let clip = NewClip {
            text: change.text,
            html: if keep_html { change.html } else { None },
            rtf: None,
            source_app: source.map(|s| s.display_name),
        };
        if let Err(e) = self.lib.borrow_mut().record_copy(&clip, limits) {
            tracing::error!("couldn't save a copy: {e}");
            return;
        }
        if self.popup.window().is_visible() {
            self.refresh_popup(true);
        }
        crate::editor_ctl::history_changed(self);
    }

    // ---------------------------------------------------------------- popup

    fn wire_popup(self: &Rc<Self>) {
        let p = &self.popup;
        p.set_keys(key_labels());
        p.set_paused(self.settings.borrow().privacy.recording_paused);
        p.on_query_edited(|_| {
            with_app(|app| {
                app.popup_state.borrow_mut().all_favorites = false;
                app.refresh_popup(false);
            })
        });
        p.on_handle_key(|key, ctrl, shift, alt| {
            let mut handled = false;
            with_app(|app| handled = app.popup_key(&key, ctrl, shift, alt));
            handled
        });
        p.on_row_clicked(|row, ctrl| {
            with_app(|app| {
                let row = row as usize;
                if ctrl {
                    app.select_row(row);
                    app.toggle_pick();
                } else {
                    app.select_row(row);
                    app.activate(false, None);
                }
            })
        });
        p.on_row_hovered(|row| {
            with_app(|app| {
                if app.popup_state.borrow().menu.is_none() {
                    app.select_row(row as usize);
                }
            })
        });
        p.on_menu_clicked(|i| {
            with_app(|app| {
                if i < 0 {
                    app.close_menu();
                } else {
                    if let Some(m) = app.popup_state.borrow_mut().menu.as_mut() {
                        m.current = i as usize;
                    }
                    app.run_menu_choice();
                }
            })
        });
        p.on_menu_hovered(|i| {
            with_app(|app| {
                if let Some(m) = app.popup_state.borrow_mut().menu.as_mut() {
                    m.current = i as usize;
                }
                app.sync_menu();
            })
        });
        p.on_open_settings(|| {
            with_app(|app| {
                app.hide_popup();
                crate::prefs_ctl::open_settings(app, None);
            })
        });
        p.on_resume_recording(|| with_app(|app| app.set_paused(false)));
        p.on_toast_clicked(|| with_app(|app| app.undo_from_popup()));
        p.set_resize_all_edges(!cfg!(target_os = "macos"));
        p.on_resize_start(|dir| with_app(|app| app.resize_start(dir)));
        p.on_resize_move(|| with_app(|app| app.resize_move()));
        p.on_resize_end(|| with_app(|app| app.resize_end()));
        p.on_focus_lost(|| {
            with_app(|app| {
                // Clicking elsewhere closes the popup (but not while it is being opened).
                if app.popup.window().is_visible() {
                    app.hide_popup();
                }
            })
        });
    }

    pub fn open_popup_for(self: &Rc<Self>, front: Option<AppIdentity>) {
        if self.popup.window().is_visible() {
            return;
        }
        {
            let mut st = self.popup_state.borrow_mut();
            st.focus = platform::foreground::remember();
            st.front_app = front;
            st.picks.clear();
            st.menu = None;
            st.preview = false;
            st.all_favorites = false;
            st.current = None;
        }
        self.popup.set_query(SharedString::new());
        self.popup.set_preview_open(false);
        self.popup.set_menu_open(false);
        self.popup.set_toast_text(SharedString::new());
        self.refresh_popup(false);
        self.popup.invoke_scroll_to_top();
        self.place_popup();
        if let Err(e) = self.popup.show() {
            tracing::error!("couldn't show the popup: {e}");
            return;
        }
        self.focus_popup();
    }

    /// Size from settings; position next to the mouse, inside its monitor.
    fn place_popup(&self) {
        let (w, h) = self.settings.borrow().popup.size();
        let window = self.popup.window();
        window.set_size(slint::LogicalSize::new(w as f32, h as f32));
        if let Some((x, y)) = platform::cursor::position() {
            window.set_position(slint::PhysicalPosition::new(x - 40, y + 16));
        }
    }

    fn focus_popup(&self) {
        use slint::winit_030::WinitWindowAccessor;
        let cursor = platform::cursor::position();
        self.popup.window().with_winit_window(|w| {
            // Keep the whole popup on the monitor the mouse is on.
            let size = w.outer_size();
            let monitors: Vec<_> = w.available_monitors().collect();
            let here = cursor.and_then(|(cx, cy)| {
                monitors.iter().find(|m| {
                    let (p, s) = (m.position(), m.size());
                    cx >= p.x
                        && cy >= p.y
                        && cx < p.x + s.width as i32
                        && cy < p.y + s.height as i32
                })
            });
            if let Some(m) = here.or(monitors.first()) {
                let (p, s) = (m.position(), m.size());
                let (cx, cy) =
                    cursor.unwrap_or((p.x + s.width as i32 / 2, p.y + s.height as i32 / 3));
                let margin = 12;
                let mut x = cx - 40;
                let mut y = cy + 16;
                if y + size.height as i32 > p.y + s.height as i32 - margin {
                    y = cy - size.height as i32 - 12; // flip above the cursor
                }
                x = x.clamp(
                    p.x + margin,
                    (p.x + s.width as i32 - size.width as i32 - margin).max(p.x + margin),
                );
                y = y.clamp(
                    p.y + margin,
                    (p.y + s.height as i32 - size.height as i32 - margin).max(p.y + margin),
                );
                w.set_outer_position(slint::winit_030::winit::dpi::PhysicalPosition::new(x, y));
            }
            // No taskbar button / Alt+Tab entry, rounded corners (Windows).
            #[cfg(windows)]
            {
                use slint::winit_030::winit::platform::windows::WindowExtWindows;
                use slint::winit_030::winit::raw_window_handle::{
                    HasWindowHandle, RawWindowHandle,
                };
                w.set_skip_taskbar(true);
                if let Ok(handle) = w.window_handle() {
                    if let RawWindowHandle::Win32(h) = handle.as_raw() {
                        platform::window::polish_popup(h.hwnd.get());
                    }
                }
            }
            w.focus_window();
        });
        self.popup.invoke_focus_search();
    }

    /// Start resizing from an edge. Windows/Linux: the OS drives the resize.
    /// macOS (no OS support for borderless windows): follow the mouse ourselves.
    fn resize_start(&self, dir: i32) {
        use slint::winit_030::winit::window::ResizeDirection as R;
        use slint::winit_030::WinitWindowAccessor;
        let direction = match dir {
            0 => R::North,
            1 => R::South,
            2 => R::East,
            3 => R::West,
            4 => R::NorthEast,
            5 => R::NorthWest,
            6 => R::SouthEast,
            _ => R::SouthWest,
        };
        let mut native = false;
        if !cfg!(target_os = "macos") {
            self.popup
                .window()
                .with_winit_window(|w| native = w.drag_resize_window(direction).is_ok());
        }
        if !native {
            if let Some(cursor) = platform::cursor::position() {
                let size = self.popup.window().size();
                self.popup_state.borrow_mut().resizing =
                    Some((dir, cursor, (size.width, size.height)));
            }
        }
    }

    fn resize_move(&self) {
        let Some((dir, (cx, cy), (w, h))) = self.popup_state.borrow().resizing else {
            return;
        };
        let Some((x, y)) = platform::cursor::position() else {
            return;
        };
        let grow_x = matches!(dir, 2 | 4 | 6);
        let grow_y = matches!(dir, 1 | 6 | 7);
        let scale = self.popup.window().scale_factor();
        let min_w = (shiftpaste_core::settings::MIN_POPUP_WIDTH as f32 * scale) as i32;
        let min_h = (shiftpaste_core::settings::MIN_POPUP_HEIGHT as f32 * scale) as i32;
        let new_w = if grow_x {
            (w as i32 + x - cx).max(min_w)
        } else {
            w as i32
        };
        let new_h = if grow_y {
            (h as i32 + y - cy).max(min_h)
        } else {
            h as i32
        };
        self.popup
            .window()
            .set_size(slint::PhysicalSize::new(new_w as u32, new_h as u32));
    }

    fn resize_end(&self) {
        self.popup_state.borrow_mut().resizing = None;
    }

    pub fn hide_popup(&self) {
        if !self.popup.window().is_visible() {
            return;
        }
        // Remember a resized popup.
        let scale = self.popup.window().scale_factor();
        let size = self.popup.window().size().to_logical(scale);
        let preview = self.popup_state.borrow().preview;
        if !preview {
            let mut s = self.settings.borrow_mut();
            let (w, h) = (size.width.round() as u32, size.height.round() as u32);
            if (w, h) != (s.popup.width, s.popup.height) && w >= 300 && h >= 200 {
                s.popup.width = w;
                s.popup.height = h;
                drop(s);
                self.save_settings();
            }
        }
        let _ = self.popup.hide();
        let mut st = self.popup_state.borrow_mut();
        st.toast_timer = None;
        st.toast_undo = false;
    }

    /// Re-run the search. `keep` keeps the selection on the same item.
    pub fn refresh_popup(&self, keep: bool) {
        let query = self.popup.get_query().to_string();
        let dark = self.is_dark();
        let mut st = self.popup_state.borrow_mut();
        let kept = if keep { selected_key(&st) } else { None };
        let results = {
            let mut lib = self.lib.borrow_mut();
            if st.all_favorites && query.trim().is_empty() {
                lib.favorites()
            } else {
                lib.search(&query, RESULT_LIMIT)
            }
        };
        let picks = st.picks.clone();
        let built = rows::build(&results, now_ms(), accent_hex(dark), |h| {
            picks
                .iter()
                .position(|p| *p == (h.kind, h.id))
                .map_or(0, |i| i as i32 + 1)
        });
        let first = built.targets.iter().position(|t| t.is_selectable());
        let current = kept
            .and_then(|k| {
                built.targets.iter().position(|t| {
                    matches!(t, RowTarget::Hit(i) if (results.hits[*i].kind, results.hits[*i].id) == k)
                })
            })
            .or(first);
        self.popup
            .set_rows(ModelRc::new(VecModel::from(built.rows)));
        self.popup.set_current(current.map_or(-1, |c| c as i32));
        self.popup
            .set_count_text(count_text(&results, &self.lib.borrow()).into());
        let (title, message) =
            empty_texts(&results, &query, &self.settings.borrow().hotkey.shortcut);
        self.popup.set_empty_title(title.into());
        self.popup.set_empty_message(message.into());
        self.popup.set_picked_count(st.picks.len() as i32);
        st.targets = built.targets;
        st.results = Some(results);
        st.current = current;
        drop(st);
        self.update_preview();
        if let Some(c) = current {
            self.reveal(c);
        }
    }

    fn select_row(&self, row: usize) {
        let mut st = self.popup_state.borrow_mut();
        if !st.targets.get(row).is_some_and(|t| t.is_selectable()) || st.current == Some(row) {
            return;
        }
        st.current = Some(row);
        drop(st);
        self.popup.set_current(row as i32);
        self.update_preview();
    }

    fn move_selection(&self, delta: i32) {
        let st = self.popup_state.borrow();
        let selectable: Vec<usize> = st
            .targets
            .iter()
            .enumerate()
            .filter(|(_, t)| t.is_selectable())
            .map(|(i, _)| i)
            .collect();
        let Some(pos) = st
            .current
            .and_then(|c| selectable.iter().position(|&s| s == c))
        else {
            return;
        };
        let next = (pos as i32 + delta).clamp(0, selectable.len() as i32 - 1) as usize;
        drop(st);
        self.select_row(selectable[next]);
        self.reveal(selectable[next]);
    }

    /// Scroll so a row is visible (rows have fixed heights per kind).
    fn reveal(&self, row: usize) {
        let st = self.popup_state.borrow();
        let height = |t: &RowTarget| match t {
            RowTarget::Section => 28.0,
            RowTarget::MoreFavorites => 34.0,
            RowTarget::Hit(_) => 54.0,
        };
        let top: f32 = st.targets.iter().take(row).map(height).sum();
        let h = st.targets.get(row).map_or(54.0, height);
        drop(st);
        self.popup.invoke_reveal_row(top, h);
    }

    fn current_hit(&self) -> Option<Hit> {
        let st = self.popup_state.borrow();
        match st.targets.get(st.current?)? {
            RowTarget::Hit(i) => st.results.as_ref()?.hits.get(*i).cloned(),
            _ => None,
        }
    }

    fn toggle_pick(&self) {
        let Some(hit) = self.current_hit() else {
            return;
        };
        let key = (hit.kind, hit.id);
        {
            let mut st = self.popup_state.borrow_mut();
            match st.picks.iter().position(|p| *p == key) {
                Some(i) => {
                    st.picks.remove(i);
                }
                None => st.picks.push(key),
            }
        }
        self.refresh_popup(true);
    }

    fn update_preview(&self) {
        let open = self.popup_state.borrow().preview;
        self.popup.set_preview_open(open);
        if !open {
            return;
        }
        let Some(hit) = self.current_hit() else {
            self.popup.set_preview_title(SharedString::new());
            self.popup.set_preview_body(SharedString::new());
            self.popup.set_preview_meta(SharedString::new());
            return;
        };
        let lib = self.lib.borrow();
        let now = now_ms();
        let (kind, body, meta) = match hit.kind {
            Kind::History => match lib.history_item(hit.id) {
                Ok(h) => {
                    let mut meta =
                        format!("Copied {}", time_ago(h.last_copied_at, now).to_lowercase());
                    if h.copy_count > 1 {
                        meta += &format!(" · {} times", h.copy_count);
                    }
                    if let Some(app) = &h.source_app {
                        meta += &format!(" · from {app}");
                    }
                    if h.has_formatting() {
                        meta += " · has formatting";
                    }
                    ("History".to_string(), h.content, meta)
                }
                Err(_) => return,
            },
            Kind::Snippet => match lib.snippet(hit.id) {
                Ok(s) => {
                    let mut meta = hit.collection.clone().unwrap_or_default();
                    if !s.tags.is_empty() {
                        meta += &format!(" · #{}", s.tags.join(" #"));
                    }
                    if s.use_count > 0 {
                        meta += &format!(
                            " · pasted {} time{}",
                            s.use_count,
                            if s.use_count == 1 { "" } else { "s" }
                        );
                    }
                    ("Snippet".to_string(), s.content, meta)
                }
                Err(_) => return,
            },
        };
        self.popup.set_preview_kind(kind.into());
        self.popup.set_preview_title(hit.title.as_str().into());
        self.popup.set_preview_body(body.into());
        self.popup.set_preview_meta(meta.into());
    }

    fn toggle_preview(&self) {
        let open = {
            let mut st = self.popup_state.borrow_mut();
            st.preview = !st.preview;
            st.preview
        };
        let (w, h) = self.settings.borrow().popup.size();
        let extra = if open {
            self.popup.get_preview_width()
        } else {
            0.0
        };
        self.popup
            .window()
            .set_size(slint::LogicalSize::new(w as f32 + extra, h as f32));
        self.update_preview();
    }

    fn popup_key(self: &Rc<Self>, key: &str, ctrl: bool, shift: bool, alt: bool) -> bool {
        let Some(k) = key.chars().next() else {
            return false;
        };
        let is = |special: Key| k == char::from(special);

        if self.popup_state.borrow().menu.is_some() {
            if is(Key::UpArrow) {
                self.menu_step(-1);
            } else if is(Key::DownArrow) {
                self.menu_step(1);
            } else if is(Key::Return) {
                self.run_menu_choice();
            } else if is(Key::Escape) {
                self.close_menu();
            } else if let Some(d) = k.to_digit(10) {
                self.menu_by_number(d as usize);
            }
            return true; // the menu is modal
        }

        if is(Key::UpArrow) {
            self.move_selection(-1);
        } else if is(Key::DownArrow) {
            self.move_selection(1);
        } else if is(Key::PageUp) {
            self.move_selection(-8);
        } else if is(Key::PageDown) {
            self.move_selection(8);
        } else if is(Key::Return) {
            if alt {
                self.open_transform_menu();
            } else {
                self.activate(shift, None);
            }
        } else if is(Key::Tab) || is(Key::Backtab) {
            self.toggle_preview();
        } else if is(Key::Escape) {
            let picked = !self.popup_state.borrow().picks.is_empty();
            if picked {
                self.popup_state.borrow_mut().picks.clear();
                self.refresh_popup(true);
            } else if !self.popup.get_query().is_empty() {
                self.popup.set_query(SharedString::new());
                self.popup_state.borrow_mut().all_favorites = false;
                self.refresh_popup(false);
            } else {
                self.hide_popup();
            }
        } else if ctrl && k == ' ' {
            self.toggle_pick();
        } else if ctrl && k.eq_ignore_ascii_case(&'d') {
            self.star_current();
        } else if ctrl && k.eq_ignore_ascii_case(&'s') {
            self.open_save_menu();
        } else if ctrl && k.eq_ignore_ascii_case(&'e') {
            let hit = self.current_hit();
            self.hide_popup();
            crate::editor_ctl::open_editor(self, hit.map(|h| (h.kind, h.id)));
        } else if ctrl && k == ',' {
            self.hide_popup();
            crate::prefs_ctl::open_settings(self, None);
        } else if ctrl && k.eq_ignore_ascii_case(&'z') && self.popup_state.borrow().toast_undo {
            self.undo_from_popup();
        } else if (is(Key::Delete) && self.popup.get_query().is_empty())
            || (ctrl && (is(Key::Delete) || is(Key::Backspace)))
        {
            self.delete_current();
        } else {
            return false;
        }
        true
    }

    fn star_current(&self) {
        let Some(hit) = self.current_hit() else {
            return;
        };
        let result = match hit.kind {
            Kind::Snippet => self
                .lib
                .borrow_mut()
                .set_starred(&[hit.id], !hit.starred)
                .map(|_| {
                    if hit.starred {
                        "Removed from favorites"
                    } else {
                        "Added to favorites"
                    }
                }),
            Kind::History => self
                .lib
                .borrow_mut()
                .save_history_to_collection(hit.id, None, true)
                .map(|_| "Saved to Favorites"),
        };
        match result {
            Ok(msg) => {
                self.refresh_popup(true);
                self.toast(msg, true);
                crate::editor_ctl::library_changed(self);
            }
            Err(e) => self.toast(&e.to_string(), false),
        }
    }

    fn delete_current(&self) {
        let Some(hit) = self.current_hit() else {
            return;
        };
        if hit.kind != Kind::History {
            self.toast("Snippets are deleted in the editor (Ctrl+E).", false);
            return;
        }
        if self.lib.borrow_mut().delete_history(&[hit.id]).is_ok() {
            self.refresh_popup(false);
            self.toast("Item deleted", true);
            crate::editor_ctl::history_changed(self);
        }
    }

    fn undo_from_popup(&self) {
        let label = self.lib.borrow_mut().undo();
        if let Ok(Some(label)) = label {
            self.refresh_popup(true);
            self.toast(&format!("Undone: {label}"), false);
            crate::editor_ctl::library_changed(self);
        }
    }

    fn toast(&self, text: &str, undo: bool) {
        self.popup.set_toast_text(text.into());
        self.popup
            .set_toast_action(if undo { "Undo" } else { "" }.into());
        let timer = Timer::default();
        timer.start(slint::TimerMode::SingleShot, Duration::from_secs(5), || {
            with_app(|app| {
                app.popup.set_toast_text(SharedString::new());
                app.popup_state.borrow_mut().toast_undo = false;
            })
        });
        let mut st = self.popup_state.borrow_mut();
        st.toast_undo = undo;
        st.toast_timer = Some(timer);
    }

    // ----------------------------------------------------------------- menus

    fn open_transform_menu(&self) {
        let picks = self.popup_state.borrow().picks.len();
        let n = if picks > 0 { picks } else { 1 };
        if self.current_hit().is_none() && picks == 0 {
            return;
        }
        let mut entries = Vec::new();
        let section = |label: &str| {
            (
                MenuEntry {
                    label: label.into(),
                    shortcut: SharedString::new(),
                    section: true,
                },
                None,
            )
        };
        let mut num = 0;
        let mut item = |label: &str, action: MenuAction| {
            num += 1;
            (
                MenuEntry {
                    label: label.into(),
                    shortcut: num.to_string().into(),
                    section: false,
                },
                Some(action),
            )
        };
        if picks > 1 {
            entries.push(section("Join"));
            for j in Joiner::ALL {
                entries.push(item(j.label(), MenuAction::Join(j)));
            }
            entries.push(section("Then change case"));
        }
        for t in Transform::ALL {
            entries.push(item(t.label(), MenuAction::Transform(t)));
        }
        let title = if n > 1 {
            format!("Paste {n} items as")
        } else {
            "Paste as".to_string()
        };
        self.open_menu(title, entries);
    }

    fn open_save_menu(&self) {
        let Some(hit) = self.current_hit() else {
            return;
        };
        if hit.kind != Kind::History {
            self.toast(
                "This is already a snippet. Use the editor to copy it elsewhere.",
                false,
            );
            return;
        }
        let collections = self.lib.borrow().collections().unwrap_or_default();
        let mut entries: Vec<_> = collections
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let shortcut = if i < 9 {
                    (i + 1).to_string()
                } else {
                    String::new()
                };
                (
                    MenuEntry {
                        label: c.name.as_str().into(),
                        shortcut: shortcut.into(),
                        section: false,
                    },
                    Some(MenuAction::SaveTo(c.id)),
                )
            })
            .collect();
        entries.push((
            MenuEntry {
                label: "New collection…".into(),
                shortcut: SharedString::new(),
                section: false,
            },
            Some(MenuAction::SaveToNew),
        ));
        self.open_menu("Save to collection".into(), entries);
    }

    fn open_menu(&self, title: String, entries: Vec<(MenuEntry, Option<MenuAction>)>) {
        let current = entries.iter().position(|(_, a)| a.is_some()).unwrap_or(0);
        self.popup_state.borrow_mut().menu = Some(PopupMenu {
            title,
            entries,
            current,
        });
        self.sync_menu();
    }

    fn sync_menu(&self) {
        let st = self.popup_state.borrow();
        match &st.menu {
            Some(m) => {
                self.popup.set_menu_title(m.title.as_str().into());
                let items: Vec<MenuEntry> = m.entries.iter().map(|(e, _)| e.clone()).collect();
                self.popup
                    .set_menu_items(ModelRc::new(VecModel::from(items)));
                self.popup.set_menu_current(m.current as i32);
                self.popup.set_menu_open(true);
            }
            None => self.popup.set_menu_open(false),
        }
    }

    fn close_menu(&self) {
        self.popup_state.borrow_mut().menu = None;
        self.sync_menu();
    }

    fn menu_step(&self, delta: i32) {
        if let Some(m) = self.popup_state.borrow_mut().menu.as_mut() {
            let n = m.entries.len() as i32;
            let mut i = m.current as i32;
            for _ in 0..n {
                i = (i + delta).rem_euclid(n);
                if m.entries[i as usize].1.is_some() {
                    break;
                }
            }
            m.current = i as usize;
        }
        self.sync_menu();
    }

    fn menu_by_number(&self, n: usize) {
        let idx = self.popup_state.borrow().menu.as_ref().and_then(|m| {
            m.entries
                .iter()
                .position(|(e, _)| e.shortcut.as_str() == n.to_string())
        });
        if let Some(i) = idx {
            if let Some(m) = self.popup_state.borrow_mut().menu.as_mut() {
                m.current = i;
            }
            self.run_menu_choice();
        }
    }

    fn run_menu_choice(&self) {
        let action = {
            let st = self.popup_state.borrow();
            st.menu
                .as_ref()
                .and_then(|m| m.entries.get(m.current).and_then(|(_, a)| *a))
        };
        self.close_menu();
        let Some(action) = action else { return };
        match action {
            MenuAction::Transform(t) => self.activate(true, Some((t, Joiner::NewLine))),
            MenuAction::Join(j) => {
                // Joining offers case changes too: a second menu would be noise,
                // so join pastes directly; case transforms apply per item.
                self.activate_join(j);
            }
            MenuAction::SaveTo(cid) => self.save_current_to(Some(cid)),
            MenuAction::SaveToNew => {
                let name = self
                    .lib
                    .borrow()
                    .store()
                    .unique_collection_name("New collection")
                    .unwrap_or_default();
                let created = self.lib.borrow_mut().create_collection(&name);
                if let Ok(c) = created {
                    self.save_current_to(Some(c.id));
                }
            }
        }
    }

    fn save_current_to(&self, cid: Option<Id>) {
        let Some(hit) = self.current_hit() else {
            return;
        };
        let saved = self
            .lib
            .borrow_mut()
            .save_history_to_collection(hit.id, cid, false);
        match saved {
            Ok(s) => {
                let name = self
                    .lib
                    .borrow()
                    .store()
                    .collection(s.collection_id)
                    .map(|c| c.name)
                    .unwrap_or_default();
                self.toast(&format!("Saved to “{name}”"), true);
                crate::editor_ctl::library_changed(self);
            }
            Err(e) => self.toast(&e.to_string(), false),
        }
    }

    // ----------------------------------------------------------------- paste

    /// Paste the picked items (or the selected one). `plain` drops formatting;
    /// `transform` applies a case/whitespace change.
    fn activate(&self, plain: bool, transform: Option<(Transform, Joiner)>) {
        let hits = self.hits_to_paste();
        if hits.is_empty() {
            // "Show all favorites…" row.
            let st = self.popup_state.borrow();
            let is_more =
                st.current.and_then(|c| st.targets.get(c)) == Some(&RowTarget::MoreFavorites);
            drop(st);
            if is_more {
                self.popup_state.borrow_mut().all_favorites = true;
                self.refresh_popup(false);
            }
            return;
        }
        let joiner = transform.map_or(Joiner::NewLine, |(_, j)| j);
        let Some((mut text, html)) = self.collect_text(&hits, joiner) else {
            return;
        };
        if let Some((t, _)) = transform {
            text = t.apply(&text);
        }
        let html = if plain || transform.is_some() || hits.len() > 1 {
            None
        } else {
            html
        };
        self.paste_text(text, html, hits.iter().map(|h| (h.kind, h.id)).collect());
    }

    fn activate_join(&self, joiner: Joiner) {
        let hits = self.hits_to_paste();
        if let Some((text, _)) = self.collect_text(&hits, joiner) {
            self.paste_text(text, None, hits.iter().map(|h| (h.kind, h.id)).collect());
        }
    }

    fn hits_to_paste(&self) -> Vec<Hit> {
        let st = self.popup_state.borrow();
        let Some(results) = st.results.as_ref() else {
            return Vec::new();
        };
        if !st.picks.is_empty() {
            // In pick order, even if some picks scrolled out of the results.
            let lib = self.lib.borrow();
            return st
                .picks
                .iter()
                .filter_map(|&(kind, id)| {
                    results
                        .hits
                        .iter()
                        .find(|h| (h.kind, h.id) == (kind, id))
                        .cloned()
                        .or_else(|| {
                            let exists = match kind {
                                Kind::History => lib.history_item(id).is_ok(),
                                Kind::Snippet => lib.snippet(id).is_ok(),
                            };
                            exists.then(|| Hit {
                                kind,
                                id,
                                collection_id: None,
                                collection: None,
                                title: String::new(),
                                title_matches: vec![],
                                preview: String::new(),
                                preview_matches: vec![],
                                tags: vec![],
                                starred: false,
                                timestamp: 0,
                                has_formatting: false,
                            })
                        })
                })
                .collect();
        }
        drop(st);
        self.current_hit().into_iter().collect()
    }

    fn collect_text(&self, hits: &[Hit], joiner: Joiner) -> Option<(String, Option<String>)> {
        let lib = self.lib.borrow();
        let mut texts = Vec::new();
        let mut html = None;
        for h in hits {
            match h.kind {
                Kind::History => {
                    let item = lib.history_item(h.id).ok()?;
                    html = item.html.clone();
                    texts.push(item.content);
                }
                Kind::Snippet => texts.push(lib.snippet(h.id).ok()?.content),
            }
        }
        let text = if texts.len() == 1 {
            texts.pop()?
        } else {
            joiner.join(&texts)
        };
        Some((text, html))
    }

    /// Put `text` on the clipboard, close the popup, give focus back and paste.
    pub fn paste_text(&self, text: String, html: Option<String>, used: Vec<(Kind, Id)>) {
        *self.self_copy.borrow_mut() = Some(text.clone());
        if let Err(e) = platform::clipboard::write(&text, html.as_deref()) {
            self.toast(&format!("Couldn't use the clipboard: {e}"), false);
            return;
        }
        for (kind, id) in used {
            let _ = self.lib.borrow_mut().mark_used(kind, id);
        }
        let (focus, front) = {
            let st = self.popup_state.borrow();
            (st.focus, st.front_app.clone())
        };
        let keys = {
            let s = self.settings.borrow();
            paste_keys_for(front.as_ref(), &s.paste.overrides, Platform::current())
        };
        self.hide_popup();
        crate::editor_ctl::history_changed(self);
        if platform::is_wayland() {
            // Wayland doesn't let apps type into other windows; the text is on
            // the clipboard, ready for Ctrl+V.
            return;
        }
        Timer::single_shot(Duration::from_millis(90), move || {
            if let Some(f) = focus {
                platform::foreground::restore(f);
            }
            Timer::single_shot(Duration::from_millis(40), move || {
                with_app(|app| {
                    let result = match app.hotkeys.borrow_mut().as_mut() {
                        Some(h) => {
                            h.suspended(|| platform::input::send_paste(to_platform_keys(keys)))
                        }
                        None => platform::input::send_paste(to_platform_keys(keys)),
                    };
                    if let Err(e) = result {
                        tracing::warn!("paste keystroke failed: {e}");
                        *app.notice.borrow_mut() = Some(format!(
                            "{e} The text is on the clipboard, so you can press Ctrl+V."
                        ));
                    }
                })
            });
        });
    }
}

fn selected_key(st: &PopupState) -> Option<(Kind, Id)> {
    let results = st.results.as_ref()?;
    match st.targets.get(st.current?)? {
        RowTarget::Hit(i) => results.hits.get(*i).map(|h| (h.kind, h.id)),
        _ => None,
    }
}

fn count_text(r: &Results, lib: &Library) -> String {
    match r.mode {
        Mode::Home => {
            let n = lib.store().history_count().unwrap_or(0);
            format!("{} in history", group(n))
        }
        Mode::Search => format!(
            "{} {}",
            group(r.total as i64),
            if r.total == 1 { "match" } else { "matches" }
        ),
        Mode::Filtered => {
            let one = r
                .hits
                .first()
                .and_then(|h| h.collection.clone())
                .filter(|c| {
                    r.hits
                        .iter()
                        .all(|h| h.collection.as_deref() == Some(c.as_str()))
                });
            let noun = if r.total == 1 { "snippet" } else { "snippets" };
            match one {
                Some(c) => format!("{} {noun} in {c}", group(r.total as i64)),
                None => format!("{} {noun}", group(r.total as i64)),
            }
        }
    }
}

fn empty_texts(r: &Results, query: &str, shortcut: &str) -> (String, String) {
    match r.mode {
        Mode::Home => (
            "Your clipboard history is empty".into(),
            format!("Copy any text and it will show up here. Press {shortcut} to open Shift Paste anytime."),
        ),
        Mode::Search => (
            format!("No matches for “{}”", query.trim()),
            "Letters match in order, so “1884” finds “18 mm 8 x 4”. Type @ to search a collection, # for a tag.".into(),
        ),
        Mode::Filtered => (
            "Nothing found".into(),
            "Check the collection name after @, or the tag after #.".into(),
        ),
    }
}

/// 1234 → "1,234".
pub fn group(n: i64) -> String {
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 {
        out.insert(0, '-');
    }
    out
}

pub fn to_identity(info: platform::foreground::AppInfo) -> AppIdentity {
    AppIdentity::new(info.name, info.ids)
}

fn to_platform_keys(keys: PasteKeys) -> platform::input::PasteKeys {
    use platform::input::PasteKeys as P;
    match keys {
        PasteKeys::CtrlV => P::CtrlV,
        PasteKeys::CtrlShiftV => P::CtrlShiftV,
        PasteKeys::ShiftInsert => P::ShiftInsert,
        PasteKeys::CmdV => P::CmdV,
    }
}

/// Keep `want_html` in sync with the setting (the watcher reads it live).
pub fn sync_formatting_flag(app: &App) {
    app.want_html.store(
        app.settings.borrow().history.keep_formatting,
        Ordering::Relaxed,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grouping() {
        assert_eq!(group(0), "0");
        assert_eq!(group(999), "999");
        assert_eq!(group(1234), "1,234");
        assert_eq!(group(1234567), "1,234,567");
    }
}
