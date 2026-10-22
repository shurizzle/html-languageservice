//! Port of the js-beautify option handling (`Options`, `_mergeOpts`, `_normalizeOpts`).

use std::collections::BTreeMap;
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

/// Logs a diagnostic once per process, so repeated format requests do not
/// flood stderr with the same warning.
pub(crate) fn warn_once(message: String) {
    static SEEN: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let seen = SEEN.get_or_init(|| Mutex::new(HashSet::new()));
    let Ok(mut seen) = seen.lock() else {
        return;
    };
    if seen.insert(message.clone()) {
        eprintln!("html-languageservice: {message}");
    }
}

/// A value of the js-beautify options object.
#[derive(Debug, Clone, PartialEq)]
pub enum OptValue {
    Bool(bool),
    Number(f64),
    Str(String),
    List(Vec<OptValue>),
    Map(RawOptions),
}

/// The (normalized) raw options object.
pub type RawOptions = BTreeMap<String, OptValue>;

impl OptValue {
    /// JavaScript truthiness (`!!value`).
    pub fn truthy(&self) -> bool {
        match self {
            OptValue::Bool(b) => *b,
            OptValue::Number(n) => *n != 0.0 && !n.is_nan(),
            OptValue::Str(s) => !s.is_empty(),
            OptValue::List(_) | OptValue::Map(_) => true,
        }
    }

    /// Port of `parseInt(value, 10)`.
    fn as_int(&self) -> Option<i64> {
        match self {
            OptValue::Number(n) => {
                if n.is_nan() || n.is_infinite() {
                    None
                } else {
                    Some(n.trunc() as i64)
                }
            }
            OptValue::Str(s) => parse_int_prefix(s),
            _ => None,
        }
    }
}

fn parse_int_prefix(s: &str) -> Option<i64> {
    let s = s.trim_start();
    let bytes = s.as_bytes();
    let mut end = 0;
    if end < bytes.len() && (bytes[end] == b'-' || bytes[end] == b'+') {
        end += 1;
    }
    let digits_start = end;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    if end == digits_start {
        return None;
    }
    s[..end].parse::<i64>().ok()
}

/// Port of `_normalizeOpts`.
fn normalize_opts(options: &RawOptions) -> RawOptions {
    let mut converted = RawOptions::new();
    for (key, value) in options {
        converted.insert(key.replace('-', "_"), value.clone());
    }
    converted
}

/// Port of `_mergeOpts`.
pub fn merge_opts(all_options: &RawOptions, child_field_name: Option<&str>) -> RawOptions {
    let all_options = normalize_opts(all_options);
    let mut final_opts = RawOptions::new();

    for (name, value) in &all_options {
        if Some(name.as_str()) != child_field_name {
            final_opts.insert(name.clone(), value.clone());
        }
    }

    if let Some(child) = child_field_name
        && let Some(OptValue::Map(child_opts)) = all_options.get(child)
    {
        for (name, value) in child_opts {
            final_opts.insert(name.clone(), value.clone());
        }
    }
    final_opts
}

/// Split a string on `/[^a-zA-Z0-9_\/\-]+/`.
fn split_on_non_word(s: &str) -> Vec<OptValue> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut last_was_sep = false;
    for ch in s.chars() {
        let is_word = ch.is_ascii_alphanumeric() || ch == '_' || ch == '/' || ch == '-';
        if is_word {
            current.push(ch);
            last_was_sep = false;
        } else {
            result.push(OptValue::Str(std::mem::take(&mut current)));
            last_was_sep = true;
        }
    }
    if last_was_sep {
        result.push(OptValue::Str(String::new()));
    } else {
        result.push(OptValue::Str(current));
    }
    result
}

/// Replace the first occurrence of `from` with `to`.
fn replace_first(s: &str, from: &str, to: &str) -> String {
    match s.find(from) {
        Some(i) => format!("{}{}{}", &s[..i], to, &s[i + from.len()..]),
        None => s.to_string(),
    }
}

/// Port of `_get_boolean`.
pub fn get_boolean(raw: &RawOptions, name: &str, default_value: bool) -> bool {
    match raw.get(name) {
        None => default_value,
        Some(value) => value.truthy(),
    }
}

/// Port of `_get_characters`.
pub fn get_characters(raw: &RawOptions, name: &str, default_value: &str) -> String {
    let mut result = default_value.to_string();
    if let Some(OptValue::Str(s)) = raw.get(name) {
        result = replace_first(s, "\\r", "\r");
        result = replace_first(&result, "\\n", "\n");
        result = replace_first(&result, "\\t", "\t");
    }
    result
}

/// Port of `_get_number`.
pub fn get_number(raw: &RawOptions, name: &str, default_value: Option<i32>) -> i32 {
    let default_value = default_value.unwrap_or(0);
    match raw.get(name).and_then(|v| v.as_int()) {
        Some(n) => n as i32,
        None => default_value,
    }
}

/// Port of `_get_array`.
pub fn get_array(raw: &RawOptions, name: &str, default_value: Vec<OptValue>) -> Vec<OptValue> {
    let mut result = default_value;
    match raw.get(name) {
        Some(OptValue::List(items)) => result = items.clone(),
        Some(OptValue::Str(s)) => result = split_on_non_word(s),
        _ => {}
    }
    result
}

/// Port of `_get_array` restricted to string entries.
pub fn get_string_array(raw: &RawOptions, name: &str, default: &[&str]) -> Vec<String> {
    let values = get_array(
        raw,
        name,
        default
            .iter()
            .map(|s| OptValue::Str(s.to_string()))
            .collect(),
    );
    values
        .iter()
        .filter_map(|v| match v {
            OptValue::Str(s) => Some(s.clone()),
            _ => None,
        })
        .collect()
}

/// Port of `_is_valid_selection` + `_get_selection_list`.
///
/// Invalid values are dropped; when nothing valid remains the default is used
/// instead of throwing like js-beautify does.
pub fn get_selection_list(
    raw: &RawOptions,
    name: &str,
    selection_list: &[&str],
    default_value: Option<Vec<&str>>,
) -> Vec<String> {
    let default_value: Vec<String> = default_value
        .map(|values| values.into_iter().map(str::to_string).collect())
        .unwrap_or_else(|| {
            selection_list
                .first()
                .map(|value| vec![(*value).to_string()])
                .unwrap_or_default()
        });
    let raw_values = get_string_array(raw, name, &[]);
    let invalid: Vec<String> = raw_values
        .iter()
        .filter(|item| !selection_list.contains(&item.as_str()))
        .cloned()
        .collect();
    let result: Vec<String> = raw_values
        .into_iter()
        .filter(|item| selection_list.contains(&item.as_str()))
        .collect();
    if !invalid.is_empty() {
        warn_once(format!(
            "ignoring invalid value(s) for option `{name}`: {} (allowed: {})",
            invalid.join(", "),
            selection_list.join(", ")
        ));
    }
    if result.is_empty() {
        if !invalid.is_empty() {
            warn_once(format!(
                "using default for option `{name}`: {}",
                default_value.join(", ")
            ));
        }
        default_value
    } else {
        result
    }
}

/// Port of `_get_selection`. Multiple valid values keep the first one.
pub fn get_selection(
    raw: &RawOptions,
    name: &str,
    selection_list: &[&str],
    default_value: Option<Vec<&str>>,
) -> String {
    let mut result = get_selection_list(raw, name, selection_list, default_value);
    if result.len() > 1 {
        warn_once(format!(
            "option `{name}` accepts a single value, using `{}`",
            result[0]
        ));
        result.truncate(1);
    }
    result.into_iter().next().unwrap_or_default()
}

/// Port of the base js-beautify `Options` class.
#[derive(Debug, Clone)]
pub struct BaseOptions {
    pub raw_options: RawOptions,
    pub disabled: bool,
    pub eol: String,
    pub end_with_newline: bool,
    pub indent_size: i32,
    pub indent_char: String,
    pub indent_level: i32,
    pub preserve_newlines: bool,
    pub max_preserve_newlines: i32,
    pub indent_with_tabs: bool,
    pub wrap_line_length: i32,
    pub indent_empty_lines: bool,
    pub templating: Vec<String>,
}

impl BaseOptions {
    pub fn new(options: &RawOptions, merge_child_field: Option<&str>) -> Self {
        let raw_options = merge_opts(options, merge_child_field);

        let preserve_newlines = get_boolean(&raw_options, "preserve_newlines", true);
        let mut max_preserve_newlines =
            get_number(&raw_options, "max_preserve_newlines", Some(32786));
        if !preserve_newlines {
            max_preserve_newlines = 0;
        }

        let mut indent_char = get_characters(&raw_options, "indent_char", " ");
        let mut indent_size = get_number(&raw_options, "indent_size", Some(4));
        let indent_with_tabs = get_boolean(&raw_options, "indent_with_tabs", indent_char == "\t");
        if indent_with_tabs {
            indent_char = "\t".to_string();
            if indent_size == 1 {
                indent_size = 4;
            }
        }

        let max_char = get_number(&raw_options, "max_char", None);
        let wrap_line_length = get_number(&raw_options, "wrap_line_length", Some(max_char));

        let templating = get_selection_list(
            &raw_options,
            "templating",
            &[
                "auto",
                "none",
                "angular",
                "django",
                "erb",
                "handlebars",
                "php",
                "smarty",
            ],
            Some(vec!["auto"]),
        );

        Self {
            disabled: get_boolean(&raw_options, "disabled", false),
            eol: get_characters(&raw_options, "eol", "auto"),
            end_with_newline: get_boolean(&raw_options, "end_with_newline", false),
            indent_size,
            indent_char,
            indent_level: get_number(&raw_options, "indent_level", None),
            preserve_newlines,
            max_preserve_newlines,
            indent_with_tabs,
            wrap_line_length,
            indent_empty_lines: get_boolean(&raw_options, "indent_empty_lines", false),
            templating,
            raw_options,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(name: &str, value: OptValue) -> RawOptions {
        let mut options = RawOptions::new();
        options.insert(name.to_string(), value);
        options
    }

    #[test]
    fn invalid_selection_falls_back_to_the_first_default() {
        let options = raw("wrap_attributes", OptValue::Str("banana".to_string()));
        assert_eq!(
            get_selection(&options, "wrap_attributes", &["auto", "force"], None),
            "auto"
        );
    }

    #[test]
    fn invalid_selection_list_values_are_dropped() {
        let options = raw("templating", OptValue::Str("django,xxx".to_string()));
        assert_eq!(
            get_selection_list(
                &options,
                "templating",
                &["auto", "django"],
                Some(vec!["auto"])
            ),
            vec!["django"]
        );
    }

    #[test]
    fn empty_selection_list_uses_the_default() {
        let options = raw("templating", OptValue::Str(String::new()));
        assert_eq!(
            get_selection_list(&options, "templating", &["auto"], Some(vec!["auto"])),
            vec!["auto"]
        );
    }

    #[test]
    fn multiple_valid_selections_keep_the_first() {
        let options = raw("wrap_attributes", OptValue::Str("force,expand".to_string()));
        assert_eq!(
            get_selection(
                &options,
                "wrap_attributes",
                &["auto", "force", "expand"],
                None
            ),
            "force"
        );
    }
}
