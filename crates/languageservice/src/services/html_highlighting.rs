//! Port of `src/services/htmlHighlighting.ts`.

use crate::html_language_types::{ScannerState, TokenType};
use crate::parser::html_parser::HtmlDocument;
use crate::parser::html_scanner::create_scanner;
use crate::text_document::TextDocument;
use crate::types::{DocumentHighlight, DocumentHighlightKind, Position, Range};

pub fn find_document_highlights(
    document: &TextDocument,
    position: Position,
    html_document: &HtmlDocument,
) -> Vec<DocumentHighlight> {
    let offset = document.offset_at(position);
    let node = html_document.node(html_document.find_node_at(offset));
    if node.tag.is_none() {
        return Vec::new();
    }
    let mut result = Vec::new();
    let start_tag_range = get_tag_name_range(TokenType::StartTag, document, node.start);
    let end_tag_range = node
        .end_tag_start
        .and_then(|start| get_tag_name_range(TokenType::EndTag, document, start));
    if start_tag_range
        .as_ref()
        .is_some_and(|r| covers(r, position))
        || end_tag_range.as_ref().is_some_and(|r| covers(r, position))
    {
        if let Some(range) = start_tag_range {
            result.push(DocumentHighlight {
                kind: DocumentHighlightKind::Read,
                range,
            });
        }
        if let Some(range) = end_tag_range {
            result.push(DocumentHighlight {
                kind: DocumentHighlightKind::Read,
                range,
            });
        }
    }
    result
}

fn is_before_or_equal(pos1: Position, pos2: Position) -> bool {
    pos1.line < pos2.line || (pos1.line == pos2.line && pos1.character <= pos2.character)
}

fn covers(range: &Range, position: Position) -> bool {
    is_before_or_equal(range.start, position) && is_before_or_equal(position, range.end)
}

fn get_tag_name_range(
    token_type: TokenType,
    document: &TextDocument,
    start_offset: usize,
) -> Option<Range> {
    let mut scanner = create_scanner(
        document.get_text(),
        start_offset,
        ScannerState::WithinContent,
        false,
    );
    let mut token = scanner.scan();
    while token != TokenType::Eos && token != token_type {
        token = scanner.scan();
    }
    if token != TokenType::Eos {
        return Some(Range {
            start: document.position_at(scanner.get_token_offset()),
            end: document.position_at(scanner.get_token_end()),
        });
    }
    None
}
