//! Port of `src/services/htmlCompletion.ts`.
//!
//! The path-completion half of `doComplete2` lives in
//! [`crate::services::path_completion`]; where upstream is asynchronous, the
//! [`FileSystemProvider`](crate::types::FileSystemProvider) callback here is
//! synchronous.

use std::collections::HashMap;

use crate::html_language_types::{ScannerState, TokenType};
use crate::language_facts::DataManager;
use crate::language_facts::data_provider::generate_documentation;
use crate::parser::html_entities::entities;
use crate::parser::html_parser::{HtmlDocument, NodeId};
use crate::parser::html_scanner::{LAN, Scanner, create_scanner};
use crate::text_document::TextDocument;
use crate::types::{
    AttributeDefaultValue, ClientCapabilities, Command, CompletionConfiguration, CompletionItem,
    CompletionItemKind, CompletionList, HtmlAttributeValueContext, HtmlContentContext,
    ICompletionParticipant, IHtmlDataProvider, InsertTextFormat, MarkupInput, MarkupKind, Position,
    Range, TextEdit,
};
use crate::utils::strings::{ends_with, is_js_whitespace, is_letter_or_digit_unit, starts_with};

pub struct HtmlCompletion<'a> {
    supports_markdown: bool,
    data_manager: &'a DataManager,
}

impl<'a> HtmlCompletion<'a> {
    pub fn new(
        client_capabilities: Option<&ClientCapabilities>,
        data_manager: &'a DataManager,
    ) -> Self {
        let supports_markdown = match client_capabilities {
            None => true,
            Some(caps) => caps
                .text_document
                .completion
                .completion_item
                .documentation_format
                .contains(&MarkupKind::Markdown),
        };
        Self {
            supports_markdown,
            data_manager,
        }
    }

    pub fn do_complete(
        &self,
        document: &TextDocument,
        position: Position,
        html_document: &HtmlDocument,
        settings: Option<&CompletionConfiguration>,
        participants: &[Box<dyn ICompletionParticipant>],
    ) -> CompletionList {
        let refs: Vec<&dyn ICompletionParticipant> =
            participants.iter().map(|p| p.as_ref()).collect();
        self.do_complete_with_participants(document, position, html_document, settings, &refs)
    }

    /// Variant used by [`crate::language_service::LanguageService::do_complete2`],
    /// which injects the path-completion participant as a borrowed reference.
    pub fn do_complete_with_participants(
        &self,
        document: &TextDocument,
        position: Position,
        html_document: &HtmlDocument,
        settings: Option<&CompletionConfiguration>,
        participants: &[&dyn ICompletionParticipant],
    ) -> CompletionList {
        let list =
            self.do_complete_inner(document, position, html_document, settings, participants);
        self.convert_completion_list(list)
    }

    fn do_complete_inner(
        &self,
        document: &TextDocument,
        position: Position,
        html_document: &HtmlDocument,
        settings: Option<&CompletionConfiguration>,
        participants: &[&dyn ICompletionParticipant],
    ) -> CompletionList {
        let data_providers: Vec<&dyn IHtmlDataProvider> = self
            .data_manager
            .get_data_providers()
            .into_iter()
            .filter(|p| {
                p.is_applicable(document.language_id())
                    && settings.is_none_or(|s| s.provider_enabled(p.id()))
            })
            .collect();

        let offset = document.offset_at(position);
        let node = html_document.find_node_before(offset);

        let mut ctx = CompletionContext {
            document,
            position,
            html_document,
            settings,
            data_providers: &data_providers,
            does_support_markdown: self.supports_markdown,
            data_manager: self.data_manager,
            text: document.get_text(),
            units: document.get_text().encode_utf16().collect(),
            offset,
            node,
            participants,
            result: CompletionList::default(),
            current_tag: String::new(),
            current_attribute_name: String::new(),
            void_elements: None,
        };

        let mut scanner = create_scanner(
            ctx.text,
            ctx.node_start(),
            ScannerState::WithinContent,
            false,
        );
        let mut token = scanner.scan();

        while token != TokenType::Eos && scanner.get_token_offset() <= offset {
            match token {
                TokenType::StartTagOpen => {
                    if scanner.get_token_end() == offset {
                        let end_pos = ctx.scan_next_for_end_pos(
                            &mut scanner,
                            &mut token,
                            TokenType::StartTag,
                        );
                        if position.line == 0 {
                            ctx.suggest_doctype(offset as i64, end_pos as i64);
                        }
                        return ctx.collect_tag_suggestions(offset, end_pos);
                    }
                }
                TokenType::StartTag => {
                    if scanner.get_token_offset() <= offset && offset <= scanner.get_token_end() {
                        return ctx.collect_open_tag_suggestions(
                            scanner.get_token_offset(),
                            scanner.get_token_end(),
                        );
                    }
                    ctx.current_tag = scanner.get_token_text();
                }
                TokenType::AttributeName => {
                    if scanner.get_token_offset() <= offset && offset <= scanner.get_token_end() {
                        return ctx.collect_attribute_name_suggestions(
                            scanner.get_token_offset(),
                            scanner.get_token_end(),
                        );
                    }
                    ctx.current_attribute_name = scanner.get_token_text();
                }
                TokenType::DelimiterAssign => {
                    if scanner.get_token_end() == offset {
                        let end_pos = ctx.scan_next_for_end_pos(
                            &mut scanner,
                            &mut token,
                            TokenType::AttributeValue,
                        );
                        return ctx.collect_attribute_value_suggestions(offset, end_pos);
                    }
                }
                TokenType::AttributeValue => {
                    if scanner.get_token_offset() <= offset && offset <= scanner.get_token_end() {
                        return ctx.collect_attribute_value_suggestions(
                            scanner.get_token_offset(),
                            scanner.get_token_end(),
                        );
                    }
                }
                TokenType::Whitespace => {
                    if offset <= scanner.get_token_end() {
                        match scanner.get_scanner_state() {
                            ScannerState::AfterOpeningStartTag => {
                                let start_pos = scanner.get_token_offset();
                                let end_tag_pos = ctx.scan_next_for_end_pos(
                                    &mut scanner,
                                    &mut token,
                                    TokenType::StartTag,
                                );
                                return ctx.collect_tag_suggestions(start_pos, end_tag_pos);
                            }
                            ScannerState::WithinTag | ScannerState::AfterAttributeName => {
                                return ctx.collect_attribute_name_suggestions(
                                    scanner.get_token_end(),
                                    offset,
                                );
                            }
                            ScannerState::BeforeAttributeValue => {
                                return ctx.collect_attribute_value_suggestions(
                                    scanner.get_token_end(),
                                    offset,
                                );
                            }
                            ScannerState::AfterOpeningEndTag => {
                                return ctx.collect_close_tag_suggestions(
                                    scanner.get_token_offset() as i64 - 1,
                                    false,
                                    offset,
                                );
                            }
                            ScannerState::WithinContent => {
                                return ctx.collect_inside_content();
                            }
                            _ => {}
                        }
                    }
                }
                TokenType::EndTagOpen => {
                    if offset <= scanner.get_token_end() {
                        let after_open_bracket = scanner.get_token_offset() as i64 + 1;
                        let end_offset =
                            ctx.scan_next_for_end_pos(&mut scanner, &mut token, TokenType::EndTag);
                        return ctx.collect_close_tag_suggestions(
                            after_open_bracket,
                            false,
                            end_offset,
                        );
                    }
                }
                TokenType::EndTag => {
                    if offset <= scanner.get_token_end() {
                        let mut start = scanner.get_token_offset() as i64 - 1;
                        while start >= 0 {
                            let ch = ctx.units.get(start as usize).copied();
                            if ch == Some(b'/' as u16) {
                                return ctx.collect_close_tag_suggestions(
                                    start,
                                    false,
                                    scanner.get_token_end(),
                                );
                            } else if !ch.map(is_whitespace_unit).unwrap_or(false) {
                                break;
                            }
                            start -= 1;
                        }
                    }
                }
                TokenType::StartTagClose => {
                    if offset <= scanner.get_token_end() && !ctx.current_tag.is_empty() {
                        let tag = ctx.current_tag.clone();
                        return ctx
                            .collect_auto_close_tag_suggestion(scanner.get_token_end(), &tag);
                    }
                }
                TokenType::Content => {
                    if offset <= scanner.get_token_end() {
                        return ctx.collect_inside_content();
                    }
                }
                _ => {
                    if offset <= scanner.get_token_end() {
                        return std::mem::take(&mut ctx.result);
                    }
                }
            }
            token = scanner.scan();
        }

        ctx.result
    }

    /// Port of `doQuoteComplete`.
    pub fn do_quote_complete(
        &self,
        document: &TextDocument,
        position: Position,
        html_document: &HtmlDocument,
        settings: Option<&CompletionConfiguration>,
    ) -> Option<String> {
        let offset = document.offset_at(position);
        if offset == 0 {
            return None;
        }
        let default_value = settings
            .and_then(|s| s.attribute_default_value)
            .unwrap_or(AttributeDefaultValue::DoubleQuotes);
        if default_value == AttributeDefaultValue::Empty {
            return None;
        }
        let units: Vec<u16> = document.get_text().encode_utf16().collect();
        if units.get(offset - 1) != Some(&(b'=' as u16)) {
            return None;
        }
        let value = if default_value == AttributeDefaultValue::DoubleQuotes {
            "\"$1\""
        } else {
            "'$1'"
        };
        let node = html_document.find_node_before(offset);
        let node = html_document.node(node);
        let has_attributes = !node.attributes.is_empty();
        if has_attributes
            && node.start < offset
            && node.end_tag_start.is_none_or(|end| end > offset)
        {
            let mut scanner = create_scanner(
                document.get_text(),
                node.start,
                ScannerState::WithinContent,
                false,
            );
            let mut token = scanner.scan();
            while token != TokenType::Eos && scanner.get_token_end() <= offset {
                if token == TokenType::AttributeName && scanner.get_token_end() == offset - 1 {
                    token = scanner.scan();
                    if token != TokenType::DelimiterAssign {
                        return None;
                    }
                    token = scanner.scan();
                    if token == TokenType::Unknown || token == TokenType::AttributeValue {
                        return None;
                    }
                    return Some(value.to_string());
                }
                token = scanner.scan();
            }
        }
        None
    }

    /// Port of `doTagComplete`.
    pub fn do_tag_complete(
        &self,
        document: &TextDocument,
        position: Position,
        html_document: &HtmlDocument,
    ) -> Option<String> {
        let offset = document.offset_at(position);
        if offset == 0 {
            return None;
        }
        let units: Vec<u16> = document.get_text().encode_utf16().collect();
        let ch = units.get(offset - 1).copied();

        if ch == Some(b'>' as u16) {
            let node_id = html_document.find_node_before(offset);
            let node = html_document.node(node_id);
            if let Some(tag) = &node.tag
                && node.start < offset
                && node.end_tag_start.is_none_or(|end| end > offset)
            {
                let void_elements = self.data_manager.get_void_elements(document.language_id());
                if !self.data_manager.is_void_element(tag, &void_elements) {
                    let mut scanner = create_scanner(
                        document.get_text(),
                        node.start,
                        ScannerState::WithinContent,
                        false,
                    );
                    let mut token = scanner.scan();
                    while token != TokenType::Eos && scanner.get_token_end() <= offset {
                        if token == TokenType::StartTagClose && scanner.get_token_end() == offset {
                            return Some(format!("$0</{tag}>"));
                        }
                        token = scanner.scan();
                    }
                }
            }
        } else if ch == Some(b'/' as u16) {
            let mut curr = html_document.find_node_before(offset);
            loop {
                let node = html_document.node(curr);
                if !(node.closed && !node.end_tag_start.is_some_and(|end| end > offset)) {
                    break;
                }
                match html_document.parent(curr) {
                    Some(parent) => curr = parent,
                    None => break,
                }
            }
            let node = html_document.node(curr);
            if let Some(tag) = &node.tag {
                let mut scanner = create_scanner(
                    document.get_text(),
                    node.start,
                    ScannerState::WithinContent,
                    false,
                );
                let mut token = scanner.scan();
                while token != TokenType::Eos && scanner.get_token_end() <= offset {
                    if token == TokenType::EndTagOpen && scanner.get_token_end() == offset {
                        if units.get(offset) != Some(&(b'>' as u16)) {
                            return Some(format!("{tag}>"));
                        }
                        return Some(tag.clone());
                    }
                    token = scanner.scan();
                }
            }
        }
        None
    }

    fn convert_completion_list(&self, mut list: CompletionList) -> CompletionList {
        if !self.supports_markdown {
            for item in &mut list.items {
                if let Some(MarkupInput::Markup(markup)) = &item.documentation {
                    item.documentation =
                        Some(MarkupInput::Markup(crate::types::MarkupContent::new(
                            MarkupKind::PlainText,
                            markup.value.clone(),
                        )));
                }
            }
        }
        list
    }
}

struct CompletionContext<'a> {
    document: &'a TextDocument,
    position: Position,
    html_document: &'a HtmlDocument,
    settings: Option<&'a CompletionConfiguration>,
    data_providers: &'a [&'a dyn IHtmlDataProvider],
    does_support_markdown: bool,
    data_manager: &'a DataManager,
    text: &'a str,
    units: Vec<u16>,
    offset: usize,
    node: NodeId,
    participants: &'a [&'a dyn ICompletionParticipant],
    result: CompletionList,
    current_tag: String,
    current_attribute_name: String,
    void_elements: Option<Vec<String>>,
}

impl<'a> CompletionContext<'a> {
    fn node_start(&self) -> usize {
        self.html_document.node(self.node).start
    }

    fn substring(&self, start: usize, end: usize) -> String {
        let end = end.min(self.units.len());
        let start = start.min(end);
        String::from_utf16_lossy(&self.units[start..end])
    }

    fn char_at(&self, index: usize) -> Option<u16> {
        self.units.get(index).copied()
    }

    fn attribute_default_value(&self) -> AttributeDefaultValue {
        self.settings
            .and_then(|s| s.attribute_default_value)
            .unwrap_or(AttributeDefaultValue::DoubleQuotes)
    }

    fn get_replace_range(&self, replace_start: i64, replace_end: i64) -> Range {
        let mut start = replace_start;
        if start > self.offset as i64 {
            start = self.offset as i64;
        }
        Range {
            start: self.document.position_at(start.max(0) as usize),
            end: self.document.position_at(replace_end.max(0) as usize),
        }
    }

    fn collect_open_tag_suggestions(
        &mut self,
        after_open_bracket: usize,
        tag_name_end: usize,
    ) -> CompletionList {
        let range = self.get_replace_range(after_open_bracket as i64, tag_name_end as i64);
        for provider in self.data_providers {
            for tag in provider.provide_tags() {
                self.result.items.push(CompletionItem {
                    label: tag.name.clone(),
                    kind: Some(CompletionItemKind::Property),
                    documentation: generate_documentation(tag, None, self.does_support_markdown)
                        .map(MarkupInput::Markup),
                    text_edit: Some(TextEdit::replace(range, tag.name.clone())),
                    insert_text_format: Some(InsertTextFormat::PlainText),
                    ..Default::default()
                });
            }
        }
        self.result.clone()
    }

    fn get_line_indent(&self, offset: usize) -> Option<String> {
        let mut start = offset;
        while start > 0 {
            match self.char_at(start - 1) {
                Some(ch) if ch == b'\n' as u16 || ch == b'\r' as u16 => {
                    return Some(self.substring(start, offset));
                }
                Some(ch) if is_whitespace_unit(ch) => {}
                _ => return None,
            }
            start -= 1;
        }
        Some(self.substring(0, offset))
    }

    fn collect_close_tag_suggestions(
        &mut self,
        after_open_bracket: i64,
        in_open_tag: bool,
        tag_name_end: usize,
    ) -> CompletionList {
        if self.settings.is_some_and(|s| s.hide_end_tag_suggestions) {
            return self.result.clone();
        }
        let range = self.get_replace_range(after_open_bracket, tag_name_end as i64);
        let close_tag = if is_followed_by(
            self.text,
            tag_name_end,
            ScannerState::WithinEndTag,
            TokenType::EndTagClose,
        ) {
            ""
        } else {
            ">"
        };
        let mut curr = if in_open_tag {
            self.html_document.parent(self.node)
        } else {
            Some(self.node)
        };
        while let Some(node_id) = curr {
            let node = self.html_document.node(node_id);
            let tag = node.tag.clone();
            if let Some(tag) = tag
                && (!node.closed || node.end_tag_start.is_some_and(|end| end > self.offset))
            {
                let mut item = CompletionItem {
                    label: format!("/{tag}"),
                    kind: Some(CompletionItemKind::Property),
                    filter_text: Some(format!("/{tag}")),
                    text_edit: Some(TextEdit::replace(range, format!("/{tag}{close_tag}"))),
                    insert_text_format: Some(InsertTextFormat::PlainText),
                    ..Default::default()
                };
                let start_indent = self.get_line_indent(node.start);
                let end_indent = self.get_line_indent((after_open_bracket - 1).max(0) as usize);
                if let (Some(start_indent), Some(end_indent)) = (start_indent, end_indent)
                    && start_indent != end_indent
                {
                    let insert_text = format!("{start_indent}</{tag}{close_tag}");
                    let end_indent_len = end_indent.encode_utf16().count();
                    item.text_edit = Some(TextEdit::replace(
                        self.get_replace_range(
                            after_open_bracket - 1 - end_indent_len as i64,
                            self.offset as i64,
                        ),
                        insert_text,
                    ));
                    item.filter_text = Some(format!("{end_indent}</{tag}"));
                }
                self.result.items.push(item);
                return self.result.clone();
            }
            curr = self.html_document.parent(node_id);
        }
        if in_open_tag {
            return self.result.clone();
        }

        for provider in self.data_providers {
            for tag in provider.provide_tags() {
                self.result.items.push(CompletionItem {
                    label: format!("/{}", tag.name),
                    kind: Some(CompletionItemKind::Property),
                    documentation: generate_documentation(tag, None, self.does_support_markdown)
                        .map(MarkupInput::Markup),
                    filter_text: Some(format!("/{}{close_tag}", tag.name)),
                    text_edit: Some(TextEdit::replace(
                        range,
                        format!("/{}{close_tag}", tag.name),
                    )),
                    insert_text_format: Some(InsertTextFormat::PlainText),
                    ..Default::default()
                });
            }
        }
        self.result.clone()
    }

    fn collect_auto_close_tag_suggestion(
        &mut self,
        tag_close_end: usize,
        tag: &str,
    ) -> CompletionList {
        if self
            .settings
            .is_some_and(|s| s.hide_auto_complete_proposals)
        {
            return self.result.clone();
        }
        let void_elements = self
            .void_elements
            .get_or_insert_with(|| self.data_manager.get_void_elements_for(self.data_providers))
            .clone();
        if !self.data_manager.is_void_element(tag, &void_elements) {
            let pos = self.document.position_at(tag_close_end);
            self.result.items.push(CompletionItem {
                label: format!("</{tag}>"),
                kind: Some(CompletionItemKind::Property),
                filter_text: Some(format!("</{tag}>")),
                text_edit: Some(TextEdit::insert(pos, format!("$0</{tag}>"))),
                insert_text_format: Some(InsertTextFormat::Snippet),
                ..Default::default()
            });
        }
        self.result.clone()
    }

    fn collect_tag_suggestions(&mut self, tag_start: usize, tag_end: usize) -> CompletionList {
        self.collect_open_tag_suggestions(tag_start, tag_end);
        self.collect_close_tag_suggestions(tag_start as i64, true, tag_end)
    }

    fn get_existing_attributes(&self) -> HashMap<String, bool> {
        let node = self.html_document.node(self.node);
        node.attributes
            .iter()
            .map(|(name, _)| (name.clone(), true))
            .collect()
    }

    fn collect_attribute_name_suggestions(
        &mut self,
        name_start: usize,
        name_end: usize,
    ) -> CompletionList {
        let mut replace_end = self.offset;
        while replace_end < name_end && self.char_at(replace_end) != Some(LAN) {
            replace_end += 1;
        }
        let current_attribute = self.substring(name_start, name_end);
        let range = self.get_replace_range(name_start as i64, replace_end as i64);
        let mut value = String::new();
        if !is_followed_by(
            self.text,
            name_end,
            ScannerState::AfterAttributeName,
            TokenType::DelimiterAssign,
        ) {
            match self.attribute_default_value() {
                AttributeDefaultValue::Empty => value = "=$1".to_string(),
                AttributeDefaultValue::SingleQuotes => value = "='$1'".to_string(),
                AttributeDefaultValue::DoubleQuotes => value = "=\"$1\"".to_string(),
            }
        }

        let mut seen_attributes = self.get_existing_attributes();
        // include current typing attribute
        seen_attributes.insert(current_attribute, false);

        let current_tag = self.current_tag.clone();
        let does_support_markdown = self.does_support_markdown;
        for provider in self.data_providers {
            for attr in provider.provide_attributes(&current_tag) {
                if seen_attributes.get(&attr.name).copied().unwrap_or(false) {
                    continue;
                }
                seen_attributes.insert(attr.name.clone(), true);

                let mut code_snippet = attr.name.clone();
                let mut command = None;
                if attr.value_set.as_deref() != Some("v") && !value.is_empty() {
                    code_snippet.push_str(&value);
                    if attr.value_set.is_some() || attr.name == "style" {
                        command = Some(Command {
                            title: "Suggest".to_string(),
                            command: "editor.action.triggerSuggest".to_string(),
                        });
                    }
                }

                self.result.items.push(CompletionItem {
                    label: attr.name.clone(),
                    kind: Some(if attr.value_set.as_deref() == Some("handler") {
                        CompletionItemKind::Function
                    } else {
                        CompletionItemKind::Value
                    }),
                    documentation: generate_documentation(&attr, None, does_support_markdown)
                        .map(MarkupInput::Markup),
                    text_edit: Some(TextEdit::replace(range, code_snippet)),
                    insert_text_format: Some(InsertTextFormat::Snippet),
                    command,
                    ..Default::default()
                });
            }
        }
        self.collect_data_attributes_suggestions(range, &seen_attributes);
        self.result.clone()
    }

    fn collect_data_attributes_suggestions(
        &mut self,
        range: Range,
        seen_attributes: &HashMap<String, bool>,
    ) {
        const DATA_ATTR: &str = "data-";
        let mut data_attributes: Vec<(String, String)> = Vec::new();
        data_attributes.push((DATA_ATTR.to_string(), format!("{DATA_ATTR}$1=\"$2\"")));

        let mut stack: Vec<NodeId> = self.html_document.roots().to_vec();
        while let Some(node_id) = stack.pop() {
            let node = self.html_document.node(node_id);
            for (attr, _) in &node.attributes {
                if starts_with(attr, DATA_ATTR)
                    && !data_attributes.iter().any(|(n, _)| n == attr)
                    && !seen_attributes.get(attr).copied().unwrap_or(false)
                {
                    data_attributes.push((attr.clone(), format!("{attr}=\"$1\"")));
                }
            }
            stack.extend(self.html_document.children(node_id).iter().copied());
        }

        for (attr, snippet) in data_attributes {
            self.result.items.push(CompletionItem {
                label: attr,
                kind: Some(CompletionItemKind::Value),
                text_edit: Some(TextEdit::replace(range, snippet)),
                insert_text_format: Some(InsertTextFormat::Snippet),
                ..Default::default()
            });
        }
    }

    fn collect_attribute_value_suggestions(
        &mut self,
        value_start: usize,
        value_end: usize,
    ) -> CompletionList {
        let range: Range;
        let add_quotes: bool;
        let value_prefix: String;

        if self.offset > value_start
            && self.offset <= value_end
            && is_quote_unit(self.char_at(value_start))
        {
            // inside quoted attribute
            let value_content_start = value_start + 1;
            let mut value_content_end = value_end;
            if value_end > value_start && self.char_at(value_end - 1) == self.char_at(value_start) {
                // Even when a matching quote is found, the scanner may have latched
                // onto an unrelated later quote across HTML tag boundaries. If any
                // `<` appears between the cursor and the alleged closing quote,
                // treat the value as unclosed and clamp so the completion cannot
                // delete subsequent markup.
                let mut crosses_tag_boundary = false;
                for i in self.offset..value_end - 1 {
                    if self.char_at(i) == Some(LAN) {
                        crosses_tag_boundary = true;
                        break;
                    }
                }
                value_content_end = if crosses_tag_boundary {
                    self.offset
                } else {
                    value_content_end - 1
                };
            } else {
                value_content_end = self.offset;
            }

            let ws_before = get_word_start(&self.units, self.offset, value_content_start);
            let ws_after = get_word_end(&self.units, self.offset, value_content_end);
            range = self.get_replace_range(ws_before as i64, ws_after as i64);
            value_prefix = if self.offset >= value_content_start && self.offset <= value_content_end
            {
                self.substring(value_content_start, self.offset)
            } else {
                String::new()
            };
            add_quotes = false;
        } else {
            range = self.get_replace_range(value_start as i64, value_end as i64);
            value_prefix = self.substring(value_start, self.offset);
            add_quotes = true;
        }

        if !self.participants.is_empty() {
            let tag = self.current_tag.to_lowercase();
            let attribute = self.current_attribute_name.to_lowercase();
            let full_range = self.get_replace_range(value_start as i64, value_end as i64);
            let attributes = self.html_document.node(self.node).attributes.clone();
            for participant in self.participants {
                participant.on_html_attribute_value(&HtmlAttributeValueContext {
                    position: self.position,
                    tag: tag.clone(),
                    attribute: attribute.clone(),
                    value: value_prefix.clone(),
                    range: full_range,
                    attributes: attributes.clone(),
                });
            }
        }

        let current_tag = self.current_tag.clone();
        let current_attribute_name = self.current_attribute_name.clone();
        let does_support_markdown = self.does_support_markdown;
        let mut items: Vec<CompletionItem> = Vec::new();
        for provider in self.data_providers {
            for value in provider.provide_values(&current_tag, &current_attribute_name) {
                let insert_text = if add_quotes {
                    format!("\"{}\"", value.name)
                } else {
                    value.name.clone()
                };
                items.push(CompletionItem {
                    label: value.name.clone(),
                    filter_text: Some(insert_text.clone()),
                    kind: Some(CompletionItemKind::Unit),
                    documentation: generate_documentation(&value, None, does_support_markdown)
                        .map(MarkupInput::Markup),
                    text_edit: Some(TextEdit::replace(range, insert_text)),
                    insert_text_format: Some(InsertTextFormat::PlainText),
                    ..Default::default()
                });
            }
        }
        self.result.items.extend(items);

        self.collect_character_entity_proposals()
    }

    fn scan_next_for_end_pos(
        &self,
        scanner: &mut Scanner,
        token: &mut TokenType,
        next_token: TokenType,
    ) -> usize {
        if self.offset == scanner.get_token_end() {
            *token = scanner.scan();
            if *token == next_token && scanner.get_token_offset() == self.offset {
                return scanner.get_token_end();
            }
        }
        self.offset
    }

    fn collect_inside_content(&mut self) -> CompletionList {
        for participant in self.participants {
            participant.on_html_content(&HtmlContentContext {
                position: self.position,
            });
        }
        self.collect_character_entity_proposals()
    }

    fn collect_character_entity_proposals(&mut self) -> CompletionList {
        let mut k = self.offset as i64 - 1;
        let mut character_start = self.position.character as i64;
        while k >= 0 && is_letter_or_digit_unit(&self.units, k as usize) {
            k -= 1;
            character_start -= 1;
        }
        if k >= 0 && self.char_at(k as usize) == Some(b'&' as u16) {
            let range = Range {
                start: Position::create(self.position.line, (character_start - 1) as u32),
                end: self.position,
            };
            for (entity, replacement) in entities() {
                if ends_with(entity, ";") {
                    let label = format!("&{entity}");
                    self.result.items.push(CompletionItem {
                        label: label.clone(),
                        kind: Some(CompletionItemKind::Keyword),
                        documentation: Some(MarkupInput::String(format!(
                            "Character entity representing '{replacement}'"
                        ))),
                        text_edit: Some(TextEdit::replace(range, label)),
                        insert_text_format: Some(InsertTextFormat::PlainText),
                        ..Default::default()
                    });
                }
            }
        }
        self.result.clone()
    }

    fn suggest_doctype(&mut self, replace_start: i64, replace_end: i64) {
        let range = self.get_replace_range(replace_start, replace_end);
        self.result.items.push(CompletionItem {
            label: "!DOCTYPE".to_string(),
            kind: Some(CompletionItemKind::Property),
            documentation: Some(MarkupInput::String(
                "A preamble for an HTML document.".to_string(),
            )),
            text_edit: Some(TextEdit::replace(range, "!DOCTYPE html>")),
            insert_text_format: Some(InsertTextFormat::PlainText),
            ..Default::default()
        });
    }
}

/// Port of `isQuote`.
fn is_quote_unit(u: Option<u16>) -> bool {
    matches!(u, Some(c) if c == b'"' as u16 || c == b'\'' as u16)
}

/// Port of `isWhiteSpace`.
fn is_whitespace_unit(c: u16) -> bool {
    char::from_u32(c as u32).is_some_and(is_js_whitespace)
}

/// Port of `isFollowedBy`.
fn is_followed_by(
    s: &str,
    offset: usize,
    initial_state: ScannerState,
    expected_token: TokenType,
) -> bool {
    let mut scanner = create_scanner(s, offset, initial_state, false);
    let mut token = scanner.scan();
    while token == TokenType::Whitespace {
        token = scanner.scan();
    }
    token == expected_token
}

/// Port of `getWordStart`.
fn get_word_start(units: &[u16], mut offset: usize, limit: usize) -> usize {
    while offset > limit
        && !units
            .get(offset - 1)
            .copied()
            .map(is_whitespace_unit)
            .unwrap_or(false)
    {
        offset -= 1;
    }
    offset
}

/// Port of `getWordEnd`.
fn get_word_end(units: &[u16], mut offset: usize, limit: usize) -> usize {
    while offset < limit
        && !units
            .get(offset)
            .copied()
            .map(is_whitespace_unit)
            .unwrap_or(false)
    {
        offset += 1;
    }
    offset
}
