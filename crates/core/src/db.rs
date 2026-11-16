//! SQLite storage for collections, snippets and clipboard history.
//!
//! - WAL journal + foreign keys; every multi-row change runs in a transaction.
//! - Schema versions live in `PRAGMA user_version`; a database from a newer
//!   app version is refused rather than modified.
//! - A damaged database file is moved aside (never deleted) and a fresh one created.
//! - Ids use AUTOINCREMENT so a deleted id is never reused; undo/redo restores
//!   rows by their exact id and must never overwrite a newer row.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, ErrorCode, OptionalExtension, Row};
use sha2::{Digest, Sha256};

use crate::model::{clean_name, Collection, HistoryItem, Id, NewClip, Snippet};
use crate::{Error, Result};

/// Plain text longer than this is truncated before it is stored.
pub const MAX_TEXT_BYTES: usize = 1_000_000;
/// HTML/RTF payloads larger than this are dropped (the plain text is still kept).
pub const MAX_RICH_BYTES: usize = 2_000_000;

const SCHEMA_VERSION: i64 = 1;

const SCHEMA_V1: &str = "
CREATE TABLE collections (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT    NOT NULL,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);
CREATE TABLE snippets (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    title         TEXT    NOT NULL DEFAULT '',
    content       TEXT    NOT NULL,
    notes         TEXT    NOT NULL DEFAULT '',
    tags          TEXT    NOT NULL DEFAULT '',
    sort_order    INTEGER NOT NULL DEFAULT 0,
    starred_at    INTEGER,
    use_count     INTEGER NOT NULL DEFAULT 0,
    last_used_at  INTEGER,
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL
);
CREATE INDEX idx_snippets_collection ON snippets(collection_id, sort_order);
CREATE TABLE history (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    content         TEXT    NOT NULL,
    content_hash    TEXT    NOT NULL UNIQUE,
    html            TEXT,
    rtf             TEXT,
    source_app      TEXT,
    copy_count      INTEGER NOT NULL DEFAULT 1,
    first_copied_at INTEGER NOT NULL,
    last_copied_at  INTEGER NOT NULL
);
CREATE INDEX idx_history_recent ON history(last_copied_at DESC);
";

const COLLECTION_COLS: &str = "id, name, sort_order, created_at, updated_at";
const SNIPPET_COLS: &str = "id, collection_id, title, content, notes, tags, sort_order, \
                            starred_at, use_count, last_used_at, created_at, updated_at";
const HISTORY_COLS: &str = "id, content, content_hash, html, rtf, source_app, copy_count, \
                            first_copied_at, last_copied_at";

/// Result of opening a store from disk.
pub struct Opened {
    pub store: Store,
    /// Set when the existing file was damaged: where it was moved to.
    pub recovered_backup: Option<PathBuf>,
}

/// Outcome of recording a clipboard capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recorded {
    pub id: Id,
    /// False when the same text was already in history (it was bumped to the top).
    pub is_new: bool,
}

pub struct Store {
    conn: Connection,
}

impl Store {
    /// Open (or create) the database at `path`, recovering from a damaged file.
    pub fn open(path: &Path) -> Result<Opened> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        match Self::open_file(path) {
            Ok(store) => Ok(Opened {
                store,
                recovered_backup: None,
            }),
            Err(err) if path.exists() && is_corruption(&err) => {
                let backup = sibling(path, &format!(".damaged-{}", crate::time::now_ms()));
                std::fs::rename(path, &backup)?;
                for suffix in ["-wal", "-shm"] {
                    let side = sibling(path, suffix);
                    if side.exists() {
                        let _ = std::fs::rename(&side, sibling(&backup, suffix));
                    }
                }
                Ok(Opened {
                    store: Self::open_file(path)?,
                    recovered_backup: Some(backup),
                })
            }
            Err(err) => Err(err),
        }
    }

    pub fn open_in_memory() -> Result<Store> {
        Self::init(Connection::open_in_memory()?)
    }

    fn open_file(path: &Path) -> Result<Store> {
        let conn = Connection::open(path)?;
        let ok: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
        if ok != "ok" {
            return Err(Error::Corrupt);
        }
        conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        Self::init(conn)
    }

    fn init(conn: Connection) -> Result<Store> {
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.busy_timeout(std::time::Duration::from_secs(3))?;
        let mut store = Store { conn };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&mut self) -> Result<()> {
        let version: i64 = self
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version > SCHEMA_VERSION {
            return Err(Error::NewerSchema(version));
        }
        let tx = self.conn.transaction()?;
        if version < 1 {
            tx.execute_batch(SCHEMA_V1)?;
        }
        // Future migrations go here: `if version < 2 { ... }`.
        tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Run `f` inside one transaction; it is rolled back if `f` fails.
    pub fn transaction<T>(&mut self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let tx = self.conn.transaction()?;
        let out = f(&tx)?;
        tx.commit()?;
        Ok(out)
    }

    // ----------------------------------------------------------------- collections

    /// All collections in display order.
    pub fn collections(&self) -> Result<Vec<Collection>> {
        collections(&self.conn)
    }

    /// Number of snippets per collection id.
    pub fn snippet_counts(&self) -> Result<std::collections::HashMap<Id, i64>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT collection_id, COUNT(*) FROM snippets GROUP BY collection_id",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn collection(&self, id: Id) -> Result<Collection> {
        get_collection(&self.conn, id)?.ok_or(Error::NotFound)
    }

    /// Case-insensitive lookup by name.
    pub fn collection_by_name(&self, name: &str) -> Result<Option<Collection>> {
        let wanted = clean_name(name).to_lowercase();
        Ok(self
            .collections()?
            .into_iter()
            .find(|c| c.name.to_lowercase() == wanted))
    }

    /// `base`, or `base (2)`, `base (3)`… so that no other collection has the name.
    pub fn unique_collection_name(&self, base: &str) -> Result<String> {
        unique_collection_name(&self.conn, base, None)
    }

    // -------------------------------------------------------------------- snippets

    /// Snippets of one collection in their manual order.
    pub fn snippets(&self, collection_id: Id) -> Result<Vec<Snippet>> {
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {SNIPPET_COLS} FROM snippets WHERE collection_id = ?1 ORDER BY sort_order, id"
        ))?;
        let rows = stmt.query_map([collection_id], snippet_from_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn snippet(&self, id: Id) -> Result<Snippet> {
        get_snippet(&self.conn, id)?.ok_or(Error::NotFound)
    }

    pub fn snippets_by_ids(&self, ids: &[Id]) -> Result<Vec<Snippet>> {
        ids.iter().map(|&id| self.snippet(id)).collect()
    }

    /// Every tag in use, sorted, for autocomplete.
    pub fn all_tags(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT DISTINCT tags FROM snippets WHERE tags <> ''")?;
        let mut tags: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .iter()
            .flat_map(|t| split_tags(t))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        tags.sort();
        Ok(tags)
    }

    /// Record that a snippet was pasted (for ranking ties).
    pub fn mark_snippet_used(&self, id: Id, now: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE snippets SET use_count = use_count + 1, last_used_at = ?2 WHERE id = ?1",
            params![id, now],
        )?;
        Ok(())
    }

    // --------------------------------------------------------------------- history

    /// Save a clipboard capture. Identical text bumps the existing item instead of
    /// adding a duplicate. Returns `None` when there is nothing worth keeping.
    pub fn record_copy(&self, clip: &NewClip, now: i64) -> Result<Option<Recorded>> {
        let text = truncate_utf8(&clip.text, MAX_TEXT_BYTES);
        if text.trim().is_empty() {
            return Ok(None);
        }
        let (id, copy_count): (Id, i64) = self.conn.query_row(
            "INSERT INTO history (content, content_hash, html, rtf, source_app, copy_count,
                                  first_copied_at, last_copied_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?6)
             ON CONFLICT(content_hash) DO UPDATE SET
                 last_copied_at = excluded.last_copied_at,
                 copy_count     = history.copy_count + 1,
                 html           = COALESCE(excluded.html, history.html),
                 rtf            = COALESCE(excluded.rtf, history.rtf),
                 source_app     = COALESCE(excluded.source_app, history.source_app)
             RETURNING id, copy_count",
            params![
                text,
                hash_text(text),
                rich(&clip.html),
                rich(&clip.rtf),
                clip.source_app,
                now
            ],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok(Some(Recorded {
            id,
            is_new: copy_count == 1,
        }))
    }

    /// Move a history item to the top (used when it is pasted from Shift Paste).
    pub fn touch_history(&self, id: Id, now: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE history SET last_copied_at = ?2 WHERE id = ?1",
            params![id, now],
        )?;
        Ok(())
    }

    /// Most recent first.
    pub fn history(&self, limit: usize, offset: usize) -> Result<Vec<HistoryItem>> {
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {HISTORY_COLS} FROM history ORDER BY last_copied_at DESC, id DESC LIMIT ?1 OFFSET ?2"
        ))?;
        let rows = stmt.query_map(params![limit as i64, offset as i64], history_from_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn history_item(&self, id: Id) -> Result<HistoryItem> {
        get_history(&self.conn, id)?.ok_or(Error::NotFound)
    }

    pub fn history_by_hash(&self, hash: &str) -> Result<Option<HistoryItem>> {
        get_history_by_hash(&self.conn, hash)
    }

    pub fn history_count(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM history", [], |r| r.get(0))?)
    }

    /// Apply the count and age limits; returns how many items were removed.
    pub fn enforce_history_limits(
        &self,
        max_items: Option<u32>,
        max_age_days: Option<u32>,
        now: i64,
    ) -> Result<usize> {
        let mut removed = 0;
        if let Some(days) = max_age_days {
            let cutoff = now - i64::from(days) * crate::time::DAY;
            removed += self
                .conn
                .execute("DELETE FROM history WHERE last_copied_at < ?1", [cutoff])?;
        }
        if let Some(max) = max_items {
            removed += self.conn.execute(
                "DELETE FROM history WHERE id IN (
                     SELECT id FROM history ORDER BY last_copied_at DESC, id DESC
                     LIMIT -1 OFFSET ?1)",
                [i64::from(max)],
            )?;
        }
        Ok(removed)
    }

    /// Apps that history items were copied from, most recent first (for the app picker).
    pub fn recent_source_apps(&self, limit: usize) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT source_app FROM history WHERE source_app IS NOT NULL AND source_app <> ''
             GROUP BY source_app ORDER BY MAX(last_copied_at) DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Delete all history (collections are untouched). Returns how many were removed.
    pub fn clear_history(&self) -> Result<usize> {
        let n = self.conn.execute("DELETE FROM history", [])?;
        // Give the space back; failure here is harmless.
        let _ = self
            .conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); VACUUM;");
        Ok(n)
    }
}

// ------------------------------------------------------------------ helpers (pub(crate))
// Free functions take a `&Connection` so they work both directly and inside a
// transaction (a `Transaction` derefs to `Connection`).

pub(crate) fn collections(conn: &Connection) -> Result<Vec<Collection>> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {COLLECTION_COLS} FROM collections ORDER BY sort_order, id"
    ))?;
    let rows = stmt.query_map([], collection_from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub(crate) fn get_collection(conn: &Connection, id: Id) -> Result<Option<Collection>> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLLECTION_COLS} FROM collections WHERE id = ?1"),
            [id],
            collection_from_row,
        )
        .optional()?)
}

pub(crate) fn unique_collection_name(
    conn: &Connection,
    base: &str,
    ignore_id: Option<Id>,
) -> Result<String> {
    let mut base = clean_name(base);
    if base.is_empty() {
        base = "Untitled".into();
    }
    let taken: HashSet<String> = collections(conn)?
        .into_iter()
        .filter(|c| Some(c.id) != ignore_id)
        .map(|c| c.name.to_lowercase())
        .collect();
    if !taken.contains(&base.to_lowercase()) {
        return Ok(base);
    }
    (2..)
        .map(|n| format!("{base} ({n})"))
        .find(|name| !taken.contains(&name.to_lowercase()))
        .ok_or_else(|| Error::invalid("No free collection name"))
}

pub(crate) fn name_taken(conn: &Connection, name: &str, ignore_id: Option<Id>) -> Result<bool> {
    let wanted = name.to_lowercase();
    Ok(collections(conn)?
        .iter()
        .any(|c| Some(c.id) != ignore_id && c.name.to_lowercase() == wanted))
}

pub(crate) fn next_collection_order(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM collections",
        [],
        |r| r.get(0),
    )?)
}

pub(crate) fn put_collection(conn: &Connection, c: &Collection) -> Result<()> {
    conn.execute(
        "INSERT INTO collections (id, name, sort_order, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(id) DO UPDATE SET name = excluded.name, sort_order = excluded.sort_order,
             created_at = excluded.created_at, updated_at = excluded.updated_at",
        params![c.id, c.name, c.sort_order, c.created_at, c.updated_at],
    )?;
    Ok(())
}

/// Insert a new collection; `c.id` is ignored and the new id returned.
pub(crate) fn insert_collection(conn: &Connection, c: &Collection) -> Result<Id> {
    conn.execute(
        "INSERT INTO collections (name, sort_order, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
        params![c.name, c.sort_order, c.created_at, c.updated_at],
    )?;
    Ok(conn.last_insert_rowid())
}

pub(crate) fn delete_collection(conn: &Connection, id: Id) -> Result<()> {
    conn.execute("DELETE FROM collections WHERE id = ?1", [id])?;
    Ok(())
}

pub(crate) fn get_snippet(conn: &Connection, id: Id) -> Result<Option<Snippet>> {
    Ok(conn
        .query_row(
            &format!("SELECT {SNIPPET_COLS} FROM snippets WHERE id = ?1"),
            [id],
            snippet_from_row,
        )
        .optional()?)
}

pub(crate) fn snippets_in(conn: &Connection, collection_id: Id) -> Result<Vec<Snippet>> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {SNIPPET_COLS} FROM snippets WHERE collection_id = ?1 ORDER BY sort_order, id"
    ))?;
    let rows = stmt.query_map([collection_id], snippet_from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub(crate) fn next_snippet_order(conn: &Connection, collection_id: Id) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM snippets WHERE collection_id = ?1",
        [collection_id],
        |r| r.get(0),
    )?)
}

/// Insert a new snippet; `s.id` is ignored and the new id returned.
pub(crate) fn insert_snippet(conn: &Connection, s: &Snippet) -> Result<Id> {
    conn.execute(
        "INSERT INTO snippets (collection_id, title, content, notes, tags, sort_order, starred_at,
                               use_count, last_used_at, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            s.collection_id,
            s.title,
            s.content,
            s.notes,
            s.tags.join(","),
            s.sort_order,
            s.starred_at,
            s.use_count,
            s.last_used_at,
            s.created_at,
            s.updated_at
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Insert or overwrite a snippet with its exact id (used by undo/redo).
pub(crate) fn put_snippet(conn: &Connection, s: &Snippet) -> Result<()> {
    conn.execute(
        "INSERT INTO snippets (id, collection_id, title, content, notes, tags, sort_order,
                               starred_at, use_count, last_used_at, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(id) DO UPDATE SET collection_id = excluded.collection_id,
             title = excluded.title, content = excluded.content, notes = excluded.notes,
             tags = excluded.tags, sort_order = excluded.sort_order,
             starred_at = excluded.starred_at, use_count = excluded.use_count,
             last_used_at = excluded.last_used_at, created_at = excluded.created_at,
             updated_at = excluded.updated_at",
        params![
            s.id,
            s.collection_id,
            s.title,
            s.content,
            s.notes,
            s.tags.join(","),
            s.sort_order,
            s.starred_at,
            s.use_count,
            s.last_used_at,
            s.created_at,
            s.updated_at
        ],
    )?;
    Ok(())
}

pub(crate) fn delete_snippet(conn: &Connection, id: Id) -> Result<()> {
    conn.execute("DELETE FROM snippets WHERE id = ?1", [id])?;
    Ok(())
}

pub(crate) fn get_history(conn: &Connection, id: Id) -> Result<Option<HistoryItem>> {
    Ok(conn
        .query_row(
            &format!("SELECT {HISTORY_COLS} FROM history WHERE id = ?1"),
            [id],
            history_from_row,
        )
        .optional()?)
}

pub(crate) fn get_history_by_hash(conn: &Connection, hash: &str) -> Result<Option<HistoryItem>> {
    Ok(conn
        .query_row(
            &format!("SELECT {HISTORY_COLS} FROM history WHERE content_hash = ?1"),
            [hash],
            history_from_row,
        )
        .optional()?)
}

/// Insert or overwrite a history item with its exact id (used by undo/redo).
pub(crate) fn put_history(conn: &Connection, h: &HistoryItem) -> Result<()> {
    conn.execute(
        "INSERT INTO history (id, content, content_hash, html, rtf, source_app, copy_count,
                              first_copied_at, last_copied_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(id) DO UPDATE SET content = excluded.content,
             content_hash = excluded.content_hash, html = excluded.html, rtf = excluded.rtf,
             source_app = excluded.source_app, copy_count = excluded.copy_count,
             first_copied_at = excluded.first_copied_at, last_copied_at = excluded.last_copied_at",
        params![
            h.id,
            h.content,
            h.content_hash,
            h.html,
            h.rtf,
            h.source_app,
            h.copy_count,
            h.first_copied_at,
            h.last_copied_at
        ],
    )?;
    Ok(())
}

pub(crate) fn delete_history(conn: &Connection, id: Id) -> Result<()> {
    conn.execute("DELETE FROM history WHERE id = ?1", [id])?;
    Ok(())
}

fn collection_from_row(r: &Row) -> rusqlite::Result<Collection> {
    Ok(Collection {
        id: r.get(0)?,
        name: r.get(1)?,
        sort_order: r.get(2)?,
        created_at: r.get(3)?,
        updated_at: r.get(4)?,
    })
}

pub(crate) fn snippet_from_row(r: &Row) -> rusqlite::Result<Snippet> {
    Ok(Snippet {
        id: r.get(0)?,
        collection_id: r.get(1)?,
        title: r.get(2)?,
        content: r.get(3)?,
        notes: r.get(4)?,
        tags: split_tags(&r.get::<_, String>(5)?),
        sort_order: r.get(6)?,
        starred_at: r.get(7)?,
        use_count: r.get(8)?,
        last_used_at: r.get(9)?,
        created_at: r.get(10)?,
        updated_at: r.get(11)?,
    })
}

pub(crate) fn history_from_row(r: &Row) -> rusqlite::Result<HistoryItem> {
    Ok(HistoryItem {
        id: r.get(0)?,
        content: r.get(1)?,
        content_hash: r.get(2)?,
        html: r.get(3)?,
        rtf: r.get(4)?,
        source_app: r.get(5)?,
        copy_count: r.get(6)?,
        first_copied_at: r.get(7)?,
        last_copied_at: r.get(8)?,
    })
}

fn split_tags(stored: &str) -> Vec<String> {
    stored
        .split(',')
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Formatting payloads over the size cap are dropped (the plain text is kept).
fn rich(payload: &Option<String>) -> Option<&str> {
    payload.as_deref().filter(|s| s.len() <= MAX_RICH_BYTES)
}

/// Hex SHA-256 of the text, used to de-duplicate history.
pub fn hash_text(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    let mut out = String::with_capacity(64);
    for byte in digest.iter() {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Cut `text` to at most `max` bytes without splitting a character.
pub fn truncate_utf8(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

fn is_corruption(err: &Error) -> bool {
    match err {
        Error::Corrupt => true,
        Error::Db(rusqlite::Error::SqliteFailure(e, _)) => {
            matches!(e.code, ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase)
        }
        _ => false,
    }
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::DAY;

    fn clip(text: &str) -> NewClip {
        NewClip {
            text: text.into(),
            ..Default::default()
        }
    }

    #[test]
    fn duplicate_copies_bump_instead_of_duplicating() {
        let store = Store::open_in_memory().unwrap();
        let a = store.record_copy(&clip("hello"), 1_000).unwrap().unwrap();
        let b = store.record_copy(&clip("world"), 2_000).unwrap().unwrap();
        let a2 = store.record_copy(&clip("hello"), 3_000).unwrap().unwrap();
        assert!(a.is_new && b.is_new && !a2.is_new);
        assert_eq!(a.id, a2.id);
        let items = store.history(10, 0).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].content, "hello");
        assert_eq!(items[0].copy_count, 2);
        assert_eq!(items[0].first_copied_at, 1_000);
        assert_eq!(items[0].last_copied_at, 3_000);
    }

    #[test]
    fn blank_copies_are_ignored_and_huge_ones_truncated() {
        let store = Store::open_in_memory().unwrap();
        assert!(store.record_copy(&clip("  \n\t "), 1).unwrap().is_none());
        let big = "é".repeat(MAX_TEXT_BYTES); // 2 bytes per char
        let rec = store.record_copy(&clip(&big), 1).unwrap().unwrap();
        let item = store.history_item(rec.id).unwrap();
        assert!(item.content.len() <= MAX_TEXT_BYTES);
        assert!(item.content.chars().all(|c| c == 'é'));
    }

    #[test]
    fn rich_text_is_kept_and_refreshed() {
        let store = Store::open_in_memory().unwrap();
        let mut c = clip("Bold");
        c.html = Some("<b>Bold</b>".into());
        let id = store.record_copy(&c, 1).unwrap().unwrap().id;
        // A later plain copy of the same text keeps the earlier formatting.
        store.record_copy(&clip("Bold"), 2).unwrap();
        assert_eq!(
            store.history_item(id).unwrap().html.as_deref(),
            Some("<b>Bold</b>")
        );
    }

    #[test]
    fn limits_by_count_and_age() {
        let store = Store::open_in_memory().unwrap();
        let now = 100 * DAY;
        for i in 0..10 {
            store
                .record_copy(&clip(&format!("item {i}")), now - i * DAY)
                .unwrap();
        }
        // Age: items older than 5 days go (item 6..9).
        assert_eq!(store.enforce_history_limits(None, Some(5), now).unwrap(), 4);
        // Count: keep the newest 3.
        assert_eq!(store.enforce_history_limits(Some(3), None, now).unwrap(), 3);
        let left: Vec<_> = store
            .history(10, 0)
            .unwrap()
            .into_iter()
            .map(|h| h.content)
            .collect();
        assert_eq!(left, vec!["item 0", "item 1", "item 2"]);
        assert_eq!(store.enforce_history_limits(None, None, now).unwrap(), 0);
    }

    #[test]
    fn unique_names_are_case_insensitive() {
        let store = Store::open_in_memory().unwrap();
        let c = Collection {
            id: 0,
            name: "Work".into(),
            sort_order: 0,
            created_at: 0,
            updated_at: 0,
        };
        insert_collection(store.conn(), &c).unwrap();
        assert_eq!(store.unique_collection_name("work").unwrap(), "work (2)");
        assert_eq!(store.unique_collection_name("  ").unwrap(), "Untitled");
        assert!(store.collection_by_name("WORK").unwrap().is_some());
    }

    #[test]
    fn deleting_a_collection_deletes_its_snippets() {
        let store = Store::open_in_memory().unwrap();
        let cid = insert_collection(
            store.conn(),
            &Collection {
                id: 0,
                name: "A".into(),
                sort_order: 0,
                created_at: 0,
                updated_at: 0,
            },
        )
        .unwrap();
        let sid = insert_snippet(
            store.conn(),
            &Snippet {
                collection_id: cid,
                content: "x".into(),
                tags: vec!["t".into()],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(store.snippet(sid).unwrap().tags, vec!["t"]);
        delete_collection(store.conn(), cid).unwrap();
        assert!(matches!(store.snippet(sid), Err(Error::NotFound)));
    }

    #[test]
    fn damaged_file_is_moved_aside_and_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shiftpaste.db");
        std::fs::write(
            &path,
            b"this is not a sqlite database at all, just junk bytes",
        )
        .unwrap();
        let opened = Store::open(&path).unwrap();
        let backup = opened
            .recovered_backup
            .clone()
            .expect("should report recovery");
        assert!(backup.exists());
        assert_eq!(opened.store.history_count().unwrap(), 0);
        // Reopening the new file is clean.
        drop(opened);
        assert!(Store::open(&path).unwrap().recovered_backup.is_none());
    }

    #[test]
    fn newer_schema_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.pragma_update(None, "user_version", 99).unwrap();
        }
        assert!(matches!(Store::open(&path), Err(Error::NewerSchema(99))));
        assert!(path.exists(), "must not touch a newer database");
    }
}
