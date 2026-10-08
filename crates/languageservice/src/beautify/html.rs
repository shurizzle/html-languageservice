//! Port of `src/beautify/beautify-html.js` (`html_beautify`).
//!
//! Includes the js-beautify base `Tokenizer`, `Token`/`TokenStream` machinery
//! (the CSS port inlines its own loop instead) plus the HTML tokenizer and
//! `Beautifier`.

use regex::Regex;
use std::sync::OnceLock;

use super::options::{
    BaseOptions, OptValue, RawOptions, get_boolean, get_characters, get_number, get_selection,
    get_string_array,
};
use super::output::Output;
use super::scanner::{
    Directives, InputScanner, JsPattern, Pattern, TemplatablePattern, ignores_start,
};

pub const TOKEN_START: &str = "TK_START";
pub const TOKEN_RAW: &str = "TK_RAW";
pub const TOKEN_EOF: &str = "TK_EOF";

pub const TOKEN_TAG_OPEN: &str = "TK_TAG_OPEN";
pub const TOKEN_TAG_CLOSE: &str = "TK_TAG_CLOSE";
pub const TOKEN_CONTROL_FLOW_OPEN: &str = "TK_CONTROL_FLOW_OPEN";
pub const TOKEN_CONTROL_FLOW_CLOSE: &str = "TK_CONTROL_FLOW_CLOSE";
pub const TOKEN_ATTRIBUTE: &str = "TK_ATTRIBUTE";
pub const TOKEN_EQUALS: &str = "TK_EQUALS";
pub const TOKEN_VALUE: &str = "TK_VALUE";
pub const TOKEN_COMMENT: &str = "TK_COMMENT";
pub const TOKEN_TEXT: &str = "TK_TEXT";
pub const TOKEN_UNKNOWN: &str = "TK_UNKNOWN";

const TK_CONTENT: &str = "TK_CONTENT";

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

fn line_break() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("\\r\\n|[\\r\\n]").unwrap())
}

/// `source_text.match(/^[\t ]*/)[0]`.
fn leading_indent(source: &str) -> String {
    source
        .find(|c: char| c != '\t' && c != ' ')
        .map(|i| source[..i].to_string())
        .unwrap_or_else(|| source.to_string())
}

/// Port of `regexp_escape` (js-beautify `get_literal_regexp`).
fn literal_regexp(s: &str) -> Pattern {
    let mut escaped = String::new();
    for ch in s.chars() {
        if "[\\^$*+?.()|{}\\]/-".contains(ch) {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    Pattern::new(escaped)
}

#[derive(Debug, Clone)]
pub struct HtmlToken {
    pub type_: String,
    pub text: String,
    pub newlines: usize,
    pub whitespace_before: String,
    pub parent: Option<usize>,
    pub next: Option<usize>,
    pub previous: Option<usize>,
    pub opened: Option<usize>,
    pub closed: Option<usize>,
    pub directives: Option<Vec<(String, String)>>,
}

impl HtmlToken {
    fn new(type_: &str, text: String, newlines: usize, whitespace_before: String) -> Self {
        Self {
            type_: type_.to_string(),
            text,
            newlines,
            whitespace_before,
            parent: None,
            next: None,
            previous: None,
            opened: None,
            closed: None,
            directives: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct HtmlOptions {
    pub base: BaseOptions,
    pub indent_inner_html: bool,
    pub indent_body_inner_html: bool,
    pub indent_head_inner_html: bool,
    pub indent_handlebars: bool,
    pub wrap_attributes: String,
    pub wrap_attributes_min_attrs: i32,
    pub wrap_attributes_indent_size: i32,
    pub extra_liners: Vec<String>,
    pub inline: Vec<String>,
    pub inline_custom_elements: bool,
    pub void_elements: Vec<String>,
    pub unformatted: Vec<String>,
    pub content_unformatted: Vec<String>,
    pub unformatted_content_delimiter: String,
    pub indent_scripts: String,
}

const WRAP_ATTRIBUTES_VALUES: [&str; 7] = [
    "auto",
    "force",
    "force-aligned",
    "force-expand-multiline",
    "aligned-multiple",
    "preserve",
    "preserve-aligned",
];

const DEFAULT_INLINE: [&str; 57] = [
    "a", "abbr", "area", "audio", "b", "bdi", "bdo", "br", "button", "canvas", "cite", "code",
    "data", "datalist", "del", "dfn", "em", "embed", "i", "iframe", "img", "input", "ins", "kbd",
    "keygen", "label", "map", "mark", "math", "meter", "noscript", "object", "output", "progress",
    "q", "ruby", "s", "samp", "select", "small", "span", "strong", "sub", "sup", "svg", "template",
    "textarea", "time", "u", "var", "video", "wbr", "text", "acronym", "big", "strike", "tt",
];

const DEFAULT_VOID: [&str; 19] = [
    "area",
    "base",
    "br",
    "col",
    "embed",
    "hr",
    "img",
    "input",
    "keygen",
    "link",
    "menuitem",
    "meta",
    "param",
    "source",
    "track",
    "wbr",
    "!doctype",
    "?xml",
    "basefont",
];

impl HtmlOptions {
    pub fn new(options: &RawOptions) -> Self {
        let mut base = BaseOptions::new(options, Some("html"));
        if base.templating.len() == 1 && base.templating[0] == "auto" {
            base.templating = ["django", "erb", "handlebars", "php"]
                .iter()
                .map(|s| s.to_string())
                .collect();
        }
        let raw = base.raw_options.clone();

        let mut void_elements = get_string_array(&raw, "void_elements", &DEFAULT_VOID);
        void_elements.push("isindex".to_string());

        Self {
            indent_inner_html: get_boolean(&raw, "indent_inner_html", false),
            indent_body_inner_html: get_boolean(&raw, "indent_body_inner_html", true),
            indent_head_inner_html: get_boolean(&raw, "indent_head_inner_html", true),
            indent_handlebars: get_boolean(&raw, "indent_handlebars", true),
            wrap_attributes: get_selection(
                &raw,
                "wrap_attributes",
                &WRAP_ATTRIBUTES_VALUES,
                None,
            ),
            wrap_attributes_min_attrs: get_number(&raw, "wrap_attributes_min_attrs", Some(2)),
            wrap_attributes_indent_size: get_number(
                &raw,
                "wrap_attributes_indent_size",
                Some(base.indent_size),
            ),
            extra_liners: get_string_array(&raw, "extra_liners", &["head", "body", "/html"]),
            inline: get_string_array(&raw, "inline", &DEFAULT_INLINE),
            inline_custom_elements: get_boolean(&raw, "inline_custom_elements", true),
            void_elements,
            unformatted: get_string_array(&raw, "unformatted", &[]),
            content_unformatted: get_string_array(&raw, "content_unformatted", &[
                "pre",
                "textarea",
            ]),
            unformatted_content_delimiter: get_characters(
                &raw,
                "unformatted_content_delimiter",
                "",
            ),
            indent_scripts: get_selection(
                &raw,
                "indent_scripts",
                &["normal", "keep", "separate"],
                None,
            ),
            base,
        }
    }
}

// ---------------------------------------------------------------------------
// Printer
// ---------------------------------------------------------------------------

struct Printer {
    indent_level: i32,
    alignment_size: i32,
    max_preserve_newlines: i32,
    preserve_newlines: bool,
    output: Output,
}

impl Printer {
    fn new(options: &HtmlOptions, base_indent_string: &str) -> Self {
        Self {
            indent_level: 0,
            alignment_size: 0,
            max_preserve_newlines: options.base.max_preserve_newlines,
            preserve_newlines: options.base.preserve_newlines,
            output: Output::new(&options.base, base_indent_string),
        }
    }

    fn set_space_before_token(&mut self, value: bool, non_breaking: bool) {
        self.output.space_before_token = value;
        self.output.non_breaking_space = non_breaking;
    }

    fn set_wrap_point(&mut self) {
        self.output
            .set_indent(Some(self.indent_level as i64), Some(self.alignment_size as i64));
        self.output.set_wrap_point();
    }

    fn print_newline(&mut self, force: bool) {
        self.output.add_new_line(force);
    }

    fn previous_token_wrapped(&self) -> bool {
        self.output.previous_token_wrapped
    }

    fn indent(&mut self) {
        self.indent_level += 1;
    }

    fn deindent(&mut self) {
        if self.indent_level > 0 {
            self.indent_level -= 1;
            self.output
                .set_indent(Some(self.indent_level as i64), Some(self.alignment_size as i64));
        }
    }

    fn get_full_indent(&mut self, level: i32) -> String {
        let level = self.indent_level + level;
        if level < 1 {
            return String::new();
        }
        self.output.get_indent_string(level as i64, 0)
    }
}

// ---------------------------------------------------------------------------
// Patterns / tokenizer
// ---------------------------------------------------------------------------

fn directives_core() -> &'static Directives {
    static D: OnceLock<Directives> = OnceLock::new();
    D.get_or_init(|| Directives::new("<\\!--", "-->"))
}

struct HtmlPatterns {
    word: TemplatablePattern,
    word_control_flow_close_excluded: TemplatablePattern,
    single_quote: TemplatablePattern,
    double_quote: TemplatablePattern,
    attribute: TemplatablePattern,
    element_name: TemplatablePattern,
    angular_control_flow_start: JsPattern,
    handlebars_comment: JsPattern,
    handlebars: JsPattern,
    handlebars_open: JsPattern,
    handlebars_raw_close: JsPattern,
    comment: JsPattern,
    cdata: JsPattern,
    conditional_comment: JsPattern,
    processing: JsPattern,
    unformatted_content_delimiter: Option<JsPattern>,
}

impl HtmlPatterns {
    fn new(options: &HtmlOptions) -> Self {
        let reader = TemplatablePattern::new().read_options(&options.base.templating);
        let pr = JsPattern::default();

        let mut word = reader.clone().until(Pattern::new("[\\n\\r\\t <]"));
        let mut word_control_flow_close_excluded =
            reader.clone().until(Pattern::new("[\\n\\r\\t <}]"));
        if options.indent_handlebars {
            word = word.exclude("handlebars");
            word_control_flow_close_excluded =
                word_control_flow_close_excluded.exclude("handlebars");
        }

        let unformatted_content_delimiter = if options.unformatted_content_delimiter.is_empty() {
            None
        } else {
            let literal = literal_regexp(&options.unformatted_content_delimiter);
            Some(pr.matching(literal.clone()).until_after(literal))
        };

        Self {
            word,
            word_control_flow_close_excluded,
            single_quote: reader.clone().until_after(Pattern::new("'")),
            double_quote: reader.clone().until_after(Pattern::new("\"")),
            attribute: reader.clone().until(Pattern::new("[\\n\\r\\t =>]|/>")),
            element_name: reader.clone().until(Pattern::new("[\\n\\r\\t >/]")),
            angular_control_flow_start: JsPattern::default()
                .matching(Pattern::new("@[a-zA-Z]+[^({]*[({]")),
            handlebars_comment: JsPattern::starting_with(Pattern::new("\\{\\{!--"))
                .until_after(Pattern::new("--\\}\\}")),
            handlebars: JsPattern::starting_with(Pattern::new("\\{\\{"))
                .until_after(Pattern::new("\\}\\}")),
            handlebars_open: JsPattern::default().until(Pattern::new("[\\n\\r\\t }]")),
            handlebars_raw_close: JsPattern::default().until(Pattern::new("\\}\\}")),
            comment: JsPattern::starting_with(Pattern::new("<!--"))
                .until_after(Pattern::new("-->")),
            cdata: JsPattern::starting_with(Pattern::new("<!\\[CDATA\\["))
                .until_after(Pattern::new("\\]\\]>")),
            conditional_comment: JsPattern::starting_with(Pattern::new("<!\\["))
                .until_after(Pattern::new("\\]>")),
            processing: JsPattern::starting_with(Pattern::new("<\\?"))
                .until_after(Pattern::new("\\?>")),
            unformatted_content_delimiter,
        }
    }
}

struct HtmlTokenizer {
    input: InputScanner,
    options: HtmlOptions,
    patterns: HtmlPatterns,
    whitespace: super::scanner::WhitespacePattern,
    current_tag_name: String,
    tokens: Vec<HtmlToken>,
}

impl HtmlTokenizer {
    fn new(source_text: &str, options: &HtmlOptions) -> Self {
        Self {
            input: InputScanner::new(source_text),
            patterns: HtmlPatterns::new(options),
            whitespace: super::scanner::WhitespacePattern::new(),
            current_tag_name: String::new(),
            tokens: Vec::new(),
            options: options.clone(),
        }
    }

    fn create_token(&self, type_: &str, text: String) -> HtmlToken {
        HtmlToken::new(
            type_,
            text,
            self.whitespace.newline_count,
            self.whitespace.whitespace_before_token.clone(),
        )
    }

    fn read_whitespace(&mut self) -> String {
        self.whitespace.read(&mut self.input)
    }

    fn token_type(&self, index: Option<usize>) -> &str {
        match index {
            Some(i) => &self.tokens[i].type_,
            None => TOKEN_START,
        }
    }

    fn reset(&mut self) {
        self.current_tag_name = String::new();
    }

    fn is_opening(&self, idx: usize) -> bool {
        let t = &self.tokens[idx].type_;
        t == TOKEN_TAG_OPEN || t == TOKEN_CONTROL_FLOW_OPEN
    }

    fn is_closing(&self, idx: usize, open: usize) -> bool {
        let cur = &self.tokens[idx];
        let op = &self.tokens[open];
        (cur.type_ == TOKEN_TAG_CLOSE
            && (((cur.text == ">" || cur.text == "/>") && op.text.starts_with('<'))
                || (cur.text == "}}" && op.text.starts_with("{{"))))
            || (cur.type_ == TOKEN_CONTROL_FLOW_CLOSE
                && cur.text == "}"
                && op.text.ends_with('{'))
    }

    fn tokenize(&mut self) -> Vec<HtmlToken> {
        self.input.restart();
        self.tokens.clear();
        self.reset();

        let mut previous: Option<usize> = None;
        let mut open_token: Option<usize> = None;
        let mut open_stack: Vec<Option<usize>> = Vec::new();

        loop {
            let prev_type = self.token_type(previous).to_string();
            if prev_type == TOKEN_EOF {
                break;
            }
            let token = self.get_next_token(previous, open_token);
            let idx = self.tokens.len();
            self.tokens.push(token);
            self.tokens[idx].parent = open_token;

            if self.is_opening(idx) {
                open_stack.push(open_token);
                open_token = Some(idx);
            } else if let Some(ot) = open_token
                && self.is_closing(idx, ot)
            {
                self.tokens[idx].opened = Some(ot);
                self.tokens[ot].closed = Some(idx);
                open_token = open_stack.pop().flatten();
                self.tokens[idx].parent = open_token;
            }

            self.tokens[idx].previous = previous;
            if let Some(p) = previous {
                self.tokens[p].next = Some(idx);
            }
            previous = Some(idx);
        }

        std::mem::take(&mut self.tokens)
    }

    fn get_next_token(&mut self, previous: Option<usize>, open_token: Option<usize>) -> HtmlToken {
        self.read_whitespace();
        let c = self.input.peek(0);
        if c.is_none() {
            return self.create_token(TOKEN_EOF, String::new());
        }
        let c = c.unwrap();

        let mut token = self.read_open_handlebars(c, open_token);
        if token.is_none() {
            token = self.read_attribute(c, previous, open_token);
        }
        if token.is_none() {
            token = self.read_close(c, open_token);
        }
        if token.is_none() {
            token = self.read_script_and_style(c, previous);
        }
        if token.is_none() {
            token = self.read_control_flows(c, open_token);
        }
        if token.is_none() {
            token = self.read_raw_content(c, previous, open_token);
        }
        if token.is_none() {
            token = self.read_content_word(c, open_token);
        }
        if token.is_none() {
            token = self.read_comment_or_cdata(c);
        }
        if token.is_none() {
            token = self.read_processing(c);
        }
        if token.is_none() {
            token = self.read_open(c, open_token);
        }
        token.unwrap_or_else(|| {
            let text = self.input.next_char().map(String::from).unwrap_or_default();
            self.create_token(TOKEN_UNKNOWN, text)
        })
    }

    fn read_open_handlebars(&mut self, c: char, open_token: Option<usize>) -> Option<HtmlToken> {
        let can_open = open_token.is_none()
            || open_token.is_some_and(|o| self.tokens[o].type_ == TOKEN_CONTROL_FLOW_OPEN);
        if !can_open {
            return None;
        }
        let angular = self.options.base.templating.iter().any(|t| t == "angular");
        if (angular || self.options.indent_handlebars) && c == '{' && self.input.peek(1) == Some('{')
        {
            if self.options.indent_handlebars && self.input.peek(2) == Some('!') {
                let mut s = self.patterns.handlebars_comment.read(&mut self.input);
                if s.is_empty() {
                    s = self.patterns.handlebars.read(&mut self.input);
                }
                Some(self.create_token(TOKEN_COMMENT, s))
            } else {
                let s = self.patterns.handlebars_open.read(&mut self.input);
                Some(self.create_token(TOKEN_TAG_OPEN, s))
            }
        } else {
            None
        }
    }

    fn read_attribute(
        &mut self,
        c: char,
        previous: Option<usize>,
        open_token: Option<usize>,
    ) -> Option<HtmlToken> {
        let open = open_token?;
        if !self.tokens[open].text.starts_with('<') {
            return None;
        }
        if c == '=' {
            let text = self.input.next_char().map(String::from).unwrap_or_default();
            return Some(self.create_token(TOKEN_EQUALS, text));
        }
        if c == '"' || c == '\'' {
            let mut content = self.input.next_char().map(String::from).unwrap_or_default();
            if c == '"' {
                content += &self.patterns.double_quote.read(&mut self.input);
            } else {
                content += &self.patterns.single_quote.read(&mut self.input);
            }
            return Some(self.create_token(TOKEN_VALUE, content));
        }
        let resulting = self.patterns.attribute.read(&mut self.input);
        if resulting.is_empty() {
            return None;
        }
        if self.token_type(previous) == TOKEN_EQUALS {
            Some(self.create_token(TOKEN_VALUE, resulting))
        } else {
            Some(self.create_token(TOKEN_ATTRIBUTE, resulting))
        }
    }

    fn read_close(&mut self, c: char, open_token: Option<usize>) -> Option<HtmlToken> {
        let open = open_token?;
        if self.tokens[open].type_ != TOKEN_TAG_OPEN {
            return None;
        }
        if self.tokens[open].text.starts_with('<')
            && (c == '>' || (c == '/' && self.input.peek(1) == Some('>')))
        {
            let mut s = self.input.next_char().map(String::from).unwrap_or_default();
            if c == '/' {
                s += &self.input.next_char().map(String::from).unwrap_or_default();
            }
            return Some(self.create_token(TOKEN_TAG_CLOSE, s));
        }
        if self.tokens[open].text.starts_with('{')
            && c == '}'
            && self.input.peek(1) == Some('}')
        {
            self.input.next_char();
            self.input.next_char();
            return Some(self.create_token(TOKEN_TAG_CLOSE, "}}".to_string()));
        }
        None
    }

    fn read_script_and_style(&mut self, c: char, previous: Option<usize>) -> Option<HtmlToken> {
        let prev = previous?;
        if self.tokens[prev].type_ != TOKEN_TAG_CLOSE
            || !self.tokens[prev].opened.is_some_and(|o| self.tokens[o].text.starts_with('<'))
            || self.tokens[prev].text.starts_with('/')
        {
            return None;
        }
        let opened = self.tokens[prev].opened.unwrap();
        let tag_name = self.tokens[opened].text[1..].to_lowercase();
        if tag_name != "script" && tag_name != "style" {
            return None;
        }
        if let Some(mut token) = self.read_comment_or_cdata(c) {
            token.type_ = TOKEN_TEXT.to_string();
            return Some(token);
        }
        let re = Regex::new(&format!("(?is)</{tag_name}[\\n\\r\\t ]*?>", tag_name = regex::escape(&tag_name))).unwrap();
        let resulting = self.input.read_until(&Pattern::new(re.as_str()), false);
        if !resulting.is_empty() {
            return Some(self.create_token(TOKEN_TEXT, resulting));
        }
        None
    }

    fn read_raw_content(
        &mut self,
        _c: char,
        previous: Option<usize>,
        open_token: Option<usize>,
    ) -> Option<HtmlToken> {
        let mut resulting = String::new();
        if open_token.is_some_and(|o| self.tokens[o].text.starts_with('{')) {
            resulting = self.patterns.handlebars_raw_close.read(&mut self.input);
        } else if let Some(prev) = previous
            && self.tokens[prev].type_ == TOKEN_TAG_CLOSE
            && self.tokens[prev]
                .opened
                .is_some_and(|o| self.tokens[o].text.starts_with('<'))
            && !self.tokens[prev].text.starts_with('/')
        {
            let opened = self.tokens[prev].opened.unwrap();
            let tag_name = self.tokens[opened].text[1..].to_lowercase();
            if self.is_content_unformatted(&tag_name) {
                let re = Regex::new(&format!(
                    "(?is)</{}[\\n\\r\\t ]*?>",
                    regex::escape(&tag_name)
                ))
                .unwrap();
                resulting = self.input.read_until(&Pattern::new(re.as_str()), false);
            }
        }
        if !resulting.is_empty() {
            return Some(self.create_token(TOKEN_TEXT, resulting));
        }
        None
    }

    fn is_content_unformatted(&self, tag_name: &str) -> bool {
        !self.options.void_elements.iter().any(|v| v == tag_name)
            && (self.options.content_unformatted.iter().any(|v| v == tag_name)
                || self.options.unformatted.iter().any(|v| v == tag_name))
    }

    fn read_content_word(&mut self, c: char, open_token: Option<usize>) -> Option<HtmlToken> {
        let mut resulting = String::new();
        if !self.options.unformatted_content_delimiter.is_empty()
            && c == self.options.unformatted_content_delimiter.chars().next().unwrap()
                && let Some(p) = &self.patterns.unformatted_content_delimiter {
                    resulting = p.read(&mut self.input);
                }
        if resulting.is_empty() {
            if open_token.is_some_and(|o| self.tokens[o].type_ == TOKEN_CONTROL_FLOW_OPEN) {
                resulting = self
                    .patterns
                    .word_control_flow_close_excluded
                    .read(&mut self.input);
            } else {
                resulting = self.patterns.word.read(&mut self.input);
            }
        }
        if !resulting.is_empty() {
            return Some(self.create_token(TOKEN_TEXT, resulting));
        }
        None
    }

    fn read_comment_or_cdata(&mut self, c: char) -> Option<HtmlToken> {
        if c != '<' {
            return None;
        }
        let peek1 = self.input.peek(1);
        if peek1 != Some('!') {
            return None;
        }
        let mut resulting = self.patterns.comment.read(&mut self.input);
        let mut directives = None;
        if !resulting.is_empty() {
            directives = directives_core().get_directives(&resulting);
            if ignores_start(&directives) {
                resulting += &directives_core().read_ignored(&mut self.input);
            }
        } else {
            resulting = self.patterns.cdata.read(&mut self.input);
        }
        if resulting.is_empty() {
            return None;
        }
        let mut token = self.create_token(TOKEN_COMMENT, resulting);
        token.directives = directives;
        Some(token)
    }

    fn read_processing(&mut self, c: char) -> Option<HtmlToken> {
        if c != '<' {
            return None;
        }
        let peek1 = self.input.peek(1);
        if peek1 != Some('!') && peek1 != Some('?') {
            return None;
        }
        let mut resulting = self.patterns.conditional_comment.read(&mut self.input);
        if resulting.is_empty() {
            resulting = self.patterns.processing.read(&mut self.input);
        }
        if resulting.is_empty() {
            return None;
        }
        Some(self.create_token(TOKEN_COMMENT, resulting))
    }

    fn read_open(&mut self, c: char, open_token: Option<usize>) -> Option<HtmlToken> {
        let can_open = open_token.is_none()
            || open_token.is_some_and(|o| self.tokens[o].type_ == TOKEN_CONTROL_FLOW_OPEN);
        if !can_open || c != '<' {
            return None;
        }
        let mut resulting = self.input.next_char().map(String::from).unwrap_or_default();
        if self.input.peek(0) == Some('/') {
            resulting += &self.input.next_char().map(String::from).unwrap_or_default();
        }
        resulting += &self.patterns.element_name.read(&mut self.input);
        Some(self.create_token(TOKEN_TAG_OPEN, resulting))
    }

    fn read_control_flows(&mut self, c: char, open_token: Option<usize>) -> Option<HtmlToken> {
        if !self.options.base.templating.iter().any(|t| t == "angular") {
            return None;
        }
        if c == '@' {
            let mut resulting = self.patterns.angular_control_flow_start.read(&mut self.input);
            if resulting.is_empty() {
                return None;
            }
            let mut opening = if resulting.ends_with('(') { 1 } else { 0 };
            let mut closing = 0;
            while !(resulting.ends_with('{') && opening == closing) {
                let Some(next) = self.input.next_char() else {
                    break;
                };
                if next == '(' {
                    opening += 1;
                } else if next == ')' {
                    closing += 1;
                }
                resulting.push(next);
            }
            return Some(self.create_token(TOKEN_CONTROL_FLOW_OPEN, resulting));
        }
        if c == '}'
            && open_token.is_some_and(|o| self.tokens[o].type_ == TOKEN_CONTROL_FLOW_OPEN)
        {
            let resulting = self.input.next_char().map(String::from).unwrap_or_default();
            return Some(self.create_token(TOKEN_CONTROL_FLOW_CLOSE, resulting));
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Parser tokens / tag stack
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct ParserToken {
    parent: Option<usize>,
    text: String,
    tag_name: String,
    is_inline_element: bool,
    is_unformatted: bool,
    is_content_unformatted: bool,
    is_empty_element: bool,
    is_start_tag: bool,
    is_end_tag: bool,
    indent_content: bool,
    multiline_content: bool,
    custom_beautifier_name: Option<String>,
    start_tag_token: Option<usize>,
    attr_count: i32,
    has_wrapped_attrs: bool,
    alignment_size: i32,
    tag_complete: bool,
    tag_start_char: String,
    tag_check: String,
}

impl ParserToken {
    fn empty() -> Self {
        Self {
            parent: None,
            text: String::new(),
            tag_name: String::new(),
            is_inline_element: false,
            is_unformatted: false,
            is_content_unformatted: false,
            is_empty_element: false,
            is_start_tag: false,
            is_end_tag: false,
            indent_content: false,
            multiline_content: false,
            custom_beautifier_name: None,
            start_tag_token: None,
            attr_count: 0,
            has_wrapped_attrs: false,
            alignment_size: 0,
            tag_complete: true,
            tag_start_char: String::new(),
            tag_check: String::new(),
        }
    }

    fn from_raw(
        options: &HtmlOptions,
        parent: Option<usize>,
        raw: &HtmlToken,
        tokens: &[HtmlToken],
    ) -> Self {
        let mut token = Self::empty();
        token.tag_complete = false;
        token.parent = parent;
        token.text = raw.text.clone();
        token.tag_start_char = raw.text.chars().next().map(String::from).unwrap_or_default();

        if token.tag_start_char == "<" {
            let re = regex_tag_check();
            token.tag_check = re
                .captures(&raw.text)
                .map(|c| c[1].to_string())
                .unwrap_or_default();
        } else {
            let re = regex_handlebar_check();
            token.tag_check = re
                .captures(&raw.text)
                .map(|c| c[1].to_string())
                .unwrap_or_default();
            if (raw.text.starts_with("{{#>") || raw.text.starts_with("{{~#>"))
                && token.tag_check.starts_with('>')
            {
                if token.tag_check == ">" {
                    if let Some(next) = raw.next {
                        token.tag_check = tokens[next]
                            .text
                            .split(' ')
                            .next()
                            .unwrap_or_default()
                            .to_string();
                    }
                } else {
                    token.tag_check = raw
                        .text
                        .split('>')
                        .nth(1)
                        .unwrap_or_default()
                        .to_string();
                }
            }
        }

        token.tag_check = token.tag_check.to_lowercase();

        if raw.type_ == TOKEN_COMMENT {
            token.tag_complete = true;
        }

        token.is_start_tag = !token.tag_check.starts_with('/');
        token.tag_name = if !token.is_start_tag {
            token.tag_check.chars().skip(1).collect()
        } else {
            token.tag_check.clone()
        };
        token.is_end_tag = !token.is_start_tag
            || raw
                .closed
                .is_some_and(|c| tokens[c].text == "/>");

        let mut handlebar_starts = 2usize;
        if token.tag_start_char == "{" && token.text.chars().count() >= 3
            && token.text.chars().nth(2) == Some('~') {
                handlebar_starts = 3;
            }

        let char_count = token.text.chars().count();
        token.is_end_tag = token.is_end_tag
            || (token.tag_start_char == "{"
                && (!options.indent_handlebars
                    || char_count < 3
                    || token
                        .text
                        .chars()
                        .nth(handlebar_starts)
                        .is_some_and(|ch| ch != '#' && ch != '^')));

        token
    }
}

struct TagFrame {
    tag: String,
    indent_level: i32,
    parser_token: usize,
}

#[derive(Default)]
struct TagStack {
    frames: Vec<TagFrame>,
}

impl TagStack {
    fn new() -> Self {
        Self { frames: Vec::new() }
    }

    fn get_parser_token(&self) -> Option<usize> {
        self.frames.last().map(|f| f.parser_token)
    }

    fn record_tag(&mut self, tag: String, parser_token: usize, indent_level: i32) {
        self.frames.push(TagFrame {
            tag,
            indent_level,
            parser_token,
        });
    }

    fn get_frame(&self, tag_list: &[&str], stop_list: Option<&[&str]>) -> Option<usize> {
        let mut i = self.frames.len();
        while i > 0 {
            i -= 1;
            let tag = &self.frames[i].tag;
            if tag_list.iter().any(|t| t == tag) {
                return Some(i);
            }
            if stop_list.is_some_and(|s| s.iter().any(|t| t == tag)) {
                return None;
            }
        }
        None
    }

    fn try_pop(
        &mut self,
        tag: &str,
        stop_list: Option<&[&str]>,
        printer: &mut Printer,
    ) -> Option<usize> {
        let i = self.get_frame(&[tag], stop_list)?;
        let frame = self.frames.split_off(i);
        let first = frame.into_iter().next().unwrap();
        printer.indent_level = first.indent_level;
        Some(first.parser_token)
    }

    fn indent_to_tag(&mut self, tag_list: &[&str], printer: &mut Printer) {
        if let Some(i) = self.get_frame(tag_list, None) {
            printer.indent_level = self.frames[i].indent_level;
        }
    }
}

// ---------------------------------------------------------------------------
// Beautifier
// ---------------------------------------------------------------------------

const P_CLOSERS: [&str; 29] = [
    "address",
    "article",
    "aside",
    "blockquote",
    "details",
    "div",
    "dl",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hr",
    "main",
    "menu",
    "nav",
    "ol",
    "p",
    "pre",
    "section",
    "table",
    "ul",
];

const P_PARENT_EXCLUDES: [&str; 7] = ["a", "audio", "del", "ins", "map", "noscript", "video"];

fn regex_tag_check() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("^<([^\\s>]*)").unwrap())
}

fn regex_handlebar_check() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("^\\{\\{~?(?:\\^|#\\*?)?([^\\s}]+)").unwrap())
}

fn regex_handlebar_if() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("\\{\\{#if").unwrap())
}


fn regex_js_type() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new("module|importmap|((text|application|dojo)/(x-)?(javascript|ecmascript|jscript|livescript|(ld\\+)?json|method|aspect))").unwrap()
    })
}

fn regex_html_type() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("(text|application|dojo)/(x-)?(html)").unwrap())
}

fn regex_null_type() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("test/null").unwrap())
}

fn regex_comment_or_cdata() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("^(<!--|<!\\[CDATA\\[)").unwrap())
}

fn regex_cdata_block() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new("^(<!--[^\\n]*|<!\\[CDATA\\[)(\\n?)([ \\t\\n]*)([\\s\\S]*)(-->|]]>)$").unwrap()
    })
}

fn regex_trailing_ws_line() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("\\n[ \\t]*$").unwrap())
}

fn regex_trailing_spaces() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("[ \\t]+$").unwrap())
}

pub struct HtmlBeautifier {
    source_text: String,
    options: HtmlOptions,
    is_wrap_attributes_force: bool,
    is_wrap_attributes_force_expand_multiline: bool,
    is_wrap_attributes_force_aligned: bool,
    is_wrap_attributes_aligned_multiple: bool,
    is_wrap_attributes_preserve: bool,
    is_wrap_attributes_preserve_aligned: bool,
    printer: Printer,
    tokens: Vec<HtmlToken>,
    position: usize,
    tag_stack: TagStack,
    parser_tokens: Vec<ParserToken>,
}

impl HtmlBeautifier {
    pub fn new(source_text: &str, options: &RawOptions) -> Self {
        let options = HtmlOptions::new(options);
        let wrap = options.wrap_attributes.clone();
        let printer = Printer::new(&options, "");
        Self {
            source_text: source_text.to_string(),
            is_wrap_attributes_force: wrap.starts_with("force"),
            is_wrap_attributes_force_expand_multiline: wrap == "force-expand-multiline",
            is_wrap_attributes_force_aligned: wrap == "force-aligned",
            is_wrap_attributes_aligned_multiple: wrap == "aligned-multiple",
            is_wrap_attributes_preserve: wrap.starts_with("preserve"),
            is_wrap_attributes_preserve_aligned: wrap == "preserve-aligned",
            printer,
            tokens: Vec::new(),
            position: 0,
            tag_stack: TagStack::new(),
            parser_tokens: Vec::new(),
            options,
        }
    }

    fn next_token(&mut self) -> Option<usize> {
        if self.position < self.tokens.len() {
            let i = self.position;
            self.position += 1;
            Some(i)
        } else {
            None
        }
    }

    fn peek_token(&self, index: usize) -> Option<usize> {
        let i = self.position + index;
        if i < self.tokens.len() { Some(i) } else { None }
    }

    fn prev_type_of(&self, raw: usize) -> String {
        match self.tokens[raw].previous {
            Some(p) => self.tokens[p].type_.clone(),
            None => TOKEN_START.to_string(),
        }
    }

    fn child_options(&self) -> RawOptions {
        let mut raw = self.options.base.raw_options.clone();
        raw.insert("eol".to_string(), OptValue::Str("\n".to_string()));
        raw
    }

    fn add_raw_token(&mut self, raw: usize) {
        let newlines = self.tokens[raw].newlines;
        let whitespace = self.tokens[raw].whitespace_before.clone();
        let text = self.tokens[raw].text.clone();
        self.printer
            .output
            .add_raw_token(newlines, &whitespace, &text);
    }

    fn print_token(&mut self, raw: usize) {
        let text = self.tokens[raw].text.clone();
        if !text.is_empty() {
            self.printer.output.set_indent(
                Some(self.printer.indent_level as i64),
                Some(self.printer.alignment_size as i64),
            );
            self.printer.output.add_token(&text);
        }
    }

    fn print_preserved_newlines(&mut self, raw: usize, prev_type: &str) -> bool {
        let token = &self.tokens[raw];
        let mut newlines = if token.type_ != TOKEN_TEXT && prev_type != TOKEN_TEXT {
            if token.newlines > 0 { 1 } else { 0 }
        } else {
            0
        };
        if self.printer.preserve_newlines {
            let max = self.printer.max_preserve_newlines;
            newlines = if (token.newlines as i32) < max + 1 {
                token.newlines as i32
            } else {
                max + 1
            };
        }
        for n in 0..newlines {
            self.printer.print_newline(n > 0);
        }
        newlines != 0
    }

    fn traverse_whitespace(&mut self, raw: usize) -> bool {
        if self.tokens[raw].whitespace_before.is_empty() && self.tokens[raw].newlines == 0 {
            return false;
        }
        let prev_type = self.prev_type_of(raw);
        if !self.print_preserved_newlines(raw, &prev_type) {
            self.printer.output.space_before_token = true;
        }
        true
    }

    fn get_type_attribute(&self, start: usize) -> Option<String> {
        let mut raw = self.tokens[start].next;
        while let Some(r) = raw {
            if self.tokens[r].type_ == TOKEN_EOF || self.tokens[start].closed == Some(r) {
                break;
            }
            if self.tokens[r].type_ == TOKEN_ATTRIBUTE && self.tokens[r].text == "type" {
                if let Some(n1) = self.tokens[r].next
                    && self.tokens[n1].type_ == TOKEN_EQUALS
                    && let Some(n2) = self.tokens[n1].next
                    && self.tokens[n2].type_ == TOKEN_VALUE
                {
                    return Some(self.tokens[n2].text.clone());
                }
                break;
            }
            raw = self.tokens[r].next;
        }
        None
    }

    fn get_custom_beautifier_name(&self, tag_check: &str, raw: usize) -> Option<String> {
        self.tokens[raw].closed?;
        let mut type_attribute = if tag_check == "script" {
            Some("text/javascript".to_string())
        } else if tag_check == "style" {
            Some("text/css".to_string())
        } else {
            None
        };
        type_attribute = self.get_type_attribute(raw).or(type_attribute);
        let type_attribute = type_attribute.unwrap_or_default();

        if type_attribute.contains("text/css") {
            Some("css".to_string())
        } else if regex_js_type().is_match(&type_attribute) {
            Some("javascript".to_string())
        } else if regex_html_type().is_match(&type_attribute) {
            Some("html".to_string())
        } else if regex_null_type().is_match(&type_attribute) {
            Some("null".to_string())
        } else {
            None
        }
    }

    fn get_tag_open_token(&mut self, raw: usize) -> usize {
        let parent = self.tag_stack.get_parser_token();
        let token = ParserToken::from_raw(
            &self.options,
            parent,
            &self.tokens[raw],
            &self.tokens,
        );
        let idx = self.parser_tokens.len();
        self.parser_tokens.push(token);

        self.parser_tokens[idx].alignment_size = self.options.wrap_attributes_indent_size;

        let tag_check = self.parser_tokens[idx].tag_check.clone();
        if self.options.void_elements.iter().any(|v| v == &tag_check) {
            self.parser_tokens[idx].is_end_tag = true;
        }

        let is_start = self.parser_tokens[idx].is_start_tag;
        let is_end = self.parser_tokens[idx].is_end_tag;
        let complete = self.parser_tokens[idx].tag_complete;
        self.parser_tokens[idx].is_empty_element = complete || (is_start && is_end);

        self.parser_tokens[idx].is_unformatted =
            !complete && self.options.unformatted.iter().any(|v| v == &tag_check);
        let is_empty = self.parser_tokens[idx].is_empty_element;
        self.parser_tokens[idx].is_content_unformatted = !is_empty
            && self
                .options
                .content_unformatted
                .iter()
                .any(|v| v == &tag_check);

        let tag_name = self.parser_tokens[idx].tag_name.clone();
        let inline_custom = self.options.inline_custom_elements && tag_name.contains('-');
        self.parser_tokens[idx].is_inline_element =
            self.options.inline.iter().any(|v| v == &tag_name)
                || inline_custom
                || self.parser_tokens[idx].tag_start_char == "{";

        idx
    }

    fn do_optional_end_element(&mut self, pt: usize) -> bool {
        if self.parser_tokens[pt].is_empty_element
            || !self.parser_tokens[pt].is_start_tag
            || self.parser_tokens[pt].parent.is_none()
        {
            return false;
        }

        let name = self.parser_tokens[pt].tag_name.clone();
        let mut result = false;

        let parent_is_p = self.parser_tokens[pt]
            .parent
            .is_some_and(|p| self.parser_tokens[p].tag_name == "p");

        if name == "body" {
            result = result || self.tag_stack.try_pop("head", None, &mut self.printer).is_some();
        } else if name == "li" {
            result = result
                || self
                    .tag_stack
                    .try_pop("li", Some(&["ol", "ul", "menu"]), &mut self.printer)
                    .is_some();
        } else if name == "dd" || name == "dt" {
            result = result
                || self
                    .tag_stack
                    .try_pop("dt", Some(&["dl"]), &mut self.printer)
                    .is_some();
            result = result
                || self
                    .tag_stack
                    .try_pop("dd", Some(&["dl"]), &mut self.printer)
                    .is_some();
        } else if parent_is_p && P_CLOSERS.contains(&name.as_str()) {
            let p_parent = self.parser_tokens[pt]
                .parent
                .and_then(|p| self.parser_tokens[p].parent);
            if p_parent.is_none()
                || !P_PARENT_EXCLUDES.contains(&self.parser_tokens[p_parent.unwrap()].tag_name.as_str())
            {
                result = result
                    || self.tag_stack.try_pop("p", None, &mut self.printer).is_some();
            }
        } else if name == "rp" || name == "rt" {
            result = result
                || self
                    .tag_stack
                    .try_pop("rt", Some(&["ruby", "rtc"]), &mut self.printer)
                    .is_some();
            result = result
                || self
                    .tag_stack
                    .try_pop("rp", Some(&["ruby", "rtc"]), &mut self.printer)
                    .is_some();
        } else if name == "optgroup" {
            result = result
                || self
                    .tag_stack
                    .try_pop("optgroup", Some(&["select"]), &mut self.printer)
                    .is_some();
        } else if name == "option" {
            result = result
                || self
                    .tag_stack
                    .try_pop("option", Some(&["select", "datalist", "optgroup"]), &mut self.printer)
                    .is_some();
        } else if name == "colgroup" {
            result = result
                || self
                    .tag_stack
                    .try_pop("caption", Some(&["table"]), &mut self.printer)
                    .is_some();
        } else if name == "thead" {
            result = result
                || self
                    .tag_stack
                    .try_pop("caption", Some(&["table"]), &mut self.printer)
                    .is_some();
            result = result
                || self
                    .tag_stack
                    .try_pop("colgroup", Some(&["table"]), &mut self.printer)
                    .is_some();
        } else if name == "tbody" || name == "tfoot" {
            result = result
                || self
                    .tag_stack
                    .try_pop("caption", Some(&["table"]), &mut self.printer)
                    .is_some();
            result = result
                || self
                    .tag_stack
                    .try_pop("colgroup", Some(&["table"]), &mut self.printer)
                    .is_some();
            result = result
                || self
                    .tag_stack
                    .try_pop("thead", Some(&["table"]), &mut self.printer)
                    .is_some();
            result = result
                || self
                    .tag_stack
                    .try_pop("tbody", Some(&["table"]), &mut self.printer)
                    .is_some();
        } else if name == "tr" {
            result = result
                || self
                    .tag_stack
                    .try_pop("caption", Some(&["table"]), &mut self.printer)
                    .is_some();
            result = result
                || self
                    .tag_stack
                    .try_pop("colgroup", Some(&["table"]), &mut self.printer)
                    .is_some();
            result = result
                || self
                    .tag_stack
                    .try_pop("tr", Some(&["table", "thead", "tbody", "tfoot"]), &mut self.printer)
                    .is_some();
        } else if name == "th" || name == "td" {
            result = result
                || self
                    .tag_stack
                    .try_pop(
                        "td",
                        Some(&["table", "thead", "tbody", "tfoot", "tr"]),
                        &mut self.printer,
                    )
                    .is_some();
            result = result
                || self
                    .tag_stack
                    .try_pop(
                        "th",
                        Some(&["table", "thead", "tbody", "tfoot", "tr"]),
                        &mut self.printer,
                    )
                    .is_some();
        }

        self.parser_tokens[pt].parent = self.tag_stack.get_parser_token();
        result
    }

    fn calculate_parent_multiline(&mut self, pt: usize) {
        if let Some(parent) = self.parser_tokens[pt].parent
            && self.printer.output.just_added_newline() {
                let child_inline = self.parser_tokens[pt].is_inline_element
                    || self.parser_tokens[pt].is_unformatted;
                if !(child_inline && self.parser_tokens[parent].is_inline_element) {
                    self.parser_tokens[parent].multiline_content = true;
                }
            }
    }

    fn set_tag_position(
        &mut self,
        raw: usize,
        parser_token: usize,
        last_tag_token: usize,
        last_token_type: &str,
    ) {
        if !self.parser_tokens[parser_token].is_empty_element {
            if self.parser_tokens[parser_token].is_end_tag {
                let tag = self.parser_tokens[parser_token].tag_name.clone();
                let popped = self.tag_stack.try_pop(&tag, None, &mut self.printer);
                self.parser_tokens[parser_token].start_tag_token = popped;
            } else {
                if self.do_optional_end_element(parser_token)
                    && !self.parser_tokens[parser_token].is_inline_element
                {
                    self.printer.print_newline(false);
                }

                let tag = self.parser_tokens[parser_token].tag_name.clone();
                let indent = self.printer.indent_level;
                self.tag_stack.record_tag(tag, parser_token, indent);

                let name = self.parser_tokens[parser_token].tag_name.clone();
                if (name == "script" || name == "style")
                    && !(self.parser_tokens[parser_token].is_unformatted
                        || self.parser_tokens[parser_token].is_content_unformatted)
                {
                    let tag_check = self.parser_tokens[parser_token].tag_check.clone();
                    let custom = self.get_custom_beautifier_name(&tag_check, raw);
                    self.parser_tokens[parser_token].custom_beautifier_name = custom;
                }
            }
        }

        let tag_check = self.parser_tokens[parser_token].tag_check.clone();
        if self.options.extra_liners.iter().any(|v| v == &tag_check) {
            self.printer.print_newline(false);
            if !self.printer.output.just_added_blankline() {
                self.printer.print_newline(true);
            }
        }

        let is_empty = self.parser_tokens[parser_token].is_empty_element;
        let is_end = self.parser_tokens[parser_token].is_end_tag;
        let is_inline = self.parser_tokens[parser_token].is_inline_element;
        let is_unformatted = self.parser_tokens[parser_token].is_unformatted;
        let is_content_unformatted = self.parser_tokens[parser_token].is_content_unformatted;
        let tag_start_char = self.parser_tokens[parser_token].tag_start_char.clone();
        let tag_check = self.parser_tokens[parser_token].tag_check.clone();
        let tag_name = self.parser_tokens[parser_token].tag_name.clone();

        if is_empty {
            if tag_start_char == "{" && tag_check == "else" {
                self.tag_stack
                    .indent_to_tag(&["if", "unless", "each"], &mut self.printer);
                self.parser_tokens[parser_token].indent_content = true;
                let found = self.printer.output.current_line_has_match(regex_handlebar_if());
                if !found {
                    self.printer.print_newline(false);
                }
            }

            if tag_name == "!--"
                && last_token_type == TOKEN_TAG_CLOSE
                && self.parser_tokens[last_tag_token].is_end_tag
                && !self.tokens[raw].text.contains('\n')
            {
            } else {
                if !(is_inline || is_unformatted) {
                    self.printer.print_newline(false);
                }
                self.calculate_parent_multiline(parser_token);
            }
        } else if is_end {
            let mut do_end_expand = self.parser_tokens[parser_token]
                .start_tag_token
                .is_some_and(|s| self.parser_tokens[s].multiline_content);
            do_end_expand = do_end_expand
                || (!is_inline
                    && !(self.parser_tokens[last_tag_token].is_inline_element
                        || self.parser_tokens[last_tag_token].is_unformatted)
                    && !(last_token_type == TOKEN_TAG_CLOSE
                        && self.parser_tokens[parser_token].start_tag_token == Some(last_tag_token))
                    && last_token_type != TK_CONTENT);

            if is_content_unformatted || is_unformatted {
                do_end_expand = false;
            }

            if do_end_expand {
                self.printer.print_newline(false);
            }
        } else {
            self.parser_tokens[parser_token].indent_content =
                self.parser_tokens[parser_token].custom_beautifier_name.is_none();

            if tag_start_char == "<" {
                if tag_name == "html" {
                    self.parser_tokens[parser_token].indent_content = self.options.indent_inner_html;
                } else if tag_name == "head" {
                    self.parser_tokens[parser_token].indent_content =
                        self.options.indent_head_inner_html;
                } else if tag_name == "body" {
                    self.parser_tokens[parser_token].indent_content =
                        self.options.indent_body_inner_html;
                }
            }

            if !(is_inline || is_unformatted)
                && (last_token_type != TK_CONTENT || is_content_unformatted)
            {
                self.printer.print_newline(false);
            }

            self.calculate_parent_multiline(parser_token);
        }
    }

    fn handle_tag_open(
        &mut self,
        raw: usize,
        last_tag_token: usize,
        last_token_type: &str,
    ) -> usize {
        let parser_token = self.get_tag_open_token(raw);

        if (self.parser_tokens[last_tag_token].is_unformatted
            || self.parser_tokens[last_tag_token].is_content_unformatted)
            && !self.parser_tokens[last_tag_token].is_empty_element
            && self.tokens[raw].type_ == TOKEN_TAG_OPEN
            && !self.parser_tokens[parser_token].is_start_tag
        {
            self.add_raw_token(raw);
            let tag = self.parser_tokens[parser_token].tag_name.clone();
            let popped = self.tag_stack.try_pop(&tag, None, &mut self.printer);
            self.parser_tokens[parser_token].start_tag_token = popped;
        } else {
            self.traverse_whitespace(raw);
            self.set_tag_position(raw, parser_token, last_tag_token, last_token_type);
            if !self.parser_tokens[parser_token].is_inline_element {
                self.printer.set_wrap_point();
            }
            self.print_token(raw);
        }

        if self.parser_tokens[parser_token].is_start_tag && self.is_wrap_attributes_force {
            let mut peek_index = 0usize;
            while let Some(peek) = self.peek_token(peek_index) {
                let ty = self.tokens[peek].type_.clone();
                if ty == TOKEN_ATTRIBUTE {
                    self.parser_tokens[parser_token].attr_count += 1;
                }
                peek_index += 1;
                if ty == TOKEN_EOF || ty == TOKEN_TAG_CLOSE {
                    break;
                }
            }
        }

        if self.is_wrap_attributes_force_aligned
            || self.is_wrap_attributes_aligned_multiple
            || self.is_wrap_attributes_preserve_aligned
        {
            let len = utf16_len(&self.tokens[raw].text) as i32;
            self.parser_tokens[parser_token].alignment_size = len + 1;
        }

        if !self.parser_tokens[parser_token].tag_complete
            && !self.parser_tokens[parser_token].is_unformatted
        {
            self.printer.alignment_size = self.parser_tokens[parser_token].alignment_size;
        }

        parser_token
    }

    fn handle_control_flow_open(&mut self, raw: usize) -> String {
        let ty = self.tokens[raw].type_.clone();
        let space = self.tokens[raw].newlines > 0 || !self.tokens[raw].whitespace_before.is_empty();
        self.printer.set_space_before_token(space, true);
        if self.tokens[raw].newlines > 0 {
            let prev_type = self.prev_type_of(raw);
            self.print_preserved_newlines(raw, &prev_type);
        } else {
            self.printer.set_space_before_token(space, true);
        }
        self.print_token(raw);
        self.printer.indent();
        ty
    }

    fn handle_control_flow_close(&mut self, raw: usize) -> String {
        let ty = self.tokens[raw].type_.clone();
        self.printer.deindent();
        if self.tokens[raw].newlines > 0 {
            let prev_type = self.prev_type_of(raw);
            self.print_preserved_newlines(raw, &prev_type);
        } else {
            let space =
                self.tokens[raw].newlines > 0 || !self.tokens[raw].whitespace_before.is_empty();
            self.printer.set_space_before_token(space, true);
        }
        self.print_token(raw);
        ty
    }

    fn handle_tag_close(&mut self, raw: usize, last_tag_token: usize) -> String {
        self.printer.alignment_size = 0;
        self.parser_tokens[last_tag_token].tag_complete = true;

        let space = self.tokens[raw].newlines > 0 || !self.tokens[raw].whitespace_before.is_empty();
        self.printer.set_space_before_token(space, true);

        if self.parser_tokens[last_tag_token].is_unformatted {
            self.add_raw_token(raw);
        } else {
            if self.parser_tokens[last_tag_token].tag_start_char == "<" {
                let first = self.tokens[raw].text.chars().next();
                self.printer.set_space_before_token(first == Some('/'), true);
                if self.is_wrap_attributes_force_expand_multiline
                    && self.parser_tokens[last_tag_token].has_wrapped_attrs
                {
                    self.printer.print_newline(false);
                }
            }
            self.print_token(raw);
        }

        if self.parser_tokens[last_tag_token].indent_content
            && !(self.parser_tokens[last_tag_token].is_unformatted
                || self.parser_tokens[last_tag_token].is_content_unformatted)
        {
            self.printer.indent();
            self.parser_tokens[last_tag_token].indent_content = false;
        }

        if !self.parser_tokens[last_tag_token].is_inline_element
            && !(self.parser_tokens[last_tag_token].is_unformatted
                || self.parser_tokens[last_tag_token].is_content_unformatted)
        {
            self.printer.set_wrap_point();
        }

        TOKEN_TAG_CLOSE.to_string()
    }

    fn handle_inside_tag(
        &mut self,
        raw: usize,
        last_tag_token: usize,
        last_token_type: &str,
    ) -> String {
        let mut wrapped = self.parser_tokens[last_tag_token].has_wrapped_attrs;
        let ty = self.tokens[raw].type_.clone();
        let space = self.tokens[raw].newlines > 0 || !self.tokens[raw].whitespace_before.is_empty();
        self.printer.set_space_before_token(space, true);

        if self.parser_tokens[last_tag_token].is_unformatted {
            self.add_raw_token(raw);
        } else if self.parser_tokens[last_tag_token].tag_start_char == "{" && ty == TOKEN_TEXT {
            let prev_type = self.prev_type_of(raw);
            if self.print_preserved_newlines(raw, &prev_type) {
                self.tokens[raw].newlines = 0;
                self.add_raw_token(raw);
            } else {
                self.print_token(raw);
            }
        } else {
            if ty == TOKEN_ATTRIBUTE {
                self.printer.set_space_before_token(true, false);
            } else if ty == TOKEN_EQUALS
                || (ty == TOKEN_VALUE && self.prev_type_of(raw) == TOKEN_EQUALS)
            {
                self.printer.set_space_before_token(false, false);
            }

            if ty == TOKEN_ATTRIBUTE && self.parser_tokens[last_tag_token].tag_start_char == "<" {
                if self.is_wrap_attributes_preserve || self.is_wrap_attributes_preserve_aligned {
                    self.traverse_whitespace(raw);
                    wrapped = wrapped || self.tokens[raw].newlines != 0;
                }

                if self.is_wrap_attributes_force
                    && self.parser_tokens[last_tag_token].attr_count
                        >= self.options.wrap_attributes_min_attrs
                    && (last_token_type != TOKEN_TAG_OPEN
                        || self.is_wrap_attributes_force_expand_multiline)
                {
                    self.printer.print_newline(false);
                    wrapped = true;
                }
            }
            self.print_token(raw);
            wrapped = wrapped || self.printer.previous_token_wrapped();
            self.parser_tokens[last_tag_token].has_wrapped_attrs = wrapped;
        }
        ty
    }

    fn handle_text(&mut self, raw: usize, last_tag_token: usize) -> String {
        if self.parser_tokens[last_tag_token]
            .custom_beautifier_name
            .is_some()
        {
            self.print_custom_beautifier_text(raw, last_tag_token);
        } else if self.parser_tokens[last_tag_token].is_unformatted
            || self.parser_tokens[last_tag_token].is_content_unformatted
        {
            self.add_raw_token(raw);
        } else {
            self.traverse_whitespace(raw);
            self.print_token(raw);
        }
        TK_CONTENT.to_string()
    }

    fn print_custom_beautifier_text(&mut self, raw: usize, last_tag_token: usize) {
        if self.tokens[raw].text.is_empty() {
            return;
        }
        let custom = self.parser_tokens[last_tag_token]
            .custom_beautifier_name
            .clone();
        let mut text = self.tokens[raw].text.clone();
        let mut script_indent_level = 1i32;
        let mut pre = String::new();
        let mut post = String::new();

        if self.options.indent_scripts == "keep" {
            script_indent_level = 0;
        } else if self.options.indent_scripts == "separate" {
            script_indent_level = -self.printer.indent_level;
        }

        let indentation = self.printer.get_full_indent(script_indent_level);
        text = regex_trailing_ws_line().replace(&text, "").to_string();

        if custom.as_deref() != Some("html")
            && text.starts_with('<')
            && regex_comment_or_cdata().is_match(&text)
        {
            let (g1, g2, g3, g4, g5) = match regex_cdata_block().captures(&text) {
                Some(c) => (
                    c[1].to_string(),
                    c[2].to_string(),
                    c[3].to_string(),
                    c[4].to_string(),
                    c.get(5).map(|m| m.as_str().to_string()),
                ),
                None => {
                    self.add_raw_token(raw);
                    return;
                }
            };
            pre = format!("{indentation}{g1}\n");
            text = g4;
            if let Some(g5) = g5
                && !g5.is_empty()
            {
                post = format!("{indentation}{g5}");
            }
            text = regex_trailing_ws_line().replace(&text, "").to_string();

            if (!g2.is_empty() || g3.contains('\n'))
                && let Some(m) = regex_trailing_spaces().find(&g3) {
                    self.tokens[raw].whitespace_before = m.as_str().to_string();
                }
        }

        if !text.is_empty() {
            let beautified = match custom.as_deref() {
                Some("javascript") => Some(text.clone()),
                Some("css") => {
                    Some(super::css::css_beautify(
                        &format!("{indentation}{text}"),
                        &self.child_options(),
                    ))
                }
                Some("html") => {
                    Some(html_beautify(
                        &format!("{indentation}{text}"),
                        &self.child_options(),
                    ))
                }
                _ => None,
            };

            if let Some(b) = beautified {
                text = b;
            } else {
                let white = self.tokens[raw].whitespace_before.clone();
                if !white.is_empty() {
                    let re = Regex::new(&format!("\n({})?", regex::escape(&white))).unwrap();
                    text = re.replace_all(&text, "\n").to_string();
                }
                text = format!(
                    "{indentation}{}",
                    text.replace('\n', &format!("\n{indentation}"))
                );
            }
        }

        if !pre.is_empty() {
            if text.is_empty() {
                text = format!("{pre}{post}");
            } else {
                text = format!("{pre}{text}\n{post}");
            }
        }

        self.printer.print_newline(false);
        if !text.is_empty() {
            self.tokens[raw].text = text;
            self.tokens[raw].whitespace_before = String::new();
            self.tokens[raw].newlines = 0;
            self.add_raw_token(raw);
            self.printer.print_newline(true);
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
        let base_indent_string = leading_indent(&source_text);

        let mut tokenizer = HtmlTokenizer::new(&source_text, &self.options);
        self.tokens = tokenizer.tokenize();
        self.position = 0;
        self.printer = Printer::new(&self.options, &base_indent_string);
        self.tag_stack = TagStack::new();
        self.parser_tokens = vec![ParserToken::empty()];

        let mut last_token_type = String::new();
        let mut last_tag_token = 0usize;

        while let Some(raw) = self.next_token() {
            let ty = self.tokens[raw].type_.clone();
            if ty == TOKEN_EOF {
                break;
            }

            if ty == TOKEN_TAG_OPEN || ty == TOKEN_COMMENT {
                last_tag_token = self.handle_tag_open(raw, last_tag_token, &last_token_type);
                last_token_type = TOKEN_TAG_OPEN.to_string();
            } else if ty == TOKEN_ATTRIBUTE
                || ty == TOKEN_EQUALS
                || ty == TOKEN_VALUE
                || (ty == TOKEN_TEXT && !self.parser_tokens[last_tag_token].tag_complete)
            {
                last_token_type = self.handle_inside_tag(raw, last_tag_token, &last_token_type);
            } else if ty == TOKEN_TAG_CLOSE {
                last_token_type = self.handle_tag_close(raw, last_tag_token);
            } else if ty == TOKEN_TEXT {
                last_token_type = self.handle_text(raw, last_tag_token);
            } else if ty == TOKEN_CONTROL_FLOW_OPEN {
                last_token_type = self.handle_control_flow_open(raw);
            } else if ty == TOKEN_CONTROL_FLOW_CLOSE {
                last_token_type = self.handle_control_flow_close(raw);
            } else {
                self.add_raw_token(raw);
            }
        }

        self.printer.output.get_code(&eol)
    }
}

/// Port of `html_beautify`.
pub fn html_beautify(source_text: &str, options: &RawOptions) -> String {
    let mut beautifier = HtmlBeautifier::new(source_text, options);
    beautifier.beautify()
}
