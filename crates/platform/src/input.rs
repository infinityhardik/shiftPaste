//! Synthetic keystrokes: the paste shortcut, and re-sending a hotkey's key.

use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use global_hotkey::hotkey::Code;

/// Which paste shortcut to press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteKeys {
    CtrlV,
    CtrlShiftV,
    ShiftInsert,
    CmdV,
}

use crate::{Error, Result};

fn enigo() -> Result<Enigo> {
    Enigo::new(&Settings::default()).map_err(|e| {
        Error::new(if cfg!(target_os = "macos") {
            format!("Shift Paste needs the Accessibility permission to paste ({e}).")
        } else {
            format!("Couldn't send the paste keystroke ({e}).")
        })
    })
}

/// A letter key as enigo needs it: real virtual-key codes on Windows (so
/// Ctrl+V is a shortcut, not a typed character), key symbols elsewhere.
fn letter(c: char) -> Key {
    #[cfg(windows)]
    {
        Key::Other(u32::from(c.to_ascii_uppercase()))
    }
    #[cfg(not(windows))]
    {
        Key::Unicode(c.to_ascii_lowercase())
    }
}

/// Press the paste shortcut in the app in front.
pub fn send_paste(keys: PasteKeys) -> Result<()> {
    let mut e = enigo()?;
    // Release modifiers the user may still hold (e.g. Shift from Shift+Enter)
    // so the target sees exactly the paste shortcut.
    for m in [Key::Shift, Key::Alt, Key::Control, Key::Meta] {
        let _ = e.key(m, Direction::Release);
    }
    let (mods, key): (&[Key], Key) = match keys {
        PasteKeys::CtrlV => (&[Key::Control], letter('v')),
        PasteKeys::CtrlShiftV => (&[Key::Control, Key::Shift], letter('v')),
        #[cfg(not(target_os = "macos"))]
        PasteKeys::ShiftInsert => (&[Key::Shift], Key::Insert),
        // Macs have no Insert key.
        #[cfg(target_os = "macos")]
        PasteKeys::ShiftInsert => (&[Key::Meta], letter('v')),
        PasteKeys::CmdV => (&[Key::Meta], letter('v')),
    };
    let err =
        |e: enigo::InputError| Error::new(format!("Couldn't send the paste keystroke ({e})."));
    for m in mods {
        e.key(*m, Direction::Press).map_err(err)?;
    }
    let result = e.key(key, Direction::Click).map_err(err);
    for m in mods.iter().rev() {
        let _ = e.key(*m, Direction::Release);
    }
    result
}

/// Tap a hotkey's main key (modifiers are still physically held).
pub fn tap(code: Code) -> Result<()> {
    let key = match code {
        Code::Space => Key::Space,
        #[cfg(not(target_os = "macos"))]
        Code::Insert => Key::Insert,
        Code::F1 => Key::F1,
        Code::F2 => Key::F2,
        Code::F3 => Key::F3,
        Code::F4 => Key::F4,
        Code::F5 => Key::F5,
        Code::F6 => Key::F6,
        Code::F7 => Key::F7,
        Code::F8 => Key::F8,
        Code::F9 => Key::F9,
        Code::F10 => Key::F10,
        Code::F11 => Key::F11,
        Code::F12 => Key::F12,
        other => {
            let name = other.to_string();
            let c = name
                .strip_prefix("Key")
                .or_else(|| name.strip_prefix("Digit"))
                .and_then(|s| s.chars().next())
                .ok_or_else(|| Error::new(format!("can't re-send {name}")))?;
            if c.is_ascii_digit() {
                Key::Unicode(c)
            } else {
                letter(c)
            }
        }
    };
    enigo()?
        .key(key, Direction::Click)
        .map_err(|e| Error::new(format!("Couldn't pass the shortcut through ({e}).")))
}
