//! File dialogs and opening folders/links with the system's default apps.

use std::path::{Path, PathBuf};

pub fn open_path(path: &Path) {
    if let Err(e) = open::that_detached(path) {
        tracing::warn!("couldn't open {}: {e}", path.display());
    }
}

pub fn open_url(url: &str) {
    if let Err(e) = open::that_detached(url) {
        tracing::warn!("couldn't open {url}: {e}");
    }
}

/// Pick a spreadsheet or CSV to import.
pub fn pick_import_file() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Import Excel or CSV")
        .add_filter(
            "Spreadsheets and CSV",
            &["xlsx", "xlsm", "xlsb", "xls", "ods", "csv", "tsv", "txt"],
        )
        .pick_file()
}

/// Choose where to save an export; `formats` is (label, extension) pairs.
pub fn save_file(title: &str, default_name: &str, formats: &[(&str, &str)]) -> Option<PathBuf> {
    let mut d = rfd::FileDialog::new()
        .set_title(title)
        .set_file_name(default_name);
    for (label, ext) in formats {
        d = d.add_filter(*label, &[*ext]);
    }
    d.save_file()
}

/// Pick an application (for the app lists).
pub fn pick_app() -> Option<PathBuf> {
    let d = rfd::FileDialog::new().set_title("Choose an app");
    #[cfg(windows)]
    let d = d.add_filter("Programs", &["exe"]);
    #[cfg(target_os = "macos")]
    let d = d.set_directory("/Applications");
    d.pick_file()
}

/// A blocking error message box (used before any window exists).
pub fn error_dialog(title: &str, message: &str) {
    let _ = rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title(title)
        .set_description(message)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}
