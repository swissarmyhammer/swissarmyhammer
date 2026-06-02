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

use std::future::Future;
use std::str::FromStr;
use std::sync::Arc;

use swissarmyhammer_command_service::TransactionSeam;
use swissarmyhammer_store::{StoreContext, StoreContextResolver, UndoEntryId};

tokio::task_local! {
    /// Per-task active board [`StoreContext`] for production dispatch.
    ///
    /// The kanban app is multi-board: each [`crate::substrate`]-wired board
    /// has its own [`StoreContext`], but the app installs a single
    /// app-wide `CommandService` on the plugin host. The Tauri
    /// `dispatch_command` handler resolves the target board from its
    /// `board_path` argument and scopes this task-local around the
    /// `execute` call (via [`scope_store_context`]); the task-local
    /// variant of [`StoreTransactionSeam`] then reads back the correct
    /// per-board context inside its `begin`/`end`.
    ///
    /// Outside a [`scope_store_context`] (e.g. in tests that drive the
    /// seam directly), [`StoreTransactionSeam::task_local`]'s `begin`
    /// returns `None` — the command falls back to a `txn: null`
    /// execution with no undo grouping, matching the noop seam's
    /// behavior.
    pub static CURRENT_STORE_CTX: Arc<StoreContext>;
}

/// Scope [`CURRENT_STORE_CTX`] to `ctx` for the duration of `fut`.
///
/// The production seam ([`StoreTransactionSeam::task_local`]) reads
/// [`CURRENT_STORE_CTX`] in its `begin`/`end`, so the `execute` callback
/// inherits the same per-board context the dispatcher set. Because tokio
/// task-locals are inherited on `.await` boundaries — and the command
/// service's `execute` handler never `tokio::spawn`s between begin and end
/// — every store write the callback makes on the same task sees the same
/// context.
pub async fn scope_store_context<F>(ctx: Arc<StoreContext>, fut: F) -> F::Output
where
    F: Future,
{
    CURRENT_STORE_CTX.scope(ctx, fut).await
}

/// Build the production [`StoreContextResolver`] that reads
/// [`CURRENT_STORE_CTX`].
///
/// Pair this with [`swissarmyhammer_store::StoreServer::with_resolver`]
/// at app bootstrap; the dispatcher then scopes the per-board context
/// around its call to the store tool handlers via
/// [`scope_store_context`]. Outside a scope the resolver returns `None`
/// and tool calls fail with a structured error — a dispatcher that
/// forgets to scope degrades gracefully rather than panicking.
pub fn task_local_store_resolver() -> StoreContextResolver {
    Arc::new(|| CURRENT_STORE_CTX.try_with(Arc::clone).ok())
}

/// [`TransactionSeam`] implementation backed by a kanban board's
/// [`StoreContext`].
///
/// Two constructors:
///
/// - [`Self::new`] holds an `Arc<StoreContext>` at construction — useful
///   for unit tests and single-board callers.
/// - [`Self::task_local`] resolves the context from the
///   [`CURRENT_STORE_CTX`] task-local — the production constructor for the
///   multi-board app, where the per-dispatch board is set by
///   [`scope_store_context`].
///
/// The ambient transaction slot itself is per-tokio-task (see
/// `StoreContext`'s `AmbientKey`), so the command service's `execute`
/// handler must invoke [`TransactionSeam::begin`] and
/// [`TransactionSeam::end`] on the same task it runs the callback on. The
/// service handler enforces this by never `tokio::spawn`-ing between
/// begin/end.
pub struct StoreTransactionSeam {
    /// Resolve the [`StoreContext`] to drive for the current `begin`/`end`
    /// pair. Returns `None` when no context is available (e.g.
    /// `task_local()` variant invoked outside a [`scope_store_context`]).
    resolver: Box<dyn Fn() -> Option<Arc<StoreContext>> + Send + Sync>,
}

impl StoreTransactionSeam {
    /// Build a seam that always drives `ctx` — the constructor unit tests
    /// and single-board callers use.
    pub fn new(ctx: Arc<StoreContext>) -> Self {
        let ctx = Arc::clone(&ctx);
        Self {
            resolver: Box::new(move || Some(Arc::clone(&ctx))),
        }
    }

    /// Build a seam that resolves the active context from the
    /// [`CURRENT_STORE_CTX`] task-local — the production constructor.
    ///
    /// Outside a [`scope_store_context`] the resolver returns `None`, and
    /// `begin` returns `None` to match — the command falls back to a
    /// `txn: null` execution with no undo group, exactly like the noop
    /// seam. So a dispatcher that forgets to scope the task-local degrades
    /// gracefully rather than panicking.
    pub fn task_local() -> Self {
        Self {
            resolver: Box::new(|| CURRENT_STORE_CTX.try_with(Arc::clone).ok()),
        }
    }
}

impl std::fmt::Debug for StoreTransactionSeam {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoreTransactionSeam").finish()
    }
}

impl TransactionSeam for StoreTransactionSeam {
    fn begin(&self, origin: &str) -> Option<String> {
        let ctx = (self.resolver)()?;
        let id = ctx.begin_transaction_with_origin(Some(origin.to_string()));
        Some(id.to_string())
    }

    fn end(&self, txn: &str) {
        let Some(ctx) = (self.resolver)() else {
            // No context resolves on this task (e.g. task-local seam
            // outside a `scope_store_context`). The matching `begin`
            // returned `None`, so there is nothing to close.
            return;
        };
        // `end_transaction` is stale-id-safe: it only clears the slot when
        // the id matches. An unparseable id should never happen in
        // production — `begin` returns a stringified `UndoEntryId` and the
        // command service round-trips the same string back — so the warn
        // path is a defensive net, not an expected branch.
        match UndoEntryId::from_str(txn) {
            Ok(id) => ctx.end_transaction(id),
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
            ctx.current_transaction()
                .map(|id| id.to_string())
                .as_deref(),
            Some(txn.as_str()),
            "an unparseable end must not clear the slot the matching begin set",
        );
        seam.end(&txn);
        assert!(ctx.current_transaction().is_none());
    }

    /// Outside a [`scope_store_context`] the task-local seam's `begin`
    /// returns `None` — the command falls back to a `txn: null` execution,
    /// matching the noop seam.
    #[tokio::test]
    async fn task_local_seam_returns_none_outside_scope() {
        let seam = StoreTransactionSeam::task_local();
        assert!(seam.begin("user").is_none(), "no task-local set");
        // `end` with anything is a safe no-op when the resolver returns None.
        seam.end("anything");
    }

    /// Inside a [`scope_store_context`] the task-local seam drives the
    /// scoped board's `StoreContext`: `begin` returns a real txn id, the
    /// ambient slot is set, and `end` clears it.
    #[tokio::test]
    async fn task_local_seam_drives_scoped_context() {
        let (_dir, ctx) = ctx();
        let seam = StoreTransactionSeam::task_local();
        let ctx_for_assert = Arc::clone(&ctx);
        scope_store_context(ctx, async move {
            let txn = seam.begin("user").expect("inside scope begin returns Some");
            assert_eq!(
                ctx_for_assert
                    .current_transaction()
                    .map(|id| id.to_string())
                    .as_deref(),
                Some(txn.as_str()),
            );
            seam.end(&txn);
            assert!(ctx_for_assert.current_transaction().is_none());
        })
        .await;
    }

    /// The production [`task_local_store_resolver`] returns `None` outside
    /// a [`scope_store_context`] — pairs with the seam's None-outside-scope
    /// contract so `StoreServer::with_resolver(task_local_store_resolver())`
    /// surfaces a structured error rather than panicking when the
    /// dispatcher forgets to scope.
    #[tokio::test]
    async fn task_local_store_resolver_returns_none_outside_scope() {
        let resolver = task_local_store_resolver();
        assert!(resolver().is_none(), "no CURRENT_STORE_CTX scoped");
    }

    /// Inside a [`scope_store_context`] the resolver returns the scoped
    /// `Arc<StoreContext>` (Arc identity preserved). Pins the round-trip
    /// the production wiring depends on:
    /// `StoreServer::with_resolver(task_local_store_resolver())` paired
    /// with `scope_store_context(board.store_ctx, …)` must see THIS
    /// board's context, not some other board's.
    #[tokio::test]
    async fn task_local_store_resolver_returns_scoped_context() {
        let (_dir, ctx) = ctx();
        let ctx_ptr = Arc::as_ptr(&ctx) as usize;
        let resolver = task_local_store_resolver();
        scope_store_context(ctx, async move {
            let resolved = resolver().expect("inside scope returns Some");
            assert_eq!(
                Arc::as_ptr(&resolved) as usize,
                ctx_ptr,
                "the resolver must return the same Arc instance scoped",
            );
        })
        .await;
    }

    /// Two concurrent scopes over distinct contexts get their own ambient
    /// slots — the seam routes each task to its own board. Pins the
    /// multi-board invariant the production dispatcher relies on.
    #[tokio::test]
    async fn task_local_seam_isolates_concurrent_scopes() {
        let (_dir_a, ctx_a) = ctx();
        let (_dir_b, ctx_b) = ctx();
        let seam = Arc::new(StoreTransactionSeam::task_local());

        let seam_a = Arc::clone(&seam);
        let ctx_a_clone = Arc::clone(&ctx_a);
        let a = tokio::spawn(scope_store_context(ctx_a_clone, async move {
            let txn = seam_a.begin("user").unwrap();
            // Hold the transaction briefly so the two tasks overlap.
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            seam_a.end(&txn);
            txn
        }));

        let seam_b = Arc::clone(&seam);
        let ctx_b_clone = Arc::clone(&ctx_b);
        let b = tokio::spawn(scope_store_context(ctx_b_clone, async move {
            let txn = seam_b.begin("user").unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            seam_b.end(&txn);
            txn
        }));

        let (ta, tb) = (a.await.unwrap(), b.await.unwrap());
        assert_ne!(ta, tb, "the two concurrent scopes must mint distinct txns");
        // Both ambient slots cleared.
        assert!(ctx_a.current_transaction().is_none());
        assert!(ctx_b.current_transaction().is_none());
    }
}
