//! Fill a database with realistic volumes (for memory/speed checks).
//! cargo run --release -p shiftpaste-core --example fill_db -- <db-path> [history] [snippets]
use shiftpaste_core::library::{HistoryLimits, Library, SnippetDraft};
use shiftpaste_core::model::NewClip;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("db path");
    let history: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(5_000);
    let snippets: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(2_000);
    let (mut lib, _) = Library::open(std::path::Path::new(&path)).unwrap();
    let words = [
        "invoice", "report", "address", "bank", "meeting", "client", "GST", "template", "Mumbai",
        "Pro", "draft", "École",
    ];
    let text = |i: usize, n: usize| {
        (0..n)
            .map(|k| words[(i * 7 + k * 3) % words.len()])
            .collect::<Vec<_>>()
            .join(" ")
            + &format!(" #{i}")
    };
    for i in 0..history {
        lib.record_copy(
            &NewClip {
                text: text(i, 8 + i % 30),
                ..Default::default()
            },
            HistoryLimits::default(),
        )
        .unwrap();
    }
    let per = (snippets / 10).max(1);
    for c in 0..10 {
        let col = lib
            .create_collection(&format!("Collection {}", c + 1))
            .unwrap();
        for i in 0..per {
            lib.create_snippet(
                col.id,
                SnippetDraft {
                    title: format!("Snippet {c}-{i}"),
                    content: text(i + c, 12),
                    ..Default::default()
                },
            )
            .unwrap();
        }
    }
    println!(
        "filled {path}: {} history, {} snippets",
        lib.store().history_count().unwrap(),
        per * 10
    );
}
