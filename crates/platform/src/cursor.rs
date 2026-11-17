//! Mouse position in physical desktop pixels (to open the popup next to it).

/// `None` when the OS can't tell (e.g. Wayland); the caller then centres.
pub fn position() -> Option<(i32, i32)> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::POINT;
        use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;
        let mut p = POINT { x: 0, y: 0 };
        // SAFETY: `p` is a valid out-pointer.
        (unsafe { GetCursorPos(&mut p) } != 0).then_some((p.x, p.y))
    }
    #[cfg(target_os = "macos")]
    {
        use objc2::MainThreadMarker;
        use objc2_app_kit::{NSEvent, NSScreen};
        let mtm = MainThreadMarker::new()?;
        let screen = NSScreen::mainScreen(mtm)?;
        let frame = screen.frame();
        let scale = screen.backingScaleFactor();
        let p = NSEvent::mouseLocation();
        // AppKit's origin is bottom-left; the desktop's is top-left.
        Some((
            (p.x * scale) as i32,
            ((frame.size.height - p.y) * scale) as i32,
        ))
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        if crate::is_wayland() {
            return None;
        }
        use x11rb::connection::Connection;
        use x11rb::protocol::xproto::ConnectionExt;
        let (conn, screen) = x11rb::connect(None).ok()?;
        let root = conn.setup().roots.get(screen)?.root;
        let reply = conn.query_pointer(root).ok()?.reply().ok()?;
        Some((i32::from(reply.root_x), i32::from(reply.root_y)))
    }
}
