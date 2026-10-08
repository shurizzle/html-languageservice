//! Port of the js-beautify `Output` / `OutputLine` / `IndentStringCache`.
//!
//! `OutputLine` has no parent pointer (unlike the JavaScript original); the
//! operations that need the enclosing `Output` are implemented on `Output`.

use regex::Regex;

use super::options::BaseOptions;

fn utf16_len(s: &str) -> i64 {
    s.encode_utf16().count() as i64
}

/// Port of `new Array(count + 1).join(value)`.
fn repeat_count(value: &str, count: i64) -> String {
    value.repeat(count.max(0) as usize)
}

#[derive(Debug, Clone)]
struct OutputLine {
    character_count: i64,
    indent_count: i64,
    alignment_count: i64,
    wrap_point_index: usize,
    wrap_point_character_count: i64,
    wrap_point_indent_count: i64,
    wrap_point_alignment_count: i64,
    items: Vec<String>,
}

impl OutputLine {
    fn new() -> Self {
        Self {
            character_count: 0,
            indent_count: -1,
            alignment_count: 0,
            wrap_point_index: 0,
            wrap_point_character_count: 0,
            wrap_point_indent_count: -1,
            wrap_point_alignment_count: 0,
            items: Vec::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn last(&self) -> Option<&str> {
        self.items.last().map(|s| s.as_str())
    }

    /// Port of `item(index)` (negative index counts from the end).
    fn item(&self, index: i64) -> Option<&str> {
        let len = self.items.len() as i64;
        let i = if index < 0 { len + index } else { index };
        if i < 0 {
            None
        } else {
            self.items.get(i as usize).map(|s| s.as_str())
        }
    }

    fn push(&mut self, item: String) {
        // Port of `item.length - item.lastIndexOf('\n')`, i.e. the number of
        // UTF-16 code units from the last newline (inclusive) to the end.
        match item.rfind('\n') {
            Some(idx) => {
                self.character_count = utf16_len(&item[idx..]);
            }
            None => self.character_count += utf16_len(&item),
        }
        self.items.push(item);
    }

    fn pop(&mut self) -> Option<String> {
        let item = self.items.pop();
        if let Some(ref item) = item {
            self.character_count -= utf16_len(item);
        }
        item
    }

    fn trim(&mut self) {
        while self.last() == Some(" ") {
            self.items.pop();
            self.character_count -= 1;
        }
    }
}

#[derive(Debug, Clone)]
struct IndentStringCache {
    cache: Vec<String>,
    indent_size: i64,
    indent_string: String,
    base_string: String,
    base_string_length: i64,
}

impl IndentStringCache {
    fn new(options: &BaseOptions, base_indent_string: &str) -> Self {
        let mut indent_string = options.indent_char.clone();
        if !options.indent_with_tabs {
            indent_string = repeat_count(&options.indent_char, options.indent_size as i64);
        }

        let mut base_indent_string = base_indent_string.to_string();
        if options.indent_level > 0 {
            base_indent_string = repeat_count(&indent_string, options.indent_level as i64);
        }

        Self {
            cache: vec![String::new()],
            indent_size: options.indent_size as i64,
            indent_string,
            base_string_length: base_indent_string.len() as i64,
            base_string: base_indent_string,
        }
    }

    fn get_indent_size(&self, indent: i64, column: i64) -> i64 {
        let mut result = if indent < 0 {
            0
        } else {
            self.base_string_length
        };
        result += indent * self.indent_size;
        result += column;
        result
    }

    fn get_indent_string(&mut self, indent_level: i64, column: i64) -> String {
        let mut result = self.base_string.clone();
        let mut indent_level = indent_level;
        if indent_level < 0 {
            indent_level = 0;
            result = String::new();
        }
        let column = column + indent_level * self.indent_size;
        self.ensure_cache(column);
        result.push_str(&self.cache[column as usize]);
        result
    }

    fn ensure_cache(&mut self, column: i64) {
        while column >= self.cache.len() as i64 {
            self.add_column();
        }
    }

    fn add_column(&mut self) {
        let mut column = self.cache.len() as i64;
        let mut result = String::new();
        if self.indent_size != 0 && column >= self.indent_size {
            let indent = column / self.indent_size;
            column -= indent * self.indent_size;
            result = repeat_count(&self.indent_string, indent);
        }
        if column != 0 {
            result += &repeat_count(" ", column);
        }
        self.cache.push(result);
    }
}

/// Port of the js-beautify `Output` class.
pub struct Output {
    indent_cache: IndentStringCache,
    pub raw: bool,
    end_with_newline: bool,
    pub indent_size: i64,
    pub wrap_line_length: i64,
    pub indent_empty_lines: bool,
    lines: Vec<OutputLine>,
    previous_line: Option<usize>,
    current_line: usize,
    next_line: OutputLine,
    pub space_before_token: bool,
    pub non_breaking_space: bool,
    pub previous_token_wrapped: bool,
}

impl Output {
    pub fn new(options: &BaseOptions, base_indent_string: &str) -> Self {
        let indent_cache = IndentStringCache::new(options, base_indent_string);
        let mut result = Self {
            indent_size: options.indent_size as i64,
            end_with_newline: options.end_with_newline,
            wrap_line_length: options.wrap_line_length as i64,
            indent_empty_lines: options.indent_empty_lines,
            indent_cache,
            raw: false,
            lines: Vec::new(),
            previous_line: None,
            current_line: 0,
            next_line: OutputLine::new(),
            space_before_token: false,
            non_breaking_space: false,
            previous_token_wrapped: false,
        };
        result.add_outputline();
        result
    }

    fn add_outputline(&mut self) {
        self.previous_line = Some(self.current_line);
        let cloned = self.next_line.clone();
        self.lines.push(cloned);
        self.current_line = self.lines.len() - 1;
    }

    pub fn get_line_number(&self) -> usize {
        self.lines.len()
    }

    pub fn get_indent_string(&mut self, indent: i64, column: i64) -> String {
        self.indent_cache.get_indent_string(indent, column)
    }

    fn set_line_indent(&mut self, index: usize, indent: i64, alignment: i64) {
        if self.lines[index].is_empty() {
            self.lines[index].indent_count = indent;
            self.lines[index].alignment_count = alignment;
            self.lines[index].character_count =
                self.indent_cache.get_indent_size(indent, alignment);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.previous_line.is_none() && self.lines[self.current_line].is_empty()
    }

    pub fn add_new_line(&mut self, force_newline: bool) -> bool {
        if self.is_empty() || (!force_newline && self.just_added_newline()) {
            return false;
        }
        if !self.raw {
            self.add_outputline();
        }
        true
    }

    pub fn just_added_newline(&self) -> bool {
        self.lines[self.current_line].is_empty()
    }

    pub fn just_added_blankline(&self) -> bool {
        self.is_empty()
            || (self.lines[self.current_line].is_empty()
                && self
                    .previous_line
                    .map(|p| self.lines[p].is_empty())
                    .unwrap_or(false))
    }

    pub fn previous_line_exists(&self) -> bool {
        self.previous_line.is_some()
    }

    pub fn previous_line_last_item(&self) -> Option<&str> {
        self.previous_line.and_then(|p| self.lines[p].last())
    }

    pub fn current_line_has_match(&self, pattern: &Regex) -> bool {
        self.lines[self.current_line]
            .items
            .iter()
            .any(|item| pattern.is_match(item))
    }

    pub fn set_wrap_point(&mut self) {
        let line = &mut self.lines[self.current_line];
        if self.wrap_line_length != 0 {
            line.wrap_point_index = line.items.len();
            line.wrap_point_character_count = line.character_count;
            line.wrap_point_indent_count = self.next_line.indent_count;
            line.wrap_point_alignment_count = self.next_line.alignment_count;
        }
    }

    pub fn set_indent(&mut self, indent: Option<i64>, alignment: Option<i64>) -> bool {
        let indent = indent.unwrap_or(0);
        let alignment = alignment.unwrap_or(0);

        self.set_line_indent_next(indent, alignment);

        if self.lines.len() > 1 {
            self.set_line_indent(self.current_line, indent, alignment);
            return true;
        }

        self.set_line_indent(self.current_line, 0, 0);
        false
    }

    fn set_line_indent_next(&mut self, indent: i64, alignment: i64) {
        if self.next_line.is_empty() {
            self.next_line.indent_count = indent;
            self.next_line.alignment_count = alignment;
            self.next_line.character_count = self.indent_cache.get_indent_size(indent, alignment);
        }
    }

    pub fn add_raw_token(&mut self, newlines: usize, whitespace_before: &str, text: &str) {
        for _ in 0..newlines {
            self.add_outputline();
        }
        self.set_line_indent(self.current_line, -1, 0);
        self.lines[self.current_line].push(whitespace_before.to_string());
        self.lines[self.current_line].push(text.to_string());
        self.space_before_token = false;
        self.non_breaking_space = false;
        self.previous_token_wrapped = false;
    }

    pub fn add_token(&mut self, printable_token: &str) {
        self.add_space_before_token();
        self.lines[self.current_line].push(printable_token.to_string());
        self.space_before_token = false;
        self.non_breaking_space = false;
        self.previous_token_wrapped = self.allow_wrap();
    }

    fn add_space_before_token(&mut self) {
        if self.space_before_token && !self.just_added_newline() {
            if !self.non_breaking_space {
                self.set_wrap_point();
            }
            self.lines[self.current_line].push(" ".to_string());
        }
    }

    /// Port of `OutputLine._should_wrap`.
    fn should_wrap(&self) -> bool {
        let line = &self.lines[self.current_line];
        line.wrap_point_index != 0
            && line.character_count > self.wrap_line_length
            && line.wrap_point_character_count > self.next_line.character_count
    }

    /// Port of `OutputLine._allow_wrap`.
    fn allow_wrap(&mut self) -> bool {
        if !self.should_wrap() {
            return false;
        }
        let old = self.current_line;
        self.add_new_line(false);
        let new = self.current_line;

        let wrap_point_index = self.lines[old].wrap_point_index;
        let wrap_point_character_count = self.lines[old].wrap_point_character_count;
        let wrap_point_indent_count = self.lines[old].wrap_point_indent_count;
        let wrap_point_alignment_count = self.lines[old].wrap_point_alignment_count;

        self.set_line_indent(new, wrap_point_indent_count, wrap_point_alignment_count);

        let mut items = std::mem::take(&mut self.lines[old].items);
        let tail = items.split_off(wrap_point_index);
        self.lines[new].items = tail;
        self.lines[old].items = items;

        self.lines[new].character_count +=
            self.lines[old].character_count - wrap_point_character_count;
        self.lines[old].character_count = wrap_point_character_count;

        if self.lines[new].items.first().map(|s| s.as_str()) == Some(" ") {
            self.lines[new].items.remove(0);
            self.lines[new].character_count -= 1;
        }
        true
    }

    pub fn remove_indent(&mut self, index: usize) {
        let output_length = self.lines.len();
        for i in index..output_length {
            if self.lines[i].indent_count > 0 {
                self.lines[i].indent_count -= 1;
                self.lines[i].character_count -= self.indent_size;
            }
        }
        if self.lines[self.current_line].wrap_point_indent_count > 0 {
            self.lines[self.current_line].wrap_point_indent_count -= 1;
        }
    }

    pub fn trim(&mut self, eat_newlines: bool) {
        self.lines[self.current_line].trim();

        while eat_newlines && self.lines.len() > 1 && self.lines[self.current_line].is_empty() {
            self.lines.pop();
            self.current_line = self.lines.len() - 1;
            self.lines[self.current_line].trim();
        }

        self.previous_line = if self.lines.len() > 1 {
            Some(self.lines.len() - 2)
        } else {
            None
        };
    }

    pub fn ensure_empty_line_above(&mut self, starts_with: &str, ends_with: &str) {
        let mut index = self.lines.len() as i64 - 2;
        while index >= 0 {
            let potential = &self.lines[index as usize];
            if potential.is_empty() {
                break;
            }
            let starts = potential
                .item(0)
                .map(|s| s.starts_with(starts_with))
                .unwrap_or(false);
            let ends = potential.item(-1) == Some(ends_with);
            if !starts && !ends {
                let insert_at = index as usize + 1;
                self.lines.insert(insert_at, OutputLine::new());
                if insert_at <= self.current_line {
                    self.current_line += 1;
                }
                self.previous_line = self.lines.len().checked_sub(2);
                break;
            }
            index -= 1;
        }
    }

    fn line_to_string(&mut self, index: usize) -> String {
        let (is_empty, indent_count, alignment_count) = {
            let line = &self.lines[index];
            (line.is_empty(), line.indent_count, line.alignment_count)
        };
        let mut result = String::new();
        if is_empty {
            if self.indent_empty_lines {
                result = self.indent_cache.get_indent_string(indent_count, 0);
            }
        } else {
            result = self
                .indent_cache
                .get_indent_string(indent_count, alignment_count);
            result += &self.lines[index].items.concat();
        }
        result
    }

    pub fn get_code(&mut self, eol: &str) -> String {
        self.trim(true);

        let last_item = self.lines[self.current_line].pop();
        if let Some(mut last_item) = last_item {
            if last_item.ends_with('\n') {
                last_item = last_item.trim_end_matches('\n').to_string();
            }
            self.lines[self.current_line].push(last_item);
        }

        if self.end_with_newline {
            self.add_outputline();
        }

        let mut lines = Vec::with_capacity(self.lines.len());
        for i in 0..self.lines.len() {
            lines.push(self.line_to_string(i));
        }
        let mut sweet_code = lines.join("\n");

        if eol != "\n" {
            sweet_code = sweet_code.replace('\n', eol);
        }
        sweet_code
    }
}
