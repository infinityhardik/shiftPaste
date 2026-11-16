//! OS integration for Shift Paste: clipboard, global hotkey, foreground app,
//! paste simulation, tray, launch-at-login, dialogs and single-instance IPC.
//!
//! Every function degrades gracefully: where an OS can't do something (e.g.
//! Wayland can't report the foreground app) it returns `None`/`false` instead
//! of failing, and the app explains the limitation in its UI.

pub mod autostart;
pub mod clipboard;
pub mod cursor;
pub mod foreground;
pub mod hotkey;
pub mod input;
pub mod instance;
pub mod shell;
pub mod tray;

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Error(pub String);

impl Error {
    pub fn new(msg: impl std::fmt::Display) -> Self {
        Error(msg.to_string())
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// True when running under Wayland (some features are limited there).
pub fn is_wayland() -> bool {
    cfg!(target_os = "linux") && std::env::var_os("WAYLAND_DISPLAY").is_some()
}

/// Process-wide setup. Call first thing in `main`, before any thread starts.
///
/// Linux: GTK is only used for the tray icon, but GDK's start-up loads the
/// OpenGL stack (Mesa + LLVM, ~45 MB resident) just to pick window visuals.
/// Shift Paste never draws with GL, so it's switched off unless the user
/// set `GDK_GL` themselves.
pub fn prepare_process() {
    #[cfg(target_os = "linux")]
    if std::env::var_os("GDK_GL").is_none() {
        // Single-threaded at this point, so changing the environment is safe.
        std::env::set_var("GDK_GL", "disable");
    }
}
