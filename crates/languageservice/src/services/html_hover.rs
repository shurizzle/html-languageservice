//! Port of `src/services/htmlHover.ts`.

use crate::html_language_types::{ScannerState, TokenType};
use crate::language_facts::DataManager;
use crate::language_facts::data_provider::generate_documentation;
use crate::parser::html_entities::entities;
use crate::parser::html_parser::HtmlDocument;
use crate::parser::html_scanner::create_scanner;
use crate::text_document::TextDocument;
use crate::types::{
    ClientCapabilities, Hover, HoverSettings, IHtmlDataProvider, MarkupContent, MarkupInput,
    MarkupKind, Position, Range,
};
use crate::utils::strings::is_letter_or_digit_unit;

pub struct HtmlHover<'a> {
    supports_markdown: bool,
    data_manager: &'a DataManager,
}

impl<'a> HtmlHover<'a> {
    pub fn new(
        client_capabilities: Option<&ClientCapabilities>,
        data_manager: &'a DataManager,
    ) -> Self {
        let supports_markdown = match client_capabilities {
            None => true,
            Some(caps) => caps
                .text_document
                .hover
                .content_format
                .contains(&MarkupKind::Markdown),
        };
        Self {
            supports_markdown,
            data_manager,
        }
    }

    pub fn do_hover(
        &self,
        document: &TextDocument,
        position: Position,
        html_document: &HtmlDocument,
        options: Option<&HoverSettings>,
    ) -> Option<Hover> {
        let does_support_markdown = self.supports_markdown;

        let offset = document.offset_at(position);
        let node = html_document.node(html_document.find_node_at(offset));
        let text = document.get_text();
        let units: Vec<u16> = text.encode_utf16().collect();
        let node_tag = node.tag.clone()?;
        let data_providers: Vec<&dyn IHtmlDataProvider> = self
            .data_manager
            .get_data_providers()
            .into_iter()
            .filter(|p| p.is_applicable(document.language_id()))
            .collect();

        let get_tag_hover = |curr_tag: &str, range: Range| -> Option<Hover> {
            for provider in &data_providers {
                let mut hover: Option<Hover> = None;
                for tag in provider.provide_tags() {
                    if tag.name.to_lowercase() == curr_tag.to_lowercase() {
                        let markup_content =
                            generate_documentation(tag, options, does_support_markdown)
                                .unwrap_or_else(|| {
                                    MarkupContent::new(
                                        if does_support_markdown {
                                            MarkupKind::Markdown
                                        } else {
                                            MarkupKind::PlainText
                                        },
                                        "",
                                    )
                                });
                        hover = Some(Hover {
                            contents: MarkupInput::Markup(markup_content),
                            range: Some(range),
                        });
                    }
                }
                if let Some(h) = hover {
                    return Some(Hover {
                        contents: self.convert_contents(h.contents),
                        range: h.range,
                    });
                }
            }
            None
        };

        let get_attr_hover = |curr_tag: &str, curr_attr: &str, range: Range| -> Option<Hover> {
            for provider in &data_providers {
                let mut hover: Option<Hover> = None;
                for attr in provider.provide_attributes(curr_tag) {
                    if curr_attr == attr.name && attr.description.is_some() {
                        match generate_documentation(&attr, options, does_support_markdown) {
                            Some(contents_doc) => {
                                hover = Some(Hover {
                                    contents: MarkupInput::Markup(contents_doc),
                                    range: Some(range),
                                });
                            }
                            None => hover = None,
                        }
                    }
                }
                if let Some(h) = hover {
                    return Some(Hover {
                        contents: self.convert_contents(h.contents),
                        range: h.range,
                    });
                }
            }
            None
        };

        let get_attr_value_hover = |curr_tag: &str,
                                    curr_attr: &str,
                                    curr_attr_value: &str,
                                    range: Range|
         -> Option<Hover> {
            for provider in &data_providers {
                let mut hover: Option<Hover> = None;
                for attr_value in provider.provide_values(curr_tag, curr_attr) {
                    if curr_attr_value == attr_value.name && attr_value.description.is_some() {
                        match generate_documentation(&attr_value, options, does_support_markdown) {
                            Some(contents_doc) => {
                                hover = Some(Hover {
                                    contents: MarkupInput::Markup(contents_doc),
                                    range: Some(range),
                                });
                            }
                            None => hover = None,
                        }
                    }
                }
                if let Some(h) = hover {
                    return Some(Hover {
                        contents: self.convert_contents(h.contents),
                        range: h.range,
                    });
                }
            }
            None
        };

        let get_entity_hover = |range: Range| -> Option<Hover> {
            let curr_entity = filter_entity(&units, offset);
            for (entity, replacement) in entities() {
                let label = format!("&{entity}");
                if curr_entity == label {
                    let code_unit = replacement.encode_utf16().next().unwrap_or(0);
                    let code = format!("{code_unit:X}");
                    let mut hex = String::from("U+");
                    for _ in 0..4usize.saturating_sub(code.len()) {
                        hex.push('0');
                    }
                    hex.push_str(&code);
                    let contents_doc = format!(
                        "Character entity representing '{replacement}', unicode equivalent '{hex}'"
                    );
                    return Some(Hover {
                        contents: self.convert_contents(MarkupInput::String(contents_doc)),
                        range: Some(range),
                    });
                }
            }
            None
        };

        let get_tag_name_range = |token_type: TokenType, start_offset: usize| -> Option<Range> {
            let mut scanner =
                create_scanner(text, start_offset, ScannerState::WithinContent, false);
            let mut token = scanner.scan();
            while token != TokenType::Eos
                && (scanner.get_token_end() < offset
                    || (scanner.get_token_end() == offset && token != token_type))
            {
                token = scanner.scan();
            }
            if token == token_type && offset <= scanner.get_token_end() {
                return Some(Range {
                    start: document.position_at(scanner.get_token_offset()),
                    end: document.position_at(scanner.get_token_end()),
                });
            }
            None
        };

        let get_entity_range = || -> Option<Range> {
            let mut k = offset as i64 - 1;
            let mut character_start = position.character as i64;

            while k >= 0 && is_letter_or_digit_unit(&units, k as usize) {
                k -= 1;
                character_start -= 1;
            }

            let mut n = k + 1;
            let mut character_end = character_start;

            while is_letter_or_digit_unit(&units, n as usize) {
                n += 1;
                character_end += 1;
            }

            if k >= 0 && units.get(k as usize) == Some(&(b'&' as u16)) {
                let start = Position::create(position.line, character_start as u32);
                let end = if units.get(n as usize) == Some(&(b';' as u16)) {
                    Position::create(position.line, character_end as u32 + 1)
                } else {
                    Position::create(position.line, character_end as u32)
                };
                return Some(Range { start, end });
            }

            None
        };

        if let Some(end_tag_start) = node.end_tag_start
            && offset >= end_tag_start
        {
            let tag_range = get_tag_name_range(TokenType::EndTag, end_tag_start);
            return match tag_range {
                Some(range) => get_tag_hover(&node_tag, range),
                None => None,
            };
        }

        let tag_range = get_tag_name_range(TokenType::StartTag, node.start);
        if let Some(range) = tag_range {
            return get_tag_hover(&node_tag, range);
        }

        let attr_range = get_tag_name_range(TokenType::AttributeName, node.start);
        if let Some(range) = attr_range {
            let attr = document.get_text_range(range);
            return get_attr_hover(&node_tag, &attr, range);
        }

        let entity_range = get_entity_range();
        if let Some(range) = entity_range {
            return get_entity_hover(range);
        }

        let scan_attr_and_attr_value = |node_start: usize,
                                        attr_value_start: usize|
         -> Option<String> {
            let mut scanner = create_scanner(text, node_start, ScannerState::WithinContent, false);
            let mut token = scanner.scan();
            let mut prev_attr: Option<String> = None;
            while token != TokenType::Eos && scanner.get_token_end() <= attr_value_start {
                token = scanner.scan();
                if token == TokenType::AttributeName {
                    prev_attr = Some(scanner.get_token_text());
                }
            }
            prev_attr
        };

        let attr_value_range = get_tag_name_range(TokenType::AttributeValue, node.start);
        if let Some(range) = attr_value_range {
            let attr_value = trim_quotes(&document.get_text_range(range));
            let match_attr = scan_attr_and_attr_value(node.start, document.offset_at(range.start));
            if let Some(match_attr) = match_attr {
                return get_attr_value_hover(&node_tag, &match_attr, &attr_value, range);
            }
        }

        None
    }

    fn convert_contents(&self, contents: MarkupInput) -> MarkupInput {
        if !self.supports_markdown {
            return match contents {
                MarkupInput::String(s) => MarkupInput::String(s),
                MarkupInput::Markup(m) => {
                    MarkupInput::Markup(MarkupContent::new(MarkupKind::PlainText, m.value))
                }
            };
        }
        contents
    }
}

/// Port of `trimQuotes`.
fn trim_quotes(s: &str) -> String {
    if s.encode_utf16().count() <= 1 {
        // `s.replace(/['"]/, '')` replaces only the first occurrence.
        return match s.find(['\'', '"']) {
            Some(idx) => {
                let mut result = String::new();
                result.push_str(&s[..idx]);
                result.push_str(&s[idx + 1..]);
                result
            }
            None => s.to_string(),
        };
    }

    let mut s = s;
    if s.starts_with('\'') || s.starts_with('"') {
        s = &s[1..];
    }
    if s.ends_with('\'') || s.ends_with('"') {
        s = &s[..s.len() - 1];
    }
    s.to_string()
}

/// Port of `filterEntity`.
fn filter_entity(units: &[u16], offset: usize) -> String {
    let mut k = offset as i64 - 1;
    let mut new_text = String::from("&");

    while k >= 0 && is_letter_or_digit_unit(units, k as usize) {
        k -= 1;
    }

    k += 1;

    while is_letter_or_digit_unit(units, k as usize) {
        if let Some(&unit) = units.get(k as usize) {
            new_text.push(char::from_u32(unit as u32).unwrap_or('\u{FFFD}'));
        }
        k += 1;
    }

    new_text.push(';');

    new_text
}
