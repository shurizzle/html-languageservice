//! Port of `src/services/htmlMatchingTagPosition.ts`.

use crate::parser::html_parser::HtmlDocument;
use crate::text_document::TextDocument;
use crate::types::Position;

pub fn find_matching_tag_position(
    document: &TextDocument,
    position: Position,
    html_document: &HtmlDocument,
) -> Option<Position> {
    let offset = document.offset_at(position);
    let node = html_document.node(html_document.find_node_at(offset));

    let tag = node.tag.as_deref()?;
    let end_tag_start = node.end_tag_start?;
    let tag_len = tag.encode_utf16().count();

    // Within open tag, compute close tag
    if node.start < offset && offset <= node.start + 1 + tag_len {
        let mirror_offset = (offset - 1 - node.start) + end_tag_start + 2;
        return Some(document.position_at(mirror_offset));
    }

    // Within closing tag, compute open tag
    if end_tag_start + 2 <= offset && offset <= end_tag_start + 2 + tag_len {
        let mirror_offset = (offset - 2 - end_tag_start) + node.start + 1;
        return Some(document.position_at(mirror_offset));
    }

    None
}
