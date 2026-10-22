//! Port of `src/services/htmlSelectionRange.ts`.

use crate::html_language_types::{ScannerState, TokenType};
use crate::language_facts::DataManager;
use crate::parser::html_parser::{HtmlDocument, NodeId, ParseOptions, parse};
use crate::parser::html_scanner::create_scanner;
use crate::text_document::TextDocument;
use crate::types::{Position, Range, SelectionRange};

pub struct HtmlSelectionRange<'a> {
    data_manager: &'a DataManager,
}

impl<'a> HtmlSelectionRange<'a> {
    pub fn new(data_manager: &'a DataManager) -> Self {
        Self { data_manager }
    }

    pub fn get_selection_ranges(
        &self,
        document: &TextDocument,
        positions: &[Position],
    ) -> Vec<SelectionRange> {
        let html_document = parse(
            document.get_scanner_text(),
            &self.data_manager.get_void_elements(document.language_id()),
            ParseOptions::default(),
        );
        positions
            .iter()
            .map(|p| self.get_selection_range(*p, document, &html_document))
            .collect()
    }

    fn get_selection_range(
        &self,
        position: Position,
        document: &TextDocument,
        html_document: &HtmlDocument,
    ) -> SelectionRange {
        let applicable_ranges = self.get_applicable_ranges(document, position, html_document);
        let mut prev: Option<(usize, usize)> = None;
        let mut current: Option<SelectionRange> = None;
        for index in (0..applicable_ranges.len()).rev() {
            let range = applicable_ranges[index];
            if prev.is_none() || range != prev.unwrap() {
                current = Some(SelectionRange::create(
                    Range {
                        start: document.position_at(range.0),
                        end: document.position_at(range.1),
                    },
                    current.take(),
                ));
            }
            prev = Some(range);
        }
        current.unwrap_or_else(|| {
            SelectionRange::create(
                Range {
                    start: position,
                    end: position,
                },
                None,
            )
        })
    }

    fn get_applicable_ranges(
        &self,
        document: &TextDocument,
        position: Position,
        html_document: &HtmlDocument,
    ) -> Vec<(usize, usize)> {
        let curr_offset = document.offset_at(position);
        let curr_node_id = html_document.find_node_at(curr_offset);
        let curr_node = html_document.node(curr_node_id);

        let mut result = self.get_all_parent_tag_ranges(html_document, curr_node_id);

        // Self-closing or void elements
        if let Some(start_tag_end) = curr_node.start_tag_end
            && curr_node.end_tag_start.is_none()
        {
            // The rare case of unmatching tag pairs like <div></div1>
            if start_tag_end != curr_node.end {
                return vec![(curr_node.start, curr_node.end)];
            }

            let close_text = document.get_text_range(Range {
                start: document.position_at(start_tag_end - 2),
                end: document.position_at(start_tag_end),
            });

            if close_text == "/>" {
                // Self-closing element
                result.insert(0, (curr_node.start + 1, start_tag_end - 2));
            } else {
                // Void element
                result.insert(0, (curr_node.start + 1, start_tag_end - 1));
            }

            let attribute_level_ranges =
                self.get_attribute_level_ranges(document, html_document, curr_node_id, curr_offset);
            let mut combined = attribute_level_ranges;
            combined.extend(result);
            return combined;
        }

        let (Some(start_tag_end), Some(end_tag_start)) =
            (curr_node.start_tag_end, curr_node.end_tag_start)
        else {
            return result;
        };

        result.insert(0, (curr_node.start, curr_node.end));

        if curr_node.start < curr_offset && curr_offset < start_tag_end {
            // Cursor inside `<div class="foo">`
            result.insert(0, (curr_node.start + 1, start_tag_end - 1));
            let attribute_level_ranges =
                self.get_attribute_level_ranges(document, html_document, curr_node_id, curr_offset);
            let mut combined = attribute_level_ranges;
            combined.extend(result);
            combined
        } else if start_tag_end <= curr_offset && curr_offset <= end_tag_start {
            // Cursor inside `bar`
            result.insert(0, (start_tag_end, end_tag_start));
            result
        } else {
            // Cursor inside `</div>`
            if curr_offset >= end_tag_start + 2 {
                result.insert(0, (end_tag_start + 2, curr_node.end - 1));
            }
            result
        }
    }

    fn get_all_parent_tag_ranges(
        &self,
        html_document: &HtmlDocument,
        initial_node: NodeId,
    ) -> Vec<(usize, usize)> {
        let mut curr_node = initial_node;
        let mut result: Vec<(usize, usize)> = Vec::new();
        while let Some(parent) = html_document.parent(curr_node) {
            curr_node = parent;
            result.extend(self.get_node_ranges(html_document, curr_node));
        }
        result
    }

    fn get_node_ranges(
        &self,
        html_document: &HtmlDocument,
        node_id: NodeId,
    ) -> Vec<(usize, usize)> {
        let n = html_document.node(node_id);
        if let (Some(start_tag_end), Some(end_tag_start)) = (n.start_tag_end, n.end_tag_start)
            && start_tag_end < end_tag_start
        {
            return vec![(start_tag_end, end_tag_start), (n.start, n.end)];
        }
        vec![(n.start, n.end)]
    }

    fn get_attribute_level_ranges(
        &self,
        document: &TextDocument,
        html_document: &HtmlDocument,
        node_id: NodeId,
        curr_offset: usize,
    ) -> Vec<(usize, usize)> {
        let curr_node = html_document.node(node_id);
        let curr_node_text = document.get_text_range(Range {
            start: document.position_at(curr_node.start),
            end: document.position_at(curr_node.end),
        });
        let relative_offset = curr_offset as i64 - curr_node.start as i64;
        let position_offset = curr_node.start;

        let mut scanner = create_scanner(&curr_node_text, 0, ScannerState::WithinContent, false);
        let mut token = scanner.scan();

        let mut result: Vec<(usize, usize)> = Vec::new();
        let mut is_inside_attribute = false;
        let mut attr_start = 0usize;

        while token != TokenType::Eos {
            match token {
                TokenType::AttributeName => {
                    if relative_offset < scanner.get_token_offset() as i64 {
                        is_inside_attribute = false;
                    } else {
                        if relative_offset <= scanner.get_token_end() as i64 {
                            // `class`
                            result.insert(0, (scanner.get_token_offset(), scanner.get_token_end()));
                        }
                        is_inside_attribute = true;
                        attr_start = scanner.get_token_offset();
                    }
                }
                TokenType::AttributeValue if is_inside_attribute => {
                    let value_text = scanner.get_token_text();
                    if relative_offset < scanner.get_token_offset() as i64 {
                        // `class="foo"`
                        result.push((attr_start, scanner.get_token_end()));
                    } else if relative_offset >= scanner.get_token_offset() as i64
                        && relative_offset <= scanner.get_token_end() as i64
                    {
                        // `"foo"`
                        result.insert(0, (scanner.get_token_offset(), scanner.get_token_end()));

                        // `foo`
                        let bytes = value_text.as_bytes();
                        let quoted = bytes.len() >= 2
                            && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
                                || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''));
                        if quoted
                            && relative_offset > scanner.get_token_offset() as i64
                            && relative_offset < scanner.get_token_end() as i64
                        {
                            result.insert(
                                0,
                                (scanner.get_token_offset() + 1, scanner.get_token_end() - 1),
                            );
                        }

                        // `class="foo"`
                        result.push((attr_start, scanner.get_token_end()));
                    }
                }
                _ => {}
            }
            token = scanner.scan();
        }

        result
            .into_iter()
            .map(|(a, b)| (a + position_offset, b + position_offset))
            .collect()
    }
}
