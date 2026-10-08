//! Port of `src/services/htmlLinkedEditing.ts`.

use crate::parser::html_parser::HtmlDocument;
use crate::text_document::TextDocument;
use crate::types::{Position, Range};

pub fn find_linked_editing_ranges(
    document: &TextDocument,
    position: Position,
    html_document: &HtmlDocument,
) -> Option<Vec<Range>> {
    let offset = document.offset_at(position);
    let node = html_document.node(html_document.find_node_at(offset));

    let tag_length = node
        .tag
        .as_deref()
        .map(|t| t.encode_utf16().count())
        .unwrap_or(0);

    let end_tag_start = node.end_tag_start?;

    if (node.start < offset && offset <= node.start + 1 + tag_length)
        || (end_tag_start + 2 <= offset && offset <= end_tag_start + 2 + tag_length)
    {
        return Some(vec![
            Range {
                start: document.position_at(node.start + 1),
                end: document.position_at(node.start + 1 + tag_length),
            },
            Range {
                start: document.position_at(end_tag_start + 2),
                end: document.position_at(end_tag_start + 2 + tag_length),
            },
        ]);
    }

    None
}
