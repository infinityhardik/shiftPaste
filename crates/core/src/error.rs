//! One error type for the whole core, with messages that are safe to show to users.

use std::fmt;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("The database file is damaged")]
    Corrupt,
    #[error("This data was created by a newer version of Shift Paste (schema {0}). Please update the app.")]
    NewerSchema(i64),
    #[error("File error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Could not read the spreadsheet: {0}")]
    Spreadsheet(String),
    #[error("Could not read the CSV file: {0}")]
    Csv(#[from] csv::Error),
    #[error("Could not write the Excel file: {0}")]
    Xlsx(#[from] rust_xlsxwriter::XlsxError),
    #[error("The settings file is invalid: {0}")]
    Settings(String),
    /// A problem with user input; the message is written for the user.
    #[error("{0}")]
    Invalid(String),
    #[error("The item no longer exists")]
    NotFound,
}

impl Error {
    pub fn invalid(msg: impl fmt::Display) -> Self {
        Error::Invalid(msg.to_string())
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
