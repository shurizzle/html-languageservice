//! Port of `src/services/pathCompletion.ts` (`PathCompletionParticipant`).

use std::cell::RefCell;
use std::sync::Arc;

use crate::language_facts::data_manager::DataManager;
use crate::text_document::TextDocument;
use crate::types::{
    Command, CompletionItem, CompletionItemKind, CompletionList, DocumentContext, FileSystemProvider,
    FileType, HtmlAttributeValueContext, ICompletionParticipant, Range, TextEdit,
};
use crate::utils::strings::starts_with;

const TRIGGER_SUGGEST: &str = "editor.action.triggerSuggest";

pub struct PathCompletionParticipant<'a> {
    data_manager: &'a DataManager,
    read_directory: Arc<dyn FileSystemProvider>,
    attribute_completions: RefCell<Vec<HtmlAttributeValueContext>>,
}

impl<'a> PathCompletionParticipant<'a> {
    pub fn new(
        data_manager: &'a DataManager,
        read_directory: Arc<dyn FileSystemProvider>,
    ) -> Self {
        Self {
            data_manager,
            read_directory,
            attribute_completions: RefCell::new(Vec::new()),
        }
    }

    pub fn compute_completions(
        &self,
        document: &TextDocument,
        document_context: &dyn DocumentContext,
    ) -> CompletionList {
        let mut result = CompletionList::default();
        let completions = self.attribute_completions.borrow();
        for attribute_completion in completions.iter() {
            let full_value = strip_quotes(&document.get_text_range(attribute_completion.range));
            if !is_completable_path(&full_value) {
                continue;
            }
            if full_value == "." || full_value == ".." {
                result.is_incomplete = true;
                continue;
            }
            let replace_range =
                path_to_replace_range(&attribute_completion.value, &full_value, attribute_completion.range);
            let suggestions = self.provide_path_suggestions(
                &attribute_completion.value,
                replace_range,
                document,
                document_context,
                Some(attribute_completion),
            );
            result.items.extend(suggestions);
        }
        result
    }

    fn provide_path_suggestions(
        &self,
        value_before_cursor: &str,
        replace_range: Range,
        document: &TextDocument,
        document_context: &dyn DocumentContext,
        context: Option<&HtmlAttributeValueContext>,
    ) -> Vec<CompletionItem> {
        let value_before_last_slash = match value_before_cursor.rfind('/') {
            Some(i) => &value_before_cursor[..=i],
            None => "",
        };
        let reference = if value_before_last_slash.is_empty() {
            "."
        } else {
            value_before_last_slash
        };
        let Some(parent_dir) = document_context.resolve_reference(reference, document.uri()) else {
            return Vec::new();
        };

        let mut result: Vec<CompletionItem> = Vec::new();
        let infos = self.read_directory.read_directory(&parent_dir);
        let extension_filter = self.get_extension_filter(context);

        for (name, file_type) in infos {
            if name.starts_with('.') {
                continue;
            }
            let mut item = create_completion_item(&name, file_type == FileType::Directory, replace_range);
            if let Some(filter) = &extension_filter {
                if file_type == FileType::Directory {
                    result.push(item);
                } else {
                    let matches = filter
                        .extensions
                        .iter()
                        .any(|ext| name.to_lowercase().ends_with(ext));
                    if matches {
                        item.sort_text = Some(format!("0_{name}"));
                        result.push(item);
                    } else if !filter.exclusive {
                        item.sort_text = Some(format!("1_{name}"));
                        result.push(item);
                    }
                }
            } else {
                result.push(item);
            }
        }
        result
    }

    fn get_extension_filter(
        &self,
        context: Option<&HtmlAttributeValueContext>,
    ) -> Option<ExtensionFilter> {
        let context = context?;

        let attribute_value = |name: &str| -> Option<String> {
            context
                .attributes
                .iter()
                .find(|(k, _)| k == name)
                .and_then(|(_, v)| v.clone())
        };

        if context.tag == "link" && context.attribute == "href"
            && let Some(rel) = attribute_value("rel") {
                if rel == "stylesheet" || rel == "\"stylesheet\"" || rel == "'stylesheet'" {
                    return Some(ExtensionFilter {
                        extensions: vec![".css", ".scss", ".sass", ".less"],
                        exclusive: false,
                    });
                }
                if rel == "icon"
                    || rel == "\"icon\""
                    || rel == "'icon'"
                    || rel == "apple-touch-icon"
                    || rel == "\"apple-touch-icon\""
                    || rel == "'apple-touch-icon'"
                {
                    return Some(ExtensionFilter {
                        extensions: vec![".ico", ".png", ".svg", ".jpg", ".jpeg", ".gif", ".webp"],
                        exclusive: false,
                    });
                }
            }

        if context.tag == "script" && context.attribute == "src" {
            return Some(ExtensionFilter {
                extensions: vec![".js", ".mjs", ".cjs", ".ts", ".tsx", ".jsx"],
                exclusive: false,
            });
        }
        if context.tag == "img" && context.attribute == "src" {
            return Some(ExtensionFilter {
                extensions: vec![".png", ".jpg", ".jpeg", ".gif", ".svg", ".webp", ".bmp", ".ico"],
                exclusive: false,
            });
        }
        if context.tag == "video" && context.attribute == "src" {
            return Some(ExtensionFilter {
                extensions: vec![".mp4", ".webm", ".ogg", ".mov", ".avi"],
                exclusive: false,
            });
        }
        if context.tag == "audio" && context.attribute == "src" {
            return Some(ExtensionFilter {
                extensions: vec![".mp3", ".wav", ".ogg", ".m4a", ".aac", ".flac"],
                exclusive: false,
            });
        }
        None
    }
}

impl ICompletionParticipant for PathCompletionParticipant<'_> {
    fn on_html_attribute_value(&self, context: &HtmlAttributeValueContext) {
        if self
            .data_manager
            .is_path_attribute(&context.tag, &context.attribute)
        {
            self.attribute_completions.borrow_mut().push(context.clone());
        }
    }
}

struct ExtensionFilter {
    extensions: Vec<&'static str>,
    exclusive: bool,
}

/// Port of `stripQuotes` (removes a single leading/trailing quote pair).
fn strip_quotes(full_value: &str) -> String {
    if starts_with(full_value, "'") || starts_with(full_value, "\"") {
        full_value[1..full_value.len().saturating_sub(1)].to_string()
    } else {
        full_value.to_string()
    }
}

fn is_completable_path(value: &str) -> bool {
    !(starts_with(value, "http") || starts_with(value, "https") || starts_with(value, "//"))
}

fn path_to_replace_range(value_before_cursor: &str, full_value: &str, range: Range) -> Range {
    let Some(last_slash) = value_before_cursor.rfind('/') else {
        return shift_range(range, 1, -1);
    };

    let value_after_last_slash = &full_value[last_slash + 1..];
    let start_pos = shift_position(
        range.end,
        -1 - value_after_last_slash.chars().count() as i64,
    );

    let end_pos = match value_after_last_slash.find(' ') {
        Some(index) => shift_position(start_pos, index as i64),
        None => shift_position(range.end, -1),
    };

    Range {
        start: start_pos,
        end: end_pos,
    }
}

fn create_completion_item(p: &str, is_dir: bool, replace_range: Range) -> CompletionItem {
    if is_dir {
        let label = format!("{p}/");
        CompletionItem {
            label: label.clone(),
            kind: Some(CompletionItemKind::Folder),
            text_edit: Some(TextEdit::replace(replace_range, label)),
            command: Some(Command {
                title: "Suggest".to_string(),
                command: TRIGGER_SUGGEST.to_string(),
            }),
            ..Default::default()
        }
    } else {
        CompletionItem {
            label: p.to_string(),
            kind: Some(CompletionItemKind::File),
            text_edit: Some(TextEdit::replace(replace_range, p)),
            ..Default::default()
        }
    }
}

fn shift_position(pos: crate::types::Position, offset: i64) -> crate::types::Position {
    crate::types::Position {
        line: pos.line,
        character: (pos.character as i64 + offset).max(0) as u32,
    }
}

fn shift_range(range: Range, start_offset: i64, end_offset: i64) -> Range {
    Range {
        start: shift_position(range.start, start_offset),
        end: shift_position(range.end, end_offset),
    }
}
