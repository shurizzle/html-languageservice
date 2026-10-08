//! Port of `src/services/htmlRename.ts`.

use crate::parser::html_parser::{HtmlDocument, Node};
use crate::text_document::TextDocument;
use crate::types::{Position, Range, TextEdit, WorkspaceEdit};

pub fn do_rename(
    document: &TextDocument,
    position: Position,
    new_name: &str,
    html_document: &HtmlDocument,
) -> Option<WorkspaceEdit> {
    let offset = document.offset_at(position);
    let node = html_document.node(html_document.find_node_at(offset));

    let tag = node.tag.clone()?;

    if !is_within_tag_range(node, offset, &tag) {
        return None;
    }

    let tag_len = tag.encode_utf16().count();
    let mut edits = Vec::new();

    edits.push(TextEdit::replace(
        Range {
            start: document.position_at(node.start + 1),
            end: document.position_at(node.start + 1 + tag_len),
        },
        new_name,
    ));

    if let Some(end_tag_start) = node.end_tag_start {
        edits.push(TextEdit::replace(
            Range {
                start: document.position_at(end_tag_start + 2),
                end: document.position_at(end_tag_start + 2 + tag_len),
            },
            new_name,
        ));
    }

    Some(WorkspaceEdit {
        changes: vec![(document.uri().to_string(), edits)],
    })
}

/// Port of `'<'.length` / `'</'.length`.
const LT: usize = 1;
const LT_SLASH: usize = 2;

fn is_within_tag_range(node: &Node, offset: usize, node_tag: &str) -> bool {
    let tag_len = node_tag.encode_utf16().count();
    if let Some(end_tag_start) = node.end_tag_start
        && end_tag_start + LT_SLASH <= offset
        && offset <= end_tag_start + LT_SLASH + tag_len
    {
        return true;
    }
    node.start + LT <= offset && offset <= node.start + LT + tag_len
}
