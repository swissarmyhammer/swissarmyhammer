//! Paste-handler dispatch matrix.
//!
//! Paste is a polymorphic operation: what happens when the user pastes
//! depends on *both* what's on the clipboard and what kind of entity is
//! under focus. Pasting a `tag` onto a `task` adds the tag to that task;
//! pasting a `task` into a `column` creates a new task there; pasting a
//! `task` onto another `task` might insert a sibling — every combination
//! has its own semantics.
//!
//! Rather than a giant `match` on `(clipboard_type, target_type)` pairs,
//! each pairing lives in its own `paste_handlers/{clip}_onto_{target}.rs`
//! file implementing [`PasteHandler`]. [`PasteMatrix`] is the registry
//! that maps `(clipboard_type, target_type)` to a handler instance, and
//! [`crate::commands::clipboard_commands::PasteEntityCmd`] walks the
//! scope chain innermost-first looking for the first registered, available
//! handler.
//!
//! This module ships the dispatch plumbing only — the matrix is empty.
//! Each handler lands as its own follow-up card.

pub mod actor_onto_task;
pub mod attachment_onto_task;
pub mod column_into_board;
pub mod tag_onto_task;
pub mod task_into_board;
pub mod task_into_column;
pub mod task_into_project;

#[cfg(test)]
pub(crate) mod test_support;

use crate::clipboard::ClipboardPayload;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use swissarmyhammer_commands::{CommandContext, Result};

/// A paste handler implements the semantics of pasting one entity type
/// onto another.
///
/// Handlers are registered into a [`PasteMatrix`] keyed by
/// `(clipboard_entity_type, target_entity_type)`. When the user invokes
/// `entity.paste`, the dispatcher walks the focus scope chain
/// innermost-first; for each scope moniker it asks the matrix whether
/// the handler with key `(clipboard.entity_type, scope_type)` exists and
/// whether its [`available`] guard fires for the current context. The
/// first match wins.
///
/// Implementors should be small and focused: one file per pairing under
/// `paste_handlers/{clip}_onto_{target}.rs`. The hygiene test
/// [`tests::every_registered_handler_has_a_source_file`] enforces this.
///
/// [`available`]: PasteHandler::available
#[async_trait]
pub trait PasteHandler: Send + Sync {
    /// Returns the `(clipboard_entity_type, target_entity_type)` pair this
    /// handler dispatches on.
    ///
    /// Both names should match the entity-type strings used in monikers
    /// and `ClipboardPayload::entity_type` (e.g. `"task"`, `"tag"`,
    /// `"column"`).
    fn matches(&self) -> (&'static str, &'static str);

    /// Optional fine-grained availability gate, evaluated *after* the
    /// `(clipboard_type, target_type)` pair has matched.
    ///
    /// Use this to express constraints that depend on the clipboard
    /// payload contents or the runtime context (e.g. "paste tag onto
    /// task only if the task doesn't already have it"). Returning
    /// `false` causes the dispatcher to keep walking the scope chain.
    ///
    /// Defaults to `true` — the type pair alone is sufficient to claim
    /// the paste.
    fn available(
        &self,
        _clipboard: &ClipboardPayload,
        _target: &str,
        _ctx: &CommandContext,
    ) -> bool {
        true
    }

    /// Execute the paste against `target`, using the snapshot in
    /// `clipboard`.
    ///
    /// `target` is the moniker the dispatcher matched on (e.g.
    /// `"column:doing"`). Implementations parse the id portion as
    /// needed.
    ///
    /// Returns the JSON result of the underlying operation, surfaced
    /// back to the frontend as the command result.
    async fn execute(
        &self,
        clipboard: &ClipboardPayload,
        target: &str,
        ctx: &CommandContext,
    ) -> Result<Value>;
}

/// Registry of paste handlers, keyed by
/// `(clipboard_entity_type, target_entity_type)`.
///
/// Keys are stored as `(&'static str, &'static str)` because handlers
/// always return string literals from [`PasteHandler::matches`]. Lookups
/// use the runtime-borrowed clipboard/target type strings, so [`find`]
/// scans the (small) handler set linearly rather than constructing an
/// owned probe key — there are at most a handful of pairings and the
/// linear comparison is faster than allocating two `String`s per lookup.
///
/// At most one handler may be registered per key — duplicate registrations
/// panic, since the conflict would always be a programming error and
/// silent overwrites would make the active handler depend on registration
/// order.
///
/// [`find`]: PasteMatrix::find
#[derive(Default)]
pub struct PasteMatrix {
    handlers: HashMap<(&'static str, &'static str), Arc<dyn PasteHandler>>,
}

impl PasteMatrix {
    /// Register a handler under its `matches()` key.
    ///
    /// # Panics
    ///
    /// Panics if a handler is already registered for the same
    /// `(clipboard_type, target_type)` pair. Pairing collisions always
    /// indicate a registration bug; failing loudly during init is safer
    /// than silently shadowing one of the handlers.
    pub fn register<H: PasteHandler + 'static>(&mut self, handler: H) {
        let key = handler.matches();
        if self.handlers.contains_key(&key) {
            panic!(
                "duplicate PasteHandler registration for ({}, {}) — each \
                 (clipboard_type, target_type) pair may have at most one handler",
                key.0, key.1
            );
        }
        self.handlers.insert(key, Arc::new(handler));
    }

    /// Look up the handler for `(clipboard_type, target_type)`, if any.
    ///
    /// Returns `None` when no handler is registered for that pairing.
    /// Callers (typically the `PasteEntityCmd` dispatcher) then fall
    /// through to the next moniker in the scope chain.
    ///
    /// Implemented as a linear scan over the (small) handler set so the
    /// lookup can take borrowed `&str` arguments without forcing an
    /// allocation.
    pub fn find(&self, clipboard_type: &str, target_type: &str) -> Option<&Arc<dyn PasteHandler>> {
        self.handlers
            .iter()
            .find(|((c, t), _)| *c == clipboard_type && *t == target_type)
            .map(|(_, h)| h)
    }

    /// Iterate registered handler keys.
    ///
    /// Used by hygiene tests that assert each registered pairing has a
    /// dedicated source file. The iteration order is unspecified.
    pub fn keys(&self) -> impl Iterator<Item = (&'static str, &'static str)> + '_ {
        self.handlers.keys().copied()
    }

    /// Total number of registered handlers.
    pub fn len(&self) -> usize {
        self.handlers.len()
    }

    /// True when no handlers are registered.
    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty()
    }
}

/// Build the populated [`PasteMatrix`] for production use.
pub fn register_paste_handlers() -> PasteMatrix {
    let mut matrix = PasteMatrix::default();
    matrix.register(actor_onto_task::ActorOntoTaskHandler);
    matrix.register(attachment_onto_task::AttachmentOntoTaskHandler);
    matrix.register(column_into_board::ColumnIntoBoardHandler);
    matrix.register(tag_onto_task::TagOntoTaskHandler);
    matrix.register(task_into_board::TaskIntoBoardHandler);
    matrix.register(task_into_column::TaskIntoColumnHandler);
    matrix.register(task_into_project::TaskIntoProjectHandler);
    matrix
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Stub handler used by matrix tests below.
    struct StubHandler {
        clip: &'static str,
        target: &'static str,
    }

    #[async_trait]
    impl PasteHandler for StubHandler {
        fn matches(&self) -> (&'static str, &'static str) {
            (self.clip, self.target)
        }

        async fn execute(
            &self,
            _clipboard: &ClipboardPayload,
            _target: &str,
            _ctx: &CommandContext,
        ) -> Result<Value> {
            Ok(serde_json::json!({"stub": true}))
        }
    }

    #[test]
    fn empty_matrix_finds_nothing() {
        let m = PasteMatrix::default();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
        assert!(m.find("task", "column").is_none());
    }

    #[test]
    fn register_then_find() {
        let mut m = PasteMatrix::default();
        m.register(StubHandler {
            clip: "task",
            target: "column",
        });
        assert_eq!(m.len(), 1);
        assert!(m.find("task", "column").is_some());
        assert!(m.find("tag", "task").is_none());
        assert!(m.find("task", "task").is_none());
    }

    #[test]
    #[should_panic(expected = "duplicate PasteHandler registration")]
    fn duplicate_registration_panics() {
        let mut m = PasteMatrix::default();
        m.register(StubHandler {
            clip: "task",
            target: "column",
        });
        m.register(StubHandler {
            clip: "task",
            target: "column",
        });
    }

    #[test]
    fn keys_iterates_registered_pairs() {
        let mut m = PasteMatrix::default();
        m.register(StubHandler {
            clip: "task",
            target: "column",
        });
        m.register(StubHandler {
            clip: "tag",
            target: "task",
        });
        let mut keys: Vec<_> = m.keys().collect();
        keys.sort();
        assert_eq!(keys, vec![("tag", "task"), ("task", "column")]);
    }

    #[test]
    fn register_paste_handlers_registers_all_known_pairings() {
        let m = register_paste_handlers();
        let mut keys: Vec<_> = m.keys().collect();
        keys.sort();
        assert_eq!(
            keys,
            vec![
                ("actor", "task"),
                ("attachment", "task"),
                ("column", "board"),
                ("tag", "task"),
                ("task", "board"),
                ("task", "column"),
                ("task", "project"),
            ]
        );
    }

    /// Hygiene: every `(clip, target)` pair registered in the production
    /// matrix must have a colocated source file
    /// `paste_handlers/{clip}_onto_{target}.rs` OR
    /// `paste_handlers/{clip}_into_{target}.rs`. Both naming conventions
    /// are accepted: `onto` for associations (tag onto task), `into` for
    /// container moves (task into column).
    ///
    /// Catches drift between the registration list and the file layout —
    /// keeps "one handler per file" enforceable without ceremony.
    #[test]
    fn every_registered_handler_has_a_source_file() {
        let m = register_paste_handlers();
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("commands")
            .join("paste_handlers");

        for (clip, target) in m.keys() {
            let onto = dir.join(format!("{clip}_onto_{target}.rs"));
            let into = dir.join(format!("{clip}_into_{target}.rs"));
            assert!(
                onto.exists() || into.exists(),
                "handler ({clip}, {target}) is registered but no source file \
                 found at {} or {}",
                onto.display(),
                into.display()
            );
        }
    }
}
