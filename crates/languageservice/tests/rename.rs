//! Port of `src/test/rename.test.ts`.

mod support;

use html_languageservice::*;
use support::{doc, split_marker};

fn test_rename(value: &str, new_name: &str, expected_doc_content: &str) {
    let (value, offset) = split_marker(value);
    let ls = get_language_service(LanguageServiceOptions::default());
    let document = doc("test://test/test.html", "html", &value);
    let position = document.position_at(offset);
    let html_doc = ls.parse_html_document(&document);

    let workspace_edit = ls
        .do_rename(&document, position, new_name, &html_doc)
        .expect("No workspace edits");
    let edits = workspace_edit
        .edits_for(document.uri())
        .expect("No edits for file");

    let new_doc_content = apply_edits(&document, edits);
    assert_eq!(
        new_doc_content, expected_doc_content,
        "Expected: {expected_doc_content:?} Actual: {new_doc_content:?}"
    );
}

fn test_no_rename(value: &str, new_name: &str) {
    let (value, offset) = split_marker(value);
    let ls = get_language_service(LanguageServiceOptions::default());
    let document = doc("test://test/test.html", "html", &value);
    let position = document.position_at(offset);
    let html_doc = ls.parse_html_document(&document);

    let result = ls.do_rename(&document, position, new_name, &html_doc);
    assert!(
        result.is_none(),
        "Should not rename but rename happened: {result:?}"
    );
}

#[test]
fn rename_tag() {
    test_rename("<|div></div>", "h1", "<h1></h1>");
    test_rename("<d|iv></div>", "h1", "<h1></h1>");
    test_rename("<di|v></div>", "h1", "<h1></h1>");
    test_rename("<div|></div>", "h1", "<h1></h1>");
    test_rename("<|div></div>", "h1", "<h1></h1>");
    test_rename("<|div></div>", "h1", "<h1></h1>");

    test_no_rename("|<div></div>", "h1");
    test_no_rename("<div>|</div>", "h1");
    test_no_rename("<div><|/div>", "h1");
    test_no_rename("<div></div>|", "h1");

    test_no_rename("<div |id=\"foo\"></div>", "h1");
    test_no_rename("<div i|d=\"foo\"></div>", "h1");
    test_no_rename("<div id|=\"foo\"></div>", "h1");
    test_no_rename("<div id=|\"foo\"></div>", "h1");
    test_no_rename("<div id=\"|foo\"></div>", "h1");
    test_no_rename("<div id=\"f|oo\"></div>", "h1");
    test_no_rename("<div id=\"fo|o\"></div>", "h1");
    test_no_rename("<div id=\"foo|\"></div>", "h1");
    test_no_rename("<div id=\"foo\"|></div>", "h1");
}

#[test]
fn rename_self_closing_tag() {
    test_rename("<|br>", "h1", "<h1>");
    test_rename("<|br/>", "h1", "<h1/>");
    test_rename("<|br />", "h1", "<h1 />");
}

#[test]
fn rename_inner_tag() {
    test_rename("<div><|h1></h1></div>", "h2", "<div><h2></h2></div>");
}

#[test]
fn rename_unmatched_tag() {
    test_rename("<div><|h1></div>", "h2", "<div><h2></div>");
    test_rename("<|div><h1></h1></div>", "span", "<span><h1></h1></span>");
}
