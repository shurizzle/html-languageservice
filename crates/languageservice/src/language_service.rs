//! Port of `src/htmlLanguageService.ts`: the `getLanguageService` facade.

use crate::html_language_types::ScannerState;
use crate::language_facts::data_manager::DataManager;
use crate::language_facts::data_provider::{HtmlDataProvider, html_data};
use crate::parser::html_parser::{HtmlDocument, ParseOptions, parse};
use crate::parser::html_scanner::{Scanner, create_scanner};
use crate::services::html_completion::HtmlCompletion;
use crate::services::html_folding::{FoldingRangeContext, HtmlFolding};
use crate::services::html_highlighting::find_document_highlights;
use crate::services::html_hover::HtmlHover;
use crate::services::html_links::HtmlDocumentLinks;
use crate::services::html_selection_range::HtmlSelectionRange;
use crate::services::html_symbols_provider::{find_document_symbols, find_document_symbols2};
use crate::services::{do_rename, find_linked_editing_ranges, find_matching_tag_position};
use crate::text_document::TextDocument;
use crate::types::*;

/// Port of `getLanguageService(options)`.
pub struct LanguageService {
    data_manager: DataManager,
    client_capabilities: Option<ClientCapabilities>,
    file_system_provider: Option<std::sync::Arc<dyn FileSystemProvider>>,
    participants: Vec<Box<dyn ICompletionParticipant>>,
}

/// Port of `getLanguageService`.
pub fn get_language_service(options: LanguageServiceOptions) -> LanguageService {
    let data_manager = DataManager::new(
        options.use_default_data_provider,
        options.custom_data_providers,
    );
    LanguageService {
        data_manager,
        client_capabilities: options.client_capabilities,
        file_system_provider: options.file_system_provider,
        participants: Vec::new(),
    }
}

/// Port of `newHTMLDataProvider`.
pub fn new_html_data_provider(id: &str, custom_data: HTMLDataV1) -> HtmlDataProvider {
    HtmlDataProvider::new(id, std::sync::Arc::new(custom_data))
}

/// Port of `getDefaultHTMLDataProvider`.
pub fn get_default_html_data_provider() -> HtmlDataProvider {
    HtmlDataProvider::new("default", html_data())
}

impl LanguageService {
    /// Port of `setDataProviders`.
    pub fn set_data_providers(
        &mut self,
        use_default_data_provider: bool,
        providers: Vec<Box<dyn IHtmlDataProvider>>,
    ) {
        self.data_manager
            .set_data_providers(use_default_data_provider, providers);
    }

    /// Port of the `createScanner` re-export.
    pub fn create_scanner(
        &self,
        input: &str,
        initial_offset: usize,
        initial_state: ScannerState,
        emit_pseudo_close_tags: bool,
    ) -> Scanner {
        create_scanner(input, initial_offset, initial_state, emit_pseudo_close_tags)
    }

    /// Port of `parseHTMLDocument`.
    pub fn parse_html_document(&self, document: &TextDocument) -> HtmlDocument {
        self.parse_html_document_with_options(document, ParseOptions::default())
    }

    /// Port of `parseHTMLDocument(document, options)`.
    pub fn parse_html_document_with_options(
        &self,
        document: &TextDocument,
        options: ParseOptions,
    ) -> HtmlDocument {
        parse(
            document.get_text(),
            &self.data_manager.get_void_elements(document.language_id()),
            options,
        )
    }

    /// Port of `findDocumentHighlights`.
    pub fn find_document_highlights(
        &self,
        document: &TextDocument,
        position: Position,
        html_document: &HtmlDocument,
    ) -> Vec<DocumentHighlight> {
        find_document_highlights(document, position, html_document)
    }

    /// Port of `doComplete`.
    pub fn do_complete(
        &self,
        document: &TextDocument,
        position: Position,
        html_document: &HtmlDocument,
        settings: Option<&CompletionConfiguration>,
    ) -> CompletionList {
        let completion = HtmlCompletion::new(self.client_capabilities.as_ref(), &self.data_manager);
        completion.do_complete(
            document,
            position,
            html_document,
            settings,
            &self.participants,
        )
    }

    /// Port of `setCompletionParticipants`.
    pub fn set_completion_participants(
        &mut self,
        participants: Vec<Box<dyn ICompletionParticipant>>,
    ) {
        self.participants = participants;
    }

    /// Port of `doComplete2`: like [`Self::do_complete`], but additionally runs
    /// path completion using the configured [`FileSystemProvider`]. Falls back
    /// to [`Self::do_complete`] when no provider is set.
    pub fn do_complete2(
        &self,
        document: &TextDocument,
        position: Position,
        html_document: &HtmlDocument,
        document_context: &dyn DocumentContext,
        settings: Option<&CompletionConfiguration>,
    ) -> CompletionList {
        let Some(provider) = &self.file_system_provider else {
            return self.do_complete(document, position, html_document, settings);
        };

        let participant =
            crate::services::PathCompletionParticipant::new(&self.data_manager, provider.clone());

        let mut refs: Vec<&dyn ICompletionParticipant> =
            Vec::with_capacity(self.participants.len() + 1);
        refs.push(&participant);
        for p in &self.participants {
            refs.push(p.as_ref());
        }

        let completion = HtmlCompletion::new(self.client_capabilities.as_ref(), &self.data_manager);
        let result = completion.do_complete_with_participants(
            document,
            position,
            html_document,
            settings,
            &refs,
        );

        let path_result = participant.compute_completions(document, document_context);
        CompletionList {
            is_incomplete: result.is_incomplete || path_result.is_incomplete,
            items: path_result
                .items
                .into_iter()
                .chain(result.items)
                .collect(),
        }
    }

    /// Port of `doHover`.
    pub fn do_hover(
        &self,
        document: &TextDocument,
        position: Position,
        html_document: &HtmlDocument,
        options: Option<&HoverSettings>,
    ) -> Option<Hover> {
        let hover = HtmlHover::new(self.client_capabilities.as_ref(), &self.data_manager);
        hover.do_hover(document, position, html_document, options)
    }

    /// Port of `findDocumentLinks`.
    pub fn find_document_links(
        &self,
        document: &TextDocument,
        document_context: &dyn DocumentContext,
    ) -> Vec<DocumentLink> {
        let links = HtmlDocumentLinks::new(&self.data_manager);
        links.find_document_links(document, document_context)
    }

    /// Port of `findDocumentSymbols`.
    pub fn find_document_symbols(
        &self,
        document: &TextDocument,
        html_document: &HtmlDocument,
    ) -> Vec<SymbolInformation> {
        find_document_symbols(document, html_document)
    }

    /// Port of `findDocumentSymbols2`.
    pub fn find_document_symbols2(
        &self,
        document: &TextDocument,
        html_document: &HtmlDocument,
    ) -> Vec<DocumentSymbol> {
        find_document_symbols2(document, html_document)
    }

    /// Port of `getFoldingRanges`.
    pub fn get_folding_ranges(
        &self,
        document: &TextDocument,
        context: Option<&FoldingRangeContext>,
    ) -> Vec<FoldingRange> {
        let folding = HtmlFolding::new(&self.data_manager);
        folding.get_folding_ranges(document, context)
    }

    /// Port of `getSelectionRanges`.
    pub fn get_selection_ranges(
        &self,
        document: &TextDocument,
        positions: &[Position],
    ) -> Vec<SelectionRange> {
        let selection_range = HtmlSelectionRange::new(&self.data_manager);
        selection_range.get_selection_ranges(document, positions)
    }

    /// Port of `doQuoteComplete`.
    pub fn do_quote_complete(
        &self,
        document: &TextDocument,
        position: Position,
        html_document: &HtmlDocument,
        settings: Option<&CompletionConfiguration>,
    ) -> Option<String> {
        let completion = HtmlCompletion::new(self.client_capabilities.as_ref(), &self.data_manager);
        completion.do_quote_complete(document, position, html_document, settings)
    }

    /// Port of `doTagComplete`.
    pub fn do_tag_complete(
        &self,
        document: &TextDocument,
        position: Position,
        html_document: &HtmlDocument,
    ) -> Option<String> {
        let completion = HtmlCompletion::new(self.client_capabilities.as_ref(), &self.data_manager);
        completion.do_tag_complete(document, position, html_document)
    }

    /// Port of `doRename`.
    pub fn do_rename(
        &self,
        document: &TextDocument,
        position: Position,
        new_name: &str,
        html_document: &HtmlDocument,
    ) -> Option<WorkspaceEdit> {
        do_rename(document, position, new_name, html_document)
    }

    /// Port of `findMatchingTagPosition`.
    pub fn find_matching_tag_position(
        &self,
        document: &TextDocument,
        position: Position,
        html_document: &HtmlDocument,
    ) -> Option<Position> {
        find_matching_tag_position(document, position, html_document)
    }

    /// Port of `format`.
    pub fn format(
        &self,
        document: &TextDocument,
        range: Option<Range>,
        options: &HTMLFormatConfiguration,
    ) -> Vec<TextEdit> {
        crate::services::html_formatter::format(document, range, options)
    }

    /// Port of `findLinkedEditingRanges` (also exposed as `findOnTypeRenameRanges`).
    pub fn find_linked_editing_ranges(
        &self,
        document: &TextDocument,
        position: Position,
        html_document: &HtmlDocument,
    ) -> Option<Vec<Range>> {
        find_linked_editing_ranges(document, position, html_document)
    }

    /// Port of the deprecated `findOnTypeRenameRanges` alias.
    pub fn find_on_type_rename_ranges(
        &self,
        document: &TextDocument,
        position: Position,
        html_document: &HtmlDocument,
    ) -> Option<Vec<Range>> {
        self.find_linked_editing_ranges(document, position, html_document)
    }
}
