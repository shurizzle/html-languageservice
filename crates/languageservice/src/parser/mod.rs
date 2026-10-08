//! Port of `src/parser/*.ts`.

pub mod html_entities;
pub mod html_parser;
pub mod html_scanner;

pub use html_parser::{HtmlDocument, Node, NodeId, ParseOptions, parse};
pub use html_scanner::{Scanner, convert_script_content_to_javascript, create_scanner};
