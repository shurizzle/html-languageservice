//! Port of `src/test/symbols.test.ts`.

mod support;

use html_languageservice::*;
use support::doc;

const TEST_URI: &str = "test://test/test.html";

fn test_symbol_informations_for(value: &str, expected: Vec<SymbolInformation>) {
    let ls = get_language_service(LanguageServiceOptions::default());
    let document = doc(TEST_URI, "html", value);
    let html_doc = ls.parse_html_document(&document);
    let symbols = ls.find_document_symbols(&document, &html_doc);
    assert_eq!(symbols, expected);
}

fn test_document_symbols_for(value: &str, expected: Vec<DocumentSymbol>) {
    let ls = get_language_service(LanguageServiceOptions::default());
    let document = doc(TEST_URI, "html", value);
    let html_doc = ls.parse_html_document(&document);
    let symbols = ls.find_document_symbols2(&document, &html_doc);
    assert_eq!(symbols, expected);
}

fn info(name: &str, container: &str, range: Range) -> SymbolInformation {
    SymbolInformation {
        name: name.to_string(),
        kind: SymbolKind::Field,
        location: Location::create(TEST_URI, range),
        container_name: container.to_string(),
    }
}

fn sym(name: &str, range: Range, children: Option<Vec<DocumentSymbol>>) -> DocumentSymbol {
    DocumentSymbol::create(name, SymbolKind::Field, range, range, children)
}

#[test]
fn simple() {
    test_symbol_informations_for(
        "<div></div>",
        vec![info("div", "", Range::create(0, 0, 0, 11))],
    );
    test_symbol_informations_for(
        r#"<div><input checked id="test" class="checkbox"></div>"#,
        vec![
            info("div", "", Range::create(0, 0, 0, 53)),
            info("input#test.checkbox", "div", Range::create(0, 5, 0, 47)),
        ],
    );

    test_document_symbols_for(
        "<div></div>",
        vec![sym("div", Range::create(0, 0, 0, 11), None)],
    );
    test_document_symbols_for(
        r#"<div><input checked id="test" class="checkbox"></div>"#,
        vec![sym(
            "div",
            Range::create(0, 0, 0, 53),
            Some(vec![sym(
                "input#test.checkbox",
                Range::create(0, 5, 0, 47),
                None,
            )]),
        )],
    );
}

#[test]
fn id_and_classes() {
    let content =
        r#"<html id='root'><body id="Foo" class="bar"><div class="a b"></div></body></html>"#;

    test_symbol_informations_for(
        content,
        vec![
            info("html#root", "", Range::create(0, 0, 0, 80)),
            info("body#Foo.bar", "html#root", Range::create(0, 16, 0, 73)),
            info("div.a.b", "body#Foo.bar", Range::create(0, 43, 0, 66)),
        ],
    );

    test_document_symbols_for(
        content,
        vec![sym(
            "html#root",
            Range::create(0, 0, 0, 80),
            Some(vec![sym(
                "body#Foo.bar",
                Range::create(0, 16, 0, 73),
                Some(vec![sym("div.a.b", Range::create(0, 43, 0, 66), None)]),
            )]),
        )],
    );
}

#[test]
fn self_closing() {
    let content = r#"<html><br id="Foo"><br id=Bar></html>"#;

    test_symbol_informations_for(
        content,
        vec![
            info("html", "", Range::create(0, 0, 0, 37)),
            info("br#Foo", "html", Range::create(0, 6, 0, 19)),
            info("br#Bar", "html", Range::create(0, 19, 0, 30)),
        ],
    );

    test_document_symbols_for(
        content,
        vec![sym(
            "html",
            Range::create(0, 0, 0, 37),
            Some(vec![
                sym("br#Foo", Range::create(0, 6, 0, 19), None),
                sym("br#Bar", Range::create(0, 19, 0, 30), None),
            ]),
        )],
    );
}

#[test]
fn no_attrib() {
    let content = "<html><body><div></div></body></html>";

    test_symbol_informations_for(
        content,
        vec![
            info("html", "", Range::create(0, 0, 0, 37)),
            info("body", "html", Range::create(0, 6, 0, 30)),
            info("div", "body", Range::create(0, 12, 0, 23)),
        ],
    );

    test_document_symbols_for(
        content,
        vec![sym(
            "html",
            Range::create(0, 0, 0, 37),
            Some(vec![sym(
                "body",
                Range::create(0, 6, 0, 30),
                Some(vec![sym("div", Range::create(0, 12, 0, 23), None)]),
            )]),
        )],
    );
}
