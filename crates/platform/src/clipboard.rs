//! Clipboard reading/writing and change watching.
//!
//! Change detection uses the OS's cheap change counter where there is one
//! (Windows `GetClipboardSequenceNumber`, macOS `changeCount`), so the
//! clipboard is only opened when something actually changed. Linux has no
//! counter, so the text is compared instead.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use crate::{Error, Result};

/// One clipboard change.
#[derive(Debug, Clone, Default)]
pub struct Change {
    pub text: String,
    pub html: Option<String>,
    /// Rich Text Format (Windows/macOS), e.g. from Word or Outlook.
    pub rtf: Option<String>,
    /// Marked "don't record" by the source (password managers).
    pub concealed: bool,
    /// The app in front when the change was seen.
    pub source: Option<crate::foreground::AppInfo>,
}

/// Put text (and optionally HTML and RTF) on the clipboard.
pub fn write(text: &str, html: Option<&str>, rtf: Option<&str>) -> Result<()> {
    let mut cb = arboard::Clipboard::new().map_err(Error::new)?;
    match html {
        Some(html) => cb.set().html(html, Some(text)).map_err(Error::new)?,
        None => cb.set().text(text).map_err(Error::new)?,
    }
    if let Some(rtf) = rtf {
        add_rtf(rtf);
    }
    Ok(())
}

#[cfg(windows)]
const RTF_FORMAT: &str = "Rich Text Format";

/// Read the clipboard's RTF, if any (Windows/macOS).
pub fn read_rtf() -> Option<String> {
    #[cfg(windows)]
    {
        let _open = clipboard_win::Clipboard::new_attempts(10).ok()?;
        let format = clipboard_win::raw::register_format(RTF_FORMAT)?;
        let mut bytes = Vec::new();
        clipboard_win::raw::get_vec(format.get(), &mut bytes).ok()?;
        while bytes.last() == Some(&0) {
            bytes.pop();
        }
        (!bytes.is_empty()).then(|| String::from_utf8_lossy(&bytes).into_owned())
    }
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::{NSPasteboard, NSPasteboardTypeRTF};
        // SAFETY: reading a standard pasteboard type.
        let data = NSPasteboard::generalPasteboard().dataForType(unsafe { NSPasteboardTypeRTF })?;
        let bytes = data.to_vec();
        (!bytes.is_empty()).then(|| String::from_utf8_lossy(&bytes).into_owned())
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        None
    }
}

/// Add RTF next to what was just written (without clearing it).
fn add_rtf(rtf: &str) {
    #[cfg(windows)]
    {
        let Ok(_open) = clipboard_win::Clipboard::new_attempts(10) else {
            return;
        };
        if let Some(format) = clipboard_win::raw::register_format(RTF_FORMAT) {
            let mut bytes = rtf.as_bytes().to_vec();
            bytes.push(0);
            let _ = clipboard_win::raw::set_without_clear(format.get(), &bytes);
        }
    }
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::{NSPasteboard, NSPasteboardTypeRTF};
        use objc2_foundation::{NSArray, NSData};
        let pb = NSPasteboard::generalPasteboard();
        // SAFETY: standard pasteboard type; no owner object.
        unsafe {
            pb.addTypes_owner(&NSArray::from_slice(&[NSPasteboardTypeRTF]), None);
            pb.setData_forType(
                Some(&NSData::with_bytes(rtf.as_bytes())),
                NSPasteboardTypeRTF,
            );
        }
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    let _ = rtf;
}

/// The OS change counter, if the OS has one.
pub fn change_counter() -> Option<u64> {
    #[cfg(windows)]
    {
        // SAFETY: no arguments; returns a counter (0 if no access).
        let n = unsafe { windows_sys::Win32::System::DataExchange::GetClipboardSequenceNumber() };
        Some(u64::from(n))
    }
    #[cfg(target_os = "macos")]
    {
        let pb = objc2_app_kit::NSPasteboard::generalPasteboard();
        Some(pb.changeCount() as u64)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        None
    }
}

/// Does the current clipboard content carry a "don't record" marker?
pub fn is_concealed() -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::DataExchange::{
            IsClipboardFormatAvailable, RegisterClipboardFormatW,
        };
        let has = |name: &str| {
            let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
            // SAFETY: `wide` is a valid NUL-terminated UTF-16 string.
            let fmt = unsafe { RegisterClipboardFormatW(wide.as_ptr()) };
            fmt != 0 && unsafe { IsClipboardFormatAvailable(fmt) } != 0
        };
        has("ExcludeClipboardContentFromMonitorProcessing") || has("Clipboard Viewer Ignore")
    }
    #[cfg(target_os = "macos")]
    {
        let pb = objc2_app_kit::NSPasteboard::generalPasteboard();
        let Some(types) = pb.types() else {
            return false;
        };
        types.iter().any(|t| {
            let t = t.to_string();
            t == "org.nspasteboard.ConcealedType" || t == "org.nspasteboard.TransientType"
        })
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        // KeePassXC and others mark secrets with this target.
        const HINT: &str = "x-kde-passwordManagerHint";
        if crate::is_wayland() {
            use wl_clipboard_rs::paste::{get_mime_types, ClipboardType, Seat};
            get_mime_types(ClipboardType::Regular, Seat::Unspecified)
                .is_ok_and(|m| m.contains(HINT))
        } else {
            x11_targets_contain(HINT).unwrap_or(false)
        }
    }
}

/// Ask the X11 clipboard owner for its TARGETS and look for `name`.
#[cfg(not(any(windows, target_os = "macos")))]
fn x11_targets_contain(name: &str) -> Option<bool> {
    use std::time::{Duration, Instant};
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, CreateWindowAux, WindowClass};
    use x11rb::protocol::Event;

    let (conn, screen) = x11rb::connect(None).ok()?;
    let root = conn.setup().roots.get(screen)?.root;
    let win = conn.generate_id().ok()?;
    conn.create_window(
        0,
        win,
        root,
        0,
        0,
        1,
        1,
        0,
        WindowClass::INPUT_ONLY,
        0,
        &CreateWindowAux::new(),
    )
    .ok()?;
    let atom = |n: &[u8]| {
        conn.intern_atom(false, n)
            .ok()?
            .reply()
            .ok()
            .map(|r| r.atom)
    };
    let (clipboard, targets, prop, wanted) = (
        atom(b"CLIPBOARD")?,
        atom(b"TARGETS")?,
        atom(b"SHIFTPASTE_TARGETS")?,
        atom(name.as_bytes())?,
    );
    conn.convert_selection(win, clipboard, targets, prop, x11rb::CURRENT_TIME)
        .ok()?;
    conn.flush().ok()?;
    let deadline = Instant::now() + Duration::from_millis(200);
    let found = loop {
        match conn.poll_for_event().ok()? {
            Some(Event::SelectionNotify(e)) if e.requestor == win => {
                if e.property == x11rb::NONE {
                    break false;
                }
                let reply = conn
                    .get_property(true, win, prop, AtomEnum::ATOM, 0, 4096)
                    .ok()?
                    .reply()
                    .ok()?;
                break reply
                    .value32()
                    .is_some_and(|mut atoms| atoms.any(|a| a == wanted));
            }
            Some(_) => continue,
            None if Instant::now() > deadline => break false,
            None => std::thread::sleep(Duration::from_millis(5)),
        }
    };
    let _ = conn.destroy_window(win);
    let _ = conn.flush();
    Some(found)
}

/// Background thread that reports clipboard changes.
pub struct Watcher {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Watcher {
    /// `want_html` is read on every change, so the setting can change live.
    pub fn spawn(
        want_html: Arc<AtomicBool>,
        on_change: impl Fn(Change) + Send + 'static,
    ) -> Watcher {
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = stop.clone();
        let handle = std::thread::Builder::new()
            .name("clipboard-watch".into())
            .spawn(move || watch(stop2, want_html, on_change))
            .ok();
        Watcher { stop, handle }
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn watch(stop: Arc<AtomicBool>, want_html: Arc<AtomicBool>, on_change: impl Fn(Change)) {
    let has_counter = change_counter().is_some();
    let interval = Duration::from_millis(if has_counter { 250 } else { 500 });
    let mut clipboard: Option<arboard::Clipboard> = None;
    let mut last_counter = change_counter();
    // Whatever is on the clipboard at start-up isn't a new copy: remember it
    // before watching (it may be nothing, which is fine).
    let mut last_text: Option<String> = if has_counter {
        None
    } else {
        clipboard = arboard::Clipboard::new().ok();
        clipboard.as_mut().and_then(|cb| cb.get_text().ok())
    };
    while !stop.load(Ordering::SeqCst) {
        std::thread::sleep(interval);
        if has_counter {
            let now = change_counter();
            if now == last_counter {
                continue;
            }
            last_counter = now;
        }
        if clipboard.is_none() {
            clipboard = arboard::Clipboard::new().ok();
        }
        let Some(cb) = clipboard.as_mut() else {
            continue;
        };
        let text = match cb.get_text() {
            Ok(t) => t,
            // Non-text content or the clipboard is busy: try again next tick.
            Err(_) => continue,
        };
        if !has_counter {
            if last_text.as_deref() == Some(text.as_str()) {
                continue;
            }
            last_text = Some(text.clone());
        }
        if text.trim().is_empty() {
            continue;
        }
        let concealed = is_concealed();
        let rich = !concealed && want_html.load(Ordering::Relaxed);
        let html = rich.then(|| cb.get().html().ok()).flatten();
        let rtf = rich.then(read_rtf).flatten();
        on_change(Change {
            text,
            html,
            rtf,
            concealed,
            source: crate::foreground::current_app(),
        });
    }
}
