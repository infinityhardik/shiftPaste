//! Launch at login (Windows Run key, macOS LaunchAgent, Linux autostart entry).

use auto_launch::{AutoLaunch, AutoLaunchBuilder, MacOSLaunchMode};

use crate::{Error, Result};

fn launcher() -> Result<AutoLaunch> {
    let exe = std::env::current_exe().map_err(Error::new)?;
    AutoLaunchBuilder::new()
        .set_app_name("Shift Paste")
        .set_app_path(&exe.to_string_lossy())
        .set_macos_launch_mode(MacOSLaunchMode::LaunchAgent)
        .build()
        .map_err(Error::new)
}

/// Turn launch-at-login on or off. Development builds never register
/// themselves (so `cargo run` doesn't end up starting at login).
pub fn set(enabled: bool) -> Result<()> {
    if cfg!(debug_assertions) {
        tracing::info!("debug build: not changing launch-at-login (wanted {enabled})");
        return Ok(());
    }
    let l = launcher()?;
    let on = l.is_enabled().unwrap_or(false);
    match (enabled, on) {
        (true, _) => l.enable().map_err(Error::new),
        (false, true) => l.disable().map_err(Error::new),
        (false, false) => Ok(()),
    }
}
