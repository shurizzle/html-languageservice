//! Discovery and parsing of `.html.toml` configuration files.
//!
//! Keys are written in `snake_case`; they are normalized to the `camelCase`
//! names the settings layer expects, so both spellings work. Sub-trees that
//! carry user data (`customData`, `customDataInline`) are left untouched.
//!
//! ```toml
//! use_default_data_provider = true
//!
//! [format]
//! wrap_line_length = 80
//! unformatted = ["code", "pre"]
//!
//! [completion]
//! hide_end_tag_suggestions = true
//!
//! custom_data = ["./data/web-components.html-data.json"]
//! ```

use std::fmt;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// Name looked up in every workspace root.
pub const CONFIG_FILE_NAME: &str = ".html.toml";
/// Overrides the per-user configuration path.
pub const CONFIG_ENV: &str = "HTML_LS_CONFIG";

/// A parsed configuration file.
#[derive(Debug, Clone)]
pub struct LoadedConfig {
    pub path: PathBuf,
    /// Normalized settings object (`camelCase` keys, `customData` holds inline
    /// data only).
    pub value: Value,
    /// Entries of `custom_data`, relative to the file's directory.
    pub custom_data_paths: Vec<String>,
    pub warnings: Vec<String>,
}

impl LoadedConfig {
    /// Directory relative `custom_data` paths are resolved against.
    pub fn base_dir(&self) -> &Path {
        self.path.parent().unwrap_or_else(|| Path::new("."))
    }

    /// Resolves a `custom_data` entry against the file's directory.
    pub fn resolve(&self, path: &str) -> PathBuf {
        let candidate = Path::new(path);
        if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            self.base_dir().join(candidate)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError(String);

impl ConfigError {
    fn new(message: impl Into<String>) -> ConfigError {
        ConfigError(message.into())
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ConfigError {}

/// Parse one file; a missing or malformed file is an error.
pub fn load_explicit(path: &Path) -> Result<LoadedConfig, ConfigError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| ConfigError::new(format!("cannot read {}: {e}", path.display())))?;
    let raw: toml::Value = toml::from_str(&text)
        .map_err(|e| ConfigError::new(format!("invalid TOML in {}: {e}", path.display())))?;

    let mut value = toml_to_json(raw);
    normalize_keys(&mut value);

    let mut custom_data_paths = Vec::new();
    let mut warnings = Vec::new();
    let mut inline_data = Vec::new();

    if let Some(object) = value.as_object_mut() {
        if let Some(entries) = object.remove("customData") {
            match entries {
                Value::Array(items) => {
                    for item in items {
                        match item {
                            Value::String(path) => custom_data_paths.push(path),
                            other => warnings.push(format!(
                                "{}: ignoring `custom_data` entry that is not a path: {other}",
                                path.display()
                            )),
                        }
                    }
                }
                other => warnings.push(format!(
                    "{}: `custom_data` must be an array of paths, got {other}",
                    path.display()
                )),
            }
        }
        if let Some(entries) = object.remove("customDataInline") {
            match entries {
                Value::Array(items) => inline_data = items,
                other => warnings.push(format!(
                    "{}: `custom_data_inline` must be an array of tables, got {other}",
                    path.display()
                )),
            }
        }
        if !inline_data.is_empty() {
            object.insert("customData".to_string(), Value::Array(inline_data));
        }
    }

    Ok(LoadedConfig {
        path: path.to_path_buf(),
        value,
        custom_data_paths,
        warnings,
    })
}

/// `.html.toml` in each root, in order. Malformed files become warnings.
pub fn discover(roots: &[PathBuf]) -> (Vec<LoadedConfig>, Vec<String>) {
    let mut files = Vec::new();
    let mut warnings = Vec::new();
    for root in roots {
        let candidate = root.join(CONFIG_FILE_NAME);
        if !candidate.is_file() {
            continue;
        }
        match load_explicit(&candidate) {
            Ok(loaded) => files.push(loaded),
            Err(error) => warnings.push(error.to_string()),
        }
    }
    (files, warnings)
}

/// Per-user configuration file, if one exists.
pub fn user_config_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(CONFIG_ENV) {
        return Some(PathBuf::from(path));
    }
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
    }?;
    let path = base.join("html-languageserver").join("config.toml");
    path.is_file().then_some(path)
}

fn toml_to_json(value: toml::Value) -> Value {
    match value {
        toml::Value::String(text) => Value::String(text),
        toml::Value::Integer(number) => Value::Number(number.into()),
        toml::Value::Float(number) => serde_json::Number::from_f64(number)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        toml::Value::Boolean(boolean) => Value::Bool(boolean),
        toml::Value::Datetime(datetime) => Value::String(datetime.to_string()),
        toml::Value::Array(items) => Value::Array(items.into_iter().map(toml_to_json).collect()),
        toml::Value::Table(table) => Value::Object(
            table
                .into_iter()
                .map(|(key, value)| (key, toml_to_json(value)))
                .collect(),
        ),
    }
}

fn normalize_keys(value: &mut Value) {
    match value {
        Value::Object(object) => {
            let keys: Vec<String> = object.keys().cloned().collect();
            for key in keys {
                let Some(mut child) = object.remove(&key) else {
                    continue;
                };
                let normalized = camel_case(&key);
                if !matches!(normalized.as_str(), "customData" | "customDataInline") {
                    normalize_keys(&mut child);
                }
                object.insert(normalized, child);
            }
        }
        Value::Array(items) => {
            for item in items {
                normalize_keys(item);
            }
        }
        _ => {}
    }
}

fn camel_case(key: &str) -> String {
    let mut result = String::with_capacity(key.len());
    for (index, part) in key.split('_').filter(|part| !part.is_empty()).enumerate() {
        if index == 0 {
            result.push_str(part);
            continue;
        }
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            result.extend(first.to_uppercase());
            result.push_str(chars.as_str());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("html-ls-config-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn loads_toml_and_normalizes_keys() {
        let dir = temp_dir("loads");
        let path = dir.join(CONFIG_FILE_NAME);
        std::fs::write(
            &path,
            "use_default_data_provider = false\n\
             custom_data = [\"./data/tags.json\"]\n\
             [[custom_data_inline]]\n\
             version = 1.1\n\
             tags = [{ name = \"my-tag\" }]\n\
             [format]\n\
             wrap_line_length = 80\n\
             unformatted = [\"code\", \"pre\"]\n\
             [format.css]\n\
             newline_between_rules = false\n\
             [completion]\n\
             hide_end_tag_suggestions = true\n",
        )
        .unwrap();

        let loaded = load_explicit(&path).unwrap();
        assert_eq!(loaded.custom_data_paths, vec!["./data/tags.json"]);
        assert_eq!(loaded.value["useDefaultDataProvider"], json!(false));
        assert_eq!(loaded.value["format"]["wrapLineLength"], json!(80));
        assert_eq!(
            loaded.value["format"]["unformatted"],
            json!(["code", "pre"])
        );
        assert_eq!(
            loaded.value["format"]["css"]["newlineBetweenRules"],
            json!(false)
        );
        assert_eq!(
            loaded.value["completion"]["hideEndTagSuggestions"],
            json!(true)
        );
        // Inline data is kept verbatim under `customData`.
        assert_eq!(loaded.value["customData"][0]["tags"][0]["name"], "my-tag");
        assert!(loaded.warnings.is_empty());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rejects_invalid_toml() {
        let dir = temp_dir("invalid");
        let path = dir.join(CONFIG_FILE_NAME);
        std::fs::write(&path, "this is not toml =").unwrap();
        assert!(load_explicit(&path).is_err());
        assert!(load_explicit(&dir.join("missing.toml")).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn discovers_files_in_root_order() {
        let first = temp_dir("discover-one");
        let second = temp_dir("discover-two");
        std::fs::write(
            second.join(CONFIG_FILE_NAME),
            "use_default_data_provider = true",
        )
        .unwrap();

        let (files, warnings) = discover(&[first.clone(), second.clone()]);
        assert!(warnings.is_empty());
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, second.join(CONFIG_FILE_NAME));

        std::fs::remove_dir_all(&first).unwrap();
        std::fs::remove_dir_all(&second).unwrap();
    }

    #[test]
    fn invalid_discovered_file_becomes_a_warning() {
        let dir = temp_dir("discover-invalid");
        std::fs::write(dir.join(CONFIG_FILE_NAME), "= nope").unwrap();

        let (files, warnings) = discover(std::slice::from_ref(&dir));
        assert!(files.is_empty());
        assert_eq!(warnings.len(), 1);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn camel_case_preserves_leading_segment() {
        assert_eq!(camel_case("wrap_line_length"), "wrapLineLength");
        assert_eq!(camel_case("hideEndTagSuggestions"), "hideEndTagSuggestions");
        assert_eq!(camel_case("css"), "css");
    }
}
