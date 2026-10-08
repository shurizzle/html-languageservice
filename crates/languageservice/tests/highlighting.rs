//! Port of `src/test/highlighting.test.ts`.

mod support;

use html_languageservice::*;
use support::{doc, split_marker};

fn assert_highlights(value: &str, expected_matches: &[usize], element_name: Option<&str>) {
    let (value, offset) = split_marker(value);
    let document = doc("test://test/test.html", "html", &value);
    let position = document.position_at(offset);
    let ls = get_language_service(LanguageServiceOptions::default());
    let html_doc = ls.parse_html_document(&document);

    let highlights = ls.find_document_highlights(&document, position, &html_doc);
    assert_eq!(highlights.len(), expected_matches.len());
    for (i, highlight) in highlights.iter().enumerate() {
        let element_name = element_name.expect("element name required when matches exist");
        let start = document.offset_at(highlight.range.start);
        assert_eq!(start, expected_matches[i]);
        let end = document.offset_at(highlight.range.end);
        assert_eq!(end, expected_matches[i] + element_name.len());
        let byte_start = document.byte_offset_at(start);
        let byte_end = document.byte_offset_at(end);
        assert_eq!(
            document.get_text()[byte_start..byte_end].to_lowercase(),
            element_name
        );
    }
}

#[test]
fn single() {
    assert_highlights("|<html></html>", &[], None);
    assert_highlights("<|html></html>", &[1, 8], Some("html"));
    assert_highlights("<h|tml></html>", &[1, 8], Some("html"));
    assert_highlights("<htm|l></html>", &[1, 8], Some("html"));
    assert_highlights("<html|></html>", &[1, 8], Some("html"));
    assert_highlights("<html>|</html>", &[], None);
    assert_highlights("<html><|/html>", &[], None);
    assert_highlights("<html></|html>", &[1, 8], Some("html"));
    assert_highlights("<html></h|tml>", &[1, 8], Some("html"));
    assert_highlights("<html></ht|ml>", &[1, 8], Some("html"));
    assert_highlights("<html></htm|l>", &[1, 8], Some("html"));
    assert_highlights("<html></html|>", &[1, 8], Some("html"));
    assert_highlights("<html></html>|", &[], None);
}

#[test]
fn nested() {
    assert_highlights("<html>|<div></div></html>", &[], None);
    assert_highlights("<html><|div></div></html>", &[7, 13], Some("div"));
    assert_highlights("<html><div>|</div></html>", &[], None);
    assert_highlights("<html><div></di|v></html>", &[7, 13], Some("div"));
    assert_highlights(
        "<html><div><div></div></di|v></html>",
        &[7, 24],
        Some("div"),
    );
    assert_highlights(
        "<html><div><div></div|></div></html>",
        &[12, 18],
        Some("div"),
    );
    assert_highlights(
        "<html><div><div|></div></div></html>",
        &[12, 18],
        Some("div"),
    );
    assert_highlights(
        "<html><div><div></div></div></h|tml>",
        &[1, 30],
        Some("html"),
    );
    assert_highlights(
        "<html><di|v></div><div></div></html>",
        &[7, 13],
        Some("div"),
    );
    assert_highlights(
        "<html><div></div><div></d|iv></html>",
        &[18, 24],
        Some("div"),
    );
}

#[test]
fn selfclosed() {
    assert_highlights("<html><|div/></html>", &[7], Some("div"));
    assert_highlights("<html><|br></html>", &[7], Some("br"));
    assert_highlights("<html><div><d|iv/></div></html>", &[12], Some("div"));
}

#[test]
fn case_insensivity() {
    assert_highlights(
        "<HTML><diV><Div></dIV></dI|v></html>",
        &[7, 24],
        Some("div"),
    );
    assert_highlights(
        "<HTML><diV|><Div></dIV></dIv></html>",
        &[7, 24],
        Some("div"),
    );
}

#[test]
fn incomplete() {
    assert_highlights("<div><ol><li></li></ol></p></|div>", &[1, 29], Some("div"));
}
