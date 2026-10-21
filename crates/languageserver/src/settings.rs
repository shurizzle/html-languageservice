//! Server settings, read from `initializationOptions` and
//! `workspace/didChangeConfiguration`.
//!
//! Accepts either a flat object or the VS Code style `{ "html": { ... } }`
//! wrapper (the wrapper wins when present):
//!
//! ```json
//! {
//!   "html": {
//!     "format": { "wrapLineLength": 80, "unformatted": ["code", "pre"] },
//!     "completion": {
//!       "attributeDefaultValue": "doublequotes",
//!       "hideEndTagSuggestions": true,
//!       "html5": false
//!     },
//!     "customData": [{ "version": 1.1, "tags": [] }]
//!   }
//! }
//! ```
//!
//! `customData` entries are inline `HTMLDataV1` objects; file paths (as used by
//! VS Code) are not resolved by the server.

use html_languageservice::{
    CompletionConfiguration, HTMLDataV1, HTMLFormatConfiguration, IHtmlDataProvider,
    new_html_data_provider,
};
use serde_json::Value;

use crate::protocol::FormattingOptions;

#[derive(Debug, Clone, Default)]
pub struct Settings {
    pub format: HTMLFormatConfiguration,
    pub completion: CompletionConfiguration,
    pub custom_data: Vec<HTMLDataV1>,
}

impl Settings {
    /// Parses initialization options / configuration settings.
    ///
    /// A `format` section with a wrong value type is ignored as a whole.
    pub fn from_value(value: &Value) -> Self {
        let section = value.get("html").unwrap_or(value);
        Self {
            format: section
                .get("format")
                .and_then(|value| serde_json::from_value(value.clone()).ok())
                .unwrap_or_default(),
            completion: parse_completion(section.get("completion")),
            custom_data: section
                .get("customData")
                .and_then(Value::as_array)
                .map(|entries| {
                    entries
                        .iter()
                        .filter_map(|entry| serde_json::from_value(entry.clone()).ok())
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    pub fn data_providers(&self) -> Vec<Box<dyn IHtmlDataProvider>> {
        self.custom_data
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, data)| {
                Box::new(new_html_data_provider(
                    &format!("customData[{index}]"),
                    data,
                )) as Box<dyn IHtmlDataProvider>
            })
            .collect()
    }

    /// Per-request LSP options take precedence over the `format` settings.
    pub fn format_options(&self, request: &FormattingOptions) -> HTMLFormatConfiguration {
        HTMLFormatConfiguration {
            tab_size: Some(request.tab_size as i32),
            insert_spaces: Some(request.insert_spaces),
            ..self.format.clone()
        }
    }
}

fn parse_completion(value: Option<&Value>) -> CompletionConfiguration {
    let Some(object) = value.and_then(Value::as_object) else {
        return CompletionConfiguration::default();
    };
    let mut settings = CompletionConfiguration::default();
    for (key, value) in object {
        match key.as_str() {
            "attributeDefaultValue" => {
                settings.attribute_default_value = serde_json::from_value(value.clone()).ok();
            }
            "hideAutoCompleteProposals" => {
                settings.hide_auto_complete_proposals = value.as_bool().unwrap_or(false);
            }
            "hideEndTagSuggestions" => {
                settings.hide_end_tag_suggestions = value.as_bool().unwrap_or(false);
            }
            "providers" => {
                for (id, enabled) in value.as_object().into_iter().flatten() {
                    if let Some(enabled) = enabled.as_bool() {
                        settings.provider_settings.insert(id.clone(), enabled);
                    }
                }
            }
            // Port of upstream's `settings[providerId] !== false`.
            provider => {
                if let Some(enabled) = value.as_bool() {
                    settings
                        .provider_settings
                        .insert(provider.to_string(), enabled);
                }
            }
        }
    }
    settings
}

#[cfg(test)]
mod tests {
    use super::*;
    use html_languageservice::AttributeDefaultValue;
    use serde_json::json;

    #[test]
    fn accepts_flat_and_wrapped_settings() {
        let flat = Settings::from_value(&json!({ "format": { "wrapLineLength": 80 } }));
        assert_eq!(flat.format.wrap_line_length, Some(80));

        let wrapped = Settings::from_value(&json!({
            "html": { "format": { "wrapLineLength": 100 } }
        }));
        assert_eq!(wrapped.format.wrap_line_length, Some(100));
    }

    #[test]
    fn parses_format_options() {
        let settings = Settings::from_value(&json!({
            "format": {
                "wrapLineLength": 80,
                "unformatted": ["code", "pre"],
                "templating": ["django", "handlebars"],
                "css": { "newlineBetweenRules": false }
            }
        }));
        assert_eq!(settings.format.unformatted.as_deref(), Some("code,pre"));
        assert_eq!(
            settings.format.templating,
            Some(html_languageservice::TemplatingFormatOption::List(vec![
                "django".to_string(),
                "handlebars".to_string()
            ]))
        );
        assert_eq!(
            settings
                .format
                .css
                .and_then(|css| css.newline_between_rules),
            Some(false)
        );
    }

    #[test]
    fn request_options_override_format_settings() {
        let settings = Settings::from_value(&json!({
            "format": { "tabSize": 8, "insertSpaces": false }
        }));
        let options = settings.format_options(&FormattingOptions {
            tab_size: 2,
            insert_spaces: true,
        });
        assert_eq!(options.tab_size, Some(2));
        assert_eq!(options.insert_spaces, Some(true));
    }

    #[test]
    fn parses_completion_settings() {
        let settings = Settings::from_value(&json!({
            "completion": {
                "attributeDefaultValue": "doublequotes",
                "hideEndTagSuggestions": true,
                "html5": false,
                "providers": { "svg": false }
            }
        }));
        assert_eq!(
            settings.completion.attribute_default_value,
            Some(AttributeDefaultValue::DoubleQuotes)
        );
        assert!(settings.completion.hide_end_tag_suggestions);
        assert!(!settings.completion.provider_enabled("html5"));
        assert!(!settings.completion.provider_enabled("svg"));
        assert!(settings.completion.provider_enabled("htmlentities"));
    }

    #[test]
    fn builds_custom_data_providers() {
        let settings = Settings::from_value(&json!({
            "customData": [{
                "version": 1.1,
                "tags": [{ "name": "my-tag" }]
            }]
        }));
        let providers = settings.data_providers();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].id(), "customData[0]");
        assert_eq!(providers[0].provide_tags()[0].name, "my-tag");
    }
}
