//! Port of `src/test/selectionRange.test.ts`.

mod support;

use html_languageservice::*;
use support::split_marker;

fn assert_ranges(content: &str, expected: &[(usize, &str)]) {
    let (content, offset) = split_marker(content);
    let ls = get_language_service(LanguageServiceOptions::default());
    let document = TextDocument::create("test://foo.html", "html", 1, content);
    let actual_ranges = ls.get_selection_ranges(&document, &[document.position_at(offset)]);
    assert_eq!(actual_ranges.len(), 1);

    let mut offset_pairs: Vec<(usize, String)> = Vec::new();
    let mut curr = Some(&actual_ranges[0]);
    while let Some(c) = curr {
        offset_pairs.push((
            document.offset_at(c.range.start),
            document.get_text_range(c.range),
        ));
        curr = c.parent.as_deref();
    }

    let expected: Vec<(usize, String)> =
        expected.iter().map(|(o, t)| (*o, t.to_string())).collect();
    assert_eq!(offset_pairs, expected);
}

#[test]
fn basic() {
    assert_ranges("<div|>foo</div>", &[(1, "div"), (0, "<div>foo</div>")]);
    assert_ranges("<|div>foo</div>", &[(1, "div"), (0, "<div>foo</div>")]);
    assert_ranges("<d|iv>foo</div>", &[(1, "div"), (0, "<div>foo</div>")]);

    assert_ranges("<div>|foo</div>", &[(5, "foo"), (0, "<div>foo</div>")]);
    assert_ranges("<div>f|oo</div>", &[(5, "foo"), (0, "<div>foo</div>")]);
    assert_ranges("<div>foo|</div>", &[(5, "foo"), (0, "<div>foo</div>")]);

    assert_ranges("<div>foo<|/div>", &[(0, "<div>foo</div>")]);

    assert_ranges("<div>foo</|div>", &[(10, "div"), (0, "<div>foo</div>")]);
    assert_ranges("<div>foo</di|v>", &[(10, "div"), (0, "<div>foo</div>")]);
    assert_ranges("<div>foo</div|>", &[(10, "div"), (0, "<div>foo</div>")]);
}

#[test]
fn attribute_name() {
    assert_ranges(
        r#"<div |class="foo">foo</div>"#,
        &[
            (5, "class"),
            (5, r#"class="foo""#),
            (1, r#"div class="foo""#),
            (0, r#"<div class="foo">foo</div>"#),
        ],
    );
    assert_ranges(
        r#"<div cl|ass="foo">foo</div>"#,
        &[
            (5, "class"),
            (5, r#"class="foo""#),
            (1, r#"div class="foo""#),
            (0, r#"<div class="foo">foo</div>"#),
        ],
    );
    assert_ranges(
        r#"<div class|="foo">foo</div>"#,
        &[
            (5, "class"),
            (5, r#"class="foo""#),
            (1, r#"div class="foo""#),
            (0, r#"<div class="foo">foo</div>"#),
        ],
    );
}

#[test]
fn attribute_value() {
    assert_ranges(
        r#"<div class=|"foo">foo</div>"#,
        &[
            (11, r#""foo""#),
            (5, r#"class="foo""#),
            (1, r#"div class="foo""#),
            (0, r#"<div class="foo">foo</div>"#),
        ],
    );
    assert_ranges(
        r#"<div class="foo"|>foo</div>"#,
        &[
            (11, r#""foo""#),
            (5, r#"class="foo""#),
            (1, r#"div class="foo""#),
            (0, r#"<div class="foo">foo</div>"#),
        ],
    );

    assert_ranges(
        r#"<div class="|foo">foo</div>"#,
        &[
            (12, "foo"),
            (11, r#""foo""#),
            (5, r#"class="foo""#),
            (1, r#"div class="foo""#),
            (0, r#"<div class="foo">foo</div>"#),
        ],
    );
    assert_ranges(
        r#"<div class="f|oo">foo</div>"#,
        &[
            (12, "foo"),
            (11, r#""foo""#),
            (5, r#"class="foo""#),
            (1, r#"div class="foo""#),
            (0, r#"<div class="foo">foo</div>"#),
        ],
    );
}

#[test]
fn unquoted_attribute_value() {
    assert_ranges(
        "<div class=|foo>foo</div>",
        &[
            (11, "foo"),
            (5, "class=foo"),
            (1, "div class=foo"),
            (0, "<div class=foo>foo</div>"),
        ],
    );
}

#[test]
fn multiple_attribute_value() {
    assert_ranges(
        r#"<div class="foo" id="|bar">foo</div>"#,
        &[
            (21, "bar"),
            (20, r#""bar""#),
            (17, r#"id="bar""#),
            (1, r#"div class="foo" id="bar""#),
            (0, r#"<div class="foo" id="bar">foo</div>"#),
        ],
    );
}

#[test]
fn self_closing_tags() {
    assert_ranges(
        r#"<br class="|foo"/>"#,
        &[
            (11, "foo"),
            (10, r#""foo""#),
            (4, r#"class="foo""#),
            (1, r#"br class="foo""#),
            (0, r#"<br class="foo"/>"#),
        ],
    );
    assert_ranges(
        r#"<b|r class="foo"/>"#,
        &[(1, r#"br class="foo""#), (0, r#"<br class="foo"/>"#)],
    );
}

#[test]
fn nested() {
    assert_ranges(
        "<div><div>|foo</div></div>",
        &[
            (10, "foo"),
            (5, "<div>foo</div>"),
            (0, "<div><div>foo</div></div>"),
        ],
    );
    assert_ranges(
        "<div>\n<p>|foo</p>\n</div>",
        &[
            (9, "foo"),
            (6, "<p>foo</p>"),
            (5, "\n<p>foo</p>\n"),
            (0, "<div>\n<p>foo</p>\n</div>"),
        ],
    );
}

#[test]
fn void_elements() {
    assert_ranges(
        "<meta charset='|UTF-8'>",
        &[
            (15, "UTF-8"),
            (14, "'UTF-8'"),
            (6, "charset='UTF-8'"),
            (1, "meta charset='UTF-8'"),
            (0, "<meta charset='UTF-8'>"),
        ],
    );
    assert_ranges(
        "<meta c|harset='UTF-8'>",
        &[
            (6, "charset"),
            (6, "charset='UTF-8'"),
            (1, "meta charset='UTF-8'"),
            (0, "<meta charset='UTF-8'>"),
        ],
    );
    assert_ranges(
        "<html><meta c|harset='UTF-8'></html>",
        &[
            (12, "charset"),
            (12, "charset='UTF-8'"),
            (7, "meta charset='UTF-8'"),
            (6, "<meta charset='UTF-8'>"),
            (0, "<html><meta charset='UTF-8'></html>"),
        ],
    );
}

#[test]
fn unmatching_tags() {
    assert_ranges("<div></div|1>", &[(0, "<div></div1>")]);
}

#[test]
fn unhandled() {
    assert_ranges("<!-- f|oo -->", &[(6, "")]);
    assert_ranges("<!DOCTYPE h|tml>", &[(11, "")]);
}
