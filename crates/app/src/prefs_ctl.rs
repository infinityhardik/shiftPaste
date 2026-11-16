//! Settings window controller. Every change is saved and applied immediately.

use std::rc::Rc;

use shiftpaste_core::apps::{entry_label, AppFilter, FilterMode, PasteKeys, PasteOverride};
use shiftpaste_core::presets::{self, Group, Platform, PRESET_PREFIX};
use shiftpaste_core::settings::{
    default_shortcut, Theme as ThemeSetting, DEFAULT_POPUP_HEIGHT, DEFAULT_POPUP_WIDTH,
};
use shiftpaste_platform as platform;
use slint::platform::Key;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::app::{group, with_app, App};
use crate::ui::{AppRow, PasteRow, PickRow, SettingsWindow, Theme};

/// Order matches the Settings combo box: Ctrl+V, Ctrl+Shift+V, Shift+Insert.
const PASTE_KEYS: [PasteKeys; 3] = [
    PasteKeys::CtrlV,
    PasteKeys::CtrlShiftV,
    PasteKeys::ShiftInsert,
];
const MAX_ITEMS: [u32; 5] = [100, 500, 1_000, 5_000, 0];
const MAX_AGE: [u32; 5] = [7, 30, 90, 365, 0];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Which {
    Hotkey,
    Capture,
    /// Per-app paste keystrokes.
    Paste,
}

pub struct PrefsCtl {
    pub window: SettingsWindow,
    picking: std::cell::Cell<Option<Which>>,
    picker_rows: std::cell::RefCell<Vec<(PickRow, Option<String>)>>,
    recording: std::cell::Cell<bool>,
}

pub fn open_settings(app: &Rc<App>, page: Option<i32>) {
    if app.prefs.borrow().is_none() {
        let window = SettingsWindow::new().expect("settings window");
        window.global::<Theme>().set_dark(app.is_dark());
        *app.prefs.borrow_mut() = Some(PrefsCtl {
            window,
            picking: Default::default(),
            picker_rows: Default::default(),
            recording: Default::default(),
        });
        wire(app);
    }
    let prefs = app.prefs.borrow();
    let p = prefs.as_ref().unwrap();
    p.load(app);
    if let Some(page) = page {
        p.window.set_page(page);
    }
    let _ = p.window.show();
}

fn with_prefs(f: impl FnOnce(&Rc<App>, &PrefsCtl)) {
    with_app(|app| {
        if let Some(p) = app.prefs.borrow().as_ref() {
            f(app, p);
        }
    });
}

fn wire(app: &Rc<App>) {
    let prefs = app.prefs.borrow();
    let w = &prefs.as_ref().unwrap().window;
    w.on_changed_general(|| {
        with_prefs(|app, p| {
            {
                let mut s = app.settings.borrow_mut();
                s.general.launch_at_login = p.window.get_launch_at_login();
                s.general.theme = match p.window.get_theme_mode() {
                    1 => ThemeSetting::Light,
                    2 => ThemeSetting::Dark,
                    _ => ThemeSetting::System,
                };
            }
            app.save_settings();
            app.apply_theme();
            app.apply_launch_at_login();
        })
    });
    w.on_reset_popup_size(|| {
        with_app(|app| {
            let mut s = app.settings.borrow_mut();
            s.popup.width = DEFAULT_POPUP_WIDTH;
            s.popup.height = DEFAULT_POPUP_HEIGHT;
            drop(s);
            app.save_settings();
        })
    });
    w.on_record_shortcut(|| {
        with_prefs(|_, p| {
            let on = !p.recording.get();
            p.recording.set(on);
            p.window.set_recording_shortcut(on);
            p.window.set_shortcut_error(SharedString::new());
        })
    });
    w.on_reset_shortcut(|| with_prefs(|app, p| p.apply_shortcut(app, default_shortcut())));
    w.on_shortcut_key(|key, ctrl, shift, alt, meta| {
        let mut handled = false;
        with_prefs(|app, p| handled = p.shortcut_key(app, &key, ctrl, shift, alt, meta));
        handled
    });
    w.on_hotkey_mode_changed(|m| {
        with_prefs(|app, p| {
            app.settings.borrow_mut().hotkey.apps.mode = mode_of(m);
            app.save_settings();
            p.load_lists(app);
        })
    });
    w.on_capture_mode_changed(|m| {
        with_prefs(|app, p| {
            app.settings.borrow_mut().privacy.capture.mode = mode_of(m);
            app.save_settings();
            p.load_lists(app);
        })
    });
    w.on_hotkey_remove(|i| with_prefs(|app, p| p.remove_entry(app, Which::Hotkey, i as usize)));
    w.on_capture_remove(|i| with_prefs(|app, p| p.remove_entry(app, Which::Capture, i as usize)));
    w.on_hotkey_add(|| with_prefs(|app, p| p.open_picker(app, Which::Hotkey)));
    w.on_paste_add(|| with_prefs(|app, p| p.open_picker(app, Which::Paste)));
    w.on_paste_remove(|i| {
        with_prefs(|app, p| {
            {
                let mut s = app.settings.borrow_mut();
                if (i as usize) < s.paste.overrides.len() {
                    s.paste.overrides.remove(i as usize);
                }
            }
            app.save_settings();
            p.load_lists(app);
        })
    });
    w.on_paste_keys_changed(|i, k| {
        with_prefs(|app, p| {
            {
                let mut s = app.settings.borrow_mut();
                if let Some(o) = s.paste.overrides.get_mut(i as usize) {
                    o.keys = PASTE_KEYS[(k.max(0) as usize).min(PASTE_KEYS.len() - 1)];
                }
            }
            app.save_settings();
            p.load_lists(app);
        })
    });
    w.on_capture_add(|| with_prefs(|app, p| p.open_picker(app, Which::Capture)));
    w.on_changed_history(|| {
        with_prefs(|app, p| {
            {
                let mut s = app.settings.borrow_mut();
                s.history.max_items =
                    MAX_ITEMS[(p.window.get_max_items_index().max(0) as usize).min(4)];
                s.history.max_age_days =
                    MAX_AGE[(p.window.get_max_age_index().max(0) as usize).min(4)];
                s.history.keep_formatting = p.window.get_keep_formatting();
            }
            app.save_settings();
            crate::app::sync_formatting_flag(app);
            let limits = app.settings.borrow().history.limits();
            let removed = app.lib.borrow_mut().enforce_limits(limits).unwrap_or(0);
            if removed > 0 {
                crate::editor_ctl::history_changed(app);
            }
            p.window.set_history_summary(history_summary(app).into());
        })
    });
    w.on_clear_history(|| with_app(crate::editor_ctl::confirm_clear_history));
    w.on_changed_privacy(|| {
        with_prefs(|app, p| {
            app.settings.borrow_mut().privacy.honour_password_flags =
                p.window.get_honour_password_flags();
            app.save_settings();
            app.set_paused(p.window.get_recording_paused());
        })
    });
    w.on_open_data_folder(|| with_app(|app| platform::shell::open_path(&app.paths.data_dir)));
    w.on_export_all(|| {
        with_app(|app| {
            if let Some(path) = platform::shell::save_file(
                "Export all collections",
                "Shift Paste collections.xlsx",
                &[("Excel workbook", "xlsx")],
            ) {
                if let Err(e) = app.lib.borrow().export_all(&path) {
                    tracing::warn!("export failed: {e}");
                }
            }
        })
    });
    w.on_open_link(|url| platform::shell::open_url(&url));
    w.on_picker_tab_changed(|_| with_prefs(|app, p| p.build_picker(app)));
    w.on_picker_filter_edited(|_| with_prefs(|app, p| p.build_picker(app)));
    w.on_picker_toggled(|i| with_prefs(|app, p| p.toggle_pick(app, i as usize)));
    w.on_picker_browse(|| {
        with_prefs(|app, p| {
            if let Some(path) = platform::shell::pick_app() {
                let id = path
                    .file_name()
                    .map(|n| n.to_string_lossy().trim_end_matches(".app").to_string())
                    .unwrap_or_default();
                if !id.is_empty() {
                    p.add_entry(app, id);
                }
            }
        })
    });
    w.on_picker_done(|| {
        with_prefs(|_, p| {
            p.picking.set(None);
            p.window.set_picker_open(false);
        })
    });
}

fn mode_of(m: i32) -> FilterMode {
    if m == 1 {
        FilterMode::Only
    } else {
        FilterMode::Except
    }
}

fn history_summary(app: &App) -> String {
    let lib = app.lib.borrow();
    let n = lib.store().history_count().unwrap_or(0);
    let oldest = lib
        .history(1, (n.max(1) - 1) as usize)
        .ok()
        .and_then(|v| v.into_iter().next());
    match oldest {
        Some(h) if n > 0 => format!(
            "Now: {} items, the oldest from {}.",
            group(n),
            shiftpaste_core::time::time_ago(h.last_copied_at, shiftpaste_core::time::now_ms())
                .to_lowercase()
        ),
        _ => "History is empty right now.".into(),
    }
}

impl PrefsCtl {
    fn load(&self, app: &App) {
        let s = app.settings.borrow();
        let w = &self.window;
        w.set_version(env!("CARGO_PKG_VERSION").into());
        w.set_show_paste_keys(!cfg!(target_os = "macos"));
        w.set_launch_at_login(s.general.launch_at_login);
        w.set_theme_mode(match s.general.theme {
            ThemeSetting::System => 0,
            ThemeSetting::Light => 1,
            ThemeSetting::Dark => 2,
        });
        w.set_shortcut_caps(caps(&s.hotkey.shortcut));
        w.set_shortcut_error(app.hotkey_error.borrow().as_str().into());
        w.set_pass_through_note(
            if platform::is_wayland() {
                "On Wayland the shortcut can't be passed through, so excluded apps don't receive it."
            } else {
                "Elsewhere the shortcut is passed through, so those apps' own use of it keeps working."
            }
            .into(),
        );
        w.set_max_items_index(
            MAX_ITEMS
                .iter()
                .position(|&m| m == s.history.max_items)
                .unwrap_or(1) as i32,
        );
        w.set_max_age_index(
            MAX_AGE
                .iter()
                .position(|&m| m == s.history.max_age_days)
                .unwrap_or(2) as i32,
        );
        w.set_keep_formatting(s.history.keep_formatting);
        w.set_honour_password_flags(s.privacy.honour_password_flags);
        w.set_recording_paused(s.privacy.recording_paused);
        w.set_platform_note(
            if platform::is_wayland() {
                "On Wayland the app in front usually can't be detected: “All apps except…” then records everything, and “Only these apps…” records nothing."
            } else {
                "Password managers are listed by default, as extra safety."
            }
            .into(),
        );
        let folder = app.paths.data_dir.display().to_string();
        w.set_data_folder(folder.into());
        w.set_portable(app.paths.portable);
        drop(s);
        w.set_history_summary(history_summary(app).into());
        self.load_lists(app);
    }

    fn load_lists(&self, app: &App) {
        let s = app.settings.borrow();
        self.window
            .set_hotkey_mode(i32::from(s.hotkey.apps.mode == FilterMode::Only));
        self.window
            .set_capture_mode(i32::from(s.privacy.capture.mode == FilterMode::Only));
        self.window.set_hotkey_apps(app_rows(&s.hotkey.apps));
        self.window.set_capture_apps(app_rows(&s.privacy.capture));
        let rows: Vec<PasteRow> = s
            .paste
            .overrides
            .iter()
            .map(|o| PasteRow {
                label: entry_label(&o.app).into(),
                keys: PASTE_KEYS.iter().position(|k| *k == o.keys).unwrap_or(0) as i32,
            })
            .collect();
        self.window
            .set_paste_rows(ModelRc::new(VecModel::from(rows)));
    }

    fn filter_mut<'a>(app: &'a App, which: Which) -> std::cell::RefMut<'a, AppFilter> {
        std::cell::RefMut::map(app.settings.borrow_mut(), |s| match which {
            Which::Hotkey | Which::Paste => &mut s.hotkey.apps,
            Which::Capture => &mut s.privacy.capture,
        })
    }

    /// The entries a picker mode currently has ticked.
    fn entries(app: &App, which: Which) -> Vec<String> {
        match which {
            Which::Paste => app
                .settings
                .borrow()
                .paste
                .overrides
                .iter()
                .map(|o| o.app.clone())
                .collect(),
            _ => Self::filter_mut(app, which).active_list().to_vec(),
        }
    }

    /// Tick (`Some(true)`), untick (`Some(false)`) or toggle (`None`) an entry.
    fn set_entry(app: &App, which: Which, entry: String, want: Option<bool>) {
        if which == Which::Paste {
            let mut s = app.settings.borrow_mut();
            let list = &mut s.paste.overrides;
            let pos = list.iter().position(|o| o.app.eq_ignore_ascii_case(&entry));
            match (pos, want) {
                (Some(i), None | Some(false)) => {
                    list.remove(i);
                }
                (None, None | Some(true)) => list.push(PasteOverride {
                    app: entry,
                    keys: PasteKeys::CtrlShiftV,
                }),
                _ => {}
            }
            return;
        }
        let mut f = Self::filter_mut(app, which);
        let list = f.active_list_mut();
        let pos = list.iter().position(|e| e.eq_ignore_ascii_case(&entry));
        match (pos, want) {
            (Some(i), None | Some(false)) => {
                list.remove(i);
            }
            (None, None | Some(true)) => list.push(entry),
            _ => {}
        }
    }

    fn remove_entry(&self, app: &App, which: Which, i: usize) {
        {
            let mut f = Self::filter_mut(app, which);
            let list = f.active_list_mut();
            if i < list.len() {
                list.remove(i);
            }
        }
        app.save_settings();
        self.load_lists(app);
    }

    fn add_entry(&self, app: &App, entry: String) {
        let Some(which) = self.picking.get() else {
            return;
        };
        Self::set_entry(app, which, entry, Some(true));
        app.save_settings();
        self.load_lists(app);
        self.build_picker(app);
    }

    fn open_picker(&self, app: &App, which: Which) {
        self.picking.set(Some(which));
        let only = match which {
            Which::Hotkey => app.settings.borrow().hotkey.apps.mode == FilterMode::Only,
            Which::Capture => app.settings.borrow().privacy.capture.mode == FilterMode::Only,
            Which::Paste => false,
        };
        let title = match (which, only) {
            (Which::Hotkey, false) => "Where shouldn't the shortcut open Shift Paste?",
            (Which::Hotkey, true) => "Where should the shortcut open Shift Paste?",
            (Which::Capture, false) => "Which apps' copies should not be recorded?",
            (Which::Capture, true) => "Which apps' copies should be recorded?",
            (Which::Paste, _) => "Which apps need a different paste keystroke?",
        };
        self.window.set_picker_title(title.into());
        self.window.set_picker_tab(0);
        self.window.set_picker_filter(SharedString::new());
        self.build_picker(app);
        self.window.set_picker_open(true);
    }

    fn build_picker(&self, app: &App) {
        let Some(which) = self.picking.get() else {
            return;
        };
        let list: Vec<String> = Self::entries(app, which);
        let filter = self.window.get_picker_filter().to_lowercase();
        let wanted = |name: &str| filter.is_empty() || name.to_lowercase().contains(&filter);
        let checked = |entry: &str| list.iter().any(|e| e.eq_ignore_ascii_case(entry));
        let mut rows: Vec<(PickRow, Option<String>)> = Vec::new();
        match self.window.get_picker_tab() {
            0 => {
                // Password managers first for the capture list, last for the hotkey list.
                let mut groups: Vec<Group> = Group::ALL.to_vec();
                if which == Which::Capture {
                    groups.retain(|g| *g != Group::PasswordManagers);
                    groups.insert(0, Group::PasswordManagers);
                }
                for g in groups {
                    let apps: Vec<_> = presets::presets_in(g, Platform::current())
                        .filter(|p| wanted(p.name))
                        .collect();
                    if apps.is_empty() {
                        continue;
                    }
                    rows.push((
                        PickRow {
                            section: true,
                            label: g.label().to_uppercase().into(),
                            ..Default::default()
                        },
                        None,
                    ));
                    for p in apps {
                        let entry = p.entry();
                        rows.push((
                            PickRow {
                                section: false,
                                label: p.name.into(),
                                detail: SharedString::new(),
                                checked: checked(&entry),
                            },
                            Some(entry),
                        ));
                    }
                }
            }
            1 => {
                rows.push((
                    PickRow {
                        section: true,
                        label: "APPS YOU COPIED FROM RECENTLY".into(),
                        ..Default::default()
                    },
                    None,
                ));
                let recent = app
                    .lib
                    .borrow()
                    .store()
                    .recent_source_apps(40)
                    .unwrap_or_default();
                if let Some(front) = platform::foreground::current_app() {
                    if !recent.contains(&front.name) && wanted(&front.name) {
                        rows.push((
                            PickRow {
                                section: false,
                                label: front.name.as_str().into(),
                                detail: "in front now".into(),
                                checked: checked(&front.name),
                            },
                            Some(front.name.clone()),
                        ));
                    }
                }
                for name in recent.into_iter().filter(|n| wanted(n)) {
                    rows.push((
                        PickRow {
                            section: false,
                            label: name.as_str().into(),
                            detail: SharedString::new(),
                            checked: checked(&name),
                        },
                        Some(name),
                    ));
                }
            }
            _ => {}
        }
        // Entries added by hand (Browse…) show at the end of the first tab.
        if self.window.get_picker_tab() == 0 {
            let custom: Vec<&String> = list
                .iter()
                .filter(|e| !e.starts_with(PRESET_PREFIX))
                .collect();
            if !custom.is_empty() {
                rows.push((
                    PickRow {
                        section: true,
                        label: "ADDED BY YOU".into(),
                        ..Default::default()
                    },
                    None,
                ));
                for e in custom {
                    rows.push((
                        PickRow {
                            section: false,
                            label: e.as_str().into(),
                            detail: SharedString::new(),
                            checked: true,
                        },
                        Some(e.clone()),
                    ));
                }
            }
        }
        self.window.set_picker_rows(ModelRc::new(VecModel::from(
            rows.iter().map(|(r, _)| r.clone()).collect::<Vec<_>>(),
        )));
        *self.picker_rows.borrow_mut() = rows;
    }

    fn toggle_pick(&self, app: &App, i: usize) {
        let Some(which) = self.picking.get() else {
            return;
        };
        let Some(entry) = self
            .picker_rows
            .borrow()
            .get(i)
            .and_then(|(_, e)| e.clone())
        else {
            return;
        };
        Self::set_entry(app, which, entry, None);
        app.save_settings();
        self.load_lists(app);
        self.build_picker(app);
    }

    fn shortcut_key(
        &self,
        app: &App,
        key: &str,
        ctrl: bool,
        shift: bool,
        alt: bool,
        meta: bool,
    ) -> bool {
        if !self.recording.get() {
            return false;
        }
        let Some(k) = key.chars().next() else {
            return true;
        };
        if k == char::from(Key::Escape) {
            self.recording.set(false);
            self.window.set_recording_shortcut(false);
            return true;
        }
        let modifier_only = [
            Key::Control,
            Key::ControlR,
            Key::Shift,
            Key::ShiftR,
            Key::Alt,
            Key::AltGr,
            Key::Meta,
            Key::MetaR,
        ]
        .iter()
        .any(|m| k == char::from(*m));
        if modifier_only {
            return true; // wait for the actual key
        }
        let name = key_name(k);
        let Some(name) = name else {
            self.window.set_shortcut_error(
                "That key can't be used in a shortcut. Try a letter, number or F-key.".into(),
            );
            return true;
        };
        let mut parts = Vec::new();
        if ctrl {
            parts.push("Ctrl");
        }
        if alt {
            parts.push("Alt");
        }
        if shift {
            parts.push("Shift");
        }
        if meta {
            parts.push(if cfg!(target_os = "macos") {
                "Cmd"
            } else {
                "Win"
            });
        }
        let shortcut = if parts.is_empty() {
            name.clone()
        } else {
            format!("{}+{name}", parts.join("+"))
        };
        self.apply_shortcut(app, &shortcut);
        true
    }

    fn apply_shortcut(&self, app: &App, shortcut: &str) {
        if let Err(e) = platform::hotkey::parse(shortcut) {
            self.window.set_shortcut_error(e.to_string().into());
            return;
        }
        self.recording.set(false);
        self.window.set_recording_shortcut(false);
        if app.register_shortcut(shortcut) {
            app.settings.borrow_mut().hotkey.shortcut = shortcut.to_string();
            app.save_settings();
            self.window.set_shortcut_caps(caps(shortcut));
            self.window.set_shortcut_error(SharedString::new());
        } else {
            self.window
                .set_shortcut_error(app.hotkey_error.borrow().as_str().into());
        }
    }
}

fn key_name(k: char) -> Option<String> {
    let fkeys = [
        Key::F1,
        Key::F2,
        Key::F3,
        Key::F4,
        Key::F5,
        Key::F6,
        Key::F7,
        Key::F8,
        Key::F9,
        Key::F10,
        Key::F11,
        Key::F12,
    ];
    if let Some(i) = fkeys.iter().position(|f| char::from(*f) == k) {
        return Some(format!("F{}", i + 1));
    }
    if k == ' ' {
        return Some("Space".into());
    }
    if k == char::from(Key::Insert) {
        return Some("Insert".into());
    }
    (k.is_ascii_alphanumeric() || "`-=,./;'[]\\".contains(k))
        .then(|| k.to_ascii_uppercase().to_string())
}

fn caps(shortcut: &str) -> ModelRc<SharedString> {
    let caps = platform::hotkey::caps(shortcut, cfg!(target_os = "macos"));
    ModelRc::new(VecModel::from(
        caps.into_iter().map(SharedString::from).collect::<Vec<_>>(),
    ))
}

fn app_rows(filter: &AppFilter) -> ModelRc<AppRow> {
    let rows: Vec<AppRow> = filter
        .active_list()
        .iter()
        .map(|e| {
            let detail = match e.strip_prefix(PRESET_PREFIX).and_then(presets::preset) {
                Some(p) => p.group.label().trim_end_matches('s').to_string(),
                None => "Added by you".to_string(),
            };
            AppRow {
                label: entry_label(e).into(),
                detail: detail.into(),
            }
        })
        .collect();
    ModelRc::new(VecModel::from(rows))
}
