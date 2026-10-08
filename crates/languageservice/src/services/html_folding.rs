//! Port of `src/services/htmlFolding.ts`.

use crate::html_language_types::TokenType;
use crate::language_facts::DataManager;
use crate::parser::html_scanner::create_scanner;
use crate::text_document::TextDocument;
use crate::types::{FoldingRange, FoldingRangeKind};
use crate::utils::strings::is_js_whitespace;

pub struct HtmlFolding<'a> {
    data_manager: &'a DataManager,
}

impl<'a> HtmlFolding<'a> {
    pub fn new(data_manager: &'a DataManager) -> Self {
        Self { data_manager }
    }

    fn limit_ranges(&self, mut ranges: Vec<FoldingRange>, range_limit: usize) -> Vec<FoldingRange> {
        ranges.sort_by_key(|r| (r.start_line, r.end_line));

        // compute each range's nesting level in 'nesting_levels'.
        // count the number of ranges for each level in 'nesting_level_counts'.
        let mut top: Option<FoldingRange> = None;
        let mut previous: Vec<FoldingRange> = Vec::new();
        let mut nesting_levels: Vec<Option<usize>> = Vec::new();
        let mut nesting_level_counts: Vec<Option<usize>> = Vec::new();

        for (i, range) in ranges.iter().enumerate() {
            let entry = range.clone();
            match &top {
                None => {
                    top = Some(entry);
                    set_nesting_level(&mut nesting_levels, &mut nesting_level_counts, i, 0);
                }
                Some(t) => {
                    if entry.start_line > t.start_line {
                        if entry.end_line <= t.end_line {
                            previous.push(top.clone().unwrap());
                            top = Some(entry);
                            let level = previous.len();
                            set_nesting_level(
                                &mut nesting_levels,
                                &mut nesting_level_counts,
                                i,
                                level,
                            );
                        } else if entry.start_line > t.end_line {
                            loop {
                                top = previous.pop();
                                match &top {
                                    Some(t2) if entry.start_line > t2.end_line => continue,
                                    _ => break,
                                }
                            }
                            if let Some(t) = &top {
                                previous.push(t.clone());
                            }
                            top = Some(entry);
                            let level = previous.len();
                            set_nesting_level(
                                &mut nesting_levels,
                                &mut nesting_level_counts,
                                i,
                                level,
                            );
                        }
                    }
                }
            }
        }

        let mut entries = 0usize;
        let mut max_level = 0usize;
        for (i, count) in nesting_level_counts.iter().enumerate() {
            if let Some(n) = count {
                if n + entries > range_limit {
                    max_level = i;
                    break;
                }
                entries += n;
            }
        }

        let mut result = Vec::new();
        for (i, range) in ranges.iter().enumerate() {
            if let Some(level) = nesting_levels.get(i).copied().flatten()
                && (level < max_level
                    || (level == max_level && {
                        let fits = entries < range_limit;
                        entries += 1;
                        fits
                    }))
            {
                result.push(range.clone());
            }
        }
        result
    }

    pub fn get_folding_ranges(
        &self,
        document: &TextDocument,
        context: Option<&FoldingRangeContext>,
    ) -> Vec<FoldingRange> {
        let mut scanner = create_scanner(
            document.get_text(),
            0,
            crate::html_language_types::ScannerState::WithinContent,
            false,
        );
        let mut token = scanner.scan();
        let mut ranges: Vec<FoldingRange> = Vec::new();
        let mut stack: Vec<(i64, String)> = Vec::new();
        let mut last_tag_name: Option<String> = None;
        let mut prev_start: i64 = -1;
        let mut void_elements: Option<Vec<String>> = None;

        while token != TokenType::Eos {
            match token {
                TokenType::StartTag => {
                    let tag_name = scanner.get_token_text();
                    let start_line = document.position_at(scanner.get_token_offset()).line as i64;
                    stack.push((start_line, tag_name.clone()));
                    last_tag_name = Some(tag_name);
                }
                TokenType::EndTag => {
                    last_tag_name = Some(scanner.get_token_text());
                }
                TokenType::StartTagClose
                | TokenType::EndTagClose
                | TokenType::StartTagSelfClose => {
                    let mut is_void = false;
                    if token == TokenType::StartTagClose {
                        if last_tag_name.is_none() {
                            token = scanner.scan();
                            continue;
                        }
                        let voids = void_elements.get_or_insert_with(|| {
                            self.data_manager.get_void_elements(document.language_id())
                        });
                        is_void = self
                            .data_manager
                            .is_void_element(last_tag_name.as_deref().unwrap_or(""), voids);
                    }
                    if token == TokenType::StartTagClose && !is_void {
                        token = scanner.scan();
                        continue;
                    }

                    let mut i = stack.len() as i64 - 1;
                    while i >= 0 && Some(stack[i as usize].1.as_str()) != last_tag_name.as_deref() {
                        i -= 1;
                    }
                    if i >= 0 {
                        let (start_line, _) = stack[i as usize].clone();
                        stack.truncate(i as usize);
                        let line = document.position_at(scanner.get_token_offset()).line as i64;
                        let end_line = line - 1;
                        if end_line > start_line && prev_start != start_line {
                            ranges.push(FoldingRange {
                                start_line: start_line as u32,
                                end_line: end_line as u32,
                                kind: None,
                            });
                            prev_start = start_line;
                        }
                    }
                }
                TokenType::Comment => {
                    let mut start_line =
                        document.position_at(scanner.get_token_offset()).line as i64;
                    let text = scanner.get_token_text();
                    match classify_comment(&text) {
                        Some(is_region_start) => {
                            if is_region_start {
                                stack.push((start_line, String::new()));
                            } else {
                                let mut i = stack.len() as i64 - 1;
                                while i >= 0 && !stack[i as usize].1.is_empty() {
                                    i -= 1;
                                }
                                if i >= 0 {
                                    let (stack_start_line, _) = stack[i as usize].clone();
                                    stack.truncate(i as usize);
                                    let end_line = start_line;
                                    start_line = stack_start_line;
                                    if end_line > start_line && prev_start != start_line {
                                        ranges.push(FoldingRange {
                                            start_line: start_line as u32,
                                            end_line: end_line as u32,
                                            kind: Some(FoldingRangeKind::Region),
                                        });
                                        prev_start = start_line;
                                    }
                                }
                            }
                        }
                        None => {
                            let end_line = document
                                .position_at(
                                    scanner.get_token_offset() + scanner.get_token_length(),
                                )
                                .line as i64;
                            if start_line < end_line {
                                ranges.push(FoldingRange {
                                    start_line: start_line as u32,
                                    end_line: end_line as u32,
                                    kind: Some(FoldingRangeKind::Comment),
                                });
                                prev_start = start_line;
                            }
                        }
                    }
                }
                _ => {}
            }
            token = scanner.scan();
        }

        let range_limit = context.and_then(|c| c.range_limit).unwrap_or(usize::MAX);
        if ranges.len() > range_limit {
            return self.limit_ranges(ranges, range_limit);
        }
        ranges
    }
}

fn set_nesting_level(
    levels: &mut Vec<Option<usize>>,
    counts: &mut Vec<Option<usize>>,
    index: usize,
    level: usize,
) {
    if levels.len() <= index {
        levels.resize(index + 1, None);
    }
    levels[index] = Some(level);
    if level < 30 {
        if counts.len() <= level {
            counts.resize(level + 1, None);
        }
        counts[level] = Some(counts[level].unwrap_or(0) + 1);
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FoldingRangeContext {
    pub range_limit: Option<usize>,
}

/// Port of `text.match(/^\s*#(region\b)|(endregion\b)/)`.
///
/// Returns `Some(true)` for a region start, `Some(false)` for a region end.
fn classify_comment(text: &str) -> Option<bool> {
    let trimmed = text.trim_start_matches(is_js_whitespace);
    if let Some(rest) = trimmed.strip_prefix("#region")
        && rest.chars().next().is_none_or(|c| !is_word_char(c))
    {
        return Some(true);
    }
    if let Some(idx) = text.find("endregion") {
        let rest = &text[idx + "endregion".len()..];
        if rest.chars().next().is_none_or(|c| !is_word_char(c)) {
            return Some(false);
        }
    }
    None
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}
