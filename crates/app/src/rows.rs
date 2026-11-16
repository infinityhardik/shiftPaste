//! Turn core search results into popup rows (sections, badges, highlights).

use shiftpaste_core::search::{Hit, Kind, Mode, Results};
use shiftpaste_core::time::time_ago;
use slint::{SharedString, StyledText};

use crate::ui::{PopupRow, RowKind};

/// What a row in the popup list stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowTarget {
    /// Index into `Results::hits`.
    Hit(usize),
    Section,
    /// "Show all N favorites…"
    MoreFavorites,
}

impl RowTarget {
    pub fn is_selectable(self) -> bool {
        !matches!(self, RowTarget::Section)
    }
}

pub struct BuiltRows {
    pub rows: Vec<PopupRow>,
    pub targets: Vec<RowTarget>,
}

/// Build rows for the popup. `picks` gives each hit's 1-based position in the
/// multi-selection (0 = not picked); `accent` colours matched letters.
pub fn build(
    results: &Results,
    now: i64,
    accent: &str,
    pick_of: impl Fn(&Hit) -> i32,
) -> BuiltRows {
    // A collection chip on every row says nothing when all rows share it.
    let one_collection = results.mode != Mode::Home
        && results.hits.len() > 1
        && results
            .hits
            .iter()
            .all(|h| h.kind == Kind::Snippet && h.collection_id == results.hits[0].collection_id);
    let mut rows = Vec::with_capacity(results.hits.len() + 3);
    let mut targets = Vec::with_capacity(results.hits.len() + 3);
    let section = |rows: &mut Vec<PopupRow>, targets: &mut Vec<RowTarget>, title: &str| {
        rows.push(PopupRow {
            kind: RowKind::Section,
            title: title.into(),
            ..Default::default()
        });
        targets.push(RowTarget::Section);
    };

    let mut seen_history = false;
    let mut favorites_done = false;
    for (i, hit) in results.hits.iter().enumerate() {
        if results.mode == Mode::Home {
            if i == 0 && hit.starred && hit.kind == Kind::Snippet {
                section(&mut rows, &mut targets, "Favorites");
            }
            if hit.kind == Kind::History && !seen_history {
                if i > 0 && results.more_favorites > 0 && !favorites_done {
                    rows.push(PopupRow {
                        kind: RowKind::More,
                        title: format!("Show all {} favorites", results.more_favorites + i).into(),
                        ..Default::default()
                    });
                    targets.push(RowTarget::MoreFavorites);
                }
                favorites_done = true;
                if i > 0 {
                    section(&mut rows, &mut targets, "Recent");
                }
                seen_history = true;
            }
        }
        let mut row = item_row(hit, now, accent, pick_of(hit));
        if one_collection {
            row.collection = SharedString::new();
        }
        rows.push(row);
        targets.push(RowTarget::Hit(i));
    }
    BuiltRows { rows, targets }
}

fn item_row(hit: &Hit, now: i64, accent: &str, pick: i32) -> PopupRow {
    let highlight = !hit.title_matches.is_empty() || !hit.preview_matches.is_empty();
    let (title_styled, preview_styled) = if highlight {
        (
            // Titles are bold everywhere; highlighting must not change that.
            styled(
                &format!(
                    "**{}**",
                    highlight_markup(&hit.title, &hit.title_matches, accent)
                ),
                &hit.title,
            ),
            styled(
                &highlight_markup(&hit.preview, &hit.preview_matches, accent),
                &hit.preview,
            ),
        )
    } else {
        Default::default()
    };
    PopupRow {
        kind: RowKind::Item,
        title: SharedString::from(non_empty(&hit.title, "(blank)")),
        title_styled,
        preview: hit.preview.as_str().into(),
        preview_styled,
        highlight,
        is_snippet: hit.kind == Kind::Snippet,
        collection: hit.collection.clone().unwrap_or_default().into(),
        meta: if hit.kind == Kind::History {
            time_ago(hit.timestamp, now).into()
        } else {
            SharedString::new()
        },
        starred: hit.starred,
        rich: hit.has_formatting,
        pick,
    }
}

fn non_empty<'a>(s: &'a str, fallback: &'a str) -> &'a str {
    if s.trim().is_empty() {
        fallback
    } else {
        s
    }
}

/// Styled text with matched characters coloured. Everything else is escaped so
/// clipboard content can never be interpreted as markup.
fn styled(markup: &str, plain: &str) -> StyledText {
    StyledText::from_markdown(markup).unwrap_or_else(|_| StyledText::from_plain_text(plain))
}

pub fn highlight_markup(text: &str, matches: &[u32], accent: &str) -> String {
    let mut out = String::with_capacity(text.len() + matches.len() * 24);
    let mut in_run = false;
    let mut next = matches.iter().copied().peekable();
    for (i, c) in text.chars().enumerate() {
        let hit = next.peek() == Some(&(i as u32));
        if hit {
            next.next();
        }
        if hit && !in_run {
            out.push_str("<font color=\"");
            out.push_str(accent);
            out.push_str("\">");
            in_run = true;
        } else if !hit && in_run {
            out.push_str("</font>");
            in_run = false;
        }
        push_escaped(&mut out, c);
    }
    if in_run {
        out.push_str("</font>");
    }
    out
}

fn push_escaped(out: &mut String, c: char) {
    // CommonMark lets any ASCII punctuation be backslash-escaped.
    if c.is_ascii_punctuation() {
        out.push('\\');
    }
    out.push(c);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markup_escapes_and_groups_runs() {
        let m = highlight_markup("a*b<c>", &[0, 1, 4], "#2563eb");
        assert_eq!(
            m,
            "<font color=\"#2563eb\">a\\*</font>b\\<<font color=\"#2563eb\">c</font>\\>"
        );
        assert_eq!(highlight_markup("plain", &[], "#000"), "plain");
        // Must always parse, whatever the content.
        for text in [
            "# not a heading",
            "1. not a list",
            "**x**",
            "`code`",
            "<b>tag</b>",
            "a\\b",
            "&amp;",
        ] {
            let md = highlight_markup(text, &[0], "#000");
            assert!(StyledText::from_markdown(&md).is_ok(), "{text}");
        }
    }
}
