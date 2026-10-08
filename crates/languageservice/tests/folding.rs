//! Port of `src/test/folding.test.ts`.

use html_languageservice::TextDocument;
use html_languageservice::language_facts::DataManager;
use html_languageservice::services::html_folding::{FoldingRangeContext, HtmlFolding};

fn assert_ranges(
    lines: &[&str],
    expected: &[(u32, u32, Option<&str>)],
    range_limit: Option<usize>,
) {
    let document = TextDocument::create("test://foo/bar.json", "json", 1, lines.join("\n"));
    let data_manager = DataManager::new(true, Vec::new());
    let folding = HtmlFolding::new(&data_manager);
    let actual = folding.get_folding_ranges(&document, Some(&FoldingRangeContext { range_limit }));

    let mut actual_ranges: Vec<(u32, u32, Option<String>)> = actual
        .iter()
        .map(|r| {
            (
                r.start_line,
                r.end_line,
                r.kind.map(|k| k.as_str().to_string()),
            )
        })
        .collect();
    actual_ranges.sort_by_key(|r| r.0);

    let expected: Vec<(u32, u32, Option<String>)> = expected
        .iter()
        .map(|(s, e, k)| (*s, *e, k.map(|k| k.to_string())))
        .collect();
    assert_eq!(actual_ranges, expected);
}

#[test]
fn fold_one_level() {
    assert_ranges(&["<html>", "Hello", "</html>"], &[(0, 1, None)], None);
}

#[test]
fn fold_two_level() {
    assert_ranges(
        &["<html>", "<head>", "Hello", "</head>", "</html>"],
        &[(0, 3, None), (1, 2, None)],
        None,
    );
}

#[test]
fn fold_siblings() {
    assert_ranges(
        &[
            "<html>",
            "<head>",
            "Head",
            "</head>",
            r#"<body class="f">"#,
            "Body",
            "</body>",
            "</html>",
        ],
        &[(0, 6, None), (1, 2, None), (4, 5, None)],
        None,
    );
}

#[test]
fn fold_self_closing_tags() {
    assert_ranges(
        &[
            "<div>",
            r#"<a href="top"/>"#,
            r#"<img src="s">"#,
            "<br/>",
            "<br>",
            r#"<img class="c""#,
            r#"     src="top""#,
            ">",
            "</div>",
        ],
        &[(0, 7, None), (5, 6, None)],
        None,
    );
}

#[test]
fn fold_comment() {
    assert_ranges(
        &[
            "<!--",
            " multi line",
            "-->",
            "<!-- some stuff",
            " some more stuff -->",
        ],
        &[(0, 2, Some("comment")), (3, 4, Some("comment"))],
        None,
    );
}

#[test]
fn fold_regions() {
    assert_ranges(
        &[
            "<!-- #region -->",
            "<!-- #region -->",
            "<!-- #endregion -->",
            "<!-- #endregion -->",
        ],
        &[(0, 3, Some("region")), (1, 2, Some("region"))],
        None,
    );
}

#[test]
fn fold_incomplete() {
    assert_ranges(
        &["<body>", "<div></div>", "Hello", "</div>", "</body>"],
        &[(0, 3, None)],
        None,
    );
}

#[test]
fn fold_incomplete_2() {
    assert_ranges(
        &["<be><div>", "<!-- #endregion -->", "</div>"],
        &[(0, 1, None)],
        None,
    );
}

#[test]
fn fold_intersecting_region() {
    assert_ranges(
        &[
            "<body>",
            "<!-- #region -->",
            "Hello",
            "<div></div>",
            "</body>",
            "<!-- #endregion -->",
        ],
        &[(0, 3, None)],
        None,
    );
}

#[test]
fn fold_intersecting_region_2() {
    assert_ranges(
        &[
            "<!-- #region -->",
            "<body>",
            "Hello",
            "<!-- #endregion -->",
            "<div></div>",
            "</body>",
        ],
        &[(0, 3, Some("region"))],
        None,
    );
}

#[test]
fn test_limit() {
    let input = [
        "<div>",
        " <span>",
        "  <b>",
        "  ",
        "  </b>,",
        "  <b>",
        "   <pre>",
        "  ",
        "   </pre>,",
        "   <pre>",
        "  ",
        "   </pre>,",
        "  </b>,",
        "  <b>",
        "  ",
        "  </b>,",
        "  <b>",
        "  ",
        "  </b>",
        " </span>",
        "</div>",
    ];
    assert_ranges(
        &input,
        &[
            (0, 19, None),
            (1, 18, None),
            (2, 3, None),
            (5, 11, None),
            (6, 7, None),
            (9, 10, None),
            (13, 14, None),
            (16, 17, None),
        ],
        None,
    );
    assert_ranges(
        &input,
        &[
            (0, 19, None),
            (1, 18, None),
            (2, 3, None),
            (5, 11, None),
            (6, 7, None),
            (9, 10, None),
            (13, 14, None),
            (16, 17, None),
        ],
        Some(8),
    );
    assert_ranges(
        &input,
        &[
            (0, 19, None),
            (1, 18, None),
            (2, 3, None),
            (5, 11, None),
            (6, 7, None),
            (13, 14, None),
            (16, 17, None),
        ],
        Some(7),
    );
    assert_ranges(
        &input,
        &[
            (0, 19, None),
            (1, 18, None),
            (2, 3, None),
            (5, 11, None),
            (13, 14, None),
            (16, 17, None),
        ],
        Some(6),
    );
    assert_ranges(
        &input,
        &[
            (0, 19, None),
            (1, 18, None),
            (2, 3, None),
            (5, 11, None),
            (13, 14, None),
        ],
        Some(5),
    );
    assert_ranges(
        &input,
        &[(0, 19, None), (1, 18, None), (2, 3, None), (5, 11, None)],
        Some(4),
    );
    assert_ranges(
        &input,
        &[(0, 19, None), (1, 18, None), (2, 3, None)],
        Some(3),
    );
    assert_ranges(&input, &[(0, 19, None), (1, 18, None)], Some(2));
    assert_ranges(&input, &[(0, 19, None)], Some(1));
}
