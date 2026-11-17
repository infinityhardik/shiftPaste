//! Excel / CSV import: one-click, or a wizard plan (sheet, header, column
//! mapping, target, mode). An import is a single undoable change.

use std::collections::HashSet;
use std::path::Path;

use calamine::{open_workbook_auto, Data, Reader};

use crate::library::{apply_draft, new_collection, Applied, Library, Op, SnippetDraft};
use crate::model::{parse_tags, Id, Snippet};
use crate::{db, Error, Result};

/// A file read into memory: one or more sheets of text cells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workbook {
    /// File name shown in labels, e.g. "Work.xlsx".
    pub file_name: String,
    pub sheets: Vec<Sheet>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sheet {
    pub name: String,
    pub rows: Vec<Vec<String>>,
}

impl Sheet {
    pub fn is_empty(&self) -> bool {
        self.rows
            .iter()
            .all(|r| r.iter().all(|c| c.trim().is_empty()))
    }

    pub fn column_count(&self) -> usize {
        self.rows.iter().map(Vec::len).max().unwrap_or(0)
    }
}

/// What a column becomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
    Content,
    Title,
    Tags,
    Notes,
    Ignore,
}

impl Field {
    pub const ALL: [Field; 5] = [
        Field::Content,
        Field::Title,
        Field::Tags,
        Field::Notes,
        Field::Ignore,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Field::Content => "Content",
            Field::Title => "Title",
            Field::Tags => "Tags",
            Field::Notes => "Notes",
            Field::Ignore => "Ignore",
        }
    }

    fn from_header(cell: &str) -> Option<Field> {
        match cell.trim().to_lowercase().as_str() {
            "content" | "text" | "snippet" | "value" | "clipboard" => Some(Field::Content),
            "title" | "name" | "label" => Some(Field::Title),
            "tags" | "tag" => Some(Field::Tags),
            "notes" | "note" | "comment" | "comments" | "description" => Some(Field::Notes),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    NewCollection(String),
    Existing(Id),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ImportMode {
    /// Skip rows whose content is already in the collection (or earlier in the file).
    #[default]
    SkipDuplicates,
    AddAll,
    /// Remove the collection's current snippets first.
    Replace,
}

/// How to import one sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportPlan {
    pub sheet: usize,
    pub has_header: bool,
    /// One entry per column.
    pub mapping: Vec<Field>,
    pub target: Target,
    pub mode: ImportMode,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportReport {
    /// Collections created by the import.
    pub created: Vec<(Id, String)>,
    pub added: usize,
    pub skipped_duplicates: usize,
    pub skipped_empty: usize,
    pub replaced: usize,
}

// ------------------------------------------------------------------- reading

/// Read `.xlsx/.xlsm/.xlsb/.xls/.ods` or `.csv/.tsv/.txt`.
pub fn read_file(path: &Path) -> Result<Workbook> {
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let sheets = match ext.as_str() {
        "xlsx" | "xlsm" | "xlsb" | "xls" | "ods" => read_spreadsheet(path)?,
        "csv" | "tsv" | "txt" => {
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Imported".into());
            vec![Sheet {
                name: stem,
                rows: read_csv(&std::fs::read(path)?, ext == "tsv")?,
            }]
        }
        _ => {
            return Err(Error::invalid(
                "This file type isn't supported. Use .xlsx, .xls, .ods, .csv or .tsv.",
            ))
        }
    };
    Ok(Workbook { file_name, sheets })
}

fn read_spreadsheet(path: &Path) -> Result<Vec<Sheet>> {
    let mut wb = open_workbook_auto(path).map_err(|e| Error::Spreadsheet(e.to_string()))?;
    let mut sheets = Vec::new();
    for name in wb.sheet_names() {
        let range = wb
            .worksheet_range(&name)
            .map_err(|e| Error::Spreadsheet(e.to_string()))?;
        let rows = range
            .rows()
            .map(|row| row.iter().map(cell_text).collect())
            .collect();
        sheets.push(Sheet {
            name,
            rows: trim_rows(rows),
        });
    }
    Ok(sheets)
}

fn cell_text(cell: &Data) -> String {
    match cell {
        Data::Empty | Data::Error(_) => String::new(),
        Data::String(s) | Data::DateTimeIso(s) | Data::DurationIso(s) => s.clone(),
        Data::Int(i) => i.to_string(),
        Data::Float(f) => format_number(*f),
        Data::Bool(b) => if *b { "TRUE" } else { "FALSE" }.into(),
        Data::DateTime(d) => match d.as_datetime() {
            Some(dt) if dt.time() == chrono::NaiveTime::MIN => dt.format("%Y-%m-%d").to_string(),
            Some(dt) => dt.format("%Y-%m-%d %H:%M").to_string(),
            None => d.to_string(),
        },
    }
}

/// Whole numbers without ".0"; others in shortest round-trip form.
fn format_number(f: f64) -> String {
    if f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{}", f as i64)
    } else {
        format!("{f}")
    }
}

/// Decode (BOM → UTF-8/16; else UTF-8; else Windows-1252), sniff the delimiter, parse.
fn read_csv(bytes: &[u8], tab: bool) -> Result<Vec<Vec<String>>> {
    let text = decode(bytes);
    let delimiter = if tab { b'\t' } else { sniff_delimiter(&text) };
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter)
        .from_reader(text.as_bytes());
    let mut rows = Vec::new();
    for record in reader.records() {
        rows.push(record?.iter().map(str::to_owned).collect());
    }
    Ok(trim_rows(rows))
}

fn decode(bytes: &[u8]) -> String {
    if let Some((enc, bom_len)) = encoding_rs::Encoding::for_bom(bytes) {
        return enc
            .decode_without_bom_handling(&bytes[bom_len..])
            .0
            .into_owned();
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_owned(),
        Err(_) => encoding_rs::WINDOWS_1252.decode(bytes).0.into_owned(),
    }
}

/// Pick `,` `;` or tab: the one that appears the same number of times on the
/// most of the first lines (quotes respected). Ties prefer comma.
fn sniff_delimiter(text: &str) -> u8 {
    let lines: Vec<&str> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .take(20)
        .collect();
    let mut best = (b',', 0usize);
    for cand in *b",;\t" {
        let counts: Vec<usize> = lines
            .iter()
            .map(|l| count_outside_quotes(l, cand))
            .collect();
        let Some(&first) = counts.iter().find(|&&c| c > 0) else {
            continue;
        };
        let consistent = counts.iter().filter(|&&c| c == first).count();
        if consistent > best.1 {
            best = (cand, consistent);
        }
    }
    best.0
}

fn count_outside_quotes(line: &str, delim: u8) -> usize {
    let mut in_quotes = false;
    line.bytes()
        .filter(|&b| {
            if b == b'"' {
                in_quotes = !in_quotes;
            }
            b == delim && !in_quotes
        })
        .count()
}

/// Drop trailing empty rows and trailing empty cells.
fn trim_rows(mut rows: Vec<Vec<String>>) -> Vec<Vec<String>> {
    for row in &mut rows {
        while row.last().is_some_and(|c| c.trim().is_empty()) {
            row.pop();
        }
    }
    while rows.last().is_some_and(|r| r.is_empty()) {
        rows.pop();
    }
    rows
}

// -------------------------------------------------------------------- planning

/// Guess the header and column mapping for a sheet.
pub fn auto_mapping(sheet: &Sheet) -> (bool, Vec<Field>) {
    let cols = sheet.column_count().max(1);
    let header: Vec<Option<Field>> = match sheet.rows.first() {
        Some(row) => (0..cols)
            .map(|i| row.get(i).and_then(|c| Field::from_header(c)))
            .collect(),
        None => vec![None; cols],
    };
    let has_header = header.iter().any(Option::is_some);
    let mut mapping = vec![Field::Ignore; cols];
    if has_header {
        let mut used = HashSet::new();
        for (i, f) in header.iter().enumerate() {
            if let Some(f) = f {
                if used.insert(*f) {
                    mapping[i] = *f;
                }
            }
        }
        if !used.contains(&Field::Content) {
            // e.g. "Name | Phone": the first unnamed column holds the content.
            let i = header.iter().position(Option::is_none).unwrap_or(0);
            mapping[i] = Field::Content;
        }
    } else {
        mapping[0] = Field::Content;
    }
    (has_header, mapping)
}

/// One-click import: every non-empty sheet becomes a new collection.
pub fn one_click_plans(wb: &Workbook) -> Vec<ImportPlan> {
    wb.sheets
        .iter()
        .enumerate()
        .filter(|(_, s)| !s.is_empty())
        .map(|(i, sheet)| {
            let (has_header, mapping) = auto_mapping(sheet);
            ImportPlan {
                sheet: i,
                has_header,
                mapping,
                target: Target::NewCollection(sheet.name.clone()),
                mode: ImportMode::SkipDuplicates,
            }
        })
        .collect()
}

/// The snippets a plan would create (rows without content are left out),
/// plus how many rows were empty.
pub fn drafts(wb: &Workbook, plan: &ImportPlan) -> Result<(Vec<SnippetDraft>, usize)> {
    let sheet = wb
        .sheets
        .get(plan.sheet)
        .ok_or_else(|| Error::invalid("That sheet doesn't exist."))?;
    if !plan.mapping.contains(&Field::Content) {
        return Err(Error::invalid("Choose which column holds the Content."));
    }
    let col = |f: Field| plan.mapping.iter().position(|&m| m == f);
    let (content, title, tags, notes) = (
        col(Field::Content),
        col(Field::Title),
        col(Field::Tags),
        col(Field::Notes),
    );
    let get = |row: &Vec<String>, i: Option<usize>| {
        i.and_then(|i| row.get(i))
            .map(|s| s.trim().to_owned())
            .unwrap_or_default()
    };
    let mut out = Vec::new();
    let mut empty = 0;
    for row in sheet.rows.iter().skip(usize::from(plan.has_header)) {
        let text = get(row, content);
        if text.is_empty() {
            empty += 1;
            continue;
        }
        out.push(SnippetDraft {
            content: text,
            title: get(row, title),
            tags: parse_tags(&get(row, tags)),
            notes: get(row, notes),
        });
    }
    Ok((out, empty))
}

// ------------------------------------------------------------------- applying

impl Library {
    /// Run import plans as one undoable change.
    pub fn import(&mut self, wb: &Workbook, plans: &[ImportPlan]) -> Result<ImportReport> {
        let mut prepared = Vec::new();
        for plan in plans {
            prepared.push((plan, drafts(wb, plan)?));
        }
        self.change(format!("Import “{}”", wb.file_name), None, |tx, now| {
            let mut report = ImportReport::default();
            let (mut redo, mut undo) = (Vec::new(), Vec::new());
            for (plan, (rows, empty)) in prepared {
                report.skipped_empty += empty;
                let collection_id = match &plan.target {
                    Target::NewCollection(name) => {
                        let c = new_collection(tx, name, now)?;
                        report.created.push((c.id, c.name.clone()));
                        undo.push(Op::DeleteCollection(c.id));
                        redo.push(Op::PutCollection(c.clone()));
                        c.id
                    }
                    Target::Existing(id) => db::get_collection(tx, *id)?.ok_or(Error::NotFound)?.id,
                };
                let mut seen: HashSet<String> = HashSet::new();
                for old in db::snippets_in(tx, collection_id)? {
                    if plan.mode == ImportMode::Replace {
                        db::delete_snippet(tx, old.id)?;
                        redo.push(Op::DeleteSnippet(old.id));
                        undo.push(Op::PutSnippet(old));
                        report.replaced += 1;
                    } else {
                        seen.insert(old.content.trim().to_owned());
                    }
                }
                let mut order = db::next_snippet_order(tx, collection_id)?;
                for draft in rows {
                    if plan.mode != ImportMode::AddAll && !seen.insert(draft.content.clone()) {
                        report.skipped_duplicates += 1;
                        continue;
                    }
                    let mut s = Snippet {
                        collection_id,
                        sort_order: order,
                        created_at: now,
                        updated_at: now,
                        ..Default::default()
                    };
                    order += 1;
                    apply_draft(&mut s, draft);
                    s.id = db::insert_snippet(tx, &s)?;
                    undo.push(Op::DeleteSnippet(s.id));
                    redo.push(Op::PutSnippet(s));
                    report.added += 1;
                }
            }
            // Undo runs in the reverse order of what was done.
            undo.reverse();
            Ok(Applied {
                redo,
                undo,
                value: report,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet(rows: &[&[&str]]) -> Sheet {
        Sheet {
            name: "Work".into(),
            rows: rows
                .iter()
                .map(|r| r.iter().map(|c| c.to_string()).collect())
                .collect(),
        }
    }

    #[test]
    fn header_detection_and_mapping() {
        // The 1.x sample files: Content | Timestamp | Notes.
        let s = sheet(&[&["Content", "Timestamp", "Notes"], &["Abhishek", "", ""]]);
        assert_eq!(
            auto_mapping(&s),
            (true, vec![Field::Content, Field::Ignore, Field::Notes])
        );
        // No header: column A is the content.
        let s = sheet(&[&["HDFC0001234", "x"], &["27AAAPL1234C1Z5"]]);
        assert_eq!(
            auto_mapping(&s),
            (false, vec![Field::Content, Field::Ignore])
        );
        // Header without a content column: first unnamed column is content.
        let s = sheet(&[&["Name", "Phone"], &["Home", "12345"]]);
        assert_eq!(auto_mapping(&s), (true, vec![Field::Title, Field::Content]));
    }

    #[test]
    fn csv_decoding_and_delimiters() {
        let rows = read_csv(
            "Title;Content\nBank;HDFC 1;2\n\"A;B\";x\n".as_bytes(),
            false,
        )
        .unwrap();
        assert_eq!(rows[1], vec!["Bank", "HDFC 1", "2"]);
        assert_eq!(rows[2], vec!["A;B", "x"]);
        // UTF-8 BOM and multi-line quoted cells.
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend("Content\n\"line 1\nline 2\"\nÉcole\n".as_bytes());
        let rows = read_csv(&bytes, false).unwrap();
        assert_eq!(rows[1][0], "line 1\nline 2");
        assert_eq!(rows[2][0], "École");
        // Windows-1252 bytes (Excel's "CSV" on Western Windows).
        let rows = read_csv(&[0x43, 0x61, 0x66, 0xE9, b'\n'], false).unwrap();
        assert_eq!(rows[0][0], "Café");
        // UTF-16 LE with BOM ("Unicode text" from Excel).
        let mut utf16 = vec![0xFF, 0xFE];
        for u in "A\tB\n".encode_utf16() {
            utf16.extend(u.to_le_bytes());
        }
        assert_eq!(read_csv(&utf16, true).unwrap()[0], vec!["A", "B"]);
    }

    #[test]
    fn numbers_and_dates_read_cleanly() {
        assert_eq!(format_number(1234.0), "1234");
        assert_eq!(format_number(0.1), "0.1");
        assert_eq!(cell_text(&Data::Bool(true)), "TRUE");
        assert_eq!(cell_text(&Data::Empty), "");
    }

    #[test]
    fn one_click_import_skips_duplicates_and_empties_and_undoes() {
        let wb = Workbook {
            file_name: "Work.xlsx".into(),
            sheets: vec![
                sheet(&[
                    &["Title", "Content", "Tags"],
                    &["Bank", "HDFC0001234", "work, bank"],
                    &["", "   ", ""],
                    &["Dup", "HDFC0001234", ""],
                    &["GST", "27AAAPL1234C1Z5", "#tax"],
                ]),
                Sheet {
                    name: "Empty".into(),
                    rows: vec![vec!["".into()]],
                },
            ],
        };
        let plans = one_click_plans(&wb);
        assert_eq!(plans.len(), 1, "empty sheets are skipped");
        let mut lib = Library::in_memory().unwrap();
        let report = lib.import(&wb, &plans).unwrap();
        assert_eq!(report.added, 2);
        assert_eq!(report.skipped_duplicates, 1);
        assert_eq!(report.skipped_empty, 1);
        let (cid, name) = report.created[0].clone();
        assert_eq!(name, "Work");
        let snippets = lib.snippets(cid).unwrap();
        assert_eq!(snippets[0].tags, vec!["work", "bank"]);
        assert_eq!(snippets[1].tags, vec!["tax"]);
        assert_eq!(lib.search("#tax", 10).hits.len(), 1);

        // Importing again into a new collection gets a unique name.
        let again = lib.import(&wb, &plans).unwrap();
        assert_eq!(again.created[0].1, "Work (2)");

        lib.undo().unwrap();
        lib.undo().unwrap();
        assert!(lib.collections().unwrap().is_empty());
        assert!(lib.search("hdfc", 10).hits.is_empty());
    }

    #[test]
    fn replace_mode_is_undoable() {
        let mut lib = Library::in_memory().unwrap();
        let c = lib.create_collection("Work").unwrap();
        lib.create_snippet(c.id, SnippetDraft::with_content("old"))
            .unwrap();
        let wb = Workbook {
            file_name: "new.csv".into(),
            sheets: vec![sheet(&[&["new 1"], &["new 2"]])],
        };
        let plan = ImportPlan {
            sheet: 0,
            has_header: false,
            mapping: vec![Field::Content],
            target: Target::Existing(c.id),
            mode: ImportMode::Replace,
        };
        let report = lib.import(&wb, &[plan]).unwrap();
        assert_eq!((report.added, report.replaced), (2, 1));
        assert_eq!(lib.snippets(c.id).unwrap().len(), 2);
        lib.undo().unwrap();
        let left: Vec<_> = lib
            .snippets(c.id)
            .unwrap()
            .into_iter()
            .map(|s| s.content)
            .collect();
        assert_eq!(left, ["old"]);
    }

    #[test]
    fn plan_without_content_is_rejected() {
        let wb = Workbook {
            file_name: "x.csv".into(),
            sheets: vec![sheet(&[&["a"]])],
        };
        let plan = ImportPlan {
            sheet: 0,
            has_header: false,
            mapping: vec![Field::Ignore],
            target: Target::NewCollection("X".into()),
            mode: ImportMode::AddAll,
        };
        assert!(matches!(drafts(&wb, &plan), Err(Error::Invalid(_))));
    }
}
