//! Port of the js-beautify `InputScanner`, `Pattern`, `WhitespacePattern`,
//! `TemplatablePattern` and `Directives`.
//!
//! The JavaScript originals match with a `RegExp` whose `lastIndex` is set to
//! the scan position; with the sticky (`y`) flag the match must start exactly
//! there, and without it `InputScanner.__match` rejects a match that does not
//! start at the index. Both cases therefore denote an *anchored* match, which
//! is what [`Pattern::match_len`] implements. Only `readUntil` searches
//! forward.

use regex::Regex;

/// The largest char boundary at or before `pos`.
fn snap(input: &str, pos: usize) -> usize {
    let mut pos = pos.min(input.len());
    while pos > 0 && !input.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

/// A compiled js-beautify pattern, with the source it was built from.
#[derive(Clone, Debug)]
pub struct Pattern {
    pub source: String,
    re: Regex,
}

impl Pattern {
    pub fn new(source: impl Into<String>) -> Self {
        let source = source.into();
        let re = Regex::new(&source).unwrap_or_else(|e| panic!("invalid pattern {source:?}: {e}"));
        Self { source, re }
    }

    /// Build `(?:a|b|c)` from the given sources.
    pub fn any_of(sources: &[String]) -> Self {
        Self::new(format!("(?:{})", sources.join("|")))
    }

    pub fn as_regex(&self) -> &Regex {
        &self.re
    }

    /// Port of `InputScanner.prototype.__match` used via `match()`: the match
    /// must start exactly at `pos`. Returns its length in bytes.
    pub fn match_len(&self, input: &str, pos: usize) -> Option<usize> {
        let pos = snap(input, pos);
        let m = self.re.find_at(input, pos)?;
        if m.start() == pos {
            Some(m.end() - pos)
        } else {
            None
        }
    }

    /// Port of `RegExp.exec` used by `readUntil`: leftmost match at or after `pos`.
    pub fn find(&self, input: &str, pos: usize) -> Option<(usize, usize)> {
        let pos = snap(input, pos);
        self.re.find_at(input, pos).map(|m| (m.start(), m.end()))
    }
}

/// Port of `String.prototype.split(regexp)` for a non-empty-matching regexp.
pub fn split_on_pattern(pattern: &Regex, input: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut start = 0;
    for m in pattern.find_iter(input) {
        result.push(input[start..m.start()].to_string());
        start = m.end();
    }
    if start < input.len() {
        result.push(input[start..].to_string());
    } else {
        result.push(String::new());
    }
    result
}

/// Port of the js-beautify `InputScanner`.
pub struct InputScanner {
    pub input: String,
    pub position: usize,
}

impl InputScanner {
    pub fn new(input: &str) -> Self {
        Self {
            input: input.to_string(),
            position: 0,
        }
    }

    pub fn restart(&mut self) {
        self.position = 0;
    }

    pub fn back(&mut self) {
        if self.position > 0 {
            self.position -= 1;
        }
    }

    pub fn has_next(&self) -> bool {
        self.position < self.input.len()
    }

    pub fn next_char(&mut self) -> Option<char> {
        if self.position >= self.input.len() {
            return None;
        }
        let ch = self.input[self.position..].chars().next()?;
        self.position += ch.len_utf8();
        Some(ch)
    }

    pub fn peek(&self, index: i64) -> Option<char> {
        let idx = self.position as i64 + index;
        if idx < 0 {
            return None;
        }
        let idx = idx as usize;
        if idx >= self.input.len() || !self.input.is_char_boundary(idx) {
            return None;
        }
        self.input[idx..].chars().next()
    }

    /// Port of `InputScanner.prototype.test` (anchored, without consuming).
    pub fn test(&self, pattern: &Pattern) -> bool {
        pattern.match_len(&self.input, self.position).is_some()
    }

    /// Port of `InputScanner.prototype.testChar`.
    pub fn test_char(&self, pattern: &Regex, index: i64) -> bool {
        match self.peek(index) {
            Some(ch) => pattern.is_match(&ch.to_string()),
            None => false,
        }
    }

    /// Port of `InputScanner.prototype.read`: an anchored `starting_pattern`
    /// match, then `readUntil(until_pattern, until_after)` (only when the
    /// starting pattern matched, or when there is none).
    pub fn read(
        &mut self,
        starting_pattern: Option<&Pattern>,
        until_pattern: Option<&Pattern>,
        until_after: bool,
    ) -> String {
        let mut val = String::new();
        let mut matched = false;
        if let Some(starting) = starting_pattern
            && let Some(len) = starting.match_len(&self.input, self.position)
        {
            let end = self.position + len;
            val.push_str(&self.input[self.position..end]);
            self.position = end;
            matched = true;
        }
        if let Some(until) = until_pattern
            && (matched || starting_pattern.is_none())
        {
            val += &self.read_until(until, until_after);
        }
        val
    }

    /// Port of `InputScanner.prototype.readUntil`.
    pub fn read_until(&mut self, pattern: &Pattern, until_after: bool) -> String {
        let match_index = match pattern.find(&self.input, self.position) {
            Some((start, end)) => {
                if until_after {
                    end
                } else {
                    start
                }
            }
            None => self.input.len(),
        };
        let val = self.input[self.position..match_index].to_string();
        self.position = match_index;
        val
    }

    pub fn read_until_after(&mut self, pattern: &Pattern) -> String {
        self.read_until(pattern, true)
    }

    /// Port of `InputScanner.prototype.peekUntilAfter`.
    pub fn peek_until_after(&mut self, pattern: &Pattern) -> String {
        let start = self.position;
        let val = self.read_until_after(pattern);
        self.position = start;
        val
    }

    /// Port of `InputScanner.prototype.lookBack`.
    pub fn look_back(&self, test_val: &str) -> bool {
        let start = self.position as i64 - 1;
        if start < test_val.len() as i64 {
            return false;
        }
        let start = start as usize;
        let from = start - test_val.len();
        if !self.input.is_char_boundary(from) || !self.input.is_char_boundary(start) {
            return false;
        }
        self.input[from..start].to_lowercase() == test_val
    }
}

/// Port of the js-beautify `Pattern` class.
#[derive(Clone, Debug, Default)]
pub struct JsPattern {
    pub starting: Option<Pattern>,
    pub match_pattern: Option<Pattern>,
    pub until: Option<Pattern>,
    pub until_after: bool,
}

impl JsPattern {
    /// `pattern.starting_with(regexp)`
    pub fn starting_with(pattern: Pattern) -> Self {
        Self {
            starting: Some(pattern),
            ..Default::default()
        }
    }

    /// `pattern.until(regexp)`
    pub fn until(mut self, pattern: Pattern) -> Self {
        self.until_after = false;
        self.until = Some(pattern);
        self
    }

    /// `pattern.until_after(regexp)`
    pub fn until_after(mut self, pattern: Pattern) -> Self {
        self.until_after = true;
        self.until = Some(pattern);
        self
    }

    /// `pattern.matching(regexp)`
    pub fn matching(mut self, pattern: Pattern) -> Self {
        self.match_pattern = Some(pattern);
        self
    }

    /// Port of `Pattern.prototype.read`.
    pub fn read(&self, input: &mut InputScanner) -> String {
        let mut result = input.read(self.starting.as_ref(), None, false);
        if self.starting.is_none() || !result.is_empty() {
            result += &input.read(
                self.match_pattern.as_ref(),
                self.until.as_ref(),
                self.until_after,
            );
        }
        result
    }
}

/// Port of `WhitespacePattern`.
///
/// Only the root instance (no parent) is used by the bundled beautifiers, so
/// the `matching(whitespace_chars, newline_chars)` constructor is not needed.
pub struct WhitespacePattern {
    match_pattern: Pattern,
    newline_pattern: Pattern,
    pub newline_count: usize,
    pub whitespace_before_token: String,
}

impl WhitespacePattern {
    pub fn new() -> Self {
        Self {
            match_pattern: Pattern::new("[\\t \\n\\r]+"),
            newline_pattern: Pattern::new("\\r\\n|[\\n\\r]"),
            newline_count: 0,
            whitespace_before_token: String::new(),
        }
    }

    /// Port of `WhitespacePattern.prototype.read`.
    pub fn read(&mut self, input: &mut InputScanner) -> String {
        self.newline_count = 0;
        self.whitespace_before_token = String::new();

        let resulting_string = input.read(Some(&self.match_pattern), None, false);
        if resulting_string == " " {
            self.whitespace_before_token = " ".to_string();
        } else if !resulting_string.is_empty() {
            let matches = split_on_pattern(self.newline_pattern.as_regex(), &resulting_string);
            self.newline_count = matches.len() - 1;
            self.whitespace_before_token = matches[self.newline_count].clone();
        }
        resulting_string
    }
}

impl Default for WhitespacePattern {
    fn default() -> Self {
        Self::new()
    }
}

/// The templating languages tracked by `TemplatablePattern`.
#[derive(Clone, Copy, Default, Debug)]
pub struct TemplateNames {
    pub django: bool,
    pub erb: bool,
    pub handlebars: bool,
    pub php: bool,
    pub smarty: bool,
    pub angular: bool,
}

impl TemplateNames {
    fn set(&mut self, language: &str, value: bool) {
        match language {
            "django" => self.django = value,
            "erb" => self.erb = value,
            "handlebars" => self.handlebars = value,
            "php" => self.php = value,
            "smarty" => self.smarty = value,
            "angular" => self.angular = value,
            _ => {}
        }
    }

    const LANGUAGES: [&'static str; 6] =
        ["django", "erb", "handlebars", "php", "smarty", "angular"];
}

/// The sub-patterns of `TemplatablePattern.__patterns`.
#[derive(Clone, Debug)]
struct TemplatePatterns {
    handlebars_comment: JsPattern,
    handlebars_unescaped: JsPattern,
    handlebars: JsPattern,
    php: JsPattern,
    erb: JsPattern,
    django: JsPattern,
    django_value: JsPattern,
    django_comment: JsPattern,
    smarty: JsPattern,
    smarty_comment: JsPattern,
    smarty_literal: JsPattern,
}

fn new_template_patterns() -> TemplatePatterns {
    TemplatePatterns {
        handlebars_comment: JsPattern::starting_with(Pattern::new(r"\{\{!--"))
            .until_after(Pattern::new("--\\}\\}")),
        handlebars_unescaped: JsPattern::starting_with(Pattern::new(r"\{\{\{"))
            .until_after(Pattern::new(r"\}\}\}")),
        handlebars: JsPattern::starting_with(Pattern::new(r"\{\{"))
            .until_after(Pattern::new(r"\}\}")),
        php: JsPattern::starting_with(Pattern::new(r"<\?(?:[= ]|php)"))
            .until_after(Pattern::new(r"\?>")),
        erb: JsPattern::starting_with(Pattern::new("<%[^%]")).until_after(Pattern::new("[^%]%>")),
        django: JsPattern::starting_with(Pattern::new(r"\{%")).until_after(Pattern::new("%\\}")),
        django_value: JsPattern::starting_with(Pattern::new(r"\{\{"))
            .until_after(Pattern::new(r"\}\}")),
        django_comment: JsPattern::starting_with(Pattern::new(r"\{#"))
            .until_after(Pattern::new("#\\}")),
        // `{(?=[^}{\s\n])` cannot be expressed without lookahead; consuming the
        // lookahead character yields the same overall match text.
        smarty: JsPattern::starting_with(Pattern::new(r"\{[^}{\s\n]"))
            .until_after(Pattern::new(r"[^\s\n]\}")),
        smarty_comment: JsPattern::starting_with(Pattern::new(r"\{\*"))
            .until_after(Pattern::new(r"\*\}")),
        smarty_literal: JsPattern::starting_with(Pattern::new(r"\{literal\}"))
            .until_after(Pattern::new(r"\{/literal\}")),
    }
}

/// Port of `TemplatablePattern`.
#[derive(Clone, Debug)]
pub struct TemplatablePattern {
    pub js: JsPattern,
    template_pattern: Pattern,
    pub disabled: TemplateNames,
    pub excluded: TemplateNames,
    patterns: TemplatePatterns,
}

impl TemplatablePattern {
    pub fn new() -> Self {
        let mut result = Self {
            js: JsPattern::default(),
            template_pattern: Pattern::new("(?:)"),
            disabled: TemplateNames::default(),
            excluded: TemplateNames::default(),
            patterns: new_template_patterns(),
        };
        result.update();
        result
    }

    /// Port of `TemplatablePattern.prototype.read_options`.
    pub fn read_options(mut self, templating: &[String]) -> Self {
        for language in TemplateNames::LANGUAGES {
            self.disabled
                .set(language, !templating.iter().any(|t| t == language));
        }
        self.update();
        self
    }

    /// Port of `TemplatablePattern.prototype.exclude`.
    pub fn exclude(mut self, language: &str) -> Self {
        self.excluded.set(language, true);
        self.update();
        self
    }

    /// Port of `TemplatablePattern.prototype.until` / `until_after` / `matching`.
    pub fn until(mut self, pattern: Pattern) -> Self {
        self.js.until_after = false;
        self.js.until = Some(pattern);
        self.update();
        self
    }

    pub fn until_after(mut self, pattern: Pattern) -> Self {
        self.js.until_after = true;
        self.js.until = Some(pattern);
        self.update();
        self
    }

    /// Port of `TemplatablePattern.prototype.__set_templated_pattern`.
    fn update(&mut self) {
        let mut items: Vec<String> = Vec::new();

        if !self.disabled.php {
            items.push(self.patterns.php.starting.as_ref().unwrap().source.clone());
        }
        if !self.disabled.handlebars {
            items.push(
                self.patterns
                    .handlebars
                    .starting
                    .as_ref()
                    .unwrap()
                    .source
                    .clone(),
            );
        }
        if !self.disabled.angular {
            items.push(
                self.patterns
                    .handlebars
                    .starting
                    .as_ref()
                    .unwrap()
                    .source
                    .clone(),
            );
        }
        if !self.disabled.erb {
            items.push(self.patterns.erb.starting.as_ref().unwrap().source.clone());
        }
        if !self.disabled.django {
            items.push(
                self.patterns
                    .django
                    .starting
                    .as_ref()
                    .unwrap()
                    .source
                    .clone(),
            );
            items.push(
                self.patterns
                    .django_value
                    .starting
                    .as_ref()
                    .unwrap()
                    .source
                    .clone(),
            );
            items.push(
                self.patterns
                    .django_comment
                    .starting
                    .as_ref()
                    .unwrap()
                    .source
                    .clone(),
            );
        }
        if !self.disabled.smarty {
            items.push(
                self.patterns
                    .smarty
                    .starting
                    .as_ref()
                    .unwrap()
                    .source
                    .clone(),
            );
        }

        if let Some(until) = &self.js.until {
            items.push(until.source.clone());
        }
        self.template_pattern = Pattern::any_of(&items);
    }

    /// Port of `TemplatablePattern.prototype.read`.
    pub fn read(&self, input: &mut InputScanner) -> String {
        let mut result = if self.js.match_pattern.is_some() {
            input.read(self.js.starting.as_ref(), None, false)
        } else {
            input.read(
                self.js.starting.as_ref(),
                Some(&self.template_pattern),
                false,
            )
        };
        let mut next = self.read_template(input);
        while !next.is_empty() {
            if let Some(match_pattern) = &self.js.match_pattern {
                next += &input.read(Some(match_pattern), None, false);
            } else {
                next += &input.read_until(&self.template_pattern, false);
            }
            result += &next;
            next = self.read_template(input);
        }

        if self.js.until_after
            && let Some(until) = &self.js.until
        {
            result += &input.read_until(until, true);
        }
        result
    }

    /// Port of `TemplatablePattern.prototype._read_template`.
    fn read_template(&self, input: &mut InputScanner) -> String {
        let mut resulting_string = String::new();
        let c = input.peek(0);
        if c == Some('<') {
            let peek1 = input.peek(1);
            if !self.disabled.php && !self.excluded.php && peek1 == Some('?') {
                resulting_string = self.patterns.php.read(input);
            }
            if resulting_string.is_empty()
                && !self.disabled.erb
                && !self.excluded.erb
                && peek1 == Some('%')
            {
                resulting_string = self.patterns.erb.read(input);
            }
        } else if c == Some('{') {
            if !self.disabled.handlebars && !self.excluded.handlebars {
                if resulting_string.is_empty() {
                    resulting_string = self.patterns.handlebars_comment.read(input);
                }
                if resulting_string.is_empty() {
                    resulting_string = self.patterns.handlebars_unescaped.read(input);
                }
                if resulting_string.is_empty() {
                    resulting_string = self.patterns.handlebars.read(input);
                }
            }
            if !self.disabled.django {
                if !self.excluded.django && !self.excluded.handlebars && resulting_string.is_empty()
                {
                    resulting_string = self.patterns.django_value.read(input);
                }
                if !self.excluded.django {
                    if resulting_string.is_empty() {
                        resulting_string = self.patterns.django_comment.read(input);
                    }
                    if resulting_string.is_empty() {
                        resulting_string = self.patterns.django.read(input);
                    }
                }
            }
            if !self.disabled.smarty && self.disabled.django && self.disabled.handlebars {
                if resulting_string.is_empty() {
                    resulting_string = self.patterns.smarty_comment.read(input);
                }
                if resulting_string.is_empty() {
                    resulting_string = self.patterns.smarty_literal.read(input);
                }
                if resulting_string.is_empty() {
                    resulting_string = self.patterns.smarty.read(input);
                }
            }
        }
        resulting_string
    }
}

impl Default for TemplatablePattern {
    fn default() -> Self {
        Self::new()
    }
}

/// Port of the js-beautify `Directives` class.
pub struct Directives {
    block_pattern: Regex,
    directive_pattern: Regex,
    end_ignore_pattern: Pattern,
}

impl Directives {
    pub fn new(start_block_pattern: &str, end_block_pattern: &str) -> Self {
        Self {
            block_pattern: Regex::new(&format!(
                "{start_block_pattern}\\s*beautify(\\s+\\w+[:]\\w+)+\\s*{end_block_pattern}"
            ))
            .expect("valid directives block pattern"),
            directive_pattern: Regex::new(" (\\w+)[:](\\w+)").expect("valid directive pattern"),
            end_ignore_pattern: Pattern::new(format!(
                "{start_block_pattern}\\sbeautify\\signore:end\\s{end_block_pattern}"
            )),
        }
    }

    /// Port of `Directives.prototype.get_directives`.
    pub fn get_directives(&self, text: &str) -> Option<Vec<(String, String)>> {
        if !self.block_pattern.is_match(text) {
            return None;
        }
        Some(
            self.directive_pattern
                .captures_iter(text)
                .map(|c| (c[1].to_string(), c[2].to_string()))
                .collect(),
        )
    }

    /// Port of `Directives.prototype.readIgnored`.
    pub fn read_ignored(&self, input: &mut InputScanner) -> String {
        input.read_until_after(&self.end_ignore_pattern)
    }
}

/// Port of `directives.ignore === 'start'`.
pub fn ignores_start(directives: &Option<Vec<(String, String)>>) -> bool {
    directives
        .as_ref()
        .is_some_and(|d| d.iter().any(|(k, v)| k == "ignore" && v == "start"))
}
