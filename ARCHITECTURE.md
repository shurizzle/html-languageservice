# Architecture

Rust reimplementation of
[microsoft/vscode-html-languageservice](https://github.com/microsoft/vscode-html-languageservice).
Module layout, function names and control flow deliberately mirror the
TypeScript source so ports stay easy to diff/verify.

Workspace keeps each crate under `crates/`:

| Crate | Path |
|---|---|
| `html-languageservice` (library) | `crates/html-languageservice/` |
| `html-languageserver` (bin) | `crates/html-languageserver/` |

## Directory layout

Mirrors upstream `src/` (paths below are relative to
`crates/html-languageservice/`):

| Path | Upstream | Responsibility |
|---|---|---|
| `src/lib.rs` | `htmlLanguageService.ts` | crate root + public re-exports |
| `src/language_service.rs` | `htmlLanguageService.ts` | `LanguageService` facade (`get_language_service`) |
| `src/types.rs` | `htmlLanguageTypes.ts` | LSP types, custom-data types, options |
| `src/text_document.rs` | `vscode-languageserver-textdocument` | `TextDocument` (UTF-16 offsets, line offsets) |
| `src/uri.rs` | `vscode-uri` | subset of `URI` used by links |
| `src/html_language_types.rs` | `htmlLanguageTypes.ts` | `TokenType`, `ScannerState` |
| `src/parser/html_scanner.rs` | `parser/htmlScanner.ts` | tokenizer |
| `src/parser/html_parser.rs` | `parser/htmlParser.ts` | document tree |
| `src/parser/html_entities.rs` | `parser/htmlEntities.ts` | entity table (generated) |
| `src/parser/php.rs` | — | masks `<?php ... ?>` blocks before scanning (mago lexer) |
| `src/language_facts/data_provider.rs` | `languageFacts/dataProvider.ts` | `HtmlDataProvider`, docs/baseline |
| `src/language_facts/data_manager.rs` | `languageFacts/dataManager.ts` | provider registry, void/path attrs |
| `src/language_facts/data/web_custom_data.json` | `data/webCustomData.ts` | default HTML data (116 tags) |
| `src/services/*.rs` | `services/*.ts` | one module per service |
| `src/beautify/*.rs` | `beautify/*.js` | js-beautify core (`scanner`/`output`/`options`), `css`, `html` |
| `src/utils/*.rs` | `utils/*.ts` | small string/array helpers |
| `crates/html-languageserver/` | — | LSP server crate (`cli`/`transport`/`main`) built on this library |

## Key types

- `TokenType`, `ScannerState` (`html_language_types.rs`) — plain enums.
- `Scanner` (`html_scanner.rs`) — borrows nothing; owns a `Vec<u16>` of the
  source so it can slice UTF-16 units. `create_scanner(input, initial_offset,
  initial_state, emit_pseudo_close_tags)`.
- `HtmlDocument`/`Node`/`NodeId` (`html_parser.rs`) — **arena**: `Vec<Node>`
  addressed by `NodeId` (a `usize`). Parent links are `Option<NodeId>`, so no
  `Rc<RefCell<..>>` and no reference cycles. Root node is id `0`; `roots()` are
  its children (matching upstream's synthetic document node).
- `TextDocument` (`text_document.rs`) — stores text + UTF-16 line offsets and
  byte offsets so `position_at`/`offset_at` match JS string semantics.
- `LanguageService` (`language_service.rs`) — owns `DataManager` (and options).
  Services are constructed per call borrowing `&self.data_manager`, avoiding a
  self-referential struct. Completion participants are stored on the facade and
  passed into `do_complete`.
- `Uri` (`uri.rs`) — `parse`/`with_query`/`with_fragment`/`to_string_with`;
  `parse` returns `Result` where upstream throws.

## Control flow

1. `TextDocument` → `parse_html_document` → `parse()` (scanner → `HtmlDocument`).
2. Services take `(&TextDocument, Position|&HtmlDocument, ...)` and return LSP
   structs. `html_scanner` is re-run by services that need token-level detail
   (highlighting, hover, completion, selection range, folding, links).
3. `HtmlCompletion` is the largest service: one `do_complete_inner` scan loop
   dispatching per `TokenType`, appending to a shared `result` list (helpers
   return `self.result.clone()`, matching upstream's shared mutable list).

## Data flow / invariants

- **Offsets are UTF-16 code units** everywhere (scanner token offsets, node
  `start`/`end`, `Position.character`), matching JS. `TextDocument::byte_offset_at`
  bridges to byte slices when needed. `TextDocument::get_scanner_text()` caches a
  copy with PHP blocks blanked out (same UTF-16 offsets), rebuilt only when the
  document version changes.
- `Node` attributes are an insertion-ordered `Vec<(String, Option<String>)>`
  (`None` = valueless), reproducing JS object key order for symbol names
  (`div#id.class`).
- Default HTML data is embedded via `include_str!` of `web_custom_data.json`
  and parsed once (`OnceLock` + `serde_json`).
- Entity/hover/baseline strings are copied verbatim from upstream (invisible
  code points are `\u{..}`-escaped so clippy stays clean).

## Design decisions

- Deps stay small: `serde`/`serde_json` (data loading) and `mago-syntax`
  (PHP block detection). URI and text-document logic are ported rather than
  pulled in, to control the exact UTF-16/RFC-3986 semantics the tests assert.
- `getLanguageService()` becomes `get_language_service(LanguageServiceOptions)`;
  `LanguageServiceOptions::default()` = upstream's no-arg call.
- `vscode-uri`'s "namespace/static" helpers become associated functions /
  free functions.

## Feature parity notes

- All services from `src/` are ported. `doComplete2` / `pathCompletion.ts`
  use a synchronous `FileSystemProvider` callback (upstream's is a promise),
  covered by `tests/path_completions.rs` with fixtures under
  `tests/path_completion_fixtures/`. `htmlFormatter` / `beautify` (including
  the embedded CSS options) are covered by `tests/formatter.rs`.
- `findOnTypeRenameRanges` is available as
  `LanguageService::find_on_type_rename_ranges`;
  `parseHTMLDocument(document, options)` as
  `LanguageService::parse_html_document_with_options`.
- `FileSystemProvider::stat` (upstream interface member) is not exposed: no
  service reads it, only `readDirectory` is used.

## Entry points

- Library: `get_language_service(...)` in `language_service.rs`.
- Server configuration: `crates/languageserver/src/config.rs` loads `.html.toml`
  (user dir or workspace roots, or a `--config` file) and
  `crates/languageserver/src/settings.rs` merges the layers
  (file < `initializationOptions` < `workspace/configuration` <
  `didChangeConfiguration`); `crates/languageserver/src/workspace.rs` tracks
  workspace folders and maps document URIs to roots.
- LSP server binary: `crates/html-languageserver/src/main.rs` (`lsp-server`
  transport, no `lsp-types` dependency; params/results via `serde_json`). CLI
  parsing is in `crates/html-languageserver/src/cli.rs` and transport setup in
  `crates/html-languageserver/src/transport.rs` (stdio/TCP from `lsp-server`,
  named pipe/Unix socket via the public `Message` API). Settings
  (`format`/`completion`/`customData`) come from `initializationOptions` and
  `workspace/didChangeConfiguration`, parsed in `src/settings.rs`; custom data
  entries are inline `HTMLDataV1` objects (no file paths).
- Tests: `crates/html-languageservice/tests/*.rs` (integration crates), one file
  per upstream `*.test.ts`, with shared helpers in
  `crates/html-languageservice/tests/support/mod.rs` (document builder, `|`
  marker splitting, and a Node-`url.resolve`-compatible resolver for link
  tests).
