//! Port of `src/languageFacts/dataManager.ts`.

use crate::language_facts::data_provider::{HtmlDataProvider, html_data};
use crate::parser::html_parser::is_void_element as is_void_element_in;
use crate::types::IHtmlDataProvider;

/// Selected from https://stackoverflow.com/a/2725168/1780148
const PATH_TAG_AND_ATTR: &[(&str, &[&str])] = &[
    // HTML 4
    ("a", &["href"]),
    ("area", &["href"]),
    ("body", &["background"]),
    ("blockquote", &["cite"]),
    ("del", &["cite"]),
    ("form", &["action"]),
    ("frame", &["src", "longdesc"]),
    ("img", &["src", "longdesc"]),
    ("ins", &["cite"]),
    ("link", &["href"]),
    ("object", &["data"]),
    ("q", &["cite"]),
    ("script", &["src"]),
    // HTML 5
    ("audio", &["src"]),
    ("button", &["formaction"]),
    ("command", &["icon"]),
    ("embed", &["src"]),
    ("html", &["manifest"]),
    ("input", &["src", "formaction"]),
    ("source", &["src"]),
    ("track", &["src"]),
    ("video", &["src", "poster"]),
];

pub struct DataManager {
    data_providers: Vec<Box<dyn IHtmlDataProvider>>,
}

impl DataManager {
    pub fn new(
        use_default_data_provider: bool,
        providers: Vec<Box<dyn IHtmlDataProvider>>,
    ) -> Self {
        Self {
            data_providers: Self::build(use_default_data_provider, providers),
        }
    }

    fn build(
        built_in: bool,
        providers: Vec<Box<dyn IHtmlDataProvider>>,
    ) -> Vec<Box<dyn IHtmlDataProvider>> {
        let mut data_providers: Vec<Box<dyn IHtmlDataProvider>> = Vec::new();
        if built_in {
            data_providers.push(Box::new(HtmlDataProvider::new("html5", html_data())));
        }
        data_providers.extend(providers);
        data_providers
    }

    pub fn set_data_providers(
        &mut self,
        built_in: bool,
        providers: Vec<Box<dyn IHtmlDataProvider>>,
    ) {
        self.data_providers = Self::build(built_in, providers);
    }

    pub fn get_data_providers(&self) -> Vec<&dyn IHtmlDataProvider> {
        self.data_providers.iter().map(|p| p.as_ref()).collect()
    }

    pub fn is_void_element(&self, e: &str, void_elements: &[String]) -> bool {
        is_void_element_in(e, void_elements)
    }

    pub fn get_void_elements(&self, language_id: &str) -> Vec<String> {
        let providers: Vec<&dyn IHtmlDataProvider> = self
            .data_providers
            .iter()
            .map(|p| p.as_ref())
            .filter(|p| p.is_applicable(language_id))
            .collect();
        self.get_void_elements_for(&providers)
    }

    pub fn get_void_elements_for(&self, data_providers: &[&dyn IHtmlDataProvider]) -> Vec<String> {
        let mut void_tags: Vec<String> = Vec::new();
        for provider in data_providers {
            for tag in provider.provide_tags() {
                if tag.void == Some(true) {
                    void_tags.push(tag.name.clone());
                }
            }
        }
        void_tags.sort();
        void_tags
    }

    pub fn is_path_attribute(&self, tag: &str, attr: &str) -> bool {
        if attr == "src" || attr == "href" {
            return true;
        }
        if let Some((_, attrs)) = PATH_TAG_AND_ATTR.iter().find(|(t, _)| *t == tag) {
            return attrs.contains(&attr);
        }
        false
    }
}
