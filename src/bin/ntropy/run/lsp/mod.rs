// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The `ntropy lsp` language server (ADR 0029, `docs/design/language-server.md`).
//!
//! A synchronous JSON-RPC server over `lsp-server`. This module owns the
//! lifecycle (initialize, encoding negotiation, the dispatch loop, and
//! shutdown) and is the only untested glue, mirroring how the interactive
//! picker keeps its terminal loop out of tests (ADR 0021, ADR 0027). The pure
//! logic it drives lives in unit-tested submodules.

mod cache;
mod completion;
mod documents;
mod navigation;
mod offset;
mod uri;
mod vault;

use std::collections::HashSet;
use std::process::ExitCode;

use anyhow::{Context, Result};
use lsp_server::{Connection, ErrorCode, Message, Notification, Request, RequestId, Response};
use lsp_types::notification::{
    DidChangeTextDocument, DidChangeWatchedFiles, DidCloseTextDocument, DidOpenTextDocument, Exit,
    Notification as _, ShowMessage,
};
use lsp_types::request::{
    Completion, DocumentLinkRequest, GotoDefinition, RegisterCapability, Request as _,
    WorkspaceSymbolRequest,
};
use lsp_types::{
    CompletionOptions, CompletionParams, DidChangeTextDocumentParams, DidChangeWatchedFilesParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DocumentLinkOptions, DocumentLinkParams,
    GotoDefinitionParams, InitializeParams, OneOf, PositionEncodingKind, ServerCapabilities,
    TextDocumentSyncCapability, TextDocumentSyncKind, Uri, WorkspaceSymbolParams,
    WorkspaceSymbolResponse,
};
use serde::Serialize;
use serde_json::json;

use cache::Cache;
use documents::Documents;

pub use offset::Encoding;

/// How a serve session ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    /// A clean `shutdown` then `exit`.
    Shutdown,
    /// `exit` without a preceding `shutdown`, or a dropped connection.
    Aborted,
}

impl Outcome {
    fn exit_code(self) -> ExitCode {
        match self {
            Outcome::Shutdown => ExitCode::SUCCESS,
            Outcome::Aborted => ExitCode::FAILURE,
        }
    }
}

/// Serve the language server over stdio. The entry point for `ntropy lsp`.
pub fn run() -> Result<ExitCode> {
    let (connection, io_threads) = Connection::stdio();
    let outcome = serve(&connection).context("while serving the language server")?;
    io_threads.join().context("while joining the I/O threads")?;
    Ok(outcome.exit_code())
}

/// Run the full lifecycle on a connection: initialize, then the dispatch loop.
///
/// Split from [`run`] so it can be driven over an in-memory connection in tests.
fn serve(connection: &Connection) -> Result<Outcome> {
    let (id, params) = connection
        .initialize_start()
        .context("while waiting for the initialize request")?;
    let params: InitializeParams =
        serde_json::from_value(params).context("while parsing the initialize parameters")?;
    let encoding = negotiate_encoding(&params);
    let dynamic_watchers = wants_dynamic_watchers(&params);
    let snippet_support = wants_snippets(&params);

    let result = json!({
        "capabilities": server_capabilities(encoding),
        "serverInfo": { "name": "ntropy", "version": env!("CARGO_PKG_VERSION") },
    });
    connection
        .initialize_finish(id, result)
        .context("while finishing initialization")?;

    let mut server = Server::new(connection, encoding, dynamic_watchers, snippet_support);
    // `initialize_finish` already consumed the `initialized` notification, so the
    // registration is sent here rather than from the dispatch loop.
    server.register_watchers()?;
    server.run()
}

/// Pick UTF-8 when the client offers it, otherwise the protocol default UTF-16.
fn negotiate_encoding(params: &InitializeParams) -> Encoding {
    let offered = params
        .capabilities
        .general
        .as_ref()
        .and_then(|general| general.position_encodings.as_ref());
    match offered {
        Some(encodings) if encodings.contains(&PositionEncodingKind::UTF8) => Encoding::Utf8,
        _ => Encoding::Utf16,
    }
}

/// Whether the client supports dynamic registration of file watchers.
fn wants_dynamic_watchers(params: &InitializeParams) -> bool {
    params
        .capabilities
        .workspace
        .as_ref()
        .and_then(|workspace| workspace.did_change_watched_files.as_ref())
        .and_then(|watched| watched.dynamic_registration)
        .unwrap_or(false)
}

/// Whether the client renders snippet completion items (`$0` placeholders).
fn wants_snippets(params: &InitializeParams) -> bool {
    params
        .capabilities
        .text_document
        .as_ref()
        .and_then(|doc| doc.completion.as_ref())
        .and_then(|completion| completion.completion_item.as_ref())
        .and_then(|item| item.snippet_support)
        .unwrap_or(false)
}

/// The capabilities advertised to the client. Feature phases extend this.
fn server_capabilities(encoding: Encoding) -> ServerCapabilities {
    ServerCapabilities {
        position_encoding: Some(encoding_kind(encoding)),
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        completion_provider: Some(CompletionOptions {
            trigger_characters: Some(vec!["[".to_owned(), "(".to_owned()]),
            ..Default::default()
        }),
        definition_provider: Some(OneOf::Left(true)),
        document_link_provider: Some(DocumentLinkOptions {
            resolve_provider: Some(false),
            work_done_progress_options: Default::default(),
        }),
        workspace_symbol_provider: Some(OneOf::Left(true)),
        ..Default::default()
    }
}

fn encoding_kind(encoding: Encoding) -> PositionEncodingKind {
    match encoding {
        Encoding::Utf8 => PositionEncodingKind::UTF8,
        Encoding::Utf16 => PositionEncodingKind::UTF16,
    }
}

/// The running server: connection, negotiated settings, and session state.
struct Server<'c> {
    connection: &'c Connection,
    encoding: Encoding,
    dynamic_watchers: bool,
    snippet_support: bool,
    documents: Documents,
    cache: Cache,
}

impl<'c> Server<'c> {
    fn new(
        connection: &'c Connection,
        encoding: Encoding,
        dynamic_watchers: bool,
        snippet_support: bool,
    ) -> Self {
        Self {
            connection,
            encoding,
            dynamic_watchers,
            snippet_support,
            documents: Documents::new(),
            cache: Cache::new(),
        }
    }

    /// Dispatch messages until the connection shuts down or the channel closes.
    fn run(&mut self) -> Result<Outcome> {
        while let Ok(message) = self.connection.receiver.recv() {
            match message {
                Message::Request(request) => {
                    if self
                        .connection
                        .handle_shutdown(&request)
                        .context("while handling the shutdown request")?
                    {
                        return Ok(Outcome::Shutdown);
                    }
                    self.handle_request(request)?;
                }
                Message::Notification(notification) => {
                    if let Some(outcome) = self.handle_notification(notification)? {
                        return Ok(outcome);
                    }
                }
                // Responses to our own requests (e.g. capability registration).
                Message::Response(_) => {}
            }
        }
        // The channel closed without a clean shutdown handshake.
        Ok(Outcome::Aborted)
    }

    /// Dispatch a request to its handler, falling back to `MethodNotFound`.
    fn handle_request(&mut self, request: Request) -> Result<()> {
        match request.method.as_str() {
            Completion::METHOD => self.on_completion(request),
            GotoDefinition::METHOD => self.on_definition(request),
            DocumentLinkRequest::METHOD => self.on_document_link(request),
            WorkspaceSymbolRequest::METHOD => self.on_workspace_symbol(request),
            _ => self.respond_unhandled(request),
        }
    }

    /// Answer a `textDocument/completion` request.
    fn on_completion(&mut self, request: Request) -> Result<()> {
        let id = request.id.clone();
        let result = serde_json::from_value::<CompletionParams>(request.params)
            .ok()
            .and_then(|params| self.completion(&params));
        self.respond_with(id, result)
    }

    /// Compute completions for a request, or `None` when there is nothing to
    /// offer (document not open, not in a vault, or no link context).
    fn completion(&mut self, params: &CompletionParams) -> Option<lsp_types::CompletionList> {
        let uri = &params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let text = self.documents.get(uri)?.to_owned();
        let vault::Lookup::Found(vault) = vault::for_document(uri) else {
            return None;
        };
        let entries = self.cache.entries(&vault);
        let offset = offset::position_to_offset(&text, position, self.encoding);
        completion::complete(&text, offset, self.encoding, entries, self.snippet_support)
    }

    /// Answer a `textDocument/definition` request.
    fn on_definition(&mut self, request: Request) -> Result<()> {
        let id = request.id.clone();
        let result = serde_json::from_value::<GotoDefinitionParams>(request.params)
            .ok()
            .and_then(|params| {
                let position = params.text_document_position_params;
                let uri = &position.text_document.uri;
                let text = self.documents.get(uri)?.to_owned();
                let vault::Lookup::Found(vault) = vault::for_document(uri) else {
                    return None;
                };
                let entries = self.cache.entries(&vault);
                let offset = offset::position_to_offset(&text, position.position, self.encoding);
                navigation::definition(&text, offset, entries)
            });
        self.respond_with(id, result)
    }

    /// Answer a `textDocument/documentLink` request.
    fn on_document_link(&mut self, request: Request) -> Result<()> {
        let id = request.id.clone();
        let result = serde_json::from_value::<DocumentLinkParams>(request.params)
            .ok()
            .and_then(|params| {
                let uri = &params.text_document.uri;
                let text = self.documents.get(uri)?.to_owned();
                let vault::Lookup::Found(vault) = vault::for_document(uri) else {
                    return None;
                };
                let entries = self.cache.entries(&vault);
                Some(navigation::document_links(&text, self.encoding, entries))
            });
        self.respond_with(id, result)
    }

    /// Answer a `workspace/symbol` request over every touched vault.
    fn on_workspace_symbol(&mut self, request: Request) -> Result<()> {
        let id = request.id.clone();
        let result = serde_json::from_value::<WorkspaceSymbolParams>(request.params)
            .ok()
            .map(|params| {
                let entries = self.cache.all_entries();
                WorkspaceSymbolResponse::Nested(navigation::workspace_symbols(
                    &params.query,
                    &entries,
                ))
            });
        self.respond_with(id, result)
    }

    /// Send a successful response, using JSON `null` for an absent result.
    fn respond_with<T: Serialize>(&self, id: RequestId, result: Option<T>) -> Result<()> {
        let response = match result {
            Some(value) => Response::new_ok(id, value),
            None => Response::new_ok(id, serde_json::Value::Null),
        };
        self.connection
            .sender
            .send(Message::Response(response))
            .context("while sending a response")
    }

    /// Handle a notification, returning an [`Outcome`] only when it ends serving.
    fn handle_notification(&mut self, notification: Notification) -> Result<Option<Outcome>> {
        match notification.method.as_str() {
            Exit::METHOD => return Ok(Some(Outcome::Aborted)),
            DidOpenTextDocument::METHOD => {
                if let Ok(params) =
                    serde_json::from_value::<DidOpenTextDocumentParams>(notification.params)
                {
                    self.on_did_open(params)?;
                }
            }
            DidChangeTextDocument::METHOD => {
                if let Ok(params) =
                    serde_json::from_value::<DidChangeTextDocumentParams>(notification.params)
                {
                    self.on_did_change(params);
                }
            }
            DidCloseTextDocument::METHOD => {
                if let Ok(params) =
                    serde_json::from_value::<DidCloseTextDocumentParams>(notification.params)
                {
                    self.documents.remove(&params.text_document.uri);
                }
            }
            DidChangeWatchedFiles::METHOD => {
                if let Ok(params) =
                    serde_json::from_value::<DidChangeWatchedFilesParams>(notification.params)
                {
                    self.on_watched_files(params);
                }
            }
            // $/setTrace, $/cancelRequest and the like need no action.
            _ => {}
        }
        Ok(None)
    }

    /// Store an opened document and, without a client watcher, refresh its vault.
    fn on_did_open(&mut self, params: DidOpenTextDocumentParams) -> Result<()> {
        let document = params.text_document;
        let uri = document.uri;
        self.documents.set(uri.clone(), document.text);
        if !self.dynamic_watchers {
            self.refresh_vault(&uri)?;
        }
        Ok(())
    }

    /// Replace a document's text. Sync is FULL, so the last change is the buffer.
    fn on_did_change(&mut self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(change) = params.content_changes.into_iter().next_back() {
            self.documents.set(uri, change.text);
        }
    }

    /// Invalidate each affected vault once, coalescing a burst of file events.
    fn on_watched_files(&mut self, params: DidChangeWatchedFilesParams) {
        let mut invalidated = HashSet::new();
        for change in params.changes {
            if let vault::Lookup::Found(found) = vault::for_document(&change.uri) {
                let root = found.root().to_path_buf();
                if invalidated.insert(root.clone()) {
                    self.cache.invalidate(&root);
                }
            }
        }
    }

    /// Drop the cached entries for a document's vault so the next read rescans.
    fn refresh_vault(&mut self, uri: &Uri) -> Result<()> {
        match vault::for_document(uri) {
            vault::Lookup::Found(found) => self.cache.invalidate(found.root()),
            vault::Lookup::Broken(message) => self.show_error(&message)?,
            vault::Lookup::None => {}
        }
        Ok(())
    }

    /// Ask the client to watch `**/*.md` so on-disk changes refresh the cache.
    fn register_watchers(&self) -> Result<()> {
        if !self.dynamic_watchers {
            return Ok(());
        }
        let params = json!({
            "registrations": [{
                "id": "ntropy-watched-files",
                "method": "workspace/didChangeWatchedFiles",
                "registerOptions": { "watchers": [{ "globPattern": "**/*.md" }] },
            }],
        });
        let request = Request {
            id: RequestId::from("ntropy-register-watched-files".to_owned()),
            method: RegisterCapability::METHOD.to_owned(),
            params,
        };
        self.connection
            .sender
            .send(Message::Request(request))
            .context("while registering file watchers")
    }

    /// Show the user an error message (e.g. a broken vault pointer).
    fn show_error(&self, message: &str) -> Result<()> {
        let notification = Notification {
            method: ShowMessage::METHOD.to_owned(),
            // 1 is `MessageType::ERROR`.
            params: json!({ "type": 1, "message": message }),
        };
        self.connection
            .sender
            .send(Message::Notification(notification))
            .context("while sending a message to the client")
    }

    /// Reply to a request whose method this phase does not implement.
    fn respond_unhandled(&self, request: Request) -> Result<()> {
        let response = Response::new_err(
            request.id,
            ErrorCode::MethodNotFound as i32,
            format!("unsupported method: {}", request.method),
        );
        self.connection
            .sender
            .send(Message::Response(response))
            .context("while sending a response")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::{self, JoinHandle};

    /// Spawn the server on an in-memory connection, returning the client end and
    /// the server's join handle (yielding its [`Outcome`]).
    fn start() -> (Connection, JoinHandle<Outcome>) {
        let (server, client) = Connection::memory();
        let handle = thread::spawn(move || serve(&server).expect("serve runs cleanly"));
        (client, handle)
    }

    fn request(id: i32, method: &str, params: serde_json::Value) -> Message {
        Message::Request(Request {
            id: RequestId::from(id),
            method: method.to_owned(),
            params,
        })
    }

    fn notification(method: &str, params: serde_json::Value) -> Message {
        Message::Notification(Notification {
            method: method.to_owned(),
            params,
        })
    }

    fn recv(client: &Connection) -> Message {
        client.receiver.recv().expect("the server sends a message")
    }

    /// Drive the initialize handshake with the given client `capabilities` and
    /// return the server's `InitializeResult`.
    fn initialize(client: &Connection, capabilities: serde_json::Value) -> serde_json::Value {
        client
            .sender
            .send(request(
                1,
                "initialize",
                json!({ "capabilities": capabilities }),
            ))
            .unwrap();
        let Message::Response(response) = recv(client) else {
            panic!("expected the initialize response");
        };
        client
            .sender
            .send(notification("initialized", json!({})))
            .unwrap();
        response.result.expect("initialize result")
    }

    /// Cleanly stop the server and assert it reports a clean shutdown.
    fn shutdown(client: &Connection, handle: JoinHandle<Outcome>) {
        client
            .sender
            .send(request(99, "shutdown", json!(null)))
            .unwrap();
        let Message::Response(_) = recv(client) else {
            panic!("expected the shutdown response");
        };
        client
            .sender
            .send(notification("exit", json!(null)))
            .unwrap();
        assert_eq!(handle.join().unwrap(), Outcome::Shutdown);
    }

    fn position_encoding(result: &serde_json::Value) -> String {
        result["capabilities"]["positionEncoding"]
            .as_str()
            .unwrap_or("utf-16")
            .to_owned()
    }

    #[test]
    fn advertises_full_sync_and_negotiated_encoding() {
        let (client, handle) = start();
        let result = initialize(
            &client,
            json!({ "general": { "positionEncodings": ["utf-8", "utf-16"] } }),
        );
        assert_eq!(result["capabilities"]["textDocumentSync"], json!(1));
        assert_eq!(position_encoding(&result), "utf-8");
        shutdown(&client, handle);
    }

    #[test]
    fn picks_utf16_when_utf8_not_offered() {
        let (client, handle) = start();
        let result = initialize(
            &client,
            json!({ "general": { "positionEncodings": ["utf-16"] } }),
        );
        assert_eq!(position_encoding(&result), "utf-16");
        shutdown(&client, handle);
    }

    #[test]
    fn defaults_to_utf16_when_no_encodings_offered() {
        let (client, handle) = start();
        let result = initialize(&client, json!({}));
        assert_eq!(position_encoding(&result), "utf-16");
        shutdown(&client, handle);
    }

    #[test]
    fn unknown_request_gets_method_not_found_and_loop_continues() {
        let (client, handle) = start();
        initialize(&client, json!({}));
        client
            .sender
            .send(request(2, "textDocument/foo", json!({})))
            .unwrap();
        let Message::Response(response) = recv(&client) else {
            panic!("expected an error response");
        };
        let error = response.error.expect("error payload");
        assert_eq!(error.code, ErrorCode::MethodNotFound as i32);
        // The loop is still alive: a clean shutdown still works.
        shutdown(&client, handle);
    }

    #[test]
    fn unknown_notification_is_ignored() {
        let (client, handle) = start();
        initialize(&client, json!({}));
        client
            .sender
            .send(notification("$/setTrace", json!({ "value": "off" })))
            .unwrap();
        // No response is expected; the server stays responsive to shutdown.
        shutdown(&client, handle);
    }

    #[test]
    fn exit_without_shutdown_aborts() {
        let (server, client) = Connection::memory();
        let handle = thread::spawn(move || serve(&server).expect("serve runs cleanly"));
        initialize(&client, json!({}));
        client
            .sender
            .send(notification("exit", json!(null)))
            .unwrap();
        assert_eq!(handle.join().unwrap(), Outcome::Aborted);
    }

    #[test]
    fn registers_a_markdown_watcher_when_supported() {
        let (client, handle) = start();
        initialize(
            &client,
            json!({ "workspace": { "didChangeWatchedFiles": { "dynamicRegistration": true } } }),
        );
        let Message::Request(register) = recv(&client) else {
            panic!("expected a registration request");
        };
        assert_eq!(register.method, "client/registerCapability");
        let registration = &register.params["registrations"][0];
        assert_eq!(registration["method"], "workspace/didChangeWatchedFiles");
        assert_eq!(
            registration["registerOptions"]["watchers"][0]["globPattern"],
            "**/*.md"
        );
        shutdown(&client, handle);
    }

    #[test]
    fn does_not_register_a_watcher_when_unsupported() {
        let (client, handle) = start();
        initialize(&client, json!({}));
        // With no registration sent, the next message is the shutdown response.
        client
            .sender
            .send(request(99, "shutdown", json!(null)))
            .unwrap();
        let Message::Response(response) = recv(&client) else {
            panic!("expected the shutdown response, not a registration");
        };
        assert!(response.error.is_none());
        client
            .sender
            .send(notification("exit", json!(null)))
            .unwrap();
        assert_eq!(handle.join().unwrap(), Outcome::Shutdown);
    }

    #[test]
    fn document_lifecycle_notifications_keep_the_server_responsive() {
        let (client, handle) = start();
        initialize(&client, json!({}));
        let uri = "file:///tmp/ntropy-test/note.md";
        let open = json!({
            "textDocument": { "uri": uri, "languageId": "markdown", "version": 1, "text": "hello" }
        });
        client
            .sender
            .send(notification("textDocument/didOpen", open))
            .unwrap();
        let change = json!({
            "textDocument": { "uri": uri, "version": 2 },
            "contentChanges": [{ "text": "hello world" }]
        });
        client
            .sender
            .send(notification("textDocument/didChange", change))
            .unwrap();
        let close = json!({ "textDocument": { "uri": uri } });
        client
            .sender
            .send(notification("textDocument/didClose", close))
            .unwrap();
        // The server processed all three and remains responsive.
        shutdown(&client, handle);
    }

    #[test]
    fn watched_files_notification_is_handled() {
        let (client, handle) = start();
        initialize(&client, json!({}));
        let changed = json!({
            "changes": [{ "uri": "file:///tmp/ntropy-test/a.md", "type": 2 }]
        });
        client
            .sender
            .send(notification("workspace/didChangeWatchedFiles", changed))
            .unwrap();
        shutdown(&client, handle);
    }

    // ===== End-to-end flows over the in-memory connection and a real vault =====

    const ULID_TARGET: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const ULID_NEW: &str = "01BRZ3NDEKTSV4RRFFQ69G5FAV";

    /// Create a temporary vault with the given `<ulid>-<slug>.md` notes, returning
    /// the guard and the canonicalized root.
    fn temp_vault(notes: &[(&str, &str, &str)]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = std::fs::canonicalize(dir.path()).expect("canonicalize");
        std::fs::create_dir_all(root.join("all-notes")).expect("all-notes");
        std::fs::create_dir_all(root.join(".ntropy")).expect(".ntropy");
        for (ulid, slug, content) in notes {
            std::fs::write(
                root.join("all-notes").join(format!("{ulid}-{slug}.md")),
                content,
            )
            .expect("write note");
        }
        (dir, root)
    }

    /// The `file:` URI of a path inside the vault's `all-notes/`.
    fn note_uri(root: &std::path::Path, name: &str) -> String {
        format!("file://{}", root.join("all-notes").join(name).display())
    }

    fn did_open(client: &Connection, uri: &str, text: &str) {
        client
            .sender
            .send(notification(
                "textDocument/didOpen",
                json!({
                    "textDocument": {
                        "uri": uri, "languageId": "markdown", "version": 1, "text": text,
                    }
                }),
            ))
            .unwrap();
    }

    /// Send a request and return its result value.
    fn call(client: &Connection, method: &str, params: serde_json::Value) -> serde_json::Value {
        client.sender.send(request(10, method, params)).unwrap();
        let Message::Response(response) = recv(client) else {
            panic!("expected a response to {method}");
        };
        response.result.expect("a result")
    }

    fn completion_labels(result: &serde_json::Value) -> Vec<String> {
        result["items"]
            .as_array()
            .expect("items array")
            .iter()
            .map(|item| item["label"].as_str().unwrap_or_default().to_owned())
            .collect()
    }

    #[test]
    fn completion_offers_vault_notes_end_to_end() {
        let (_dir, root) =
            temp_vault(&[(ULID_TARGET, "target", "---\ntitle: Target\n---\nbody\n")]);
        let (client, handle) = start();
        initialize(
            &client,
            json!({ "general": { "positionEncodings": ["utf-8"] } }),
        );

        let uri = note_uri(&root, "source.md");
        did_open(&client, &uri, "see [Tar");
        let result = call(
            &client,
            "textDocument/completion",
            json!({ "textDocument": { "uri": uri }, "position": { "line": 0, "character": 8 } }),
        );
        assert!(completion_labels(&result).contains(&"Target".to_owned()));
        shutdown(&client, handle);
    }

    #[test]
    fn definition_jumps_to_target_end_to_end() {
        let (_dir, root) =
            temp_vault(&[(ULID_TARGET, "target", "---\ntitle: Target\n---\nbody\n")]);
        let (client, handle) = start();
        initialize(
            &client,
            json!({ "general": { "positionEncodings": ["utf-8"] } }),
        );

        let uri = note_uri(&root, "source.md");
        did_open(&client, &uri, &format!("[x]({ULID_TARGET}-target.md)"));
        let result = call(
            &client,
            "textDocument/definition",
            json!({ "textDocument": { "uri": uri }, "position": { "line": 0, "character": 1 } }),
        );
        assert!(
            result["uri"]
                .as_str()
                .expect("a target uri")
                .ends_with(&format!("{ULID_TARGET}-target.md"))
        );
        shutdown(&client, handle);
    }

    #[test]
    fn workspace_symbol_finds_notes_end_to_end() {
        let (_dir, root) =
            temp_vault(&[(ULID_TARGET, "target", "---\ntitle: Target\n---\nbody\n")]);
        let (client, handle) = start();
        initialize(
            &client,
            json!({ "general": { "positionEncodings": ["utf-8"] } }),
        );

        // Open a document so the vault's cache is populated.
        let uri = note_uri(&root, "source.md");
        did_open(&client, &uri, "[Tar");
        call(
            &client,
            "textDocument/completion",
            json!({ "textDocument": { "uri": uri }, "position": { "line": 0, "character": 4 } }),
        );

        let symbols = call(&client, "workspace/symbol", json!({ "query": "Target" }));
        let names: Vec<&str> = symbols
            .as_array()
            .expect("symbol array")
            .iter()
            .map(|symbol| symbol["name"].as_str().unwrap_or_default())
            .collect();
        assert!(names.contains(&"Target"));
        shutdown(&client, handle);
    }

    #[test]
    fn watched_file_change_refreshes_completion_end_to_end() {
        let (_dir, root) =
            temp_vault(&[(ULID_TARGET, "target", "---\ntitle: Target\n---\nbody\n")]);
        let (client, handle) = start();
        initialize(
            &client,
            json!({ "general": { "positionEncodings": ["utf-8"] } }),
        );

        let uri = note_uri(&root, "source.md");
        did_open(&client, &uri, "[");
        let params =
            json!({ "textDocument": { "uri": uri }, "position": { "line": 0, "character": 1 } });
        let before = call(&client, "textDocument/completion", params.clone());
        assert_eq!(completion_labels(&before).len(), 1);

        // Create a new note on disk and report it via the watcher.
        std::fs::write(
            root.join("all-notes").join(format!("{ULID_NEW}-new.md")),
            "---\ntitle: New Note\n---\nbody\n",
        )
        .unwrap();
        client
            .sender
            .send(notification(
                "workspace/didChangeWatchedFiles",
                json!({ "changes": [{ "uri": note_uri(&root, &format!("{ULID_NEW}-new.md")), "type": 1 }] }),
            ))
            .unwrap();

        let after = call(&client, "textDocument/completion", params);
        assert_eq!(completion_labels(&after).len(), 2);
        shutdown(&client, handle);
    }
}
