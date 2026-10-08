//! Port of `src/languageFacts/*.ts`.

pub mod data_manager;
pub mod data_provider;

pub use data_manager::DataManager;
pub use data_provider::{
    HtmlDataProvider, generate_documentation, get_default_html_data_provider,
    get_missing_baseline_browsers, html_data, normalize_markup_content,
};
