//! End-to-end LSP session: configuration files, workspace folders and the
//! settings precedence order.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};

struct Client {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: i32,
    /// Set on drop; the watchdog kills the server if the test hangs.
    finished: Arc<AtomicBool>,
}

impl Client {
    fn start(args: &[&str]) -> Client {
        let mut child = Command::new(env!("CARGO_BIN_EXE_html-languageserver"))
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn server");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let finished = Arc::new(AtomicBool::new(false));
        let watchdog = Arc::clone(&finished);
        thread::spawn(move || {
            for _ in 0..300 {
                thread::sleep(Duration::from_millis(100));
                if watchdog.load(Ordering::SeqCst) {
                    return;
                }
            }
            // Unblock the test instead of letting it wait forever.
            std::process::exit(1);
        });
        Client {
            child,
            stdin,
            stdout,
            next_id: 1,
            finished,
        }
    }

    fn send(&mut self, message: Value) {
        let body = message.to_string();
        write!(self.stdin, "Content-Length: {}\r\n\r\n{body}", body.len()).unwrap();
        self.stdin.flush().unwrap();
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        self.response(id)
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.send(json!({ "jsonrpc": "2.0", "method": method, "params": params }));
    }

    fn read(&mut self) -> Value {
        let mut length = 0usize;
        loop {
            let mut header = String::new();
            self.stdout.read_line(&mut header).expect("read header");
            let header = header.trim();
            if header.is_empty() {
                break;
            }
            if let Some(value) = header.strip_prefix("Content-Length:") {
                length = value.trim().parse().expect("content length");
            }
        }
        let mut body = vec![0u8; length];
        self.stdout.read_exact(&mut body).expect("read body");
        serde_json::from_slice(&body).expect("json body")
    }

    /// Reads until the response to `id`, answering server-initiated requests.
    fn response(&mut self, id: i32) -> Value {
        loop {
            let message = self.read();
            if message.get("method").and_then(Value::as_str).is_some() {
                if message.get("id").is_some() {
                    let reply = json!({
                        "jsonrpc": "2.0",
                        "id": message["id"],
                        "result": null
                    });
                    self.send(reply);
                }
                continue;
            }
            if message.get("id").and_then(Value::as_i64) == Some(id as i64) {
                return message;
            }
        }
    }

    fn initialize(&mut self, root: Option<&Path>) -> Value {
        let mut params = json!({ "processId": null, "capabilities": {} });
        if let Some(root) = root {
            let uri = url::Url::from_directory_path(root).unwrap().to_string();
            params["rootUri"] = json!(uri);
            params["workspaceFolders"] = json!([{ "uri": uri, "name": "workspace" }]);
        }
        let response = self.request("initialize", params);
        self.notify("initialized", json!({}));
        response
    }

    fn open(&mut self, text: &str) {
        self.notify(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": "file:///workspace/test.html",
                    "languageId": "html",
                    "version": 1,
                    "text": text
                }
            }),
        );
    }

    fn format_edits(&mut self, text: &str) -> Vec<Value> {
        self.open(text);
        let response = self.request(
            "textDocument/formatting",
            json!({
                "textDocument": { "uri": "file:///workspace/test.html" },
                "options": { "tabSize": 4, "insertSpaces": true }
            }),
        );
        response["result"].as_array().cloned().unwrap_or_default()
    }

    fn format(&mut self) -> String {
        let edits = self.format_edits("<div><span>x</span></div>");
        edits
            .first()
            .and_then(|edit| edit["newText"].as_str())
            .unwrap_or_default()
            .to_string()
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.finished.store(true, Ordering::SeqCst);
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let path =
            std::env::temp_dir().join(format!("html-ls-session-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }

    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, contents).unwrap();
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn reads_html_toml_from_the_workspace() {
    let dir = TempDir::new("workspace-config");
    dir.write(".html.toml", "[format]\nend_with_newline = true\n");

    let mut client = Client::start(&["--stdio"]);
    let response = client.initialize(Some(&dir.0));
    assert!(
        response["result"]["capabilities"]["workspace"]["workspaceFolders"]["supported"]
            .as_bool()
            .unwrap_or(false)
    );

    client.open("<div><span>x</span></div>");
    assert!(client.format().ends_with('\n'));
}

#[test]
fn did_change_configuration_overrides_the_file() {
    let dir = TempDir::new("push-config");
    dir.write(".html.toml", "[format]\nend_with_newline = true\n");

    let mut client = Client::start(&["--stdio"]);
    client.initialize(Some(&dir.0));
    client.open("<div><span>x</span></div>");
    assert!(client.format().ends_with('\n'));

    client.notify(
        "workspace/didChangeConfiguration",
        json!({ "settings": { "html": { "format": { "endWithNewline": false } } } }),
    );
    assert!(!client.format().ends_with('\n'));
}

#[test]
fn reports_an_invalid_configuration_file_and_keeps_running() {
    let dir = TempDir::new("invalid-config");
    dir.write(".html.toml", "= not toml");

    let mut client = Client::start(&["--stdio"]);
    let response = client.initialize(Some(&dir.0));
    assert!(response["result"]["capabilities"].is_object());

    // The warning is sent right after the `initialize` response.
    let notification = client.read();
    assert_eq!(notification["method"], "window/showMessage");
    assert_eq!(notification["params"]["type"], 2);

    // The server still answers requests.
    let text = "<html><head><title>t</title></head><body><p>x</p></body></html>";
    assert!(!client.format_edits(text).is_empty());
}

#[test]
fn explicit_config_ignores_the_workspace_file() {
    let dir = TempDir::new("explicit-config");
    dir.write(".html.toml", "[format]\nend_with_newline = true\n");
    let explicit = dir.write("custom.toml", "[format]\nend_with_newline = false\n");

    let mut client = Client::start(&["--stdio", "--config", explicit.to_str().unwrap()]);
    client.initialize(Some(&dir.0));
    client.open("<div><span>x</span></div>");
    assert!(!client.format().ends_with('\n'));
}
