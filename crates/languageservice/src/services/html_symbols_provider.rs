//! Port of `src/services/htmlSymbolsProvider.ts`.

use crate::parser::html_parser::{HtmlDocument, Node, NodeId};
use crate::text_document::TextDocument;
use crate::types::{DocumentSymbol, Location, Range, SymbolInformation, SymbolKind};
use crate::utils::strings::{split_whitespace_runs, strip_quotes};

pub fn find_document_symbols(
    document: &TextDocument,
    html_document: &HtmlDocument,
) -> Vec<SymbolInformation> {
    let mut symbols = Vec::new();
    let symbols2 = find_document_symbols2(document, html_document);
    for symbol in &symbols2 {
        walk(document, symbol, None, &mut symbols);
    }
    symbols
}

fn walk(
    document: &TextDocument,
    node: &DocumentSymbol,
    parent: Option<&DocumentSymbol>,
    symbols: &mut Vec<SymbolInformation>,
) {
    symbols.push(SymbolInformation {
        name: node.name.clone(),
        kind: node.kind,
        location: Location::create(document.uri(), node.range),
        container_name: parent.map(|p| p.name.clone()).unwrap_or_default(),
    });
    if let Some(children) = &node.children {
        for child in children {
            walk(document, child, Some(node), symbols);
        }
    }
}

pub fn find_document_symbols2(
    document: &TextDocument,
    html_document: &HtmlDocument,
) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();
    for &root in html_document.roots() {
        provide_file_symbols_internal(document, html_document, root, &mut symbols);
    }
    symbols
}

fn provide_file_symbols_internal(
    document: &TextDocument,
    html_document: &HtmlDocument,
    node_id: NodeId,
    symbols: &mut Vec<DocumentSymbol>,
) {
    let node = html_document.node(node_id);
    let name = node_to_name(node);
    let range = Range {
        start: document.position_at(node.start),
        end: document.position_at(node.end),
    };
    let mut symbol = DocumentSymbol::create(name, SymbolKind::Field, range, range, None);

    let children = html_document.children(node_id).to_vec();
    for child in children {
        let mut child_symbols = Vec::new();
        provide_file_symbols_internal(document, html_document, child, &mut child_symbols);
        symbol
            .children
            .get_or_insert_with(Vec::new)
            .extend(child_symbols);
    }

    symbols.push(symbol);
}

fn node_to_name(node: &Node) -> String {
    let mut name = node.tag.clone().unwrap_or_default();

    let id = node.get_attribute("id");
    let classes = node.get_attribute("class");

    if let Some(Some(id)) = id
        && !id.is_empty()
    {
        name.push('#');
        name.push_str(&strip_quotes(id));
    }

    if let Some(Some(classes)) = classes
        && !classes.is_empty()
    {
        let stripped = strip_quotes(classes);
        for class_name in split_whitespace_runs(&stripped) {
            name.push('.');
            name.push_str(class_name);
        }
    }

    if name.is_empty() {
        "?".to_string()
    } else {
        name
    }
}
