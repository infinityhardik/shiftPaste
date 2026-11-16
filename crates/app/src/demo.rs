//! Realistic sample data for screenshots, demos and UI tests.

use shiftpaste_core::library::{HistoryLimits, Library, SnippetDraft};
use shiftpaste_core::model::NewClip;
use shiftpaste_core::time::{DAY, HOUR, MINUTE};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

/// A library filled with sample history, collections and favorites.
/// Returns the library and the "now" timestamp used for relative times.
pub fn sample_library() -> (Library, i64) {
    let now = shiftpaste_core::time::now_ms();
    let clock = Arc::new(AtomicI64::new(now - 30 * DAY));
    let c2 = clock.clone();
    let mut lib = Library::in_memory()
        .expect("in-memory library")
        .with_clock(move || c2.load(Ordering::SeqCst));
    let at = |t: i64| clock.store(t, Ordering::SeqCst);

    at(now - 20 * DAY);
    let work = lib.create_collection("Work").unwrap();
    let personal = lib.create_collection("Personal").unwrap();
    let templates = lib.create_collection("Email templates").unwrap();
    let snippets: [(i64, &str, &str, &[&str], bool); 9] = [
        (work.id, "Bank IFSC", "HDFC0001234", &["bank"], true),
        (work.id, "GST number", "27AAAPL1234C1Z5", &["tax"], true),
        (work.id, "Office address", "Infinity Tower, 4th Floor\nLink Road, Malad West\nMumbai 400064", &["address"], false),
        (work.id, "Product code · Pro", "18 mm 8 x 4 LL Pro", &["catalog"], false),
        (personal.id, "Home address", "12 Park Street, Bandra West, Mumbai 400050", &["address"], true),
        (personal.id, "Phone", "+91 98200 12345", &[], false),
        (templates.id, "Invoice reminder", "Dear {name},\n\nA friendly reminder that invoice #2026-114 is due on Friday. Please let me know if you need anything from our side.\n\nThanks,\nHardik", &["invoice"], false),
        (templates.id, "Meeting follow-up", "Thanks for your time today. As discussed, I'll share the quarterly report by Monday.", &["follow-up"], false),
        (templates.id, "Out of office", "I'm away until Monday with limited access to email.", &[], false),
    ];
    for (cid, title, content, tags, star) in snippets {
        let s = lib
            .create_snippet(
                cid,
                SnippetDraft {
                    title: title.into(),
                    content: content.into(),
                    notes: String::new(),
                    tags: tags.iter().map(|t| t.to_string()).collect(),
                },
            )
            .unwrap();
        if star {
            lib.set_starred(&[s.id], true).unwrap();
        }
    }

    let history: [(i64, &str, Option<&str>); 10] = [
        (6 * DAY, "Quarterly revenue grew 18% year over year, driven by the Pro line.", None),
        (2 * DAY, "invoice_2026_114_final.pdf", None),
        (26 * HOUR, "SELECT name, email FROM customers WHERE city = 'Mumbai' ORDER BY name;", None),
        (5 * HOUR, "Dear team,\nPlease find attached the quarterly report for review. Let me know if anything needs to change before Friday's meeting with the client.", Some("<p>Dear team,</p>")),
        (3 * HOUR, "https://github.com/infinityhardik/shiftPaste", None),
        (95 * MINUTE, "Invoice #2026-114 · ₹ 48,500 due 21 Nov", None),
        (40 * MINUTE, "École Centrale · naïve café menu", None),
        (12 * MINUTE, "Meeting moved to 4:30 pm, same room", None),
        (4 * MINUTE, "18 mm 8 x 4 LL Pro", None),
        (MINUTE / 2, "INV-2026-0042", None),
    ];
    for (ago, text, html) in history {
        at(now - ago);
        lib.record_copy(
            &NewClip {
                text: text.into(),
                html: html.map(str::to_owned),
                ..Default::default()
            },
            HistoryLimits::default(),
        )
        .unwrap();
    }
    at(now);
    (lib, now)
}
