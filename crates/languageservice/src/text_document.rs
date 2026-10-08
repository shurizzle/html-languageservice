//! Port of `vscode-languageserver-textdocument`'s `TextDocument`.
//!
//! Line/character offsets are expressed in UTF-16 code units, matching the
//! JavaScript implementation (and therefore the scanner, which also works on
//! UTF-16 code units).

use crate::types::{Position, Range, TextEdit};

#[derive(Debug, Clone)]
pub struct TextDocument {
    uri: String,
    language_id: String,
    version: i32,
    text: String,
    utf16_len: usize,
    line_offsets_utf16: Vec<usize>,
    line_offsets_byte: Vec<usize>,
}

impl TextDocument {
    pub fn create(
        uri: impl Into<String>,
        language_id: impl Into<String>,
        version: i32,
        content: impl Into<String>,
    ) -> Self {
        let text = content.into();
        let (utf16_len, line_offsets_utf16, line_offsets_byte) = compute_line_offsets(&text);
        Self {
            uri: uri.into(),
            language_id: language_id.into(),
            version,
            text,
            utf16_len,
            line_offsets_utf16,
            line_offsets_byte,
        }
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }

    pub fn language_id(&self) -> &str {
        &self.language_id
    }

    pub fn version(&self) -> i32 {
        self.version
    }

    pub fn line_count(&self) -> usize {
        self.line_offsets_utf16.len()
    }

    /// Full document text.
    pub fn get_text(&self) -> &str {
        &self.text
    }

    /// Text covered by `range`.
    pub fn get_text_range(&self, range: Range) -> String {
        let start = self.byte_offset_at(self.offset_at(range.start));
        let end = self.byte_offset_at(self.offset_at(range.end));
        self.text[start..end].to_string()
    }

    pub fn position_at(&self, offset: usize) -> Position {
        let offset = offset.min(self.utf16_len);
        let line = self.line_of(offset);
        let line_offset = self.line_offsets_utf16[line];
        let offset = self.ensure_before_eol(offset, line_offset);
        Position {
            line: line as u32,
            character: (offset - line_offset) as u32,
        }
    }

    pub fn offset_at(&self, position: Position) -> usize {
        let line = position.line as usize;
        if line >= self.line_offsets_utf16.len() {
            return self.utf16_len;
        }
        let line_offset = self.line_offsets_utf16[line];
        if position.character == 0 {
            return line_offset;
        }
        let next_line_offset = if line + 1 < self.line_offsets_utf16.len() {
            self.line_offsets_utf16[line + 1]
        } else {
            self.utf16_len
        };
        let offset = (line_offset + position.character as usize).min(next_line_offset);
        self.ensure_before_eol(offset, line_offset)
    }

    fn line_of(&self, utf16: usize) -> usize {
        self.line_offsets_utf16
            .partition_point(|&x| x <= utf16)
            .saturating_sub(1)
    }

    /// Byte offset in the underlying UTF-8 string for a UTF-16 code unit offset.
    pub fn byte_offset_at(&self, utf16: usize) -> usize {
        let utf16 = utf16.min(self.utf16_len);
        let line = self.line_of(utf16);
        let mut u = self.line_offsets_utf16[line];
        let mut b = self.line_offsets_byte[line];
        let end_b = self
            .line_offsets_byte
            .get(line + 1)
            .copied()
            .unwrap_or(self.text.len());
        while u < utf16 && b < end_b {
            let ch = self.text[b..].chars().next().unwrap();
            let w = ch.len_utf16();
            if u + w > utf16 {
                break;
            }
            u += w;
            b += ch.len_utf8();
        }
        b
    }

    fn ensure_before_eol(&self, offset: usize, line_offset: usize) -> usize {
        let mut offset = offset;
        while offset > line_offset {
            let b = self.byte_offset_at(offset - 1);
            let ch = self.text[b..].chars().next().unwrap();
            if ch != '\r' && ch != '\n' {
                break;
            }
            offset -= 1;
        }
        offset
    }
}

/// Returns `(utf16_len, utf16_line_offsets, byte_line_offsets)`.
fn compute_line_offsets(text: &str) -> (usize, Vec<usize>, Vec<usize>) {
    let mut utf16_offsets = vec![0usize];
    let mut byte_offsets = vec![0usize];
    let mut utf16 = 0usize;
    let mut iter = text.char_indices().peekable();
    while let Some((b, ch)) = iter.next() {
        match ch {
            '\r' => {
                if let Some(&(_, '\n')) = iter.peek() {
                    iter.next();
                    utf16 += 2;
                    utf16_offsets.push(utf16);
                    byte_offsets.push(b + 2);
                } else {
                    utf16 += 1;
                    utf16_offsets.push(utf16);
                    byte_offsets.push(b + 1);
                }
            }
            '\n' => {
                utf16 += 1;
                utf16_offsets.push(utf16);
                byte_offsets.push(b + 1);
            }
            _ => utf16 += ch.len_utf16(),
        }
    }
    (utf16, utf16_offsets, byte_offsets)
}

/// Port of `TextDocument.applyEdits`.
pub fn apply_edits(document: &TextDocument, edits: &[TextEdit]) -> String {
    let text = document.get_text();
    let mut sorted: Vec<&TextEdit> = edits.iter().collect();
    sorted.sort_by_key(|e| (e.range.start.line, e.range.start.character));

    let mut spans = String::new();
    let mut last_modified_offset = 0usize;
    for e in sorted {
        let start_offset = document.offset_at(e.range.start);
        if start_offset < last_modified_offset {
            continue;
        }
        if start_offset > last_modified_offset {
            spans.push_str(
                &text[document.byte_offset_at(last_modified_offset)
                    ..document.byte_offset_at(start_offset)],
            );
        }
        spans.push_str(&e.new_text);
        last_modified_offset = document.offset_at(e.range.end);
    }
    spans.push_str(&text[document.byte_offset_at(last_modified_offset)..]);
    spans
}
