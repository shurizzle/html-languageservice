//! Port of `src/test/hover.test.ts` (and `hoverUtil.ts`).

mod support;

use html_languageservice::language_facts::data_provider::BASELINE_HIGH;
use html_languageservice::*;
use support::split_marker;

fn assert_hover(
    value: &str,
    expected_content: Option<MarkupContent>,
    expected_offset: Option<usize>,
) {
    let (value, offset) = split_marker(value);
    let document = doc("test://test/test.html", "html", &value);
    let position = document.position_at(offset);
    let ls = get_language_service(LanguageServiceOptions::default());
    let html_doc = ls.parse_html_document(&document);

    let hover = ls.do_hover(&document, position, &html_doc, None);
    let actual = hover.as_ref().map(|h| h.contents.clone());
    match expected_content {
        Some(expected) => {
            assert_eq!(actual, Some(MarkupInput::Markup(expected)));
        }
        None => assert!(actual.is_none(), "expected no hover, got {actual:?}"),
    }

    let actual_offset = hover
        .as_ref()
        .and_then(|h| h.range.map(|r| document.offset_at(r.start)));
    assert_eq!(actual_offset, expected_offset);
}

fn doc(uri: &str, language_id: &str, content: &str) -> TextDocument {
    TextDocument::create(uri, language_id, 0, content)
}

fn assert_hover2(
    value: &str,
    contents: MarkupInput,
    range_text: &str,
    options: LanguageServiceOptions,
    hover_settings: Option<HoverSettings>,
) {
    let (value, offset) = split_marker(value);
    let document = doc("test://test/test.html", "html", &value);
    let position = document.position_at(offset);
    let ls = get_language_service(options);
    let html_doc = ls.parse_html_document(&document);

    let hover = ls.do_hover(&document, position, &html_doc, hover_settings.as_ref());
    if let Some(hover) = hover {
        match &contents {
            MarkupInput::String(s) => {
                assert_eq!(hover.contents, MarkupInput::String(s.clone()));
            }
            MarkupInput::Markup(m) => {
                let actual = hover.contents.as_markup();
                assert_eq!(actual.kind, m.kind);
                assert_eq!(actual.value, m.value);
            }
        }
        if let Some(range) = hover.range {
            assert_eq!(range_text, document.get_text_range(range));
        }
    }
}

fn markdown(value: String) -> MarkupInput {
    MarkupInput::Markup(MarkupContent::new(MarkupKind::Markdown, value))
}

#[test]
fn single() {
    let description_and_reference = format!(
        "The html element represents the root of an HTML document.\n\n![Baseline icon]({BASELINE_HIGH}) _Widely available across major browsers (Baseline since 2015)_\n\n[MDN Reference](https://developer.mozilla.org/docs/Web/HTML/Reference/Elements/html)"
    );

    let html_content = MarkupContent::new(MarkupKind::Markdown, description_and_reference.clone());
    let close_html_content =
        MarkupContent::new(MarkupKind::Markdown, description_and_reference.clone());

    let entity_description =
        "Character entity representing '\u{00A0}', unicode equivalent 'U+00A0'";

    assert_hover("|<html></html>", None, None);
    assert_hover("<|html></html>", Some(html_content.clone()), Some(1));
    assert_hover("<h|tml></html>", Some(html_content.clone()), Some(1));
    assert_hover("<htm|l></html>", Some(html_content.clone()), Some(1));
    assert_hover("<html|></html>", Some(html_content.clone()), Some(1));
    assert_hover("<html>|</html>", None, None);
    assert_hover("<html><|/html>", None, None);
    assert_hover("<html></|html>", Some(close_html_content.clone()), Some(8));
    assert_hover("<html></h|tml>", Some(close_html_content.clone()), Some(8));
    assert_hover("<html></ht|ml>", Some(close_html_content.clone()), Some(8));
    assert_hover("<html></htm|l>", Some(close_html_content.clone()), Some(8));
    assert_hover("<html></html|>", Some(close_html_content.clone()), Some(8));
    assert_hover("<html></html>|", None, None);

    assert_hover2(
        "<html>|&nbsp;</html>",
        MarkupInput::String(String::new()),
        "",
        LanguageServiceOptions::default(),
        None,
    );
    assert_hover2(
        "<html>&|nbsp;</html>",
        MarkupInput::String(entity_description.to_string()),
        "nbsp;",
        LanguageServiceOptions::default(),
        None,
    );
    assert_hover2(
        "<html>&n|bsp;</html>",
        MarkupInput::String(entity_description.to_string()),
        "nbsp;",
        LanguageServiceOptions::default(),
        None,
    );
    assert_hover2(
        "<html>&nb|sp;</html>",
        MarkupInput::String(entity_description.to_string()),
        "nbsp;",
        LanguageServiceOptions::default(),
        None,
    );
    assert_hover2(
        "<html>&nbs|p;</html>",
        MarkupInput::String(entity_description.to_string()),
        "nbsp;",
        LanguageServiceOptions::default(),
        None,
    );
    assert_hover2(
        "<html>&nbsp|;</html>",
        MarkupInput::String(entity_description.to_string()),
        "nbsp;",
        LanguageServiceOptions::default(),
        None,
    );
    assert_hover2(
        "<html>&nbsp;|</html>",
        MarkupInput::String(String::new()),
        "",
        LanguageServiceOptions::default(),
        None,
    );

    let no_description = markdown(
        "[MDN Reference](https://developer.mozilla.org/docs/Web/HTML/Reference/Elements/html)"
            .to_string(),
    );
    assert_hover2(
        "<html|></html>",
        no_description,
        "html",
        LanguageServiceOptions::default(),
        Some(HoverSettings {
            documentation: Some(false),
            references: None,
        }),
    );

    let no_references = markdown(format!(
        "The html element represents the root of an HTML document.\n\n![Baseline icon]({BASELINE_HIGH}) _Widely available across major browsers (Baseline since 2015)_"
    ));
    assert_hover2(
        "<html|></html>",
        no_references,
        "html",
        LanguageServiceOptions::default(),
        Some(HoverSettings {
            documentation: None,
            references: Some(false),
        }),
    );
}
