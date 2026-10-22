//! Port of `src/test/scanner.test.ts`.

use html_languageservice::html_language_types::{ScannerState, TokenType};
use html_languageservice::parser::{convert_script_content_to_javascript, create_scanner};

#[derive(Debug, PartialEq)]
struct Tok {
    offset: usize,
    ty: TokenType,
    content: Option<String>,
}

type Expected<'a> = &'a [(usize, TokenType, Option<&'a str>)];

fn assert_tokens(tests: &[(&str, Expected)]) {
    let mut scanner_state = ScannerState::WithinContent;
    for (input, expected) in tests {
        let mut scanner = create_scanner(input, 0, scanner_state, false);
        let mut actual = Vec::new();
        let mut ty = scanner.scan();
        while ty != TokenType::Eos {
            let content = if ty == TokenType::StartTag || ty == TokenType::EndTag {
                Some(scanner.get_token_text())
            } else {
                None
            };
            actual.push(Tok {
                offset: scanner.get_token_offset(),
                ty,
                content,
            });
            ty = scanner.scan();
        }
        let exp: Vec<Tok> = expected
            .iter()
            .map(|(o, t, c)| Tok {
                offset: *o,
                ty: *t,
                content: c.map(|s| s.to_string()),
            })
            .collect();
        assert_eq!(actual, exp, "input: {input:?}");
        scanner_state = scanner.get_scanner_state();
    }
}

use TokenType::*;

#[test]
fn open_start_tag_1() {
    assert_tokens(&[(
        "<abc",
        &[(0, StartTagOpen, None), (1, StartTag, Some("abc"))],
    )]);
}

#[test]
fn open_start_tag_2() {
    assert_tokens(&[(
        "<input",
        &[(0, StartTagOpen, None), (1, StartTag, Some("input"))],
    )]);
}

#[test]
fn open_start_tag_with_invalid_tag() {
    assert_tokens(&[(
        "< abc",
        &[
            (0, StartTagOpen, None),
            (1, Whitespace, None),
            (2, StartTag, Some("abc")),
        ],
    )]);
}

#[test]
fn open_start_tag_3() {
    assert_tokens(&[(
        "< abc>",
        &[
            (0, StartTagOpen, None),
            (1, Whitespace, None),
            (2, StartTag, Some("abc")),
            (5, StartTagClose, None),
        ],
    )]);
}

#[test]
fn open_start_tag_4() {
    assert_tokens(&[(
        "i <len;",
        &[
            (0, Content, None),
            (2, StartTagOpen, None),
            (3, StartTag, Some("len")),
            (6, Unknown, None),
        ],
    )]);
}

#[test]
fn open_start_tag_5() {
    assert_tokens(&[("<", &[(0, StartTagOpen, None)])]);
}

#[test]
fn open_end_tag() {
    assert_tokens(&[("</a", &[(0, EndTagOpen, None), (2, EndTag, Some("a"))])]);
}

#[test]
fn complete_start_tag() {
    assert_tokens(&[(
        "<abc>",
        &[
            (0, StartTagOpen, None),
            (1, StartTag, Some("abc")),
            (4, StartTagClose, None),
        ],
    )]);
}

#[test]
fn complete_start_tag_with_whitespace() {
    assert_tokens(&[(
        "<abc >",
        &[
            (0, StartTagOpen, None),
            (1, StartTag, Some("abc")),
            (4, Whitespace, None),
            (5, StartTagClose, None),
        ],
    )]);
}

#[test]
fn bug_9809_start_tag_namespace_prefix() {
    assert_tokens(&[(
        "<foo:bar>",
        &[
            (0, StartTagOpen, None),
            (1, StartTag, Some("foo:bar")),
            (8, StartTagClose, None),
        ],
    )]);
}

#[test]
fn complete_end_tag() {
    assert_tokens(&[(
        "</abc>",
        &[
            (0, EndTagOpen, None),
            (2, EndTag, Some("abc")),
            (5, EndTagClose, None),
        ],
    )]);
}

#[test]
fn complete_end_tag_with_whitespace() {
    assert_tokens(&[(
        "</abc  >",
        &[
            (0, EndTagOpen, None),
            (2, EndTag, Some("abc")),
            (5, Whitespace, None),
            (7, EndTagClose, None),
        ],
    )]);
}

#[test]
fn empty_tag() {
    assert_tokens(&[(
        "<abc />",
        &[
            (0, StartTagOpen, None),
            (1, StartTag, Some("abc")),
            (4, Whitespace, None),
            (5, StartTagSelfClose, None),
        ],
    )]);
}

#[test]
fn embedded_content_1() {
    assert_tokens(&[(
        "<script type=\"text/javascript\">var i= 10;</script>",
        &[
            (0, StartTagOpen, None),
            (1, StartTag, Some("script")),
            (7, Whitespace, None),
            (8, AttributeName, None),
            (12, DelimiterAssign, None),
            (13, AttributeValue, None),
            (30, StartTagClose, None),
            (31, Script, None),
            (41, EndTagOpen, None),
            (43, EndTag, Some("script")),
            (49, EndTagClose, None),
        ],
    )]);
}

#[test]
fn embedded_content_2() {
    assert_tokens(&[
        (
            "<script type=\"text/javascript\">",
            &[
                (0, StartTagOpen, None),
                (1, StartTag, Some("script")),
                (7, Whitespace, None),
                (8, AttributeName, None),
                (12, DelimiterAssign, None),
                (13, AttributeValue, None),
                (30, StartTagClose, None),
            ],
        ),
        ("var i= 10;", &[(0, Script, None)]),
        (
            "</script>",
            &[
                (0, EndTagOpen, None),
                (2, EndTag, Some("script")),
                (8, EndTagClose, None),
            ],
        ),
    ]);
}

#[test]
fn embedded_content_3() {
    assert_tokens(&[
        (
            "<script type=\"text/javascript\">var i= 10;",
            &[
                (0, StartTagOpen, None),
                (1, StartTag, Some("script")),
                (7, Whitespace, None),
                (8, AttributeName, None),
                (12, DelimiterAssign, None),
                (13, AttributeValue, None),
                (30, StartTagClose, None),
                (31, Script, None),
            ],
        ),
        (
            "</script>",
            &[
                (0, EndTagOpen, None),
                (2, EndTag, Some("script")),
                (8, EndTagClose, None),
            ],
        ),
    ]);
}

#[test]
fn embedded_content_4() {
    assert_tokens(&[
        (
            "<script type=\"text/javascript\">",
            &[
                (0, StartTagOpen, None),
                (1, StartTag, Some("script")),
                (7, Whitespace, None),
                (8, AttributeName, None),
                (12, DelimiterAssign, None),
                (13, AttributeValue, None),
                (30, StartTagClose, None),
            ],
        ),
        (
            "var i= 10;</script>",
            &[
                (0, Script, None),
                (10, EndTagOpen, None),
                (12, EndTag, Some("script")),
                (18, EndTagClose, None),
            ],
        ),
    ]);
}

#[test]
fn embedded_content_5() {
    assert_tokens(&[(
        "<script type=\"text/plain\">a\n<a</script>",
        &[
            (0, StartTagOpen, None),
            (1, StartTag, Some("script")),
            (7, Whitespace, None),
            (8, AttributeName, None),
            (12, DelimiterAssign, None),
            (13, AttributeValue, None),
            (25, StartTagClose, None),
            (26, Script, None),
            (30, EndTagOpen, None),
            (32, EndTag, Some("script")),
            (38, EndTagClose, None),
        ],
    )]);
}

#[test]
fn embedded_content_6() {
    assert_tokens(&[(
        "<script>a</script><script>b</script>",
        &[
            (0, StartTagOpen, None),
            (1, StartTag, Some("script")),
            (7, StartTagClose, None),
            (8, Script, None),
            (9, EndTagOpen, None),
            (11, EndTag, Some("script")),
            (17, EndTagClose, None),
            (18, StartTagOpen, None),
            (19, StartTag, Some("script")),
            (25, StartTagClose, None),
            (26, Script, None),
            (27, EndTagOpen, None),
            (29, EndTag, Some("script")),
            (35, EndTagClose, None),
        ],
    )]);
}

#[test]
fn embedded_content_7() {
    assert_tokens(&[(
        "<script type=\"text/javascript\"></script>",
        &[
            (0, StartTagOpen, None),
            (1, StartTag, Some("script")),
            (7, Whitespace, None),
            (8, AttributeName, None),
            (12, DelimiterAssign, None),
            (13, AttributeValue, None),
            (30, StartTagClose, None),
            (31, EndTagOpen, None),
            (33, EndTag, Some("script")),
            (39, EndTagClose, None),
        ],
    )]);
}

#[test]
fn embedded_content_8() {
    assert_tokens(&[(
        "<script>var i= 10;</script>",
        &[
            (0, StartTagOpen, None),
            (1, StartTag, Some("script")),
            (7, StartTagClose, None),
            (8, Script, None),
            (18, EndTagOpen, None),
            (20, EndTag, Some("script")),
            (26, EndTagClose, None),
        ],
    )]);
}

#[test]
fn embedded_content_9() {
    assert_tokens(&[(
        "<script type=\"text/javascript\" src=\"main.js\"></script>",
        &[
            (0, StartTagOpen, None),
            (1, StartTag, Some("script")),
            (7, Whitespace, None),
            (8, AttributeName, None),
            (12, DelimiterAssign, None),
            (13, AttributeValue, None),
            (30, Whitespace, None),
            (31, AttributeName, None),
            (34, DelimiterAssign, None),
            (35, AttributeValue, None),
            (44, StartTagClose, None),
            (45, EndTagOpen, None),
            (47, EndTag, Some("script")),
            (53, EndTagClose, None),
        ],
    )]);
}

#[test]
fn embedded_content_10() {
    assert_tokens(&[(
        "<script><!-- alert(\"<script></script>\"); --></script>",
        &[
            (0, StartTagOpen, None),
            (1, StartTag, Some("script")),
            (7, StartTagClose, None),
            (8, Script, None),
            (44, EndTagOpen, None),
            (46, EndTag, Some("script")),
            (52, EndTagClose, None),
        ],
    )]);
}

#[test]
fn embedded_content_11() {
    assert_tokens(&[(
        "<script><!-- alert(\"<script></script>\"); </script>",
        &[
            (0, StartTagOpen, None),
            (1, StartTag, Some("script")),
            (7, StartTagClose, None),
            (8, Script, None),
            (41, EndTagOpen, None),
            (43, EndTag, Some("script")),
            (49, EndTagClose, None),
        ],
    )]);
}

#[test]
fn embedded_content_12() {
    assert_tokens(&[(
        "<script><!-- alert(\"</script>\"); </script>",
        &[
            (0, StartTagOpen, None),
            (1, StartTag, Some("script")),
            (7, StartTagClose, None),
            (8, Script, None),
            (20, EndTagOpen, None),
            (22, EndTag, Some("script")),
            (28, EndTagClose, None),
            (29, Content, None),
            (33, EndTagOpen, None),
            (35, EndTag, Some("script")),
            (41, EndTagClose, None),
        ],
    )]);
}

#[test]
fn embedded_content_13() {
    assert_tokens(&[(
        "<script> alert(\"<script></script>\"); </script>",
        &[
            (0, StartTagOpen, None),
            (1, StartTag, Some("script")),
            (7, StartTagClose, None),
            (8, Script, None),
            (24, EndTagOpen, None),
            (26, EndTag, Some("script")),
            (32, EndTagClose, None),
            (33, Content, None),
            (37, EndTagOpen, None),
            (39, EndTag, Some("script")),
            (45, EndTagClose, None),
        ],
    )]);
}

#[test]
fn tag_with_attribute() {
    assert_tokens(&[
        (
            "<abc foo=\"bar\">",
            &[
                (0, StartTagOpen, None),
                (1, StartTag, Some("abc")),
                (4, Whitespace, None),
                (5, AttributeName, None),
                (8, DelimiterAssign, None),
                (9, AttributeValue, None),
                (14, StartTagClose, None),
            ],
        ),
        (
            "<abc foo='bar'>",
            &[
                (0, StartTagOpen, None),
                (1, StartTag, Some("abc")),
                (4, Whitespace, None),
                (5, AttributeName, None),
                (8, DelimiterAssign, None),
                (9, AttributeValue, None),
                (14, StartTagClose, None),
            ],
        ),
    ]);
}

#[test]
fn tag_with_empty_attributes() {
    assert_tokens(&[
        (
            "<abc foo=\"\">",
            &[
                (0, StartTagOpen, None),
                (1, StartTag, Some("abc")),
                (4, Whitespace, None),
                (5, AttributeName, None),
                (8, DelimiterAssign, None),
                (9, AttributeValue, None),
                (11, StartTagClose, None),
            ],
        ),
        (
            "<abc foo=''>",
            &[
                (0, StartTagOpen, None),
                (1, StartTag, Some("abc")),
                (4, Whitespace, None),
                (5, AttributeName, None),
                (8, DelimiterAssign, None),
                (9, AttributeValue, None),
                (11, StartTagClose, None),
            ],
        ),
    ]);
}

#[test]
fn tag_with_attributes() {
    assert_tokens(&[(
        "<abc foo=\"bar\" bar='foo'>",
        &[
            (0, StartTagOpen, None),
            (1, StartTag, Some("abc")),
            (4, Whitespace, None),
            (5, AttributeName, None),
            (8, DelimiterAssign, None),
            (9, AttributeValue, None),
            (14, Whitespace, None),
            (15, AttributeName, None),
            (18, DelimiterAssign, None),
            (19, AttributeValue, None),
            (24, StartTagClose, None),
        ],
    )]);
}

#[test]
fn tag_with_attributes_no_quotes() {
    assert_tokens(&[(
        "<abc foo=bar bar=help-me>",
        &[
            (0, StartTagOpen, None),
            (1, StartTag, Some("abc")),
            (4, Whitespace, None),
            (5, AttributeName, None),
            (8, DelimiterAssign, None),
            (9, AttributeValue, None),
            (12, Whitespace, None),
            (13, AttributeName, None),
            (16, DelimiterAssign, None),
            (17, AttributeValue, None),
            (24, StartTagClose, None),
        ],
    )]);
}

#[test]
fn tag_with_attributes_no_quotes_self_close() {
    assert_tokens(&[
        (
            "<abc foo=bar/>",
            &[
                (0, StartTagOpen, None),
                (1, StartTag, Some("abc")),
                (4, Whitespace, None),
                (5, AttributeName, None),
                (8, DelimiterAssign, None),
                (9, AttributeValue, None),
                (12, StartTagSelfClose, None),
            ],
        ),
        (
            "<abc foo=http://example.com/foo/bar/>",
            &[
                (0, StartTagOpen, None),
                (1, StartTag, Some("abc")),
                (4, Whitespace, None),
                (5, AttributeName, None),
                (8, DelimiterAssign, None),
                (9, AttributeValue, None),
                (35, StartTagSelfClose, None),
            ],
        ),
    ]);
}

#[test]
fn tag_with_attribute_and_whitespace() {
    assert_tokens(&[(
        "<abc foo=  \"bar\">",
        &[
            (0, StartTagOpen, None),
            (1, StartTag, Some("abc")),
            (4, Whitespace, None),
            (5, AttributeName, None),
            (8, DelimiterAssign, None),
            (9, Whitespace, None),
            (11, AttributeValue, None),
            (16, StartTagClose, None),
        ],
    )]);
}

#[test]
fn tag_with_attribute_and_whitespace_2() {
    assert_tokens(&[(
        "<abc foo = \"bar\">",
        &[
            (0, StartTagOpen, None),
            (1, StartTag, Some("abc")),
            (4, Whitespace, None),
            (5, AttributeName, None),
            (8, Whitespace, None),
            (9, DelimiterAssign, None),
            (10, Whitespace, None),
            (11, AttributeValue, None),
            (16, StartTagClose, None),
        ],
    )]);
}

#[test]
fn tag_with_name_only_attribute_1() {
    assert_tokens(&[(
        "<abc foo>",
        &[
            (0, StartTagOpen, None),
            (1, StartTag, Some("abc")),
            (4, Whitespace, None),
            (5, AttributeName, None),
            (8, StartTagClose, None),
        ],
    )]);
}

#[test]
fn tag_with_name_only_attribute_2() {
    assert_tokens(&[(
        "<abc foo bar>",
        &[
            (0, StartTagOpen, None),
            (1, StartTag, Some("abc")),
            (4, Whitespace, None),
            (5, AttributeName, None),
            (8, Whitespace, None),
            (9, AttributeName, None),
            (12, StartTagClose, None),
        ],
    )]);
}

#[test]
fn tag_with_interesting_attribute_name() {
    assert_tokens(&[(
        "<abc foo!@#=\"bar\">",
        &[
            (0, StartTagOpen, None),
            (1, StartTag, Some("abc")),
            (4, Whitespace, None),
            (5, AttributeName, None),
            (11, DelimiterAssign, None),
            (12, AttributeValue, None),
            (17, StartTagClose, None),
        ],
    )]);
}

#[test]
fn tag_with_angular_attribute_name() {
    assert_tokens(&[(
        "<abc #myinput (click)=\"bar\" [value]=\"someProperty\" *ngIf=\"someCondition\">",
        &[
            (0, StartTagOpen, None),
            (1, StartTag, Some("abc")),
            (4, Whitespace, None),
            (5, AttributeName, None),
            (13, Whitespace, None),
            (14, AttributeName, None),
            (21, DelimiterAssign, None),
            (22, AttributeValue, None),
            (27, Whitespace, None),
            (28, AttributeName, None),
            (35, DelimiterAssign, None),
            (36, AttributeValue, None),
            (50, Whitespace, None),
            (51, AttributeName, None),
            (56, DelimiterAssign, None),
            (57, AttributeValue, None),
            (72, StartTagClose, None),
        ],
    )]);
}

#[test]
fn tag_with_invalid_attribute_value() {
    assert_tokens(&[(
        "<abc foo=\">",
        &[
            (0, StartTagOpen, None),
            (1, StartTag, Some("abc")),
            (4, Whitespace, None),
            (5, AttributeName, None),
            (8, DelimiterAssign, None),
            (9, AttributeValue, None),
        ],
    )]);
}

#[test]
fn simple_comment_1() {
    assert_tokens(&[(
        "<!--a-->",
        &[
            (0, StartCommentTag, None),
            (4, Comment, None),
            (5, EndCommentTag, None),
        ],
    )]);
}

#[test]
fn simple_comment_2() {
    assert_tokens(&[(
        "<!--a>foo bar</a -->",
        &[
            (0, StartCommentTag, None),
            (4, Comment, None),
            (17, EndCommentTag, None),
        ],
    )]);
}

#[test]
fn multiline_comment() {
    assert_tokens(&[(
        "<!--a>\nfoo \nbar</a -->",
        &[
            (0, StartCommentTag, None),
            (4, Comment, None),
            (19, EndCommentTag, None),
        ],
    )]);
}

#[test]
fn simple_doctype() {
    assert_tokens(&[(
        "<!Doctype a>",
        &[
            (0, StartDoctypeTag, None),
            (9, Doctype, None),
            (11, EndDoctypeTag, None),
        ],
    )]);
}

#[test]
fn simple_doctype_2() {
    assert_tokens(&[(
        "<!doctype a>",
        &[
            (0, StartDoctypeTag, None),
            (9, Doctype, None),
            (11, EndDoctypeTag, None),
        ],
    )]);
}

#[test]
fn simple_doctype_4() {
    assert_tokens(&[(
        "<!DOCTYPE a\n\"foo\" 'bar'>",
        &[
            (0, StartDoctypeTag, None),
            (9, Doctype, None),
            (23, EndDoctypeTag, None),
        ],
    )]);
}

#[test]
fn incomplete() {
    assert_tokens(&[("    ", &[(0, Content, None)])]);
    assert_tokens(&[(
        "<!---   ",
        &[(0, StartCommentTag, None), (4, Comment, None)],
    )]);
    assert_tokens(&[(
        "<style>color:red",
        &[
            (0, StartTagOpen, None),
            (1, StartTag, Some("style")),
            (6, StartTagClose, None),
            (7, Styles, None),
        ],
    )]);
    assert_tokens(&[(
        "<script>alert(\"!!\")",
        &[
            (0, StartTagOpen, None),
            (1, StartTag, Some("script")),
            (7, StartTagClose, None),
            (8, Script, None),
        ],
    )]);
    assert_tokens(&[(
        "<input label= />",
        &[
            (0, StartTagOpen, None),
            (1, StartTag, Some("input")),
            (6, Whitespace, None),
            (7, AttributeName, None),
            (12, DelimiterAssign, None),
            (13, Whitespace, None),
            (14, StartTagSelfClose, None),
        ],
    )]);
}

#[test]
fn tag_with_hyphenated_name_vue_custom_element() {
    assert_tokens(&[(
        "<s-c-feature>content</s-c-feature>",
        &[
            (0, StartTagOpen, None),
            (1, StartTag, Some("s-c-feature")),
            (12, StartTagClose, None),
            (13, Content, None),
            (20, EndTagOpen, None),
            (22, EndTag, Some("s-c-feature")),
            (33, EndTagClose, None),
        ],
    )]);
}

#[test]
fn self_closing_tag_with_hyphenated_name() {
    assert_tokens(&[(
        "<my-component />",
        &[
            (0, StartTagOpen, None),
            (1, StartTag, Some("my-component")),
            (13, Whitespace, None),
            (14, StartTagSelfClose, None),
        ],
    )]);
}

#[test]
fn tag_with_hyphenated_name_and_attributes() {
    assert_tokens(&[(
        "<el-button type=\"primary\">",
        &[
            (0, StartTagOpen, None),
            (1, StartTag, Some("el-button")),
            (10, Whitespace, None),
            (11, AttributeName, None),
            (15, DelimiterAssign, None),
            (16, AttributeValue, None),
            (25, StartTagClose, None),
        ],
    )]);
}

#[test]
fn php_block_is_skipped() {
    assert_tokens(&[(
        r#"<?php echo "?>"; ?><div>"#,
        &[
            (0, Content, None),
            (19, StartTagOpen, None),
            (20, StartTag, Some("div")),
            (23, StartTagClose, None),
        ],
    )]);
}

#[test]
fn php_interpolation_with_nested_string_is_skipped() {
    let input = "<?php echo \"{$a[\"?>\"]}\"; ?><div>";
    let divider = input[..input.find("<div>").unwrap()].encode_utf16().count();
    let mut scanner = create_scanner(input, 0, ScannerState::WithinContent, false);

    assert_eq!(scanner.scan(), Content);
    assert_eq!(scanner.get_token_offset(), 0);
    assert_eq!(scanner.scan(), StartTagOpen);
    assert_eq!(scanner.get_token_offset(), divider);
}

#[test]
fn php_line_comment_close_tag_ends_block() {
    assert_tokens(&[(
        "<?php // ?>\n<div>",
        &[
            (0, Content, None),
            (12, StartTagOpen, None),
            (13, StartTag, Some("div")),
            (16, StartTagClose, None),
        ],
    )]);
}

#[test]
fn php_unclosed_block_swallows_rest() {
    assert_tokens(&[("<?php echo 1;", &[(0, Content, None)])]);
}

#[test]
fn convert_script_content() {
    assert_eq!(
        convert_script_content_to_javascript("<!-- var x = 1; -->"),
        "/*  var x = 1;  */"
    );
    assert_eq!(
        convert_script_content_to_javascript("/* --> a b\n  */"),
        "/* --> a b\n  */"
    );
    assert_eq!(
        convert_script_content_to_javascript("<!-- code --> /* --> */"),
        "/*  code  */ /* --> */"
    );
    assert_eq!(
        convert_script_content_to_javascript("<!-- a --> <!-- b -->"),
        "/*  a  */ /*  b  */"
    );
    assert_eq!(
        convert_script_content_to_javascript("<!-- <!-- --> -->"),
        "/*  /*   */  */"
    );
    assert_eq!(convert_script_content_to_javascript(""), "");
    assert_eq!(
        convert_script_content_to_javascript("var x = 1;"),
        "var x = 1;"
    );
    assert_eq!(
        convert_script_content_to_javascript("<!-- var x = 1;"),
        "/*  var x = 1;"
    );
}
