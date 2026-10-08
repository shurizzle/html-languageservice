//! Port of `src/test/pathCompletions.test.ts`.

use std::sync::Arc;

use html_languageservice::{
    CompletionItem, CompletionItemKind, CompletionList, DocumentContext, FileSystemProvider,
    FileType, LanguageServiceOptions, TextDocument, apply_edits, get_language_service,
};

const FIXTURE_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/path_completion_fixtures");

fn file_uri(path: &str) -> String {
    format!("file://{path}")
}

fn uri_to_path(uri: &str) -> String {
    uri.strip_prefix("file://").unwrap_or(uri).to_string()
}

fn normalize(path: &str) -> String {
    let absolute = path.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            p => parts.push(p),
        }
    }
    let joined = parts.join("/");
    if absolute {
        format!("/{joined}")
    } else {
        joined
    }
}

struct TestFsProvider;

impl FileSystemProvider for TestFsProvider {
    fn read_directory(&self, uri: &str) -> Vec<(String, FileType)> {
        let path = uri_to_path(uri);
        let Ok(entries) = std::fs::read_dir(&path) else {
            return Vec::new();
        };
        let mut result = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let file_type = match entry.file_type() {
                Ok(t) if t.is_dir() => FileType::Directory,
                Ok(t) if t.is_file() => FileType::File,
                _ => FileType::Unknown,
            };
            result.push((name, file_type));
        }
        result
    }
}

struct WorkspaceDocumentContext {
    workspace_folder: String,
}

impl DocumentContext for WorkspaceDocumentContext {
    fn resolve_reference(&self, reference: &str, base: &str) -> Option<String> {
        if reference.starts_with('/') {
            let joined = normalize(&format!(
                "{}/{}",
                uri_to_path(&self.workspace_folder),
                reference
            ));
            return Some(format!("file://{joined}"));
        }

        let base_path = uri_to_path(base);
        let directory = match base_path.rfind('/') {
            Some(i) => &base_path[..i],
            None => "",
        };
        let joined = normalize(&format!("{directory}/{reference}"));
        Some(format!("file://{joined}"))
    }
}

#[derive(Default)]
struct Item {
    label: &'static str,
    kind: Option<CompletionItemKind>,
    result_text: Option<&'static str>,
    not_available: bool,
}

#[derive(Default)]
struct Expected {
    count: Option<usize>,
    items: Vec<Item>,
}

fn assert_completion(list: &CompletionList, expected: &Item, document: &TextDocument) {
    let matches: Vec<&CompletionItem> = list
        .items
        .iter()
        .filter(|c| c.label == expected.label)
        .collect();

    if expected.not_available {
        assert!(matches.is_empty(), "{} should not exist", expected.label);
        return;
    }

    assert_eq!(
        matches.len(),
        1,
        "{} should exist exactly once: {:?}",
        expected.label,
        list.items.iter().map(|c| &c.label).collect::<Vec<_>>()
    );
    let item = matches[0];
    if let Some(kind) = expected.kind {
        assert_eq!(item.kind, Some(kind), "kind for {}", expected.label);
    }
    if let Some(result_text) = expected.result_text
        && let Some(edit) = &item.text_edit
    {
        let formatted = apply_edits(document, std::slice::from_ref(edit));
        assert_eq!(formatted, result_text, "result text for {}", expected.label);
    }
}

fn test_completion2_for(value: &str, expected: &Expected, uri: &str, workspace_folder: &str) {
    let offset = value.find('|').unwrap_or(0);
    let content = format!("{}{}", &value[..offset], &value[offset.saturating_add(1).min(value.len())..]);

    let document = TextDocument::create(uri, "html", 0, &content);
    let position = document.position_at(offset);

    let service = get_language_service(LanguageServiceOptions {
        file_system_provider: Some(Arc::new(TestFsProvider)),
        ..Default::default()
    });
    let context = WorkspaceDocumentContext {
        workspace_folder: workspace_folder.to_string(),
    };
    let html_document = service.parse_html_document(&document);
    let list = service.do_complete2(&document, position, &html_document, &context, None);

    if let Some(count) = expected.count {
        assert_eq!(list.items.len(), count, "item count for {value:?}");
    }
    for item in &expected.items {
        assert_completion(&list, item, &document);
    }
}

fn root_uri() -> String {
    file_uri(FIXTURE_ROOT)
}

fn index_uri() -> String {
    format!("{}/index.html", root_uri())
}

fn about_uri() -> String {
    format!("{}/about/about.html", root_uri())
}

#[test]
fn basics_correct_label_kind_result_command() {
    test_completion2_for(
        "<script src=\"./|\">",
        &Expected {
            count: None,
            items: vec![
                Item {
                    label: "about/",
                    kind: Some(CompletionItemKind::Folder),
                    result_text: Some("<script src=\"./about/\">"),
                    ..Default::default()
                },
                Item {
                    label: "index.html",
                    kind: Some(CompletionItemKind::File),
                    result_text: Some("<script src=\"./index.html\">"),
                    ..Default::default()
                },
                Item {
                    label: "src/",
                    kind: Some(CompletionItemKind::Folder),
                    result_text: Some("<script src=\"./src/\">"),
                    ..Default::default()
                },
            ],
        },
        &index_uri(),
        &root_uri(),
    );
}

#[test]
fn basics_single_quote() {
    test_completion2_for(
        "<script src='./|'>",
        &Expected {
            count: None,
            items: vec![
                Item {
                    label: "about/",
                    kind: Some(CompletionItemKind::Folder),
                    result_text: Some("<script src='./about/'>"),
                    ..Default::default()
                },
                Item {
                    label: "index.html",
                    kind: Some(CompletionItemKind::File),
                    result_text: Some("<script src='./index.html'>"),
                    ..Default::default()
                },
                Item {
                    label: "src/",
                    kind: Some(CompletionItemKind::Folder),
                    result_text: Some("<script src='./src/'>"),
                    ..Default::default()
                },
            ],
        },
        &index_uri(),
        &root_uri(),
    );
}

#[test]
fn no_completion_for_remote_paths() {
    for value in [
        "<script src=\"http:\">",
        "<script src=\"http:/|\">",
        "<script src=\"http://|\">",
        "<script src=\"https:|\">",
        "<script src=\"https:/|\">",
        "<script src=\"https://|\">",
        "<script src=\"//|\">",
    ] {
        test_completion2_for(value, &Expected::default(), &index_uri(), &root_uri());
    }
}

#[test]
fn relative_path() {
    test_completion2_for(
        "<script src=\"../|\">",
        &Expected {
            count: None,
            items: vec![
                Item {
                    label: "about/",
                    result_text: Some("<script src=\"../about/\">"),
                    ..Default::default()
                },
                Item {
                    label: "index.html",
                    result_text: Some("<script src=\"../index.html\">"),
                    ..Default::default()
                },
                Item {
                    label: "src/",
                    result_text: Some("<script src=\"../src/\">"),
                    ..Default::default()
                },
            ],
        },
        &about_uri(),
        &root_uri(),
    );

    test_completion2_for(
        "<script src=\"../src/|\">",
        &Expected {
            count: None,
            items: vec![
                Item {
                    label: "feature.js",
                    result_text: Some("<script src=\"../src/feature.js\">"),
                    ..Default::default()
                },
                Item {
                    label: "test.js",
                    result_text: Some("<script src=\"../src/test.js\">"),
                    ..Default::default()
                },
            ],
        },
        &about_uri(),
        &root_uri(),
    );
}

#[test]
fn absolute_path() {
    test_completion2_for(
        "<script src=\"/|\">",
        &Expected {
            count: None,
            items: vec![
                Item {
                    label: "about/",
                    result_text: Some("<script src=\"/about/\">"),
                    ..Default::default()
                },
                Item {
                    label: "index.html",
                    result_text: Some("<script src=\"/index.html\">"),
                    ..Default::default()
                },
                Item {
                    label: "src/",
                    result_text: Some("<script src=\"/src/\">"),
                    ..Default::default()
                },
            ],
        },
        &index_uri(),
        &root_uri(),
    );

    test_completion2_for(
        "<script src=\"/src/|\">",
        &Expected {
            count: None,
            items: vec![
                Item {
                    label: "feature.js",
                    result_text: Some("<script src=\"/src/feature.js\">"),
                    ..Default::default()
                },
                Item {
                    label: "test.js",
                    result_text: Some("<script src=\"/src/test.js\">"),
                    ..Default::default()
                },
            ],
        },
        &about_uri(),
        &root_uri(),
    );
}

#[test]
fn empty_path_value() {
    test_completion2_for(
        "<script src=\"|\">",
        &Expected {
            count: None,
            items: vec![
                Item {
                    label: "about/",
                    result_text: Some("<script src=\"about/\">"),
                    ..Default::default()
                },
                Item {
                    label: "index.html",
                    result_text: Some("<script src=\"index.html\">"),
                    ..Default::default()
                },
                Item {
                    label: "src/",
                    result_text: Some("<script src=\"src/\">"),
                    ..Default::default()
                },
            ],
        },
        &index_uri(),
        &root_uri(),
    );

    test_completion2_for(
        "<script src=\"|\">",
        &Expected {
            count: None,
            items: vec![
                Item {
                    label: "about.css",
                    result_text: Some("<script src=\"about.css\">"),
                    ..Default::default()
                },
                Item {
                    label: "about.html",
                    result_text: Some("<script src=\"about.html\">"),
                    ..Default::default()
                },
                Item {
                    label: "media/",
                    result_text: Some("<script src=\"media/\">"),
                    ..Default::default()
                },
            ],
        },
        &about_uri(),
        &root_uri(),
    );
}

#[test]
fn incomplete_path() {
    test_completion2_for(
        "<script src=\"/src/f|\">",
        &Expected {
            count: None,
            items: vec![
                Item {
                    label: "feature.js",
                    result_text: Some("<script src=\"/src/feature.js\">"),
                    ..Default::default()
                },
                Item {
                    label: "test.js",
                    result_text: Some("<script src=\"/src/test.js\">"),
                    ..Default::default()
                },
            ],
        },
        &about_uri(),
        &root_uri(),
    );

    test_completion2_for(
        "<script src=\"../src/f|\">",
        &Expected {
            count: None,
            items: vec![
                Item {
                    label: "feature.js",
                    result_text: Some("<script src=\"../src/feature.js\">"),
                    ..Default::default()
                },
                Item {
                    label: "test.js",
                    result_text: Some("<script src=\"../src/test.js\">"),
                    ..Default::default()
                },
            ],
        },
        &about_uri(),
        &root_uri(),
    );
}

#[test]
fn no_leading_dot_or_slash() {
    test_completion2_for(
        "<script src=\"s|\">",
        &Expected {
            count: None,
            items: vec![
                Item {
                    label: "about/",
                    result_text: Some("<script src=\"about/\">"),
                    ..Default::default()
                },
                Item {
                    label: "index.html",
                    result_text: Some("<script src=\"index.html\">"),
                    ..Default::default()
                },
                Item {
                    label: "src/",
                    result_text: Some("<script src=\"src/\">"),
                    ..Default::default()
                },
            ],
        },
        &index_uri(),
        &root_uri(),
    );

    test_completion2_for(
        "<script src=\"src/|\">",
        &Expected {
            count: None,
            items: vec![
                Item {
                    label: "feature.js",
                    result_text: Some("<script src=\"src/feature.js\">"),
                    ..Default::default()
                },
                Item {
                    label: "test.js",
                    result_text: Some("<script src=\"src/test.js\">"),
                    ..Default::default()
                },
            ],
        },
        &index_uri(),
        &root_uri(),
    );

    test_completion2_for(
        "<script src=\"src/f|\">",
        &Expected {
            count: None,
            items: vec![
                Item {
                    label: "feature.js",
                    result_text: Some("<script src=\"src/feature.js\">"),
                    ..Default::default()
                },
                Item {
                    label: "test.js",
                    result_text: Some("<script src=\"src/test.js\">"),
                    ..Default::default()
                },
            ],
        },
        &index_uri(),
        &root_uri(),
    );

    test_completion2_for(
        "<script src=\"s|\">",
        &Expected {
            count: None,
            items: vec![
                Item {
                    label: "about.css",
                    result_text: Some("<script src=\"about.css\">"),
                    ..Default::default()
                },
                Item {
                    label: "about.html",
                    result_text: Some("<script src=\"about.html\">"),
                    ..Default::default()
                },
                Item {
                    label: "media/",
                    result_text: Some("<script src=\"media/\">"),
                    ..Default::default()
                },
            ],
        },
        &about_uri(),
        &root_uri(),
    );

    test_completion2_for(
        "<script src=\"media/|\">",
        &Expected {
            count: None,
            items: vec![Item {
                label: "icon.pic",
                result_text: Some("<script src=\"media/icon.pic\">"),
                ..Default::default()
            }],
        },
        &about_uri(),
        &root_uri(),
    );

    test_completion2_for(
        "<script src=\"media/f|\">",
        &Expected {
            count: None,
            items: vec![Item {
                label: "icon.pic",
                result_text: Some("<script src=\"media/icon.pic\">"),
                ..Default::default()
            }],
        },
        &about_uri(),
        &root_uri(),
    );
}

#[test]
fn trigger_completion_in_middle_of_path() {
    test_completion2_for(
        "<script src=\"src/f|eature.js\">",
        &Expected {
            count: None,
            items: vec![
                Item {
                    label: "feature.js",
                    result_text: Some("<script src=\"src/feature.js\">"),
                    ..Default::default()
                },
                Item {
                    label: "test.js",
                    result_text: Some("<script src=\"src/test.js\">"),
                    ..Default::default()
                },
            ],
        },
        &index_uri(),
        &root_uri(),
    );

    test_completion2_for(
        "<script src=\"s|rc/feature.js\">",
        &Expected {
            count: None,
            items: vec![
                Item {
                    label: "about/",
                    result_text: Some("<script src=\"about/\">"),
                    ..Default::default()
                },
                Item {
                    label: "index.html",
                    result_text: Some("<script src=\"index.html\">"),
                    ..Default::default()
                },
                Item {
                    label: "src/",
                    result_text: Some("<script src=\"src/\">"),
                    ..Default::default()
                },
            ],
        },
        &index_uri(),
        &root_uri(),
    );

    test_completion2_for(
        "<script src=\"media/f|eature.js\">",
        &Expected {
            count: None,
            items: vec![Item {
                label: "icon.pic",
                result_text: Some("<script src=\"media/icon.pic\">"),
                ..Default::default()
            }],
        },
        &about_uri(),
        &root_uri(),
    );

    test_completion2_for(
        "<script src=\"m|edia/feature.js\">",
        &Expected {
            count: None,
            items: vec![
                Item {
                    label: "about.css",
                    result_text: Some("<script src=\"about.css\">"),
                    ..Default::default()
                },
                Item {
                    label: "about.html",
                    result_text: Some("<script src=\"about.html\">"),
                    ..Default::default()
                },
                Item {
                    label: "media/",
                    result_text: Some("<script src=\"media/\">"),
                    ..Default::default()
                },
            ],
        },
        &about_uri(),
        &root_uri(),
    );
}

#[test]
fn trigger_completion_in_middle_of_path_and_with_whitespaces() {
    test_completion2_for(
        "<script src=\"./| about/about.html>",
        &Expected {
            count: None,
            items: vec![
                Item {
                    label: "about/",
                    result_text: Some("<script src=\"./about/ about/about.html>"),
                    ..Default::default()
                },
                Item {
                    label: "index.html",
                    result_text: Some("<script src=\"./index.html about/about.html>"),
                    ..Default::default()
                },
                Item {
                    label: "src/",
                    result_text: Some("<script src=\"./src/ about/about.html>"),
                    ..Default::default()
                },
            ],
        },
        &index_uri(),
        &root_uri(),
    );

    test_completion2_for(
        "<script src=\"./a|bout /about.html>",
        &Expected {
            count: None,
            items: vec![
                Item {
                    label: "about/",
                    result_text: Some("<script src=\"./about/ /about.html>"),
                    ..Default::default()
                },
                Item {
                    label: "index.html",
                    result_text: Some("<script src=\"./index.html /about.html>"),
                    ..Default::default()
                },
                Item {
                    label: "src/",
                    result_text: Some("<script src=\"./src/ /about.html>"),
                    ..Default::default()
                },
            ],
        },
        &index_uri(),
        &root_uri(),
    );
}

#[test]
fn completion_should_ignore_files_folders_starting_with_dot() {
    test_completion2_for(
        "<script src=\"./|\"",
        &Expected {
            count: Some(4),
            items: Vec::new(),
        },
        &index_uri(),
        &root_uri(),
    );
}

#[test]
fn trigger_completion_in_src_and_href_attributes_of_custom_elements() {
    test_completion2_for(
        "<my-custom-element src=\"../|\">",
        &Expected {
            count: None,
            items: vec![
                Item {
                    label: "about/",
                    result_text: Some("<my-custom-element src=\"../about/\">"),
                    ..Default::default()
                },
                Item {
                    label: "index.html",
                    result_text: Some("<my-custom-element src=\"../index.html\">"),
                    ..Default::default()
                },
                Item {
                    label: "src/",
                    result_text: Some("<my-custom-element src=\"../src/\">"),
                    ..Default::default()
                },
            ],
        },
        &about_uri(),
        &root_uri(),
    );

    test_completion2_for(
        "<my-custom-element href=\"../src/|\">",
        &Expected {
            count: None,
            items: vec![
                Item {
                    label: "feature.js",
                    result_text: Some("<my-custom-element href=\"../src/feature.js\">"),
                    ..Default::default()
                },
                Item {
                    label: "test.js",
                    result_text: Some("<my-custom-element href=\"../src/test.js\">"),
                    ..Default::default()
                },
            ],
        },
        &about_uri(),
        &root_uri(),
    );
}

#[test]
fn filter_css_files_for_link_rel_stylesheet() {
    test_completion2_for(
        "<link rel=\"stylesheet\" href=\"|\">",
        &Expected {
            count: None,
            items: vec![
                Item {
                    label: "about.css",
                    kind: Some(CompletionItemKind::File),
                    result_text: Some("<link rel=\"stylesheet\" href=\"about.css\">"),
                    ..Default::default()
                },
                Item {
                    label: "about.html",
                    kind: Some(CompletionItemKind::File),
                    result_text: Some("<link rel=\"stylesheet\" href=\"about.html\">"),
                    ..Default::default()
                },
                Item {
                    label: "media/",
                    kind: Some(CompletionItemKind::Folder),
                    result_text: Some("<link rel=\"stylesheet\" href=\"media/\">"),
                    ..Default::default()
                },
            ],
        },
        &about_uri(),
        &root_uri(),
    );

    test_completion2_for(
        "<link rel=\"stylesheet\" href=\"./|\">",
        &Expected {
            count: None,
            items: vec![
                Item {
                    label: "about/",
                    kind: Some(CompletionItemKind::Folder),
                    result_text: Some("<link rel=\"stylesheet\" href=\"./about/\">"),
                    ..Default::default()
                },
                Item {
                    label: "styles.css",
                    kind: Some(CompletionItemKind::File),
                    result_text: Some("<link rel=\"stylesheet\" href=\"./styles.css\">"),
                    ..Default::default()
                },
                Item {
                    label: "index.html",
                    kind: Some(CompletionItemKind::File),
                    result_text: Some("<link rel=\"stylesheet\" href=\"./index.html\">"),
                    ..Default::default()
                },
                Item {
                    label: "src/",
                    kind: Some(CompletionItemKind::Folder),
                    result_text: Some("<link rel=\"stylesheet\" href=\"./src/\">"),
                    ..Default::default()
                },
            ],
        },
        &index_uri(),
        &root_uri(),
    );

    test_completion2_for(
        "<link rel='stylesheet' href=\"./|\">",
        &Expected {
            count: None,
            items: vec![
                Item {
                    label: "about/",
                    kind: Some(CompletionItemKind::Folder),
                    result_text: Some("<link rel='stylesheet' href=\"./about/\">"),
                    ..Default::default()
                },
                Item {
                    label: "styles.css",
                    kind: Some(CompletionItemKind::File),
                    result_text: Some("<link rel='stylesheet' href=\"./styles.css\">"),
                    ..Default::default()
                },
                Item {
                    label: "index.html",
                    kind: Some(CompletionItemKind::File),
                    result_text: Some("<link rel='stylesheet' href=\"./index.html\">"),
                    ..Default::default()
                },
                Item {
                    label: "src/",
                    kind: Some(CompletionItemKind::Folder),
                    result_text: Some("<link rel='stylesheet' href=\"./src/\">"),
                    ..Default::default()
                },
            ],
        },
        &index_uri(),
        &root_uri(),
    );
}

#[test]
fn prioritize_script_files_for_script_tag() {
    test_completion2_for(
        "<script src=\"./|\">",
        &Expected {
            count: None,
            items: vec![
                Item {
                    label: "about/",
                    kind: Some(CompletionItemKind::Folder),
                    result_text: Some("<script src=\"./about/\">"),
                    ..Default::default()
                },
                Item {
                    label: "index.html",
                    kind: Some(CompletionItemKind::File),
                    result_text: Some("<script src=\"./index.html\">"),
                    ..Default::default()
                },
                Item {
                    label: "src/",
                    kind: Some(CompletionItemKind::Folder),
                    result_text: Some("<script src=\"./src/\">"),
                    ..Default::default()
                },
            ],
        },
        &index_uri(),
        &root_uri(),
    );

    test_completion2_for(
        "<script src=\"./src/|\">",
        &Expected {
            count: None,
            items: vec![
                Item {
                    label: "feature.js",
                    kind: Some(CompletionItemKind::File),
                    result_text: Some("<script src=\"./src/feature.js\">"),
                    ..Default::default()
                },
                Item {
                    label: "test.js",
                    kind: Some(CompletionItemKind::File),
                    result_text: Some("<script src=\"./src/test.js\">"),
                    ..Default::default()
                },
            ],
        },
        &index_uri(),
        &root_uri(),
    );
}

#[test]
fn no_filtering_for_link_tag_without_rel_attribute() {
    test_completion2_for(
        "<link href=\"./|\">",
        &Expected {
            count: None,
            items: vec![
                Item {
                    label: "about/",
                    kind: Some(CompletionItemKind::Folder),
                    result_text: Some("<link href=\"./about/\">"),
                    ..Default::default()
                },
                Item {
                    label: "index.html",
                    kind: Some(CompletionItemKind::File),
                    result_text: Some("<link href=\"./index.html\">"),
                    ..Default::default()
                },
                Item {
                    label: "src/",
                    kind: Some(CompletionItemKind::Folder),
                    result_text: Some("<link href=\"./src/\">"),
                    ..Default::default()
                },
                Item {
                    label: "styles.css",
                    kind: Some(CompletionItemKind::File),
                    result_text: Some("<link href=\"./styles.css\">"),
                    ..Default::default()
                },
            ],
        },
        &index_uri(),
        &root_uri(),
    );
}

#[test]
fn prioritize_image_files_for_link_rel_icon() {
    test_completion2_for(
        "<link rel=\"icon\" href=\"|\">",
        &Expected {
            count: None,
            items: vec![
                Item {
                    label: "about.css",
                    kind: Some(CompletionItemKind::File),
                    result_text: Some("<link rel=\"icon\" href=\"about.css\">"),
                    ..Default::default()
                },
                Item {
                    label: "about.html",
                    kind: Some(CompletionItemKind::File),
                    result_text: Some("<link rel=\"icon\" href=\"about.html\">"),
                    ..Default::default()
                },
                Item {
                    label: "media/",
                    kind: Some(CompletionItemKind::Folder),
                    result_text: Some("<link rel=\"icon\" href=\"media/\">"),
                    ..Default::default()
                },
            ],
        },
        &about_uri(),
        &root_uri(),
    );
}
