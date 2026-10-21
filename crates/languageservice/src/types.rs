//! Port of the LSP-facing types in `src/htmlLanguageTypes.ts`.
//!
//! The LSP types (`Position`, `Range`, `CompletionItem`, ...) stand in for
//! those imported from `vscode-languageserver-types`; the `I*Data` types plus
//! [`HTMLDataV1`] mirror the ones defined by the language service itself.

use serde::Deserialize;

/// Zero-based line/character position. `character` counts UTF-16 code units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

impl Position {
    pub fn create(line: u32, character: u32) -> Self {
        Self { line, character }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

impl Range {
    pub fn create(
        start_line: u32,
        start_character: u32,
        end_line: u32,
        end_character: u32,
    ) -> Self {
        Self {
            start: Position::create(start_line, start_character),
            end: Position::create(end_line, end_character),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub uri: String,
    pub range: Range,
}

impl Location {
    pub fn create(uri: impl Into<String>, range: Range) -> Self {
        Self {
            uri: uri.into(),
            range,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarkupKind {
    PlainText,
    Markdown,
}

impl MarkupKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            MarkupKind::PlainText => "plaintext",
            MarkupKind::Markdown => "markdown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MarkupContent {
    pub kind: MarkupKind,
    pub value: String,
}

impl MarkupContent {
    pub fn new(kind: MarkupKind, value: impl Into<String>) -> Self {
        Self {
            kind,
            value: value.into(),
        }
    }
}

/// A value that is either a plain string or a [`MarkupContent`].
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum MarkupInput {
    String(String),
    Markup(MarkupContent),
}

impl MarkupInput {
    /// Returns the [`MarkupContent`] view, wrapping plain strings as markdown.
    pub fn as_markup(&self) -> MarkupContent {
        match self {
            MarkupInput::String(s) => MarkupContent::new(MarkupKind::Markdown, s.clone()),
            MarkupInput::Markup(m) => m.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    pub range: Range,
    pub new_text: String,
}

impl TextEdit {
    pub fn replace(range: Range, new_text: impl Into<String>) -> Self {
        Self {
            range,
            new_text: new_text.into(),
        }
    }

    pub fn insert(position: Position, new_text: impl Into<String>) -> Self {
        Self::replace(
            Range {
                start: position,
                end: position,
            },
            new_text,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub title: String,
    pub command: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum InsertTextFormat {
    PlainText = 1,
    Snippet = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CompletionItemKind {
    Text = 1,
    Method = 2,
    Function = 3,
    Constructor = 4,
    Field = 5,
    Variable = 6,
    Class = 7,
    Interface = 8,
    Module = 9,
    Property = 10,
    Unit = 11,
    Value = 12,
    Enum = 13,
    Keyword = 14,
    Snippet = 15,
    Color = 16,
    File = 17,
    Reference = 18,
    Folder = 19,
    EnumMember = 20,
    Constant = 21,
    Struct = 22,
    Event = 23,
    Operator = 24,
    TypeParameter = 25,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompletionItem {
    pub label: String,
    pub kind: Option<CompletionItemKind>,
    pub documentation: Option<MarkupInput>,
    pub filter_text: Option<String>,
    pub text_edit: Option<TextEdit>,
    pub insert_text_format: Option<InsertTextFormat>,
    pub command: Option<Command>,
    pub sort_text: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompletionList {
    pub is_incomplete: bool,
    pub items: Vec<CompletionItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hover {
    pub contents: MarkupInput,
    pub range: Option<Range>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HoverSettings {
    pub documentation: Option<bool>,
    pub references: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompletionConfiguration {
    /// Port of `settings[providerId]`; only an explicit `false` disables a provider.
    pub provider_settings: std::collections::HashMap<String, bool>,
    pub hide_auto_complete_proposals: bool,
    pub hide_end_tag_suggestions: bool,
    pub attribute_default_value: Option<AttributeDefaultValue>,
}

impl CompletionConfiguration {
    /// Port of `settings[providerId] !== false`.
    pub fn provider_enabled(&self, id: &str) -> bool {
        !matches!(self.provider_settings.get(id), Some(false))
    }

    pub fn disable_provider(&mut self, id: impl Into<String>) {
        self.provider_settings.insert(id.into(), false);
    }
}

/// Port of `HTMLFormatConfiguration`'s `templating` union (`boolean | array`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum TemplatingFormatOption {
    Bool(bool),
    List(Vec<String>),
}

/// Port of `EmbeddedCSSFormatConfiguration`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EmbeddedCSSFormatConfiguration {
    pub newline_between_selectors: Option<bool>,
    pub newline_between_rules: Option<bool>,
    pub space_around_selector_separator: Option<bool>,
    pub brace_style: Option<String>,
    pub preserve_new_lines: Option<bool>,
    pub max_preserve_new_lines: Option<i32>,
}

/// Port of `HTMLFormatConfiguration`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HTMLFormatConfiguration {
    pub tab_size: Option<i32>,
    pub insert_spaces: Option<bool>,
    pub indent_empty_lines: Option<bool>,
    pub wrap_line_length: Option<i32>,
    #[serde(deserialize_with = "deserialize_optional_string_or_list")]
    pub unformatted: Option<String>,
    #[serde(deserialize_with = "deserialize_optional_string_or_list")]
    pub content_unformatted: Option<String>,
    pub indent_inner_html: Option<bool>,
    pub wrap_attributes: Option<String>,
    pub wrap_attributes_indent_size: Option<i32>,
    pub preserve_new_lines: Option<bool>,
    pub max_preserve_new_lines: Option<i32>,
    pub indent_handlebars: Option<bool>,
    pub end_with_newline: Option<bool>,
    #[serde(deserialize_with = "deserialize_optional_string_or_list")]
    pub extra_liners: Option<String>,
    pub indent_scripts: Option<String>,
    pub templating: Option<TemplatingFormatOption>,
    pub unformatted_content_delimiter: Option<String>,
    pub css: Option<EmbeddedCSSFormatConfiguration>,
}

/// `string | string[]`, stored comma-separated as the formatter expects.
#[derive(Deserialize)]
#[serde(untagged)]
enum StringOrList {
    String(String),
    List(Vec<String>),
}

fn deserialize_optional_string_or_list<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<StringOrList>::deserialize(deserializer)?;
    Ok(value.map(|value| match value {
        StringOrList::String(text) => text,
        StringOrList::List(list) => list.join(","),
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttributeDefaultValue {
    Empty,
    SingleQuotes,
    DoubleQuotes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum DocumentHighlightKind {
    Text = 1,
    Read = 2,
    Write = 3,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentHighlight {
    pub range: Range,
    pub kind: DocumentHighlightKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentLink {
    pub range: Range,
    pub target: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldingRangeKind {
    Comment,
    Imports,
    Region,
}

impl FoldingRangeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            FoldingRangeKind::Comment => "comment",
            FoldingRangeKind::Imports => "imports",
            FoldingRangeKind::Region => "region",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldingRange {
    pub start_line: u32,
    pub end_line: u32,
    pub kind: Option<FoldingRangeKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SymbolKind {
    File = 1,
    Module = 2,
    Namespace = 3,
    Package = 4,
    Class = 5,
    Method = 6,
    Property = 7,
    Field = 8,
    Constructor = 9,
    Enum = 10,
    Interface = 11,
    Function = 12,
    Variable = 13,
    Constant = 14,
    String = 15,
    Number = 16,
    Boolean = 17,
    Array = 18,
    Object = 19,
    Key = 20,
    Null = 21,
    EnumMember = 22,
    Struct = 23,
    Event = 24,
    Operator = 25,
    TypeParameter = 26,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolInformation {
    pub name: String,
    pub kind: SymbolKind,
    pub location: Location,
    pub container_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSymbol {
    pub name: String,
    pub detail: Option<String>,
    pub kind: SymbolKind,
    pub range: Range,
    pub selection_range: Range,
    pub children: Option<Vec<DocumentSymbol>>,
}

impl DocumentSymbol {
    pub fn create(
        name: impl Into<String>,
        kind: SymbolKind,
        range: Range,
        selection_range: Range,
        children: Option<Vec<DocumentSymbol>>,
    ) -> Self {
        Self {
            name: name.into(),
            detail: None,
            kind,
            range,
            selection_range,
            children,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionRange {
    pub range: Range,
    pub parent: Option<Box<SelectionRange>>,
}

impl SelectionRange {
    pub fn create(range: Range, parent: Option<SelectionRange>) -> Self {
        Self {
            range,
            parent: parent.map(Box::new),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceEdit {
    pub changes: Vec<(String, Vec<TextEdit>)>,
}

impl WorkspaceEdit {
    pub fn edits_for(&self, uri: &str) -> Option<&Vec<TextEdit>> {
        self.changes.iter().find(|(u, _)| u == uri).map(|(_, e)| e)
    }
}

/// Port of the `DocumentContext` interface.
pub trait DocumentContext {
    fn resolve_reference(&self, reference: &str, base: &str) -> Option<String>;
}

#[derive(Debug, Clone, Default)]
pub struct CompletionItemClientCapabilities {
    pub documentation_format: Vec<MarkupKind>,
}

#[derive(Debug, Clone, Default)]
pub struct CompletionClientCapabilities {
    pub completion_item: CompletionItemClientCapabilities,
}

#[derive(Debug, Clone, Default)]
pub struct HoverClientCapabilities {
    pub content_format: Vec<MarkupKind>,
}

#[derive(Debug, Clone, Default)]
pub struct TextDocumentClientCapabilities {
    pub completion: CompletionClientCapabilities,
    pub hover: HoverClientCapabilities,
}

/// Describes what LSP capabilities the client supports.
#[derive(Debug, Clone, Default)]
pub struct ClientCapabilities {
    pub text_document: TextDocumentClientCapabilities,
}

impl ClientCapabilities {
    pub fn latest() -> Self {
        Self {
            text_document: TextDocumentClientCapabilities {
                completion: CompletionClientCapabilities {
                    completion_item: CompletionItemClientCapabilities {
                        documentation_format: vec![MarkupKind::Markdown, MarkupKind::PlainText],
                    },
                },
                hover: HoverClientCapabilities {
                    content_format: vec![MarkupKind::Markdown, MarkupKind::PlainText],
                },
            },
        }
    }
}

/// Port of the `FileType` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum FileType {
    Unknown = 0,
    File = 1,
    Directory = 2,
    SymbolicLink = 64,
}

/// Port of the `FileSystemProvider` interface.
///
/// The upstream `readDirectory` is asynchronous; this crate is synchronous,
/// so the callback returns the directory entries directly (or an empty vector
/// to model a rejected promise).
pub trait FileSystemProvider: Send + Sync {
    fn read_directory(&self, uri: &str) -> Vec<(String, FileType)>;
}

/// Port of the `LanguageServiceOptions` interface.
pub struct LanguageServiceOptions {
    /// Unless set to `false`, the default HTML data provider is used alongside
    /// `custom_data_providers`. Defaults to `true`.
    pub use_default_data_provider: bool,
    /// Additional tag/attribute/attribute-value data providers.
    pub custom_data_providers: Vec<Box<dyn IHtmlDataProvider>>,
    /// Abstract file system access, used for path completion.
    pub file_system_provider: Option<std::sync::Arc<dyn FileSystemProvider>>,
    /// Describes the LSP capabilities the client supports.
    pub client_capabilities: Option<ClientCapabilities>,
}

impl Default for LanguageServiceOptions {
    fn default() -> Self {
        Self {
            use_default_data_provider: true,
            custom_data_providers: Vec::new(),
            file_system_provider: None,
            client_capabilities: None,
        }
    }
}

impl std::fmt::Debug for LanguageServiceOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LanguageServiceOptions")
            .field("use_default_data_provider", &self.use_default_data_provider)
            .field("custom_data_providers", &self.custom_data_providers.len())
            .field("file_system_provider", &self.file_system_provider.is_some())
            .field("client_capabilities", &self.client_capabilities)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct HtmlAttributeValueContext {
    pub position: Position,
    pub tag: String,
    pub attribute: String,
    pub value: String,
    pub range: Range,
    pub attributes: Vec<(String, Option<String>)>,
}

#[derive(Debug, Clone)]
pub struct HtmlContentContext {
    pub position: Position,
}

/// Port of the `ICompletionParticipant` interface.
pub trait ICompletionParticipant {
    fn on_html_attribute_value(&self, _context: &HtmlAttributeValueContext) {}
    fn on_html_content(&self, _context: &HtmlContentContext) {}
}

// ---------------------------------------------------------------------------
// Custom data (`IHTMLDataProvider` input)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct IReference {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum Baseline {
    Bool(bool),
    Str(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BaselineStatus {
    pub baseline: Baseline,
    #[serde(default)]
    pub baseline_low_date: Option<String>,
    #[serde(default)]
    pub baseline_high_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct IValueData {
    pub name: String,
    #[serde(default)]
    pub description: Option<MarkupInput>,
    #[serde(default)]
    pub references: Option<Vec<IReference>>,
    #[serde(default)]
    pub browsers: Option<Vec<String>>,
    #[serde(default)]
    pub status: Option<BaselineStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct IAttributeData {
    pub name: String,
    #[serde(default)]
    pub description: Option<MarkupInput>,
    #[serde(default, rename = "valueSet")]
    pub value_set: Option<String>,
    #[serde(default)]
    pub values: Option<Vec<IValueData>>,
    #[serde(default)]
    pub references: Option<Vec<IReference>>,
    #[serde(default)]
    pub browsers: Option<Vec<String>>,
    #[serde(default)]
    pub status: Option<BaselineStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct ITagData {
    pub name: String,
    #[serde(default)]
    pub description: Option<MarkupInput>,
    #[serde(default)]
    pub attributes: Vec<IAttributeData>,
    #[serde(default)]
    pub references: Option<Vec<IReference>>,
    #[serde(default)]
    pub void: Option<bool>,
    #[serde(default)]
    pub browsers: Option<Vec<String>>,
    #[serde(default)]
    pub status: Option<BaselineStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct IValueSet {
    pub name: String,
    pub values: Vec<IValueData>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct HTMLDataV1 {
    #[serde(default)]
    pub version: f64,
    #[serde(default)]
    pub tags: Vec<ITagData>,
    #[serde(default, rename = "globalAttributes")]
    pub global_attributes: Vec<IAttributeData>,
    #[serde(default, rename = "valueSets")]
    pub value_sets: Vec<IValueSet>,
}

/// Port of the `IHTMLDataProvider` interface.
pub trait IHtmlDataProvider {
    fn id(&self) -> &str;
    fn is_applicable(&self, language_id: &str) -> bool;
    fn provide_tags(&self) -> &[ITagData];
    fn provide_attributes(&self, tag: &str) -> Vec<IAttributeData>;
    fn provide_values(&self, tag: &str, attribute: &str) -> Vec<IValueData>;
}
