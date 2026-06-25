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

// The offset conversions are the tested foundation that ranges depend on. Their
// callers (link/tag completion and navigation) land in later phases, so the
// module has no non-test caller yet; the allow is removed once it is wired in.
#[allow(dead_code)]
mod offset;

use std::process::ExitCode;

use anyhow::{Context, Result};
use lsp_server::{Connection, ErrorCode, Message, Request, Response};
use lsp_types::notification::{Exit, Notification as _};
use lsp_types::{
    InitializeParams, PositionEncodingKind, ServerCapabilities, TextDocumentSyncCapability,
    TextDocumentSyncKind,
};
use serde_json::json;

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

    let result = json!({
        "capabilities": server_capabilities(encoding),
        "serverInfo": { "name": "ntropy", "version": env!("CARGO_PKG_VERSION") },
    });
    connection
        .initialize_finish(id, result)
        .context("while finishing initialization")?;

    main_loop(connection, encoding)
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

/// The capabilities advertised to the client. Feature phases extend this.
fn server_capabilities(encoding: Encoding) -> ServerCapabilities {
    ServerCapabilities {
        position_encoding: Some(encoding_kind(encoding)),
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        ..Default::default()
    }
}

fn encoding_kind(encoding: Encoding) -> PositionEncodingKind {
    match encoding {
        Encoding::Utf8 => PositionEncodingKind::UTF8,
        Encoding::Utf16 => PositionEncodingKind::UTF16,
    }
}

/// Dispatch messages until the connection shuts down or the channel closes.
fn main_loop(connection: &Connection, _encoding: Encoding) -> Result<Outcome> {
    while let Ok(message) = connection.receiver.recv() {
        match message {
            Message::Request(request) => {
                if connection
                    .handle_shutdown(&request)
                    .context("while handling the shutdown request")?
                {
                    return Ok(Outcome::Shutdown);
                }
                respond_unhandled(connection, request)?;
            }
            Message::Notification(notification) => {
                if notification.method == Exit::METHOD {
                    // `exit` without a preceding `shutdown` is an abnormal exit.
                    return Ok(Outcome::Aborted);
                }
                // Other notifications ($/setTrace, $/cancelRequest, ...) are
                // safely ignored until a later phase handles them.
            }
            Message::Response(_) => {}
        }
    }
    // The channel closed without a clean shutdown handshake.
    Ok(Outcome::Aborted)
}

/// Reply to a request whose method this phase does not implement.
fn respond_unhandled(connection: &Connection, request: Request) -> Result<()> {
    let response = Response::new_err(
        request.id,
        ErrorCode::MethodNotFound as i32,
        format!("unsupported method: {}", request.method),
    );
    connection
        .sender
        .send(Message::Response(response))
        .context("while sending a response")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_server::{Notification, RequestId};
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

    /// Drive the initialize handshake with the given client `general`
    /// capabilities and return the server's `InitializeResult`.
    fn initialize(client: &Connection, general: serde_json::Value) -> serde_json::Value {
        client
            .sender
            .send(request(
                1,
                "initialize",
                json!({ "capabilities": { "general": general } }),
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
        let result = initialize(&client, json!({ "positionEncodings": ["utf-8", "utf-16"] }));
        assert_eq!(result["capabilities"]["textDocumentSync"], json!(1));
        assert_eq!(position_encoding(&result), "utf-8");
        shutdown(&client, handle);
    }

    #[test]
    fn picks_utf16_when_utf8_not_offered() {
        let (client, handle) = start();
        let result = initialize(&client, json!({ "positionEncodings": ["utf-16"] }));
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
}
