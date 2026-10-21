//! Minimal LSP parameter types (only the fields the server reads).

use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TextDocumentIdentifier {
    pub uri: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionedTextDocumentIdentifier {
    pub uri: String,
    pub version: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentItem {
    pub uri: String,
    pub language_id: String,
    pub version: i32,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DidOpenTextDocumentParams {
    pub text_document: TextDocumentItem,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TextDocumentContentChangeEvent {
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DidChangeTextDocumentParams {
    pub text_document: VersionedTextDocumentIdentifier,
    pub content_changes: Vec<TextDocumentContentChangeEvent>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentParams {
    pub text_document: TextDocumentIdentifier,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentPositionParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionRangeParams {
    pub text_document: TextDocumentIdentifier,
    pub positions: Vec<Position>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DidChangeConfigurationParams {
    #[serde(default)]
    pub settings: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormattingOptions {
    pub tab_size: u32,
    pub insert_spaces: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentFormattingParams {
    pub text_document: TextDocumentIdentifier,
    pub options: FormattingOptions,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentRangeFormattingParams {
    pub text_document: TextDocumentIdentifier,
    pub range: Range,
    pub options: FormattingOptions,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
    pub new_name: String,
}

/// Both map onto the same shape (`{ textDocument }`).
pub type DocumentSymbolParams = TextDocumentParams;
pub type DocumentLinkParams = TextDocumentParams;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct WorkspaceFolder {
    pub uri: String,
    #[serde(default)]
    pub name: String,
}

/// Subset of `InitializeParams` the server reads.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    #[serde(default)]
    pub root_uri: Option<String>,
    #[serde(default)]
    pub root_path: Option<String>,
    #[serde(default)]
    pub workspace_folders: Option<Vec<WorkspaceFolder>>,
    #[serde(default)]
    pub capabilities: ClientCapabilities,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ClientCapabilities {
    #[serde(default)]
    pub workspace: WorkspaceClientCapabilities,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceClientCapabilities {
    #[serde(default)]
    pub configuration: bool,
    #[serde(default)]
    pub did_change_watched_files: DidChangeWatchedFilesClientCapabilities,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DidChangeWatchedFilesClientCapabilities {
    #[serde(default)]
    pub dynamic_registration: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkspaceFoldersChangeEvent {
    #[serde(default)]
    pub added: Vec<WorkspaceFolder>,
    #[serde(default)]
    pub removed: Vec<WorkspaceFolder>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DidChangeWorkspaceFoldersParams {
    pub event: WorkspaceFoldersChangeEvent,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileEvent {
    pub uri: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DidChangeWatchedFilesParams {
    #[serde(default)]
    pub changes: Vec<FileEvent>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_initialize_params() {
        let params: InitializeParams = serde_json::from_value(json!({
            "rootUri": "file:///work",
            "capabilities": {
                "workspace": {
                    "configuration": true,
                    "didChangeWatchedFiles": { "dynamicRegistration": true }
                }
            }
        }))
        .unwrap();
        assert_eq!(params.root_uri.as_deref(), Some("file:///work"));
        assert!(params.capabilities.workspace.configuration);
        assert!(
            params
                .capabilities
                .workspace
                .did_change_watched_files
                .dynamic_registration
        );
    }

    #[test]
    fn initializes_from_empty_object() {
        let params: InitializeParams = serde_json::from_value(json!({})).unwrap();
        assert!(params.workspace_folders.is_none());
        assert!(!params.capabilities.workspace.configuration);
    }
}
