//! Port of `src/test/linkedEditing.test.ts`.

mod support;

use html_languageservice::*;
use support::{doc, split_marker};

fn test_linked_editing(value: &str, expected: &[(usize, &str)]) {
    let (value, offset) = split_marker(value);
    let ls = get_language_service(LanguageServiceOptions::default());
    let document = doc("test://test/test.html", "html", &value);
    let position = document.position_at(offset);
    let html_doc = ls.parse_html_document(&document);

    let synced_regions = ls.find_linked_editing_ranges(&document, position, &html_doc);
    match synced_regions {
        None => assert!(
            expected.is_empty(),
            "No linked editing ranges for {value:?} but expecting {expected:?}"
        ),
        Some(ranges) => {
            let actual: Vec<(usize, String)> = ranges
                .iter()
                .map(|r| (document.offset_at(r.start), document.get_text_range(*r)))
                .collect();
            let expected: Vec<(usize, String)> =
                expected.iter().map(|(o, t)| (*o, t.to_string())).collect();
            assert_eq!(
                actual, expected,
                "Actual {actual:?} does not match expected"
            );
        }
    }
}

#[test]
fn linked_editing() {
    test_linked_editing("|<div></div>", &[]);
    test_linked_editing("<|div></div>", &[(1, "div"), (7, "div")]);
    test_linked_editing("<d|iv></div>", &[(1, "div"), (7, "div")]);
    test_linked_editing("<di|v></div>", &[(1, "div"), (7, "div")]);
    test_linked_editing("<div|></div>", &[(1, "div"), (7, "div")]);

    test_linked_editing("<div>|</div>", &[]);
    test_linked_editing("<div><|/div>", &[]);

    test_linked_editing("<div></|div>", &[(1, "div"), (7, "div")]);
    test_linked_editing("<div></d|iv>", &[(1, "div"), (7, "div")]);
    test_linked_editing("<div></di|v>", &[(1, "div"), (7, "div")]);
    test_linked_editing("<div></div|>", &[(1, "div"), (7, "div")]);

    test_linked_editing("<div></div>|", &[]);
    test_linked_editing("<div><div|</div>", &[]);
    test_linked_editing("<div><div><div|</div></div>", &[]);

    test_linked_editing("<div| ></div>", &[(1, "div"), (8, "div")]);
    test_linked_editing("<div| id=\"foo\"></div>", &[(1, "div"), (16, "div")]);

    test_linked_editing("<|></>", &[(1, ""), (4, "")]);
    test_linked_editing("<><div></div></|>", &[(1, ""), (15, "")]);
}
