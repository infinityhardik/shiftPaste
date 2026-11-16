//! Import real spreadsheet files (the 1.x sample master file).

use shiftpaste_core::import::{auto_mapping, one_click_plans, read_file, Field};
use shiftpaste_core::library::Library;

#[test]
fn imports_the_1x_sample_master_file() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/work-1x.xlsx");
    let wb = read_file(&path).unwrap();
    assert_eq!(wb.sheets[0].name, "Work");
    // The header row "Content | Timestamp | Notes" is detected, not imported as an item.
    let (has_header, mapping) = auto_mapping(&wb.sheets[0]);
    assert!(has_header);
    assert_eq!(mapping, vec![Field::Content, Field::Ignore, Field::Notes]);

    let mut lib = Library::in_memory().unwrap();
    let report = lib.import(&wb, &one_click_plans(&wb)).unwrap();
    let items: Vec<String> = lib
        .snippets(report.created[0].0)
        .unwrap()
        .into_iter()
        .map(|s| s.content)
        .collect();
    assert_eq!(items, ["Abhishek", "Parth", "Infinity", "X"]);
}
