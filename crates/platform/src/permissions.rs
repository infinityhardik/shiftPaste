//! OS permissions Shift Paste needs (macOS Accessibility, to paste for you).

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

/// Can Shift Paste send the paste keystroke? Only macOS asks for permission.
pub fn can_paste() -> bool {
    #[cfg(target_os = "macos")]
    {
        // SAFETY: no arguments; only reads the current process's trust state.
        unsafe { AXIsProcessTrusted() }
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Open the settings page where the user grants that permission.
pub fn open_paste_settings() {
    #[cfg(target_os = "macos")]
    crate::shell::open_url(
        "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
    );
}
