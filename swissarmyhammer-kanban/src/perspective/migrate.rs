//! Opt-in migration of legacy view-id-less perspectives.
//!
//! # Why save-time, not load-time
//!
//! The migration that pins a legacy `view_id: None` perspective to a single
//! matching view runs at **save time**, not load time. Two reasons:
//!
//! 1. **Non-destructive by design.** The task spec calls for an opt-in
//!    migration: a perspective YAML on disk must stay untouched until the
//!    user (or a command) actually re-saves it. Running the migration at
//!    load time would silently rewrite every legacy file on the next read,
//!    which the spec explicitly forbids.
//! 2. **Cleanest wiring.** `PerspectiveContext::open` (in the
//!    `swissarmyhammer-perspectives` crate) does not know about
//!    `ViewsContext`; only this crate's command layer
//!    (`AddPerspective`/`UpdatePerspective`/`RenamePerspective`) has access
//!    to both. Plugging the helper there means every save path benefits
//!    without leaking a `ViewsContext` dependency into the
//!    perspectives crate.
//!
//! A separate **load-time** helper, [`log_legacy_perspectives_once`], emits
//! a single `info!` (ambiguous) or `warn!` (orphaned) log line per legacy
//! perspective so users discover *why* a given YAML did not migrate.

use std::collections::HashSet;
use std::sync::Mutex;

use swissarmyhammer_perspectives::Perspective;
use swissarmyhammer_views::ViewsContext;

/// Try to auto-pin a legacy `view_id: None` perspective to a single matching
/// view at save time.
///
/// # Behavior
///
/// - If `perspective.view_id` is already `Some`, this is a no-op and returns
///   `false`.
/// - Otherwise, counts views whose [`kind`](swissarmyhammer_views::ViewKind)
///   serializes to a string equal to `perspective.view`. If **exactly one**
///   matches, sets `perspective.view_id = Some(matched_view.id)` and returns
///   `true`. Zero or multiple matches leave the perspective untouched.
///
/// # When to call
///
/// Call this immediately before persisting a perspective via
/// [`PerspectiveContext::write`](swissarmyhammer_perspectives::PerspectiveContext::write).
/// The matching view set is the live workspace view registry, so the result
/// reflects the user's *current* configuration, not the one in force when
/// the perspective was first written.
///
/// # Returns
///
/// `true` when the helper assigned a `view_id`; `false` otherwise (already
/// pinned, ambiguous, or orphaned).
pub fn maybe_pin_view_id_on_save(perspective: &mut Perspective, views: &ViewsContext) -> bool {
    if perspective.view_id.is_some() {
        return false;
    }

    let matches = matching_views_by_kind(views, &perspective.view);

    if let [single] = matches.as_slice() {
        perspective.view_id = Some(single.to_string());
        true
    } else {
        false
    }
}

/// Collect view ids whose kind serializes to the given perspective `view`
/// string (e.g. `"board"`, `"grid"`).
///
/// `ViewKind` serializes via serde as kebab-case so this comparison matches
/// the same string-typed `view` field stored on `Perspective`.
fn matching_views_by_kind<'a>(views: &'a ViewsContext, view_kind: &str) -> Vec<&'a str> {
    views
        .all_views()
        .iter()
        .filter(|v| {
            serde_json::to_value(&v.kind)
                .ok()
                .and_then(|val| val.as_str().map(str::to_string))
                .as_deref()
                == Some(view_kind)
        })
        .map(|v| v.id.as_str())
        .collect()
}

/// Process-wide guard set of perspective ids already logged.
///
/// Tracks `(perspective_id)` keys so a long-running session — which can
/// re-enter `gather_perspectives` thousands of times — emits exactly one
/// `info!` / `warn!` per legacy perspective regardless of how many
/// view-switching events fire.
static LOGGED_LEGACY_PERSPECTIVES: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// Reset the once-per-process log guard. Test-only — production code never
/// calls this. Lets `#[traced_test]` cases re-enter the helper between tests.
///
/// Gated behind `#[cfg(any(test, feature = "test-support"))]` so unit tests
/// in this crate AND integration tests / downstream crates that enable
/// `test-support` can both reach it; production binaries omit it entirely.
#[cfg(any(test, feature = "test-support"))]
pub fn reset_legacy_log_guard_for_test() {
    if let Ok(mut guard) = LOGGED_LEGACY_PERSPECTIVES.lock() {
        *guard = None;
    }
}

/// Emit a one-time `info!` / `warn!` line per legacy view-id-less
/// perspective so the user understands why it did not migrate.
///
/// # Logging rules
///
/// For each perspective with `view_id: None`:
///
/// - **Ambiguous** (`>= 2` views of the same kind): one
///   `info!("perspective <id> remains shared across all <kind> views — open
///   it in a specific view and save to pin it")`.
/// - **Orphan** (`0` views of that kind): one `warn!("perspective <id> has
///   view kind <kind> but no matching view registered")`.
/// - **Unambiguous** (`1` view of that kind): no log — the next save will
///   pin it via [`maybe_pin_view_id_on_save`], so we stay silent.
/// - Already pinned (`view_id: Some(_)`): never logs.
///
/// A process-wide guard suppresses repeat emissions: invoking this twice
/// for the same perspective id is a no-op on the second call. Use
/// [`reset_legacy_log_guard_for_test`] in tests that need a fresh guard.
pub fn log_legacy_perspectives_once(perspectives: &[Perspective], views: &ViewsContext) {
    let mut guard = match LOGGED_LEGACY_PERSPECTIVES.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    let logged = guard.get_or_insert_with(HashSet::new);

    for p in perspectives {
        if p.view_id.is_some() {
            continue;
        }
        if logged.contains(&p.id) {
            continue;
        }
        let matches = matching_views_by_kind(views, &p.view);
        match matches.len() {
            0 => {
                tracing::warn!(
                    perspective_id = %p.id,
                    view_kind = %p.view,
                    "perspective {} has view kind {} but no matching view registered",
                    p.id, p.view
                );
                logged.insert(p.id.clone());
            }
            1 => {
                // Unambiguous — `maybe_pin_view_id_on_save` will migrate it
                // on next write. Stay silent so we don't spam the user.
            }
            _ => {
                tracing::info!(
                    perspective_id = %p.id,
                    view_kind = %p.view,
                    "perspective {} remains shared across all {} views — open it in a specific view and save to pin it",
                    p.id, p.view
                );
                logged.insert(p.id.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_views::{ViewDef, ViewKind};

    /// Build a minimal `ViewsContext` carrying the supplied `ViewDef`s.
    ///
    /// `ViewsContext` is normally built by its loader from disk; for unit
    /// tests we fabricate it via a tempdir + `write_view` so the in-memory
    /// state mirrors what production would produce.
    async fn views_with(defs: Vec<ViewDef>) -> (tempfile::TempDir, ViewsContext) {
        let temp = tempfile::TempDir::new().unwrap();
        let dir = temp.path().to_path_buf();
        let mut views = ViewsContext::open(&dir).build().await.unwrap();
        for def in defs {
            views.write_view(&def).await.unwrap();
        }
        (temp, views)
    }

    fn grid_view(id: &str) -> ViewDef {
        ViewDef {
            id: id.to_string(),
            name: id.to_string(),
            icon: None,
            kind: ViewKind::Grid,
            entity_type: Some("task".into()),
            card_fields: Vec::new(),
            commands: Vec::new(),
        }
    }

    fn board_view(id: &str) -> ViewDef {
        ViewDef {
            id: id.to_string(),
            name: id.to_string(),
            icon: None,
            kind: ViewKind::Board,
            entity_type: Some("task".into()),
            card_fields: Vec::new(),
            commands: Vec::new(),
        }
    }

    #[tokio::test]
    async fn maybe_pin_pins_view_id_when_exactly_one_matches() {
        let (_t, views) = views_with(vec![board_view("view-board-1")]).await;
        let mut p = Perspective::new("p1", "Default", "board");
        let pinned = maybe_pin_view_id_on_save(&mut p, &views);
        assert!(
            pinned,
            "should pin when exactly one view of the kind exists"
        );
        assert_eq!(p.view_id.as_deref(), Some("view-board-1"));
    }

    #[tokio::test]
    async fn maybe_pin_skips_when_multiple_kind_matches() {
        let (_t, views) = views_with(vec![grid_view("grid-a"), grid_view("grid-b")]).await;
        let mut p = Perspective::new("p1", "Default", "grid");
        let pinned = maybe_pin_view_id_on_save(&mut p, &views);
        assert!(!pinned, "ambiguous — must not auto-pin");
        assert!(p.view_id.is_none());
    }

    #[tokio::test]
    async fn maybe_pin_skips_when_no_kind_match() {
        let (_t, views) = views_with(vec![board_view("view-board-1")]).await;
        let mut p = Perspective::new("p1", "Default", "grid");
        let pinned = maybe_pin_view_id_on_save(&mut p, &views);
        assert!(!pinned, "no matching view — must not auto-pin");
        assert!(p.view_id.is_none());
    }

    #[tokio::test]
    async fn maybe_pin_noop_when_already_pinned() {
        let (_t, views) = views_with(vec![board_view("view-board-1")]).await;
        let mut p = Perspective::new("p1", "Default", "board");
        p.view_id = Some("explicit-id".into());
        let pinned = maybe_pin_view_id_on_save(&mut p, &views);
        assert!(!pinned, "already-pinned perspective is a no-op");
        assert_eq!(
            p.view_id.as_deref(),
            Some("explicit-id"),
            "must not overwrite an explicit view_id"
        );
    }
}
