//! Port of `src/services/htmlFormatter.ts` (`format`).

use regex::Regex;
use std::sync::OnceLock;

use crate::beautify::html::html_beautify;
use crate::beautify::options::{OptValue, RawOptions};
use crate::text_document::TextDocument;
use crate::types::{EmbeddedCSSFormatConfiguration, HTMLFormatConfiguration, Position, Range, TextEdit, TemplatingFormatOption};

/// Port of `format(document, range, options)`.
pub fn format(
    document: &TextDocument,
    range: Option<Range>,
    options: &HTMLFormatConfiguration,
) -> Vec<TextEdit> {
    let mut value = document.get_text().to_string();
    let mut includes_end = true;
    let mut initial_indent_level = 0i64;
    let tab_size = options.tab_size.unwrap_or(4) as i64;
    let mut range = range;

    if let Some(r) = range {
        let mut start_offset = document.offset_at(r.start);
        let mut end_offset = document.offset_at(r.end);

        if let Some((s, e)) = expand_range_to_ignore_block(&value, start_offset, end_offset) {
            start_offset = s;
            end_offset = e;
        }

        let mut extended_start = start_offset;
        while extended_start > 0 && is_whitespace(&value, extended_start - 1) {
            extended_start -= 1;
        }
        if extended_start == 0 || is_eol(&value, extended_start - 1) {
            start_offset = extended_start;
        } else if extended_start < start_offset {
            start_offset = extended_start + 1;
        }

        let mut extended_end = end_offset;
        while extended_end < value.len() && is_whitespace(&value, extended_end) {
            extended_end += 1;
        }
        if extended_end == value.len() || is_eol(&value, extended_end) {
            end_offset = extended_end;
        }

        let start_position = document.position_at(start_offset);
        let end_position = document.position_at(end_offset);
        range = Some(Range {
            start: start_position,
            end: end_position,
        });

        let first_half = &value[..start_offset];
        if ends_inside_tag(first_half) {
            let substring = value[start_offset..end_offset].to_string();
            return vec![TextEdit::replace(
                Range {
                    start: start_position,
                    end: end_position,
                },
                substring,
            )];
        }

        includes_end = end_offset == value.len();
        value = value[start_offset..end_offset].to_string();

        if start_offset != 0 {
            let start_of_line_offset =
                document.offset_at(Position::create(range.unwrap().start.line, 0));
            initial_indent_level = compute_indent_level(document.get_text(), start_of_line_offset, options);
        }
    } else {
        let end = document.position_at(value.len());
        range = Some(Range {
            start: Position::create(0, 0),
            end,
        });
    }

    let html_options = build_options(options, tab_size, includes_end);
    let mut result = html_beautify(value.trim_start(), &html_options);

    if initial_indent_level > 0 {
        let indent = if options.insert_spaces.unwrap_or(false) {
            " ".repeat((tab_size * initial_indent_level) as usize)
        } else {
            "\t".repeat(initial_indent_level as usize)
        };
        result = result.split('\n').collect::<Vec<_>>().join(&format!("\n{indent}"));
        if range.unwrap().start.character == 0 {
            result = format!("{indent}{result}");
        }
    }

    if result == value {
        return Vec::new();
    }

    let document_end = document.position_at(document.get_text().len());
    let r = range.unwrap();
    if r.end.line > document_end.line
        || (r.end.line == document_end.line && r.end.character > document_end.character)
    {
        range = Some(Range {
            start: r.start,
            end: document_end,
        });
    }

    vec![TextEdit::replace(range.unwrap(), result)]
}

fn build_options(
    options: &HTMLFormatConfiguration,
    tab_size: i64,
    includes_end: bool,
) -> RawOptions {
    let mut html: RawOptions = RawOptions::new();
    html.insert("indent_size".to_string(), OptValue::Number(tab_size as f64));
    html.insert(
        "indent_char".to_string(),
        OptValue::Str(if options.insert_spaces.unwrap_or(false) {
            " ".to_string()
        } else {
            "\t".to_string()
        }),
    );
    html.insert(
        "indent_empty_lines".to_string(),
        OptValue::Bool(get_bool(options.indent_empty_lines, false)),
    );
    html.insert(
        "wrap_line_length".to_string(),
        OptValue::Number(get_num(options.wrap_line_length, 120) as f64),
    );

    if let Some(v) = get_tags_format_option(&options.unformatted) {
        html.insert(
            "unformatted".to_string(),
            OptValue::List(v.into_iter().map(OptValue::Str).collect()),
        );
    }
    if let Some(v) = get_tags_format_option(&options.content_unformatted) {
        html.insert(
            "content_unformatted".to_string(),
            OptValue::List(v.into_iter().map(OptValue::Str).collect()),
        );
    }

    html.insert(
        "indent_inner_html".to_string(),
        OptValue::Bool(get_bool(options.indent_inner_html, false)),
    );
    html.insert(
        "preserve_newlines".to_string(),
        OptValue::Bool(get_bool(options.preserve_new_lines, true)),
    );
    html.insert(
        "max_preserve_newlines".to_string(),
        OptValue::Number(get_num(options.max_preserve_new_lines, 32786) as f64),
    );
    html.insert(
        "indent_handlebars".to_string(),
        OptValue::Bool(get_bool(options.indent_handlebars, false)),
    );
    html.insert(
        "end_with_newline".to_string(),
        OptValue::Bool(includes_end && get_bool(options.end_with_newline, false)),
    );

    if let Some(v) = get_tags_format_option(&options.extra_liners) {
        html.insert(
            "extra_liners".to_string(),
            OptValue::List(v.into_iter().map(OptValue::Str).collect()),
        );
    }

    html.insert(
        "wrap_attributes".to_string(),
        OptValue::Str(
            options
                .wrap_attributes
                .clone()
                .unwrap_or_else(|| "auto".to_string()),
        ),
    );
    if let Some(v) = options.wrap_attributes_indent_size {
        html.insert(
            "wrap_attributes_indent_size".to_string(),
            OptValue::Number(v as f64),
        );
    }

    html.insert("eol".to_string(), OptValue::Str("\n".to_string()));
    html.insert(
        "indent_scripts".to_string(),
        OptValue::Str(
            options
                .indent_scripts
                .clone()
                .unwrap_or_else(|| "normal".to_string()),
        ),
    );
    html.insert(
        "templating".to_string(),
        OptValue::List(
            get_templating_format_option(options)
                .into_iter()
                .map(OptValue::Str)
                .collect(),
        ),
    );
    html.insert(
        "unformatted_content_delimiter".to_string(),
        OptValue::Str(options.unformatted_content_delimiter.clone().unwrap_or_default()),
    );

    if let Some(css) = &options.css {
        html.insert("css".to_string(), OptValue::Map(css_options(css)));
    }

    html
}

fn css_options(css: &EmbeddedCSSFormatConfiguration) -> RawOptions {
    let mut raw = RawOptions::new();
    if let Some(v) = css.newline_between_selectors {
        raw.insert("selector_separator_newline".to_string(), OptValue::Bool(v));
    }
    if let Some(v) = css.newline_between_rules {
        raw.insert("newline_between_rules".to_string(), OptValue::Bool(v));
    }
    if let Some(v) = css.space_around_selector_separator {
        raw.insert(
            "space_around_selector_separator".to_string(),
            OptValue::Bool(v),
        );
    }
    if let Some(v) = &css.brace_style {
        raw.insert("brace_style".to_string(), OptValue::Str(v.clone()));
    }
    if let Some(v) = css.preserve_new_lines {
        raw.insert("preserve_newlines".to_string(), OptValue::Bool(v));
    }
    if let Some(v) = css.max_preserve_new_lines {
        raw.insert("max_preserve_newlines".to_string(), OptValue::Number(v as f64));
    }
    raw
}

fn get_bool(value: Option<bool>, default: bool) -> bool {
    value.unwrap_or(default)
}

fn get_num(value: Option<i32>, default: i32) -> i32 {
    value.unwrap_or(default)
}

/// Port of `getTagsFormatOption`.
fn get_tags_format_option(value: &Option<String>) -> Option<Vec<String>> {
    let list = value.as_ref()?;
    if list.is_empty() {
        return Some(Vec::new());
    }
    Some(
        list.split(',')
            .map(|t| t.trim().to_lowercase())
            .collect(),
    )
}

/// Port of `getTemplatingFormatOption(options, 'all')`.
fn get_templating_format_option(options: &HTMLFormatConfiguration) -> Vec<String> {
    match &options.templating {
        Some(TemplatingFormatOption::Bool(true)) => vec!["auto".to_string()],
        Some(TemplatingFormatOption::Bool(false)) | None => vec!["none".to_string()],
        Some(TemplatingFormatOption::List(list)) => list.clone(),
    }
}

/// Port of `computeIndentLevel`.
fn compute_indent_level(content: &str, offset: usize, options: &HTMLFormatConfiguration) -> i64 {
    let tab_size = options.tab_size.unwrap_or(4) as i64;
    let mut n_chars = 0i64;
    for ch in content[offset..].chars() {
        if ch == ' ' {
            n_chars += 1;
        } else if ch == '\t' {
            n_chars += tab_size;
        } else {
            break;
        }
    }
    n_chars / tab_size
}

fn is_eol(text: &str, offset: usize) -> bool {
    matches!(text.as_bytes().get(offset), Some(b'\r') | Some(b'\n'))
}

fn is_whitespace(text: &str, offset: usize) -> bool {
    matches!(text.as_bytes().get(offset), Some(b' ') | Some(b'\t'))
}

/// Port of `.*[<][^>]*$` over `first_half`: the last `<` has no `>` after it.
fn ends_inside_tag(text: &str) -> bool {
    match text.rfind('<') {
        None => false,
        Some(lt) => match text.rfind('>') {
            None => true,
            Some(gt) => lt > gt,
        },
    }
}

fn ignore_start_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("<!--\\s*beautify\\s+ignore:start\\s*-->").unwrap())
}

fn ignore_end_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("<!--\\s*beautify\\s+ignore:end\\s*-->").unwrap())
}

/// Port of `expandRangeToIgnoreBlock`.
fn expand_range_to_ignore_block(
    text: &str,
    start_offset: usize,
    end_offset: usize,
) -> Option<(usize, usize)> {
    let mut last_ignore_start: Option<(usize, usize)> = None;
    for m in ignore_start_pattern().find_iter(text) {
        if m.start() > start_offset {
            break;
        }
        last_ignore_start = Some((m.start(), m.end()));
    }
    let (start, start_end) = last_ignore_start?;

    let ignore_end = ignore_end_pattern().find_at(text, start_end)?;
    let block_end = ignore_end.end();

    if start_offset >= start && end_offset <= block_end {
        Some((start, block_end))
    } else {
        None
    }
}
