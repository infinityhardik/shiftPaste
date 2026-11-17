//! Native touches for the popup window.

/// Windows: make the popup a tool window (no taskbar button, not in
/// Alt+Tab, like Shift Paste 1.x) and ask Windows 11 for rounded corners.
/// Safe to call repeatedly; a no-op on other systems.
pub fn polish_popup(hwnd: isize) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::HWND;
        use windows_sys::Win32::Graphics::Dwm::{
            DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
        };
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, SWP_FRAMECHANGED,
            SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_EX_APPWINDOW,
            WS_EX_TOOLWINDOW,
        };
        let hwnd = hwnd as HWND;
        // SAFETY: `hwnd` is the popup's live window handle from winit.
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let wanted = (style | WS_EX_TOOLWINDOW as isize) & !(WS_EX_APPWINDOW as isize);
            if wanted != style {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, wanted);
                SetWindowPos(
                    hwnd,
                    std::ptr::null_mut(),
                    0,
                    0,
                    0,
                    0,
                    SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            // Windows 10 doesn't know this attribute and simply ignores it.
            let round = DWMWCP_ROUND;
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                (&round as *const i32).cast(),
                std::mem::size_of::<i32>() as u32,
            );
        }
    }
    #[cfg(not(windows))]
    let _ = hwnd;
}

/// Windows: bring a window to the front without touching its size, position
/// or maximized state: no `ShowWindow` on a visible window and no synthetic
/// keystrokes (an injected Alt can open the app's menu). A minimized window
/// is un-minimized back to how it was (maximized stays maximized).
/// Returns true when the window ends up in front. `false` on other systems.
pub fn activate(hwnd: isize) -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::HWND;
        use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, IsIconic, IsWindow,
            SetForegroundWindow, ShowWindow, SW_RESTORE,
        };
        let hwnd = hwnd as HWND;
        // SAFETY: plain window-manager calls; IsWindow validates the handle and
        // every AttachThreadInput is undone before returning.
        unsafe {
            if IsWindow(hwnd) == 0 {
                return false;
            }
            if IsIconic(hwnd) != 0 {
                // Only for minimized windows: SW_RESTORE brings back the
                // previous state, maximized included.
                ShowWindow(hwnd, SW_RESTORE);
            }
            if GetForegroundWindow() == hwnd {
                return true;
            }
            if SetForegroundWindow(hwnd) != 0 && GetForegroundWindow() == hwnd {
                return true;
            }
            // Windows refused (focus lock): share input state with the window
            // in front for a moment, which lets the switch through.
            let front = GetForegroundWindow();
            let front_thread = GetWindowThreadProcessId(front, std::ptr::null_mut());
            let me = GetCurrentThreadId();
            if front_thread != 0
                && front_thread != me
                && AttachThreadInput(me, front_thread, 1) != 0
            {
                BringWindowToTop(hwnd);
                SetForegroundWindow(hwnd);
                AttachThreadInput(me, front_thread, 0);
            }
            GetForegroundWindow() == hwnd
        }
    }
    #[cfg(not(windows))]
    {
        let _ = hwnd;
        false
    }
}
