//! Render the editor window with sample data (design review).
//! xvfb-run cargo run -p shiftpaste --example editor_screens -- <out-dir>

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use shiftpaste_app::demo;
use shiftpaste_app::ui::{CollectionItem, EditorWindow, ListItem, Theme};
use shiftpaste_core::library::Library;
use shiftpaste_core::search::one_line;
use shiftpaste_core::time::{format_local, time_ago};
use slint::{ComponentHandle, ModelRc, VecModel};

fn collections(lib: &Library) -> Vec<CollectionItem> {
    let counts = lib.store().snippet_counts().unwrap();
    lib.collections()
        .unwrap()
        .into_iter()
        .map(|c| CollectionItem {
            favorites: c.name == "Favorites",
            count: *counts.get(&c.id).unwrap_or(&0) as i32,
            name: c.name.into(),
        })
        .collect()
}

fn show_collection(ui: &EditorWindow, lib: &Library, idx: usize, sel: usize) {
    let cols = lib.collections().unwrap();
    let c = &cols[idx];
    let snippets = lib.snippets(c.id).unwrap();
    ui.set_view(0);
    ui.set_collections(ModelRc::new(VecModel::from(collections(lib))));
    ui.set_current_collection(idx as i32);
    ui.set_list_title(c.name.as_str().into());
    ui.set_list_subtitle(format!("{} snippets", snippets.len()).into());
    let items: Vec<ListItem> = snippets
        .iter()
        .map(|s| ListItem {
            title: s.display_title().into(),
            preview: if s.title.is_empty() {
                String::new()
            } else {
                one_line(&s.content, 120)
            }
            .into(),
            meta: Default::default(),
            starred: s.is_starred(),
            rich: false,
        })
        .collect();
    ui.set_items(ModelRc::new(VecModel::from(items)));
    ui.set_current_item(sel as i32);
    let s = &snippets[sel];
    ui.set_has_detail(true);
    ui.set_detail_title(s.title.as_str().into());
    ui.set_detail_content(s.content.as_str().into());
    ui.set_detail_notes("Registered office. Use for invoices and GST filings.".into());
    let tags: Vec<slint::SharedString> = s.tags.iter().map(|t| t.as_str().into()).collect();
    ui.set_detail_tags(ModelRc::new(VecModel::from(tags)));
    ui.set_detail_starred(s.is_starred());
    ui.set_detail_meta(
        format!(
            "Created {} · Pasted 4 times, last yesterday",
            format_local(s.created_at)
        )
        .into(),
    );
    ui.set_saved_text("Saved".into());
    ui.set_can_undo(true);
    ui.set_undo_label("Edit snippet".into());
}

fn show_history(ui: &EditorWindow, lib: &Library, now: i64) {
    let hist = lib.history(50, 0).unwrap();
    ui.set_view(1);
    ui.set_list_title("Clipboard history".into());
    ui.set_list_subtitle(format!("{} items · kept for 90 days, up to 500", hist.len()).into());
    let items: Vec<ListItem> = hist
        .iter()
        .map(|h| ListItem {
            title: shiftpaste_core::model::first_line(&h.content).into(),
            preview: one_line(
                h.content
                    .trim_start()
                    .strip_prefix(shiftpaste_core::model::first_line(&h.content))
                    .unwrap_or(""),
                120,
            )
            .into(),
            meta: time_ago(h.last_copied_at, now).into(),
            starred: false,
            rich: h.has_formatting(),
        })
        .collect();
    ui.set_items(ModelRc::new(VecModel::from(items)));
    ui.set_current_item(1);
    ui.set_has_detail(true);
    ui.set_detail_content(hist[1].content.as_str().into());
    ui.set_detail_meta(
        format!(
            "Copied {} · {} time(s)",
            time_ago(hist[1].last_copied_at, now).to_lowercase(),
            hist[1].copy_count
        )
        .into(),
    );
    ui.set_saved_text("".into());
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "screenshots".into());
    std::fs::create_dir_all(&out).unwrap();
    let (lib, now) = demo::sample_library();
    let lib = Rc::new(RefCell::new(lib));
    let ui = EditorWindow::new().unwrap();
    ui.window().set_size(slint::LogicalSize::new(1080.0, 680.0));
    show_collection(&ui, &lib.borrow(), 0, 2);
    ui.show().unwrap();
    let names = [
        "editor-collection-light",
        "editor-history-dark",
        "editor-welcome-light",
    ];
    let step = Rc::new(RefCell::new(0usize));
    let weak = ui.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(600),
        move || {
            let ui = weak.unwrap();
            let mut i = step.borrow_mut();
            let snap = ui.window().take_snapshot().unwrap();
            let path = format!("{out}/{}.png", names[*i]);
            let file = std::fs::File::create(&path).unwrap();
            let mut enc =
                png::Encoder::new(std::io::BufWriter::new(file), snap.width(), snap.height());
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            enc.write_header()
                .unwrap()
                .write_image_data(snap.as_bytes())
                .unwrap();
            println!("saved {path}");
            *i += 1;
            match *i {
                1 => {
                    ui.global::<Theme>().set_dark(true);
                    show_history(&ui, &lib.borrow(), now);
                }
                2 => {
                    ui.global::<Theme>().set_dark(false);
                    show_collection(&ui, &lib.borrow(), 0, 2);
                    ui.set_first_run(true);
                }
                _ => slint::quit_event_loop().unwrap(),
            }
        },
    );
    slint::run_event_loop_until_quit().unwrap();
}
