//! Workspace folders: the roots used to discover `.html.toml` files and to
//! map a document URI back to its folder.

use std::path::PathBuf;

use crate::protocol::{InitializeParams, WorkspaceFolder};

/// Workspace roots, in initialization order.
#[derive(Debug, Clone, Default)]
pub struct Workspace {
    folders: Vec<Folder>,
}

#[derive(Debug, Clone)]
struct Folder {
    uri: String,
    path: PathBuf,
}

impl Workspace {
    /// Port of the upstream "workspace folders, else rootUri, else rootPath"
    /// fallback used to decide where a document lives.
    pub fn from_initialize(params: &InitializeParams) -> (Workspace, Vec<String>) {
        let mut warnings = Vec::new();
        let mut workspace = Workspace::default();

        if let Some(folders) = &params.workspace_folders
            && !folders.is_empty()
        {
            workspace.add_folders(folders, &mut warnings);
            return (workspace, warnings);
        }

        if let Some(uri) = &params.root_uri
            && let Some(path) = uri_to_path(uri)
        {
            workspace.folders.push(Folder {
                uri: uri.clone(),
                path,
            });
        } else if let Some(path) = &params.root_path {
            workspace.folders.push(Folder {
                uri: path_to_uri(path),
                path: PathBuf::from(path),
            });
        }
        (workspace, warnings)
    }

    pub fn add_folders(&mut self, folders: &[WorkspaceFolder], warnings: &mut Vec<String>) {
        for folder in folders {
            if self.folders.iter().any(|known| known.uri == folder.uri) {
                continue;
            }
            let Some(path) = uri_to_path(&folder.uri) else {
                let label = if folder.name.is_empty() {
                    folder.uri.as_str()
                } else {
                    folder.name.as_str()
                };
                warnings.push(format!(
                    "ignoring workspace folder `{label}`: not a file URI"
                ));
                continue;
            };
            self.folders.push(Folder {
                uri: folder.uri.clone(),
                path,
            });
        }
    }

    pub fn remove_folders(&mut self, folders: &[WorkspaceFolder]) {
        self.folders
            .retain(|known| !folders.iter().any(|removed| removed.uri == known.uri));
    }

    pub fn roots(&self) -> Vec<PathBuf> {
        self.folders
            .iter()
            .map(|folder| folder.path.clone())
            .collect()
    }
}

/// `file://` URIs become paths; any other scheme is rejected.
fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let parsed = url::Url::parse(uri).ok()?;
    if parsed.scheme() != "file" {
        return None;
    }
    parsed.to_file_path().ok()
}

fn path_to_uri(path: &str) -> String {
    match url::Url::from_file_path(path) {
        Ok(uri) => uri.to_string(),
        Err(()) => path.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(uri: &str) -> WorkspaceFolder {
        WorkspaceFolder {
            uri: uri.to_string(),
            name: String::new(),
        }
    }

    #[test]
    fn reads_workspace_folders() {
        let params = InitializeParams {
            workspace_folders: Some(vec![folder("file:///work/one")]),
            ..Default::default()
        };
        let (workspace, warnings) = Workspace::from_initialize(&params);
        assert!(warnings.is_empty());
        assert_eq!(workspace.roots(), vec![PathBuf::from("/work/one")]);
    }

    #[test]
    fn falls_back_to_root_uri() {
        let params = InitializeParams {
            root_uri: Some("file:///work/root".to_string()),
            ..Default::default()
        };
        let (workspace, _) = Workspace::from_initialize(&params);
        assert_eq!(workspace.roots(), vec![PathBuf::from("/work/root")]);
    }

    #[test]
    fn ignores_non_file_uris() {
        let params = InitializeParams {
            workspace_folders: Some(vec![folder("https://example.com/project")]),
            ..Default::default()
        };
        let (workspace, warnings) = Workspace::from_initialize(&params);
        assert!(workspace.roots().is_empty());
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn adds_and_removes_folders() {
        let mut workspace = Workspace::default();
        let mut warnings = Vec::new();
        workspace.add_folders(&[folder("file:///a"), folder("file:///b")], &mut warnings);
        workspace.add_folders(&[folder("file:///b")], &mut warnings);
        assert_eq!(workspace.roots().len(), 2);
        workspace.remove_folders(&[folder("file:///a")]);
        assert_eq!(workspace.roots(), vec![PathBuf::from("/b")]);
    }
}
