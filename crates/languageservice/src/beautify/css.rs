//! Port of `src/beautify/beautify-css.js` (`css_beautify`).

use regex::Regex;
use std::sync::OnceLock;

use super::options::{BaseOptions, RawOptions, get_boolean, get_selection_list};
use super::output::Output;
use super::scanner::{Directives, InputScanner, Pattern, ignores_start};

/// Port of the CSS `Options` class.
pub struct CssOptions {
    pub base: BaseOptions,
    pub selector_separator_newline: bool,
    pub newline_between_rules: bool,
    pub space_around_combinator: bool,
    pub brace_style: String,
}

impl CssOptions {
    pub fn new(options: &RawOptions) -> Self {
        let base = BaseOptions::new(options, Some("css"));
        let raw = &base.raw_options;
        let selector_separator_newline = get_boolean(raw, "selector_separator_newline", true);
        let newline_between_rules = get_boolean(raw, "newline_between_rules", true);
        let space_around_selector_separator =
            get_boolean(raw, "space_around_selector_separator", false);
        let space_around_combinator =
            get_boolean(raw, "space_around_combinator", false) || space_around_selector_separator;

        let brace_style_split = get_selection_list(
            raw,
            "brace_style",
            &[
                "collapse",
                "expand",
                "end-expand",
                "none",
                "preserve-inline",
            ],
            None,
        );
        let mut brace_style = "collapse".to_string();
        for bs in &brace_style_split {
            if bs != "expand" {
                brace_style = "collapse".to_string();
            } else {
                brace_style = bs.clone();
            }
        }

        Self {
            selector_separator_newline,
            newline_between_rules,
            space_around_combinator,
            brace_style,
            base,
        }
    }
}

fn line_break() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("\\r\\n|[\\r\\n]").unwrap())
}

fn whitespace_char() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("\\s").unwrap())
}

fn whitespace_pattern() -> &'static Pattern {
    static P: OnceLock<Pattern> = OnceLock::new();
    P.get_or_init(|| Pattern::new("(?:\\s|\\n)+"))
}

fn block_comment_pattern() -> &'static Pattern {
    static P: OnceLock<Pattern> = OnceLock::new();
    P.get_or_init(|| Pattern::new("/\\*(?:[\\s\\S]*?)((?:\\*/)|$)"))
}

fn comment_pattern() -> &'static Pattern {
    static P: OnceLock<Pattern> = OnceLock::new();
    P.get_or_init(|| Pattern::new("//(?:[^\n\r\u{2028}\u{2029}]*)"))
}

fn variable_boundary_pattern() -> &'static Pattern {
    static P: OnceLock<Pattern> = OnceLock::new();
    P.get_or_init(|| Pattern::new(r#"[: ,;{}()\[\]/='"]"#))
}

fn directives_core() -> &'static Directives {
    static D: OnceLock<Directives> = OnceLock::new();
    D.get_or_init(|| Directives::new("/\\*", "\\*/"))
}

const NESTED_AT_RULE: [&str; 6] = [
    "page",
    "font-face",
    "keyframes",
    "media",
    "supports",
    "document",
];
const CONDITIONAL_GROUP_RULE: [&str; 3] = ["media", "supports", "document"];
const NON_SEMICOLON_NEWLINE_PROPERTY: [&str; 2] = ["grid-template-areas", "grid-template"];

/// Port of the CSS `Beautifier` class.
pub struct CssBeautifier {
    source_text: String,
    options: CssOptions,
    ch: Option<String>,
    input: InputScanner,
    output: Output,
    indent_level: i32,
    nested_level: i32,
}

impl CssBeautifier {
    pub fn new(source_text: &str, options: &RawOptions) -> Self {
        let options = CssOptions::new(options);
        let output = Output::new(&options.base, "");
        Self {
            source_text: source_text.to_string(),
            options,
            ch: None,
            input: InputScanner::new(""),
            output,
            indent_level: 0,
            nested_level: 0,
        }
    }

    /// Port of `Beautifier.prototype.eatString`.
    fn eat_string(&mut self, end_chars: &str) -> String {
        let mut result = String::new();
        self.ch = self.input.next_char().map(|c| c.to_string());
        while let Some(ch) = self.ch.clone() {
            if ch.is_empty() {
                break;
            }
            result.push_str(&ch);
            if ch == "\\" {
                if let Some(next) = self.input.next_char() {
                    result.push(next);
                }
            } else if end_chars.contains(&ch) || ch == "\n" {
                break;
            }
            self.ch = self.input.next_char().map(|c| c.to_string());
        }
        result
    }

    /// Port of `Beautifier.prototype.eatWhitespace`.
    fn eat_whitespace(&mut self, allow_at_least_one_new_line: bool) -> bool {
        let result = self.input.test_char(whitespace_char(), 0);
        let mut newline_count = 0;
        while self.input.test_char(whitespace_char(), 0) {
            self.ch = self.input.next_char().map(|c| c.to_string());
            if allow_at_least_one_new_line && self.ch.as_deref() == Some("\n")
                && (newline_count == 0 || newline_count < self.options.base.max_preserve_newlines) {
                    newline_count += 1;
                    self.output.add_new_line(true);
                }
        }
        result
    }

    /// Port of `Beautifier.prototype.foundNestedPseudoClass`.
    fn found_nested_pseudo_class(&self) -> bool {
        let mut open_paren = 0;
        let mut i = 1i64;
        let mut ch = self.input.peek(i);
        while let Some(c) = ch {
            if c == '{' {
                return true;
            } else if c == '(' {
                open_paren += 1;
            } else if c == ')' {
                if open_paren == 0 {
                    return false;
                }
                open_paren -= 1;
            } else if c == ';' || c == '}' {
                return false;
            }
            i += 1;
            ch = self.input.peek(i);
        }
        false
    }

    /// Port of `Beautifier.prototype.print_string`.
    fn print_string(&mut self, output_string: &str) {
        self.output.set_indent(Some(self.indent_level as i64), None);
        self.output.non_breaking_space = true;
        self.output.add_token(output_string);
    }

    /// Port of `Beautifier.prototype.preserveSingleSpace`.
    fn preserve_single_space(&mut self, is_after_space: bool) {
        if is_after_space {
            self.output.space_before_token = true;
        }
    }

    fn indent(&mut self) {
        self.indent_level += 1;
    }

    fn outdent(&mut self) {
        if self.indent_level > 0 {
            self.indent_level -= 1;
        }
    }

    pub fn beautify(&mut self) -> String {
        if self.options.base.disabled {
            return self.source_text.clone();
        }

        let source_text = self.source_text.clone();
        let mut eol = self.options.base.eol.clone();
        if eol == "auto" {
            eol = "\n".to_string();
            if !source_text.is_empty()
                && let Some(m) = line_break().find(&source_text)
            {
                eol = m.as_str().to_string();
            }
        }

        let source_text = line_break().replace_all(&source_text, "\n").to_string();
        let base_indent_string = source_text
            .find(|c: char| c != '\t' && c != ' ')
            .map(|i| &source_text[..i])
            .unwrap_or(source_text.as_str())
            .to_string();

        self.output = Output::new(&self.options.base, &base_indent_string);
        self.input = InputScanner::new(&source_text);
        self.indent_level = 0;
        self.nested_level = 0;
        self.ch = None;
        let mut paren_level = 0;
        let mut inside_rule = false;
        let mut inside_property_value = false;
        let mut entering_conditional_group = false;
        let mut inside_non_nested_at_rule = false;
        let mut inside_scss_map = false;
        let mut top_character: Option<String> = self.ch.clone();
        let mut inside_non_semi_colon_values = false;

        loop {
            let whitespace = self.input.read(Some(whitespace_pattern()), None, false);
            let is_after_space = !whitespace.is_empty();
            let previous_ch = top_character.clone();
            self.ch = self.input.next_char().map(|c| c.to_string());
            if self.ch.as_deref() == Some("\\") && self.input.has_next()
                && let Some(next) = self.input.next_char() {
                    self.ch = Some(format!("{}{next}", self.ch.clone().unwrap_or_default()));
                }
            top_character = self.ch.clone();

            let ch = self.ch.clone();
            if ch.as_deref().is_none_or(|s| s.is_empty()) {
                break;
            }
            let ch = ch.unwrap();

            if ch == "/" && self.input.peek(0) == Some('*') {
                self.output.add_new_line(false);
                self.input.back();

                let mut comment = self.input.read(Some(block_comment_pattern()), None, false);

                let directives = directives_core().get_directives(&comment);
                if ignores_start(&directives) {
                    comment += &directives_core().read_ignored(&mut self.input);
                }

                self.print_string(&comment);

                self.eat_whitespace(true);

                self.output.add_new_line(false);
            } else if ch == "/" && self.input.peek(0) == Some('/') {
                self.output.space_before_token = true;
                self.input.back();
                let comment = self.input.read(Some(comment_pattern()), None, false);
                self.print_string(&comment);

                self.eat_whitespace(true);
            } else if ch == "$" {
                self.preserve_single_space(is_after_space);

                self.print_string(&ch);

                let mut variable = self.input.peek_until_after(variable_boundary_pattern());

                if ends_with_space_or_colon(&variable) {
                    variable = self.eat_string(": ").trim_end().to_string();
                    self.print_string(&variable);
                    self.output.space_before_token = true;
                }

                if paren_level == 0 && variable.contains(':') {
                    inside_property_value = true;
                    self.indent();
                }
            } else if ch == "@" {
                self.preserve_single_space(is_after_space);

                if self.input.peek(0) == Some('{') {
                    let s = self.eat_string("}");
                    self.print_string(&format!("{ch}{s}"));
                } else {
                    self.print_string(&ch);

                    let mut variable_or_rule =
                        self.input.peek_until_after(variable_boundary_pattern());

                    if ends_with_space_or_colon(&variable_or_rule) {
                        variable_or_rule = self.eat_string(": ").trim_end().to_string();
                        self.print_string(&variable_or_rule);
                        self.output.space_before_token = true;
                    }

                    if paren_level == 0 && variable_or_rule.contains(':') {
                        inside_property_value = true;
                        self.indent();
                    } else if NESTED_AT_RULE.contains(&variable_or_rule.as_str()) {
                        self.nested_level += 1;
                        if CONDITIONAL_GROUP_RULE.contains(&variable_or_rule.as_str()) {
                            entering_conditional_group = true;
                        }
                    } else if paren_level == 0 && !inside_property_value {
                        inside_non_nested_at_rule = true;
                    }
                }
            } else if ch == "#" && self.input.peek(0) == Some('{') {
                self.preserve_single_space(is_after_space);
                let s = self.eat_string("}");
                self.print_string(&format!("{ch}{s}"));
            } else if ch == "{" {
                if inside_property_value {
                    inside_property_value = false;
                    self.outdent();
                }

                inside_non_nested_at_rule = false;

                if entering_conditional_group {
                    entering_conditional_group = false;
                    inside_rule = self.indent_level >= self.nested_level;
                } else {
                    inside_rule = self.indent_level >= self.nested_level - 1;
                }
                if self.options.newline_between_rules
                    && inside_rule
                    && self.output.previous_line_exists()
                    && self.output.previous_line_last_item() != Some("{")
                {
                    self.output.ensure_empty_line_above("/", ",");
                }

                self.output.space_before_token = true;

                if self.options.brace_style == "expand" {
                    self.output.add_new_line(false);
                    self.print_string(&ch);
                    self.indent();
                    self.output.set_indent(Some(self.indent_level as i64), None);
                } else {
                    if previous_ch.as_deref() == Some("(") {
                        self.output.space_before_token = false;
                    } else if previous_ch.as_deref() != Some(",") {
                        self.indent();
                    }
                    self.print_string(&ch);
                }

                self.eat_whitespace(true);
                self.output.add_new_line(false);
            } else if ch == "}" {
                self.outdent();
                self.output.add_new_line(false);
                if previous_ch.as_deref() == Some("{") {
                    self.output.trim(true);
                }

                if inside_property_value {
                    self.outdent();
                    inside_property_value = false;
                }
                self.print_string(&ch);
                inside_rule = false;
                if self.nested_level != 0 {
                    self.nested_level -= 1;
                }

                self.eat_whitespace(true);
                self.output.add_new_line(false);

                if self.options.newline_between_rules && !self.output.just_added_blankline()
                    && self.input.peek(0) != Some('}') {
                        self.output.add_new_line(true);
                    }
                if self.input.peek(0) == Some(')') {
                    self.output.trim(true);
                    if self.options.brace_style == "expand" {
                        self.output.add_new_line(true);
                    }
                }
            } else if ch == ":" {
                for property in NON_SEMICOLON_NEWLINE_PROPERTY {
                    if self.input.look_back(property) {
                        inside_non_semi_colon_values = true;
                        break;
                    }
                }

                if (inside_rule || entering_conditional_group)
                    && !(self.input.look_back("&") || self.found_nested_pseudo_class())
                    && !self.input.look_back("(")
                    && !inside_non_nested_at_rule
                    && paren_level == 0
                {
                    self.print_string(":");
                    if !inside_property_value {
                        inside_property_value = true;
                        self.output.space_before_token = true;
                        self.eat_whitespace(true);
                        self.indent();
                    }
                } else {
                    if self.input.look_back(" ") {
                        self.output.space_before_token = true;
                    }
                    if self.input.peek(0) == Some(':') {
                        self.ch = self.input.next_char().map(|c| c.to_string());
                        self.print_string("::");
                    } else {
                        self.print_string(":");
                    }
                }
            } else if ch == "\"" || ch == "'" {
                let preserve_quote_space =
                    previous_ch.as_deref() == Some("\"") || previous_ch.as_deref() == Some("'");
                self.preserve_single_space(preserve_quote_space || is_after_space);
                let s = self.eat_string(&ch);
                self.print_string(&format!("{ch}{s}"));
                self.eat_whitespace(true);
            } else if ch == ";" {
                inside_non_semi_colon_values = false;
                if paren_level == 0 {
                    if inside_property_value {
                        self.outdent();
                        inside_property_value = false;
                    }
                    inside_non_nested_at_rule = false;
                    self.print_string(&ch);
                    self.eat_whitespace(true);

                    if self.input.peek(0) != Some('/') {
                        self.output.add_new_line(false);
                    }
                } else {
                    self.print_string(&ch);
                    self.eat_whitespace(true);
                    self.output.space_before_token = true;
                }
            } else if ch == "(" {
                if self.input.look_back("url") {
                    self.print_string(&ch);
                    self.eat_whitespace(false);
                    paren_level += 1;
                    self.indent();
                    self.ch = self.input.next_char().map(|c| c.to_string());
                    if self.ch.as_deref() == Some(")")
                        || self.ch.as_deref() == Some("\"")
                        || self.ch.as_deref() == Some("'")
                    {
                        self.input.back();
                    } else if let Some(inner) = self.ch.clone()
                        && !inner.is_empty() {
                            let s = self.eat_string(")");
                            self.print_string(&format!("{inner}{s}"));
                            if paren_level != 0 {
                                paren_level -= 1;
                                self.outdent();
                            }
                        }
                } else {
                    let mut space_needed = false;
                    if self.input.look_back("with") {
                        space_needed = true;
                    }
                    self.preserve_single_space(is_after_space || space_needed);
                    self.print_string(&ch);

                    if inside_property_value
                        && previous_ch.as_deref() == Some("$")
                        && self.options.selector_separator_newline
                    {
                        self.output.add_new_line(false);
                        inside_scss_map = true;
                    } else {
                        self.eat_whitespace(false);
                        paren_level += 1;
                        self.indent();
                    }
                }
            } else if ch == ")" {
                if paren_level != 0 {
                    paren_level -= 1;
                    self.outdent();
                }
                if inside_scss_map
                    && self.input.peek(0) == Some(';')
                    && self.options.selector_separator_newline
                {
                    inside_scss_map = false;
                    self.outdent();
                    self.output.add_new_line(false);
                }
                self.print_string(&ch);
            } else if ch == "," {
                self.print_string(&ch);
                self.eat_whitespace(true);
                if self.options.selector_separator_newline
                    && (!inside_property_value || inside_scss_map)
                    && paren_level == 0
                    && !inside_non_nested_at_rule
                {
                    self.output.add_new_line(false);
                } else {
                    self.output.space_before_token = true;
                }
            } else if (ch == ">" || ch == "+" || ch == "~")
                && !inside_property_value
                && paren_level == 0
            {
                if self.options.space_around_combinator {
                    self.output.space_before_token = true;
                    self.print_string(&ch);
                    self.output.space_before_token = true;
                } else {
                    self.print_string(&ch);
                    self.eat_whitespace(false);
                    if self.ch_is_whitespace() {
                        self.ch = Some(String::new());
                    }
                }
            } else if ch == "]" {
                self.print_string(&ch);
            } else if ch == "[" {
                self.preserve_single_space(is_after_space);
                self.print_string(&ch);
            } else if ch == "=" {
                self.eat_whitespace(false);
                self.print_string("=");
                if self.ch_is_whitespace() {
                    self.ch = Some(String::new());
                }
            } else if ch == "!" && !self.input.look_back("\\") {
                self.output.space_before_token = true;
                self.print_string(&ch);
            } else {
                let preserve_after_space =
                    previous_ch.as_deref() == Some("\"") || previous_ch.as_deref() == Some("'");
                self.preserve_single_space(preserve_after_space || is_after_space);
                self.print_string(&ch);

                if !self.output.just_added_newline()
                    && self.input.peek(0) == Some('\n')
                    && inside_non_semi_colon_values
                {
                    self.output.add_new_line(false);
                }
            }
        }

        self.output.get_code(&eol)
    }

    fn ch_is_whitespace(&self) -> bool {
        match &self.ch {
            Some(s) => whitespace_char().is_match(s),
            None => false,
        }
    }
}

fn ends_with_space_or_colon(s: &str) -> bool {
    s.ends_with(' ') || s.ends_with(':')
}

/// Port of `css_beautify`.
pub fn css_beautify(source_text: &str, options: &RawOptions) -> String {
    let mut beautifier = CssBeautifier::new(source_text, options);
    beautifier.beautify()
}
