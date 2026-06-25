// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The set of open document buffers (ADR 0029).
//!
//! Documents sync as `TextDocumentSyncKind::FULL`, so each change carries the
//! whole text and the store simply replaces it. The buffer is only read to
//! determine cursor context for a request; it is never merged into the note
//! cache, which reflects saved on-disk state.

use std::collections::HashMap;

use lsp_types::Uri;

/// Open document buffers, keyed by URI.
#[derive(Debug, Default)]
pub struct Documents {
    texts: HashMap<Uri, String>,
}

impl Documents {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record (or replace) a document's full text.
    pub fn set(&mut self, uri: Uri, text: String) {
        self.texts.insert(uri, text);
    }

    /// Forget a closed document.
    pub fn remove(&mut self, uri: &Uri) {
        self.texts.remove(uri);
    }

    /// The current text of an open document, if any.
    ///
    /// Read by completion in the next phase to determine cursor context; the
    /// dispatch loop only writes the store, so the allow is removed once
    /// completion is wired in.
    #[allow(dead_code)]
    pub fn get(&self, uri: &Uri) -> Option<&str> {
        self.texts.get(uri).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn uri(text: &str) -> Uri {
        Uri::from_str(text).expect("uri")
    }

    #[test]
    fn set_get_and_replace() {
        let mut docs = Documents::new();
        let u = uri("file:///v/a.md");
        docs.set(u.clone(), "first".into());
        assert_eq!(docs.get(&u), Some("first"));
        docs.set(u.clone(), "second".into());
        assert_eq!(docs.get(&u), Some("second"));
    }

    #[test]
    fn remove_forgets_the_document() {
        let mut docs = Documents::new();
        let u = uri("file:///v/a.md");
        docs.set(u.clone(), "x".into());
        docs.remove(&u);
        assert_eq!(docs.get(&u), None);
    }

    #[test]
    fn unknown_document_is_none() {
        let docs = Documents::new();
        assert_eq!(docs.get(&uri("file:///v/missing.md")), None);
    }
}
