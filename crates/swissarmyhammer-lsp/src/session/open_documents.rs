//! The open-document policy of an [`LspSession`](super::LspSession).
//!
//! This is the one place that decides which documents stay open on the
//! server. Two rules apply:
//!
//! - **The owner.** Each open document records who uses it: a tool op
//!   ([`DocumentOwner::Tool`]) or the indexing worker
//!   ([`DocumentOwner::Indexer`]). A tool use is sticky: when a tool op syncs a
//!   document, the document belongs to the tool ops from then on. The indexer
//!   can close only a document that it owns, so it never closes a document
//!   that a hover, definition or diagnostics op uses.
//! - **The limit.** The session keeps at most [`MAX_OPEN_DOCUMENTS`] documents
//!   open. Before a new document opens at the limit, the least recently used
//!   document closes. Each sync of a document marks it as used, so the document
//!   that a live op just synced is the last one to close.

use std::collections::HashMap;

use super::{DocState, DocumentOwner};

/// The most documents that one session keeps open on the server at a time.
///
/// A language server such as sourcekit-lsp does work for each open document
/// when a file changes, so an unbounded open set costs much CPU. The value is
/// large enough that the documents of the live tool ops stay open, and small
/// enough that the server does little work for documents nobody uses.
pub const MAX_OPEN_DOCUMENTS: usize = 50;

/// One document that the server has open.
#[derive(Debug, Clone, Copy)]
struct OpenDocument {
    /// What the server knows about the document.
    state: DocState,
    /// Who uses the document.
    owner: DocumentOwner,
    /// The tick of the last use. A higher tick is a more recent use.
    last_used: u64,
}

/// The set of documents that the server has open, with their owner and their
/// last use.
#[derive(Debug, Default)]
pub(super) struct OpenDocuments {
    /// `uri -> ` the open document.
    entries: HashMap<String, OpenDocument>,
    /// The tick that the next use gets. It only goes up.
    clock: u64,
}

impl OpenDocuments {
    /// The state of the open document `uri`, or `None` when it is not open.
    pub(super) fn state(&self, uri: &str) -> Option<DocState> {
        self.entries.get(uri).map(|document| document.state)
    }

    /// Tell if the document `uri` is open.
    pub(super) fn contains(&self, uri: &str) -> bool {
        self.entries.contains_key(uri)
    }

    /// The owner of the open document `uri`, or `None` when it is not open.
    pub(super) fn owner(&self, uri: &str) -> Option<DocumentOwner> {
        self.entries.get(uri).map(|document| document.owner)
    }

    /// Record a document that the server just opened, as the most recent use.
    pub(super) fn insert(&mut self, uri: String, state: DocState, owner: DocumentOwner) {
        let last_used = self.tick();
        self.entries.insert(
            uri,
            OpenDocument {
                state,
                owner,
                last_used,
            },
        );
    }

    /// Mark the open document `uri` as the most recent use by `owner`.
    ///
    /// A use by a tool op makes the tool ops the owner. A use by the indexer
    /// keeps the owner. A document that is not open is not changed.
    pub(super) fn touch(&mut self, uri: &str, owner: DocumentOwner) {
        let last_used = self.tick();
        if let Some(document) = self.entries.get_mut(uri) {
            document.last_used = last_used;
            if owner == DocumentOwner::Tool {
                document.owner = DocumentOwner::Tool;
            }
        }
    }

    /// Replace the state of the open document `uri`.
    pub(super) fn set_state(&mut self, uri: &str, state: DocState) {
        if let Some(document) = self.entries.get_mut(uri) {
            document.state = state;
        }
    }

    /// Forget the document `uri`.
    pub(super) fn remove(&mut self, uri: &str) {
        self.entries.remove(uri);
    }

    /// Forget every document.
    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }

    /// The document that must close before one more document can open.
    ///
    /// Returns the least recently used document when the set is at
    /// [`MAX_OPEN_DOCUMENTS`], and `None` when there is space.
    pub(super) fn eviction_candidate(&self) -> Option<String> {
        if self.entries.len() < MAX_OPEN_DOCUMENTS {
            return None;
        }
        self.entries
            .iter()
            .min_by_key(|(_, document)| document.last_used)
            .map(|(uri, _)| uri.clone())
    }

    /// A copy of the state of each open document, keyed by uri.
    pub(super) fn snapshot(&self) -> HashMap<String, DocState> {
        self.entries
            .iter()
            .map(|(uri, document)| (uri.clone(), document.state))
            .collect()
    }

    /// Give the tick for a new use and advance the clock.
    fn tick(&mut self) -> u64 {
        let tick = self.clock;
        self.clock += 1;
        tick
    }
}
