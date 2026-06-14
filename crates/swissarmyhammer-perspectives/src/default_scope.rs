//! Default-perspective scope invariants shared by every creator.
//!
//! The "Default" perspective is ensured by several independent writers — the
//! kanban board-open reconciliation (`swissarmyhammer-kanban`'s
//! `perspective::ensure_default`), the kanban `add perspective` op's ensure
//! mode, and the `views` MCP server's `save perspective` op (the production
//! path behind the `perspective-commands` plugin). They must all agree on:
//!
//! - what the **scope** of a perspective is (pinned view id, else view kind);
//! - the **deterministic id** an ensure-created default lands under, so
//!   concurrent windows and stale caches converge on ONE file instead of
//!   accumulating duplicates;
//! - the **matching rule** that decides whether an existing perspective
//!   already serves a scope;
//! - which scope components are **safe to embed in a filename**;
//! - what counts as a **customized** (user-authored, never auto-deleted)
//!   perspective.
//!
//! Keeping the definitions here — the crate both consumers already depend on
//! — makes drift between the writers impossible (live regression
//! 01KTY6T1GPY94VYWANE9X41SKJ: the ensure semantics were fixed on one writer
//! while production dispatched through another).

use crate::types::Perspective;

/// Name of the auto-created default perspective.
pub const DEFAULT_PERSPECTIVE_NAME: &str = "Default";

/// Maximum length of a scope component embedded in the deterministic
/// `default-<scope>.yaml` filename. Real view ids are 26-char ULIDs;
/// anything past this bound risks exceeding filesystem filename limits.
const MAX_SCOPE_COMPONENT_LEN: usize = 128;

/// Deterministic id for the default perspective of a view scope.
///
/// `scope` is the perspective's view-instance id when pinned, else its view
/// kind (e.g. `"board"`). Deriving the id from the scope makes ensure-style
/// creation idempotent at the filesystem level: every creator converges on
/// the same `default-<scope>.yaml` file.
pub fn default_perspective_id(scope: &str) -> String {
    format!("default-{scope}")
}

/// The scope a perspective belongs to: its pinned view-instance id when
/// set, else its view kind (legacy shared-by-kind).
pub fn perspective_scope(p: &Perspective) -> &str {
    p.view_id.as_deref().unwrap_or(&p.view)
}

/// Whether a caller-supplied scope component is safe to embed in the
/// deterministic `default-<scope>.yaml` filename.
///
/// Rejects empty strings, leading `.`, path separators (`/`, `\`),
/// parent-dir references (`..`), and bounds the length (see
/// [`MAX_SCOPE_COMPONENT_LEN`]). A `view_id` failing this check must never
/// reach [`default_perspective_id`] — callers fall back to the view-kind
/// scope instead.
pub fn is_safe_scope_component(scope: &str) -> bool {
    !scope.is_empty()
        && scope.len() <= MAX_SCOPE_COMPONENT_LEN
        && !scope.starts_with('.')
        && !scope.contains('/')
        && !scope.contains('\\')
        && !scope.contains("..")
}

/// The ensure / `if_absent` matching rule: when both sides carry a
/// `view_id` they must match exactly; otherwise fall back to a view-kind
/// match. Mirrors the frontend perspective filter.
pub fn matches_scope(p: &Perspective, view_id: Option<&str>, view_kind: &str) -> bool {
    match (view_id, p.view_id.as_deref()) {
        (Some(vid), Some(pvid)) => vid == pvid,
        _ => p.view == view_kind,
    }
}

/// Whether the user put anything into this perspective beyond the
/// auto-created shell. Customized perspectives are never deleted by
/// reconciliation.
pub fn is_customized(p: &Perspective) -> bool {
    p.filter.is_some() || p.group.is_some() || !p.sort.is_empty() || !p.fields.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pinned(view: &str, view_id: Option<&str>) -> Perspective {
        let mut p = Perspective::new("01AAA", DEFAULT_PERSPECTIVE_NAME, view);
        p.view_id = view_id.map(String::from);
        p
    }

    #[test]
    fn matches_scope_follows_view_id_first_kind_fallback() {
        let p = pinned("board", Some("view-1"));
        assert!(matches_scope(&p, Some("view-1"), "board"));
        assert!(!matches_scope(&p, Some("view-2"), "board"));
        // No incoming view_id: kind fallback applies even to pinned ones.
        assert!(matches_scope(&p, None, "board"));

        let legacy = pinned("board", None);
        assert!(matches_scope(&legacy, Some("view-1"), "board"));
        assert!(!matches_scope(&legacy, Some("view-1"), "grid"));
    }

    #[test]
    fn safe_scope_component_rejects_path_escapes_and_overlong_ids() {
        assert!(is_safe_scope_component("01JMVIEW0000000000BOARD0"));
        assert!(is_safe_scope_component("board"));
        assert!(!is_safe_scope_component(""));
        assert!(!is_safe_scope_component(".hidden"));
        assert!(!is_safe_scope_component("a/b"));
        assert!(!is_safe_scope_component("a\\b"));
        assert!(!is_safe_scope_component("../escape"));
        assert!(!is_safe_scope_component(&"x".repeat(129)));
    }
}
