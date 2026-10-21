//! Layered server settings.
//!
//! Layers, from lowest to highest precedence: configuration files
//! (`.html.toml`), `initializationOptions`, `workspace/configuration` and
//! `workspace/didChangeConfiguration`.
//!
//! Each layer accepts either a flat object or the VS Code style
//! `{ "html": { ... } }` wrapper (the wrapper wins when present).

use std::collections::HashMap;

use html_languageservice::{
    AttributeDefaultValue, CompletionConfiguration, HTMLDataV1, HTMLFormatConfiguration,
    IHtmlDataProvider, new_html_data_provider,
};
use serde_json::Value;

use crate::config::LoadedConfig;
use crate::protocol::FormattingOptions;

/// Partially specified completion settings, so layers can be merged.
#[derive(Debug, Clone, Default)]
pub struct CompletionSettings {
    pub hide_auto_complete_proposals: Option<bool>,
    pub hide_end_tag_suggestions: Option<bool>,
    pub attribute_default_value: Option<AttributeDefaultValue>,
    pub provider_settings: HashMap<String, bool>,
}

impl CompletionSettings {
    fn from_value(value: Option<&Value>) -> Self {
        let Some(object) = value.and_then(Value::as_object) else {
            return CompletionSettings::default();
        };
        let mut settings = CompletionSettings::default();
        for (key, value) in object {
            match key.as_str() {
                "attributeDefaultValue" => {
                    settings.attribute_default_value = serde_json::from_value(value.clone()).ok();
                }
                "hideAutoCompleteProposals" => {
                    settings.hide_auto_complete_proposals = value.as_bool();
                }
                "hideEndTagSuggestions" => {
                    settings.hide_end_tag_suggestions = value.as_bool();
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

    fn merge(&mut self, high: &CompletionSettings) {
        self.hide_auto_complete_proposals = high
            .hide_auto_complete_proposals
            .or(self.hide_auto_complete_proposals);
        self.hide_end_tag_suggestions = high
            .hide_end_tag_suggestions
            .or(self.hide_end_tag_suggestions);
        self.attribute_default_value = high
            .attribute_default_value
            .or(self.attribute_default_value);
        self.provider_settings.extend(
            high.provider_settings
                .iter()
                .map(|(id, on)| (id.clone(), *on)),
        );
    }

    /// Fills the unset values with their defaults.
    pub fn resolve(&self) -> CompletionConfiguration {
        CompletionConfiguration {
            provider_settings: self.provider_settings.clone(),
            hide_auto_complete_proposals: self.hide_auto_complete_proposals.unwrap_or(false),
            hide_end_tag_suggestions: self.hide_end_tag_suggestions.unwrap_or(false),
            attribute_default_value: self.attribute_default_value,
        }
    }
}

/// One merged configuration layer.
#[derive(Debug, Clone, Default)]
pub struct Settings {
    pub format: HTMLFormatConfiguration,
    pub completion: CompletionSettings,
    pub custom_data: Vec<HTMLDataV1>,
    pub use_default_data_provider: Option<bool>,
}

impl Settings {
    /// Parses `initializationOptions` / `didChangeConfiguration` settings.
    pub fn from_value(value: &Value) -> Self {
        let section = value.get("html").unwrap_or(value);
        Settings {
            format: section
                .get("format")
                .and_then(|value| serde_json::from_value(value.clone()).ok())
                .unwrap_or_default(),
            completion: CompletionSettings::from_value(section.get("completion")),
            custom_data: inline_data(section),
            use_default_data_provider: section
                .get("useDefaultDataProvider")
                .and_then(Value::as_bool),
        }
    }

    /// Builds the layer contributed by a `.html.toml` file, resolving
    /// `custom_data` paths relative to the file.
    pub fn from_config_file(file: &LoadedConfig) -> (Settings, Vec<String>) {
        let mut settings = Settings::from_value(&file.value);
        let mut warnings = file.warnings.clone();
        for path in &file.custom_data_paths {
            let resolved = file.resolve(path);
            match std::fs::read_to_string(&resolved) {
                Ok(text) => match serde_json::from_str::<HTMLDataV1>(&text) {
                    Ok(data) => settings.custom_data.push(data),
                    Err(error) => warnings.push(format!(
                        "invalid custom data `{}`: {error}",
                        resolved.display()
                    )),
                },
                Err(error) => warnings.push(format!(
                    "cannot read custom data `{}`: {error}",
                    resolved.display()
                )),
            }
        }
        (settings, warnings)
    }

    /// `high` wins; completion providers and custom data are combined.
    pub fn merge(low: Settings, high: Settings) -> Settings {
        let mut completion = low.completion;
        completion.merge(&high.completion);
        Settings {
            format: merge_format(low.format, high.format),
            completion,
            custom_data: low
                .custom_data
                .into_iter()
                .chain(high.custom_data)
                .collect(),
            use_default_data_provider: high
                .use_default_data_provider
                .or(low.use_default_data_provider),
        }
    }

    pub fn use_default_data_provider(&self) -> bool {
        self.use_default_data_provider.unwrap_or(true)
    }

    pub fn completion(&self) -> CompletionConfiguration {
        self.completion.resolve()
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

fn inline_data(section: &Value) -> Vec<HTMLDataV1> {
    section
        .get("customData")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| serde_json::from_value(entry.clone()).ok())
                .collect()
        })
        .unwrap_or_default()
}

macro_rules! merge_format_fields {
    ($low:expr, $high:expr, $($field:ident),+ $(,)?) => {
        HTMLFormatConfiguration { $($field: $high.$field.or($low.$field)),+ }
    };
}

fn merge_format(
    low: HTMLFormatConfiguration,
    high: HTMLFormatConfiguration,
) -> HTMLFormatConfiguration {
    merge_format_fields!(
        low,
        high,
        tab_size,
        insert_spaces,
        indent_empty_lines,
        wrap_line_length,
        unformatted,
        content_unformatted,
        indent_inner_html,
        wrap_attributes,
        wrap_attributes_indent_size,
        preserve_new_lines,
        max_preserve_new_lines,
        indent_handlebars,
        end_with_newline,
        extra_liners,
        indent_scripts,
        templating,
        unformatted_content_delimiter,
        css,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use html_languageservice::TemplatingFormatOption;
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
            Some(TemplatingFormatOption::List(vec![
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
        assert!(settings.completion.hide_end_tag_suggestions.unwrap());
        let completion = settings.completion();
        assert!(!completion.provider_enabled("html5"));
        assert!(!completion.provider_enabled("svg"));
        assert!(completion.provider_enabled("htmlentities"));
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

    #[test]
    fn merge_prefers_the_higher_layer() {
        let low = Settings::from_value(&json!({
            "useDefaultDataProvider": false,
            "format": { "wrapLineLength": 80, "tabSize": 8 },
            "completion": { "hideEndTagSuggestions": true, "html5": false },
            "customData": [{ "version": 1.1, "tags": [{ "name": "low" }] }]
        }));
        let high = Settings::from_value(&json!({
            "format": { "tabSize": 2 },
            "completion": { "hideEndTagSuggestions": false, "svg": false },
            "customData": [{ "version": 1.1, "tags": [{ "name": "high" }] }]
        }));
        let merged = Settings::merge(low, high);
        assert_eq!(merged.format.wrap_line_length, Some(80));
        assert_eq!(merged.format.tab_size, Some(2));
        assert_eq!(merged.completion.hide_end_tag_suggestions, Some(false));
        assert_eq!(merged.custom_data.len(), 2);
        assert!(!merged.use_default_data_provider());
        let completion = merged.completion();
        assert!(!completion.provider_enabled("html5"));
        assert!(!completion.provider_enabled("svg"));
    }

    #[test]
    fn reads_custom_data_from_relative_paths() {
        let dir = std::env::temp_dir().join(format!("html-ls-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("data")).unwrap();
        std::fs::write(
            dir.join("data/tags.json"),
            r#"{"version":1.1,"tags":[{"name":"file-tag"}]}"#,
        )
        .unwrap();

        let config_path = dir.join(".html.toml");
        std::fs::write(
            &config_path,
            "custom_data = [\"./data/tags.json\", \"./data/missing.json\"]\n",
        )
        .unwrap();

        let loaded = crate::config::load_explicit(&config_path).unwrap();
        let (settings, warnings) = Settings::from_config_file(&loaded);
        assert_eq!(settings.custom_data.len(), 1);
        assert_eq!(
            settings.data_providers()[0].provide_tags()[0].name,
            "file-tag"
        );
        assert_eq!(warnings.len(), 1);

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
