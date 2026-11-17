//! Search speed check: 5,000 history items + 20,000 snippets.
//! Run with: cargo run --release -p shiftpaste-core --example bench_search
use shiftpaste_core::model::{Collection, HistoryItem, Snippet};
use shiftpaste_core::search::SearchIndex;
use std::time::Instant;

fn main() {
    let words = [
        "invoice",
        "report",
        "address",
        "bank",
        "IFSC",
        "HDFC",
        "Mumbai",
        "18 mm",
        "Pro",
        "meeting",
        "Friday",
        "quarterly",
        "client",
        "GST",
        "template",
        "École",
        "naïve",
    ];
    let text = |i: usize| {
        (0..12)
            .map(|k| words[(i * 7 + k * 3) % words.len()])
            .collect::<Vec<_>>()
            .join(" ")
            + &format!(" #{i} lorem ipsum dolor sit amet consectetur")
    };
    let mut idx = SearchIndex::default();
    for c in 1..=20 {
        idx.set_collection(&Collection {
            id: c,
            name: format!("Collection {c}"),
            sort_order: c,
            created_at: 0,
            updated_at: 0,
        });
    }
    let t0 = Instant::now();
    for i in 0..5_000 {
        idx.upsert_history(&HistoryItem {
            id: i as i64,
            content: text(i),
            last_copied_at: i as i64,
            ..Default::default()
        });
    }
    for i in 0..20_000 {
        idx.upsert_snippet(&Snippet {
            id: i as i64,
            collection_id: 1 + (i % 20) as i64,
            title: format!("Snippet {i}"),
            content: text(i + 99),
            created_at: 1,
            ..Default::default()
        });
    }
    println!("indexed {} items in {:?}", idx.len(), t0.elapsed());
    for q in [
        "",
        "i",
        "in",
        "inv",
        "invo",
        "invoice",
        "1884",
        "bank ifsc",
        "ecole",
        "@coll",
        "zzzzq",
    ] {
        let t = Instant::now();
        let mut r = idx.search(q, 50);
        for _ in 0..9 {
            r = idx.search(q, 50);
        }
        println!(
            "{q:>10}: {:>6} matches, {:>7.2} ms/search",
            r.total,
            t.elapsed().as_secs_f64() * 100.0
        );
    }
}
