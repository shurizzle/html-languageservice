//! Port of `src/services/*.ts`.

pub mod html_completion;
pub mod html_folding;
pub mod html_formatter;
pub mod html_highlighting;
pub mod html_hover;
pub mod html_linked_editing;
pub mod html_links;
pub mod html_matching_tag_position;
pub mod html_rename;
pub mod html_selection_range;
pub mod html_symbols_provider;
pub mod path_completion;

pub use html_completion::HtmlCompletion;
pub use html_folding::HtmlFolding;
pub use html_highlighting::find_document_highlights;
pub use html_hover::HtmlHover;
pub use html_linked_editing::find_linked_editing_ranges;
pub use html_links::HtmlDocumentLinks;
pub use html_matching_tag_position::find_matching_tag_position;
pub use html_rename::do_rename;
pub use html_selection_range::HtmlSelectionRange;
pub use html_symbols_provider::{find_document_symbols, find_document_symbols2};
pub use path_completion::PathCompletionParticipant;
