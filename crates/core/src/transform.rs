//! Paste transforms (Alt+Enter menu) and joining several items for multi-paste.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transform {
    Upper,
    Lower,
    Title,
    /// Remove leading/trailing whitespace of the whole text.
    Trim,
    /// Collapse all whitespace, including line breaks, into single spaces.
    SingleLine,
}

impl Transform {
    pub const ALL: [Transform; 5] = [
        Transform::Upper,
        Transform::Lower,
        Transform::Title,
        Transform::Trim,
        Transform::SingleLine,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Transform::Upper => "UPPERCASE",
            Transform::Lower => "lowercase",
            Transform::Title => "Title Case",
            Transform::Trim => "Trimmed",
            Transform::SingleLine => "Single line",
        }
    }

    pub fn apply(self, text: &str) -> String {
        match self {
            Transform::Upper => text.to_uppercase(),
            Transform::Lower => text.to_lowercase(),
            Transform::Title => title_case(text),
            Transform::Trim => text.trim().to_owned(),
            Transform::SingleLine => text.split_whitespace().collect::<Vec<_>>().join(" "),
        }
    }
}

/// Upper-case the first letter of every word and lower-case the rest,
/// keeping the original whitespace and punctuation.
fn title_case(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at_word_start = true;
    for c in text.chars() {
        if c.is_alphanumeric() {
            if at_word_start {
                out.extend(c.to_uppercase());
            } else {
                out.extend(c.to_lowercase());
            }
            at_word_start = false;
        } else {
            out.push(c);
            // Apostrophes inside words ("don't") don't start a new word.
            at_word_start = !matches!(c, '\'' | '’');
        }
    }
    out
}

/// How several selected items are joined into one paste.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Joiner {
    #[default]
    NewLine,
    Comma,
    Space,
    /// Fills neighbouring cells when pasted into a spreadsheet.
    Tab,
}

impl Joiner {
    pub const ALL: [Joiner; 4] = [Joiner::NewLine, Joiner::Comma, Joiner::Space, Joiner::Tab];

    pub fn label(self) -> &'static str {
        match self {
            Joiner::NewLine => "One per line",
            Joiner::Comma => "Comma-separated",
            Joiner::Space => "Space-separated",
            Joiner::Tab => "Tab-separated (spreadsheet cells)",
        }
    }

    fn separator(self) -> &'static str {
        match self {
            Joiner::NewLine => "\n",
            Joiner::Comma => ", ",
            Joiner::Space => " ",
            Joiner::Tab => "\t",
        }
    }

    pub fn join<S: AsRef<str>>(self, items: &[S]) -> String {
        items
            .iter()
            .map(|s| s.as_ref().trim_end_matches(['\r', '\n']))
            .collect::<Vec<_>>()
            .join(self.separator())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transforms() {
        let t = "  hello WORLD\n  it's élan ";
        assert_eq!(Transform::Upper.apply(t), "  HELLO WORLD\n  IT'S ÉLAN ");
        assert_eq!(Transform::Lower.apply("ÉCOLE"), "école");
        assert_eq!(Transform::Title.apply(t), "  Hello World\n  It's Élan ");
        assert_eq!(Transform::Trim.apply(t), "hello WORLD\n  it's élan");
        assert_eq!(Transform::SingleLine.apply(t), "hello WORLD it's élan");
        assert_eq!(Transform::Title.apply("18 mm pro-model"), "18 Mm Pro-Model");
    }

    #[test]
    fn joining() {
        let items = ["A\n", "B", "C"];
        assert_eq!(Joiner::NewLine.join(&items), "A\nB\nC");
        assert_eq!(Joiner::Comma.join(&items), "A, B, C");
        assert_eq!(Joiner::Tab.join(&items), "A\tB\tC");
        assert_eq!(Joiner::Space.join::<&str>(&[]), "");
    }
}
