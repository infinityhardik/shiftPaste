//! Editor window controller: collections, snippets, history, import/export.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use shiftpaste_core::library::{SnippetDraft, FAVORITES_NAME};
use shiftpaste_core::model::{first_line, parse_tags, Collection, HistoryItem, Id, Snippet};
use shiftpaste_core::search::{one_line, Kind};
use shiftpaste_core::time::{format_local, now_ms, time_ago};
use shiftpaste_platform as platform;
use slint::platform::Key;
use slint::{ComponentHandle, Model, ModelRc, SharedString, Timer, VecModel};

use crate::app::{group, with_app, App};
use crate::ui::{CollectionItem, EditorWindow, ListItem, PickerItem, Theme};

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Collection(Id),
    History,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ItemRef {
    Snippet(Id),
    History(Id),
}

enum Picker {
    MoveTo(Id),
    CopyTo(Id),
    SaveTo(Id),
    Export,
}

enum Confirm {
    DeleteCollection(Id),
    ClearHistory,
}

enum ToastAction {
    Undo,
    AdjustImport(PathBuf),
}

struct State {
    view: View,
    collections: Vec<Collection>,
    items: Vec<ItemRef>,
    current: Option<usize>,
    /// True while Rust fills the detail fields (their `edited` must be ignored).
    loading: bool,
    picker: Option<(Picker, Vec<Option<Id>>)>,
    confirm: Option<Confirm>,
    toast: Option<ToastAction>,
    toast_timer: Option<Timer>,
}

pub struct EditorCtl {
    pub window: EditorWindow,
    state: RefCell<State>,
}

/// Open (or focus) the editor; optionally select an item.
pub fn open_editor(app: &Rc<App>, focus: Option<(Kind, Id)>) {
    if app.editor.borrow().is_none() {
        let window = EditorWindow::new().expect("editor window");
        window.global::<Theme>().set_dark(app.is_dark());
        window.set_mod_key(crate::key_labels().r#mod);
        let caps = platform::hotkey::caps(
            &app.settings.borrow().hotkey.shortcut,
            cfg!(target_os = "macos"),
        );
        window.set_hotkey_caps(ModelRc::new(VecModel::from(
            caps.into_iter().map(SharedString::from).collect::<Vec<_>>(),
        )));
        window.set_first_run(!app.settings.borrow().general.welcome_done);
        let first = app
            .lib
            .borrow()
            .collections()
            .unwrap_or_default()
            .first()
            .map(|c| c.id);
        *app.editor.borrow_mut() = Some(EditorCtl {
            window,
            state: RefCell::new(State {
                view: first.map_or(View::History, View::Collection),
                collections: Vec::new(),
                items: Vec::new(),
                current: None,
                loading: false,
                picker: None,
                confirm: None,
                toast: None,
                toast_timer: None,
            }),
        });
        wire(app);
    }
    let ed_ref = app.editor.borrow();
    let ed = ed_ref.as_ref().unwrap();
    if let Some((kind, id)) = focus {
        let view = match kind {
            Kind::History => View::History,
            Kind::Snippet => app
                .lib
                .borrow()
                .snippet(id)
                .map(|s| View::Collection(s.collection_id))
                .unwrap_or(View::History),
        };
        ed.state.borrow_mut().view = view;
        ed.window.set_filter(SharedString::new());
    }
    ed.refresh_all(app);
    if let Some((kind, id)) = focus {
        let target = match kind {
            Kind::History => ItemRef::History(id),
            Kind::Snippet => ItemRef::Snippet(id),
        };
        let idx = ed.state.borrow().items.iter().position(|r| *r == target);
        if let Some(i) = idx {
            ed.select(app, i);
        }
    }
    let _ = ed.window.show();
    if let Some(msg) = app.notice.borrow_mut().take() {
        ed.toast(&msg, None);
    }
}

/// The library changed somewhere else (popup, import): refresh if open.
pub fn library_changed(app: &App) {
    if let Some(ed) = app.editor.borrow().as_ref() {
        ed.refresh_all(app);
    }
}

/// History changed (new copy): refresh the history list if it's showing.
pub fn history_changed(app: &App) {
    if let Some(ed) = app.editor.borrow().as_ref() {
        if ed.state.borrow().view == View::History && ed.window.window().is_visible() {
            ed.refresh_items(app, true);
        }
    }
}

/// "Clear history…" from the tray or settings: confirm in the editor.
pub fn confirm_clear_history(app: &Rc<App>) {
    open_editor(app, None);
    if let Some(ed) = app.editor.borrow().as_ref() {
        let n = app.lib.borrow().store().history_count().unwrap_or(0);
        ed.confirm(
            Confirm::ClearHistory,
            "Clear clipboard history?",
            &format!("All {} history items will be deleted. Your collections are kept. This can't be undone.", group(n)),
            "Clear history",
        );
    }
}

fn with_ed(f: impl FnOnce(&Rc<App>, &EditorCtl)) {
    with_app(|app| {
        if let Some(ed) = app.editor.borrow().as_ref() {
            f(app, ed);
        }
    });
}

fn wire(app: &Rc<App>) {
    let ed_ref = app.editor.borrow();
    let w = &ed_ref.as_ref().unwrap().window;
    w.on_view_history(|| {
        with_ed(|app, ed| {
            ed.state.borrow_mut().view = View::History;
            ed.window.set_filter(SharedString::new());
            ed.refresh_all(app);
        })
    });
    w.on_collection_clicked(|i| {
        with_ed(|app, ed| {
            let id = ed.state.borrow().collections.get(i as usize).map(|c| c.id);
            if let Some(id) = id {
                ed.state.borrow_mut().view = View::Collection(id);
                ed.window.set_filter(SharedString::new());
                ed.refresh_all(app);
            }
        })
    });
    w.on_collection_new(|| {
        with_ed(|app, ed| {
            let created = app.lib.borrow_mut().create_collection("New collection");
            match created {
                Ok(c) => {
                    ed.state.borrow_mut().view = View::Collection(c.id);
                    ed.refresh_all(app);
                    let idx = ed
                        .state
                        .borrow()
                        .collections
                        .iter()
                        .position(|x| x.id == c.id);
                    ed.window
                        .set_renaming_collection(idx.map_or(-1, |i| i as i32));
                }
                Err(e) => ed.toast(&e.to_string(), None),
            }
        })
    });
    w.on_collection_renamed(|i, name| {
        with_ed(|app, ed| {
            let id = ed.state.borrow().collections.get(i as usize).map(|c| c.id);
            if let Some(id) = id {
                let r = app.lib.borrow_mut().rename_collection(id, &name);
                if let Err(e) = r {
                    ed.toast(&e.to_string(), None);
                }
                ed.refresh_all(app);
            }
        })
    });
    w.on_collection_delete(|i| {
        with_ed(|app, ed| {
            let c = ed.state.borrow().collections.get(i as usize).cloned();
            if let Some(c) = c {
                let n = app
                    .lib
                    .borrow()
                    .snippets(c.id)
                    .map(|s| s.len())
                    .unwrap_or(0);
                ed.confirm(
                    Confirm::DeleteCollection(c.id),
                    &format!("Delete “{}”?", c.name),
                    &format!(
                        "Its {n} snippet{} will be deleted too. You can undo this with {}+Z.",
                        if n == 1 { "" } else { "s" },
                        crate::key_labels().r#mod
                    ),
                    "Delete collection",
                );
            }
        })
    });
    w.on_item_clicked(|i| with_ed(|app, ed| ed.select(app, i as usize)));
    w.on_item_new(|| {
        with_ed(|app, ed| {
            let View::Collection(cid) = ed.state.borrow().view else {
                return;
            };
            let created = app
                .lib
                .borrow_mut()
                .create_snippet(cid, SnippetDraft::default());
            if let Ok(s) = created {
                ed.window.set_filter(SharedString::new());
                ed.refresh_all(app);
                let idx = ed
                    .state
                    .borrow()
                    .items
                    .iter()
                    .position(|r| *r == ItemRef::Snippet(s.id));
                if let Some(i) = idx {
                    ed.select(app, i);
                }
            }
        })
    });
    w.on_item_moved(|from, to| {
        with_ed(|app, ed| {
            let View::Collection(cid) = ed.state.borrow().view else {
                return;
            };
            let mut ids: Vec<Id> = ed
                .state
                .borrow()
                .items
                .iter()
                .filter_map(|r| match r {
                    ItemRef::Snippet(id) => Some(*id),
                    _ => None,
                })
                .collect();
            let (from, to) = (from as usize, to as usize);
            if from >= ids.len() || to >= ids.len() {
                return;
            }
            let id = ids.remove(from);
            ids.insert(to, id);
            if app.lib.borrow_mut().reorder_snippets(cid, &ids).is_ok() {
                ed.refresh_items(app, true);
            }
        })
    });
    w.on_item_star(|i| with_ed(|app, ed| ed.star(app, i as usize)));
    w.on_item_move_to(|i| with_ed(|app, ed| ed.open_collection_picker(app, i as usize, true)));
    w.on_item_copy_to(|i| with_ed(|app, ed| ed.open_collection_picker(app, i as usize, false)));
    w.on_item_save_to(|i| with_ed(|app, ed| ed.open_save_picker(app, i as usize)));
    w.on_item_delete(|i| with_ed(|app, ed| ed.delete(app, i as usize)));
    w.on_filter_edited(|_| with_ed(|app, ed| ed.refresh_items(app, false)));
    w.on_detail_edited(|| with_ed(|app, ed| ed.save_detail(app)));
    w.on_tag_added(|text| {
        with_ed(|app, ed| ed.edit_tags(app, |tags| tags.extend(parse_tags(&text))))
    });
    w.on_tag_removed(|i| {
        with_ed(|app, ed| {
            ed.edit_tags(app, |tags| {
                if (i as usize) < tags.len() {
                    tags.remove(i as usize);
                }
            })
        })
    });
    w.on_undo(|| with_ed(|app, ed| ed.undo_redo(app, true)));
    w.on_redo(|| with_ed(|app, ed| ed.undo_redo(app, false)));
    w.on_import(|| {
        with_ed(|app, ed| {
            ed.window.set_first_run(false);
            ed.import(app)
        })
    });
    w.on_export(|| with_ed(|app, ed| ed.open_export_picker(app)));
    w.on_open_settings(|| with_app(|app| crate::prefs_ctl::open_settings(app, None)));
    w.on_menu_clicked(|i| with_ed(|app, ed| ed.picker_chosen(app, i)));
    w.on_confirm_done(|ok| with_ed(|app, ed| ed.confirmed(app, ok)));
    w.on_toast_clicked(|| with_ed(|app, ed| ed.toast_clicked(app)));
    w.on_welcome_done(|| {
        with_ed(|app, ed| {
            ed.window.set_first_run(false);
            app.settings.borrow_mut().general.welcome_done = true;
            app.save_settings();
        })
    });
    w.on_handle_key(|key, ctrl, shift, _alt| {
        let mut handled = false;
        with_ed(|app, ed| handled = ed.key(app, &key, ctrl, shift));
        handled
    });
    w.window().on_close_requested(|| {
        with_ed(|app, ed| {
            if ed.window.get_first_run() {
                app.settings.borrow_mut().general.welcome_done = true;
                app.save_settings();
            }
        });
        slint::CloseRequestResponse::HideWindow
    });
}

impl EditorCtl {
    fn refresh_all(&self, app: &App) {
        self.refresh_collections(app);
        self.refresh_items(app, true);
        self.refresh_undo(app);
    }

    fn refresh_undo(&self, app: &App) {
        let lib = app.lib.borrow();
        self.window.set_can_undo(lib.can_undo());
        self.window.set_can_redo(lib.can_redo());
        self.window
            .set_undo_label(lib.undo_label().unwrap_or_default().into());
        self.window
            .set_redo_label(lib.redo_label().unwrap_or_default().into());
    }

    fn refresh_collections(&self, app: &App) {
        let lib = app.lib.borrow();
        let collections = lib.collections().unwrap_or_default();
        let counts = lib.store().snippet_counts().unwrap_or_default();
        let mut st = self.state.borrow_mut();
        // A deleted collection falls back to the first one (or history).
        if let View::Collection(id) = st.view {
            if !collections.iter().any(|c| c.id == id) {
                st.view = collections
                    .first()
                    .map_or(View::History, |c| View::Collection(c.id));
            }
        }
        let items: Vec<CollectionItem> = collections
            .iter()
            .map(|c| CollectionItem {
                name: c.name.as_str().into(),
                count: *counts.get(&c.id).unwrap_or(&0) as i32,
                favorites: c.name.eq_ignore_ascii_case(FAVORITES_NAME),
            })
            .collect();
        let current = match st.view {
            View::Collection(id) => collections
                .iter()
                .position(|c| c.id == id)
                .map_or(-1, |i| i as i32),
            _ => -1,
        };
        self.window
            .set_view(if st.view == View::History { 1 } else { 0 });
        self.window
            .set_collections(ModelRc::new(VecModel::from(items)));
        self.window.set_current_collection(current);
        st.collections = collections;
    }

    /// Rebuild the middle list. `keep` keeps the selected item if it still exists.
    fn refresh_items(&self, app: &App, keep: bool) {
        let filter: String = self
            .window
            .get_filter()
            .to_lowercase()
            .split_whitespace()
            .collect();
        let lib = app.lib.borrow();
        let now = now_ms();
        let view = self.state.borrow().view;
        let (refs, rows, title, subtitle): (Vec<ItemRef>, Vec<ListItem>, String, String) =
            match view {
                View::Collection(cid) => {
                    let name = lib
                        .store()
                        .collection(cid)
                        .map(|c| c.name)
                        .unwrap_or_default();
                    let all = lib.snippets(cid).unwrap_or_default();
                    let total = all.len();
                    let shown: Vec<Snippet> = all
                        .into_iter()
                        .filter(|s| matches(&filter, &s.title, &s.content))
                        .collect();
                    let subtitle = if filter.is_empty() {
                        format!(
                            "{} snippet{}",
                            group(total as i64),
                            if total == 1 { "" } else { "s" }
                        )
                    } else {
                        format!("{} of {} snippets", shown.len(), total)
                    };
                    (
                        shown.iter().map(|s| ItemRef::Snippet(s.id)).collect(),
                        shown.iter().map(snippet_row).collect(),
                        name,
                        subtitle,
                    )
                }
                View::History => {
                    let all = lib.history(5_000, 0).unwrap_or_default();
                    let total = all.len();
                    let shown: Vec<HistoryItem> = all
                        .into_iter()
                        .filter(|h| matches(&filter, "", &h.content))
                        .collect();
                    let s = app.settings.borrow();
                    let keep_text = match (s.history.max_items, s.history.max_age_days) {
                        (0, 0) => "kept forever".to_string(),
                        (0, d) => format!("kept for {d} days"),
                        (n, 0) => format!("up to {}", group(i64::from(n))),
                        (n, d) => format!("kept for {d} days, up to {}", group(i64::from(n))),
                    };
                    let subtitle = if filter.is_empty() {
                        format!("{} items · {keep_text}", group(total as i64))
                    } else {
                        format!("{} of {} items", shown.len(), total)
                    };
                    (
                        shown.iter().map(|h| ItemRef::History(h.id)).collect(),
                        shown.iter().map(|h| history_row(h, now)).collect(),
                        "Clipboard history".into(),
                        subtitle,
                    )
                }
            };
        drop(lib);
        let previous = {
            let st = self.state.borrow();
            st.current.and_then(|c| st.items.get(c).copied())
        };
        self.window.set_list_title(title.into());
        self.window.set_list_subtitle(subtitle.into());
        self.window.set_items(ModelRc::new(VecModel::from(rows)));
        let keep_idx = if keep {
            previous.and_then(|p| refs.iter().position(|r| *r == p))
        } else {
            None
        };
        let same_item = keep_idx.is_some();
        self.state.borrow_mut().items = refs;
        match keep_idx {
            Some(i) if same_item => {
                // Same item still selected: keep the detail fields as they are
                // (the user may be typing in them).
                self.state.borrow_mut().current = Some(i);
                self.window.set_current_item(i as i32);
            }
            _ => {
                let first = (!self.state.borrow().items.is_empty()).then_some(0);
                match first {
                    Some(i) => self.select(app, i),
                    None => self.clear_detail(),
                }
            }
        }
    }

    fn clear_detail(&self) {
        self.state.borrow_mut().current = None;
        self.window.set_current_item(-1);
        self.window.set_has_detail(false);
    }

    fn select(&self, app: &App, i: usize) {
        let Some(item) = self.state.borrow().items.get(i).copied() else {
            return;
        };
        self.state.borrow_mut().current = Some(i);
        self.state.borrow_mut().loading = true;
        self.window.set_current_item(i as i32);
        let lib = app.lib.borrow();
        match item {
            ItemRef::Snippet(id) => {
                if let Ok(s) = lib.snippet(id) {
                    self.window.set_detail_title(s.title.as_str().into());
                    self.window.set_detail_content(s.content.as_str().into());
                    self.window.set_detail_notes(s.notes.as_str().into());
                    self.window.set_detail_tags(tags_model(&s.tags));
                    self.window.set_detail_starred(s.is_starred());
                    self.window.set_detail_meta(snippet_meta(&s).into());
                    self.window.set_has_detail(true);
                }
            }
            ItemRef::History(id) => {
                if let Ok(h) = lib.history_item(id) {
                    self.window.set_detail_title(SharedString::new());
                    self.window.set_detail_content(h.content.as_str().into());
                    self.window.set_detail_notes(SharedString::new());
                    self.window.set_detail_tags(tags_model(&[]));
                    self.window.set_detail_starred(false);
                    self.window.set_detail_meta(history_meta(&h).into());
                    self.window.set_has_detail(true);
                }
            }
        }
        self.window.set_saved_text(SharedString::new());
        self.state.borrow_mut().loading = false;
    }

    fn current(&self) -> Option<(usize, ItemRef)> {
        let st = self.state.borrow();
        let i = st.current?;
        Some((i, *st.items.get(i)?))
    }

    /// Autosave the detail fields.
    fn save_detail(&self, app: &App) {
        if self.state.borrow().loading {
            return;
        }
        let Some((i, item)) = self.current() else {
            return;
        };
        let result = match item {
            ItemRef::Snippet(id) => {
                let tags = app
                    .lib
                    .borrow()
                    .snippet(id)
                    .map(|s| s.tags)
                    .unwrap_or_default();
                let draft = SnippetDraft {
                    title: self.window.get_detail_title().into(),
                    content: self.window.get_detail_content().into(),
                    notes: self.window.get_detail_notes().into(),
                    tags,
                };
                let saved = app.lib.borrow_mut().update_snippet(id, draft);
                saved.map(|s| snippet_row(&s))
            }
            ItemRef::History(id) => {
                let text: String = self.window.get_detail_content().into();
                let saved = app.lib.borrow_mut().update_history(id, &text);
                saved
                    .and_then(|_| app.lib.borrow().history_item(id))
                    .map(|h| history_row(&h, now_ms()))
            }
        };
        match result {
            Ok(row) => {
                self.window.get_items().set_row_data(i, row);
                self.window.set_saved_text("Saved".into());
                self.refresh_undo(app);
            }
            Err(e) => self.window.set_saved_text(e.to_string().into()),
        }
    }

    fn edit_tags(&self, app: &App, f: impl FnOnce(&mut Vec<String>)) {
        let Some((i, ItemRef::Snippet(id))) = self.current() else {
            return;
        };
        let Ok(s) = app.lib.borrow().snippet(id) else {
            return;
        };
        let mut tags = s.tags.clone();
        f(&mut tags);
        let draft = SnippetDraft {
            title: s.title,
            content: s.content,
            notes: s.notes,
            tags,
        };
        let saved = app.lib.borrow_mut().update_snippet(id, draft);
        if let Ok(s) = saved {
            self.window.set_detail_tags(tags_model(&s.tags));
            self.window.get_items().set_row_data(i, snippet_row(&s));
            self.window.set_saved_text("Saved".into());
            self.refresh_undo(app);
        }
    }

    fn star(&self, app: &App, i: usize) {
        let Some(item) = self.state.borrow().items.get(i).copied() else {
            return;
        };
        let result = match item {
            ItemRef::Snippet(id) => {
                let starred = app
                    .lib
                    .borrow()
                    .snippet(id)
                    .map(|s| s.is_starred())
                    .unwrap_or(false);
                app.lib.borrow_mut().set_starred(&[id], !starred).map(|_| {
                    if starred {
                        "Removed from favorites"
                    } else {
                        "Added to favorites"
                    }
                })
            }
            ItemRef::History(id) => app
                .lib
                .borrow_mut()
                .save_history_to_collection(id, None, true)
                .map(|_| "Saved to Favorites"),
        };
        match result {
            Ok(msg) => {
                self.refresh_all(app);
                self.toast(msg, Some(ToastAction::Undo));
            }
            Err(e) => self.toast(&e.to_string(), None),
        }
    }

    fn delete(&self, app: &App, i: usize) {
        let Some(item) = self.state.borrow().items.get(i).copied() else {
            return;
        };
        let (result, what) = match item {
            ItemRef::Snippet(id) => {
                let name = app
                    .lib
                    .borrow()
                    .snippet(id)
                    .map(|s| s.display_title().to_owned())
                    .unwrap_or_default();
                (
                    app.lib.borrow_mut().delete_snippets(&[id]),
                    format!("Deleted “{}”", short(&name)),
                )
            }
            ItemRef::History(id) => (
                app.lib.borrow_mut().delete_history(&[id]),
                "History item deleted".to_string(),
            ),
        };
        if result.is_ok() {
            self.refresh_all(app);
            self.toast(&what, Some(ToastAction::Undo));
        }
    }

    fn undo_redo(&self, app: &App, undo: bool) {
        let result = if undo {
            app.lib.borrow_mut().undo()
        } else {
            app.lib.borrow_mut().redo()
        };
        match result {
            Ok(Some(label)) => {
                self.refresh_all(app);
                if let Some(Some((i, _))) = Some(self.current()) {
                    self.select(app, i);
                }
                self.toast(
                    &format!("{}: {label}", if undo { "Undone" } else { "Redone" }),
                    None,
                );
            }
            Ok(None) => {}
            Err(e) => self.toast(
                &format!("Couldn't {}: {e}", if undo { "undo" } else { "redo" }),
                None,
            ),
        }
        if app.popup.window().is_visible() {
            app.refresh_popup(true);
        }
    }

    // ---------------------------------------------------------- pickers / menus

    fn open_collection_picker(&self, app: &App, i: usize, moving: bool) {
        let Some(ItemRef::Snippet(id)) = self.state.borrow().items.get(i).copied() else {
            return;
        };
        let View::Collection(here) = self.state.borrow().view else {
            return;
        };
        let options: Vec<(String, Option<Id>)> = app
            .lib
            .borrow()
            .collections()
            .unwrap_or_default()
            .into_iter()
            .filter(|c| c.id != here)
            .map(|c| (c.name, Some(c.id)))
            .chain(std::iter::once(("New collection…".to_string(), None)))
            .collect();
        let title = if moving { "Move to" } else { "Copy to" };
        self.open_picker(
            if moving {
                Picker::MoveTo(id)
            } else {
                Picker::CopyTo(id)
            },
            title,
            options,
        );
    }

    fn open_save_picker(&self, app: &App, i: usize) {
        let Some(ItemRef::History(id)) = self.state.borrow().items.get(i).copied() else {
            return;
        };
        let options: Vec<(String, Option<Id>)> = app
            .lib
            .borrow()
            .collections()
            .unwrap_or_default()
            .into_iter()
            .map(|c| (c.name, Some(c.id)))
            .chain(std::iter::once(("New collection…".to_string(), None)))
            .collect();
        self.open_picker(Picker::SaveTo(id), "Save to collection", options);
    }

    fn open_export_picker(&self, _app: &App) {
        let in_collection = matches!(self.state.borrow().view, View::Collection(_));
        let mut options: Vec<(String, Option<Id>)> = Vec::new();
        if in_collection {
            options.push(("This collection as CSV…".into(), Some(0)));
            options.push(("This collection as Excel…".into(), Some(1)));
        }
        options.push(("All collections as one Excel file…".into(), Some(2)));
        options.push(("Clipboard history as CSV…".into(), Some(3)));
        options.push(("Clipboard history as Excel…".into(), Some(4)));
        self.open_picker(Picker::Export, "Export", options);
    }

    fn open_picker(&self, kind: Picker, title: &str, options: Vec<(String, Option<Id>)>) {
        let items: Vec<PickerItem> = options
            .iter()
            .enumerate()
            .map(|(i, (label, _))| PickerItem {
                label: label.as_str().into(),
                hint: if i < 9 {
                    (i + 1).to_string().into()
                } else {
                    SharedString::new()
                },
            })
            .collect();
        self.window.set_menu_title(title.into());
        self.window
            .set_menu_items(ModelRc::new(VecModel::from(items)));
        self.window.set_menu_current(0);
        self.window.set_menu_open(true);
        self.state.borrow_mut().picker =
            Some((kind, options.into_iter().map(|(_, id)| id).collect()));
    }

    fn picker_chosen(&self, app: &App, i: i32) {
        self.window.set_menu_open(false);
        let Some((kind, ids)) = self.state.borrow_mut().picker.take() else {
            return;
        };
        if i < 0 {
            return;
        }
        let Some(&choice) = ids.get(i as usize) else {
            return;
        };
        let target = |app: &App| -> Option<Id> {
            match choice {
                Some(id) => Some(id),
                None => app
                    .lib
                    .borrow_mut()
                    .create_collection("New collection")
                    .ok()
                    .map(|c| c.id),
            }
        };
        let msg = match kind {
            Picker::MoveTo(sid) => target(app)
                .and_then(|t| app.lib.borrow_mut().move_snippets(&[sid], t).ok())
                .map(|_| "Moved"),
            Picker::CopyTo(sid) => target(app)
                .and_then(|t| app.lib.borrow_mut().copy_snippets(&[sid], t).ok())
                .map(|_| "Copied"),
            Picker::SaveTo(hid) => target(app)
                .and_then(|t| {
                    app.lib
                        .borrow_mut()
                        .save_history_to_collection(hid, Some(t), false)
                        .ok()
                })
                .map(|_| "Saved to collection"),
            Picker::Export => {
                self.export(app, choice.unwrap_or(-1));
                None
            }
        };
        if let Some(msg) = msg {
            self.refresh_all(app);
            self.toast(msg, Some(ToastAction::Undo));
        }
    }

    // ------------------------------------------------------------- import/export

    fn import(&self, app: &App) {
        let Some(path) = platform::shell::pick_import_file() else {
            return;
        };
        self.import_path(app, path);
    }

    /// One-click import: every sheet becomes a collection. "Adjust…" opens
    /// the wizard for the same file.
    pub fn import_path(&self, app: &App, path: PathBuf) {
        let wb = match shiftpaste_core::import::read_file(&path) {
            Ok(wb) => wb,
            Err(e) => return self.toast(&e.to_string(), None),
        };
        let plans = shiftpaste_core::import::one_click_plans(&wb);
        if plans.is_empty() {
            return self.toast("Nothing to import: the file has no rows.", None);
        }
        let result = app.lib.borrow_mut().import(&wb, &plans);
        match result {
            Ok(report) => {
                if let Some((id, _)) = report.created.first() {
                    self.state.borrow_mut().view = View::Collection(*id);
                }
                self.window.set_filter(SharedString::new());
                self.refresh_all(app);
                let into = match report.created.as_slice() {
                    [(_, name)] => format!("into “{name}”"),
                    many => format!("into {} collections", many.len()),
                };
                let mut msg = format!(
                    "Imported {} snippet{} {into}",
                    group(report.added as i64),
                    if report.added == 1 { "" } else { "s" }
                );
                if report.skipped_duplicates > 0 {
                    msg += &format!(" · {} duplicates skipped", report.skipped_duplicates);
                }
                self.toast(&msg, Some(ToastAction::AdjustImport(path)));
            }
            Err(e) => self.toast(&e.to_string(), None),
        }
    }

    fn export(&self, app: &App, choice: i64) {
        let lib = app.lib.borrow();
        let view = self.state.borrow().view;
        let name = match view {
            View::Collection(id) => lib
                .store()
                .collection(id)
                .map(|c| c.name)
                .unwrap_or_else(|_| "Collection".into()),
            _ => "Collection".into(),
        };
        let csv = [("CSV (opens in Excel)", "csv")];
        let xlsx = [("Excel workbook", "xlsx")];
        let (title, file, formats): (&str, String, &[(&str, &str)]) = match choice {
            0 => ("Export collection", format!("{name}.csv"), &csv),
            1 => ("Export collection", format!("{name}.xlsx"), &xlsx),
            2 => (
                "Export all collections",
                "Shift Paste collections.xlsx".into(),
                &xlsx,
            ),
            3 => ("Export history", "Shift Paste history.csv".into(), &csv),
            4 => ("Export history", "Shift Paste history.xlsx".into(), &xlsx),
            _ => return,
        };
        drop(lib);
        let Some(path) = platform::shell::save_file(title, &file, formats) else {
            return;
        };
        let lib = app.lib.borrow();
        let result = match (choice, view) {
            (0 | 1, View::Collection(id)) => lib.export_collection(id, &path),
            (2, _) => lib.export_all(&path),
            (3 | 4, _) => lib.export_history(&path),
            _ => return,
        };
        match result {
            Ok(r) => {
                let mut msg = format!(
                    "Exported {} row{}",
                    group(r.rows as i64),
                    if r.rows == 1 { "" } else { "s" }
                );
                if r.shortened > 0 {
                    msg += &format!(
                        " · {} long items shortened to fit Excel (use CSV to keep them whole)",
                        r.shortened
                    );
                }
                self.toast(&msg, None);
            }
            Err(e) => self.toast(&e.to_string(), None),
        }
    }

    // ------------------------------------------------------- confirm / toasts

    fn confirm(&self, what: Confirm, title: &str, message: &str, action: &str) {
        self.window.set_confirm_title(title.into());
        self.window.set_confirm_message(message.into());
        self.window.set_confirm_action(action.into());
        self.window.set_confirm_open(true);
        self.state.borrow_mut().confirm = Some(what);
    }

    fn confirmed(&self, app: &App, ok: bool) {
        self.window.set_confirm_open(false);
        let Some(what) = self.state.borrow_mut().confirm.take() else {
            return;
        };
        if !ok {
            return;
        }
        match what {
            Confirm::DeleteCollection(id) => {
                let name = app
                    .lib
                    .borrow()
                    .store()
                    .collection(id)
                    .map(|c| c.name)
                    .unwrap_or_default();
                if app.lib.borrow_mut().delete_collection(id).is_ok() {
                    self.refresh_all(app);
                    self.toast(&format!("Deleted “{name}”"), Some(ToastAction::Undo));
                }
            }
            Confirm::ClearHistory => {
                let n = app.lib.borrow_mut().clear_history().unwrap_or(0);
                self.refresh_all(app);
                if app.popup.window().is_visible() {
                    app.refresh_popup(false);
                }
                self.toast(&format!("Cleared {} history items", group(n as i64)), None);
            }
        }
    }

    fn toast(&self, text: &str, action: Option<ToastAction>) {
        self.window.set_toast_text(text.into());
        self.window.set_toast_action(
            match &action {
                Some(ToastAction::Undo) => "Undo",
                Some(ToastAction::AdjustImport(_)) => "Adjust…",
                None => "",
            }
            .into(),
        );
        let timer = Timer::default();
        timer.start(slint::TimerMode::SingleShot, Duration::from_secs(6), || {
            with_ed(|_, ed| {
                ed.window.set_toast_text(SharedString::new());
                ed.state.borrow_mut().toast = None;
            })
        });
        let mut st = self.state.borrow_mut();
        st.toast = action;
        st.toast_timer = Some(timer);
    }

    fn toast_clicked(&self, app: &Rc<App>) {
        let action = self.state.borrow_mut().toast.take();
        self.window.set_toast_text(SharedString::new());
        match action {
            Some(ToastAction::Undo) => self.undo_redo(app, true),
            Some(ToastAction::AdjustImport(path)) => {
                // Undo the one-click import, then let the user fine-tune it.
                let _ = app.lib.borrow_mut().undo();
                self.refresh_all(app);
                crate::import_ctl::open_wizard(app, path);
            }
            None => {}
        }
    }

    fn key(&self, app: &App, key: &str, ctrl: bool, shift: bool) -> bool {
        let Some(k) = key.chars().next() else {
            return false;
        };
        if k == char::from(Key::Escape) {
            if self.window.get_menu_open() {
                self.picker_chosen(app, -1);
                return true;
            }
            if self.window.get_confirm_open() {
                self.confirmed(app, false);
                return true;
            }
            return false;
        }
        if !ctrl {
            return false;
        }
        match k.to_ascii_lowercase() {
            'z' if shift => self.undo_redo(app, false),
            'z' => self.undo_redo(app, true),
            'y' => self.undo_redo(app, false),
            'n' => self.window.invoke_item_new(),
            _ => return false,
        }
        true
    }
}

/// Filter: the typed letters (spaces ignored) appear in order in the title or content.
fn matches(filter: &str, title: &str, content: &str) -> bool {
    if filter.is_empty() {
        return true;
    }
    let in_order = |hay: &str| {
        let mut chars = filter.chars();
        let mut want = chars.next();
        for c in hay.chars().flat_map(char::to_lowercase) {
            if Some(c) == want {
                want = chars.next();
                if want.is_none() {
                    return true;
                }
            }
        }
        false
    };
    in_order(title) || in_order(content)
}

fn snippet_row(s: &Snippet) -> ListItem {
    let title = s.display_title();
    let preview = if s.title.trim().is_empty() {
        let rest = s
            .content
            .trim_start()
            .strip_prefix(first_line(&s.content))
            .unwrap_or_default();
        one_line(rest, 140)
    } else {
        one_line(&s.content, 140)
    };
    ListItem {
        title: if title.is_empty() {
            "New snippet".into()
        } else {
            title.into()
        },
        preview: preview.into(),
        meta: SharedString::new(),
        starred: s.is_starred(),
        rich: false,
    }
}

fn history_row(h: &HistoryItem, now: i64) -> ListItem {
    let first = first_line(&h.content);
    let rest = h
        .content
        .trim_start()
        .strip_prefix(first)
        .unwrap_or_default();
    ListItem {
        title: one_line(first, 140).into(),
        preview: one_line(rest, 140).into(),
        meta: time_ago(h.last_copied_at, now).into(),
        starred: false,
        rich: h.has_formatting(),
    }
}

fn snippet_meta(s: &Snippet) -> String {
    let mut parts = vec![format!("Created {}", format_local(s.created_at))];
    if s.updated_at != s.created_at {
        parts.push(format!(
            "edited {}",
            time_ago(s.updated_at, now_ms()).to_lowercase()
        ));
    }
    if s.use_count > 0 {
        parts.push(format!(
            "pasted {} time{}",
            s.use_count,
            if s.use_count == 1 { "" } else { "s" }
        ));
    }
    parts.join(" · ")
}

fn history_meta(h: &HistoryItem) -> String {
    let mut parts = vec![format!(
        "Copied {}",
        time_ago(h.last_copied_at, now_ms()).to_lowercase()
    )];
    if h.copy_count > 1 {
        parts.push(format!("{} times", h.copy_count));
    }
    if let Some(app) = &h.source_app {
        parts.push(format!("from {app}"));
    }
    if h.has_formatting() {
        parts.push("has formatting".into());
    }
    parts.push(format!("first copied {}", format_local(h.first_copied_at)));
    parts.join(" · ")
}

fn tags_model(tags: &[String]) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        tags.iter()
            .map(|t| SharedString::from(t.as_str()))
            .collect::<Vec<_>>(),
    ))
}

fn short(s: &str) -> String {
    let mut out: String = s.chars().take(40).collect();
    if s.chars().count() > 40 {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::matches;

    #[test]
    fn filter_matches_in_order() {
        assert!(matches("", "", "anything"));
        assert!(matches("1884", "", "18 mm 8 x 4"));
        assert!(matches("ifsc", "Bank IFSC", "HDFC"));
        assert!(!matches("pro18", "", "18 mm Pro"));
        assert!(matches("école", "", "ÉCOLE"));
    }
}
