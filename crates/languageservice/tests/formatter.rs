//! Port of `src/test/formatter.test.ts`.

use html_languageservice::{
    HTMLFormatConfiguration, LanguageServiceOptions, Range, TemplatingFormatOption, TextDocument,
    apply_edits, get_language_service,
};

fn create_service() -> html_languageservice::LanguageService {
    get_language_service(LanguageServiceOptions::default())
}

fn strip_markers(unformatted: &str) -> (String, Option<Range>, TextDocument) {
    let first = unformatted.find('|');
    let last = unformatted.rfind('|');
    let doc_for_offset = |content: &str, offset: usize| {
        TextDocument::create("test://test.html", "html", 0, content).position_at(offset)
    };
    match (first, last) {
        (Some(a), Some(b)) if a != b => {
            let mut cleaned = String::new();
            cleaned.push_str(&unformatted[..a]);
            cleaned.push_str(&unformatted[a + 1..b]);
            cleaned.push_str(&unformatted[b + 1..]);
            let range = Range {
                start: doc_for_offset(&cleaned, a),
                end: doc_for_offset(&cleaned, b - 1),
            };
            (
                cleaned.clone(),
                Some(range),
                TextDocument::create("test://test.html", "html", 0, &cleaned),
            )
        }
        _ => (
            unformatted.to_string(),
            None,
            TextDocument::create("test://test.html", "html", 0, unformatted),
        ),
    }
}

fn format(unformatted: &str, expected: &str, insert_spaces: bool) {
    let (_content, range, document) = strip_markers(unformatted);
    let service = create_service();
    let options = HTMLFormatConfiguration {
        tab_size: Some(2),
        insert_spaces: Some(insert_spaces),
        unformatted: Some(String::new()),
        ..Default::default()
    };
    let edits = service.format(&document, range, &options);
    let formatted = apply_edits(&document, &edits);
    assert_eq!(formatted, expected);
}

fn format_with_options(unformatted: &str, expected: &str, options: HTMLFormatConfiguration) {
    let document = TextDocument::create("test://test.html", "html", 0, unformatted);
    let service = create_service();
    let edits = service.format(&document, None, &options);
    let formatted = apply_edits(&document, &edits);
    assert_eq!(formatted, expected);
}

#[test]
fn full_document() {
    let content = ["<div  class = \"foo\">", "<br>", " </div>"].join("\n");
    let expected = ["<div class=\"foo\">", "  <br>", "</div>"].join("\n");
    format(&content, &expected, true);
}

#[test]
fn range() {
    let content = [
        "<div  class = \"foo\">",
        "  |<img  src = \"foo\">|",
        " </div>",
    ]
    .join("\n");
    let expected = ["<div  class = \"foo\">", "  <img src=\"foo\">", " </div>"].join("\n");
    format(&content, &expected, true);
}

#[test]
fn range_2() {
    let content = [
        "<div  class = \"foo\">",
        "  |<img  src = \"foo\">|",
        "  ",
        " </div>",
    ]
    .join("\n");
    let expected = [
        "<div  class = \"foo\">",
        "  <img src=\"foo\">",
        "  ",
        " </div>",
    ]
    .join("\n");
    format(&content, &expected, true);
}

#[test]
fn range_3() {
    let content = [
        "<div  class = \"foo\">",
        "  |<img  src = \"foo\">|    ",
        "  ",
        " </div>",
    ]
    .join("\n");
    let expected = [
        "<div  class = \"foo\">",
        "  <img src=\"foo\">",
        "  ",
        " </div>",
    ]
    .join("\n");
    format(&content, &expected, true);
}

#[test]
fn range_4() {
    let content = [
        "<div  class = \"foo\">",
        "  |<img  src = \"foo\"|>    ",
        "  ",
        " </div>",
    ]
    .join("\n");
    let expected = [
        "<div  class = \"foo\">",
        "  <img src=\"foo\">    ",
        "  ",
        " </div>",
    ]
    .join("\n");
    format(&content, &expected, true);
}

#[test]
fn range_5() {
    let content = [
        "<div  class = \"foo\">",
        "  <|img  src = \"foo\">|    ",
        "  ",
        " </div>",
    ]
    .join("\n");
    let expected = [
        "<div  class = \"foo\">",
        "  <img  src = \"foo\">    ",
        "  ",
        " </div>",
    ]
    .join("\n");
    format(&content, &expected, true);
}

#[test]
fn range_6() {
    let content = [
        "<div|  class = \"foo\">",
        "  <img  src = \"foo\">    ",
        "  ",
        " </div>|",
    ]
    .join("\n");
    let expected = [
        "<div  class = \"foo\">",
        "  <img  src = \"foo\">    ",
        "  ",
        " </div>",
    ]
    .join("\n");
    format(&content, &expected, true);
}

#[test]
fn range_7() {
    let content = "<div |class= \"foo\"|>";
    let expected = "<div class= \"foo\">";
    format(content, expected, true);
}

#[test]
fn range_with_indent() {
    let content = [
        "<div  class = \"foo\">",
        "  |<img src = \"foo\">",
        "  <img  src = \"foo\">|",
        " </div>",
    ]
    .join("\n");
    let expected = [
        "<div  class = \"foo\">",
        "  <img src=\"foo\">",
        "  <img src=\"foo\">",
        " </div>",
    ]
    .join("\n");
    format(&content, &expected, true);
}

#[test]
fn range_with_indent_2() {
    let content = [
        "<div  class = \"foo\">",
        "|  <img  src = \"foo\">",
        "  <img  src = \"foo\">|",
        " </div>",
    ]
    .join("\n");
    let expected = [
        "<div  class = \"foo\">",
        "  <img src=\"foo\">",
        "  <img src=\"foo\">",
        " </div>",
    ]
    .join("\n");
    format(&content, &expected, true);
}

#[test]
fn range_with_indent_3() {
    let content = [
        "<div  class = \"foo\">",
        "  <div></div>   |<img  src = \"foo\"|>",
        " </div>",
    ]
    .join("\n");
    let expected = [
        "<div  class = \"foo\">",
        "  <div></div> <img src=\"foo\">",
        " </div>",
    ]
    .join("\n");
    format(&content, &expected, true);
}

#[test]
fn range_with_indent_4a() {
    let content = [
        "<div  class = \"foo\">",
        "  <div>|</div>   <img  src = \"foo\"|>",
        " </div>",
    ]
    .join("\n");
    let expected = [
        "<div  class = \"foo\">",
        "  <div></div> <img src=\"foo\">",
        " </div>",
    ]
    .join("\n");
    format(&content, &expected, true);
}

#[test]
fn range_with_indent_4b() {
    let content = [
        "<div  class = \"foo\">",
        "  <div></div>|<img src = \"foo\">",
        "    <img  src = \"foo\">|",
        " </div>",
    ]
    .join("\n");
    let expected = [
        "<div  class = \"foo\">",
        "  <div></div><img src=\"foo\">",
        "  <img src=\"foo\">",
        " </div>",
    ]
    .join("\n");
    format(&content, &expected, true);
}

#[test]
fn range_with_indent_5() {
    let content = [
        "<div  class = \"foo\">",
        "   |<img src = \"foo\">",
        "   <img  src = \"foo\">|",
        " </div>",
    ]
    .join("\n");
    let expected = [
        "<div  class = \"foo\">",
        "  <img src=\"foo\">",
        "  <img src=\"foo\">",
        " </div>",
    ]
    .join("\n");
    format(&content, &expected, true);
}

#[test]
fn range_with_indent_6() {
    let content = [
        "<div  class = \"foo\">",
        "    |<img src = \"foo\">",
        "    <img  src = \"foo\">|",
        " </div>",
    ]
    .join("\n");
    let expected = [
        "<div  class = \"foo\">",
        "    <img src=\"foo\">",
        "    <img src=\"foo\">",
        " </div>",
    ]
    .join("\n");
    format(&content, &expected, true);
}

#[test]
fn range_with_indent_7() {
    let content = [
        "<div  class = \"foo\">",
        "    <div></div>|<img src = \"foo\">",
        "      <img  src = \"foo\">|",
        " </div>",
    ]
    .join("\n");
    let expected = [
        "<div  class = \"foo\">",
        "    <div></div><img src=\"foo\">",
        "    <img src=\"foo\">",
        " </div>",
    ]
    .join("\n");
    format(&content, &expected, true);
}

#[test]
fn bug_36574() {
    let content = "<script src=\"/js/main.js\"> </script>";
    let expected = "<script src=\"/js/main.js\"> </script>";
    format(content, expected, true);
}

#[test]
fn beautify_web_1491() {
    let content = [
        "<head>",
        "    <script src=\"one.js\"></script> <!-- one -->",
        "    <script src=\"two.js\"></script> <!-- two-->",
        "</head>",
    ]
    .join("\n");
    let expected = [
        "<head>",
        "  <script src=\"one.js\"></script> <!-- one -->",
        "  <script src=\"two.js\"></script> <!-- two-->",
        "</head>",
    ]
    .join("\n");
    format(&content, &expected, true);
}

#[test]
fn bug_58693() {
    let content = "<a class=\"btn| btn-link|\"></a>";
    let expected = "<a class=\"btn btn-link\"></a>";
    format(content, expected, true);
}

#[test]
fn ignore_start_end_full_document() {
    let content = [
        "<html>",
        "<body>",
        "<!-- beautify ignore:start -->",
        "<div   class = \"x\">",
        "  <p>text</p>",
        "</div>",
        "<!-- beautify ignore:end -->",
        "<div   class = \"y\">",
        "</div>",
        "</body>",
        "</html>",
    ]
    .join("\n");
    let expected = [
        "<html>",
        "",
        "<body>",
        "  <!-- beautify ignore:start -->",
        "<div   class = \"x\">",
        "  <p>text</p>",
        "</div>",
        "<!-- beautify ignore:end -->",
        "  <div class=\"y\">",
        "  </div>",
        "</body>",
        "",
        "</html>",
    ]
    .join("\n");
    format(&content, &expected, true);
}

#[test]
fn ignore_start_end_range_inside_ignore_block() {
    let content = [
        "<!-- beautify ignore:start -->",
        "|<div   class = \"x\">",
        "  <p>text</p>",
        "</div>|",
        "<!-- beautify ignore:end -->",
    ]
    .join("\n");
    let expected = [
        "<!-- beautify ignore:start -->",
        "<div   class = \"x\">",
        "  <p>text</p>",
        "</div>",
        "<!-- beautify ignore:end -->",
    ]
    .join("\n");
    format(&content, &expected, true);
}

#[test]
fn ignore_extra_whitespace_in_comment() {
    let content = [
        "<!--  beautify  ignore:start  -->",
        "<div   class = \"x\">",
        "</div>",
        "<!--  beautify  ignore:end  -->",
    ]
    .join("\n");
    let expected = [
        "<!--  beautify  ignore:start  -->",
        "<div   class = \"x\">",
        "</div>",
        "<!--  beautify  ignore:end  -->",
    ]
    .join("\n");
    format(&content, &expected, true);
}

#[test]
fn ignore_go_template_syntax_preserved() {
    let content = [
        "<!-- beautify ignore:start -->",
        "<div {{if .Active}}class=\"active\"{{end}}>",
        "  {{template \"scripts/file-size\".}}",
        "</div>",
        "<!-- beautify ignore:end -->",
    ]
    .join("\n");
    let expected = [
        "<!-- beautify ignore:start -->",
        "<div {{if .Active}}class=\"active\"{{end}}>",
        "  {{template \"scripts/file-size\".}}",
        "</div>",
        "<!-- beautify ignore:end -->",
    ]
    .join("\n");
    format(&content, &expected, true);
}

// ---------------------------------------------------------------------------
// Embedded CSS
// ---------------------------------------------------------------------------

fn css_option<F: FnOnce(&mut html_languageservice::EmbeddedCSSFormatConfiguration)>(
    f: F,
) -> HTMLFormatConfiguration {
    let mut css = html_languageservice::EmbeddedCSSFormatConfiguration::default();
    f(&mut css);
    HTMLFormatConfiguration {
        tab_size: Some(2),
        insert_spaces: Some(true),
        css: Some(css),
        ..Default::default()
    }
}

#[test]
fn css_newline_between_selectors_false() {
    let content = [
        "<html>",
        "",
        "<head>",
        "  <style>",
        "    h1, h2 { color: red; }",
        "  </style>",
        "</head>",
        "",
        "</html>",
    ]
    .join("\n");
    let expected = [
        "<html>",
        "",
        "<head>",
        "  <style>",
        "    h1, h2 {",
        "      color: red;",
        "    }",
        "  </style>",
        "</head>",
        "",
        "</html>",
    ]
    .join("\n");
    format_with_options(
        &content,
        &expected,
        css_option(|css| css.newline_between_selectors = Some(false)),
    );
}

#[test]
fn css_newline_between_selectors_true() {
    let content = [
        "<html>",
        "",
        "<head>",
        "  <style>",
        "    h1, h2 { color: red; }",
        "  </style>",
        "</head>",
        "",
        "</html>",
    ]
    .join("\n");
    let expected = [
        "<html>",
        "",
        "<head>",
        "  <style>",
        "    h1,",
        "    h2 {",
        "      color: red;",
        "    }",
        "  </style>",
        "</head>",
        "",
        "</html>",
    ]
    .join("\n");
    format_with_options(
        &content,
        &expected,
        css_option(|css| css.newline_between_selectors = Some(true)),
    );
}

#[test]
fn css_newline_between_rules_false() {
    let content = [
        "<html>",
        "",
        "<head>",
        "  <style>",
        "    h1 { color: red; }",
        "    h2 { color: blue; }",
        "  </style>",
        "</head>",
        "",
        "</html>",
    ]
    .join("\n");
    let expected = [
        "<html>",
        "",
        "<head>",
        "  <style>",
        "    h1 {",
        "      color: red;",
        "    }",
        "    h2 {",
        "      color: blue;",
        "    }",
        "  </style>",
        "</head>",
        "",
        "</html>",
    ]
    .join("\n");
    format_with_options(
        &content,
        &expected,
        css_option(|css| css.newline_between_rules = Some(false)),
    );
}

#[test]
fn css_newline_between_rules_true() {
    let content = [
        "<html>",
        "",
        "<head>",
        "  <style>",
        "    h1 { color: red; }",
        "    h2 { color: blue; }",
        "  </style>",
        "</head>",
        "",
        "</html>",
    ]
    .join("\n");
    let expected = [
        "<html>",
        "",
        "<head>",
        "  <style>",
        "    h1 {",
        "      color: red;",
        "    }",
        "",
        "    h2 {",
        "      color: blue;",
        "    }",
        "  </style>",
        "</head>",
        "",
        "</html>",
    ]
    .join("\n");
    format_with_options(
        &content,
        &expected,
        css_option(|css| css.newline_between_rules = Some(true)),
    );
}

#[test]
fn css_space_around_selector_separator_true() {
    let content = [
        "<html>",
        "",
        "<head>",
        "  <style>",
        "    div>span { color: red; }",
        "  </style>",
        "</head>",
        "",
        "</html>",
    ]
    .join("\n");
    let expected = [
        "<html>",
        "",
        "<head>",
        "  <style>",
        "    div > span {",
        "      color: red;",
        "    }",
        "  </style>",
        "</head>",
        "",
        "</html>",
    ]
    .join("\n");
    format_with_options(
        &content,
        &expected,
        css_option(|css| css.space_around_selector_separator = Some(true)),
    );
}

#[test]
fn no_css_options_uses_defaults() {
    let content = [
        "<html>",
        "",
        "<head>",
        "  <style>",
        "    h1, h2 { color: red; }",
        "  </style>",
        "</head>",
        "",
        "</html>",
    ]
    .join("\n");
    let expected = [
        "<html>",
        "",
        "<head>",
        "  <style>",
        "    h1,",
        "    h2 {",
        "      color: red;",
        "    }",
        "  </style>",
        "</head>",
        "",
        "</html>",
    ]
    .join("\n");
    format_with_options(
        &content,
        &expected,
        HTMLFormatConfiguration {
            tab_size: Some(2),
            insert_spaces: Some(true),
            ..Default::default()
        },
    );
}

// ---------------------------------------------------------------------------
// Range validation
// ---------------------------------------------------------------------------

fn assert_range_within_document(
    content: &str,
    options: Option<HTMLFormatConfiguration>,
) -> Vec<html_languageservice::TextEdit> {
    let document = TextDocument::create("test://test.html", "html", 0, content);
    let options = options.unwrap_or(HTMLFormatConfiguration {
        tab_size: Some(2),
        insert_spaces: Some(true),
        ..Default::default()
    });
    let service = create_service();
    let edits = service.format(&document, None, &options);
    let document_end = document.position_at(content.len());
    for edit in &edits {
        assert!(
            edit.range.end.line < document_end.line
                || (edit.range.end.line == document_end.line
                    && edit.range.end.character <= document_end.character),
            "Edit range end exceeds document end for {content:?}"
        );
    }
    edits
}

#[test]
fn empty_document_returns_no_edits() {
    let edits = assert_range_within_document("", None);
    assert!(edits.is_empty());
}

#[test]
fn whitespace_only_document_returns_valid_range() {
    assert_range_within_document("   ", None);
}

#[test]
fn newline_only_document_returns_valid_range() {
    assert_range_within_document("\n", None);
}

#[test]
fn crlf_only_document_returns_valid_range() {
    assert_range_within_document("\r\n", None);
}

#[test]
fn empty_document_with_end_with_newline_returns_valid_range() {
    let edits = assert_range_within_document(
        "",
        Some(HTMLFormatConfiguration {
            tab_size: Some(2),
            insert_spaces: Some(true),
            end_with_newline: Some(true),
            ..Default::default()
        }),
    );
    if !edits.is_empty() {
        assert_eq!(edits[0].new_text, "\n");
    }
}

#[test]
fn already_formatted_document_returns_no_edits() {
    let edits = assert_range_within_document("<div>\n  <br>\n</div>", None);
    assert!(edits.is_empty());
}

#[test]
fn range_after_non_ascii_text() {
    let text = "😀<div  class = \"foo\"><br></div>";
    let document = TextDocument::create("test://test.html", "html", 0, text);
    let start = text.find('<').unwrap();
    let start = text[..start].encode_utf16().count() as u32;
    let end = text.encode_utf16().count() as u32;
    let options = HTMLFormatConfiguration {
        tab_size: Some(2),
        insert_spaces: Some(true),
        unformatted: Some(String::new()),
        ..Default::default()
    };
    let edits = create_service().format(&document, Some(Range::create(0, start, 0, end)), &options);
    let formatted = apply_edits(&document, &edits);
    assert_eq!(formatted, "😀<div class=\"foo\"><br></div>");
}

#[test]
fn invalid_options_fall_back_to_defaults() {
    let text = "<div  class = \"foo\"><br></div>";
    let document = TextDocument::create("test://test.html", "html", 0, text);
    let options = HTMLFormatConfiguration {
        tab_size: Some(2),
        insert_spaces: Some(true),
        wrap_attributes: Some("banana".to_string()),
        indent_scripts: Some("banana".to_string()),
        templating: Some(TemplatingFormatOption::List(vec!["xxx".to_string()])),
        ..Default::default()
    };
    let edits = create_service().format(&document, None, &options);
    assert_eq!(
        apply_edits(&document, &edits),
        "<div class=\"foo\"><br></div>"
    );
}

#[test]
fn zero_or_negative_tab_size_with_range_does_not_panic() {
    let text = "<p>\n  <div  class=\"x\"></div></p>";
    let document = TextDocument::create("test://test.html", "html", 0, text);
    let start = text.find("<div").unwrap();
    let start = text[..start].encode_utf16().count() as u32;
    let end = text.encode_utf16().count() as u32;
    let range = Some(Range {
        start: document.position_at(start as usize),
        end: document.position_at(end as usize),
    });
    for tab_size in [0, -2] {
        let options = HTMLFormatConfiguration {
            tab_size: Some(tab_size),
            insert_spaces: Some(true),
            unformatted: Some(String::new()),
            ..Default::default()
        };
        let edits = create_service().format(&document, range, &options);
        assert!(!apply_edits(&document, &edits).contains("  class"));
    }
}
