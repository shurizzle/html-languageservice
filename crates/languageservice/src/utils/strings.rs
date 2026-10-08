//! Port of `src/utils/strings.ts`.

pub fn starts_with(haystack: &str, needle: &str) -> bool {
    haystack.starts_with(needle)
}

/// Determines if `haystack` ends with `needle`.
pub fn ends_with(haystack: &str, needle: &str) -> bool {
    haystack.ends_with(needle)
}

/// Returns the length (in UTF-16 code units) of the common prefix of the two strings.
pub fn common_prefix_length(a: &str, b: &str) -> usize {
    let mut len = 0usize;
    let mut ai = a.encode_utf16();
    let mut bi = b.encode_utf16();
    loop {
        match (ai.next(), bi.next()) {
            (Some(x), Some(y)) if x == y => len += 1,
            _ => return len,
        }
    }
}

pub fn repeat(value: &str, mut count: usize) -> String {
    let mut s = String::new();
    let mut value = value.to_string();
    while count > 0 {
        if count & 1 == 1 {
            s.push_str(&value);
        }
        value.push_str(&value.clone());
        count >>= 1;
    }
    s
}

pub fn is_letter_or_digit(text: &str, index: usize) -> bool {
    let c = text.as_bytes().get(index).copied().unwrap_or(0);
    c.is_ascii_alphanumeric()
}

/// Port of `isLetterOrDigit` for a UTF-16 code unit index.
pub fn is_letter_or_digit_unit(units: &[u16], index: usize) -> bool {
    match units.get(index) {
        Some(&c) => {
            (b'a' as u16..=b'z' as u16).contains(&c)
                || (b'A' as u16..=b'Z' as u16).contains(&c)
                || (b'0' as u16..=b'9' as u16).contains(&c)
        }
        None => false,
    }
}

/// JavaScript `\s` character class.
pub fn is_js_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{0009}'..='\u{000D}'
            | '\u{0020}'
            | '\u{00A0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200A}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
            | '\u{FEFF}'
    )
}

/// Port of `string.split(/\s+/)`: runs of `\s` act as separators and empty
/// leading/trailing segments are preserved.
pub fn split_whitespace_runs(s: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut last = 0usize;
    let mut i = 0usize;
    while i < s.len() {
        let ch = s[i..].chars().next().unwrap();
        if is_js_whitespace(ch) {
            result.push(&s[last..i]);
            let mut j = i;
            while j < s.len() {
                let c = s[j..].chars().next().unwrap();
                if !is_js_whitespace(c) {
                    break;
                }
                j += c.len_utf8();
            }
            i = j;
            last = j;
        } else {
            i += ch.len_utf8();
        }
    }
    result.push(&s[last..]);
    result
}

/// Port of `string.replace(/["']/g, '')`.
pub fn strip_quotes(s: &str) -> String {
    s.chars().filter(|c| *c != '"' && *c != '\'').collect()
}
