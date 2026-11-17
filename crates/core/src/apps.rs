//! "Which app is this?" matching for the capture and hotkey app lists, and
//! the paste keystroke each app needs.

use serde::{Deserialize, Serialize};

use crate::presets::{self, Group, Platform, PRESET_PREFIX};

/// The app in front, as reported by the platform layer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppIdentity {
    /// Human-readable name for the UI ("Microsoft Excel").
    pub display_name: String,
    /// Normalised identifiers: exe name, bundle id, WM_CLASS, process name…
    ids: Vec<String>,
}

impl AppIdentity {
    pub fn new(display_name: impl Into<String>, ids: impl IntoIterator<Item = String>) -> Self {
        let mut norm: Vec<String> = Vec::new();
        for id in ids {
            let n = normalize(&id);
            if !n.is_empty() && !norm.contains(&n) {
                norm.push(n);
            }
        }
        AppIdentity {
            display_name: display_name.into(),
            ids: norm,
        }
    }

    pub fn ids(&self) -> &[String] {
        &self.ids
    }

    /// The best single identifier to store when the user picks this app.
    pub fn primary_id(&self) -> Option<&str> {
        self.ids.first().map(String::as_str)
    }

    fn has(&self, ident: &str) -> bool {
        let n = normalize(ident);
        self.ids.contains(&n)
    }
}

/// Lower-case, trimmed, without a trailing ".exe", so "Excel", "EXCEL.EXE" and
/// "excel.exe" are the same app.
fn normalize(id: &str) -> String {
    let lower = id.trim().to_lowercase();
    lower.strip_suffix(".exe").unwrap_or(&lower).to_owned()
}

/// Does a settings entry (`preset:excel` or a plain identifier) match the app?
pub fn entry_matches(entry: &str, app: &AppIdentity) -> bool {
    match entry.strip_prefix(PRESET_PREFIX) {
        Some(id) => presets::preset(id).is_some_and(|p| p.identifiers.iter().any(|i| app.has(i))),
        None => app.has(entry),
    }
}

/// What to show for an entry in Settings.
pub fn entry_label(entry: &str) -> String {
    match entry.strip_prefix(PRESET_PREFIX).and_then(presets::preset) {
        Some(p) => p.name.to_owned(),
        None => entry.to_owned(),
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FilterMode {
    /// Every app except those in the `except` list.
    #[default]
    Except,
    /// Only apps in the `only` list.
    Only,
}

/// An app list with a mode. Each mode has its own list, so switching mode
/// never flips a list's meaning (password managers can't accidentally become
/// the *only* apps recorded).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppFilter {
    pub mode: FilterMode,
    pub except: Vec<String>,
    pub only: Vec<String>,
}

impl AppFilter {
    pub fn excluding(entries: Vec<String>) -> Self {
        AppFilter {
            mode: FilterMode::Except,
            except: entries,
            only: Vec::new(),
        }
    }

    pub fn active_list(&self) -> &[String] {
        match self.mode {
            FilterMode::Except => &self.except,
            FilterMode::Only => &self.only,
        }
    }

    pub fn active_list_mut(&mut self) -> &mut Vec<String> {
        match self.mode {
            FilterMode::Except => &mut self.except,
            FilterMode::Only => &mut self.only,
        }
    }

    /// Is the feature allowed for this app? `None` = the app in front is unknown
    /// (e.g. Wayland): "all except" lets it through, "only these" fails safe.
    pub fn allows(&self, app: Option<&AppIdentity>) -> bool {
        match (self.mode, app) {
            (FilterMode::Except, None) => true,
            (FilterMode::Only, None) => false,
            (FilterMode::Except, Some(app)) => !self.except.iter().any(|e| entry_matches(e, app)),
            (FilterMode::Only, Some(app)) => self.only.iter().any(|e| entry_matches(e, app)),
        }
    }
}

/// The keystroke that pastes in a given app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PasteKeys {
    CtrlV,
    CtrlShiftV,
    ShiftInsert,
    CmdV,
}

impl PasteKeys {
    pub fn label(self) -> &'static str {
        match self {
            PasteKeys::CtrlV => "Ctrl+V",
            PasteKeys::CtrlShiftV => "Ctrl+Shift+V",
            PasteKeys::ShiftInsert => "Shift+Insert",
            PasteKeys::CmdV => "Cmd+V",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasteOverride {
    /// Settings entry: `preset:<id>` or an identifier.
    pub app: String,
    pub keys: PasteKeys,
}

/// Paste keystroke for the app: user overrides first, then built-in rules
/// (Linux terminals need Ctrl+Shift+V), then the platform default.
pub fn paste_keys_for(
    app: Option<&AppIdentity>,
    overrides: &[PasteOverride],
    platform: Platform,
) -> PasteKeys {
    if let Some(app) = app {
        if let Some(o) = overrides.iter().find(|o| entry_matches(&o.app, app)) {
            return o.keys;
        }
        if platform == Platform::Linux
            && presets::presets_in(Group::Terminals, Platform::Linux)
                .any(|p| p.identifiers.iter().any(|i| app.has(i)))
        {
            return PasteKeys::CtrlShiftV;
        }
    }
    match platform {
        Platform::MacOs => PasteKeys::CmdV,
        _ => PasteKeys::CtrlV,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(ids: &[&str]) -> AppIdentity {
        AppIdentity::new("App", ids.iter().map(|s| s.to_string()))
    }

    #[test]
    fn identifiers_match_loosely() {
        let excel = app(&["EXCEL.EXE"]);
        assert!(entry_matches("preset:excel", &excel));
        assert!(entry_matches("Excel", &excel));
        assert!(entry_matches("excel.exe", &excel));
        assert!(!entry_matches("preset:word", &excel));
        assert!(!entry_matches("exc", &excel), "no substring matches");
        assert!(!entry_matches("preset:does-not-exist", &excel));
        assert_eq!(entry_label("preset:excel"), "Microsoft Excel");
        assert_eq!(entry_label("MyApp.exe"), "MyApp.exe");
    }

    #[test]
    fn filter_modes_and_unknown_apps() {
        let keepass = app(&["KeePassXC.exe"]);
        let notepad = app(&["notepad.exe"]);
        let capture = AppFilter::excluding(presets::password_manager_entries(Platform::Windows));
        assert!(!capture.allows(Some(&keepass)));
        assert!(capture.allows(Some(&notepad)));
        assert!(capture.allows(None), "unknown app: lists not applied");

        let mut only = capture.clone();
        only.mode = FilterMode::Only;
        // The "only" list is separate and empty: nothing allowed, not "only password managers".
        assert!(!only.allows(Some(&keepass)));
        assert!(!only.allows(None), "unknown app fails safe");
        only.only.push("notepad".into());
        assert!(only.allows(Some(&notepad)));
    }

    #[test]
    fn paste_keystrokes() {
        let term = app(&["gnome-terminal-server"]);
        let gedit = app(&["gedit"]);
        assert_eq!(
            paste_keys_for(Some(&term), &[], Platform::Linux),
            PasteKeys::CtrlShiftV
        );
        assert_eq!(
            paste_keys_for(Some(&gedit), &[], Platform::Linux),
            PasteKeys::CtrlV
        );
        assert_eq!(paste_keys_for(None, &[], Platform::MacOs), PasteKeys::CmdV);
        let overrides = [PasteOverride {
            app: "gedit".into(),
            keys: PasteKeys::ShiftInsert,
        }];
        assert_eq!(
            paste_keys_for(Some(&gedit), &overrides, Platform::Linux),
            PasteKeys::ShiftInsert
        );
    }
}
