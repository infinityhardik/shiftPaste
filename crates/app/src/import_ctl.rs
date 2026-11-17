//! Import wizard controller ("Adjust…" after a one-click import).

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;

use shiftpaste_core::import::{
    auto_mapping, drafts, read_file, Field, ImportMode, ImportPlan, Target, Workbook,
};
use shiftpaste_core::model::{Collection, Id};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::app::{group, with_app, App};
use crate::ui::{ColumnMap, ImportWindow, PreviewRow, Theme};

pub struct ImportCtl {
    pub window: ImportWindow,
    wb: Workbook,
    mapping: RefCell<Vec<Field>>,
    collections: Vec<Collection>,
}

pub fn open_wizard(app: &Rc<App>, path: PathBuf) {
    let wb = match read_file(&path) {
        Ok(wb) => wb,
        Err(e) => {
            tracing::warn!("import: {e}");
            return;
        }
    };
    let window = ImportWindow::new().expect("import window");
    window.global::<Theme>().set_dark(app.is_dark());
    let collections = app.lib.borrow().collections().unwrap_or_default();
    let sheet_names: Vec<SharedString> = wb.sheets.iter().map(|s| s.name.as_str().into()).collect();
    window.set_file_name(wb.file_name.as_str().into());
    window.set_sheets(ModelRc::new(VecModel::from(sheet_names)));
    window.set_collections(ModelRc::new(VecModel::from(
        collections
            .iter()
            .map(|c| SharedString::from(c.name.as_str()))
            .collect::<Vec<_>>(),
    )));
    let first = wb.sheets.iter().position(|s| !s.is_empty()).unwrap_or(0);
    window.set_sheet_index(first as i32);
    *app.importer.borrow_mut() = Some(ImportCtl {
        window,
        wb,
        mapping: RefCell::new(Vec::new()),
        collections,
    });
    {
        let imp = app.importer.borrow();
        let ctl = imp.as_ref().unwrap();
        ctl.load_sheet(first);
        ctl.update(app);
        wire(ctl);
        let _ = ctl.window.show();
    }
}

fn with_imp(f: impl FnOnce(&Rc<App>, &ImportCtl)) {
    with_app(|app| {
        if let Some(c) = app.importer.borrow().as_ref() {
            f(app, c);
        }
    });
}

fn wire(ctl: &ImportCtl) {
    let w = &ctl.window;
    w.on_sheet_changed(|i| {
        with_imp(|app, c| {
            c.load_sheet(i.max(0) as usize);
            c.update(app);
        })
    });
    w.on_options_changed(|| with_imp(|app, c| c.update(app)));
    w.on_column_field_changed(|col, field| {
        with_imp(|app, c| {
            let field = Field::ALL[(field.max(0) as usize).min(Field::ALL.len() - 1)];
            {
                let mut m = c.mapping.borrow_mut();
                // Only one column can be Content/Title/Tags/Notes; the old one is ignored.
                if field != Field::Ignore {
                    for f in m.iter_mut() {
                        if *f == field {
                            *f = Field::Ignore;
                        }
                    }
                }
                if let Some(slot) = m.get_mut(col as usize) {
                    *slot = field;
                }
            }
            c.update(app);
        })
    });
    w.on_cancel(|| with_app(close));
    w.on_do_import(|| with_imp(|app, c| c.run(app)));
    w.window().on_close_requested(|| {
        with_app(|app| {
            // Drop the window after the close finishes.
            let app = app.clone();
            slint::Timer::single_shot(std::time::Duration::from_millis(0), move || {
                app.importer.borrow_mut().take();
            });
        });
        slint::CloseRequestResponse::HideWindow
    });
}

fn close(app: &Rc<App>) {
    if let Some(c) = app.importer.borrow_mut().take() {
        let _ = c.window.hide();
    }
}

impl ImportCtl {
    fn load_sheet(&self, i: usize) {
        let Some(sheet) = self.wb.sheets.get(i) else {
            return;
        };
        let (has_header, mapping) = auto_mapping(sheet);
        self.window.set_has_header(has_header);
        self.window.set_new_name(sheet.name.as_str().into());
        self.window.set_target_mode(0);
        self.window.set_import_mode(0);
        let rows = sheet.rows.len();
        let cols = sheet.column_count();
        self.window.set_file_info(
            format!(
                "{} sheet{} · {} rows · {} columns",
                self.wb.sheets.len(),
                if self.wb.sheets.len() == 1 { "" } else { "s" },
                group(rows as i64),
                cols
            )
            .into(),
        );
        *self.mapping.borrow_mut() = mapping;
    }

    fn plan(&self) -> ImportPlan {
        let target = if self.window.get_target_mode() == 1 {
            let idx = self.window.get_existing_index().max(0) as usize;
            match self.collections.get(idx) {
                Some(c) => Target::Existing(c.id),
                None => Target::NewCollection(self.window.get_new_name().into()),
            }
        } else {
            Target::NewCollection(self.window.get_new_name().into())
        };
        let mode = match self.window.get_import_mode() {
            1 => ImportMode::AddAll,
            2 if matches!(target, Target::Existing(_)) => ImportMode::Replace,
            _ => ImportMode::SkipDuplicates,
        };
        ImportPlan {
            sheet: self.window.get_sheet_index().max(0) as usize,
            has_header: self.window.get_has_header(),
            mapping: self.mapping.borrow().clone(),
            target,
            mode,
        }
    }

    /// Refresh columns, preview and the summary from the current choices.
    fn update(&self, app: &App) {
        let plan = self.plan();
        let Some(sheet) = self.wb.sheets.get(plan.sheet) else {
            return;
        };
        let header = sheet.rows.first().filter(|_| plan.has_header);
        let sample_row = sheet.rows.get(usize::from(plan.has_header));
        let cols: Vec<ColumnMap> = plan
            .mapping
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let letter = column_letter(i);
                let name = match header
                    .and_then(|h| h.get(i))
                    .filter(|h| !h.trim().is_empty())
                {
                    Some(h) => format!("Column {letter} · {h}"),
                    None => format!("Column {letter}"),
                };
                ColumnMap {
                    name: name.into(),
                    sample: sample_row
                        .and_then(|r| r.get(i))
                        .map(|s| first_line(s))
                        .unwrap_or_default()
                        .into(),
                    field: Field::ALL.iter().position(|x| x == f).unwrap_or(4) as i32,
                }
            })
            .collect();
        self.window.set_columns(ModelRc::new(VecModel::from(cols)));

        match drafts(&self.wb, &plan) {
            Ok((rows, empty)) => {
                let preview: Vec<PreviewRow> = rows
                    .iter()
                    .take(50)
                    .map(|d| PreviewRow {
                        title: d.title.as_str().into(),
                        content: first_line(&d.content).into(),
                        tags: d.tags.join(", ").into(),
                        notes: first_line(&d.notes).into(),
                    })
                    .collect();
                self.window
                    .set_preview(ModelRc::new(VecModel::from(preview)));
                self.window.set_problem(SharedString::new());
                self.window.set_summary(
                    self.summary(
                        app,
                        &plan,
                        &rows.iter().map(|d| d.content.clone()).collect::<Vec<_>>(),
                        empty,
                    )
                    .into(),
                );
            }
            Err(e) => {
                self.window
                    .set_preview(ModelRc::new(VecModel::from(Vec::<PreviewRow>::new())));
                self.window.set_problem(e.to_string().into());
            }
        }
        let name_ok =
            self.window.get_target_mode() == 1 || !self.window.get_new_name().trim().is_empty();
        self.window.set_can_import(name_ok);
        if !name_ok {
            self.window
                .set_problem("Give the new collection a name.".into());
        }
    }

    /// "Will add 40 snippets to … · 2 duplicates skipped · 1 empty row".
    fn summary(&self, app: &App, plan: &ImportPlan, contents: &[String], empty: usize) -> String {
        let mut seen: HashSet<String> = HashSet::new();
        let mut replaced = 0;
        let into = match &plan.target {
            Target::NewCollection(name) => format!("a new collection “{}”", name.trim()),
            Target::Existing(id) => {
                let existing = app.lib.borrow().snippets(*id).unwrap_or_default();
                if plan.mode == ImportMode::Replace {
                    replaced = existing.len();
                } else {
                    seen.extend(existing.into_iter().map(|s| s.content.trim().to_owned()));
                }
                format!("“{}”", self.name_of(*id))
            }
        };
        let mut dupes = 0;
        let mut added = 0;
        for c in contents {
            if plan.mode != ImportMode::AddAll && !seen.insert(c.clone()) {
                dupes += 1;
            } else {
                added += 1;
            }
        }
        let mut s = format!(
            "Will add {} snippet{} to {into}",
            group(added),
            if added == 1 { "" } else { "s" }
        );
        if replaced > 0 {
            s += &format!(", replacing {replaced}");
        }
        if dupes > 0 {
            s += &format!(
                " · {dupes} duplicate{} skipped",
                if dupes == 1 { "" } else { "s" }
            );
        }
        if empty > 0 {
            s += &format!(
                " · {empty} empty row{} skipped",
                if empty == 1 { "" } else { "s" }
            );
        }
        s
    }

    fn name_of(&self, id: Id) -> String {
        self.collections
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.name.clone())
            .unwrap_or_default()
    }

    fn run(&self, app: &Rc<App>) {
        let plan = self.plan();
        let result = app.lib.borrow_mut().import(&self.wb, &[plan]);
        match result {
            Ok(_) => {
                let app2 = app.clone();
                slint::Timer::single_shot(std::time::Duration::from_millis(0), move || {
                    close(&app2);
                    crate::editor_ctl::library_changed(&app2);
                });
            }
            Err(e) => self.window.set_problem(e.to_string().into()),
        }
    }
}

fn column_letter(i: usize) -> String {
    let mut n = i + 1;
    let mut s = String::new();
    while n > 0 {
        let r = (n - 1) % 26;
        s.insert(0, (b'A' + r as u8) as char);
        n = (n - 1) / 26;
    }
    s
}

fn first_line(s: &str) -> String {
    shiftpaste_core::search::one_line(s, 120)
}

#[cfg(test)]
mod tests {
    #[test]
    fn column_letters() {
        assert_eq!(super::column_letter(0), "A");
        assert_eq!(super::column_letter(25), "Z");
        assert_eq!(super::column_letter(26), "AA");
    }
}
