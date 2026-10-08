//! Conversions from `html-languageservice` types to LSP JSON.

use html_languageservice::{
    CompletionItem, DocumentHighlight, DocumentLink, DocumentSymbol, FoldingRange, Hover,
    MarkupInput, Range, SelectionRange, TextEdit, WorkspaceEdit,
};
use serde_json::{Value, json};

use crate::protocol;

pub fn position(p: protocol::Position) -> html_languageservice::Position {
    html_languageservice::Position::create(p.line, p.character)
}

pub fn range(r: protocol::Range) -> Range {
    Range {
        start: position(r.start),
        end: position(r.end),
    }
}

pub fn position_json(p: &html_languageservice::Position) -> Value {
    json!({ "line": p.line, "character": p.character })
}

pub fn range_json(r: &Range) -> Value {
    json!({ "start": position_json(&r.start), "end": position_json(&r.end) })
}

pub fn text_edit(edit: &TextEdit) -> Value {
    json!({ "range": range_json(&edit.range), "newText": edit.new_text })
}

pub fn completion_item(item: &CompletionItem) -> Value {
    let mut value = json!({ "label": item.label });
    if let Some(kind) = item.kind {
        value["kind"] = json!(kind as u32);
    }
    if let Some(documentation) = &item.documentation {
        value["documentation"] = markup(documentation);
    }
    if let Some(filter_text) = &item.filter_text {
        value["filterText"] = json!(filter_text);
    }
    if let Some(sort_text) = &item.sort_text {
        value["sortText"] = json!(sort_text);
    }
    if let Some(text_edit) = &item.text_edit {
        value["textEdit"] = text_edit_json(text_edit);
    }
    if let Some(format) = item.insert_text_format {
        value["insertTextFormat"] = json!(format as u32);
    }
    if let Some(command) = &item.command {
        value["command"] = json!({ "title": command.title, "command": command.command });
    }
    value
}

fn text_edit_json(edit: &TextEdit) -> Value {
    json!({ "range": range_json(&edit.range), "newText": edit.new_text })
}

fn markup(input: &MarkupInput) -> Value {
    let content = input.as_markup();
    json!({ "kind": content.kind.as_str(), "value": content.value })
}

pub fn hover(hover: &Hover) -> Value {
    let mut value = json!({ "contents": markup(&hover.contents) });
    if let Some(range) = &hover.range {
        value["range"] = range_json(range);
    }
    value
}

pub fn document_highlight(highlight: &DocumentHighlight) -> Value {
    json!({
        "range": range_json(&highlight.range),
        "kind": highlight.kind as u32,
    })
}

pub fn document_symbol(symbol: &DocumentSymbol) -> Value {
    let mut value = json!({
        "name": symbol.name,
        "kind": symbol.kind as u32,
        "range": range_json(&symbol.range),
        "selectionRange": range_json(&symbol.selection_range),
    });
    if let Some(detail) = &symbol.detail {
        value["detail"] = json!(detail);
    }
    if let Some(children) = &symbol.children {
        let children: Vec<Value> = children.iter().map(document_symbol).collect();
        value["children"] = json!(children);
    }
    value
}

pub fn folding_range(range: &FoldingRange) -> Value {
    let mut value = json!({ "startLine": range.start_line, "endLine": range.end_line });
    if let Some(kind) = range.kind {
        value["kind"] = json!(kind.as_str());
    }
    value
}

pub fn selection_range(selection: &SelectionRange) -> Value {
    let mut value = json!({ "range": range_json(&selection.range) });
    if let Some(parent) = &selection.parent {
        value["parent"] = selection_range(parent);
    }
    value
}

pub fn workspace_edit(edit: &WorkspaceEdit) -> Value {
    let changes: serde_json::Map<String, Value> = edit
        .changes
        .iter()
        .map(|(uri, edits)| {
            let edits: Vec<Value> = edits.iter().map(text_edit).collect();
            (uri.clone(), json!(edits))
        })
        .collect();
    json!({ "changes": Value::Object(changes) })
}

pub fn document_link(link: &DocumentLink) -> Value {
    let mut value = json!({ "range": range_json(&link.range) });
    if let Some(target) = &link.target {
        value["target"] = json!(target);
    }
    value
}
