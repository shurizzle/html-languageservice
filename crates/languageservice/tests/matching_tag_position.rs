//! Port of `src/test/matchingTagPosition.test.ts`.

mod support;

use html_languageservice::*;

/// The test inputs are ASCII, so byte offsets equal UTF-16 offsets.
fn test_matching_tag_position(value: &str) {
    assert!(value.is_ascii(), "input must be ASCII: {value:?}");
    let mut offset = value.find('|').expect("missing '|'");
    let mut value = format!("{}{}", &value[..offset], &value[offset + 1..]);
    let mirror_offset = value.find('$').expect("missing '$'");
    value = format!("{}{}", &value[..mirror_offset], &value[mirror_offset + 1..]);
    if mirror_offset < offset {
        offset -= 1;
    }

    let ls = get_language_service(LanguageServiceOptions::default());
    let document = TextDocument::create("test://test/test.html", "html", 0, value);
    let position = document.position_at(offset);
    let html_doc = ls.parse_html_document(&document);

    let mirror_position = ls
        .find_matching_tag_position(&document, position, &html_doc)
        .expect("Failed to find mirror position");
    assert_eq!(
        document.offset_at(mirror_position),
        mirror_offset,
        "mirror offset mismatch"
    );
}

#[test]
fn matching_position() {
    test_matching_tag_position("<|div></$div>");
    test_matching_tag_position("<d|iv></d$iv>");
    test_matching_tag_position("<di|v></di$v>");
    test_matching_tag_position("<div|></div$>");

    test_matching_tag_position("<$div></|div>");
    test_matching_tag_position("<d$iv></d|iv>");
    test_matching_tag_position("<di$v></di|v>");
    test_matching_tag_position("<div$></div|>");

    test_matching_tag_position("<div| ></div$>");
    test_matching_tag_position("<div| id=\"foo\"></div$>");

    test_matching_tag_position("<div$ ></div|>");
    test_matching_tag_position("<div$ id=\"foo\"></div|>");
}
