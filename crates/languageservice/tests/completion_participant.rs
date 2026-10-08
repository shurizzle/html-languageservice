//! Port of `src/test/completionParticipant.test.ts`.

mod support;

use std::cell::RefCell;

use html_languageservice::*;
use support::split_marker;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Actual {
    tag: String,
    attribute: String,
    value: String,
    replace_content: String,
}

struct Recorder {
    document: TextDocument,
    position: Position,
    attribute_values: RefCell<Vec<Actual>>,
    content_count: RefCell<usize>,
}

impl ICompletionParticipant for Recorder {
    fn on_html_attribute_value(&self, context: &HtmlAttributeValueContext) {
        assert_eq!(context.position, self.position);
        let replace_content = self.document.get_text_range(context.range);
        self.attribute_values.borrow_mut().push(Actual {
            tag: context.tag.clone(),
            attribute: context.attribute.clone(),
            value: context.value.clone(),
            replace_content,
        });
    }

    fn on_html_content(&self, _context: &HtmlContentContext) {
        *self.content_count.borrow_mut() += 1;
    }
}

fn prepare(value: &str) -> (TextDocument, Position, usize) {
    let (value, offset) = split_marker(value);
    let document = TextDocument::create("test://test/test.html", "html", 0, value);
    let position = document.position_at(offset);
    (document, position, offset)
}

fn test_html_attribute_values(value: &str, expected: &[(&str, &str, &str, &str)]) {
    let (document, position, _) = prepare(value);
    let recorder = std::rc::Rc::new(Recorder {
        document: document.clone(),
        position,
        attribute_values: RefCell::new(Vec::new()),
        content_count: RefCell::new(0),
    });

    struct Shared(std::rc::Rc<Recorder>);
    impl ICompletionParticipant for Shared {
        fn on_html_attribute_value(&self, context: &HtmlAttributeValueContext) {
            self.0.on_html_attribute_value(context);
        }
    }

    let mut ls = get_language_service(LanguageServiceOptions::default());
    ls.set_completion_participants(vec![Box::new(Shared(recorder.clone()))]);
    let html_doc = ls.parse_html_document(&document);
    let _ = ls.do_complete(&document, position, &html_doc, None);

    let mut actuals = recorder.attribute_values.borrow().clone();
    actuals.sort_by(|a, b| {
        let ka = format!("{}{}{}", a.tag, a.attribute, a.value);
        let kb = format!("{}{}{}", b.tag, b.attribute, b.value);
        ka.cmp(&kb)
    });
    let mut expected: Vec<Actual> = expected
        .iter()
        .map(|(tag, attribute, value, replace_content)| Actual {
            tag: tag.to_string(),
            attribute: attribute.to_string(),
            value: value.to_string(),
            replace_content: replace_content.to_string(),
        })
        .collect();
    expected.sort_by(|a, b| {
        let ka = format!("{}{}{}", a.tag, a.attribute, a.value);
        let kb = format!("{}{}{}", b.tag, b.attribute, b.value);
        ka.cmp(&kb)
    });
    assert_eq!(actuals, expected, "input {value:?}");
}

fn test_html_content(value: &str, expected_count: usize) {
    let (document, position, _) = prepare(value);
    let recorder = std::rc::Rc::new(Recorder {
        document: document.clone(),
        position,
        attribute_values: RefCell::new(Vec::new()),
        content_count: RefCell::new(0),
    });

    struct Shared(std::rc::Rc<Recorder>);
    impl ICompletionParticipant for Shared {
        fn on_html_content(&self, context: &HtmlContentContext) {
            self.0.on_html_content(context);
        }
    }

    let mut ls = get_language_service(LanguageServiceOptions::default());
    ls.set_completion_participants(vec![Box::new(Shared(recorder.clone()))]);
    let html_doc = ls.parse_html_document(&document);
    let _ = ls.do_complete(&document, position, &html_doc, None);

    assert_eq!(*recorder.content_count.borrow(), expected_count);
}

#[test]
fn on_html_attribute_value() {
    test_html_attribute_values("<|", &[]);
    test_html_attribute_values("<div |>", &[]);
    test_html_attribute_values("<div class|>", &[]);
    test_html_attribute_values("<div>|", &[]);
    test_html_attribute_values("<div>|</div>", &[]);

    test_html_attribute_values("<div class=\"|\"></div>", &[("div", "class", "", "\"\"")]);
    test_html_attribute_values("<div class=\"|f\"></div>", &[("div", "class", "", "\"f\"")]);
    test_html_attribute_values(
        "<div class=\"foo|\"></div>",
        &[("div", "class", "foo", "\"foo\"")],
    );
    test_html_attribute_values("<div class='foo'|></div>", &[("div", "class", "", "'foo'")]);
    test_html_attribute_values("<div class=|'foo'></div>", &[("div", "class", "", "'foo'")]);
    test_html_attribute_values("<div class=|foo></div>", &[("div", "class", "", "foo")]);
    test_html_attribute_values("<div class=foo|></div>", &[("div", "class", "foo", "foo")]);
    test_html_attribute_values("<div class=fo|o></div>", &[("div", "class", "fo", "foo")]);
}

#[test]
fn on_html_content() {
    test_html_content("<|", 0);
    test_html_content("<div |>", 0);
    test_html_content("<div class|>", 0);
    test_html_content("<div class=\"|\">", 0);
    test_html_content("<div class=\"foo |\">", 0);
    test_html_content("<div class=\"foo\">d|", 1);
    test_html_content("<div class=\"foo\">d|</div>", 1);
}
