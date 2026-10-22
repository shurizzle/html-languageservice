//! Port of `src/services/htmlLinks.ts`.

use crate::html_language_types::TokenType;
use crate::language_facts::DataManager;
use crate::parser::html_scanner::create_scanner;
use crate::text_document::TextDocument;
use crate::types::{DocumentContext, DocumentLink, Range};
use crate::uri::Uri;
use crate::utils::strings::{is_js_whitespace, starts_with};

const HASH: u16 = '#' as u16;

fn normalize_ref(url: &str) -> String {
    let units: Vec<u16> = url.encode_utf16().collect();
    if units.len() >= 2 {
        let first = units[0];
        let last = units[units.len() - 1];
        if first == last && (first == '\'' as u16 || first == '"' as u16) {
            return String::from_utf16_lossy(&units[1..units.len() - 1]);
        }
    }
    url.to_string()
}

/// Plain ASCII word char (`\w`).
fn is_word(c: u16) -> bool {
    (c as u8).is_ascii_alphanumeric() || c == '_' as u16
}

/// Port of `validateRef`. The trailing alternation in the upstream regex can
/// match the empty string, so the pattern reduces to `\b[^\s()<>]+`.
fn validate_ref(url: &str, language_id: &str) -> bool {
    if url.is_empty() {
        return false;
    }
    if language_id == "handlebars" && (url.contains("{{") || url.contains("}}")) {
        return false;
    }
    let units: Vec<u16> = url.encode_utf16().collect();
    for (i, &c) in units.iter().enumerate() {
        let ch = char::from_u32(c as u32);
        let excluded = c == '(' as u16
            || c == ')' as u16
            || c == '<' as u16
            || c == '>' as u16
            || ch.is_some_and(is_js_whitespace);
        if excluded {
            continue;
        }
        let prev_word = i > 0 && is_word(units[i - 1]);
        if prev_word != is_word(c) {
            return true;
        }
    }
    false
}

/// Port of `^\s*javascript\:` (case-insensitive).
fn is_javascript_uri(s: &str) -> bool {
    let rest = s.trim_start_matches(is_js_whitespace_char);
    rest.get(..11)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("javascript:"))
}

fn is_js_whitespace_char(c: char) -> bool {
    is_js_whitespace(c)
}

fn get_workspace_url(
    document_uri: &str,
    token_content: &str,
    document_context: &dyn DocumentContext,
    base: Option<&str>,
) -> Option<String> {
    if is_javascript_uri(token_content)
        || token_content.contains('\n')
        || token_content.contains('\r')
    {
        return None;
    }
    let token_content = token_content.trim_start_matches(is_js_whitespace_char);

    // `/^(\w[\w\d+.-]*):/`
    if let Some(match_end) = scheme_match_end(token_content) {
        let schema = token_content[..match_end].to_ascii_lowercase();
        if schema == "http" || schema == "https" || schema == "file" {
            return Some(token_content.to_string());
        }
        return None;
    }
    if token_content.starts_with('#') {
        return Some(format!("{document_uri}{token_content}"));
    }
    if token_content.starts_with("//") {
        let picked_scheme = if starts_with(document_uri, "https://") {
            "https"
        } else {
            "http"
        };
        let stripped = token_content.trim_start_matches(is_js_whitespace_char);
        return Some(format!("{picked_scheme}:{stripped}"));
    }
    document_context.resolve_reference(token_content, base.unwrap_or(document_uri))
}

/// Matches `/^(\w[\w\d+.-]*):/`; returns the length of the scheme (without `:`).
fn scheme_match_end(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let first = *bytes.first()?;
    if !(first.is_ascii_alphanumeric() || first == b'_') {
        return None;
    }
    let mut i = 1;
    while i < bytes.len()
        && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'_' | b'+' | b'.' | b'-'))
    {
        i += 1;
    }
    if bytes.get(i) == Some(&b':') {
        Some(i)
    } else {
        None
    }
}

fn utf16_char_at(s: &str, index: usize) -> Option<u16> {
    s.encode_utf16().nth(index)
}

fn create_link(
    document: &TextDocument,
    document_context: &dyn DocumentContext,
    attribute_value: &str,
    start_offset: usize,
    end_offset: usize,
    base: Option<&str>,
) -> Option<DocumentLink> {
    let token_content = normalize_ref(attribute_value);
    if !validate_ref(&token_content, document.language_id()) {
        return None;
    }
    let (start_offset, end_offset) =
        if token_content.encode_utf16().count() < attribute_value.encode_utf16().count() {
            (start_offset + 1, end_offset - 1)
        } else {
            (start_offset, end_offset)
        };
    let workspace_url = get_workspace_url(document.uri(), &token_content, document_context, base)?;
    let target = validate_and_clean_uri(&workspace_url, document);

    Some(DocumentLink {
        range: Range {
            start: document.position_at(start_offset),
            end: document.position_at(end_offset),
        },
        target,
    })
}

/// Port of `validateAndCleanURI`.
fn validate_and_clean_uri(uri_str: &str, document: &TextDocument) -> Option<String> {
    let mut uri = Uri::parse(uri_str).ok()?;
    let mut uri_str = uri_str.to_string();

    if uri.scheme == "file" && !uri.query.is_empty() {
        uri = uri.with_query(None).ok()?;
        uri_str = uri.to_string_with(true);
    }
    if uri.scheme == "file"
        && !uri.fragment.is_empty()
        && !(starts_with(&uri_str, document.uri())
            && utf16_char_at(&uri_str, document.uri().encode_utf16().count()) == Some(HASH))
    {
        return uri.with_fragment(None).ok().map(|u| u.to_string_with(true));
    }
    Some(uri_str)
}

/// Port of the `HTMLDocumentLinks` class.
pub struct HtmlDocumentLinks<'a> {
    data_manager: &'a DataManager,
}

impl<'a> HtmlDocumentLinks<'a> {
    pub fn new(data_manager: &'a DataManager) -> Self {
        Self { data_manager }
    }

    pub fn find_document_links(
        &self,
        document: &TextDocument,
        document_context: &dyn DocumentContext,
    ) -> Vec<DocumentLink> {
        let mut new_links: Vec<DocumentLink> = Vec::new();

        let mut scanner = create_scanner(
            document.get_scanner_text(),
            0,
            crate::html_language_types::ScannerState::WithinContent,
            false,
        );
        let mut token = scanner.scan();
        let mut last_attribute_name: Option<String> = None;
        let mut last_tag_name: Option<String> = None;
        let mut after_base = false;
        let mut base: Option<String> = None;
        let mut id_locations: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        while token != TokenType::Eos {
            match token {
                TokenType::StartTag => {
                    let tag = scanner.get_token_text().to_lowercase();
                    last_tag_name = Some(tag.clone());
                    if base.is_none() {
                        after_base = tag == "base";
                    }
                }
                TokenType::AttributeName => {
                    last_attribute_name = Some(scanner.get_token_text().to_lowercase());
                }
                TokenType::AttributeValue => {
                    if let (Some(tag), Some(attr)) = (&last_tag_name, &last_attribute_name)
                        && self.data_manager.is_path_attribute(tag, attr)
                    {
                        let attribute_value = scanner.get_token_text();
                        if !after_base {
                            let link = create_link(
                                document,
                                document_context,
                                &attribute_value,
                                scanner.get_token_offset(),
                                scanner.get_token_end(),
                                base.as_deref(),
                            );
                            if let Some(link) = link {
                                new_links.push(link);
                            }
                        }
                        if after_base && base.is_none() {
                            let mut normalized = normalize_ref(&attribute_value);
                            if !normalized.is_empty()
                                && let Some(resolved) =
                                    document_context.resolve_reference(&normalized, document.uri())
                            {
                                normalized = resolved;
                            }
                            if !normalized.is_empty() {
                                base = Some(normalized);
                            }
                        }
                        after_base = false;
                        last_attribute_name = None;
                    } else if last_attribute_name.as_deref() == Some("id") {
                        let id = normalize_ref(&scanner.get_token_text());
                        id_locations.insert(id, scanner.get_token_offset());
                    }
                }
                _ => {}
            }
            token = scanner.scan();
        }

        // change local links with ids to actual positions
        let local_with_hash = format!("{}#", document.uri());
        for link in &mut new_links {
            if let Some(target) = link.target.clone()
                && starts_with(&target, &local_with_hash)
            {
                let id = &target[local_with_hash.len()..];
                match id_locations.get(id) {
                    Some(&offset) => {
                        let pos = document.position_at(offset);
                        link.target = Some(format!(
                            "{local_with_hash}{},{}",
                            pos.line + 1,
                            pos.character + 1
                        ));
                    }
                    None => link.target = Some(document.uri().to_string()),
                }
            }
        }

        new_links
    }
}
