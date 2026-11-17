//! Global hotkey: parse "Ctrl+Shift+V", register it, and let it pass through
//! to the app in front when the popup shouldn't open there.

use std::str::FromStr;

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

use crate::{Error, Result};

/// Parse a shortcut such as "Ctrl+Shift+V", "Cmd+Shift+V" or "Alt+F2".
pub fn parse(shortcut: &str) -> Result<HotKey> {
    let mut mods = Modifiers::empty();
    let mut code = None;
    for part in shortcut.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        match part.to_lowercase().as_str() {
            "ctrl" | "control" => mods |= Modifiers::CONTROL,
            "shift" | "⇧" => mods |= Modifiers::SHIFT,
            "alt" | "option" | "opt" | "⌥" => mods |= Modifiers::ALT,
            "cmd" | "command" | "win" | "super" | "meta" | "⌘" => mods |= Modifiers::META,
            _ if code.is_some() => {
                return Err(Error::new(format!("“{shortcut}” has more than one key.")))
            }
            key => {
                code = Some(key_code(key).ok_or_else(|| {
                    Error::new(format!("“{part}” isn't a key Shift Paste can use."))
                })?)
            }
        }
    }
    let code =
        code.ok_or_else(|| Error::new("Add a key after the modifiers, e.g. Ctrl+Shift+V."))?;
    if mods.is_empty()
        && !matches!(
            code,
            Code::F1
                | Code::F2
                | Code::F3
                | Code::F4
                | Code::F5
                | Code::F6
                | Code::F7
                | Code::F8
                | Code::F9
                | Code::F10
                | Code::F11
                | Code::F12
        )
    {
        return Err(Error::new(
            "Use at least one of Ctrl, Alt, Shift or Cmd/Win, so normal typing isn't blocked.",
        ));
    }
    Ok(HotKey::new(Some(mods), code))
}

fn key_code(key: &str) -> Option<Code> {
    let k = key.to_uppercase();
    let name = match k.as_str() {
        c if c.len() == 1 && c.as_bytes()[0].is_ascii_alphabetic() => format!("Key{c}"),
        c if c.len() == 1 && c.as_bytes()[0].is_ascii_digit() => format!("Digit{c}"),
        "SPACE" => "Space".into(),
        "`" | "BACKQUOTE" => "Backquote".into(),
        "-" => "Minus".into(),
        "=" => "Equal".into(),
        "," => "Comma".into(),
        "." => "Period".into(),
        "/" => "Slash".into(),
        ";" => "Semicolon".into(),
        "'" => "Quote".into(),
        "[" => "BracketLeft".into(),
        "]" => "BracketRight".into(),
        "\\" => "Backslash".into(),
        "INSERT" | "INS" => "Insert".into(),
        f if f.starts_with('F') && f[1..].parse::<u8>().is_ok_and(|n| (1..=24).contains(&n)) => {
            f.into()
        }
        _ => return None,
    };
    Code::from_str(&name).ok()
}

/// Key caps for showing a shortcut in the UI, e.g. ["Ctrl", "⇧", "V"].
pub fn caps(shortcut: &str, mac: bool) -> Vec<String> {
    shortcut
        .split('+')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(|p| match p.to_lowercase().as_str() {
            "ctrl" | "control" => {
                if mac {
                    "⌃".into()
                } else {
                    "Ctrl".into()
                }
            }
            "shift" => "⇧".into(),
            "alt" | "option" | "opt" => {
                if mac {
                    "⌥".into()
                } else {
                    "Alt".into()
                }
            }
            "cmd" | "command" | "meta" | "super" => {
                if mac {
                    "⌘".into()
                } else {
                    "Win".into()
                }
            }
            "win" => "Win".into(),
            _ => p.to_uppercase(),
        })
        .collect()
}

/// Owns the OS registration of the popup shortcut.
pub struct Hotkeys {
    manager: GlobalHotKeyManager,
    current: Option<HotKey>,
}

impl Hotkeys {
    pub fn new() -> Result<Hotkeys> {
        Ok(Hotkeys {
            manager: GlobalHotKeyManager::new().map_err(Error::new)?,
            current: None,
        })
    }

    /// Register `shortcut`, replacing the old one. On failure the old shortcut
    /// stays active and the error explains why.
    pub fn set(&mut self, shortcut: &str) -> Result<()> {
        let new = parse(shortcut)?;
        if self.current == Some(new) {
            return Ok(());
        }
        if let Some(old) = self.current.take() {
            let _ = self.manager.unregister(old);
        }
        match self.manager.register(new) {
            Ok(()) => {
                self.current = Some(new);
                Ok(())
            }
            Err(e) => {
                tracing::warn!("couldn't register {shortcut}: {e}");
                Err(Error::new(format!(
                    "{shortcut} is already used by another app. Choose a different shortcut."
                )))
            }
        }
    }

    pub fn id(&self) -> Option<u32> {
        self.current.map(|h| h.id())
    }

    pub fn current(&self) -> Option<HotKey> {
        self.current
    }

    /// Temporarily release the shortcut while `f` runs (so a keystroke we
    /// send can't trigger it), then take it back.
    pub fn suspended<T>(&mut self, f: impl FnOnce() -> T) -> T {
        let held = self.current;
        if let Some(h) = held {
            let _ = self.manager.unregister(h);
        }
        let out = f();
        if let Some(h) = held {
            if self.manager.register(h).is_err() {
                tracing::warn!("couldn't re-register the shortcut after suspending it");
                self.current = None;
            }
        }
        out
    }

    /// Pass the shortcut through to the app in front: release it, re-send the
    /// key (the user is still holding the modifiers), take it back.
    pub fn pass_through(&mut self) {
        if let Some(h) = self.current {
            self.suspended(|| {
                if let Err(e) = crate::input::tap(h.key) {
                    tracing::warn!("pass-through failed: {e}");
                }
            });
        }
    }
}

/// Call `f` (from the hotkey thread) each time a shortcut is pressed down.
pub fn on_pressed(f: impl Fn(u32) + Send + Sync + 'static) {
    GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
        if e.state() == HotKeyState::Pressed {
            f(e.id());
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_shortcuts() {
        let h = parse("Ctrl+Shift+V").unwrap();
        assert_eq!(
            h,
            HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyV)
        );
        assert_eq!(
            parse("cmd + shift + v").unwrap(),
            HotKey::new(Some(Modifiers::META | Modifiers::SHIFT), Code::KeyV)
        );
        assert!(parse("Alt+F2").is_ok());
        assert!(parse("F9").is_ok(), "function keys may stand alone");
        assert!(parse("V").is_err(), "a bare letter would block typing");
        assert!(parse("Ctrl+Shift").is_err());
        assert!(parse("Ctrl+V+B").is_err());
        assert!(parse("Ctrl+Banana").is_err());
    }

    #[test]
    fn caps_for_display() {
        assert_eq!(caps("Ctrl+Shift+V", false), ["Ctrl", "⇧", "V"]);
        assert_eq!(caps("Cmd+Shift+V", true), ["⌘", "⇧", "V"]);
    }
}
