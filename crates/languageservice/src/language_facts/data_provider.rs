//! Port of `src/languageFacts/dataProvider.ts`.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::OnceLock;

use crate::types::{
    Baseline, BaselineStatus, HTMLDataV1, IAttributeData, IHtmlDataProvider, IReference, ITagData,
    IValueData, MarkupContent, MarkupInput, MarkupKind,
};

pub const BASELINE_LIMITED: &str = "data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMTgiIGhlaWdodD0iMTAiIHZpZXdCb3g9IjAgMCA1NDAgMzAwIiBmaWxsPSJub25lIiB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciPgogIDxzdHlsZT4KICAgIC5ncmF5LXNoYXBlIHsKICAgICAgZmlsbDogI0M2QzZDNjsgLyogTGlnaHQgbW9kZSAqLwogICAgfQoKICAgIEBtZWRpYSAocHJlZmVycy1jb2xvci1zY2hlbWU6IGRhcmspIHsKICAgICAgLmdyYXktc2hhcGUgewogICAgICAgIGZpbGw6ICM1NjU2NTY7IC8qIERhcmsgbW9kZSAqLwogICAgICB9CiAgICB9CiAgPC9zdHlsZT4KICA8cGF0aCBkPSJNMTUwIDBMMjQwIDkwTDIxMCAxMjBMMTIwIDMwTDE1MCAwWiIgZmlsbD0iI0YwOTQwOSIvPgogIDxwYXRoIGQ9Ik00MjAgMzBMNTQwIDE1MEw0MjAgMjcwTDM5MCAyNDBMNDgwIDE1MEwzOTAgNjBMNDIwIDMwWiIgY2xhc3M9ImdyYXktc2hhcGUiLz4KICA8cGF0aCBkPSJNMzMwIDE4MEwzMDAgMjEwTDM5MCAzMDBMNDIwIDI3MEwzMzAgMTgwWiIgZmlsbD0iI0YwOTQwOSIvPgogIDxwYXRoIGQ9Ik0xMjAgMzBMMTUwIDYwTDYwIDE1MEwxNTAgMjQwTDEyMCAyNzBMMCAxNTBMMTIwIDMwWiIgY2xhc3M9ImdyYXktc2hhcGUiLz4KICA8cGF0aCBkPSJNMzkwIDBMNDIwIDMwTDE1MCAzMDBMMTIwIDI3MEwzOTAgMFoiIGZpbGw9IiNGMDk0MDkiLz4KPC9zdmc+";
pub const BASELINE_LOW: &str = "data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMTgiIGhlaWdodD0iMTAiIHZpZXdCb3g9IjAgMCA1NDAgMzAwIiBmaWxsPSJub25lIiB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciPgogIDxzdHlsZT4KICAgIC5ibHVlLXNoYXBlIHsKICAgICAgZmlsbDogI0E4QzdGQTsgLyogTGlnaHQgbW9kZSAqLwogICAgfQoKICAgIEBtZWRpYSAocHJlZmVycy1jb2xvci1zY2hlbWU6IGRhcmspIHsKICAgICAgLmJsdWUtc2hhcGUgewogICAgICAgIGZpbGw6ICMyRDUwOUU7IC8qIERhcmsgbW9kZSAqLwogICAgICB9CiAgICB9CgogICAgLmRhcmtlci1ibHVlLXNoYXBlIHsKICAgICAgICBmaWxsOiAjMUI2RUYzOwogICAgfQoKICAgIEBtZWRpYSAocHJlZmVycy1jb2xvci1zY2hlbWU6IGRhcmspIHsKICAgICAgICAuZGFya2VyLWJsdWUtc2hhcGUgewogICAgICAgICAgICBmaWxsOiAjNDE4NUZGOwogICAgICAgIH0KICAgIH0KCiAgPC9zdHlsZT4KICA8cGF0aCBkPSJNMTUwIDBMMTgwIDMwTDE1MCA2MEwxMjAgMzBMMTUwIDBaIiBjbGFzcz0iYmx1ZS1zaGFwZSIvPgogIDxwYXRoIGQ9Ik0yMTAgNjBMMjQwIDkwTDIxMCAxMjBMMTgwIDkwTDIxMCA2MFoiIGNsYXNzPSJibHVlLXNoYXBlIi8+CiAgPHBhdGggZD0iTTQ1MCA2MEw0ODAgOTBMNDUwIDEyMEw0MjAgOTBMNDUwIDYwWiIgY2xhc3M9ImJsdWUtc2hhcGUiLz4KICA8cGF0aCBkPSJNNTEwIDEyMEw1NDAgMTUwTDUxMCAxODBMNDgwIDE1MEw1MTAgMTIwWiIgY2xhc3M9ImJsdWUtc2hhcGUiLz4KICA8cGF0aCBkPSJNNDUwIDE4MEw0ODAgMjEwTDQ1MCAyNDBMNDIwIDIxMEw0NTAgMTgwWiIgY2xhc3M9ImJsdWUtc2hhcGUiLz4KICA8cGF0aCBkPSJNMzkwIDI0MEw0MjAgMjcwTDM5MCAzMDBMMzYwIDI3MEwzOTAgMjQwWiIgY2xhc3M9ImJsdWUtc2hhcGUiLz4KICA8cGF0aCBkPSJNMzMwIDE4MEwzNjAgMjEwTDMzMCAyNDBMMzAwIDIxMEwzMzAgMTgwWiIgY2xhc3M9ImJsdWUtc2hhcGUiLz4KICA8cGF0aCBkPSJNOTAgNjBMMTIwIDkwTDkwIDEyMEw2MCA5MEw5MCA2MFoiIGNsYXNzPSJibHVlLXNoYXBlIi8+CiAgPHBhdGggZD0iTTM5MCAwTDQyMCAzMEwxNTAgMzAwTDAgMTUwTDMwIDEyMEwxNTAgMjQwTDM5MCAwWiIgY2xhc3M9ImRhcmtlci1ibHVlLXNoYXBlIi8+Cjwvc3ZnPg==";
pub const BASELINE_HIGH: &str = "data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMTgiIGhlaWdodD0iMTAiIHZpZXdCb3g9IjAgMCA1NDAgMzAwIiBmaWxsPSJub25lIiB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciPgogIDxzdHlsZT4KICAgIC5ncmVlbi1zaGFwZSB7CiAgICAgIGZpbGw6ICNDNEVFRDA7IC8qIExpZ2h0IG1vZGUgKi8KICAgIH0KCiAgICBAbWVkaWEgKHByZWZlcnMtY29sb3Itc2NoZW1lOiBkYXJrKSB7CiAgICAgIC5ncmVlbi1zaGFwZSB7CiAgICAgICAgZmlsbDogIzEyNTIyNTsgLyogRGFyayBtb2RlICovCiAgICAgIH0KICAgIH0KICA8L3N0eWxlPgogIDxwYXRoIGQ9Ik00MjAgMzBMMzkwIDYwTDQ4MCAxNTBMMzkwIDI0MEwzMzAgMTgwTDMwMCAyMTBMMzkwIDMwMEw1NDAgMTUwTDQyMCAzMFoiIGNsYXNzPSJncmVlbi1zaGFwZSIvPgogIDxwYXRoIGQ9Ik0xNTAgMEwzMCAxMjBMNjAgMTUwTDE1MCA2MEwyMTAgMTIwTDI0MCA5MEwxNTAgMFoiIGNsYXNzPSJncmVlbi1zaGFwZSIvPgogIDxwYXRoIGQ9Ik0zOTAgMEw0MjAgMzBMMTUwIDMwMEwwIDE1MEwzMCAxMjBMMTUwIDI0MEwzOTAgMFoiIGZpbGw9IiMxRUE0NDYiLz4KPC9zdmc+";

fn get_entry_baseline_image(status: Option<&BaselineStatus>) -> &'static str {
    let Some(status) = status else {
        return "";
    };
    match &status.baseline {
        Baseline::Str(s) if s == "low" => BASELINE_LOW,
        Baseline::Str(s) if s == "high" => BASELINE_HIGH,
        _ => BASELINE_LIMITED,
    }
}

fn baseline_status_text(status: Option<&BaselineStatus>, browsers: Option<&Vec<String>>) -> String {
    let Some(status) = status else {
        return String::new();
    };
    if matches!(status.baseline, Baseline::Bool(false)) {
        let missing = get_missing_baseline_browsers(browsers);
        let mut text = String::from("Limited availability across major browsers");
        if !missing.is_empty() {
            text.push_str(&format!(" (Not fully implemented in {missing})"));
        }
        return text;
    }
    let baseline_year = status
        .baseline_low_date
        .as_deref()
        .and_then(|d| d.split('-').next())
        .unwrap_or("");
    let word = if matches!(&status.baseline, Baseline::Str(s) if s == "low") {
        "Newly"
    } else {
        "Widely"
    };
    format!("{word} available across major browsers (Baseline since {baseline_year})")
}

/// Port of `browserNames`.
const BROWSER_NAMES: &[(&str, &str, &str)] = &[
    ("C", "Chrome", "desktop"),
    ("CA", "Chrome", "Android"),
    ("E", "Edge", "desktop"),
    ("FF", "Firefox", "desktop"),
    ("FFA", "Firefox", "Android"),
    ("S", "Safari", "macOS"),
    ("SM", "Safari", "iOS"),
];

const SHORT_COMPAT_ALTERNATIVES: [&str; 9] = ["E", "FFA", "FF", "SM", "S", "CA", "C", "IE", "O"];

/// Port of `shortCompatPattern = /(E|FFA|FF|SM|S|CA|C|IE|O)([\d|\.]+)?/`.
///
/// The pattern is not anchored, so the first position with any matching
/// alternative wins; at that position alternatives are tried left to right.
fn short_compat_browser(text: &str) -> Option<&'static str> {
    for start in 0..text.len() {
        if !text.is_char_boundary(start) {
            continue;
        }
        for prefix in SHORT_COMPAT_ALTERNATIVES {
            if text[start..].starts_with(prefix) {
                return Some(prefix);
            }
        }
    }
    None
}

/// Port of `getMissingBaselineBrowsers`.
pub fn get_missing_baseline_browsers(browsers: Option<&Vec<String>>) -> String {
    let Some(browsers) = browsers else {
        return String::new();
    };
    let mut missing: Vec<(&str, &str, &str)> = BROWSER_NAMES.to_vec();
    for short in browsers {
        if let Some(browser) = short_compat_browser(short) {
            missing.retain(|(id, _, _)| *id != browser);
        }
    }
    let mut order: Vec<&str> = Vec::new();
    let mut display: HashMap<&str, String> = HashMap::new();
    for (id, name, platform) in missing {
        if display.contains_key(name) || id == "E" {
            display.insert(name, name.to_string());
        } else {
            display.insert(name, format!("{name} on {platform}"));
        }
        if !order.contains(&name) {
            order.push(name);
        }
    }
    let names: Vec<String> = order.iter().map(|n| display[n].clone()).collect();
    list_format_disjunction(&names)
}

/// Port of `Intl.ListFormat("en", { style: "long", type: "disjunction" })`.
fn list_format_disjunction(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [a] => a.clone(),
        [a, b] => format!("{a} or {b}"),
        [rest @ .., last] => format!("{}, or {last}", rest.join(", ")),
    }
}

/// Port of `normalizeMarkupContent`.
pub fn normalize_markup_content(input: Option<&MarkupInput>) -> Option<MarkupContent> {
    let input = input?;
    Some(match input {
        MarkupInput::String(value) => MarkupContent::new(MarkupKind::Markdown, value.clone()),
        MarkupInput::Markup(markup) => {
            MarkupContent::new(MarkupKind::Markdown, markup.value.clone())
        }
    })
}

/// Common shape of the three documentable custom-data entries.
pub trait Documentable {
    fn description(&self) -> Option<&MarkupInput>;
    fn references(&self) -> Option<&Vec<IReference>>;
    fn browsers(&self) -> Option<&Vec<String>>;
    fn status(&self) -> Option<&BaselineStatus>;
}

macro_rules! impl_documentable {
    ($t:ty) => {
        impl Documentable for $t {
            fn description(&self) -> Option<&MarkupInput> {
                self.description.as_ref()
            }
            fn references(&self) -> Option<&Vec<IReference>> {
                self.references.as_ref()
            }
            fn browsers(&self) -> Option<&Vec<String>> {
                self.browsers.as_ref()
            }
            fn status(&self) -> Option<&BaselineStatus> {
                self.status.as_ref()
            }
        }
    };
}

impl_documentable!(ITagData);
impl_documentable!(IAttributeData);
impl_documentable!(IValueData);

/// Port of `generateDocumentation`.
pub fn generate_documentation(
    item: &impl Documentable,
    settings: Option<&crate::types::HoverSettings>,
    does_support_markdown: bool,
) -> Option<MarkupContent> {
    let documentation_enabled = settings.and_then(|s| s.documentation).unwrap_or(true);
    let references_enabled = settings.and_then(|s| s.references).unwrap_or(true);

    let mut value = String::new();

    if documentation_enabled && let Some(normalized) = normalize_markup_content(item.description())
    {
        value.push_str(&normalized.value);
    }

    if documentation_enabled && let Some(status) = item.status() {
        if !value.is_empty() {
            value.push_str("\n\n");
        }
        let text = baseline_status_text(Some(status), item.browsers());
        if does_support_markdown {
            value.push_str(&format!(
                "![Baseline icon]({}) _{text}_",
                get_entry_baseline_image(Some(status))
            ));
        } else {
            value.push_str(&text);
        }
    }

    if references_enabled
        && let Some(references) = item.references()
        && !references.is_empty()
    {
        if !value.is_empty() {
            value.push_str("\n\n");
        }
        if does_support_markdown {
            value.push_str(
                &references
                    .iter()
                    .map(|r| format!("[{}]({})", r.name, r.url))
                    .collect::<Vec<_>>()
                    .join(" | "),
            );
        } else {
            value.push_str(
                &references
                    .iter()
                    .map(|r| format!("{}: {}", r.name, r.url))
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
    }

    if value.is_empty() {
        return None;
    }

    Some(MarkupContent::new(
        if does_support_markdown {
            MarkupKind::Markdown
        } else {
            MarkupKind::PlainText
        },
        value,
    ))
}

/// Port of the `HTMLDataProvider` class.
pub struct HtmlDataProvider {
    id: String,
    data: Arc<HTMLDataV1>,
    tag_map: HashMap<String, usize>,
    value_set_map: HashMap<String, usize>,
}

impl HtmlDataProvider {
    pub fn new(id: impl Into<String>, custom_data: Arc<HTMLDataV1>) -> Self {
        let mut tag_map = HashMap::new();
        for (i, tag) in custom_data.tags.iter().enumerate() {
            tag_map.insert(tag.name.to_lowercase(), i);
        }
        let mut value_set_map = HashMap::new();
        for (i, vs) in custom_data.value_sets.iter().enumerate() {
            value_set_map.insert(vs.name.clone(), i);
        }
        Self {
            id: id.into(),
            data: custom_data,
            tag_map,
            value_set_map,
        }
    }
}

impl IHtmlDataProvider for HtmlDataProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn is_applicable(&self, _language_id: &str) -> bool {
        true
    }

    fn provide_tags(&self) -> &[ITagData] {
        &self.data.tags
    }

    fn provide_attributes(&self, tag: &str) -> Vec<IAttributeData> {
        let mut attributes = Vec::new();
        if let Some(&idx) = self.tag_map.get(&tag.to_lowercase()) {
            attributes.extend(self.data.tags[idx].attributes.iter().cloned());
        }
        attributes.extend(self.data.global_attributes.iter().cloned());
        attributes
    }

    fn provide_values(&self, tag: &str, attribute: &str) -> Vec<IValueData> {
        let attribute = attribute.to_lowercase();
        let mut values = Vec::new();
        let process =
            |attributes: &[IAttributeData], provider: &Self, values: &mut Vec<IValueData>| {
                for a in attributes {
                    if a.name.to_lowercase() != attribute {
                        continue;
                    }
                    if let Some(inline) = &a.values {
                        values.extend(inline.iter().cloned());
                    }
                    if let Some(set) = &a.value_set
                        && let Some(&idx) = provider.value_set_map.get(set)
                    {
                        values.extend(provider.data.value_sets[idx].values.iter().cloned());
                    }
                }
            };
        if let Some(&idx) = self.tag_map.get(&tag.to_lowercase()) {
            process(&self.data.tags[idx].attributes, self, &mut values);
        }
        process(&self.data.global_attributes, self, &mut values);
        values
    }
}

static DEFAULT_DATA: OnceLock<Arc<HTMLDataV1>> = OnceLock::new();

/// Port of `htmlData` from `webCustomData.ts`.
pub fn html_data() -> Arc<HTMLDataV1> {
    DEFAULT_DATA
        .get_or_init(|| {
            Arc::new(
                serde_json::from_str(include_str!("data/web_custom_data.json"))
                    .expect("embedded web custom data is valid"),
            )
        })
        .clone()
}

/// Port of `getDefaultHTMLDataProvider`.
pub fn get_default_html_data_provider() -> HtmlDataProvider {
    HtmlDataProvider::new("default", html_data())
}
