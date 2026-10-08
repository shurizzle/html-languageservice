//! Rust port of [vscode-html-languageservice](https://github.com/microsoft/vscode-html-languageservice).
//!
//! Module layout mirrors the TypeScript source under `src/`.

pub mod beautify;
pub mod html_language_types;
pub mod language_facts;
pub mod language_service;
pub mod parser;
pub mod services;
pub mod text_document;
pub mod types;
pub mod uri;
pub mod utils;

pub use language_facts::{DataManager, HtmlDataProvider};
pub use language_service::{
    LanguageService, get_default_html_data_provider, get_language_service, new_html_data_provider,
};
pub use text_document::{TextDocument, apply_edits};
pub use types::*;
