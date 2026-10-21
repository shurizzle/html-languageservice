//! Minimal HTML language server: LSP transport over stdio, TCP or named pipes
//! built on `html-languageservice`.
//!
//! Params/results are handled with `serde_json` values and a handful of local
//! `Deserialize` structs, so the crate does not depend on a specific
//! `lsp-types` release.

use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use html_languageservice::{
    DocumentContext, LanguageServiceOptions, TextDocument, get_language_service,
};
use lsp_server::{Connection, ErrorCode, Message, Request, Response};
use serde::Deserialize;
use serde_json::{Value, json};

mod cli;
mod convert;
mod protocol;
mod settings;
mod transport;

use protocol::*;
use settings::Settings;

/// A document tracked by the server.
struct DocumentState {
    language_id: String,
    version: i32,
    text: String,
}

struct Server {
    service: html_languageservice::LanguageService,
    documents: HashMap<String, DocumentState>,
    settings: Settings,
}

impl Server {
    fn text_document(&self, uri: &str) -> Option<TextDocument> {
        let state = self.documents.get(uri)?;
        Some(TextDocument::create(
            uri,
            &state.language_id,
            state.version,
            &state.text,
        ))
    }

    fn handle_request(
        &self,
        connection: &Connection,
        req: Request,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        match req.method.as_str() {
            "textDocument/completion" => {
                self.completion(connection, req);
            }
            "textDocument/hover" => {
                self.hover(connection, req);
            }
            "textDocument/documentHighlight" => {
                self.document_highlight(connection, req);
            }
            "textDocument/documentSymbol" => {
                self.document_symbol(connection, req);
            }
            "textDocument/foldingRange" => {
                self.folding_range(connection, req);
            }
            "textDocument/selectionRange" => {
                self.selection_range(connection, req);
            }
            "textDocument/formatting" => {
                self.formatting(connection, req);
            }
            "textDocument/rangeFormatting" => {
                self.range_formatting(connection, req);
            }
            "textDocument/rename" => {
                self.rename(connection, req);
            }
            "textDocument/documentLink" => {
                self.document_link(connection, req);
            }
            "textDocument/linkedEditingRange" => {
                self.linked_editing_range(connection, req);
            }
            _ => {
                let _ = connection.sender.send(Message::Response(Response::new_err(
                    req.id,
                    ErrorCode::MethodNotFound as i32,
                    format!("unknown request: {}", req.method),
                )));
            }
        }
        Ok(())
    }

    fn completion(&self, connection: &Connection, req: Request) {
        let params: CompletionParams = match parse(&req) {
            Ok(p) => p,
            Err(response) => {
                let _ = connection.sender.send(Message::Response(response));
                return;
            }
        };
        let Some(document) = self.text_document(&params.text_document.uri) else {
            return respond_null(connection, req.id);
        };
        let html_document = self.service.parse_html_document(&document);
        let list = self.service.do_complete(
            &document,
            convert::position(params.position),
            &html_document,
            Some(&self.settings.completion),
        );
        let items: Vec<Value> = list.items.iter().map(convert::completion_item).collect();
        respond(
            connection,
            req.id,
            json!({ "isIncomplete": list.is_incomplete, "items": items }),
        );
    }

    fn hover(&self, connection: &Connection, req: Request) {
        let params: TextDocumentPositionParams = match parse(&req) {
            Ok(p) => p,
            Err(response) => {
                let _ = connection.sender.send(Message::Response(response));
                return;
            }
        };
        let Some(document) = self.text_document(&params.text_document.uri) else {
            return respond_null(connection, req.id);
        };
        let html_document = self.service.parse_html_document(&document);
        let hover = self.service.do_hover(
            &document,
            convert::position(params.position),
            &html_document,
            None,
        );
        match hover {
            Some(hover) => respond(connection, req.id, convert::hover(&hover)),
            None => respond_null(connection, req.id),
        }
    }

    fn document_highlight(&self, connection: &Connection, req: Request) {
        let params: TextDocumentPositionParams = match parse(&req) {
            Ok(p) => p,
            Err(response) => {
                let _ = connection.sender.send(Message::Response(response));
                return;
            }
        };
        let Some(document) = self.text_document(&params.text_document.uri) else {
            return respond_null(connection, req.id);
        };
        let html_document = self.service.parse_html_document(&document);
        let highlights = self.service.find_document_highlights(
            &document,
            convert::position(params.position),
            &html_document,
        );
        let result: Vec<Value> = highlights.iter().map(convert::document_highlight).collect();
        respond(connection, req.id, json!(result));
    }

    fn document_symbol(&self, connection: &Connection, req: Request) {
        let params: DocumentSymbolParams = match parse(&req) {
            Ok(p) => p,
            Err(response) => {
                let _ = connection.sender.send(Message::Response(response));
                return;
            }
        };
        let Some(document) = self.text_document(&params.text_document.uri) else {
            return respond_null(connection, req.id);
        };
        let html_document = self.service.parse_html_document(&document);
        let symbols = self
            .service
            .find_document_symbols2(&document, &html_document);
        let result: Vec<Value> = symbols.iter().map(convert::document_symbol).collect();
        respond(connection, req.id, json!(result));
    }

    fn folding_range(&self, connection: &Connection, req: Request) {
        let params: TextDocumentParams = match parse(&req) {
            Ok(p) => p,
            Err(response) => {
                let _ = connection.sender.send(Message::Response(response));
                return;
            }
        };
        let Some(document) = self.text_document(&params.text_document.uri) else {
            return respond_null(connection, req.id);
        };
        let ranges = self.service.get_folding_ranges(&document, None);
        let result: Vec<Value> = ranges.iter().map(convert::folding_range).collect();
        respond(connection, req.id, json!(result));
    }

    fn selection_range(&self, connection: &Connection, req: Request) {
        let params: SelectionRangeParams = match parse(&req) {
            Ok(p) => p,
            Err(response) => {
                let _ = connection.sender.send(Message::Response(response));
                return;
            }
        };
        let Some(document) = self.text_document(&params.text_document.uri) else {
            return respond_null(connection, req.id);
        };
        let positions: Vec<html_languageservice::Position> = params
            .positions
            .iter()
            .copied()
            .map(convert::position)
            .collect();
        let ranges = self.service.get_selection_ranges(&document, &positions);
        let result: Vec<Value> = ranges.iter().map(convert::selection_range).collect();
        respond(connection, req.id, json!(result));
    }

    fn formatting(&self, connection: &Connection, req: Request) {
        let params: DocumentFormattingParams = match parse(&req) {
            Ok(p) => p,
            Err(response) => {
                let _ = connection.sender.send(Message::Response(response));
                return;
            }
        };
        let Some(document) = self.text_document(&params.text_document.uri) else {
            return respond_null(connection, req.id);
        };
        let edits = self.service.format(
            &document,
            None,
            &self.settings.format_options(&params.options),
        );
        let result: Vec<Value> = edits.iter().map(convert::text_edit).collect();
        respond(connection, req.id, json!(result));
    }

    fn range_formatting(&self, connection: &Connection, req: Request) {
        let params: DocumentRangeFormattingParams = match parse(&req) {
            Ok(p) => p,
            Err(response) => {
                let _ = connection.sender.send(Message::Response(response));
                return;
            }
        };
        let Some(document) = self.text_document(&params.text_document.uri) else {
            return respond_null(connection, req.id);
        };
        let edits = self.service.format(
            &document,
            Some(convert::range(params.range)),
            &self.settings.format_options(&params.options),
        );
        let result: Vec<Value> = edits.iter().map(convert::text_edit).collect();
        respond(connection, req.id, json!(result));
    }

    fn rename(&self, connection: &Connection, req: Request) {
        let params: RenameParams = match parse(&req) {
            Ok(p) => p,
            Err(response) => {
                let _ = connection.sender.send(Message::Response(response));
                return;
            }
        };
        let Some(document) = self.text_document(&params.text_document.uri) else {
            return respond_null(connection, req.id);
        };
        let html_document = self.service.parse_html_document(&document);
        let edit = self.service.do_rename(
            &document,
            convert::position(params.position),
            &params.new_name,
            &html_document,
        );
        match edit {
            Some(edit) => respond(connection, req.id, convert::workspace_edit(&edit)),
            None => respond_null(connection, req.id),
        }
    }

    fn document_link(&self, connection: &Connection, req: Request) {
        let params: DocumentLinkParams = match parse(&req) {
            Ok(p) => p,
            Err(response) => {
                let _ = connection.sender.send(Message::Response(response));
                return;
            }
        };
        let Some(document) = self.text_document(&params.text_document.uri) else {
            return respond_null(connection, req.id);
        };
        let resolver = UrlResolver;
        let links = self.service.find_document_links(&document, &resolver);
        let result: Vec<Value> = links.iter().map(convert::document_link).collect();
        respond(connection, req.id, json!(result));
    }

    fn linked_editing_range(&self, connection: &Connection, req: Request) {
        let params: TextDocumentPositionParams = match parse(&req) {
            Ok(p) => p,
            Err(response) => {
                let _ = connection.sender.send(Message::Response(response));
                return;
            }
        };
        let Some(document) = self.text_document(&params.text_document.uri) else {
            return respond_null(connection, req.id);
        };
        let html_document = self.service.parse_html_document(&document);
        let ranges = self.service.find_linked_editing_ranges(
            &document,
            convert::position(params.position),
            &html_document,
        );
        match ranges {
            Some(ranges) => {
                let result: Vec<Value> = ranges.iter().map(convert::range_json).collect();
                respond(connection, req.id, json!({ "ranges": result }));
            }
            None => respond_null(connection, req.id),
        }
    }

    fn handle_notification(&mut self, not: lsp_server::Notification) {
        match not.method.as_str() {
            "textDocument/didOpen" => {
                if let Ok(params) = serde_json::from_value::<DidOpenTextDocumentParams>(not.params)
                {
                    let doc = params.text_document;
                    self.documents.insert(
                        doc.uri,
                        DocumentState {
                            language_id: doc.language_id,
                            version: doc.version,
                            text: doc.text,
                        },
                    );
                }
            }
            "textDocument/didChange" => {
                if let Ok(params) =
                    serde_json::from_value::<DidChangeTextDocumentParams>(not.params)
                    && let Some(change) = params.content_changes.into_iter().last()
                    && let Some(state) = self.documents.get_mut(&params.text_document.uri)
                {
                    state.version = params.text_document.version;
                    state.text = change.text;
                }
            }
            "textDocument/didClose" => {
                if let Ok(params) = serde_json::from_value::<TextDocumentParams>(not.params) {
                    self.documents.remove(&params.text_document.uri);
                }
            }
            "workspace/didChangeConfiguration" => {
                if let Ok(params) =
                    serde_json::from_value::<DidChangeConfigurationParams>(not.params)
                {
                    self.set_settings(Settings::from_value(&params.settings));
                }
            }
            _ => {}
        }
    }

    fn set_settings(&mut self, settings: Settings) {
        self.service
            .set_data_providers(true, settings.data_providers());
        self.settings = settings;
    }
}

/// Resolves document-link references against the document URI using `url`.
struct UrlResolver;

impl DocumentContext for UrlResolver {
    fn resolve_reference(&self, reference: &str, base: &str) -> Option<String> {
        let base = url::Url::parse(base).ok()?;
        base.join(reference).ok().map(|u| u.to_string())
    }
}

fn parse<P: for<'de> Deserialize<'de>>(req: &Request) -> Result<P, Response> {
    serde_json::from_value(req.params.clone()).map_err(|e| {
        Response::new_err(
            req.id.clone(),
            ErrorCode::InvalidParams as i32,
            format!("invalid params for {}: {e}", req.method),
        )
    })
}

fn respond(connection: &Connection, id: lsp_server::RequestId, value: Value) {
    let _ = connection
        .sender
        .send(Message::Response(Response::new_ok(id, value)));
}

fn respond_null(connection: &Connection, id: lsp_server::RequestId) {
    respond(connection, id, Value::Null);
}

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let options = match cli::Options::parse(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("error: {error}\n");
            eprint!("{}", cli::help_text());
            std::process::exit(2);
        }
    };

    if options.help {
        print!("{}", cli::help_text());
        return Ok(());
    }
    if options.version {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let (connection, io_threads) = transport::create(&options.transport)?;

    // Exiting while the client is alive is a bug; `shutdown`/`exit` sets the
    // flag so the watchdog can report a clean exit afterwards.
    let shutdown_received = Arc::new(AtomicBool::new(false));
    if let Some(process_id) = options.client_process_id {
        watch_client_process(process_id, Arc::clone(&shutdown_received));
    }

    let capabilities = json!({
        "textDocumentSync": 1,
        "completionProvider": {
            "triggerCharacters": ["<", "\"", "'", "=", ".", "/", ":", " "],
            "resolveProvider": false
        },
        "hoverProvider": true,
        "documentHighlightProvider": true,
        "documentSymbolProvider": true,
        "foldingRangeProvider": true,
        "selectionRangeProvider": true,
        "documentFormattingProvider": true,
        "documentRangeFormattingProvider": true,
        "renameProvider": { "prepareProvider": false },
        "documentLinkProvider": { "resolveProvider": false },
        "linkedEditingRangeProvider": true
    });

    let initialize_params = connection.initialize(capabilities)?;

    // Upstream starts the same watchdog from `initializeParams.processId` when
    // no `--clientProcessId` was given on the command line.
    if options.client_process_id.is_none()
        && let Some(process_id) = initialize_params.get("processId").and_then(Value::as_u64)
    {
        watch_client_process(process_id as u32, Arc::clone(&shutdown_received));
    }

    let settings = initialize_params
        .get("initializationOptions")
        .map(Settings::from_value)
        .unwrap_or_default();
    let mut service = get_language_service(LanguageServiceOptions::default());
    service.set_data_providers(true, settings.data_providers());
    let mut server = Server {
        service,
        documents: HashMap::new(),
        settings,
    };

    for message in &connection.receiver {
        match message {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    shutdown_received.store(true, Ordering::SeqCst);
                    break;
                }
                server.handle_request(&connection, req)?;
            }
            Message::Notification(not) => server.handle_notification(not),
            Message::Response(_) => {}
        }
    }

    // Drop the connection so the writer channel closes before joining the
    // transport threads.
    drop(connection);
    io_threads.join()?;
    Ok(())
}

/// Watch `process_id` and exit once it disappears, mirroring
/// `vscode-languageserver`'s `--clientProcessId` exit timer.
fn watch_client_process(process_id: u32, shutdown_received: Arc<AtomicBool>) {
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(3));
            if !process_alive(process_id) {
                let code = if shutdown_received.load(Ordering::SeqCst) {
                    0
                } else {
                    1
                };
                std::process::exit(code);
            }
        }
    });
}

#[cfg(unix)]
fn process_alive(process_id: u32) -> bool {
    // `kill(pid, 0)` only performs error checking: success or `EPERM` means the
    // process exists, `ESRCH` means it is gone.
    unsafe extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    if unsafe { kill(process_id as i32, 0) } == 0 {
        return true;
    }
    const EPERM: i32 = 1;
    std::io::Error::last_os_error().raw_os_error() == Some(EPERM)
}

#[cfg(windows)]
fn process_alive(process_id: u32) -> bool {
    use std::ffi::c_void;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit: i32, process_id: u32) -> *mut c_void;
        fn WaitForSingleObject(handle: *mut c_void, milliseconds: u32) -> u32;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }

    const SYNCHRONIZE: u32 = 0x0010_0000;
    const WAIT_TIMEOUT: u32 = 0x0000_0102;

    unsafe {
        let handle = OpenProcess(SYNCHRONIZE, 0, process_id);
        if handle.is_null() {
            return false;
        }
        let status = WaitForSingleObject(handle, 0);
        CloseHandle(handle);
        status == WAIT_TIMEOUT
    }
}

#[cfg(not(any(unix, windows)))]
fn process_alive(_process_id: u32) -> bool {
    true
}
