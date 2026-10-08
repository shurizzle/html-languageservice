//! Port of `src/test/completion.test.ts` (and `completionUtil.ts`).
#![allow(clippy::field_reassign_with_default)]

mod support;

use html_languageservice::language_facts::data_provider::BASELINE_HIGH;
use html_languageservice::*;

#[derive(Default)]
struct Item {
    label: &'static str,
    documentation: Option<MarkupInput>,
    result_text: Option<&'static str>,
    filter_text: Option<&'static str>,
    not_available: bool,
}

impl Item {
    fn label(label: &'static str) -> Self {
        Self {
            label,
            ..Default::default()
        }
    }

    fn result(label: &'static str, result_text: &'static str) -> Self {
        Self {
            label,
            result_text: Some(result_text),
            ..Default::default()
        }
    }

    fn unavailable(label: &'static str) -> Self {
        Self {
            label,
            not_available: true,
            ..Default::default()
        }
    }
}

fn assert_completion(completions: &CompletionList, expected: &Item, document: &TextDocument) {
    let matches: Vec<&CompletionItem> = completions
        .items
        .iter()
        .filter(|c| c.label == expected.label)
        .collect();
    if expected.not_available {
        assert_eq!(
            matches.len(),
            0,
            "{} should not exist in results",
            expected.label
        );
        return;
    }

    assert_eq!(
        matches.len(),
        1,
        "{} should only exist once: Actual: {}",
        expected.label,
        completions
            .items
            .iter()
            .map(|c| c.label.clone())
            .collect::<Vec<_>>()
            .join(", ")
    );
    let m = matches[0];
    if let Some(expected_doc) = &expected.documentation {
        match expected_doc {
            MarkupInput::String(s) => {
                assert_eq!(m.documentation, Some(MarkupInput::String(s.clone())))
            }
            MarkupInput::Markup(mc) => {
                assert_eq!(
                    m.documentation.as_ref().map(|d| d.as_markup().value),
                    Some(mc.value.clone())
                );
            }
        }
    }
    if let Some(result_text) = expected.result_text
        && let Some(edit) = &m.text_edit
    {
        let applied = apply_edits(document, std::slice::from_ref(edit));
        assert_eq!(applied, result_text);
    }
    if let Some(filter_text) = expected.filter_text {
        assert_eq!(m.filter_text.as_deref(), Some(filter_text));
    }
}

fn completion_for_with(
    value: &str,
    count: Option<usize>,
    items: &[Item],
    settings: Option<&CompletionConfiguration>,
    ls_options: LanguageServiceOptions,
) {
    let (value, offset) = support::split_marker(value);
    let ls = get_language_service(ls_options);
    let document = TextDocument::create("test://test/test.html", "html", 0, value);
    let position = document.position_at(offset);
    let html_doc = ls.parse_html_document(&document);
    let list = ls.do_complete(&document, position, &html_doc, settings);

    // no duplicate labels
    let mut labels: Vec<&str> = list.items.iter().map(|i| i.label.as_str()).collect();
    labels.sort_unstable();
    for pair in labels.windows(2) {
        assert_ne!(
            pair[0], pair[1],
            "Duplicate label {} in {:?}",
            pair[0], labels
        );
    }

    if let Some(count) = count {
        assert_eq!(list.items.len(), count);
    }
    for item in items {
        assert_completion(&list, item, &document);
    }
}

/// `testCompletionFor(value, { items })` with default settings.
fn completion_for(value: &str, items: &[Item]) {
    completion_for_with(value, None, items, None, LanguageServiceOptions::default());
}

fn count_for(value: &str, count: usize) {
    completion_for_with(
        value,
        Some(count),
        &[],
        None,
        LanguageServiceOptions::default(),
    );
}

fn completion_for_settings(value: &str, items: &[Item], settings: &CompletionConfiguration) {
    completion_for_with(
        value,
        None,
        items,
        Some(settings),
        LanguageServiceOptions::default(),
    );
}

fn quote_completion(
    value: &str,
    expected: Option<&str>,
    options: Option<&CompletionConfiguration>,
) {
    let (value, offset) = support::split_marker(value);
    let ls = get_language_service(LanguageServiceOptions::default());
    let document = TextDocument::create("test://test/test.html", "html", 0, value);
    let position = document.position_at(offset);
    let html_doc = ls.parse_html_document(&document);
    let actual = ls.do_quote_complete(&document, position, &html_doc, options);
    assert_eq!(actual.as_deref(), expected);
}

fn tag_completion(value: &str, expected: Option<&str>) {
    let (value, offset) = support::split_marker(value);
    let ls = get_language_service(LanguageServiceOptions::default());
    let document = TextDocument::create("test://test/test.html", "html", 0, value);
    let position = document.position_at(offset);
    let html_doc = ls.parse_html_document(&document);
    let actual = ls.do_tag_complete(&document, position, &html_doc);
    assert_eq!(actual.as_deref(), expected);
}

#[test]
fn complete() {
    completion_for(
        "<|",
        &[
            Item::result("!DOCTYPE", "<!DOCTYPE html>"),
            Item::result("iframe", "<iframe"),
            Item::result("h1", "<h1"),
            Item::result("div", "<div"),
        ],
    );

    completion_for(
        "\n<|",
        &[
            Item::unavailable("!DOCTYPE"),
            Item::label("iframe"),
            Item::label("h1"),
            Item::label("div"),
        ],
    );

    completion_for(
        "< |",
        &[
            Item::result("iframe", "<iframe"),
            Item::result("h1", "<h1"),
            Item::result("div", "<div"),
        ],
    );

    completion_for(
        "<h|",
        &[
            Item::result("html", "<html"),
            Item::result("h1", "<h1"),
            Item::result("header", "<header"),
        ],
    );

    completion_for("<input|", &[Item::result("input", "<input")]);
    completion_for("<inp|ut", &[Item::result("input", "<input")]);
    completion_for("<|inp", &[Item::result("input", "<input")]);
    completion_for(
        "<input |",
        &[
            Item::result("type", "<input type=\"$1\""),
            Item::result("style", "<input style=\"$1\""),
            Item::result("onmousemove", "<input onmousemove=\"$1\""),
        ],
    );

    completion_for(
        "<input t|",
        &[
            Item::result("type", "<input type=\"$1\""),
            Item::result("tabindex", "<input tabindex=\"$1\""),
        ],
    );

    completion_for(
        "<input t|ype",
        &[
            Item::result("type", "<input type=\"$1\""),
            Item::result("tabindex", "<input tabindex=\"$1\""),
        ],
    );

    completion_for(
        "<input t|ype=\"text\"",
        &[
            Item::result("type", "<input type=\"text\""),
            Item::result("tabindex", "<input tabindex=\"text\""),
        ],
    );

    completion_for(
        "<input type=\"text\" |",
        &[
            Item::result("style", "<input type=\"text\" style=\"$1\""),
            Item::unavailable("type"),
            Item::result("size", "<input type=\"text\" size=\"$1\""),
        ],
    );

    completion_for(
        "<input | type=\"text\"",
        &[
            Item::result("style", "<input style=\"$1\" type=\"text\""),
            Item::unavailable("type"),
            Item::result("size", "<input size=\"$1\" type=\"text\""),
        ],
    );

    completion_for(
        "<input type=\"text\" type=\"number\" |",
        &[
            Item::result("style", "<input type=\"text\" type=\"number\" style=\"$1\""),
            Item::unavailable("type"),
            Item::result("size", "<input type=\"text\" type=\"number\" size=\"$1\""),
        ],
    );

    completion_for(
        "<input type=\"text\" s|",
        &[
            Item::result("style", "<input type=\"text\" style=\"$1\""),
            Item::result("src", "<input type=\"text\" src=\"$1\""),
            Item::result("size", "<input type=\"text\" size=\"$1\""),
        ],
    );

    completion_for(
        "<input di| type=\"text\"",
        &[
            Item::result("disabled", "<input disabled type=\"text\""),
            Item::result("dir", "<input dir=\"$1\" type=\"text\""),
        ],
    );

    completion_for(
        "<input disabled | type=\"text\"",
        &[
            Item::result("dir", "<input disabled dir=\"$1\" type=\"text\""),
            Item::result("style", "<input disabled style=\"$1\" type=\"text\""),
        ],
    );

    completion_for(
        "<input type=|",
        &[
            Item::result("text", "<input type=\"text\""),
            Item::result("checkbox", "<input type=\"checkbox\""),
        ],
    );
    completion_for(
        "<input type=\"c|",
        &[
            Item::result("color", "<input type=\"color"),
            Item::result("checkbox", "<input type=\"checkbox"),
        ],
    );
    completion_for(
        "<input type=\"|",
        &[
            Item::result("color", "<input type=\"color"),
            Item::result("checkbox", "<input type=\"checkbox"),
        ],
    );
    completion_for(
        "<input type= |",
        &[
            Item::result("color", "<input type= \"color\""),
            Item::result("checkbox", "<input type= \"checkbox\""),
        ],
    );
    completion_for(
        "<input src=\"c\" type=\"color|\" ",
        &[Item::result("color", "<input src=\"c\" type=\"color\" ")],
    );
    completion_for(
        "<iframe sandbox=\"allow-forms |",
        &[Item::result(
            "allow-modals",
            "<iframe sandbox=\"allow-forms allow-modals",
        )],
    );
    completion_for(
        "<iframe sandbox=\"allow-forms allow-modals|",
        &[Item::result(
            "allow-modals",
            "<iframe sandbox=\"allow-forms allow-modals",
        )],
    );
    completion_for(
        "<iframe sandbox=\"allow-forms all|\"",
        &[Item::result(
            "allow-modals",
            "<iframe sandbox=\"allow-forms allow-modals\"",
        )],
    );
    completion_for(
        "<iframe sandbox=\"allow-forms a|llow-modals \"",
        &[Item::result(
            "allow-modals",
            "<iframe sandbox=\"allow-forms allow-modals \"",
        )],
    );
    completion_for(
        "<input src=\"c\" type=color| ",
        &[Item::result("color", "<input src=\"c\" type=\"color\" ")],
    );

    // unclosed string literals should not extend the replace range past the cursor
    completion_for(
        "<th><input type=\"che|</th>",
        &[Item::result("checkbox", "<th><input type=\"checkbox</th>")],
    );
    completion_for(
        "<th><input type=\"che|</th><td></td>",
        &[Item::result(
            "checkbox",
            "<th><input type=\"checkbox</th><td></td>",
        )],
    );
    completion_for(
        "<th><input type=\"che|</th><p title=\"later\"></p>",
        &[Item::result(
            "checkbox",
            "<th><input type=\"checkbox</th><p title=\"later\"></p>",
        )],
    );
    completion_for(
        "<th><input type=\"che|</th>\n<p title=\"later\"></p>",
        &[Item::result(
            "checkbox",
            "<th><input type=\"checkbox</th>\n<p title=\"later\"></p>",
        )],
    );
    completion_for(
        "<th><input type=\"che|\n</th>",
        &[Item::result(
            "checkbox",
            "<th><input type=\"checkbox\n</th>",
        )],
    );

    completion_for(
        "<div dir=|></div>",
        &[
            Item::result("ltr", "<div dir=\"ltr\"></div>"),
            Item::result("rtl", "<div dir=\"rtl\"></div>"),
        ],
    );
    completion_for(
        "<ul><|>",
        &[
            Item::result("/ul", "<ul></ul>"),
            Item::result("li", "<ul><li>"),
        ],
    );
    completion_for(
        "<ul><li><|",
        &[
            Item::result("/li", "<ul><li></li>"),
            Item::result("a", "<ul><li><a"),
        ],
    );
    completion_for("<goo></|>", &[Item::result("/goo", "<goo></goo>")]);
    completion_for("<foo></f|", &[Item::result("/foo", "<foo></foo>")]);
    completion_for("<foo></f|o", &[Item::result("/foo", "<foo></foo>")]);
    completion_for("<foo></|fo", &[Item::result("/foo", "<foo></foo>")]);
    completion_for("<foo></ |>", &[Item::result("/foo", "<foo></foo>")]);
    completion_for("<span></ s|", &[Item::result("/span", "<span></span>")]);
    completion_for("<li><br></ |>", &[Item::result("/li", "<li><br></li>")]);
    count_for("<li/|>", 0);
    count_for("  <div/|   ", 0);
    completion_for(
        "<foo><br/></ f|>",
        &[Item::result("/foo", "<foo><br/></foo>")],
    );
    completion_for("<li><div/></|", &[Item::result("/li", "<li><div/></li>")]);
    count_for("<li><br/|>", 0);
    count_for("<li><br>a/|", 0);

    completion_for(
        "<foo><bar></bar></|   ",
        &[Item::result("/foo", "<foo><bar></bar></foo>   ")],
    );
    completion_for(
        "<div>\n  <form>\n    <div>\n      <label></label>\n      <|\n    </div>\n  </form></div>",
        &[
            Item::result(
                "span",
                "<div>\n  <form>\n    <div>\n      <label></label>\n      <span\n    </div>\n  </form></div>",
            ),
            Item::result(
                "/div",
                "<div>\n  <form>\n    <div>\n      <label></label>\n    </div>\n    </div>\n  </form></div>",
            ),
        ],
    );
    completion_for(
        "<body><div><div></div></div></|  >",
        &[Item::result(
            "/body",
            "<body><div><div></div></div></body  >",
        )],
    );
    completion_for(
        "<body>\n  <div>\n    </|",
        &[Item::result("/div", "<body>\n  <div>\n  </div>")],
    );
    completion_for(
        "<div><a hre|</div>",
        &[Item::result("href", "<div><a href=\"$1\"</div>")],
    );
    completion_for(
        "<a><b>foo</b><|f>",
        &[
            Item::result("/a", "<a><b>foo</b></a>"),
            Item::unavailable("/f"),
        ],
    );
    completion_for(
        "<a><b>foo</b><| bar.",
        &[
            Item::result("/a", "<a><b>foo</b></a> bar."),
            Item::unavailable("/bar"),
        ],
    );
    completion_for(
        "<div><h1><br><span></span><img></| </h1></div>",
        &[Item::result(
            "/h1",
            "<div><h1><br><span></span><img></h1> </h1></div>",
        )],
    );
    completion_for("<div>|", &[Item::result("</div>", "<div>$0</div>")]);
    {
        let mut s = CompletionConfiguration::default();
        s.hide_auto_complete_proposals = true;
        completion_for_settings("<div>|", &[Item::unavailable("</div>")], &s);
    }
    completion_for("<div d|", &[Item::result("data-", "<div data-$1=\"$2\"")]);
    completion_for(
        "<div no-data-test=\"no-data\" d|",
        &[Item::unavailable("no-data-test")],
    );
    completion_for(
        "<div data-custom=\"test\"><div d|",
        &[
            Item::result("data-", "<div data-custom=\"test\"><div data-$1=\"$2\""),
            Item::result(
                "data-custom",
                "<div data-custom=\"test\"><div data-custom=\"$1\"",
            ),
        ],
    );
    completion_for(
        "<div data-custom=\"test\"><div data-custom-two=\"2\"></div></div>\n <div d|",
        &[
            Item::result(
                "data-",
                "<div data-custom=\"test\"><div data-custom-two=\"2\"></div></div>\n <div data-$1=\"$2\"",
            ),
            Item::result(
                "data-custom",
                "<div data-custom=\"test\"><div data-custom-two=\"2\"></div></div>\n <div data-custom=\"$1\"",
            ),
            Item::result(
                "data-custom-two",
                "<div data-custom=\"test\"><div data-custom-two=\"2\"></div></div>\n <div data-custom-two=\"$1\"",
            ),
        ],
    );
    completion_for(
        "<body data-ng-app=\"\"><div id=\"first\" data-ng-include=\" 'firstdoc.html' \"></div><div id=\"second\" inc|></div></body>",
        &[Item::result(
            "data-ng-include",
            "<body data-ng-app=\"\"><div id=\"first\" data-ng-include=\" 'firstdoc.html' \"></div><div id=\"second\" data-ng-include=\"$1\"></div></body>",
        )],
    );
}

#[test]
fn references() {
    let doc = format!(
        "The div element has no special meaning at all. It represents its children. It can be used with the class, lang, and title attributes to mark up semantics common to a group of consecutive elements.\n\n![Baseline icon]({BASELINE_HIGH}) _Widely available across major browsers (Baseline since 2015)_\n\n[MDN Reference](https://developer.mozilla.org/docs/Web/HTML/Reference/Elements/div)"
    );

    completion_for(
        "<d|",
        &[Item {
            label: "div",
            result_text: Some("<div"),
            documentation: Some(MarkupInput::Markup(MarkupContent::new(
                MarkupKind::Markdown,
                doc,
            ))),
            ..Default::default()
        }],
    );
}

#[test]
fn case_sensitivity() {
    completion_for(
        "<LI></|",
        &[Item::result("/LI", "<LI></LI>"), Item::unavailable("/li")],
    );
    completion_for("<lI></|", &[Item::result("/lI", "<lI></lI>")]);
    completion_for("<iNpUt |", &[Item::result("type", "<iNpUt type=\"$1\"")]);
    completion_for(
        "<INPUT TYPE=|",
        &[Item::result("color", "<INPUT TYPE=\"color\"")],
    );
    completion_for("<dIv>|", &[Item::result("</dIv>", "<dIv>$0</dIv>")]);
}

#[test]
fn handlebar_completion() {
    completion_for(
        "<script id=\"entry-template\" type=\"text/x-handlebars-template\"> <| </script>",
        &[Item::result(
            "div",
            "<script id=\"entry-template\" type=\"text/x-handlebars-template\"> <div </script>",
        )],
    );
}

#[test]
fn support_script_type_text_html() {
    completion_for(
        "<script id=\"html-template\" type=\"text/html\"> <| </script>",
        &[Item::result(
            "div",
            "<script id=\"html-template\" type=\"text/html\"> <div </script>",
        )],
    );
}

#[test]
fn complete_aria() {
    const ARIA: &[&str] = &[
        "aria-activedescendant",
        "aria-atomic",
        "aria-autocomplete",
        "aria-busy",
        "aria-checked",
        "aria-colcount",
        "aria-colindex",
        "aria-colspan",
        "aria-controls",
        "aria-current",
        "aria-describedby",
        "aria-disabled",
        "aria-dropeffect",
        "aria-errormessage",
        "aria-expanded",
        "aria-flowto",
        "aria-grabbed",
        "aria-haspopup",
        "aria-hidden",
        "aria-invalid",
        "aria-label",
        "aria-labelledby",
        "aria-level",
        "aria-live",
        "aria-modal",
        "aria-multiline",
        "aria-multiselectable",
        "aria-orientation",
        "aria-owns",
        "aria-placeholder",
        "aria-posinset",
        "aria-pressed",
        "aria-readonly",
        "aria-relevant",
        "aria-required",
        "aria-roledescription",
        "aria-rowcount",
        "aria-rowindex",
        "aria-rowspan",
        "aria-selected",
        "aria-setsize",
        "aria-sort",
        "aria-valuemax",
        "aria-valuemin",
        "aria-valuenow",
        "aria-valuetext",
    ];
    let items: Vec<Item> = ARIA.iter().map(|l| Item::label(l)).collect();

    completion_for("<div  |> </div >", &items);
    completion_for("<span  |> </span >", &items);
    completion_for("<input  |> </input >", &items);
}

#[test]
fn settings() {
    {
        let mut s = CompletionConfiguration::default();
        s.disable_provider("html5");
        completion_for_settings("<|", &[Item::unavailable("div")], &s);
    }
    {
        let mut s = CompletionConfiguration::default();
        s.attribute_default_value = Some(AttributeDefaultValue::DoubleQuotes);
        completion_for_settings(
            "<div clas|",
            &[Item::result("class", "<div class=\"$1\"")],
            &s,
        );
    }
    {
        let mut s = CompletionConfiguration::default();
        s.attribute_default_value = Some(AttributeDefaultValue::SingleQuotes);
        completion_for_settings(
            "<div clas|",
            &[Item::result("class", "<div class='$1'")],
            &s,
        );
    }
    {
        let mut s = CompletionConfiguration::default();
        s.attribute_default_value = Some(AttributeDefaultValue::Empty);
        completion_for_settings("<div clas|", &[Item::result("class", "<div class=$1")], &s);
    }
}

#[test]
fn do_quote_complete() {
    quote_completion("<a foo=|", Some("\"$1\""), None);
    {
        let mut s = CompletionConfiguration::default();
        s.attribute_default_value = Some(AttributeDefaultValue::SingleQuotes);
        quote_completion("<a foo=|", Some("'$1'"), Some(&s));
    }
    {
        let mut s = CompletionConfiguration::default();
        s.attribute_default_value = Some(AttributeDefaultValue::Empty);
        quote_completion("<a foo=|", None, Some(&s));
    }
    quote_completion("<a foo=|=", None, None);
    quote_completion("<a foo=|\"bar\"", None, None);
    quote_completion("<a foo=|></a>", Some("\"$1\""), None);
    quote_completion("<a foo=\"bar=|\"", None, None);
    quote_completion("<a baz=| foo=\"bar\">", Some("\"$1\""), None);
    quote_completion("<a>< foo=| /a>", None, None);
    quote_completion("<a></ foo=| a>", None, None);
    quote_completion("<a foo=\"bar\" \n baz=| ></a>", Some("\"$1\""), None);
}

#[test]
fn do_tag_complete() {
    tag_completion("<div>|", Some("$0</div>"));
    tag_completion("<div>|</div>", None);
    tag_completion("<div class=\"\">|", Some("$0</div>"));
    tag_completion("<img>|", None);
    tag_completion("<div><br></|", Some("div>"));
    tag_completion("<div><br><span></span></|", Some("div>"));
    tag_completion(
        "<div><h1><br><span></span><img></| </h1></div>",
        Some("h1>"),
    );
    tag_completion(
        "<ng-template><td><ng-template></|   </td> </ng-template>",
        Some("ng-template>"),
    );
    tag_completion("<div><br></|>", Some("div"));
}

#[test]
fn character_entities() {
    completion_for(
        "<div>&|",
        &[
            Item::result("&hookrightarrow;", "<div>&hookrightarrow;"),
            Item::result("&plus;", "<div>&plus;"),
        ],
    );
    completion_for(
        "<div>Hello&|",
        &[Item::result(
            "&ZeroWidthSpace;",
            "<div>Hello&ZeroWidthSpace;",
        )],
    );
    completion_for(
        "<div>Hello&gt|",
        &[Item::result("&gtrdot;", "<div>Hello&gtrdot;")],
    );
    completion_for(
        "<div class=\"&g|\"",
        &[Item::result("&grave;", "<div class=\"&grave;\"")],
    );
    completion_for(
        "<div class=&d|",
        &[Item::result("&duarr;", "<div class=&duarr;")],
    );
    completion_for("<div &d|", &[Item::unavailable("&duarr;")]);
    completion_for("<div&d|", &[Item::unavailable("&duarr;")]);
}

#[test]
fn filter_text_for_close_tag_suggestion() {
    completion_for(
        "<div> <| </div>",
        &[Item {
            label: "/div",
            filter_text: Some("/div"),
            ..Default::default()
        }],
    );
    completion_for(
        "<div>\n  <|\n</div>",
        &[Item {
            label: "/div",
            filter_text: Some("  </div"),
            ..Default::default()
        }],
    );
}

#[test]
fn non_matching_close_tag_suggestion() {
    completion_for("</|", &[Item::result("/a", "</a>")]);
    completion_for("<div></div></|", &[Item::result("/a", "<div></div></a>")]);
}

#[test]
fn hide_end_tag_suggestions_setting() {
    completion_for("<body>\n<|", &[Item::label("/body"), Item::label("div")]);

    {
        let mut s = CompletionConfiguration::default();
        s.hide_end_tag_suggestions = true;
        completion_for_settings(
            "<body>\n<|",
            &[Item::unavailable("/body"), Item::label("div")],
            &s,
        );
    }
    {
        let mut s = CompletionConfiguration::default();
        s.hide_end_tag_suggestions = true;
        s.disable_provider("html5");
        completion_for_settings(
            "<body>\n<|",
            &[Item::unavailable("/body"), Item::unavailable("div")],
            &s,
        );
    }
    {
        let mut s = CompletionConfiguration::default();
        s.hide_end_tag_suggestions = true;
        completion_for_settings("<div>\n  <|\n</div>", &[Item::unavailable("/div")], &s);
    }
    {
        let mut s = CompletionConfiguration::default();
        s.hide_end_tag_suggestions = true;
        completion_for_settings("</|", &[Item::unavailable("/a")], &s);
    }
}
