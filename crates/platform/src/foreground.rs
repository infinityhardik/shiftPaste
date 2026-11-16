//! The app in front (for app lists and paste keystrokes), and returning focus
//! to it after the popup closes.

/// The app in front: a display name and every identifier we could find
/// (exe file name, app name, macOS bundle id).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppInfo {
    pub name: String,
    pub ids: Vec<String>,
}

/// Identify the app in front. `None` when the OS can't tell (e.g. Wayland).
#[cfg(not(target_os = "macos"))]
pub fn current_app() -> Option<AppInfo> {
    if crate::is_wayland() {
        return None;
    }
    let w = active_win_pos_rs::get_active_window().ok()?;
    let mut ids = Vec::new();
    if let Some(name) = w.process_path.file_name() {
        ids.push(name.to_string_lossy().into_owned());
    }
    if !w.app_name.is_empty() {
        ids.push(w.app_name.clone());
    }
    let name = if w.app_name.is_empty() {
        ids.first().cloned().unwrap_or_default()
    } else {
        w.app_name
    };
    Some(AppInfo { name, ids })
}

/// Identify the app in front (bundle id, name, executable) via AppKit.
#[cfg(target_os = "macos")]
pub fn current_app() -> Option<AppInfo> {
    let app = objc2_app_kit::NSWorkspace::sharedWorkspace().frontmostApplication()?;
    let mut ids = Vec::new();
    if let Some(bundle) = app.bundleIdentifier() {
        ids.push(bundle.to_string());
    }
    let name = app
        .localizedName()
        .map(|n| n.to_string())
        .unwrap_or_default();
    if !name.is_empty() {
        ids.push(name.clone());
    }
    if let Some(exe) = app.executableURL().and_then(|u| u.lastPathComponent()) {
        ids.push(exe.to_string());
    }
    Some(AppInfo { name, ids })
}

/// Something we can give focus back to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusTarget {
    #[cfg(windows)]
    hwnd: isize,
    #[cfg(target_os = "macos")]
    pid: i32,
    #[cfg(not(any(windows, target_os = "macos")))]
    _none: (),
}

/// Remember the window in front (call before showing the popup).
pub fn remember() -> Option<FocusTarget> {
    #[cfg(windows)]
    {
        // SAFETY: no arguments; returns the foreground window or null.
        let hwnd = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
        (!hwnd.is_null()).then_some(FocusTarget {
            hwnd: hwnd as isize,
        })
    }
    #[cfg(target_os = "macos")]
    {
        let ws = objc2_app_kit::NSWorkspace::sharedWorkspace();
        let app = ws.frontmostApplication()?;
        let pid = app.processIdentifier();
        (pid != std::process::id() as i32).then_some(FocusTarget { pid })
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        Some(FocusTarget { _none: () })
    }
}

/// Bring the remembered window back to the front. Returns false if it's gone
/// (on Linux the window manager does this when the popup hides).
pub fn restore(target: FocusTarget) -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{IsWindow, SetForegroundWindow};
        let hwnd = target.hwnd as windows_sys::Win32::Foundation::HWND;
        // SAFETY: `hwnd` came from GetForegroundWindow; IsWindow validates it.
        unsafe { IsWindow(hwnd) != 0 && SetForegroundWindow(hwnd) != 0 }
    }
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
        match NSRunningApplication::runningApplicationWithProcessIdentifier(target.pid) {
            #[allow(deprecated)]
            Some(app) => app.activateWithOptions(NSApplicationActivationOptions::empty()),
            None => false,
        }
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = target;
        false
    }
}
