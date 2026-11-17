//! Render the real windows with sample data and save PNGs, for design review.
//! Run (Linux, headless): xvfb-run cargo run -p shiftpaste --example screenshots -- <out-dir>

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use shiftpaste_app::ui::{MenuEntry, PopupWindow, Theme};
use shiftpaste_app::{accent_hex, demo, key_labels, rows};
use shiftpaste_core::library::Library;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

struct Shot {
    name: &'static str,
    dark: bool,
    query: &'static str,
    preview: bool,
    picks: usize,
    menu: bool,
    select: usize,
}

fn apply(ui: &PopupWindow, lib: &mut Library, now: i64, shot: &Shot) {
    ui.global::<Theme>().set_dark(shot.dark);
    ui.set_keys(key_labels());
    ui.set_query(shot.query.into());
    let results = lib.search(shot.query, 60);
    let picks: Vec<_> = results
        .hits
        .iter()
        .take(shot.picks)
        .map(|h| (h.kind, h.id))
        .collect();
    let built = rows::build(&results, now, accent_hex(shot.dark), |h| {
        picks
            .iter()
            .position(|p| *p == (h.kind, h.id))
            .map_or(0, |i| i as i32 + 1)
    });
    let selectable: Vec<usize> = built
        .targets
        .iter()
        .enumerate()
        .filter(|(_, t)| t.is_selectable())
        .map(|(i, _)| i)
        .collect();
    let current = selectable
        .get(shot.select)
        .copied()
        .map_or(-1, |i| i as i32);
    ui.set_rows(ModelRc::new(VecModel::from(built.rows)));
    ui.set_current(current);
    ui.set_picked_count(shot.picks as i32);
    ui.set_count_text(
        match results.mode {
            shiftpaste_core::search::Mode::Search => format!("{} matches", results.total),
            shiftpaste_core::search::Mode::Filtered => format!("{} snippets", results.total),
            shiftpaste_core::search::Mode::Home => {
                format!("{} items", lib.store().history_count().unwrap())
            }
        }
        .into(),
    );
    if results.hits.is_empty() {
        ui.set_empty_title(format!("No matches for “{}”", shot.query).into());
        ui.set_empty_message(
            "Letters match in order, so “1884” finds “18 mm 8 x 4”. Type @ to search a collection, # for a tag."
                .into(),
        );
    }
    ui.set_preview_open(shot.preview);
    if shot.preview {
        if let Some(rows::RowTarget::Hit(h)) = built.targets.get(current as usize) {
            let hit = &results.hits[*h];
            let item = lib.history_item(hit.id).unwrap();
            ui.set_preview_kind("History".into());
            ui.set_preview_title(hit.title.as_str().into());
            ui.set_preview_body(item.content.as_str().into());
            ui.set_preview_meta(
                format!(
                    "Copied {} · {} time(s) · Has formatting",
                    shiftpaste_core::time::time_ago(item.last_copied_at, now).to_lowercase(),
                    item.copy_count
                )
                .into(),
            );
        }
    }
    ui.set_menu_open(shot.menu);
    if shot.menu {
        ui.set_menu_title(SharedString::from("Paste 2 items as"));
        let entries: Vec<MenuEntry> = [
            ("Join", "", true),
            ("One per line", "1", false),
            ("Comma-separated", "2", false),
            ("Tab-separated (spreadsheet cells)", "3", false),
            ("Change case", "", true),
            ("UPPERCASE", "4", false),
            ("lowercase", "5", false),
            ("Title Case", "6", false),
        ]
        .into_iter()
        .map(|(label, shortcut, section)| MenuEntry {
            label: label.into(),
            shortcut: shortcut.into(),
            section,
        })
        .collect();
        ui.set_menu_items(ModelRc::new(VecModel::from(entries)));
        ui.set_menu_current(1);
    }
    let width = if shot.preview { 460.0 + 380.0 } else { 460.0 };
    ui.window().set_size(slint::LogicalSize::new(width, 540.0));
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "screenshots".into());
    std::fs::create_dir_all(&out).unwrap();
    let shots = [
        Shot {
            name: "popup-home-light",
            dark: false,
            query: "",
            preview: false,
            picks: 0,
            menu: false,
            select: 0,
        },
        Shot {
            name: "popup-search-light",
            dark: false,
            query: "inv",
            preview: false,
            picks: 0,
            menu: false,
            select: 0,
        },
        Shot {
            name: "popup-filter-light",
            dark: false,
            query: "@work",
            preview: false,
            picks: 0,
            menu: false,
            select: 1,
        },
        Shot {
            name: "popup-preview-dark",
            dark: true,
            query: "",
            preview: true,
            picks: 0,
            menu: false,
            select: 6,
        },
        Shot {
            name: "popup-multiselect-dark",
            dark: true,
            query: "",
            preview: false,
            picks: 2,
            menu: true,
            select: 1,
        },
        Shot {
            name: "popup-empty-light",
            dark: false,
            query: "zzzq",
            preview: false,
            picks: 0,
            menu: false,
            select: 0,
        },
    ];
    let (lib, now) = demo::sample_library();
    let lib = Rc::new(RefCell::new(lib));
    let ui = PopupWindow::new().unwrap();
    apply(&ui, &mut lib.borrow_mut(), now, &shots[0]);
    ui.show().unwrap();

    let step = Rc::new(RefCell::new(0usize));
    let weak = ui.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(500),
        move || {
            let ui = weak.unwrap();
            let mut i = step.borrow_mut();
            let snap = ui.window().take_snapshot().expect("snapshot");
            let path = format!("{out}/{}.png", shots[*i].name);
            save_png(&path, &snap);
            println!("saved {path} ({}x{})", snap.width(), snap.height());
            *i += 1;
            if *i == shots.len() {
                slint::quit_event_loop().unwrap();
                return;
            }
            apply(&ui, &mut lib.borrow_mut(), now, &shots[*i]);
        },
    );
    slint::run_event_loop_until_quit().unwrap();
}

fn save_png(path: &str, buf: &slint::SharedPixelBuffer<slint::Rgba8Pixel>) {
    let file = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), buf.width(), buf.height());
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .unwrap()
        .write_image_data(buf.as_bytes())
        .unwrap();
}
