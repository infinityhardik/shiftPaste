//! The library: storage + search index + undo/redo, always kept in sync.
//!
//! Every change made from the editor or popup goes through a [`Library`] method.
//! Each one runs in a single database transaction and records the "before" and
//! "after" rows as [`Op`]s, so undo/redo is generic: it just re-applies rows.
//! Rapid edits of the same item (autosave while typing) merge into one undo step.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::db::{self, Recorded, Store, MAX_TEXT_BYTES};
use crate::model::{clean_name, normalize_tags, Collection, HistoryItem, Id, NewClip, Snippet};
use crate::search::{Kind, Results, SearchIndex, INDEX_CHARS};
use crate::{Error, Result};

/// Undo steps kept per session.
const UNDO_LIMIT: usize = 200;
/// Edits of the same item closer together than this form one undo step.
const COALESCE_MS: i64 = 1_500;
/// Collection used by "save to favorites" (Ctrl+D on a history item).
pub const FAVORITES_NAME: &str = "Favorites";

/// One row-level change; undo/redo are lists of these.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    PutCollection(Collection),
    DeleteCollection(Id),
    PutSnippet(Snippet),
    DeleteSnippet(Id),
    PutHistory(HistoryItem),
    DeleteHistory(Id),
}

#[derive(Debug, Clone)]
struct Change {
    label: String,
    undo: Vec<Op>,
    redo: Vec<Op>,
    /// (item, last edit time) for merging rapid edits.
    coalesce: Option<((Kind, Id), i64)>,
}

/// The editable fields of a snippet.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SnippetDraft {
    pub title: String,
    pub content: String,
    pub notes: String,
    pub tags: Vec<String>,
}

impl SnippetDraft {
    pub fn with_content(content: impl Into<String>) -> Self {
        SnippetDraft {
            content: content.into(),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HistoryLimits {
    pub max_items: Option<u32>,
    pub max_age_days: Option<u32>,
}

pub(crate) struct Applied<T> {
    pub redo: Vec<Op>,
    pub undo: Vec<Op>,
    pub value: T,
}

type Clock = Box<dyn Fn() -> i64 + Send>;

pub struct Library {
    store: Store,
    index: SearchIndex,
    undo: Vec<Change>,
    redo: Vec<Change>,
    clock: Clock,
}

impl Library {
    /// Open the library at `path`. The second value is set when a damaged
    /// database was moved aside and replaced (tell the user where it went).
    pub fn open(path: &Path) -> Result<(Library, Option<PathBuf>)> {
        let opened = Store::open(path)?;
        let lib = Library::from_store(opened.store)?;
        Ok((lib, opened.recovered_backup))
    }

    pub fn in_memory() -> Result<Library> {
        Library::from_store(Store::open_in_memory()?)
    }

    fn from_store(store: Store) -> Result<Library> {
        let mut lib = Library {
            store,
            index: SearchIndex::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            clock: Box::new(crate::time::now_ms),
        };
        lib.reload_index()?;
        Ok(lib)
    }

    /// Use a custom clock (tests).
    pub fn with_clock(mut self, clock: impl Fn() -> i64 + Send + 'static) -> Self {
        self.clock = Box::new(clock);
        self
    }

    fn now(&self) -> i64 {
        (self.clock)()
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    // --------------------------------------------------------------------- search

    pub fn search(&mut self, query: &str, limit: usize) -> Results {
        self.index.search(query, limit)
    }

    pub fn favorites(&mut self) -> Results {
        self.index.favorites()
    }

    /// Rebuild the in-memory index from the database.
    pub fn reload_index(&mut self) -> Result<()> {
        self.index.clear();
        for c in self.store.collections()? {
            self.index.set_collection(&c);
        }
        let conn = self.store.conn();
        let mut stmt = conn.prepare(&format!(
            "SELECT id, collection_id, title, substr(content, 1, {INDEX_CHARS}), notes, tags,
                    sort_order, starred_at, use_count, last_used_at, created_at, updated_at
             FROM snippets"
        ))?;
        for s in stmt.query_map([], db::snippet_from_row)? {
            let s = s?;
            if !s.content.trim().is_empty() {
                self.index.upsert_snippet(&s);
            }
        }
        let mut stmt = conn.prepare(&format!(
            "SELECT id, substr(content, 1, {INDEX_CHARS}), content_hash, html IS NOT NULL,
                    rtf IS NOT NULL, source_app, copy_count, first_copied_at, last_copied_at
             FROM history"
        ))?;
        let rows = stmt.query_map([], |r| {
            Ok(HistoryItem {
                id: r.get(0)?,
                content: r.get(1)?,
                content_hash: r.get(2)?,
                // Only "has formatting" matters to the index.
                html: r.get::<_, bool>(3)?.then(String::new),
                rtf: r.get::<_, bool>(4)?.then(String::new),
                source_app: r.get(5)?,
                copy_count: r.get(6)?,
                first_copied_at: r.get(7)?,
                last_copied_at: r.get(8)?,
            })
        })?;
        for h in rows {
            self.index.upsert_history(&h?);
        }
        Ok(())
    }

    // ------------------------------------------------------- capture (not undoable)

    /// Save a clipboard capture and apply the history limits.
    pub fn record_copy(
        &mut self,
        clip: &NewClip,
        limits: HistoryLimits,
    ) -> Result<Option<Recorded>> {
        let recorded = self.store.record_copy(clip, self.now())?;
        if let Some(r) = recorded {
            let item = self.store.history_item(r.id)?;
            self.index.upsert_history(&item);
        }
        self.enforce_limits(limits)?;
        Ok(recorded)
    }

    /// Apply the history limits; returns how many items were removed.
    pub fn enforce_limits(&mut self, limits: HistoryLimits) -> Result<usize> {
        let removed =
            self.store
                .enforce_history_limits(limits.max_items, limits.max_age_days, self.now())?;
        if removed > 0 {
            let mut stmt = self.store.conn().prepare_cached("SELECT id FROM history")?;
            let remaining: HashSet<Id> = stmt
                .query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            for id in self.index.history_ids().difference(&remaining) {
                self.index.remove(Kind::History, *id);
            }
        }
        Ok(removed)
    }

    /// Delete all history (asks the user first in the UI; not undoable).
    pub fn clear_history(&mut self) -> Result<usize> {
        let n = self.store.clear_history()?;
        for id in self.index.history_ids() {
            self.index.remove(Kind::History, id);
        }
        // Undo steps that touch history would resurrect single items; drop them.
        let touches_history = |c: &Change| {
            c.undo
                .iter()
                .chain(&c.redo)
                .any(|op| matches!(op, Op::PutHistory(_) | Op::DeleteHistory(_)))
        };
        self.undo.retain(|c| !touches_history(c));
        self.redo.retain(|c| !touches_history(c));
        Ok(n)
    }

    /// Record that an item was pasted: history moves to the top, snippets count a use.
    pub fn mark_used(&mut self, kind: Kind, id: Id) -> Result<()> {
        let now = self.now();
        match kind {
            Kind::History => self.store.touch_history(id, now)?,
            Kind::Snippet => self.store.mark_snippet_used(id, now)?,
        }
        self.index.touch(kind, id, now);
        Ok(())
    }

    // ---------------------------------------------------------------- collections

    pub fn collections(&self) -> Result<Vec<Collection>> {
        self.store.collections()
    }

    pub fn create_collection(&mut self, name: &str) -> Result<Collection> {
        self.change("New collection", None, |tx, now| {
            let c = new_collection(tx, name, now)?;
            Ok(Applied {
                redo: vec![Op::PutCollection(c.clone())],
                undo: vec![Op::DeleteCollection(c.id)],
                value: c,
            })
        })
    }

    pub fn rename_collection(&mut self, id: Id, name: &str) -> Result<()> {
        let name = clean_name(name);
        if name.is_empty() {
            return Err(Error::invalid("The collection name can't be empty."));
        }
        if self.store.collection(id)?.name == name {
            return Ok(());
        }
        self.change("Rename collection", None, |tx, now| {
            let before = db::get_collection(tx, id)?.ok_or(Error::NotFound)?;
            if db::name_taken(tx, &name, Some(id))? {
                return Err(Error::invalid(format!(
                    "A collection named “{name}” already exists."
                )));
            }
            let after = Collection {
                name: name.clone(),
                updated_at: now,
                ..before.clone()
            };
            db::put_collection(tx, &after)?;
            Ok(Applied {
                redo: vec![Op::PutCollection(after)],
                undo: vec![Op::PutCollection(before)],
                value: (),
            })
        })
    }

    pub fn delete_collection(&mut self, id: Id) -> Result<()> {
        let name = self.store.collection(id)?.name;
        self.change(format!("Delete “{name}”"), None, |tx, _| {
            let before = db::get_collection(tx, id)?.ok_or(Error::NotFound)?;
            let snippets = db::snippets_in(tx, id)?;
            db::delete_collection(tx, id)?;
            let mut undo = vec![Op::PutCollection(before)];
            undo.extend(snippets.into_iter().map(Op::PutSnippet));
            Ok(Applied {
                redo: vec![Op::DeleteCollection(id)],
                undo,
                value: (),
            })
        })
    }

    /// Set the display order of collections to `ids`.
    pub fn reorder_collections(&mut self, ids: &[Id]) -> Result<()> {
        self.change("Reorder collections", None, |tx, _| {
            let (mut redo, mut undo) = (Vec::new(), Vec::new());
            for (pos, &id) in ids.iter().enumerate() {
                let before = db::get_collection(tx, id)?.ok_or(Error::NotFound)?;
                if before.sort_order != pos as i64 {
                    let after = Collection {
                        sort_order: pos as i64,
                        ..before.clone()
                    };
                    db::put_collection(tx, &after)?;
                    redo.push(Op::PutCollection(after));
                    undo.push(Op::PutCollection(before));
                }
            }
            Ok(Applied {
                redo,
                undo,
                value: (),
            })
        })
    }

    // ------------------------------------------------------------------- snippets

    pub fn snippets(&self, collection_id: Id) -> Result<Vec<Snippet>> {
        self.store.snippets(collection_id)
    }

    pub fn snippet(&self, id: Id) -> Result<Snippet> {
        self.store.snippet(id)
    }

    /// Add a snippet at the end of a collection.
    pub fn create_snippet(&mut self, collection_id: Id, draft: SnippetDraft) -> Result<Snippet> {
        self.change("New snippet", None, |tx, now| {
            db::get_collection(tx, collection_id)?.ok_or(Error::NotFound)?;
            let mut s = Snippet {
                collection_id,
                sort_order: db::next_snippet_order(tx, collection_id)?,
                created_at: now,
                updated_at: now,
                ..Default::default()
            };
            apply_draft(&mut s, draft);
            s.id = db::insert_snippet(tx, &s)?;
            Ok(Applied {
                redo: vec![Op::PutSnippet(s.clone())],
                undo: vec![Op::DeleteSnippet(s.id)],
                value: s,
            })
        })
    }

    /// Save edits; rapid consecutive edits of one snippet are one undo step.
    pub fn update_snippet(&mut self, id: Id, draft: SnippetDraft) -> Result<Snippet> {
        self.change("Edit snippet", Some((Kind::Snippet, id)), |tx, now| {
            let before = db::get_snippet(tx, id)?.ok_or(Error::NotFound)?;
            let mut after = before.clone();
            apply_draft(&mut after, draft);
            if after == before {
                return Ok(Applied {
                    redo: vec![],
                    undo: vec![],
                    value: before,
                });
            }
            after.updated_at = now;
            db::put_snippet(tx, &after)?;
            Ok(Applied {
                redo: vec![Op::PutSnippet(after.clone())],
                undo: vec![Op::PutSnippet(before)],
                value: after,
            })
        })
    }

    pub fn delete_snippets(&mut self, ids: &[Id]) -> Result<()> {
        self.change(plural("Delete", ids.len(), "snippet"), None, |tx, _| {
            let (mut redo, mut undo) = (Vec::new(), Vec::new());
            for &id in ids {
                if let Some(before) = db::get_snippet(tx, id)? {
                    db::delete_snippet(tx, id)?;
                    redo.push(Op::DeleteSnippet(id));
                    undo.push(Op::PutSnippet(before));
                }
            }
            Ok(Applied {
                redo,
                undo,
                value: (),
            })
        })
    }

    /// Move snippets to the end of another collection, keeping their order.
    pub fn move_snippets(&mut self, ids: &[Id], target: Id) -> Result<()> {
        self.change(plural("Move", ids.len(), "snippet"), None, |tx, now| {
            db::get_collection(tx, target)?.ok_or(Error::NotFound)?;
            let mut order = db::next_snippet_order(tx, target)?;
            let (mut redo, mut undo) = (Vec::new(), Vec::new());
            for &id in ids {
                let before = db::get_snippet(tx, id)?.ok_or(Error::NotFound)?;
                if before.collection_id == target {
                    continue;
                }
                let after = Snippet {
                    collection_id: target,
                    sort_order: order,
                    updated_at: now,
                    ..before.clone()
                };
                order += 1;
                db::put_snippet(tx, &after)?;
                redo.push(Op::PutSnippet(after));
                undo.push(Op::PutSnippet(before));
            }
            Ok(Applied {
                redo,
                undo,
                value: (),
            })
        })
    }

    /// Copy snippets to the end of a collection; returns the new ids.
    pub fn copy_snippets(&mut self, ids: &[Id], target: Id) -> Result<Vec<Id>> {
        self.change(plural("Copy", ids.len(), "snippet"), None, |tx, now| {
            db::get_collection(tx, target)?.ok_or(Error::NotFound)?;
            let first = db::next_snippet_order(tx, target)?;
            let (mut redo, mut undo, mut new_ids) = (Vec::new(), Vec::new(), Vec::new());
            for (order, &id) in (first..).zip(ids) {
                let src = db::get_snippet(tx, id)?.ok_or(Error::NotFound)?;
                let mut copy = Snippet {
                    id: 0,
                    collection_id: target,
                    sort_order: order,
                    starred_at: None,
                    use_count: 0,
                    last_used_at: None,
                    created_at: now,
                    updated_at: now,
                    ..src
                };
                copy.id = db::insert_snippet(tx, &copy)?;
                new_ids.push(copy.id);
                undo.push(Op::DeleteSnippet(copy.id));
                redo.push(Op::PutSnippet(copy));
            }
            Ok(Applied {
                redo,
                undo,
                value: new_ids,
            })
        })
    }

    /// Set the manual order of a collection's snippets to `ids`.
    pub fn reorder_snippets(&mut self, collection_id: Id, ids: &[Id]) -> Result<()> {
        self.change("Reorder snippets", None, |tx, _| {
            let (mut redo, mut undo) = (Vec::new(), Vec::new());
            for (pos, &id) in ids.iter().enumerate() {
                let before = db::get_snippet(tx, id)?.ok_or(Error::NotFound)?;
                if before.collection_id != collection_id {
                    return Err(Error::invalid("Snippet is not in this collection."));
                }
                if before.sort_order != pos as i64 {
                    let after = Snippet {
                        sort_order: pos as i64,
                        ..before.clone()
                    };
                    db::put_snippet(tx, &after)?;
                    redo.push(Op::PutSnippet(after));
                    undo.push(Op::PutSnippet(before));
                }
            }
            Ok(Applied {
                redo,
                undo,
                value: (),
            })
        })
    }

    /// Add or remove snippets from favorites.
    pub fn set_starred(&mut self, ids: &[Id], starred: bool) -> Result<()> {
        let label = if starred {
            "Add to favorites"
        } else {
            "Remove from favorites"
        };
        self.change(label, None, |tx, now| {
            let (mut redo, mut undo) = (Vec::new(), Vec::new());
            for &id in ids {
                let before = db::get_snippet(tx, id)?.ok_or(Error::NotFound)?;
                if before.is_starred() == starred {
                    continue;
                }
                let after = Snippet {
                    starred_at: starred.then_some(now),
                    ..before.clone()
                };
                db::put_snippet(tx, &after)?;
                redo.push(Op::PutSnippet(after));
                undo.push(Op::PutSnippet(before));
            }
            Ok(Applied {
                redo,
                undo,
                value: (),
            })
        })
    }

    // -------------------------------------------------------------- history editing

    pub fn history(&self, limit: usize, offset: usize) -> Result<Vec<HistoryItem>> {
        self.store.history(limit, offset)
    }

    pub fn history_item(&self, id: Id) -> Result<HistoryItem> {
        self.store.history_item(id)
    }

    /// Change the text of a history item. Formatting is dropped (it no longer
    /// matches the text). If the new text equals another item, the two merge.
    pub fn update_history(&mut self, id: Id, content: &str) -> Result<()> {
        if content.trim().is_empty() {
            return Err(Error::invalid(
                "The text can't be empty. Delete the item instead.",
            ));
        }
        let content = db::truncate_utf8(content, MAX_TEXT_BYTES).to_owned();
        self.change("Edit history item", Some((Kind::History, id)), |tx, _| {
            let before = db::get_history(tx, id)?.ok_or(Error::NotFound)?;
            if before.content == content {
                return Ok(Applied {
                    redo: vec![],
                    undo: vec![],
                    value: (),
                });
            }
            let hash = db::hash_text(&content);
            let (mut redo, mut undo) = (Vec::new(), Vec::new());
            let mut after = HistoryItem {
                content: content.clone(),
                content_hash: hash.clone(),
                html: None,
                rtf: None,
                ..before.clone()
            };
            if let Some(dup) = db::get_history_by_hash(tx, &hash)?.filter(|d| d.id != id) {
                after.copy_count += dup.copy_count;
                after.first_copied_at = after.first_copied_at.min(dup.first_copied_at);
                after.last_copied_at = after.last_copied_at.max(dup.last_copied_at);
                db::delete_history(tx, dup.id)?;
                redo.push(Op::DeleteHistory(dup.id));
                undo.push(Op::PutHistory(dup));
            }
            db::put_history(tx, &after)?;
            redo.push(Op::PutHistory(after));
            // Restore the original first, then the merged-away duplicate.
            undo.insert(0, Op::PutHistory(before));
            Ok(Applied {
                redo,
                undo,
                value: (),
            })
        })
    }

    pub fn delete_history(&mut self, ids: &[Id]) -> Result<()> {
        self.change(
            plural("Delete", ids.len(), "history item"),
            None,
            |tx, _| {
                let (mut redo, mut undo) = (Vec::new(), Vec::new());
                for &id in ids {
                    if let Some(before) = db::get_history(tx, id)? {
                        db::delete_history(tx, id)?;
                        redo.push(Op::DeleteHistory(id));
                        undo.push(Op::PutHistory(before));
                    }
                }
                Ok(Applied {
                    redo,
                    undo,
                    value: (),
                })
            },
        )
    }

    /// Save a history item as a snippet. `target: None` means the "Favorites"
    /// collection (created on first use). If the collection already has the same
    /// text, that snippet is reused instead of making a duplicate.
    pub fn save_history_to_collection(
        &mut self,
        history_id: Id,
        target: Option<Id>,
        star: bool,
    ) -> Result<Snippet> {
        let label = if star {
            "Add to favorites"
        } else {
            "Save to collection"
        };
        self.change(label, None, |tx, now| {
            let h = db::get_history(tx, history_id)?.ok_or(Error::NotFound)?;
            let (mut redo, mut undo) = (Vec::new(), Vec::new());
            let collection_id = match target {
                Some(id) => db::get_collection(tx, id)?.ok_or(Error::NotFound)?.id,
                None => match db::collections(tx)?
                    .into_iter()
                    .find(|c| c.name.eq_ignore_ascii_case(FAVORITES_NAME))
                {
                    Some(c) => c.id,
                    None => {
                        let c = new_collection(tx, FAVORITES_NAME, now)?;
                        redo.push(Op::PutCollection(c.clone()));
                        undo.push(Op::DeleteCollection(c.id));
                        c.id
                    }
                },
            };
            let existing = db::snippets_in(tx, collection_id)?
                .into_iter()
                .find(|s| s.content == h.content);
            let snippet = match existing {
                Some(s) if !star || s.is_starred() => s,
                Some(before) => {
                    let after = Snippet {
                        starred_at: Some(now),
                        ..before.clone()
                    };
                    db::put_snippet(tx, &after)?;
                    redo.push(Op::PutSnippet(after.clone()));
                    undo.insert(0, Op::PutSnippet(before));
                    after
                }
                None => {
                    let mut s = Snippet {
                        collection_id,
                        content: h.content.clone(),
                        sort_order: db::next_snippet_order(tx, collection_id)?,
                        starred_at: star.then_some(now),
                        created_at: now,
                        updated_at: now,
                        ..Default::default()
                    };
                    s.id = db::insert_snippet(tx, &s)?;
                    redo.push(Op::PutSnippet(s.clone()));
                    // Delete the snippet before (possibly) its new collection.
                    undo.insert(0, Op::DeleteSnippet(s.id));
                    s
                }
            };
            Ok(Applied {
                redo,
                undo,
                value: snippet,
            })
        })
    }

    // ------------------------------------------------------------------ undo/redo

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Label of the step that `undo()` would revert, e.g. "Delete 3 snippets".
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|c| c.label.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|c| c.label.as_str())
    }

    /// Revert the last change; returns its label.
    pub fn undo(&mut self) -> Result<Option<String>> {
        let Some(change) = self.undo.pop() else {
            return Ok(None);
        };
        self.store.transaction(|tx| apply_ops(tx, &change.undo))?;
        self.sync_index(&change.undo);
        let label = change.label.clone();
        self.redo.push(Change {
            coalesce: None,
            ..change
        });
        Ok(Some(label))
    }

    /// Re-apply the last undone change; returns its label.
    pub fn redo(&mut self) -> Result<Option<String>> {
        let Some(change) = self.redo.pop() else {
            return Ok(None);
        };
        self.store.transaction(|tx| apply_ops(tx, &change.redo))?;
        self.sync_index(&change.redo);
        let label = change.label.clone();
        self.undo.push(change);
        Ok(Some(label))
    }

    /// Run one undoable change in a transaction and record it.
    pub(crate) fn change<T>(
        &mut self,
        label: impl Into<String>,
        coalesce: Option<(Kind, Id)>,
        f: impl FnOnce(&Connection, i64) -> Result<Applied<T>>,
    ) -> Result<T> {
        let now = self.now();
        let applied = self.store.transaction(|tx| f(tx, now))?;
        if applied.redo.is_empty() {
            return Ok(applied.value);
        }
        self.sync_index(&applied.redo);
        self.redo.clear();
        if let (Some(key), Some(top)) = (coalesce, self.undo.last_mut()) {
            if matches!(top.coalesce, Some((k, at)) if k == key && now - at <= COALESCE_MS) {
                top.redo = applied.redo;
                top.coalesce = Some((key, now));
                return Ok(applied.value);
            }
        }
        self.undo.push(Change {
            label: label.into(),
            undo: applied.undo,
            redo: applied.redo,
            coalesce: coalesce.map(|k| (k, now)),
        });
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        Ok(applied.value)
    }

    fn sync_index(&mut self, ops: &[Op]) {
        for op in ops {
            match op {
                Op::PutCollection(c) => self.index.set_collection(c),
                Op::DeleteCollection(id) => self.index.remove_collection(*id),
                Op::PutSnippet(s) if s.content.trim().is_empty() => {
                    self.index.remove(Kind::Snippet, s.id)
                }
                Op::PutSnippet(s) => self.index.upsert_snippet(s),
                Op::DeleteSnippet(id) => self.index.remove(Kind::Snippet, *id),
                Op::PutHistory(h) => self.index.upsert_history(h),
                Op::DeleteHistory(id) => self.index.remove(Kind::History, *id),
            }
        }
    }
}

pub(crate) fn new_collection(tx: &Connection, name: &str, now: i64) -> Result<Collection> {
    let mut c = Collection {
        id: 0,
        name: db::unique_collection_name(tx, name, None)?,
        sort_order: db::next_collection_order(tx)?,
        created_at: now,
        updated_at: now,
    };
    c.id = db::insert_collection(tx, &c)?;
    Ok(c)
}

pub(crate) fn apply_draft(s: &mut Snippet, draft: SnippetDraft) {
    s.title = draft.title.trim().to_owned();
    s.content = db::truncate_utf8(&draft.content, MAX_TEXT_BYTES).to_owned();
    s.notes = draft.notes;
    s.tags = normalize_tags(draft.tags.iter().map(String::as_str));
}

fn apply_ops(tx: &Connection, ops: &[Op]) -> Result<()> {
    for op in ops {
        match op {
            Op::PutCollection(c) => db::put_collection(tx, c)?,
            Op::DeleteCollection(id) => db::delete_collection(tx, *id)?,
            Op::PutSnippet(s) => db::put_snippet(tx, s)?,
            Op::DeleteSnippet(id) => db::delete_snippet(tx, *id)?,
            Op::PutHistory(h) => db::put_history(tx, h)?,
            Op::DeleteHistory(id) => db::delete_history(tx, *id)?,
        }
    }
    Ok(())
}

fn plural(verb: &str, n: usize, noun: &str) -> String {
    if n == 1 {
        format!("{verb} {noun}")
    } else {
        format!("{verb} {n} {noun}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI64, Ordering};
    use std::sync::Arc;

    fn lib_with_clock() -> (Library, Arc<AtomicI64>) {
        let t = Arc::new(AtomicI64::new(1_000_000));
        let t2 = t.clone();
        let lib = Library::in_memory()
            .unwrap()
            .with_clock(move || t2.load(Ordering::SeqCst));
        (lib, t)
    }

    fn draft(title: &str, content: &str) -> SnippetDraft {
        SnippetDraft {
            title: title.into(),
            content: content.into(),
            ..Default::default()
        }
    }

    #[test]
    fn create_edit_undo_redo() {
        let (mut lib, t) = lib_with_clock();
        let c = lib.create_collection("Work").unwrap();
        let s = lib.create_snippet(c.id, draft("Bank", "HDFC")).unwrap();
        assert_eq!(lib.search("hdfc", 10).hits.len(), 1);

        // Three quick edits merge into one undo step.
        for text in ["HDFC0", "HDFC00", "HDFC0001234"] {
            t.fetch_add(300, Ordering::SeqCst);
            lib.update_snippet(s.id, draft("Bank", text)).unwrap();
        }
        assert_eq!(lib.undo_label(), Some("Edit snippet"));
        lib.undo().unwrap();
        assert_eq!(lib.snippet(s.id).unwrap().content, "HDFC");
        lib.redo().unwrap();
        assert_eq!(lib.snippet(s.id).unwrap().content, "HDFC0001234");

        // A later edit (after the merge window) is its own step.
        t.fetch_add(5_000, Ordering::SeqCst);
        lib.update_snippet(s.id, draft("Bank IFSC", "HDFC0001234"))
            .unwrap();
        lib.undo().unwrap();
        assert_eq!(lib.snippet(s.id).unwrap().title, "Bank");

        // Undo everything back to an empty library.
        while lib.can_undo() {
            lib.undo().unwrap();
        }
        assert!(lib.collections().unwrap().is_empty());
        assert!(lib.search("hdfc", 10).hits.is_empty());
        // ...and redo it all.
        while lib.can_redo() {
            lib.redo().unwrap();
        }
        assert_eq!(lib.snippet(s.id).unwrap().title, "Bank IFSC");
        assert_eq!(lib.search("hdfc", 10).hits.len(), 1);
    }

    #[test]
    fn unchanged_edits_do_not_create_undo_steps() {
        let (mut lib, _) = lib_with_clock();
        let c = lib.create_collection("A").unwrap();
        let s = lib.create_snippet(c.id, draft("", "x")).unwrap();
        lib.update_snippet(s.id, draft("", "x")).unwrap();
        assert_eq!(lib.undo_label(), Some("New snippet"));
    }

    #[test]
    fn delete_collection_is_undoable_with_its_snippets() {
        let (mut lib, _) = lib_with_clock();
        let c = lib.create_collection("Work").unwrap();
        for i in 0..3 {
            lib.create_snippet(c.id, draft("", &format!("item {i}")))
                .unwrap();
        }
        lib.delete_collection(c.id).unwrap();
        assert!(lib.search("@work", 10).hits.is_empty());
        lib.undo().unwrap();
        assert_eq!(lib.snippets(c.id).unwrap().len(), 3);
        assert_eq!(lib.search("@work", 10).hits.len(), 3);
    }

    #[test]
    fn deleted_ids_are_never_reused() {
        let (mut lib, _) = lib_with_clock();
        let c = lib.create_collection("A").unwrap();
        let s1 = lib.create_snippet(c.id, draft("", "one")).unwrap();
        lib.delete_snippets(&[s1.id]).unwrap();
        let s2 = lib.create_snippet(c.id, draft("", "two")).unwrap();
        assert_ne!(s1.id, s2.id);
        // Undo the create, then undo the delete: "one" comes back, "two" is gone.
        lib.undo().unwrap();
        lib.undo().unwrap();
        assert_eq!(lib.snippet(s1.id).unwrap().content, "one");
    }

    #[test]
    fn move_copy_reorder_star() {
        let (mut lib, _) = lib_with_clock();
        let a = lib.create_collection("A").unwrap();
        let b = lib.create_collection("B").unwrap();
        let ids: Vec<Id> = (0..3)
            .map(|i| {
                lib.create_snippet(a.id, draft("", &format!("s{i}")))
                    .unwrap()
                    .id
            })
            .collect();
        lib.reorder_snippets(a.id, &[ids[2], ids[0], ids[1]])
            .unwrap();
        let order: Vec<_> = lib
            .snippets(a.id)
            .unwrap()
            .into_iter()
            .map(|s| s.content)
            .collect();
        assert_eq!(order, ["s2", "s0", "s1"]);

        lib.move_snippets(&[ids[0]], b.id).unwrap();
        assert_eq!(lib.snippets(b.id).unwrap().len(), 1);
        let copies = lib.copy_snippets(&[ids[1]], b.id).unwrap();
        assert_eq!(lib.snippets(b.id).unwrap().len(), 2);
        assert_eq!(lib.undo_label(), Some("Copy snippet"));

        lib.set_starred(&[copies[0]], true).unwrap();
        assert!(lib.snippet(copies[0]).unwrap().is_starred());
        assert_eq!(lib.search("", 10).hits[0].id, copies[0]);
        lib.undo().unwrap();
        assert!(!lib.snippet(copies[0]).unwrap().is_starred());
    }

    #[test]
    fn rename_validation() {
        let (mut lib, _) = lib_with_clock();
        let a = lib.create_collection("Work").unwrap();
        let b = lib.create_collection("Home").unwrap();
        assert!(matches!(
            lib.rename_collection(b.id, " work "),
            Err(Error::Invalid(_))
        ));
        assert!(matches!(
            lib.rename_collection(b.id, "   "),
            Err(Error::Invalid(_))
        ));
        lib.rename_collection(a.id, "Office").unwrap();
        assert_eq!(lib.search("@off", 10).mode, crate::search::Mode::Filtered);
        // Creating a duplicate name auto-suffixes instead of failing.
        assert_eq!(lib.create_collection("home").unwrap().name, "home (2)");
    }

    #[test]
    fn history_edit_merge_delete_and_favorite() {
        let (mut lib, t) = lib_with_clock();
        let limits = HistoryLimits::default();
        let clip = |s: &str| NewClip {
            text: s.into(),
            ..Default::default()
        };
        let a = lib.record_copy(&clip("alpha"), limits).unwrap().unwrap().id;
        t.fetch_add(10, Ordering::SeqCst);
        let b = lib.record_copy(&clip("beta"), limits).unwrap().unwrap().id;

        // Editing "beta" into "alpha" merges the two items.
        lib.update_history(b, "alpha").unwrap();
        assert_eq!(lib.store().history_count().unwrap(), 1);
        assert_eq!(lib.history_item(b).unwrap().copy_count, 2);
        lib.undo().unwrap();
        assert_eq!(lib.history_item(a).unwrap().content, "alpha");
        assert_eq!(lib.history_item(b).unwrap().content, "beta");

        // Ctrl+D on a history item: saved to an auto-created, starred Favorites.
        let s = lib.save_history_to_collection(a, None, true).unwrap();
        assert!(s.is_starred());
        let fav = lib
            .store()
            .collection_by_name("favorites")
            .unwrap()
            .unwrap();
        assert_eq!(s.collection_id, fav.id);
        // Doing it again reuses the same snippet.
        let again = lib.save_history_to_collection(a, None, true).unwrap();
        assert_eq!(again.id, s.id);
        // Undo removes both the snippet and the auto-created collection.
        lib.undo().unwrap();
        assert!(lib.collections().unwrap().is_empty());

        lib.delete_history(&[a, b]).unwrap();
        assert!(lib.search("", 10).hits.is_empty());
        lib.undo().unwrap();
        assert_eq!(lib.search("", 10).hits.len(), 2);
        assert!(matches!(
            lib.update_history(a, "  "),
            Err(Error::Invalid(_))
        ));
    }

    #[test]
    fn capture_respects_limits_and_index() {
        let (mut lib, t) = lib_with_clock();
        let limits = HistoryLimits {
            max_items: Some(3),
            max_age_days: None,
        };
        for i in 0..5 {
            t.fetch_add(10, Ordering::SeqCst);
            lib.record_copy(
                &NewClip {
                    text: format!("copy {i}"),
                    ..Default::default()
                },
                limits,
            )
            .unwrap();
        }
        let shown: Vec<_> = lib
            .search("", 10)
            .hits
            .into_iter()
            .map(|h| h.title)
            .collect();
        assert_eq!(shown, ["copy 4", "copy 3", "copy 2"]);
        let id = lib.search("copy 2", 1).hits[0].id;
        t.fetch_add(10, Ordering::SeqCst);
        lib.mark_used(Kind::History, id).unwrap();
        assert_eq!(lib.search("", 10).hits[0].title, "copy 2");
        assert_eq!(lib.clear_history().unwrap(), 3);
        assert!(lib.search("", 10).hits.is_empty());
    }

    #[test]
    fn empty_snippets_are_not_offered_for_pasting() {
        let (mut lib, _) = lib_with_clock();
        let c = lib.create_collection("A").unwrap();
        let s = lib.create_snippet(c.id, draft("Draft", "")).unwrap();
        assert!(lib.search("@a", 10).hits.is_empty());
        lib.update_snippet(s.id, draft("Draft", "now has text"))
            .unwrap();
        assert_eq!(lib.search("@a", 10).hits.len(), 1);
    }
}
