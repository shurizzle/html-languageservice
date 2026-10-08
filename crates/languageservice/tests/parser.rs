//! Port of `src/test/parser.test.ts`.

use html_languageservice::parser::{HtmlDocument, NodeId, ParseOptions, parse};

fn void_elements() -> Vec<String> {
    let mut v = vec![
        "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param",
        "source", "track", "wbr",
    ];
    v.sort();
    v.iter().map(|s| s.to_string()).collect()
}

fn parse_text(text: &str, create_nodes_for_orphan_end_tags: bool) -> HtmlDocument {
    parse(
        text,
        &void_elements(),
        ParseOptions {
            create_nodes_for_orphan_end_tags,
        },
    )
}

#[derive(Debug, PartialEq)]
struct Exp {
    tag: Option<&'static str>,
    start: usize,
    end: usize,
    end_tag_start: Option<usize>,
    closed: bool,
    children: Vec<Exp>,
}

fn to_exp(doc: &HtmlDocument, id: NodeId) -> Exp {
    let node = doc.node(id);
    Exp {
        tag: node
            .tag
            .as_deref()
            .map(|s| Box::leak(s.to_string().into_boxed_str()) as &str),
        start: node.start,
        end: node.end,
        end_tag_start: node.end_tag_start,
        closed: node.closed,
        children: node.children.iter().map(|c| to_exp(doc, *c)).collect(),
    }
}

fn actual_roots(doc: &HtmlDocument) -> Vec<Exp> {
    doc.roots().iter().map(|id| to_exp(doc, *id)).collect()
}

fn assert_document(input: &str, expected: Vec<Exp>, orphan: bool) {
    let doc = parse_text(input, orphan);
    assert_eq!(actual_roots(&doc), expected, "input: {input:?}");
}

fn assert_node_before(input: &str, offset: usize, expected_tag: Option<&str>) {
    let doc = parse_text(input, false);
    let id = doc.find_node_before(offset);
    assert_eq!(
        doc.node(id).tag.as_deref(),
        expected_tag,
        "offset {offset} in {input:?}"
    );
}

#[derive(Debug, PartialEq)]
struct AttrExp {
    tag: Option<&'static str>,
    attributes: Vec<(String, Option<String>)>,
    children: Vec<AttrExp>,
}

fn to_attr_exp(doc: &HtmlDocument, id: NodeId) -> AttrExp {
    let node = doc.node(id);
    AttrExp {
        tag: node
            .tag
            .as_deref()
            .map(|s| Box::leak(s.to_string().into_boxed_str()) as &str),
        attributes: node.attributes.clone(),
        children: node.children.iter().map(|c| to_attr_exp(doc, *c)).collect(),
    }
}

fn assert_attributes(input: &str, expected: Vec<AttrExp>) {
    let doc = parse_text(input, false);
    let actual: Vec<AttrExp> = doc
        .roots()
        .iter()
        .map(|id| to_attr_exp(&doc, *id))
        .collect();
    assert_eq!(actual, expected, "input: {input:?}");
}

// --- helpers to build expected trees tersely ---

#[allow(clippy::too_many_arguments)]
const fn node(
    tag: Option<&'static str>,
    start: usize,
    end: usize,
    end_tag_start: Option<usize>,
    closed: bool,
    children: Vec<Exp>,
) -> Exp {
    Exp {
        tag,
        start,
        end,
        end_tag_start,
        closed,
        children,
    }
}

fn attr(
    tag: Option<&'static str>,
    attributes: Vec<(&str, Option<&str>)>,
    children: Vec<AttrExp>,
) -> AttrExp {
    AttrExp {
        tag,
        attributes: attributes
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.map(|s| s.to_string())))
            .collect(),
        children,
    }
}

#[test]
fn simple() {
    assert_document(
        "<html></html>",
        vec![node(Some("html"), 0, 13, Some(6), true, vec![])],
        false,
    );
    assert_document(
        "<html><body></body></html>",
        vec![node(
            Some("html"),
            0,
            26,
            Some(19),
            true,
            vec![node(Some("body"), 6, 19, Some(12), true, vec![])],
        )],
        false,
    );
    assert_document(
        "<html><head></head><body></body></html>",
        vec![node(
            Some("html"),
            0,
            39,
            Some(32),
            true,
            vec![
                node(Some("head"), 6, 19, Some(12), true, vec![]),
                node(Some("body"), 19, 32, Some(25), true, vec![]),
            ],
        )],
        false,
    );
}

#[test]
fn self_close() {
    assert_document(
        "<br/>",
        vec![node(Some("br"), 0, 5, None, true, vec![])],
        false,
    );
    assert_document(
        "<div><br/><span></span></div>",
        vec![node(
            Some("div"),
            0,
            29,
            Some(23),
            true,
            vec![
                node(Some("br"), 5, 10, None, true, vec![]),
                node(Some("span"), 10, 23, Some(16), true, vec![]),
            ],
        )],
        false,
    );
}

#[test]
fn empty_tag() {
    assert_document(
        "<meta>",
        vec![node(Some("meta"), 0, 6, None, true, vec![])],
        false,
    );
    assert_document(
        "<div><input type=\"button\"><span><br><br></span></div>",
        vec![node(
            Some("div"),
            0,
            53,
            Some(47),
            true,
            vec![
                node(Some("input"), 5, 26, None, true, vec![]),
                node(
                    Some("span"),
                    26,
                    47,
                    Some(40),
                    true,
                    vec![
                        node(Some("br"), 32, 36, None, true, vec![]),
                        node(Some("br"), 36, 40, None, true, vec![]),
                    ],
                ),
            ],
        )],
        false,
    );
}

#[test]
fn missing_tags() {
    assert_document("</meta>", vec![], false);
    assert_document(
        "<div></div></div>",
        vec![node(Some("div"), 0, 11, Some(5), true, vec![])],
        false,
    );
    assert_document(
        "<div><div></div>",
        vec![node(
            Some("div"),
            0,
            16,
            None,
            false,
            vec![node(Some("div"), 5, 16, Some(10), true, vec![])],
        )],
        false,
    );
    assert_document(
        "<title><div></title>",
        vec![node(
            Some("title"),
            0,
            20,
            Some(12),
            true,
            vec![node(Some("div"), 7, 12, None, false, vec![])],
        )],
        false,
    );
    assert_document(
        "<h1><div><span></h1>",
        vec![node(
            Some("h1"),
            0,
            20,
            Some(15),
            true,
            vec![node(
                Some("div"),
                4,
                15,
                None,
                false,
                vec![node(Some("span"), 9, 15, None, false, vec![])],
            )],
        )],
        false,
    );
}

#[test]
fn closing_tag_without_opening_opt_in() {
    assert_document(
        "</div>",
        vec![node(Some("div"), 0, 6, Some(0), true, vec![])],
        true,
    );
    assert_document(
        "<div></span></div>",
        vec![node(
            Some("div"),
            0,
            18,
            Some(12),
            true,
            vec![node(Some("span"), 5, 12, Some(5), true, vec![])],
        )],
        true,
    );
    assert_document(
        "</a></b>",
        vec![
            node(Some("a"), 0, 4, Some(0), true, vec![]),
            node(Some("b"), 4, 8, Some(4), true, vec![]),
        ],
        true,
    );
}

#[test]
fn closing_tag_without_opening_default_discards() {
    assert_document("</div>", vec![], false);
    assert_document("</a></b>", vec![], false);
    assert_document(
        "<div></span></div>",
        vec![node(Some("div"), 0, 18, Some(12), true, vec![])],
        false,
    );
}

#[test]
fn missing_brackets() {
    assert_document(
        "<div><div</div>",
        vec![node(
            Some("div"),
            0,
            15,
            Some(9),
            true,
            vec![node(Some("div"), 5, 9, None, false, vec![])],
        )],
        false,
    );
    assert_document(
        "<div><div\n</div>",
        vec![node(
            Some("div"),
            0,
            16,
            Some(10),
            true,
            vec![node(Some("div"), 5, 10, None, false, vec![])],
        )],
        false,
    );
    assert_document(
        "<div><div></div</div>",
        vec![node(
            Some("div"),
            0,
            21,
            Some(15),
            true,
            vec![node(Some("div"), 5, 15, Some(10), true, vec![])],
        )],
        false,
    );
}

#[test]
fn find_node_before() {
    let s = "<div><input type=\"button\"><span><br><hr></span></div>";
    assert_node_before(s, 0, None);
    assert_node_before(s, 1, Some("div"));
    assert_node_before(s, 5, Some("div"));
    assert_node_before(s, 6, Some("input"));
    assert_node_before(s, 25, Some("input"));
    assert_node_before(s, 26, Some("input"));
    assert_node_before(s, 27, Some("span"));
    assert_node_before(s, 32, Some("span"));
    assert_node_before(s, 33, Some("br"));
    assert_node_before(s, 36, Some("br"));
    assert_node_before(s, 37, Some("hr"));
    assert_node_before(s, 40, Some("hr"));
    assert_node_before(s, 41, Some("hr"));
    assert_node_before(s, 42, Some("hr"));
    assert_node_before(s, 47, Some("span"));
    assert_node_before(s, 48, Some("span"));
    assert_node_before(s, 52, Some("span"));
    assert_node_before(s, 53, Some("div"));
}

#[test]
fn find_node_before_incomplete_node() {
    let s = "<div><span><br></div>";
    assert_node_before(s, 15, Some("br"));
    assert_node_before(s, 18, Some("br"));
    assert_node_before(s, 21, Some("div"));
}

#[test]
fn attributes() {
    let s = "<div class=\"these are my-classes\" id=\"test\"><span aria-describedby=\"test\"></span></div>";
    assert_attributes(
        s,
        vec![attr(
            Some("div"),
            vec![
                ("class", Some("\"these are my-classes\"")),
                ("id", Some("\"test\"")),
            ],
            vec![attr(
                Some("span"),
                vec![("aria-describedby", Some("\"test\""))],
                vec![],
            )],
        )],
    );
}

#[test]
fn attributes_without_value() {
    let s = "<div checked id=\"test\"></div>";
    assert_attributes(
        s,
        vec![attr(
            Some("div"),
            vec![("checked", None), ("id", Some("\"test\""))],
            vec![],
        )],
    );
}
