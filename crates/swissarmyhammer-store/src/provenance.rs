//! Cross-layer correlation + provenance for data-change events.
//!
//! [`EventProvenance`] carries the ambient transaction id (`txn`) and the
//! actor classification (`origin`) that a data-change event was produced
//! under. It lives in `swissarmyhammer-store` because every higher layer
//! (`-entity`, `-views`, `-perspectives`) depends on this crate and emits
//! events that must carry the same shape, and because the `txn` is sourced
//! from the store's own ambient transaction slot
//! ([`StoreContext::current_transaction`](crate::StoreContext::current_transaction)).
//!
//! `origin` is a free-form string by design — the wire contract is
//! `"user" | "agent:<id>" | "undo" | "redo" | "watcher"` — so a new actor
//! class can be introduced without a breaking enum change. The
//! constructors here cover the fixed in-layer cases (user / watcher / undo /
//! redo); `agent:<id>` is stamped by the wiring layer from the caller
//! identity.

use serde::{Deserialize, Serialize};

/// The provenance of a single data-change event: the transaction it belongs
/// to and who caused it.
///
/// A `None` `txn` means the change was made outside any transaction (a legacy
/// per-write mutation, or a watcher-sourced external edit that carries no
/// in-process transaction). `origin` classifies the actor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventProvenance {
    /// The transaction / undo-group id this change belongs to, when one is
    /// active. `None` for changes made outside any transaction.
    pub txn: Option<String>,
    /// The actor classification: `"user"`, `"agent:<id>"`, `"undo"`,
    /// `"redo"`, or `"watcher"`.
    pub origin: String,
}

impl Default for EventProvenance {
    /// The default provenance for a directly user-initiated change with no
    /// active transaction: `origin: "user"`, `txn: None`.
    fn default() -> Self {
        Self::user()
    }
}

impl EventProvenance {
    /// Provenance for a directly user-initiated edit with no active
    /// transaction: `origin: "user"`, `txn: None`. This is what the plain
    /// `write`/`delete` paths emit.
    pub fn user() -> Self {
        Self {
            txn: None,
            origin: "user".to_string(),
        }
    }

    /// Provenance for a watcher-sourced refresh (an external process rewrote
    /// the file on disk): `origin: "watcher"`, `txn: None`.
    pub fn watcher() -> Self {
        Self {
            txn: None,
            origin: "watcher".to_string(),
        }
    }

    /// Provenance for an undo-sourced reconcile, optionally carrying the
    /// reversed command's transaction id: `origin: "undo"`.
    pub fn undo(txn: Option<impl Into<String>>) -> Self {
        Self {
            txn: txn.map(Into::into),
            origin: "undo".to_string(),
        }
    }

    /// Provenance for a redo-sourced reconcile, optionally carrying the
    /// reapplied command's transaction id: `origin: "redo"`.
    pub fn redo(txn: Option<impl Into<String>>) -> Self {
        Self {
            txn: txn.map(Into::into),
            origin: "redo".to_string(),
        }
    }

    /// Provenance with an explicit `txn` and `origin`.
    pub fn new(txn: Option<impl Into<String>>, origin: impl Into<String>) -> Self {
        Self {
            txn: txn.map(Into::into),
            origin: origin.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_is_untransacted() {
        let p = EventProvenance::user();
        assert_eq!(p.origin, "user");
        assert!(p.txn.is_none());
    }

    #[test]
    fn watcher_is_untransacted() {
        let p = EventProvenance::watcher();
        assert_eq!(p.origin, "watcher");
        assert!(p.txn.is_none());
    }

    #[test]
    fn undo_redo_carry_txn() {
        let u = EventProvenance::undo(Some("txn-1"));
        assert_eq!(u.origin, "undo");
        assert_eq!(u.txn.as_deref(), Some("txn-1"));

        let r = EventProvenance::redo(None::<String>);
        assert_eq!(r.origin, "redo");
        assert!(r.txn.is_none());
    }

    #[test]
    fn default_is_user() {
        assert_eq!(EventProvenance::default(), EventProvenance::user());
    }
}
