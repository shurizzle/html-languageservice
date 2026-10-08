//! Shared helpers for the integration tests.
#![allow(dead_code)]

use html_languageservice::*;

/// Creates a document with the given content (version 0).
pub fn doc(uri: &str, language_id: &str, content: &str) -> TextDocument {
    TextDocument::create(uri, language_id, 0, content)
}

/// Replaces the `|` marker in `value` with nothing and returns
/// `(cleaned, utf16_offset_at_marker)`.
pub fn split_marker(value: &str) -> (String, usize) {
    let byte = value.find('|').expect("marker '|' not found");
    let offset = value[..byte].encode_utf16().count();
    let cleaned = format!("{}{}", &value[..byte], &value[byte + 1..]);
    (cleaned, offset)
}

/// Test [`DocumentContext`] resolving references like Node's `url.resolve`
/// (RFC 3986 section 5.2).
pub struct TestDocumentContext;

impl DocumentContext for TestDocumentContext {
    fn resolve_reference(&self, reference: &str, base: &str) -> Option<String> {
        Some(resolve_url(base, reference))
    }
}

type Parts = (
    Option<String>,
    Option<String>,
    String,
    Option<String>,
    Option<String>,
);

fn split_uri(s: &str) -> Parts {
    let bytes = s.as_bytes();
    let mut i = 0;

    let mut scheme = None;
    if let Some(colon) = s.find(':')
        && !s[..colon].contains(['/', '?', '#'])
    {
        scheme = Some(s[..colon].to_string());
        i = colon + 1;
    }

    let mut authority = None;
    if bytes[i..].starts_with(b"//") {
        let start = i + 2;
        let end = s[start..]
            .find(['/', '?', '#'])
            .map(|p| start + p)
            .unwrap_or(s.len());
        authority = Some(s[start..end].to_string());
        i = end;
    }

    let path_end = s[i..].find(['?', '#']).map(|p| i + p).unwrap_or(s.len());
    let path = s[i..path_end].to_string();
    i = path_end;

    let mut query = None;
    if bytes.get(i) == Some(&b'?') {
        let start = i + 1;
        let end = s[start..].find('#').map(|p| start + p).unwrap_or(s.len());
        query = Some(s[start..end].to_string());
        i = end;
    }

    let mut fragment = None;
    if bytes.get(i) == Some(&b'#') {
        fragment = Some(s[i + 1..].to_string());
    }

    (scheme, authority, path, query, fragment)
}

fn merge(base: &Parts, reference_path: &str) -> String {
    if base.1.is_some() && base.2.is_empty() {
        return format!("/{reference_path}");
    }
    match base.2.rfind('/') {
        Some(idx) => format!("{}{}", &base.2[..idx + 1], reference_path),
        None => reference_path.to_string(),
    }
}

fn remove_dot_segments(path: &str) -> String {
    let mut input = path.to_string();
    let mut output = String::new();
    while !input.is_empty() {
        if let Some(rest) = input.strip_prefix("../") {
            input = rest.to_string();
        } else if let Some(rest) = input.strip_prefix("./") {
            input = rest.to_string();
        } else if let Some(rest) = input.strip_prefix("/./") {
            input = format!("/{rest}");
        } else if input == "/." {
            input = "/".to_string();
        } else if let Some(rest) = input.strip_prefix("/../") {
            input = format!("/{rest}");
            output = truncate_last_segment(&output);
        } else if input == "/.." {
            input = "/".to_string();
            output = truncate_last_segment(&output);
        } else if input == "." || input == ".." {
            input.clear();
        } else {
            let end = if let Some(rest) = input.strip_prefix('/') {
                rest.find('/').map(|p| p + 1).unwrap_or(input.len())
            } else {
                input.find('/').unwrap_or(input.len())
            };
            output.push_str(&input[..end]);
            input = input[end..].to_string();
        }
    }
    output
}

fn truncate_last_segment(output: &str) -> String {
    match output.rfind('/') {
        Some(idx) => output[..idx].to_string(),
        None => String::new(),
    }
}

fn rebuild(parts: &Parts) -> String {
    let mut res = String::new();
    if let Some(scheme) = &parts.0 {
        res.push_str(scheme);
        res.push(':');
    }
    if let Some(authority) = &parts.1 {
        res.push_str("//");
        res.push_str(authority);
    }
    res.push_str(&parts.2);
    if let Some(query) = &parts.3 {
        res.push('?');
        res.push_str(query);
    }
    if let Some(fragment) = &parts.4 {
        res.push('#');
        res.push_str(fragment);
    }
    res
}

fn resolve_url(base: &str, reference: &str) -> String {
    let base = split_uri(base);
    let reference = split_uri(reference);
    let (scheme, authority, path, query) = if reference.0.is_some() {
        (
            reference.0.clone(),
            reference.1.clone(),
            remove_dot_segments(&reference.2),
            reference.3.clone(),
        )
    } else if reference.1.is_some() {
        (
            base.0.clone(),
            reference.1.clone(),
            remove_dot_segments(&reference.2),
            reference.3.clone(),
        )
    } else if reference.2.is_empty() {
        (
            base.0.clone(),
            base.1.clone(),
            base.2.clone(),
            reference.3.clone().or_else(|| base.3.clone()),
        )
    } else {
        let path = if reference.2.starts_with('/') {
            remove_dot_segments(&reference.2)
        } else {
            remove_dot_segments(&merge(&base, &reference.2))
        };
        (base.0.clone(), base.1.clone(), path, reference.3.clone())
    };
    rebuild(&(scheme, authority, path, query, reference.4))
}
