//! Store-backed [`TransactionSeam`] for the command service.
//!
//! Wraps a kanban board's [`StoreContext`] so the [`CommandService`]'s
//! `execute` handler brackets each command in a real store transaction:
//! [`StoreContext::begin_transaction_with_origin`] opens one ambient
//! transaction on the current tokio task; every entity/perspective/view write
//! the command makes on that task inherits the same `txn` and the same
//! `origin`, and the emitted `commands/executed` notification reads back that
//! `txn` too.
//!
//! Lives in this crate because it bridges `swissarmyhammer-command-service`
//! (the Tier-0 service core that cannot name the store) with
//! `swissarmyhammer-store` (the data layer the kanban app owns). The kanban
//! app constructs one of these per board and hands it to
//! [`install_commands_module_with`](swissarmyhammer_command_service::install_commands_module_with)
//! as the production transaction seam.
//!
//! [`CommandService`]: swissarmyhammer_command_service::CommandService

use std::str::FromStr;
use std::sync::Arc;

use swissarmyhammer_command_service::TransactionSeam;
use swissarmyhammer_store::{StoreContext, UndoEntryId};

/// [`TransactionSeam`] implementation backed by a kanban board's
/// [`StoreContext`].
///
/// The ambient transaction slot is per-tokio-task (see `StoreContext`'s
/// `AmbientKey`), so the command service's `execute` handler must invoke
/// [`TransactionSeam::begin`] and [`TransactionSeam::end`] on the same task
/// it runs the callback on. The service handler enforces this by never
/// `tokio::spawn`-ing between begin/end.
pub struct StoreTransactionSeam {
    ctx: Arc<StoreContext>,
}

impl StoreTransactionSeam {
    /// Wrap `ctx` as a [`TransactionSeam`].
    pub fn new(ctx: Arc<StoreContext>) -> Self {
        Self { ctx }
    }
}

impl std::fmt::Debug for StoreTransactionSeam {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoreTransactionSeam").finish()
    }
}

impl TransactionSeam for StoreTransactionSeam {
    fn begin(&self, origin: &str) -> Option<String> {
        let id = self
            .ctx
            .begin_transaction_with_origin(Some(origin.to_string()));
        Some(id.to_string())
    }

    fn end(&self, txn: &str) {
        // `end_transaction` is stale-id-safe: it only clears the slot when
        // the id matches. An unparseable id should never happen in
        // production — `begin` returns a stringified `UndoEntryId` and the
        // command service round-trips the same string back — so the warn
        // path is a defensive net, not an expected branch.
        match UndoEntryId::from_str(txn) {
            Ok(id) => self.ctx.end_transaction(id),
            Err(e) => {
                tracing::warn!(
                    txn = %txn,
                    error = %e,
                    "StoreTransactionSeam.end called with unparseable txn id; \
                     dropping the close"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn ctx() -> (TempDir, Arc<StoreContext>) {
        let dir = TempDir::new().unwrap();
        let ctx = Arc::new(StoreContext::new(dir.path().to_path_buf()));
        (dir, ctx)
    }

    /// `begin` opens an ambient transaction whose id matches what
    /// `current_transaction()` reads back on the same task; `end` clears it.
    #[tokio::test]
    async fn begin_sets_ambient_slot_end_clears_it() {
        let (_dir, ctx) = ctx();
        let seam = StoreTransactionSeam::new(Arc::clone(&ctx));

        assert!(ctx.current_transaction().is_none(), "starts clean");
        let txn = seam.begin("user").expect("store-backed seam returns Some");

        let current = ctx
            .current_transaction()
            .expect("begin opened an ambient transaction")
            .to_string();
        assert_eq!(
            current, txn,
            "the seam's returned txn must match the ambient slot's id",
        );

        seam.end(&txn);
        assert!(
            ctx.current_transaction().is_none(),
            "end must clear the ambient slot",
        );
    }

    /// `begin` stamps the caller-derived `origin` into the ambient slot, so
    /// `current_provenance()` (what a forward entity write reads at emit
    /// time) returns the same `origin` the command's `commands/executed`
    /// will carry.
    #[tokio::test]
    async fn begin_stamps_origin_into_current_provenance() {
        let (_dir, ctx) = ctx();
        let seam = StoreTransactionSeam::new(Arc::clone(&ctx));

        let txn = seam.begin("agent:alice").expect("seam returns Some");
        let prov = ctx.current_provenance();
        assert_eq!(
            prov.txn.as_deref(),
            Some(txn.as_str()),
            "current_provenance must carry the same txn the seam opened",
        );
        assert_eq!(
            prov.origin, "agent:alice",
            "current_provenance must carry the caller-derived origin",
        );
        seam.end(&txn);
    }

    /// A stale or unparseable `end` does not panic and does not clobber a
    /// txn opened by a different `begin`. The slot stays whatever the
    /// matching `begin` left it.
    #[tokio::test]
    async fn end_with_unparseable_id_is_a_safe_noop() {
        let (_dir, ctx) = ctx();
        let seam = StoreTransactionSeam::new(Arc::clone(&ctx));

        let txn = seam.begin("user").expect("seam returns Some");
        seam.end("not-a-ulid");
        // The bogus end did NOT clear the slot — the real `txn` is still
        // active until the matching end runs.
        assert_eq!(
            ctx.current_transaction().map(|id| id.to_string()).as_deref(),
            Some(txn.as_str()),
            "an unparseable end must not clear the slot the matching begin set",
        );
        seam.end(&txn);
        assert!(ctx.current_transaction().is_none());
    }
}
