//! Port of `src/parser/htmlScanner.ts`.
//!
//! Operates on UTF-16 code units to match JavaScript string semantics exactly.

use crate::html_language_types::{ScannerState, TokenType};

const BNG: u16 = b'!' as u16;
const MIN: u16 = b'-' as u16;
pub const LAN: u16 = b'<' as u16;
const RAN: u16 = b'>' as u16;
const FSL: u16 = b'/' as u16;
const EQS: u16 = b'=' as u16;
const DQO: u16 = b'"' as u16;
const SQO: u16 = b'\'' as u16;
const BQT: u16 = b'`' as u16;
const NWL: u16 = b'\n' as u16;
const CAR: u16 = b'\r' as u16;
const LFD: u16 = 0x0C;
const WSP: u16 = b' ' as u16;
const TAB: u16 = b'\t' as u16;

fn unit_to_string(units: &[u16]) -> String {
    String::from_utf16_lossy(units)
}

/// JavaScript `\s` character class.
fn is_js_whitespace(c: u16) -> bool {
    matches!(
        c,
        0x09..=0x0D
            | 0x20
            | 0xA0
            | 0x1680
            | 0x2000..=0x200A
            | 0x2028
            | 0x2029
            | 0x202F
            | 0x205F
            | 0x3000
            | 0xFEFF
    )
}

fn is_word(c: u16) -> bool {
    (c >= b'a' as u16 && c <= b'z' as u16)
        || (c >= b'A' as u16 && c <= b'Z' as u16)
        || (c >= b'0' as u16 && c <= b'9' as u16)
        || c == b'_' as u16
}

fn is_element_name_start(c: u16) -> bool {
    c == b':' as u16 || is_word(c)
}

fn is_element_name_part(c: u16) -> bool {
    is_element_name_start(c) || c == b'.' as u16 || c == MIN
}

/// Matches `[^\s"'></=\x00-\x0F\x7F\x80-\x9F]`.
fn is_attr_name_char(c: u16) -> bool {
    !is_js_whitespace(c)
        && c != DQO
        && c != SQO
        && c != RAN
        && c != LAN
        && c != FSL
        && c != EQS
        && c > 0x0F
        && !(0x7F..=0x9F).contains(&c)
}

/// Matches `[^\s"'`=<>]`.
fn is_attr_value_char(c: u16) -> bool {
    !is_js_whitespace(c) && c != DQO && c != SQO && c != BQT && c != EQS && c != LAN && c != RAN
}

fn ascii_lower(c: u16) -> u16 {
    if (b'A' as u16..=b'Z' as u16).contains(&c) {
        c + 32
    } else {
        c
    }
}

struct MultiLineStream {
    source: Vec<u16>,
    len: usize,
    position: usize,
}

impl MultiLineStream {
    fn new(source: Vec<u16>, position: usize) -> Self {
        let len = source.len();
        Self {
            source,
            len,
            position,
        }
    }

    fn eos(&self) -> bool {
        self.len <= self.position
    }

    fn pos(&self) -> usize {
        self.position
    }

    fn go_back(&mut self, n: usize) {
        self.position -= n;
    }

    fn advance(&mut self, n: usize) {
        self.position += n;
    }

    fn go_to_end(&mut self) {
        self.position = self.source.len();
    }

    fn peek_char(&self, n: i32) -> u16 {
        let idx = self.position as i64 + n as i64;
        if idx < 0 {
            return 0;
        }
        self.source.get(idx as usize).copied().unwrap_or(0)
    }

    fn advance_if_char(&mut self, ch: u16) -> bool {
        if self.source.get(self.position).copied().unwrap_or(0) == ch {
            self.position += 1;
            return true;
        }
        false
    }

    fn advance_if_chars(&mut self, ch: &[u16]) -> bool {
        if self.position + ch.len() > self.source.len() {
            return false;
        }
        for (i, &expected) in ch.iter().enumerate() {
            if self.source.get(self.position + i).copied().unwrap_or(0) != expected {
                return false;
            }
        }
        self.position += ch.len();
        true
    }

    fn advance_until_char(&mut self, ch: u16) -> bool {
        while self.position < self.source.len() {
            if self.source[self.position] == ch {
                return true;
            }
            self.advance(1);
        }
        false
    }

    fn advance_until_chars(&mut self, ch: &[u16]) -> bool {
        while self.position + ch.len() <= self.source.len() {
            let mut i = 0;
            while i < ch.len() && self.source[self.position + i] == ch[i] {
                i += 1;
            }
            if i == ch.len() {
                return true;
            }
            self.advance(1);
        }
        self.go_to_end();
        false
    }

    fn skip_whitespace(&mut self) -> bool {
        let n = self
            .advance_while_char(|ch| ch == WSP || ch == TAB || ch == NWL || ch == LFD || ch == CAR);
        n > 0
    }

    fn advance_while_char(&mut self, condition: impl Fn(u16) -> bool) -> usize {
        let pos_now = self.position;
        while self.position < self.len && condition(self.source[self.position]) {
            self.position += 1;
        }
        self.position - pos_now
    }

    /// Port of `advanceIfRegExp(/^[_:\w][_:\w.\d-]*/)` followed by `toLowerCase()`.
    fn next_element_name(&mut self) -> String {
        let start = self.position;
        if self.position < self.len && is_element_name_start(self.source[self.position]) {
            self.position += 1;
            while self.position < self.len && is_element_name_part(self.source[self.position]) {
                self.position += 1;
            }
        }
        unit_to_string(&self.source[start..self.position]).to_lowercase()
    }

    /// Port of `advanceIfRegExp(/^[^\s"'></=\x00-\x0F\x7F\x80-\x9F]*/)` + `toLowerCase()`.
    fn next_attribute_name(&mut self) -> String {
        let start = self.position;
        while self.position < self.len && is_attr_name_char(self.source[self.position]) {
            self.position += 1;
        }
        unit_to_string(&self.source[start..self.position]).to_lowercase()
    }

    /// Port of `advanceIfRegExp(/^!doctype/i)`.
    fn advance_if_doctype(&mut self) -> bool {
        const DOCTYPE: &[u8] = b"!doctype";
        if self.position + DOCTYPE.len() > self.source.len() {
            return false;
        }
        for (i, &b) in DOCTYPE.iter().enumerate() {
            if ascii_lower(self.source[self.position + i]) != b as u16 {
                return false;
            }
        }
        self.position += DOCTYPE.len();
        true
    }

    /// Port of `advanceIfRegExp(/^[^\s"'`=<>]+/)`.
    fn advance_if_attr_value(&mut self) -> Option<(usize, usize)> {
        let start = self.position;
        while self.position < self.len && is_attr_value_char(self.source[self.position]) {
            self.position += 1;
        }
        if self.position > start {
            Some((start, self.position))
        } else {
            None
        }
    }

    /// Port of `advanceIfRegExp(/<!--|-->|<\/?script\s*\/?>?/i)`.
    ///
    /// The regex is not anchored, so this searches forward for the first match
    /// and advances to the end of it. Returns the matched text.
    fn advance_if_script_tag(&mut self) -> Option<String> {
        let mut i = self.position;
        while i < self.len {
            if let Some(len) = self.match_script_at(i) {
                let matched = unit_to_string(&self.source[i..i + len]);
                self.position = i + len;
                return Some(matched);
            }
            i += 1;
        }
        None
    }

    fn match_script_at(&self, i: usize) -> Option<usize> {
        // `<!--`
        if i + 4 <= self.len
            && self.source[i] == LAN
            && self.source[i + 1] == BNG
            && self.source[i + 2] == MIN
            && self.source[i + 3] == MIN
        {
            return Some(4);
        }
        // `-->`
        if i + 3 <= self.len
            && self.source[i] == MIN
            && self.source[i + 1] == MIN
            && self.source[i + 2] == RAN
        {
            return Some(3);
        }
        // `<\/?script\s*\/?>?`
        if self.source[i] != LAN {
            return None;
        }
        let mut j = i + 1;
        if self.source.get(j).copied() == Some(FSL) {
            j += 1;
        }
        const SCRIPT: &[u8] = b"script";
        if j + SCRIPT.len() > self.len {
            return None;
        }
        for (k, &b) in SCRIPT.iter().enumerate() {
            if ascii_lower(self.source[j + k]) != b as u16 {
                return None;
            }
        }
        j += SCRIPT.len();
        while j < self.len && is_js_whitespace(self.source[j]) {
            j += 1;
        }
        if self.source.get(j).copied() == Some(FSL) {
            j += 1;
        }
        if self.source.get(j).copied() == Some(RAN) {
            j += 1;
        }
        Some(j - i)
    }

    /// Port of `advanceUntilRegExp(/<\/style/i)`. Positions at the match start.
    fn advance_until_style_end(&mut self) -> bool {
        let mut i = self.position;
        while i < self.len {
            if self.match_style_end_at(i) {
                self.position = i;
                return true;
            }
            i += 1;
        }
        self.go_to_end();
        false
    }

    fn match_style_end_at(&self, i: usize) -> bool {
        const STYLE: &[u8] = b"</style";
        if i + STYLE.len() > self.len {
            return false;
        }
        for (k, &b) in STYLE.iter().enumerate() {
            if ascii_lower(self.source[i + k]) != b as u16 {
                return false;
            }
        }
        true
    }

    fn substring(&self, start: usize, end: usize) -> String {
        let end = end.min(self.len);
        let start = start.min(end);
        unit_to_string(&self.source[start..end])
    }
}

/// Port of `createScanner`. Operates on `input` starting at `initial_offset`
/// (a UTF-16 code-unit offset).
pub fn create_scanner(
    input: &str,
    initial_offset: usize,
    initial_state: ScannerState,
    emit_pseudo_close_tags: bool,
) -> Scanner {
    Scanner::new(input, initial_offset, initial_state, emit_pseudo_close_tags)
}

/// Port of the `Scanner` interface returned by `createScanner`.
pub struct Scanner {
    stream: MultiLineStream,
    state: ScannerState,
    token_offset: usize,
    token_type: TokenType,
    token_error: Option<String>,
    has_space_after_tag: bool,
    last_tag: String,
    last_attribute_name: Option<String>,
    last_type_value: Option<String>,
    emit_pseudo_close_tags: bool,
}

impl Scanner {
    pub fn new(
        input: &str,
        initial_offset: usize,
        initial_state: ScannerState,
        emit_pseudo_close_tags: bool,
    ) -> Self {
        let mut units: Vec<u16> = input.encode_utf16().collect();
        super::php::mask_php(input, &mut units);
        Self {
            stream: MultiLineStream::new(units, initial_offset),
            state: initial_state,
            token_offset: 0,
            token_type: TokenType::Unknown,
            token_error: None,
            has_space_after_tag: false,
            last_tag: String::new(),
            last_attribute_name: None,
            last_type_value: None,
            emit_pseudo_close_tags,
        }
    }

    fn finish_token(
        &mut self,
        offset: usize,
        token_type: TokenType,
        error_message: Option<String>,
    ) -> TokenType {
        self.token_type = token_type;
        self.token_offset = offset;
        self.token_error = error_message;
        token_type
    }

    pub fn scan(&mut self) -> TokenType {
        let offset = self.stream.pos();
        let old_state = self.state;
        let token = self.internal_scan();
        if token != TokenType::Eos
            && offset == self.stream.pos()
            && !(self.emit_pseudo_close_tags
                && (token == TokenType::StartTagClose || token == TokenType::EndTagClose))
        {
            eprintln!(
                "Scanner.scan has not advanced at offset {offset}, state before: {old_state:?} after: {:?}",
                self.state
            );
            self.stream.advance(1);
            return self.finish_token(offset, TokenType::Unknown, None);
        }
        token
    }

    fn internal_scan(&mut self) -> TokenType {
        let offset = self.stream.pos();
        if self.stream.eos() {
            return self.finish_token(offset, TokenType::Eos, None);
        }

        match self.state {
            ScannerState::WithinComment => {
                if self.stream.advance_if_chars(&[MIN, MIN, RAN]) {
                    self.state = ScannerState::WithinContent;
                    return self.finish_token(offset, TokenType::EndCommentTag, None);
                }
                self.stream.advance_until_chars(&[MIN, MIN, RAN]);
                self.finish_token(offset, TokenType::Comment, None)
            }
            ScannerState::WithinDoctype => {
                if self.stream.advance_if_char(RAN) {
                    self.state = ScannerState::WithinContent;
                    return self.finish_token(offset, TokenType::EndDoctypeTag, None);
                }
                self.stream.advance_until_char(RAN);
                self.finish_token(offset, TokenType::Doctype, None)
            }
            ScannerState::WithinContent => {
                if self.stream.advance_if_char(LAN) {
                    if !self.stream.eos() && self.stream.peek_char(0) == BNG {
                        if self.stream.advance_if_chars(&[BNG, MIN, MIN]) {
                            self.state = ScannerState::WithinComment;
                            return self.finish_token(offset, TokenType::StartCommentTag, None);
                        }
                        if self.stream.advance_if_doctype() {
                            self.state = ScannerState::WithinDoctype;
                            return self.finish_token(offset, TokenType::StartDoctypeTag, None);
                        }
                    }
                    if self.stream.advance_if_char(FSL) {
                        self.state = ScannerState::AfterOpeningEndTag;
                        return self.finish_token(offset, TokenType::EndTagOpen, None);
                    }
                    self.state = ScannerState::AfterOpeningStartTag;
                    return self.finish_token(offset, TokenType::StartTagOpen, None);
                }
                self.stream.advance_until_char(LAN);
                self.finish_token(offset, TokenType::Content, None)
            }
            ScannerState::AfterOpeningEndTag => {
                let tag_name = self.stream.next_element_name();
                if !tag_name.is_empty() {
                    self.state = ScannerState::WithinEndTag;
                    return self.finish_token(offset, TokenType::EndTag, None);
                }
                if self.stream.skip_whitespace() {
                    return self.finish_token(
                        offset,
                        TokenType::Whitespace,
                        Some("Tag name must directly follow the open bracket.".to_string()),
                    );
                }
                self.state = ScannerState::WithinEndTag;
                self.stream.advance_until_char(RAN);
                if offset < self.stream.pos() {
                    return self.finish_token(
                        offset,
                        TokenType::Unknown,
                        Some("End tag name expected.".to_string()),
                    );
                }
                self.internal_scan()
            }
            ScannerState::WithinEndTag => {
                if self.stream.skip_whitespace() {
                    return self.finish_token(offset, TokenType::Whitespace, None);
                }
                if self.stream.advance_if_char(RAN) {
                    self.state = ScannerState::WithinContent;
                    return self.finish_token(offset, TokenType::EndTagClose, None);
                }
                if self.emit_pseudo_close_tags && self.stream.peek_char(0) == LAN {
                    self.state = ScannerState::WithinContent;
                    return self.finish_token(
                        offset,
                        TokenType::EndTagClose,
                        Some("Closing bracket missing.".to_string()),
                    );
                }
                let error_message = Some("Closing bracket expected.".to_string());
                self.stream.advance(1);
                self.state = ScannerState::WithinContent;
                self.finish_token(offset, TokenType::Unknown, error_message)
            }
            ScannerState::AfterOpeningStartTag => {
                self.last_tag = self.stream.next_element_name();
                self.last_type_value = None;
                self.last_attribute_name = None;
                if !self.last_tag.is_empty() {
                    self.has_space_after_tag = false;
                    self.state = ScannerState::WithinTag;
                    return self.finish_token(offset, TokenType::StartTag, None);
                }
                if self.stream.skip_whitespace() {
                    return self.finish_token(
                        offset,
                        TokenType::Whitespace,
                        Some("Tag name must directly follow the open bracket.".to_string()),
                    );
                }
                self.state = ScannerState::WithinTag;
                self.stream.advance_until_char(RAN);
                if offset < self.stream.pos() {
                    return self.finish_token(
                        offset,
                        TokenType::Unknown,
                        Some("Start tag name expected.".to_string()),
                    );
                }
                self.internal_scan()
            }
            ScannerState::WithinTag => {
                if self.stream.skip_whitespace() {
                    self.has_space_after_tag = true;
                    return self.finish_token(offset, TokenType::Whitespace, None);
                }
                if self.has_space_after_tag {
                    let name = self.stream.next_attribute_name();
                    if !name.is_empty() {
                        self.last_attribute_name = Some(name);
                        self.state = ScannerState::AfterAttributeName;
                        self.has_space_after_tag = false;
                        return self.finish_token(offset, TokenType::AttributeName, None);
                    }
                }
                if self.stream.advance_if_chars(&[FSL, RAN]) {
                    self.state = ScannerState::WithinContent;
                    return self.finish_token(offset, TokenType::StartTagSelfClose, None);
                }
                if self.stream.advance_if_char(RAN) {
                    if self.last_tag == "script" {
                        let stays_html = matches!(
                            self.last_type_value.as_deref(),
                            Some("text/x-handlebars-template") | Some("text/html")
                        );
                        if stays_html {
                            self.state = ScannerState::WithinContent;
                        } else {
                            self.state = ScannerState::WithinScriptContent;
                        }
                    } else if self.last_tag == "style" {
                        self.state = ScannerState::WithinStyleContent;
                    } else {
                        self.state = ScannerState::WithinContent;
                    }
                    return self.finish_token(offset, TokenType::StartTagClose, None);
                }
                if self.emit_pseudo_close_tags && self.stream.peek_char(0) == LAN {
                    self.state = ScannerState::WithinContent;
                    return self.finish_token(
                        offset,
                        TokenType::StartTagClose,
                        Some("Closing bracket missing.".to_string()),
                    );
                }
                self.stream.advance(1);
                self.finish_token(
                    offset,
                    TokenType::Unknown,
                    Some("Unexpected character in tag.".to_string()),
                )
            }
            ScannerState::AfterAttributeName => {
                if self.stream.skip_whitespace() {
                    self.has_space_after_tag = true;
                    return self.finish_token(offset, TokenType::Whitespace, None);
                }
                if self.stream.advance_if_char(EQS) {
                    self.state = ScannerState::BeforeAttributeValue;
                    return self.finish_token(offset, TokenType::DelimiterAssign, None);
                }
                self.state = ScannerState::WithinTag;
                self.internal_scan()
            }
            ScannerState::BeforeAttributeValue => {
                if self.stream.skip_whitespace() {
                    return self.finish_token(offset, TokenType::Whitespace, None);
                }
                let mut has_value = false;
                if let Some((start, mut end)) = self.stream.advance_if_attr_value() {
                    if self.stream.peek_char(0) == RAN && self.stream.peek_char(-1) == FSL {
                        self.stream.go_back(1);
                        end -= 1;
                    }
                    let attribute_value = self.stream.substring(start, end);
                    if self.last_attribute_name.as_deref() == Some("type") {
                        self.last_type_value = Some(attribute_value);
                    }
                    has_value = end > start;
                }
                if has_value {
                    self.state = ScannerState::WithinTag;
                    self.has_space_after_tag = false;
                    return self.finish_token(offset, TokenType::AttributeValue, None);
                }
                let ch = self.stream.peek_char(0);
                if ch == SQO || ch == DQO {
                    self.stream.advance(1);
                    if self.stream.advance_until_char(ch) {
                        self.stream.advance(1);
                    }
                    if self.last_attribute_name.as_deref() == Some("type") {
                        let value = self
                            .stream
                            .substring(offset + 1, self.stream.pos().wrapping_sub(1));
                        self.last_type_value = Some(value);
                    }
                    self.state = ScannerState::WithinTag;
                    self.has_space_after_tag = false;
                    return self.finish_token(offset, TokenType::AttributeValue, None);
                }
                self.state = ScannerState::WithinTag;
                self.has_space_after_tag = false;
                self.internal_scan()
            }
            ScannerState::WithinScriptContent => {
                let mut script_state = 1;
                while !self.stream.eos() {
                    match self.stream.advance_if_script_tag() {
                        None => {
                            self.stream.go_to_end();
                            return self.finish_token(offset, TokenType::Script, None);
                        }
                        Some(m) => {
                            if m == "<!--" {
                                if script_state == 1 {
                                    script_state = 2;
                                }
                            } else if m == "-->" {
                                script_state = 1;
                            } else if !m.starts_with("</") {
                                if script_state == 2 {
                                    script_state = 3;
                                }
                            } else if script_state == 3 {
                                script_state = 2;
                            } else {
                                let len = m.encode_utf16().count();
                                self.stream.go_back(len);
                                break;
                            }
                        }
                    }
                }
                self.state = ScannerState::WithinContent;
                if offset < self.stream.pos() {
                    return self.finish_token(offset, TokenType::Script, None);
                }
                self.internal_scan()
            }
            ScannerState::WithinStyleContent => {
                self.stream.advance_until_style_end();
                self.state = ScannerState::WithinContent;
                if offset < self.stream.pos() {
                    return self.finish_token(offset, TokenType::Styles, None);
                }
                self.internal_scan()
            }
        }
    }

    pub fn get_token_type(&self) -> TokenType {
        self.token_type
    }

    pub fn get_token_offset(&self) -> usize {
        self.token_offset
    }

    pub fn get_token_length(&self) -> usize {
        self.stream.pos() - self.token_offset
    }

    pub fn get_token_end(&self) -> usize {
        self.stream.pos()
    }

    pub fn get_token_text(&self) -> String {
        self.stream.substring(self.token_offset, self.stream.pos())
    }

    pub fn get_token_error(&self) -> Option<&str> {
        self.token_error.as_deref()
    }

    pub fn get_scanner_state(&self) -> ScannerState {
        self.state
    }
}

/// Port of `convertScriptContentToJavaScript`: converts HTML comment markers in
/// inline script content to JavaScript block comment markers, only replacing
/// matched pairs.
pub fn convert_script_content_to_javascript(content: &str) -> String {
    let units: Vec<u16> = content.encode_utf16().collect();
    let mut result: Vec<u16> = Vec::with_capacity(units.len());
    let mut depth = 0usize;
    let mut i = 0;
    while i < units.len() {
        if i + 4 <= units.len()
            && units[i] == LAN
            && units[i + 1] == BNG
            && units[i + 2] == MIN
            && units[i + 3] == MIN
        {
            depth += 1;
            result.extend_from_slice(&[b'/' as u16, b'*' as u16, b' ' as u16]);
            i += 4;
        } else if i + 3 <= units.len()
            && units[i] == MIN
            && units[i + 1] == MIN
            && units[i + 2] == RAN
        {
            if depth > 0 {
                depth -= 1;
                result.extend_from_slice(&[b' ' as u16, b'*' as u16, b'/' as u16]);
            } else {
                result.extend_from_slice(&[MIN, MIN, RAN]);
            }
            i += 3;
        } else {
            result.push(units[i]);
            i += 1;
        }
    }
    String::from_utf16_lossy(&result)
}
