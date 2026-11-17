//! Where settings, data and logs live (normal install or portable mode).

use std::path::{Path, PathBuf};

use crate::{Error, Result};

/// A folder with this name next to the executable turns on portable mode.
pub const PORTABLE_DIR: &str = "ShiftPaste-data";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub log_dir: PathBuf,
    pub portable: bool,
    /// Set when a portable folder exists but can't be written (e.g. a locked
    /// USB stick): the normal locations are used and this explains why.
    pub portable_problem: Option<String>,
}

impl AppPaths {
    /// Resolve paths. `exe_dir` is the folder holding the executable.
    pub fn resolve(exe_dir: Option<&Path>) -> Result<AppPaths> {
        let mut problem = None;
        if let Some(dir) = exe_dir.map(|d| d.join(PORTABLE_DIR)) {
            if dir.is_dir() {
                match check_writable(&dir) {
                    Ok(()) => return Ok(AppPaths::portable(dir)),
                    Err(e) => {
                        problem = Some(format!(
                            "The portable data folder “{}” can't be written ({e}), so your data is being kept in the normal location instead.",
                            dir.display()
                        ))
                    }
                }
            }
        }
        let dirs = directories::ProjectDirs::from("com", "infinityhardik", "ShiftPaste")
            .ok_or_else(|| Error::invalid("Couldn't find your home folder."))?;
        let data_dir = dirs.data_local_dir().to_path_buf();
        Ok(AppPaths {
            config_dir: dirs.config_dir().to_path_buf(),
            log_dir: data_dir.join("logs"),
            data_dir,
            portable: false,
            portable_problem: problem,
        })
    }

    fn portable(dir: PathBuf) -> AppPaths {
        AppPaths {
            config_dir: dir.clone(),
            log_dir: dir.join("logs"),
            data_dir: dir,
            portable: true,
            portable_problem: None,
        }
    }

    pub fn settings_file(&self) -> PathBuf {
        self.config_dir.join("settings.toml")
    }

    pub fn database_file(&self) -> PathBuf {
        self.data_dir.join("shiftpaste.db")
    }
}

fn check_writable(dir: &Path) -> std::io::Result<()> {
    let probe = dir.join(".write-test");
    std::fs::write(&probe, b"ok")?;
    std::fs::remove_file(&probe)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_mode_when_folder_exists() {
        let exe_dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(exe_dir.path().join(PORTABLE_DIR)).unwrap();
        let paths = AppPaths::resolve(Some(exe_dir.path())).unwrap();
        assert!(paths.portable);
        assert!(paths.database_file().starts_with(exe_dir.path()));
        assert!(paths.settings_file().starts_with(exe_dir.path()));
    }

    #[test]
    fn normal_mode_without_folder() {
        let exe_dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::resolve(Some(exe_dir.path())).unwrap();
        assert!(!paths.portable);
        assert!(!paths.database_file().starts_with(exe_dir.path()));
        assert!(paths.portable_problem.is_none());
    }
}
