//! Render Settings and the import wizard (design review).
//! xvfb-run cargo run -p shiftpaste --example settings_screens -- <out-dir>

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use shiftpaste_app::ui::{
    AppRow, ColumnMap, ImportWindow, PickRow, PreviewRow, SettingsWindow, Theme,
};
use shiftpaste_core::apps::entry_label;
use shiftpaste_core::import::{
    auto_mapping, drafts, Field, ImportMode, ImportPlan, Sheet, Target, Workbook,
};
use shiftpaste_core::presets::{presets_in, Group, Platform};
use shiftpaste_core::settings::Settings;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

fn model<T: Clone + 'static>(v: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(v))
}

fn save(path: &str, snap: &slint::SharedPixelBuffer<slint::Rgba8Pixel>) {
    let file = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), snap.width(), snap.height());
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .unwrap()
        .write_image_data(snap.as_bytes())
        .unwrap();
    println!("saved {path}");
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "screenshots".into());
    std::fs::create_dir_all(&out).unwrap();
    let settings = Settings::default();

    let s = SettingsWindow::new().unwrap();
    s.window().set_size(slint::LogicalSize::new(780.0, 600.0));
    s.set_version("2.0.0-dev".into());
    s.set_launch_at_login(true);
    s.set_shortcut_caps(model(vec!["Ctrl".into(), "⇧".into(), "V".into()]));
    s.set_pass_through_note(
        "In other apps the shortcut is passed through, so their own Ctrl+⇧+V keeps working.".into(),
    );
    s.set_hotkey_apps(model(vec![
        AppRow {
            label: "Microsoft Excel".into(),
            detail: "Popular app".into(),
        },
        AppRow {
            label: "Windows Terminal".into(),
            detail: "Popular app".into(),
        },
    ]));
    let capture: Vec<AppRow> = settings
        .privacy
        .capture
        .except
        .iter()
        .take(6)
        .map(|e| AppRow {
            label: entry_label(e).into(),
            detail: "Password manager".into(),
        })
        .collect();
    s.set_capture_apps(model(capture));
    s.set_honour_password_flags(true);
    s.set_history_summary("Now: 1,284 items, oldest from 2 months ago.".into());
    s.set_data_folder("C:\\Users\\Hardik\\AppData\\Local\\infinityhardik\\ShiftPaste\\data".into());
    s.set_page(1);
    s.show().unwrap();

    let mut picker: Vec<PickRow> = Vec::new();
    for g in [Group::Office, Group::Terminals, Group::Browsers] {
        picker.push(PickRow {
            section: true,
            label: g.label().to_uppercase().into(),
            ..Default::default()
        });
        for p in presets_in(g, Platform::Windows) {
            picker.push(PickRow {
                section: false,
                label: p.name.into(),
                detail: SharedString::new(),
                checked: p.id == "excel" || p.id == "windows-terminal",
            });
        }
    }

    // Import wizard with a realistic sheet.
    let wb = Workbook {
        file_name: "Product catalog.xlsx".into(),
        sheets: vec![Sheet {
            name: "Catalog".into(),
            rows: [
                ["Name", "Description", "Price", "Tags"],
                ["Pro 18", "18 mm 8 x 4 LL Pro", "1499", "catalog, pro"],
                ["Pro 12", "12 mm 8 x 4 LL Pro", "1299", "catalog, pro"],
                ["Lite 18", "18 mm 6 x 3 Lite", "899", "catalog"],
                [
                    "Edge band",
                    "PVC edge band 22 mm white",
                    "45",
                    "accessories",
                ],
                [
                    "Hinge soft-close",
                    "Soft-close hinge 110°, pair",
                    "320",
                    "hardware",
                ],
            ]
            .iter()
            .map(|r| r.iter().map(|c| c.to_string()).collect())
            .collect(),
        }],
    };
    let (has_header, mut mapping) = auto_mapping(&wb.sheets[0]);
    mapping[1] = Field::Content;
    mapping[2] = Field::Notes;
    let plan = ImportPlan {
        sheet: 0,
        has_header,
        mapping: mapping.clone(),
        target: Target::NewCollection("Catalog".into()),
        mode: ImportMode::SkipDuplicates,
    };
    let (rows, _) = drafts(&wb, &plan).unwrap();
    let w = ImportWindow::new().unwrap();
    w.window().set_size(slint::LogicalSize::new(920.0, 600.0));
    w.set_file_name(wb.file_name.as_str().into());
    w.set_file_info("1 sheet · 5 rows · 4 columns".into());
    w.set_sheets(model(vec!["Catalog".into()]));
    w.set_has_header(has_header);
    let cols: Vec<ColumnMap> = (0..4)
        .map(|i| ColumnMap {
            name: format!(
                "Column {} · {}",
                (b'A' + i as u8) as char,
                wb.sheets[0].rows[0][i]
            )
            .into(),
            sample: wb.sheets[0].rows[1][i].as_str().into(),
            field: Field::ALL.iter().position(|f| *f == mapping[i]).unwrap() as i32,
        })
        .collect();
    w.set_columns(model(cols));
    w.set_new_name("Catalog".into());
    w.set_collections(model(vec!["Work".into(), "Personal".into()]));
    w.set_preview(model(
        rows.iter()
            .map(|d| PreviewRow {
                title: d.title.as_str().into(),
                content: d.content.as_str().into(),
                tags: d.tags.join(", ").into(),
                notes: d.notes.as_str().into(),
            })
            .collect(),
    ));
    w.set_summary("Will add 5 snippets to a new collection “Catalog”.".into());

    let step = Rc::new(RefCell::new(0usize));
    let (sw, ww) = (s.as_weak(), w.as_weak());
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(600),
        move || {
            let (s, w) = (sw.unwrap(), ww.unwrap());
            let mut i = step.borrow_mut();
            match *i {
                0 => {
                    save(
                        &format!("{out}/settings-shortcut-light.png"),
                        &s.window().take_snapshot().unwrap(),
                    );
                    s.global::<Theme>().set_dark(true);
                    s.set_page(3);
                }
                1 => {
                    save(
                        &format!("{out}/settings-privacy-dark.png"),
                        &s.window().take_snapshot().unwrap(),
                    );
                    s.global::<Theme>().set_dark(false);
                    s.set_page(1);
                    s.set_picker_open(true);
                    s.set_picker_title("Where shouldn't the shortcut open Shift Paste?".into());
                    s.set_picker_rows(ModelRc::new(VecModel::from(picker.clone())));
                }
                2 => {
                    save(
                        &format!("{out}/settings-picker-light.png"),
                        &s.window().take_snapshot().unwrap(),
                    );
                    s.hide().unwrap();
                    w.show().unwrap();
                }
                3 => {
                    save(
                        &format!("{out}/import-wizard-light.png"),
                        &w.window().take_snapshot().unwrap(),
                    );
                    slint::quit_event_loop().unwrap();
                }
                _ => {}
            }
            *i += 1;
        },
    );
    slint::run_event_loop_until_quit().unwrap();
}
