//! Port of `src/test/links.test.ts`.

mod support;

use html_languageservice::*;
use support::TestDocumentContext;

fn test_link_creation(model_url: &str, token_content: &str, expected: Option<&str>) {
    let lang_id = if model_url.ends_with(".hbs") {
        "handlebars"
    } else {
        "html"
    };
    let document = TextDocument::create(
        model_url,
        lang_id,
        0,
        format!("<a href=\"{token_content}\">"),
    );
    let ls = get_language_service(LanguageServiceOptions::default());
    let links = ls.find_document_links(&document, &TestDocumentContext);
    let actual = links.first().and_then(|l| l.target.clone());
    assert_eq!(actual.as_deref(), expected, "token {token_content:?}");
}

#[derive(Debug, PartialEq, Eq)]
struct ExpectedLink {
    offset: u32,
    length: u32,
    target: String,
}

fn test_link_detection(value: &str, expected_links: &[(u32, u32, &str)]) {
    let document = TextDocument::create("file:///test/data/abc/test.html", "html", 0, value);
    let ls = get_language_service(LanguageServiceOptions::default());
    let links = ls.find_document_links(&document, &TestDocumentContext);
    let actual: Vec<ExpectedLink> = links
        .iter()
        .map(|l| ExpectedLink {
            offset: l.range.start.character,
            length: l.range.end.character - l.range.start.character,
            target: l.target.clone().unwrap_or_default(),
        })
        .collect();
    let expected: Vec<ExpectedLink> = expected_links
        .iter()
        .map(|(offset, length, target)| ExpectedLink {
            offset: *offset,
            length: *length,
            target: target.to_string(),
        })
        .collect();
    assert_eq!(actual, expected, "input {value:?}");
}

#[test]
fn link_creation() {
    test_link_creation("http://model/1.html", "javascript:void;", None);
    test_link_creation("http://model/1.html", " \tjavascript:alert(7);", None);
    test_link_creation(
        "http://model/1.html",
        " #relative",
        Some("http://model/1.html"),
    );
    test_link_creation(
        "http://model/1.html",
        "file:///C:\\Alex\\src\\path\\to\\file.txt",
        Some("file:///C:\\Alex\\src\\path\\to\\file.txt"),
    );
    test_link_creation(
        "http://model/1.html",
        "http://www.microsoft.com/",
        Some("http://www.microsoft.com/"),
    );
    test_link_creation(
        "http://model/1.html",
        "https://www.microsoft.com/",
        Some("https://www.microsoft.com/"),
    );
    test_link_creation(
        "http://model/1.html",
        "//www.microsoft.com/",
        Some("http://www.microsoft.com/"),
    );
    test_link_creation("http://model/x/1.html", "a.js", Some("http://model/x/a.js"));
    test_link_creation(
        "http://model/x/1.html",
        "./a2.js",
        Some("http://model/x/a2.js"),
    );
    test_link_creation("http://model/x/1.html", "/b.js", Some("http://model/b.js"));
    test_link_creation(
        "http://model/x/y/1.html",
        "../../c.js",
        Some("http://model/c.js"),
    );

    test_link_creation(
        "file:///C:/Alex/src/path/to/file.html",
        "javascript:void;",
        None,
    );
    test_link_creation(
        "file:///C:/Alex/src/path/to/file.html",
        " \tjavascript:alert(7);",
        None,
    );
    test_link_creation(
        "file:///C:/Alex/src/path/to/file.html",
        " #relative",
        Some("file:///C:/Alex/src/path/to/file.html"),
    );
    test_link_creation(
        "file:///C:/Alex/src/path/to/file.html",
        "file:///C:\\Alex\\src\\path\\to\\file.txt",
        Some("file:///C:\\Alex\\src\\path\\to\\file.txt"),
    );
    test_link_creation(
        "file:///C:/Alex/src/path/to/file.html",
        "http://www.microsoft.com/",
        Some("http://www.microsoft.com/"),
    );
    test_link_creation(
        "file:///C:/Alex/src/path/to/file.html",
        "https://www.microsoft.com/",
        Some("https://www.microsoft.com/"),
    );
    test_link_creation(
        "file:///C:/Alex/src/path/to/file.html",
        "  //www.microsoft.com/",
        Some("http://www.microsoft.com/"),
    );
    test_link_creation(
        "file:///C:/Alex/src/path/to/file.html",
        "a.js",
        Some("file:///C:/Alex/src/path/to/a.js"),
    );
    test_link_creation(
        "file:///C:/Alex/src/path/to/file.html",
        "/a.js",
        Some("file:///a.js"),
    );

    test_link_creation(
        "https://www.test.com/path/to/file.html",
        "file:///C:\\Alex\\src\\path\\to\\file.txt",
        Some("file:///C:\\Alex\\src\\path\\to\\file.txt"),
    );
    test_link_creation(
        "https://www.test.com/path/to/file.html",
        "//www.microsoft.com/",
        Some("https://www.microsoft.com/"),
    );
    test_link_creation(
        "https://www.test.com/path/to/file.html",
        "//www.microsoft.com/",
        Some("https://www.microsoft.com/"),
    );

    // invalid uris are ignored
    test_link_creation("https://www.test.com/path/to/file.html", "%", None);

    // Bug #18314
    test_link_creation(
        "file:///c:/Alex/working_dir/18314-link-detection/test.html",
        "/class/class.js",
        Some("file:///class/class.js"),
    );

    test_link_creation(
        "http://foo/bar.hbs",
        "/class/class.js",
        Some("http://foo/class/class.js"),
    );
    test_link_creation("http://foo/bar.hbs", "{{asset foo}}/class/class.js", None);
    test_link_creation("http://foo/bar.hbs", "{{href-to", None);
}

#[test]
fn link_detection() {
    test_link_detection(
        r#"<img src="foo.png">"#,
        &[(10, 7, "file:///test/data/abc/foo.png")],
    );
    test_link_detection(
        r#"<a href="http://server/foo.html">"#,
        &[(9, 22, "http://server/foo.html")],
    );
    test_link_detection(r#"<img src="">"#, &[]);
    test_link_detection(
        r#"<LINK HREF="a.html">"#,
        &[(12, 6, "file:///test/data/abc/a.html")],
    );
    test_link_detection("<LINK HREF=\"a.html\n>\n", &[]);
    test_link_detection(
        "<a href=http://www.example.com></a>",
        &[(8, 22, "http://www.example.com")],
    );

    test_link_detection(
        r#"<html><base href="docs/"><img src="foo.png"></html>"#,
        &[(35, 7, "file:///test/data/abc/docs/foo.png")],
    );
    test_link_detection(
        r#"<html><base href="http://www.example.com/page.html"><img src="foo.png"></html>"#,
        &[(62, 7, "http://www.example.com/foo.png")],
    );
    test_link_detection(
        r#"<html><base href=".."><img src="foo.png"></html>"#,
        &[(32, 7, "file:///test/data/foo.png")],
    );
    test_link_detection(
        r#"<html><base href="."><img src="foo.png"></html>"#,
        &[(31, 7, "file:///test/data/abc/foo.png")],
    );
    test_link_detection(
        r#"<html><base href="/docs/"><img src="foo.png"></html>"#,
        &[(36, 7, "file:///docs/foo.png")],
    );

    test_link_detection(
        r#"<a href="mailto:<%- mail %>@<%- domain %>" > <% - mail %>@<% - domain %> </a>"#,
        &[],
    );

    test_link_detection(
        r#"<link rel="icon" type="image/x-icon" href="data:@file/x-icon;base64,AAABAAIAQEAAAAEAIAAoQgAAJgA">"#,
        &[],
    );
    test_link_detection(
        r#"<blockquote cite="foo.png">"#,
        &[(18, 7, "file:///test/data/abc/foo.png")],
    );
    test_link_detection(
        r#"<style src="styles.css?t=345">"#,
        &[(12, 16, "file:///test/data/abc/styles.css")],
    );
    test_link_detection(
        r#"<a href="https://werkenvoor.be/nl/jobs?f%5B0%5D=activitydomains%3A115&f%5B1%5D=lang%3Anl">link</a>"#,
        &[(
            9,
            79,
            "https://werkenvoor.be/nl/jobs?f%5B0%5D=activitydomains%3A115&f%5B1%5D=lang%3Anl",
        )],
    );
    test_link_detection(
        r#"<a href="jobs.html?f=bar">link</a>"#,
        &[(9, 15, "file:///test/data/abc/jobs.html")],
    );
}

#[test]
fn local_targets() {
    test_link_detection(
        r##"<body><h1 id="title"></h1><a href="#title"</a></body>"##,
        &[(35, 6, "file:///test/data/abc/test.html#1,14")],
    );
    test_link_detection(
        r#"<body><h1 id="title"></h1><a href="file:///test/data/abc/test.html#title"</a></body>"#,
        &[(35, 37, "file:///test/data/abc/test.html#1,14")],
    );
    test_link_detection(
        r##"<body><h1 id="title"></h1><a href="#body"</a></body>"##,
        &[(35, 5, "file:///test/data/abc/test.html")],
    );
}
