//! User settings, stored as human-readable TOML.
//!
//! - Missing keys take their defaults, unknown keys are ignored (forward
//!   compatible), and a broken file never stops the app: defaults are used and
//!   the bad file is kept aside for the user.
//! - Saving writes a temp file and renames it, so a crash can't leave a
//!   half-written settings file.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::apps::{AppFilter, PasteOverride};
use crate::library::HistoryLimits;
use crate::presets::{self, Platform};
use crate::{Error, Result};

pub const DEFAULT_POPUP_WIDTH: u32 = 460;
pub const DEFAULT_POPUP_HEIGHT: u32 = 540;
pub const MIN_POPUP_WIDTH: u32 = 360;
pub const MIN_POPUP_HEIGHT: u32 = 300;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct Settings {
    pub general: General,
    pub hotkey: Hotkey,
    pub history: History,
    pub privacy: Privacy,
    pub popup: Popup,
    pub paste: Paste,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct General {
    pub launch_at_login: bool,
    pub theme: Theme,
    /// Set after the welcome card has been dismissed.
    pub welcome_done: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Hotkey {
    /// e.g. "Ctrl+Shift+V" / "Cmd+Shift+V".
    pub shortcut: String,
    /// Where the hotkey opens the popup (elsewhere it passes through).
    pub apps: AppFilter,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct History {
    /// 0 = unlimited.
    pub max_items: u32,
    /// 0 = never expire.
    pub max_age_days: u32,
    /// Also keep HTML/RTF so items can be pasted with formatting.
    pub keep_formatting: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Privacy {
    /// Skip copies that password managers mark as "don't record".
    pub honour_password_flags: bool,
    pub recording_paused: bool,
    /// Which apps' copies are recorded.
    pub capture: AppFilter,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Popup {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Paste {
    pub overrides: Vec<PasteOverride>,
}


impl Default for General {
    fn default() -> Self {
        General {
            launch_at_login: true,
            theme: Theme::System,
            welcome_done: false,
        }
    }
}

impl Default for Hotkey {
    fn default() -> Self {
        Hotkey {
            shortcut: default_shortcut().into(),
            apps: AppFilter::default(),
        }
    }
}

impl Default for History {
    fn default() -> Self {
        History {
            max_items: 500,
            max_age_days: 90,
            keep_formatting: false,
        }
    }
}

impl Default for Privacy {
    fn default() -> Self {
        Privacy {
            honour_password_flags: true,
            recording_paused: false,
            capture: AppFilter::excluding(presets::password_manager_entries(Platform::current())),
        }
    }
}

impl Default for Popup {
    fn default() -> Self {
        Popup {
            width: DEFAULT_POPUP_WIDTH,
            height: DEFAULT_POPUP_HEIGHT,
        }
    }
}

pub fn default_shortcut() -> &'static str {
    if Platform::current() == Platform::MacOs {
        "Cmd+Shift+V"
    } else {
        "Ctrl+Shift+V"
    }
}

impl History {
    pub fn limits(&self) -> HistoryLimits {
        HistoryLimits {
            max_items: (self.max_items > 0).then_some(self.max_items),
            max_age_days: (self.max_age_days > 0).then_some(self.max_age_days),
        }
    }
}

impl Popup {
    /// Size clamped to the minimum (a hand-edited file can't make it unusable).
    pub fn size(&self) -> (u32, u32) {
        (
            self.width.max(MIN_POPUP_WIDTH),
            self.height.max(MIN_POPUP_HEIGHT),
        )
    }
}

/// Result of loading settings.
pub struct Loaded {
    pub settings: Settings,
    /// Set when the file was unreadable: a message for the user.
    pub warning: Option<String>,
}

impl Settings {
    /// Load from `path`; a missing file gives defaults, a broken one gives
    /// defaults plus a warning (the broken file is renamed, never deleted).
    pub fn load(path: &Path) -> Loaded {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Loaded {
                    settings: Settings::default(),
                    warning: None,
                }
            }
            Err(e) => {
                return Loaded {
                    settings: Settings::default(),
                    warning: Some(format!(
                        "Couldn't read your settings ({e}). Using defaults."
                    )),
                }
            }
        };
        match toml::from_str::<Settings>(&text) {
            Ok(settings) => Loaded {
                settings,
                warning: None,
            },
            Err(e) => {
                let kept = keep_aside(path);
                let where_ = kept
                    .map(|p| format!(" It was saved as “{}”.", p.display()))
                    .unwrap_or_default();
                Loaded {
                    settings: Settings::default(),
                    warning: Some(format!(
                        "Your settings file had an error, so defaults are being used.{where_}\n{}",
                        e.message()
                    )),
                }
            }
        }
    }

    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).map_err(|e| Error::Settings(e.to_string()))
    }

    /// Save atomically: write `<path>.tmp`, then rename over `path`.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = with_suffix(path, ".tmp");
        std::fs::write(&tmp, self.to_toml()?)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

fn keep_aside(path: &Path) -> Option<PathBuf> {
    let dest = with_suffix(path, &format!(".invalid-{}", crate::time::now_ms()));
    std::fs::rename(path, &dest).ok().map(|_| dest)
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_spec() {
        let s = Settings::default();
        assert_eq!(s.history.limits().max_items, Some(500));
        assert_eq!(s.history.limits().max_age_days, Some(90));
        assert!(!s.history.keep_formatting);
        assert!(s.privacy.honour_password_flags);
        assert!(
            s.hotkey.apps.except.is_empty(),
            "nothing pre-ticked for the hotkey"
        );
        assert!(
            s.privacy
                .capture
                .except
                .iter()
                .any(|e| e == "preset:keepassxc"),
            "password managers pre-ticked for capture"
        );
        assert!(s.general.launch_at_login);
    }

    #[test]
    fn round_trip_and_partial_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        let mut s = Settings::default();
        s.history.max_items = 0;
        s.hotkey.apps.except.push("preset:excel".into());
        s.save(&path).unwrap();
        let loaded = Settings::load(&path);
        assert!(loaded.warning.is_none());
        assert_eq!(loaded.settings, s);
        assert_eq!(loaded.settings.history.limits().max_items, None);

        // A partial, hand-written file fills the rest with defaults and ignores unknown keys.
        std::fs::write(&path, "[history]\nmax_items = 42\nfuture_option = true\n").unwrap();
        let loaded = Settings::load(&path);
        assert!(loaded.warning.is_none());
        assert_eq!(loaded.settings.history.max_items, 42);
        assert_eq!(loaded.settings.history.max_age_days, 90);
    }

    #[test]
    fn broken_file_gives_defaults_and_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        std::fs::write(&path, "[history\nmax_items = \"lots\"").unwrap();
        let loaded = Settings::load(&path);
        assert_eq!(loaded.settings, Settings::default());
        assert!(loaded.warning.unwrap().contains("defaults"));
        assert!(!path.exists());
        let kept = std::fs::read_dir(dir.path()).unwrap().count();
        assert_eq!(kept, 1, "the broken file is renamed, not deleted");
    }

    #[test]
    fn popup_size_is_clamped() {
        let p = Popup {
            width: 10,
            height: 9999,
        };
        assert_eq!(p.size(), (MIN_POPUP_WIDTH, 9999));
    }
}
