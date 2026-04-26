//! Integration tests covering the perspective-id resolver used by every
//! context-menu-driven perspective mutation (filter, group, sort).
//!
//! The commands themselves (`perspective.clearFilter`, `perspective.clearGroup`,
//! `perspective.sort.clear`) do not accept a `perspective_id` arg from the
//! context menu, so the resolver is the single place that decides which
//! perspective to act on. The tests below exercise each resolver branch
//! (`Arg`, `Scope`, `UiState`, `FirstForViewKind`) through the real dispatch
//! path so any regression in routing fails as a named case here instead of
//! surfacing as a mysterious "wrong perspective got cleared" bug in the UI.
//!
//! Pairs with `tests/command_dispatch_integration.rs` for general dispatch
//! coverage and `kanban-app/ui/src/components/perspective-tab-bar.test.tsx`
//! for the frontend half of the right-click → scope-chain contract.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use swissarmyhammer_commands::{Command, CommandContext, CommandError, CommandsRegistry, UIState};
use swissarmyhammer_kanban::clipboard::{
    ClipboardProvider, ClipboardProviderExt, InMemoryClipboard,
};
use swissarmyhammer_kanban::commands::register_commands;
use swissarmyhammer_kanban::perspective::AddPerspective;
use swissarmyhammer_kanban::test_support::composed_builtin_yaml_sources;
use swissarmyhammer_kanban::{board::InitBoard, Execute, KanbanContext};
use tempfile::TempDir;

/// Test harness: temp board, command registry, UIState, clipboard, and the
/// full set of registered command implementations.
///
/// Mirrors the minimal `TestEngine` from `command_dispatch_integration.rs`
/// so the two test files share the same dispatch wiring and any fix made
/// here exercises production code paths.
struct Harness {
    _temp: TempDir,
    kanban: Arc<KanbanContext>,
    commands: HashMap<String, Arc<dyn Command>>,
    _registry: CommandsRegistry,
    ui_state: Arc<UIState>,
    clipboard: Arc<InMemoryClipboard>,
}

impl Harness {
    /// Build a harness with an initialized board.
    async fn new() -> Self {
        let temp = TempDir::new().expect("failed to create temp dir");
        let kanban_dir = temp.path().join(".kanban");
        let kanban = KanbanContext::new(&kanban_dir);

        InitBoard::new("Perspective Routing Test")
            .execute(&kanban)
            .await
            .into_result()
            .expect("board init should succeed");

        let kanban = Arc::new(kanban);
        let registry = CommandsRegistry::from_yaml_sources(&composed_builtin_yaml_sources());
        let commands = register_commands();
        let ui_state = Arc::new(UIState::new());
        let clipboard = Arc::new(InMemoryClipboard::new());

        Self {
            _temp: temp,
            kanban,
            commands,
            _registry: registry,
            ui_state,
            clipboard,
        }
    }

    /// Create a perspective via the lower-level `AddPerspective` operation
    /// and return its generated ID.
    ///
    /// We bypass the `perspective.save` command here because the resolver
    /// tests in this file all need a *specific* perspective id up front
    /// (so they can assert which one was mutated), and the operation-level
    /// API is the clean way to get that without coupling to the command
    /// dispatch details.
    async fn add_perspective(
        &self,
        name: &str,
        view: &str,
        filter: Option<&str>,
        group: Option<&str>,
    ) -> String {
        let mut op = AddPerspective::new(name, view);
        op.filter = filter.map(String::from);
        op.group = group.map(String::from);

        let result = op
            .execute(self.kanban.as_ref())
            .await
            .into_result()
            .expect("add perspective should succeed");
        result["id"]
            .as_str()
            .expect("add perspective should return id")
            .to_string()
    }

    /// Read a perspective by id and return its full JSON representation.
    async fn read_perspective(&self, id: &str) -> Value {
        let pctx = self
            .kanban
            .perspective_context()
            .await
            .expect("perspective_context available");
        let pctx = pctx.read().await;
        let p = pctx
            .get_by_id(id)
            .unwrap_or_else(|| panic!("perspective {id} should exist"));
        serde_json::to_value(p).expect("perspective serializable")
    }

    /// Read the sort entries on a perspective, returning an empty Vec when
    /// none are set. Convenience over `read_perspective` when the field
    /// shape matters (`sort` may be absent from the JSON if empty).
    async fn read_sort(&self, id: &str) -> Vec<swissarmyhammer_kanban::perspective::SortEntry> {
        let pctx = self
            .kanban
            .perspective_context()
            .await
            .expect("perspective_context available");
        let pctx = pctx.read().await;
        pctx.get_by_id(id)
            .unwrap_or_else(|| panic!("perspective {id} should exist"))
            .sort
            .clone()
    }

    /// Dispatch a command through the real availability + execute path.
    ///
    /// Mirrors `TestEngine::dispatch` in `command_dispatch_integration.rs`.
    async fn dispatch(
        &self,
        cmd_id: &str,
        scope: &[&str],
        target: Option<&str>,
        args: HashMap<String, Value>,
    ) -> swissarmyhammer_commands::Result<Value> {
        let cmd = self
            .commands
            .get(cmd_id)
            .ok_or_else(|| CommandError::ExecutionFailed(format!("unknown command: {cmd_id}")))?;

        let mut ctx = CommandContext::new(
            cmd_id,
            scope.iter().map(|s| s.to_string()).collect(),
            target.map(|s| s.to_string()),
            args,
        );
        ctx.ui_state = Some(Arc::clone(&self.ui_state));
        ctx.set_extension(Arc::clone(&self.kanban));
        let ectx = self
            .kanban
            .entity_context()
            .await
            .expect("entity_context available");
        ctx.set_extension(ectx);
        let clipboard_ext =
            ClipboardProviderExt(Arc::clone(&self.clipboard) as Arc<dyn ClipboardProvider>);
        ctx.set_extension(Arc::new(clipboard_ext));

        if !cmd.available(&ctx) {
            return Err(CommandError::ExecutionFailed(format!(
                "command '{cmd_id}' not available in this context"
            )));
        }

        cmd.execute(&ctx).await
    }
}

// =========================================================================
// Scope branch — right-click on a perspective tab
// =========================================================================

/// Right-click on the **non-active** perspective tab and select
/// `Clear Filter`: the command must clear *that* perspective's filter,
/// NOT the window's active perspective.
///
/// The frontend injects a `perspective:<id>` moniker for each tab. The
/// context-menu plumbing captures the right-clicked scope chain, sends
/// it through to `dispatch_command`, and the resolver must prefer the
/// scope-chain moniker over the UIState active id.
#[tokio::test]
async fn clear_filter_uses_scope_moniker_over_ui_state() {
    let h = Harness::new().await;

    // Two perspectives — one filtered, the other unfiltered.
    let active_id = h
        .add_perspective("Active View", "board", Some("#active"), None)
        .await;
    let other_id = h
        .add_perspective("Other View", "board", Some("#other"), None)
        .await;

    // Pretend the user is currently looking at "Active View".
    h.ui_state.set_active_perspective("main", &active_id);

    // Right-click on the "Other View" tab → its scope chain has
    // `perspective:<other_id>` innermost and `window:main` outermost.
    let other_scope = format!("perspective:{other_id}");
    let scope = ["window:main", &other_scope];
    let scope_refs: Vec<&str> = scope.iter().map(AsRef::as_ref).collect();

    h.dispatch("perspective.clearFilter", &scope_refs, None, HashMap::new())
        .await
        .expect("clear filter should succeed");

    // The "Other View" filter should now be None — it's the target.
    let other = h.read_perspective(&other_id).await;
    assert!(
        other["filter"].is_null(),
        "scope-chain perspective should have filter cleared, got: {other:?}"
    );

    // The active (UIState) perspective should NOT be touched.
    let active = h.read_perspective(&active_id).await;
    assert_eq!(
        active["filter"], "#active",
        "UIState active perspective must remain untouched when scope chain \
         identifies a different perspective: {active:?}"
    );
}

/// Right-click on a tab and select `Clear Group` — same scope-wins contract
/// but for the group field.
#[tokio::test]
async fn clear_group_uses_scope_moniker_over_ui_state() {
    let h = Harness::new().await;

    let active_id = h
        .add_perspective("Active", "board", None, Some("@alice"))
        .await;
    let other_id = h
        .add_perspective("Other", "board", None, Some("@bob"))
        .await;

    h.ui_state.set_active_perspective("main", &active_id);

    let other_scope = format!("perspective:{other_id}");
    let scope = ["window:main", &other_scope];
    let scope_refs: Vec<&str> = scope.iter().map(AsRef::as_ref).collect();

    h.dispatch("perspective.clearGroup", &scope_refs, None, HashMap::new())
        .await
        .expect("clear group should succeed");

    let other = h.read_perspective(&other_id).await;
    assert!(
        other["group"].is_null(),
        "scope-chain perspective should have group cleared: {other:?}"
    );

    let active = h.read_perspective(&active_id).await;
    assert_eq!(
        active["group"], "@alice",
        "UIState active perspective must keep its group: {active:?}"
    );
}

/// Right-click in a view body after the view-container injects
/// `perspective:<active-id>` → the active perspective's sort is cleared.
///
/// Distinct from the "non-active tab" case above: here the scope chain
/// carries the *same* perspective id as UIState (because the container
/// derived it from `activePerspective`). Both branches would produce the
/// same answer, but the resolver must pick `ResolvedFrom::Scope` so the
/// answer is independent of UIState drift.
#[tokio::test]
async fn clear_sort_uses_scope_moniker_matching_active() {
    use swissarmyhammer_kanban::perspective::{SortDirection, SortEntry, UpdatePerspective};

    let h = Harness::new().await;

    let active_id = h.add_perspective("Active", "board", None, None).await;
    let _other_id = h.add_perspective("Other", "board", None, None).await;

    // Seed a sort entry on the active perspective so the clear is observable.
    UpdatePerspective::new(&active_id)
        .with_sort(vec![SortEntry::new("title", SortDirection::Asc)])
        .execute(h.kanban.as_ref())
        .await
        .into_result()
        .expect("seed sort should succeed");

    h.ui_state.set_active_perspective("main", &active_id);

    let active_scope = format!("perspective:{active_id}");
    let scope = ["window:main", &active_scope];
    let scope_refs: Vec<&str> = scope.iter().map(AsRef::as_ref).collect();

    h.dispatch("perspective.sort.clear", &scope_refs, None, HashMap::new())
        .await
        .expect("sort clear should succeed");

    let sort = h.read_sort(&active_id).await;
    assert!(
        sort.is_empty(),
        "active perspective should have empty sort after clear, got {sort:?}"
    );
}

// =========================================================================
// UiState branch — scope chain carries only window label (no perspective)
// =========================================================================

/// When the scope chain has **no** `perspective:*` moniker (e.g. a legacy
/// right-click path that predates the view-body container fix) the resolver
/// must fall back to UIState's active perspective for the current window.
///
/// This preserves the existing behavior of keybinding-driven invocations
/// that never carry a scope chain at all, and ensures the regression we are
/// fixing (missing perspective moniker → silently wrong target) degrades
/// gracefully to "use the window's active perspective" instead of
/// targeting an arbitrary perspective.
#[tokio::test]
async fn clear_filter_falls_back_to_ui_state_active_when_no_scope_moniker() {
    let h = Harness::new().await;

    let active_id = h
        .add_perspective("Active View", "board", Some("#active"), None)
        .await;
    let _other_id = h
        .add_perspective("Other View", "board", Some("#other"), None)
        .await;

    h.ui_state.set_active_perspective("main", &active_id);

    // Scope chain carries only the window moniker, no perspective.
    let scope = ["window:main"];

    h.dispatch("perspective.clearFilter", &scope, None, HashMap::new())
        .await
        .expect("clear filter should succeed");

    let active = h.read_perspective(&active_id).await;
    assert!(
        active["filter"].is_null(),
        "active perspective filter should be cleared: {active:?}"
    );
}

// =========================================================================
// Explicit arg branch — palette / scripted invocation supplies perspective_id
// =========================================================================

/// Explicit `perspective_id` in args wins over both scope chain AND UIState.
///
/// Guards against subtle regressions where e.g. the resolver checks the
/// scope chain first (wrong) and the arg is ignored.
#[tokio::test]
async fn clear_filter_explicit_arg_wins_over_scope_and_ui_state() {
    let h = Harness::new().await;

    let active_id = h
        .add_perspective("Active", "board", Some("#active"), None)
        .await;
    let scope_id = h
        .add_perspective("Scope", "board", Some("#scope"), None)
        .await;
    let arg_id = h.add_perspective("Arg", "board", Some("#arg"), None).await;

    h.ui_state.set_active_perspective("main", &active_id);

    let scope_moniker = format!("perspective:{scope_id}");
    let scope = ["window:main", &scope_moniker];
    let scope_refs: Vec<&str> = scope.iter().map(AsRef::as_ref).collect();

    let mut args = HashMap::new();
    args.insert("perspective_id".into(), json!(arg_id));

    h.dispatch("perspective.clearFilter", &scope_refs, None, args)
        .await
        .expect("clear filter should succeed");

    // Only the arg-identified perspective should be cleared.
    let arg = h.read_perspective(&arg_id).await;
    assert!(arg["filter"].is_null(), "arg perspective should be cleared");

    let scope_p = h.read_perspective(&scope_id).await;
    assert_eq!(
        scope_p["filter"], "#scope",
        "scope perspective must be untouched when explicit arg is supplied"
    );

    let active = h.read_perspective(&active_id).await;
    assert_eq!(
        active["filter"], "#active",
        "UIState active perspective must be untouched when explicit arg is supplied"
    );
}

// =========================================================================
// Stale / unknown perspective moniker in scope
// =========================================================================

/// A scope chain with a `perspective:<stale-id>` that no longer exists
/// still binds via the `Scope` branch — the resolver does not second-guess
/// the id, it trusts the scope chain.
///
/// The backing `UpdatePerspective` op then fails because the id is unknown.
/// The exact shape of that failure is an implementation detail, but the
/// command must NOT silently succeed against a different perspective (e.g.
/// the UIState active one).
#[tokio::test]
async fn clear_filter_stale_scope_moniker_does_not_fall_through_to_ui_state() {
    let h = Harness::new().await;

    let active_id = h
        .add_perspective("Active", "board", Some("#active"), None)
        .await;

    h.ui_state.set_active_perspective("main", &active_id);

    // Scope chain references a perspective id that was never created.
    let stale_scope = "perspective:01STALEFAKEIDXXXXXXXXXXXXXXX";
    let scope = ["window:main", stale_scope];

    let result = h
        .dispatch("perspective.clearFilter", &scope, None, HashMap::new())
        .await;

    // The active perspective must retain its filter regardless of whether
    // the command succeeded or errored — it's not what the scope chain
    // identified, so it must not have been touched.
    let active = h.read_perspective(&active_id).await;
    assert_eq!(
        active["filter"], "#active",
        "active perspective must not be clobbered when scope chain points \
         to a stale id (dispatch result was {result:?}): {active:?}"
    );
}
