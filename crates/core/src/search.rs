//! In-memory search over history and snippets.
//!
//! - Matching: the query's characters must appear **in order** (spaces in the
//!   query are ignored), scored fzf-style by `nucleo-matcher`: consecutive
//!   characters and word starts score higher. Case- and accent-insensitive.
//! - Ranking: best match first; results within ~3% of each other are
//!   ordered by recency.
//! - Filters: `@work` limits to collections whose name starts with "work",
//!   `#bank` to snippets tagged "bank". A filter on its own lists the matching
//!   snippets in their manual order.
//! - Empty query ("home"): favorites first, then recent history.

use std::collections::{HashMap, HashSet};

use nucleo_matcher::pattern::{Atom, AtomKind, CaseMatching, Normalization};
use nucleo_matcher::{Config, Matcher, Utf32Str, Utf32String};
use rayon::prelude::*;

use crate::model::{first_line, Collection, HistoryItem, Id, Snippet};

/// Characters of each item kept in memory for matching and previews.
pub const INDEX_CHARS: usize = 1_000;
/// Favorites shown on the home list before "Show all favorites…".
pub const HOME_FAVORITES: usize = 10;
/// Characters of preview text handed to the UI per row.
const PREVIEW_CHARS: usize = 240;
/// Above this many candidates, scoring runs on all CPU cores.
const PARALLEL_THRESHOLD: usize = 2_048;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    History,
    Snippet,
}

#[derive(Debug, Clone)]
struct Entry {
    kind: Kind,
    id: Id,
    /// 0 for history items.
    collection_id: Id,
    /// Pre-converted for the matcher (ASCII stays 1 byte per char).
    title: Utf32String,
    /// Start of the content (≤ INDEX_CHARS chars).
    text: Utf32String,
    tags: Vec<String>,
    starred_at: Option<i64>,
    /// When it was last copied (history) or used / created (snippets).
    recency: i64,
    sort_order: i64,
    has_formatting: bool,
}

#[derive(Debug, Clone)]
struct CollectionMeta {
    name: String,
    /// Lower-case name without spaces, for `@filter` prefix matching.
    key: String,
    order: i64,
}

/// What kind of list a result set is, so the UI can pick headings and empty states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Empty query: favorites then recent history.
    Home,
    /// Only `@collection` / `#tag` filters: snippets in manual order.
    Filtered,
    /// Text search, ranked.
    Search,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub kind: Kind,
    pub id: Id,
    pub collection_id: Option<Id>,
    pub collection: Option<String>,
    /// Snippet title, or the first line of the content.
    pub title: String,
    /// Char indices in `title` that matched the query (for highlighting).
    pub title_matches: Vec<u32>,
    /// One-line preview of the rest of the content (whitespace collapsed).
    pub preview: String,
    /// Char indices in `preview` that matched the query.
    pub preview_matches: Vec<u32>,
    pub tags: Vec<String>,
    pub starred: bool,
    pub timestamp: i64,
    pub has_formatting: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Results {
    pub mode: Mode,
    pub hits: Vec<Hit>,
    /// Number of matches before `limit` was applied.
    pub total: usize,
    /// Home mode only: favorites not shown because of [`HOME_FAVORITES`].
    pub more_favorites: usize,
}

/// A parsed query.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Query {
    /// Search text with all whitespace removed.
    pub text: String,
    /// `@` filters, lower-cased, spaces removed.
    pub collections: Vec<String>,
    /// `#` filters, lower-cased.
    pub tags: Vec<String>,
}

impl Query {
    pub fn parse(input: &str) -> Query {
        let mut q = Query::default();
        for token in input.split_whitespace() {
            if let Some(name) = token.strip_prefix('@') {
                if !name.is_empty() {
                    q.collections.push(name.to_lowercase());
                }
            } else if let Some(tag) = token.strip_prefix('#') {
                if !tag.is_empty() {
                    q.tags.push(tag.to_lowercase());
                }
            } else {
                q.text.push_str(token);
            }
        }
        q
    }

    pub fn has_filters(&self) -> bool {
        !self.collections.is_empty() || !self.tags.is_empty()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty() && !self.has_filters()
    }
}

pub struct SearchIndex {
    entries: Vec<Entry>,
    pos: HashMap<(Kind, Id), usize>,
    collections: HashMap<Id, CollectionMeta>,
    matcher: Matcher,
    buf: Vec<char>,
    /// Bumped on every change that can add/remove matches or move entries.
    generation: u64,
    /// Matches of the previous text query, for narrowing while typing.
    last: Option<LastSearch>,
}

struct LastSearch {
    generation: u64,
    query: Query,
    matches: Vec<usize>,
}

impl Default for SearchIndex {
    fn default() -> Self {
        SearchIndex {
            entries: Vec::new(),
            pos: HashMap::new(),
            collections: HashMap::new(),
            matcher: Matcher::new(Config::DEFAULT),
            buf: Vec::new(),
            generation: 0,
            last: None,
        }
    }
}

impl SearchIndex {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.generation += 1;
        self.entries.clear();
        self.pos.clear();
        self.collections.clear();
    }

    pub fn set_collection(&mut self, c: &Collection) {
        self.generation += 1;
        self.collections.insert(
            c.id,
            CollectionMeta {
                name: c.name.clone(),
                key: c.name.to_lowercase().split_whitespace().collect(),
                order: c.sort_order,
            },
        );
    }

    pub fn remove_collection(&mut self, id: Id) {
        self.collections.remove(&id);
        let doomed: Vec<Id> = self
            .entries
            .iter()
            .filter(|e| e.kind == Kind::Snippet && e.collection_id == id)
            .map(|e| e.id)
            .collect();
        for sid in doomed {
            self.remove(Kind::Snippet, sid);
        }
    }

    pub fn upsert_history(&mut self, h: &HistoryItem) {
        self.upsert(Entry {
            kind: Kind::History,
            id: h.id,
            collection_id: 0,
            title: Utf32String::default(),
            text: prefix_chars(&h.content, INDEX_CHARS).into(),
            tags: Vec::new(),
            starred_at: None,
            recency: h.last_copied_at,
            sort_order: 0,
            has_formatting: h.has_formatting(),
        });
    }

    pub fn upsert_snippet(&mut self, s: &Snippet) {
        self.upsert(Entry {
            kind: Kind::Snippet,
            id: s.id,
            collection_id: s.collection_id,
            title: s.title.trim().into(),
            text: prefix_chars(&s.content, INDEX_CHARS).into(),
            tags: s.tags.clone(),
            starred_at: s.starred_at,
            recency: s.last_used_at.unwrap_or(s.created_at),
            sort_order: s.sort_order,
            has_formatting: false,
        });
    }

    pub fn remove(&mut self, kind: Kind, id: Id) {
        if let Some(i) = self.pos.remove(&(kind, id)) {
            self.generation += 1;
            self.entries.swap_remove(i);
            if let Some(moved) = self.entries.get(i) {
                self.pos.insert((moved.kind, moved.id), i);
            }
        }
    }

    /// Update the recency of an item after it was pasted.
    pub fn touch(&mut self, kind: Kind, id: Id, at: i64) {
        if let Some(&i) = self.pos.get(&(kind, id)) {
            self.entries[i].recency = at;
        }
    }

    /// Ids of all history items in the index.
    pub fn history_ids(&self) -> HashSet<Id> {
        self.entries
            .iter()
            .filter(|e| e.kind == Kind::History)
            .map(|e| e.id)
            .collect()
    }

    fn upsert(&mut self, e: Entry) {
        self.generation += 1;
        match self.pos.get(&(e.kind, e.id)) {
            Some(&i) => self.entries[i] = e,
            None => {
                self.pos.insert((e.kind, e.id), self.entries.len());
                self.entries.push(e);
            }
        }
    }

    /// Run a query. `limit` caps the number of hits returned.
    pub fn search(&mut self, input: &str, limit: usize) -> Results {
        let q = Query::parse(input);
        if q.is_empty() {
            return self.home(limit);
        }
        let allowed = self.allowed_collections(&q);
        // While typing, a longer query can only match a subset of the previous
        // matches, so only those need re-checking.
        let narrowed = self.last.take().filter(|last| {
            last.generation == self.generation
                && !q.text.is_empty()
                && q.text.starts_with(&last.query.text)
                && last.query.collections == q.collections
                && last.query.tags == q.tags
        });
        let candidates: Vec<usize> = match narrowed {
            Some(last) => last.matches,
            None => (0..self.entries.len())
                .filter(|&i| self.passes_filters(&self.entries[i], &q, allowed.as_ref()))
                .collect(),
        };

        if q.text.is_empty() {
            // Filter only: snippets in collection order, then manual order.
            let mut list = candidates;
            list.sort_by_key(|&i| {
                let e = &self.entries[i];
                let order = self
                    .collections
                    .get(&e.collection_id)
                    .map_or(0, |c| c.order);
                (order, e.collection_id, e.sort_order, e.id)
            });
            let total = list.len();
            let hits = list
                .into_iter()
                .take(limit)
                .map(|i| self.hit(i, None))
                .collect();
            return Results {
                mode: Mode::Filtered,
                hits,
                total,
                more_favorites: 0,
            };
        }

        let atom = Atom::new(
            &q.text,
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
            false,
        );
        let entries = &self.entries;
        let score = |m: &mut Matcher, i: usize| {
            let e = &entries[i];
            let text = atom.score(e.text.slice(..), m);
            let title = if e.title.is_empty() {
                None
            } else {
                atom.score(e.title.slice(..), m)
            };
            text.max(title).map(|s| (i, s))
        };
        let mut scored: Vec<(usize, u16)> = if candidates.len() > PARALLEL_THRESHOLD {
            candidates
                .par_iter()
                .map_init(|| Matcher::new(Config::DEFAULT), |m, &i| score(m, i))
                .flatten()
                .collect()
        } else {
            let m = &mut self.matcher;
            candidates.iter().filter_map(|&i| score(m, i)).collect()
        };
        self.last = Some(LastSearch {
            generation: self.generation,
            query: q.clone(),
            matches: scored.iter().map(|&(i, _)| i).collect(),
        });
        let best = u32::from(scored.iter().map(|&(_, s)| s).max().unwrap_or(1).max(1));
        // ~3% wide quality tiers relative to the best match; inside a tier the
        // most recent item wins. (nucleo scores a sloppy match ~10% below a
        // perfect one, so the tiers must be narrow.)
        let bucket = |s: u16| (u32::from(s) * 32) / best;
        scored.sort_by(|&(a, sa), &(b, sb)| {
            bucket(sb)
                .cmp(&bucket(sa))
                .then(self.entries[b].recency.cmp(&self.entries[a].recency))
                .then(sb.cmp(&sa))
                .then(self.entries[a].id.cmp(&self.entries[b].id))
        });
        let total = scored.len();
        let hits = scored
            .into_iter()
            .take(limit)
            .map(|(i, _)| self.hit(i, Some(&atom)))
            .collect();
        Results {
            mode: Mode::Search,
            hits,
            total,
            more_favorites: 0,
        }
    }

    /// Favorites first (most recently used first), then recent history.
    fn home(&mut self, limit: usize) -> Results {
        let mut favorites: Vec<usize> = (0..self.entries.len())
            .filter(|&i| self.entries[i].starred_at.is_some())
            .collect();
        favorites.sort_by_key(|&i| {
            let e = &self.entries[i];
            (
                std::cmp::Reverse(e.recency),
                std::cmp::Reverse(e.starred_at),
                e.id,
            )
        });
        let more_favorites = favorites.len().saturating_sub(HOME_FAVORITES);
        favorites.truncate(HOME_FAVORITES);

        let mut history: Vec<usize> = (0..self.entries.len())
            .filter(|&i| self.entries[i].kind == Kind::History)
            .collect();
        history.sort_by_key(|&i| {
            let e = &self.entries[i];
            (std::cmp::Reverse(e.recency), std::cmp::Reverse(e.id))
        });

        let total = favorites.len() + history.len();
        let hits = favorites
            .into_iter()
            .chain(history)
            .take(limit)
            .map(|i| self.hit(i, None))
            .collect();
        Results {
            mode: Mode::Home,
            hits,
            total,
            more_favorites,
        }
    }

    /// Every favorite, most recently used first ("Show all favorites…").
    pub fn favorites(&mut self) -> Results {
        let mut list: Vec<usize> = (0..self.entries.len())
            .filter(|&i| self.entries[i].starred_at.is_some())
            .collect();
        list.sort_by_key(|&i| {
            let e = &self.entries[i];
            (std::cmp::Reverse(e.recency), e.id)
        });
        Results {
            mode: Mode::Filtered,
            total: list.len(),
            hits: list.into_iter().map(|i| self.hit(i, None)).collect(),
            more_favorites: 0,
        }
    }

    fn allowed_collections(&self, q: &Query) -> Option<HashSet<Id>> {
        if q.collections.is_empty() {
            return None;
        }
        Some(
            self.collections
                .iter()
                .filter(|(_, meta)| {
                    q.collections
                        .iter()
                        .any(|p| meta.key.starts_with(p.as_str()))
                })
                .map(|(&id, _)| id)
                .collect(),
        )
    }

    fn passes_filters(&self, e: &Entry, q: &Query, allowed: Option<&HashSet<Id>>) -> bool {
        if q.has_filters() && e.kind == Kind::History {
            return false;
        }
        if let Some(allowed) = allowed {
            if !allowed.contains(&e.collection_id) {
                return false;
            }
        }
        q.tags
            .iter()
            .all(|want| e.tags.iter().any(|t| t.starts_with(want.as_str())))
    }

    fn hit(&mut self, i: usize, atom: Option<&Atom>) -> Hit {
        let e = &self.entries[i];
        let (title, preview) = display_parts(&e.title.to_string(), &e.text.to_string());
        let mut title_matches = Vec::new();
        let mut preview_matches = Vec::new();
        if let Some(atom) = atom {
            atom.indices(
                Utf32Str::new(&title, &mut self.buf),
                &mut self.matcher,
                &mut title_matches,
            );
            if title_matches.is_empty() {
                atom.indices(
                    Utf32Str::new(&preview, &mut self.buf),
                    &mut self.matcher,
                    &mut preview_matches,
                );
            }
            title_matches.sort_unstable();
            title_matches.dedup();
            preview_matches.sort_unstable();
            preview_matches.dedup();
        }
        let collection = (e.kind == Kind::Snippet)
            .then(|| {
                self.collections
                    .get(&e.collection_id)
                    .map(|c| c.name.clone())
            })
            .flatten();
        Hit {
            kind: e.kind,
            id: e.id,
            collection_id: (e.kind == Kind::Snippet).then_some(e.collection_id),
            collection,
            title,
            title_matches,
            preview,
            preview_matches,
            tags: e.tags.clone(),
            starred: e.starred_at.is_some(),
            timestamp: e.recency,
            has_formatting: e.has_formatting,
        }
    }
}

/// Title and one-line preview for a row.
/// - With a title: preview = the content.
/// - Without: title = first line, preview = the remaining lines.
fn display_parts(title: &str, text: &str) -> (String, String) {
    if !title.is_empty() {
        return (title.to_owned(), one_line(text, PREVIEW_CHARS));
    }
    let first = first_line(text);
    let rest = text.trim_start().strip_prefix(first).unwrap_or_default();
    (
        one_line(first, PREVIEW_CHARS),
        one_line(rest, PREVIEW_CHARS),
    )
}

/// Collapse all whitespace (including newlines) to single spaces and cap the length.
pub fn one_line(text: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (n, word) in text.split_whitespace().enumerate() {
        if n > 0 {
            out.push(' ');
        }
        out.push_str(word);
        if out.chars().count() >= max_chars {
            break;
        }
    }
    prefix_chars(&out, max_chars)
}

fn prefix_chars(text: &str, max_chars: usize) -> String {
    match text.char_indices().nth(max_chars) {
        Some((cut, _)) => text[..cut].to_owned(),
        None => text.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn history(id: Id, text: &str, at: i64) -> HistoryItem {
        HistoryItem {
            id,
            content: text.into(),
            last_copied_at: at,
            ..Default::default()
        }
    }

    fn snippet(id: Id, cid: Id, title: &str, text: &str, tags: &[&str]) -> Snippet {
        Snippet {
            id,
            collection_id: cid,
            title: title.into(),
            content: text.into(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            sort_order: id,
            created_at: 1,
            ..Default::default()
        }
    }

    fn coll(id: Id, name: &str, order: i64) -> Collection {
        Collection {
            id,
            name: name.into(),
            sort_order: order,
            created_at: 0,
            updated_at: 0,
        }
    }

    fn titles(r: &Results) -> Vec<String> {
        r.hits.iter().map(|h| h.title.clone()).collect()
    }

    #[test]
    fn query_parsing() {
        let q = Query::parse("  @Work  18 mm  #Bank ");
        assert_eq!(q.text, "18mm");
        assert_eq!(q.collections, vec!["work"]);
        assert_eq!(q.tags, vec!["bank"]);
        assert!(Query::parse("  @  # ").is_empty());
    }

    #[test]
    fn in_order_matching_like_1x() {
        let mut idx = SearchIndex::default();
        idx.upsert_history(&history(1, "18 mm 8 x 4 LL Pro", 10));
        idx.upsert_history(&history(2, "something else", 20));
        for q in ["ro", "LP", "1884", "18 mm", "18mm"] {
            let r = idx.search(q, 10);
            assert_eq!(r.hits.len(), 1, "query {q:?}");
            assert_eq!(r.hits[0].id, 1);
        }
        assert!(idx.search("pro18", 10).hits.is_empty(), "order matters");
    }

    #[test]
    fn unicode_case_and_accents() {
        let mut idx = SearchIndex::default();
        idx.upsert_history(&history(1, "ÉCOLE Paris", 10));
        assert_eq!(idx.search("école", 10).hits.len(), 1);
        assert_eq!(idx.search("ecole", 10).hits.len(), 1);
        assert_eq!(idx.search("ÉCOLE", 10).hits.len(), 1);
    }

    #[test]
    fn best_match_beats_recent_sloppy_match() {
        let mut idx = SearchIndex::default();
        idx.upsert_history(&history(1, "invoice 2024 final", 1_000)); // old, perfect
        idx.upsert_history(&history(2, "i n v o i c e scattered", 9_000)); // new, sloppy
        let r = idx.search("invoice", 10);
        assert_eq!(r.hits[0].id, 1);
    }

    #[test]
    fn near_ties_are_ordered_by_recency() {
        let mut idx = SearchIndex::default();
        idx.upsert_history(&history(1, "report draft", 1_000));
        idx.upsert_history(&history(2, "report final", 9_000));
        let r = idx.search("report", 10);
        assert_eq!(r.hits[0].id, 2, "same quality, newer first");
    }

    #[test]
    fn titles_are_searched_and_highlighted() {
        let mut idx = SearchIndex::default();
        idx.set_collection(&coll(1, "Work", 0));
        idx.upsert_snippet(&snippet(5, 1, "Bank IFSC", "HDFC0001234", &["bank"]));
        let r = idx.search("ifsc", 10);
        assert_eq!(r.hits.len(), 1);
        let h = &r.hits[0];
        assert_eq!(h.collection.as_deref(), Some("Work"));
        assert_eq!(h.title_matches, vec![5, 6, 7, 8]);
        assert_eq!(h.preview, "HDFC0001234");
    }

    #[test]
    fn filters() {
        let mut idx = SearchIndex::default();
        idx.set_collection(&coll(1, "Work Stuff", 1));
        idx.set_collection(&coll(2, "Personal", 0));
        idx.upsert_snippet(&snippet(10, 1, "GST number", "27AAAPL1234C1Z5", &["tax"]));
        idx.upsert_snippet(&snippet(11, 1, "Bank IFSC", "HDFC0001234", &["bank"]));
        idx.upsert_snippet(&snippet(12, 2, "Home address", "12 Park St", &["home"]));
        idx.upsert_history(&history(1, "GST in history", 99));

        // Filter alone: manual order, history excluded.
        let r = idx.search("@work", 10);
        assert_eq!(r.mode, Mode::Filtered);
        assert_eq!(titles(&r), vec!["GST number", "Bank IFSC"]);
        // Spaces in collection names are ignored by the filter.
        assert_eq!(idx.search("@workst", 10).hits.len(), 2);
        // Tag filter + text.
        assert_eq!(titles(&idx.search("#bank hd", 10)), vec!["Bank IFSC"]);
        // Text without filters searches history too.
        assert_eq!(idx.search("gst", 10).hits.len(), 2);
        // Unknown collection: nothing.
        assert!(idx.search("@nope", 10).hits.is_empty());
    }

    #[test]
    fn home_lists_favorites_then_history() {
        let mut idx = SearchIndex::default();
        idx.set_collection(&coll(1, "Favorites", 0));
        idx.upsert_history(&history(1, "older", 10));
        idx.upsert_history(&history(2, "newer", 20));
        let mut fav = snippet(7, 1, "Address", "12 Park St", &[]);
        fav.starred_at = Some(5);
        idx.upsert_snippet(&fav);
        idx.upsert_snippet(&snippet(8, 1, "Not starred", "x", &[]));
        let r = idx.search("   ", 10);
        assert_eq!(r.mode, Mode::Home);
        assert_eq!(titles(&r), vec!["Address", "newer", "older"]);

        for i in 0..12 {
            let mut s = snippet(100 + i, 1, &format!("Fav {i}"), "x", &[]);
            s.starred_at = Some(1);
            idx.upsert_snippet(&s);
        }
        let r = idx.search("", 50);
        assert_eq!(r.more_favorites, 3);
        assert_eq!(idx.favorites().hits.len(), 13);
    }

    #[test]
    fn upsert_remove_keep_positions_consistent() {
        let mut idx = SearchIndex::default();
        for i in 1..=5 {
            idx.upsert_history(&history(i, &format!("item {i}"), i));
        }
        idx.remove(Kind::History, 2);
        idx.remove(Kind::History, 5);
        idx.upsert_history(&history(3, "item three", 50));
        let r = idx.search("", 10);
        assert_eq!(titles(&r), vec!["item three", "item 4", "item 1"]);
        idx.set_collection(&coll(1, "C", 0));
        idx.upsert_snippet(&snippet(9, 1, "s", "x", &[]));
        idx.remove_collection(1);
        assert_eq!(idx.len(), 3);
    }

    #[test]
    fn display_parts_split_first_line() {
        let (t, p) = display_parts("", "\n  Dear team,\n  please   find\tattached ");
        assert_eq!(t, "Dear team,");
        assert_eq!(p, "please find attached");
        let (t, p) = display_parts("Greeting", "Dear team,\nthanks");
        assert_eq!((t.as_str(), p.as_str()), ("Greeting", "Dear team, thanks"));
    }
}
