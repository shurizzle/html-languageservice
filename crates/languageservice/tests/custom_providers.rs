//! Port of `src/test/customProviders.test.ts`.
#![allow(clippy::field_reassign_with_default)]

mod support;

use html_languageservice::*;
use support::split_marker;

fn markdown(value: &str) -> Option<MarkupInput> {
    Some(MarkupInput::Markup(MarkupContent::new(
        MarkupKind::Markdown,
        value,
    )))
}

fn value_data(name: &str, description: Option<MarkupInput>) -> IValueData {
    IValueData {
        name: name.to_string(),
        description,
        ..Default::default()
    }
}

fn attribute(name: &str, description: Option<MarkupInput>) -> IAttributeData {
    IAttributeData {
        name: name.to_string(),
        description,
        ..Default::default()
    }
}

fn test_data() -> HtmlDataProvider {
    let tags = vec![
        ITagData {
            name: "foo".to_string(),
            description: markdown("The `<foo>` element"),
            attributes: vec![
                IAttributeData {
                    values: Some(vec![value_data(
                        "baz",
                        markdown("The `<foo bar=\"baz\">` attribute"),
                    )]),
                    ..attribute("bar", markdown("The `<foo bar>` attribute"))
                },
                attribute("duplicateAttr", None),
            ],
            ..Default::default()
        },
        ITagData {
            name: "Bar".to_string(),
            description: markdown("The `<Bar>` element"),
            attributes: vec![attribute("Xoo", None)],
            ..Default::default()
        },
    ];

    let global_attributes = vec![
        attribute("fooAttr", markdown("`fooAttr` Attribute")),
        IAttributeData {
            value_set: Some("x".to_string()),
            ..attribute("xattr", markdown("`xattr` attributes"))
        },
        attribute("duplicateAttr", None),
    ];

    let value_sets = vec![IValueSet {
        name: "x".to_string(),
        values: vec![value_data("xval", markdown("`xval` value"))],
    }];

    new_html_data_provider(
        "test",
        HTMLDataV1 {
            version: 1.0,
            tags,
            global_attributes,
            value_sets,
        },
    )
}

fn language_options() -> LanguageServiceOptions {
    LanguageServiceOptions {
        custom_data_providers: vec![Box::new(test_data())],
        ..Default::default()
    }
}

struct ExpectItem {
    label: &'static str,
    documentation: Option<&'static str>,
    result_text: &'static str,
}

fn item(
    label: &'static str,
    documentation: Option<&'static str>,
    result_text: &'static str,
) -> ExpectItem {
    ExpectItem {
        label,
        documentation,
        result_text,
    }
}

fn test_completion_for(value: &str, expected: &[ExpectItem], options: LanguageServiceOptions) {
    let (value, offset) = split_marker(value);
    let ls = get_language_service(options);
    let document = TextDocument::create("test://test/test.html", "html", 0, value);
    let position = document.position_at(offset);
    let html_doc = ls.parse_html_document(&document);
    let list = ls.do_complete(&document, position, &html_doc, None);

    for exp in expected {
        let matches: Vec<&CompletionItem> =
            list.items.iter().filter(|c| c.label == exp.label).collect();
        assert_eq!(matches.len(), 1, "{} should exist once", exp.label);
        let m = matches[0];
        if let Some(doc) = exp.documentation {
            assert_eq!(
                m.documentation.as_ref().map(|d| d.as_markup().value),
                Some(doc.to_string())
            );
        }
        if let Some(edit) = &m.text_edit {
            assert_eq!(
                apply_edits(&document, std::slice::from_ref(edit)),
                exp.result_text
            );
        }
    }
}

fn assert_hover2(value: &str, contents: &str, range_text: &str, options: LanguageServiceOptions) {
    let (value, offset) = split_marker(value);
    let ls = get_language_service(options);
    let document = TextDocument::create("test://test/test.html", "html", 0, value);
    let position = document.position_at(offset);
    let html_doc = ls.parse_html_document(&document);

    let hover = ls
        .do_hover(&document, position, &html_doc, None)
        .expect("hover");
    assert_eq!(hover.contents.as_markup().value, contents);
    if let Some(range) = hover.range {
        assert_eq!(range_text, document.get_text_range(range));
    }
}

#[test]
fn completion() {
    let opts = language_options();

    test_completion_for(
        "<|",
        &[
            item("foo", Some("The `<foo>` element"), "<foo"),
            item("Bar", Some("The `<Bar>` element"), "<Bar"),
        ],
        language_options(),
    );

    test_completion_for(
        "<foo |",
        &[
            item("bar", Some("The `<foo bar>` attribute"), "<foo bar=\"$1\""),
            item(
                "fooAttr",
                Some("`fooAttr` Attribute"),
                "<foo fooAttr=\"$1\"",
            ),
            item("xattr", Some("`xattr` attributes"), "<foo xattr=\"$1\""),
            item("duplicateAttr", None, "<foo duplicateAttr=\"$1\""),
        ],
        language_options(),
    );

    test_completion_for(
        "<foo bar=|",
        &[item(
            "baz",
            Some("The `<foo bar=\"baz\">` attribute"),
            "<foo bar=\"baz\"",
        )],
        language_options(),
    );

    test_completion_for(
        "<foo xattr=|",
        &[item("xval", Some("`xval` value"), "<foo xattr=\"xval\"")],
        language_options(),
    );

    test_completion_for(
        "<Bar |",
        &[item("Xoo", None, "<Bar Xoo=\"$1\"")],
        language_options(),
    );

    test_completion_for(
        "<other |",
        &[
            item(
                "fooAttr",
                Some("`fooAttr` Attribute"),
                "<other fooAttr=\"$1\"",
            ),
            item("xattr", Some("`xattr` attributes"), "<other xattr=\"$1\""),
        ],
        language_options(),
    );

    test_completion_for(
        "<other xattr=|",
        &[item("xval", Some("`xval` value"), "<other xattr=\"xval\"")],
        language_options(),
    );

    drop(opts);
}

#[test]
fn hover() {
    assert_hover2(
        "<f|oo></foo>",
        "The `<foo>` element",
        "foo",
        language_options(),
    );

    assert_hover2(
        "<foo |bar></foo>",
        "The `<foo bar>` attribute",
        "bar",
        language_options(),
    );
    assert_hover2(
        "<foo |xattr></foo>",
        "`xattr` attributes",
        "xattr",
        language_options(),
    );

    assert_hover2(
        "<foo bar=\"|baz\"></foo>",
        "The `<foo bar=\"baz\">` attribute",
        "\"baz\"",
        language_options(),
    );
    assert_hover2(
        "<foo xattr=\"|xval\"></foo>",
        "`xval` value",
        "\"xval\"",
        language_options(),
    );

    assert_hover2(
        "<foo foo=\"xval\" xattr=\"|xval\"></foo>",
        "`xval` value",
        "\"xval\"",
        language_options(),
    );
}
