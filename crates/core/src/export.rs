//! Export collections and history to CSV or Excel.
//!
//! - CSV is UTF-8 **with BOM** so Excel shows accents and non-Latin text correctly.
//! - Exporting all collections to one .xlsx (a sheet per collection) round-trips:
//!   one-click import of that file restores every collection.
//! - Excel cells hold at most 32,767 characters; longer items are shortened in
//!   .xlsx files and counted in the report (CSV keeps them in full).

use std::collections::HashSet;
use std::path::Path;

use rust_xlsxwriter::{Format, FormatAlign, Workbook, Worksheet};

use crate::library::Library;
use crate::model::{Collection, HistoryItem, Id, Snippet};
use crate::time::format_local;
use crate::{Error, Result};

/// Excel's per-cell character limit.
pub const EXCEL_CELL_LIMIT: usize = 32_767;
const SNIPPET_HEADER: [&str; 4] = ["Title", "Content", "Tags", "Notes"];
const HISTORY_HEADER: [&str; 5] = [
    "Content",
    "First copied",
    "Last copied",
    "Copy count",
    "Source app",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    Csv,
    Xlsx,
}

impl FileFormat {
    pub fn from_path(path: &Path) -> Result<FileFormat> {
        match path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .as_deref()
        {
            Some("csv") => Ok(FileFormat::Csv),
            Some("xlsx") => Ok(FileFormat::Xlsx),
            _ => Err(Error::invalid(
                "Choose a file name ending in .csv or .xlsx.",
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExportReport {
    pub rows: usize,
    /// Items shortened to fit Excel's cell limit.
    pub shortened: usize,
}

impl Library {
    /// Export one collection; the format follows the file extension.
    pub fn export_collection(&self, id: Id, path: &Path) -> Result<ExportReport> {
        let c = self.store().collection(id)?;
        let snippets = self.snippets(id)?;
        match FileFormat::from_path(path)? {
            FileFormat::Csv => write_csv(path, &SNIPPET_HEADER, snippets.iter().map(snippet_row)),
            FileFormat::Xlsx => {
                let mut wb = Workbook::new();
                let mut report = ExportReport::default();
                add_snippet_sheet(&mut wb, &c.name, &snippets, &mut report)?;
                wb.save(path)?;
                Ok(report)
            }
        }
    }

    /// Every collection into one .xlsx, a sheet per collection (a full backup).
    pub fn export_all(&self, path: &Path) -> Result<ExportReport> {
        let mut wb = Workbook::new();
        let mut report = ExportReport::default();
        let mut used = HashSet::new();
        let collections: Vec<Collection> = self.collections()?;
        if collections.is_empty() {
            return Err(Error::invalid("There are no collections to export yet."));
        }
        for c in collections {
            let snippets = self.snippets(c.id)?;
            let name = sheet_name(&c.name, &mut used);
            add_snippet_sheet(&mut wb, &name, &snippets, &mut report)?;
        }
        wb.save(path)?;
        Ok(report)
    }

    /// Clipboard history, newest first.
    pub fn export_history(&self, path: &Path) -> Result<ExportReport> {
        let items = self.history(usize::MAX >> 1, 0)?;
        match FileFormat::from_path(path)? {
            FileFormat::Csv => write_csv(path, &HISTORY_HEADER, items.iter().map(history_row)),
            FileFormat::Xlsx => {
                let mut wb = Workbook::new();
                let ws = wb.add_worksheet();
                ws.set_name("History")?;
                let mut report = ExportReport::default();
                write_table(
                    ws,
                    &HISTORY_HEADER,
                    items.iter().map(history_row),
                    &mut report,
                )?;
                for (col, width) in [(0, 70), (1, 18), (2, 18), (3, 11), (4, 22)] {
                    ws.set_column_width(col, width)?;
                }
                wb.save(path)?;
                Ok(report)
            }
        }
    }
}

fn snippet_row(s: &Snippet) -> Vec<String> {
    vec![
        s.title.clone(),
        s.content.clone(),
        s.tags.join(", "),
        s.notes.clone(),
    ]
}

fn history_row(h: &HistoryItem) -> Vec<String> {
    vec![
        h.content.clone(),
        format_local(h.first_copied_at),
        format_local(h.last_copied_at),
        h.copy_count.to_string(),
        h.source_app.clone().unwrap_or_default(),
    ]
}

fn write_csv(
    path: &Path,
    header: &[&str],
    rows: impl Iterator<Item = Vec<String>>,
) -> Result<ExportReport> {
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    let mut report = ExportReport::default();
    {
        let mut w = csv::WriterBuilder::new()
            .terminator(csv::Terminator::CRLF)
            .from_writer(&mut bytes);
        w.write_record(header)?;
        for row in rows {
            w.write_record(&row)?;
            report.rows += 1;
        }
        w.flush()?;
    }
    write_atomic(path, &bytes)?;
    Ok(report)
}

/// Write to a temp file then rename, so a failed export never leaves a half file.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".partial");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn add_snippet_sheet(
    wb: &mut Workbook,
    name: &str,
    snippets: &[Snippet],
    report: &mut ExportReport,
) -> Result<()> {
    let ws = wb.add_worksheet();
    ws.set_name(sheet_name(name, &mut HashSet::new()))?;
    write_table(
        ws,
        &SNIPPET_HEADER,
        snippets.iter().map(snippet_row),
        report,
    )?;
    for (col, width) in [(0, 28), (1, 60), (2, 20), (3, 36)] {
        ws.set_column_width(col, width)?;
    }
    Ok(())
}

fn write_table(
    ws: &mut Worksheet,
    header: &[&str],
    rows: impl Iterator<Item = Vec<String>>,
    report: &mut ExportReport,
) -> Result<()> {
    let bold = Format::new().set_bold();
    let cell = Format::new().set_text_wrap().set_align(FormatAlign::Top);
    for (col, h) in header.iter().enumerate() {
        ws.write_string_with_format(0, col as u16, *h, &bold)?;
    }
    let mut last_row = 0u32;
    for (i, row) in rows.enumerate() {
        last_row = i as u32 + 1;
        for (col, value) in row.iter().enumerate() {
            let fitted = fit_cell(value);
            if fitted.len() < value.len() {
                report.shortened += 1;
            }
            ws.write_string_with_format(last_row, col as u16, fitted, &cell)?;
        }
        report.rows += 1;
    }
    ws.set_freeze_panes(1, 0)?;
    ws.autofilter(0, 0, last_row, header.len() as u16 - 1)?;
    Ok(())
}

fn fit_cell(value: &str) -> &str {
    match value.char_indices().nth(EXCEL_CELL_LIMIT) {
        Some((cut, _)) => &value[..cut],
        None => value,
    }
}

/// Excel sheet names: ≤ 31 chars, none of `[]:*?/\`, unique (case-insensitive).
fn sheet_name(name: &str, used: &mut HashSet<String>) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| !matches!(c, '[' | ']' | ':' | '*' | '?' | '/' | '\\'))
        .collect();
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let cleaned = cleaned.trim_matches('\'');
    let base: String = if cleaned.is_empty() { "Sheet" } else { cleaned }
        .chars()
        .take(31)
        .collect();
    let mut candidate = base.clone();
    let mut n = 2;
    while !used.insert(candidate.to_lowercase()) {
        let suffix = format!(" ({n})");
        let keep = 31 - suffix.chars().count();
        candidate = base.chars().take(keep).collect::<String>() + &suffix;
        n += 1;
    }
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::{one_click_plans, read_file};
    use crate::library::{HistoryLimits, SnippetDraft};
    use crate::model::NewClip;

    fn sample() -> Library {
        let mut lib = Library::in_memory().unwrap();
        let work = lib.create_collection("Work / Tax: 2026").unwrap();
        lib.create_snippet(
            work.id,
            SnippetDraft {
                title: "Bank IFSC".into(),
                content: "HDFC0001234".into(),
                tags: vec!["work".into(), "bank".into()],
                notes: "main branch".into(),
            },
        )
        .unwrap();
        lib.create_snippet(
            work.id,
            SnippetDraft::with_content("Line 1\nLine 2, with comma"),
        )
        .unwrap();
        let personal = lib.create_collection("Personal").unwrap();
        lib.create_snippet(personal.id, SnippetDraft::with_content("École · नमस्ते"))
            .unwrap();
        lib
    }

    #[test]
    fn csv_has_bom_and_round_trips() {
        let lib = sample();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("work.csv");
        let id = lib.collections().unwrap()[0].id;
        let report = lib.export_collection(id, &path).unwrap();
        assert_eq!(report.rows, 2);
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[..3], &[0xEF, 0xBB, 0xBF]);

        let wb = read_file(&path).unwrap();
        let mut fresh = Library::in_memory().unwrap();
        let r = fresh.import(&wb, &one_click_plans(&wb)).unwrap();
        let snippets = fresh.snippets(r.created[0].0).unwrap();
        assert_eq!(snippets.len(), 2);
        assert_eq!(snippets[0].title, "Bank IFSC");
        assert_eq!(snippets[0].tags, vec!["work", "bank"]);
        assert_eq!(snippets[0].notes, "main branch");
        assert_eq!(snippets[1].content, "Line 1\nLine 2, with comma");
    }

    #[test]
    fn export_all_round_trips_every_collection() {
        let lib = sample();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("backup.xlsx");
        assert_eq!(lib.export_all(&path).unwrap().rows, 3);

        let wb = read_file(&path).unwrap();
        let mut fresh = Library::in_memory().unwrap();
        let r = fresh.import(&wb, &one_click_plans(&wb)).unwrap();
        let names: Vec<_> = r.created.iter().map(|(_, n)| n.as_str()).collect();
        assert_eq!(names, ["Work Tax 2026", "Personal"]);
        let personal = fresh.snippets(r.created[1].0).unwrap();
        assert_eq!(personal[0].content, "École · नमस्ते");
        assert_eq!(
            fresh.snippets(r.created[0].0).unwrap()[0].tags,
            vec!["work", "bank"]
        );
    }

    #[test]
    fn history_export_and_long_cells() {
        let mut lib = Library::in_memory().unwrap();
        let long = "x".repeat(EXCEL_CELL_LIMIT + 10);
        for text in ["first", long.as_str()] {
            lib.record_copy(
                &NewClip {
                    text: text.into(),
                    source_app: Some("notepad.exe".into()),
                    ..Default::default()
                },
                HistoryLimits::default(),
            )
            .unwrap();
        }
        let dir = tempfile::tempdir().unwrap();
        let xlsx = dir.path().join("history.xlsx");
        let report = lib.export_history(&xlsx).unwrap();
        assert_eq!((report.rows, report.shortened), (2, 1));
        let csv = dir.path().join("history.csv");
        let report = lib.export_history(&csv).unwrap();
        assert_eq!((report.rows, report.shortened), (2, 0));
        assert!(std::fs::read_to_string(&csv)
            .unwrap()
            .contains("notepad.exe"));
        assert!(matches!(
            lib.export_history(&dir.path().join("h.txt")),
            Err(Error::Invalid(_))
        ));
    }

    #[test]
    fn sheet_names_are_valid_and_unique() {
        let mut used = HashSet::new();
        assert_eq!(sheet_name("a/b:c", &mut used), "abc");
        assert_eq!(sheet_name("ABC", &mut used), "ABC (2)");
        let long = "x".repeat(40);
        let n1 = sheet_name(&long, &mut used);
        let n2 = sheet_name(&long, &mut used);
        assert_eq!(n1.chars().count(), 31);
        assert!(n2.ends_with(" (2)") && n2.chars().count() == 31);
        assert_eq!(sheet_name(" [] ", &mut used), "Sheet");
    }
}
