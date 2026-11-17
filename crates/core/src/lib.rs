//! Shift Paste core: storage, search, settings, import/export and undo.
//!
//! This crate has no UI and no OS-specific code, so everything in it is
//! unit-tested on every platform.

pub mod apps;
pub mod db;
pub mod error;
pub mod export;
pub mod import;
pub mod library;
pub mod model;
pub mod paths;
pub mod presets;
pub mod search;
pub mod settings;
pub mod time;
pub mod transform;

pub use error::{Error, Result};
