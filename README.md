# html-languageservice

A Rust port of
[microsoft/vscode-html-languageservice](https://github.com/microsoft/vscode-html-languageservice).

The module layout, function names and control flow deliberately mirror the
TypeScript source so the two stay easy to diff and verify. Offsets, positions
and string semantics follow JavaScript (UTF-16), not Rust defaults.

This repository is a Cargo workspace with two crates:

| Crate | Path | Description |
|---|---|---|
| `html-languageservice` | `crates/languageservice/` | The library (1:1 port of upstream `src/`). |
| `html-languageserver` | `crates/languageserver/` | An LSP server binary built on the library. |

## Features

- **Parser / scanner** — `create_scanner`, `parse_html_document`, the HTML
  entity table, custom data providers.
- **Services** — completion, hover, highlights, document links, document
  symbols, folding, selection ranges, rename, linked editing, matching tag
  position.
- **Formatting** — the js-beautify HTML and CSS beautifiers, exposed through
  `LanguageService::format`, including embedded `<style>` CSS options.
- **Path completion** — `LanguageService::do_complete2` via a pluggable
  `FileSystemProvider`.
- **PHP blocks** — `<?php ... ?>` / `<?= ... ?>` regions are masked before
  scanning (via Mago's PHP lexer), so embedded PHP never parses as HTML.

Every service from upstream `src/` is ported.
`crates/languageservice/tests/*.rs` contains one integration test per
upstream `*.test.ts`.

## Usage

Add the crate to `Cargo.toml`:

```toml
[dependencies]
html-languageservice = { path = "crates/languageservice" }
```

### Completion

```rust
use html_languageservice::{get_language_service, LanguageServiceOptions, TextDocument};

let service = get_language_service(LanguageServiceOptions::default());

let document = TextDocument::create("test://test.html", "html", 0, "<div class=\"\">");
let html_document = service.parse_html_document(&document);

// Offset is measured in UTF-16 code units (here: inside the empty value).
let position = document.position_at(12);
let completions = service.do_complete(&document, position, &html_document, None);

for item in completions.items {
    println!("{}", item.label);
}
```

### Formatting

```rust
use html_languageservice::{
    apply_edits, get_language_service, HTMLFormatConfiguration, LanguageServiceOptions,
    TextDocument,
};

let service = get_language_service(LanguageServiceOptions::default());
let document =
    TextDocument::create("test://test.html", "html", 0, "<div  class = \"foo\"><br></div>");

let options = HTMLFormatConfiguration {
    tab_size: Some(2),
    insert_spaces: Some(true),
    ..Default::default()
};

// `None` formats the whole document; pass a `Range` to format a selection.
let edits = service.format(&document, None, &options);
let formatted = apply_edits(&document, &edits);
```

### Path completion

`do_complete2` needs a `FileSystemProvider`. Upstream's `readDirectory` returns
a promise; this crate is synchronous, so the callback returns the directory
entries directly.

```rust
use std::sync::Arc;

use html_languageservice::{
    get_language_service, DocumentContext, FileSystemProvider, FileType,
    LanguageServiceOptions, TextDocument,
};

struct Fs;

impl FileSystemProvider for Fs {
    fn read_directory(&self, uri: &str) -> Vec<(String, FileType)> {
        // Map `uri` to a directory and list its entries.
        Vec::new()
    }
}

let service = get_language_service(LanguageServiceOptions {
    file_system_provider: Some(Arc::new(Fs)),
    ..Default::default()
});

let document = TextDocument::create("file:///project/index.html", "html", 0, "");
let html_document = service.parse_html_document(&document);
let position = document.position_at(0);

struct Context;
impl DocumentContext for Context {
    fn resolve_reference(&self, reference: &str, base: &str) -> Option<String> {
        Some(reference.to_string())
    }
}

let _ = service.do_complete2(&document, position, &html_document, &Context, None);
```

## Differences from upstream

- **Synchronous file system.** `FileSystemProvider::read_directory` returns the
  entries directly instead of a `Promise`. `FileSystemProvider::stat` is not
  exposed because no service reads it.
- **Hand-ported LSP types.** The `Position`/`Range`/`CompletionItem`/… types
  are implemented in `crates/languageservice/src/types.rs` rather than
  pulled from `vscode-languageserver-types`, with only the fields the services
  use.
- **Rust error handling.** Upstream `throw`s become `Result` (`Uri::parse`);
  invalid beautify options are ignored (and logged to stderr) instead of
  throwing; a `tab_size` below 1 is treated as 1.
- **Deprecated aliases.** `findOnTypeRenameRanges` and
  `parseHTMLDocument(document, options)` are exposed as
  `find_on_type_rename_ranges` and `parse_html_document_with_options`.

## Language server

The `html-languageserver` crate provides an LSP server. Build and run it:

```sh
cargo build --release -p html-languageserver
./target/release/html-languageserver --stdio
```

The command line mirrors `vscode-languageserver`'s `createConnection()`, the
runtime behind the `vscode-langservers-extracted` npm bin:

```text
USAGE:
    html-languageserver [OPTIONS]

OPTIONS:
        --stdio                  Use standard input/output (default)
        --socket <PORT>          Connect to a TCP socket on 127.0.0.1:<PORT>
        --pipe <NAME>            Connect to a named pipe (Windows) or Unix
                                 domain socket (POSIX)
        --clientProcessId <PID>  Exit when the process with this id is gone
        --config <PATH>          Use this .toml file as the only config file
    -h, --help                   Print this help
    -v, --version                Print the version
```

`--socket` and `--pipe` connect to a listener owned by the client, matching
upstream's `createServerSocketTransport`/`createServerPipeTransport`.
`--node-ipc` is rejected because it needs a Node.js runtime. Exit code is `0`
after a graceful `shutdown`/`exit`, `1` when `--clientProcessId` fires before
shutdown.

Implemented requests: `textDocument/completion`, `hover`, `documentHighlight`,
`documentSymbol`, `foldingRange`, `selectionRange`, `formatting`,
`rangeFormatting`, `rename`, `documentLink`, `linkedEditingRange`, plus
`didOpen`/`didChange`(full sync)/`didClose`, plus
`didChangeConfiguration`, `didChangeWorkspaceFolders` and
`didChangeWatchedFiles`.

### Configuration

Settings come from, lowest precedence first:

1. `.html.toml` in `$XDG_CONFIG_HOME/html-languageserver/` (`%APPDATA%` on Windows);
   override the path with `HTML_LS_CONFIG`
2. `.html.toml` in every workspace folder (a deeper folder overrides a parent)
3. `initializationOptions`
4. `workspace/configuration` (only when the client advertises support)
5. `workspace/didChangeConfiguration`

`--config <PATH>` replaces both file layers (1 and 2): the given file is then
the only file-based configuration. An explicit file that is missing or invalid
is a fatal error (exit code 2); a discovered one only produces a
`window/showMessage` warning and is skipped.

Formatting options sent with `textDocument/formatting` (`tabSize`,
`insertSpaces`) override the configured ones for that request.

```toml
use_default_data_provider = true

[format]
wrap_line_length = 80
unformatted = ["code", "pre"]
indent_scripts = "normal"

[format.css]
newline_between_rules = false

[completion]
attribute_default_value = "doublequotes"   # empty | singlequotes | doublequotes
hide_end_tag_suggestions = true
providers = { html5 = false }              # false disables a provider

# Paths are relative to this file and are re-read on change.
custom_data = ["./data/web-components.html-data.json"]

# TOML arrays are homogeneous, so inline data goes in its own array.
[[custom_data_inline]]
version = 1.1
tags = [{ name = "my-tag" }]
```

Keys are snake_case; the camelCase spelling (`wrapLineLength`,
`hideEndTagSuggestions`, `useDefaultDataProvider`, …) is accepted too, so VS
Code `settings.json` fragments can be pasted verbatim under
`initializationOptions`.

Configuration is global: with several workspace folders the first `.html.toml`
found wins, because the language service holds a single data manager.

Point your editor's LSP client at the binary. For example, Neovim
(`nvim-lspconfig`) with a custom command:

```lua
vim.lsp.start({
  name = 'html-languageserver',
  cmd = { '/path/to/target/release/html-languageserver' },
  root_dir = vim.fn.getcwd(),
})
```

## Development

```sh
cargo test --workspace   # one test file per upstream *.test.ts
cargo clippy --workspace --all-targets
```

`ARCHITECTURE.md` documents the module layout, key types and design decisions.

## Attribution

This is a reimplementation of
[microsoft/vscode-html-languageservice](https://github.com/microsoft/vscode-html-languageservice),
which is licensed under the MIT License. The embedded js-beautify code and the
HTML data table originate from that project.
