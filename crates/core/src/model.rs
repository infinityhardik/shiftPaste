//! Plain data types shared by the store, search, editor and UI.

pub type Id = i64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collection {
    pub id: Id,
    pub name: String,
    pub sort_order: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// A reusable snippet inside a collection.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Snippet {
    pub id: Id,
    pub collection_id: Id,
    /// Optional short name shown in lists; searchable.
    pub title: String,
    /// What gets pasted.
    pub content: String,
    /// Private notes; not pasted, not searched.
    pub notes: String,
    /// Normalised (lower-case, trimmed, unique) tags.
    pub tags: Vec<String>,
    pub sort_order: i64,
    /// When the snippet was starred, or `None` if it isn't a favorite.
    pub starred_at: Option<i64>,
    pub use_count: i64,
    pub last_used_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Snippet {
    pub fn is_starred(&self) -> bool {
        self.starred_at.is_some()
    }

    /// The title, or the first non-empty line of the content when there is no title.
    pub fn display_title(&self) -> &str {
        let title = self.title.trim();
        if !title.is_empty() {
            return title;
        }
        first_line(&self.content)
    }
}

/// One entry of clipboard history (unique by its plain text).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HistoryItem {
    pub id: Id,
    pub content: String,
    pub content_hash: String,
    pub html: Option<String>,
    pub rtf: Option<String>,
    pub source_app: Option<String>,
    pub copy_count: i64,
    pub first_copied_at: i64,
    pub last_copied_at: i64,
}

impl HistoryItem {
    pub fn has_formatting(&self) -> bool {
        self.html.is_some() || self.rtf.is_some()
    }
}

/// A clipboard capture as delivered by the platform layer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NewClip {
    pub text: String,
    pub html: Option<String>,
    pub rtf: Option<String>,
    pub source_app: Option<String>,
}

/// First non-empty line, trimmed.
pub fn first_line(text: &str) -> &str {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
}

/// Normalise tags: trim, drop a leading `#`, lower-case, collapse inner whitespace
/// to `-`, drop empties and duplicates (keeping the first occurrence's position).
pub fn normalize_tags<'a>(tags: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in tags {
        for part in raw.split([',', ';']) {
            let tag = part
                .trim()
                .trim_start_matches('#')
                .split_whitespace()
                .collect::<Vec<_>>()
                .join("-")
                .to_lowercase();
            if !tag.is_empty() && !out.contains(&tag) {
                out.push(tag);
            }
        }
    }
    out
}

/// Parse a user-typed tag string like "work, #Bank; tax".
pub fn parse_tags(text: &str) -> Vec<String> {
    normalize_tags([text])
}

/// Clean a collection name: trim, collapse whitespace, cap the length.
pub fn clean_name(name: &str) -> String {
    let joined = name.split_whitespace().collect::<Vec<_>>().join(" ");
    joined.chars().take(80).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_are_normalised() {
        assert_eq!(
            parse_tags(" Work, #bank;tax ,, work , Home  Office"),
            vec!["work", "bank", "tax", "home-office"]
        );
        assert!(parse_tags("  ,  ;").is_empty());
    }

    #[test]
    fn display_title_falls_back_to_first_line() {
        let mut s = Snippet {
            content: "\n  Dear team,\nthanks".into(),
            ..Default::default()
        };
        assert_eq!(s.display_title(), "Dear team,");
        s.title = " Greeting ".into();
        assert_eq!(s.display_title(), "Greeting");
    }

    #[test]
    fn names_are_cleaned() {
        assert_eq!(clean_name("  My   Work \t List "), "My Work List");
        assert_eq!(clean_name(&"x".repeat(200)).len(), 80);
    }
}
