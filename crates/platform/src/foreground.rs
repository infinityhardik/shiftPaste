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
    /// Its size and maximized state when remembered, so a paste never
    /// leaves it resized.
    #[cfg(windows)]
    placement: Option<win::Placement>,
    #[cfg(target_os = "macos")]
    pid: i32,
    #[cfg(not(any(windows, target_os = "macos")))]
    _none: (),
}

/// Remember the window in front (call before showing the popup). On Windows
/// the taskbar, tray and Shift Paste's own windows are skipped, so opening
/// from the tray remembers the app you were actually using.
pub fn remember() -> Option<FocusTarget> {
    #[cfg(windows)]
    {
        win::remember().map(|hwnd| FocusTarget {
            hwnd,
            placement: win::placement(hwnd),
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

/// The app that owns a remembered window (for app lists and paste keys).
pub fn app_of(target: FocusTarget) -> Option<AppInfo> {
    #[cfg(windows)]
    {
        let exe = win::exe_name(target.hwnd)?;
        let name = exe
            .trim_end_matches(".exe")
            .trim_end_matches(".EXE")
            .to_string();
        Some(AppInfo {
            name,
            ids: vec![exe],
        })
    }
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSRunningApplication;
        let app = NSRunningApplication::runningApplicationWithProcessIdentifier(target.pid)?;
        let mut ids = Vec::new();
        if let Some(b) = app.bundleIdentifier() {
            ids.push(b.to_string());
        }
        let name = app
            .localizedName()
            .map(|n| n.to_string())
            .unwrap_or_default();
        if !name.is_empty() {
            ids.push(name.clone());
        }
        Some(AppInfo { name, ids })
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = target;
        current_app()
    }
}

#[cfg(windows)]
mod win {
    use windows_sys::Win32::Foundation::{CloseHandle, HWND, RECT};
    use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetTopWindow, GetWindow, GetWindowLongPtrW,
        GetWindowRect, GetWindowTextLengthW, GetWindowThreadProcessId, IsIconic, IsWindow,
        IsWindowVisible, IsZoomed, SetWindowPos, ShowWindow, GWL_EXSTYLE, GW_HWNDNEXT, GW_OWNER,
        SWP_NOACTIVATE, SWP_NOOWNERZORDER, SWP_NOZORDER, SW_MAXIMIZE, SW_SHOWNOACTIVATE,
        WS_EX_TOOLWINDOW,
    };

    /// A window's maximized state and outer rectangle (left, top, right, bottom).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Placement {
        maximized: bool,
        rect: [i32; 4],
    }

    /// `None` when the window is gone or minimized.
    pub fn placement(hwnd: isize) -> Option<Placement> {
        let hwnd = hwnd as HWND;
        let mut r = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        // SAFETY: plain queries; `r` is a valid out-pointer.
        unsafe {
            if IsWindow(hwnd) == 0 || IsIconic(hwnd) != 0 || GetWindowRect(hwnd, &mut r) == 0 {
                return None;
            }
            Some(Placement {
                maximized: IsZoomed(hwnd) != 0,
                rect: [r.left, r.top, r.right, r.bottom],
            })
        }
    }

    /// Put the window back the way it was if switching to it (or pasting)
    /// changed it: re-maximize it, or give back its exact size and position.
    /// A window that was minimized or closed meanwhile is left alone.
    pub fn keep(hwnd: isize, was: Placement) {
        let Some(now) = placement(hwnd) else {
            return;
        };
        if now == was || (was.maximized && now.maximized) {
            return;
        }
        tracing::info!(?was, ?now, "the paste target changed size; putting it back");
        let hwnd = hwnd as HWND;
        // SAFETY: `hwnd` was validated by `placement` just above.
        unsafe {
            if was.maximized {
                // It's the window being pasted into, so it is (and stays) in front.
                ShowWindow(hwnd, SW_MAXIMIZE);
                return;
            }
            if now.maximized {
                ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            }
            let [l, t, r, b] = was.rect;
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                l,
                t,
                r - l,
                b - t,
                SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_NOACTIVATE,
            );
        }
    }

    /// Taskbar, tray overflow and desktop windows: never paste targets.
    const SHELL_CLASSES: [&str; 6] = [
        "Shell_TrayWnd",
        "Shell_SecondaryTrayWnd",
        "NotifyIconOverflowWindow",
        "TopLevelWindowForOverflowXamlIsland",
        "Progman",
        "WorkerW",
    ];

    fn class_name(hwnd: HWND) -> String {
        let mut buf = [0u16; 128];
        // SAFETY: buffer and length match.
        let n = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }

    fn pid_of(hwnd: HWND) -> u32 {
        let mut pid = 0u32;
        // SAFETY: `pid` is a valid out-pointer.
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        pid
    }

    /// A normal, visible, top-level app window that isn't ours or the shell's.
    fn is_app_window(hwnd: HWND) -> bool {
        if hwnd.is_null()
            || pid_of(hwnd) == std::process::id()
            || SHELL_CLASSES.contains(&class_name(hwnd).as_str())
        {
            return false;
        }
        // SAFETY: plain queries on a window handle.
        unsafe {
            if IsWindowVisible(hwnd) == 0
                || !GetWindow(hwnd, GW_OWNER).is_null()
                || GetWindowLongPtrW(hwnd, GWL_EXSTYLE) & WS_EX_TOOLWINDOW as isize != 0
                || GetWindowTextLengthW(hwnd) == 0
            {
                return false;
            }
            let mut cloaked = 0u32;
            let hr = DwmGetWindowAttribute(
                hwnd,
                DWMWA_CLOAKED as u32,
                (&mut cloaked as *mut u32).cast(),
                std::mem::size_of::<u32>() as u32,
            );
            hr != 0 || cloaked == 0
        }
    }

    pub fn remember() -> Option<isize> {
        // SAFETY: plain window-manager queries.
        unsafe {
            let fg = GetForegroundWindow();
            if is_app_window(fg) {
                return Some(fg as isize);
            }
            // The taskbar/tray (or our own window) is in front: take the next
            // real app window in z-order, i.e. the one the user came from.
            let mut h = GetTopWindow(std::ptr::null_mut());
            for _ in 0..512 {
                if h.is_null() {
                    break;
                }
                if is_app_window(h) {
                    return Some(h as isize);
                }
                h = GetWindow(h, GW_HWNDNEXT);
            }
            (!fg.is_null()).then_some(fg as isize)
        }
    }

    pub fn exe_name(hwnd: isize) -> Option<String> {
        let pid = pid_of(hwnd as HWND);
        // SAFETY: the handle is checked and closed; the buffer length is passed in and updated.
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process.is_null() {
                return None;
            }
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let ok =
                QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
            CloseHandle(process);
            if ok == 0 {
                return None;
            }
            let path = String::from_utf16_lossy(&buf[..len as usize]);
            path.rsplit(['\\', '/']).next().map(str::to_owned)
        }
    }
}

/// Bring the remembered window back to the front, exactly as it was: a
/// maximized window stays maximized and nothing is moved or resized. Returns
/// false if it's gone (on Linux the window manager does this when the popup
/// hides).
pub fn restore(target: FocusTarget) -> bool {
    #[cfg(windows)]
    {
        let ok = crate::window::activate(target.hwnd);
        keep_size(target);
        ok
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

/// Windows: undo any change to the remembered window's size or maximized
/// state since it was remembered (call after restoring focus and again after
/// pasting). Elsewhere a no-op: macOS and Linux never resize on activation.
pub fn keep_size(target: FocusTarget) {
    #[cfg(windows)]
    if let Some(was) = target.placement {
        win::keep(target.hwnd, was);
    }
    #[cfg(not(windows))]
    let _ = target;
}
