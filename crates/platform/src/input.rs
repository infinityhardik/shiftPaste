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

/// Release modifiers the user may still hold (e.g. Shift from Shift+Enter)
/// so the target sees exactly the paste shortcut.
#[cfg(not(windows))]
fn release_held_modifiers(e: &mut Enigo) {
    for m in [Key::Shift, Key::Alt, Key::Control, Key::Meta] {
        let _ = e.key(m, Direction::Release);
    }
}

/// Windows: release only the modifiers that are really down. A stray Alt or
/// Windows key release reaches the app and Windows as a "tap": it can open
/// the app's menu or the Start menu, or turn the paste into a window command
/// (Alt+Space, Win+arrows) that un-maximizes or moves the window. A neutral,
/// unassigned key before releasing them keeps that from happening.
#[cfg(windows)]
fn release_held_modifiers(e: &mut Enigo) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_RCONTROL, VK_RMENU,
        VK_RSHIFT, VK_RWIN,
    };
    /// Unassigned virtual key (what AutoHotkey uses as its "menu mask" key).
    const MASK_KEY: u32 = 0xE8;
    // SAFETY: GetAsyncKeyState only reads the global key state.
    let down = |vk: u16| unsafe { GetAsyncKeyState(i32::from(vk)) } as u16 & 0x8000 != 0;
    let held: Vec<(u16, Key)> = [
        (VK_LSHIFT, Key::LShift),
        (VK_RSHIFT, Key::RShift),
        (VK_LCONTROL, Key::LControl),
        (VK_RCONTROL, Key::RControl),
        (VK_LMENU, Key::LMenu),
        (VK_RMENU, Key::RMenu),
        (VK_LWIN, Key::LWin),
        (VK_RWIN, Key::RWin),
    ]
    .into_iter()
    .filter(|(vk, _)| down(*vk))
    .collect();
    if held
        .iter()
        .any(|(vk, _)| [VK_LMENU, VK_RMENU, VK_LWIN, VK_RWIN].contains(vk))
    {
        let _ = e.key(Key::Other(MASK_KEY), Direction::Click);
    }
    for (_, key) in held {
        let _ = e.key(key, Direction::Release);
    }
}

/// Press the paste shortcut in the app in front.
pub fn send_paste(keys: PasteKeys) -> Result<()> {
    let mut e = enigo()?;
    release_held_modifiers(&mut e);
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
