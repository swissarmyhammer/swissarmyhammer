//! Shared tag resolution and application for task operations.
//!
//! A task's tags are not a stored field — they are the `#tag` markers inside
//! its body, parsed back out by [`tag_parser::parse_tags`]. Four operations
//! mutate that set: `tag task`, `untag task`, and the `tags` parameter on
//! `add task` / `update task`. All four route through [`apply_tag_refs`], so a
//! tag reference means exactly the same thing on every one of them and the
//! plural form can never drift from the singular one.
//!
//! Creating the `Tag` entity for a name that does not exist yet is not done
//! here: every caller already runs `shared::auto_create_body_tags` after its
//! write, which mints a `Tag` with an auto-color for each `#tag` in the body.

use crate::error::{KanbanError, Result};
use crate::tag_parser;
use crate::types::{short_id, SHORT_ID_LEN};
use serde_json::json;
use std::collections::HashSet;
use swissarmyhammer_entity::{Entity, EntityContext};

/// How the resolved tags combine with the tags already on the task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TagApply {
    /// Add the tags, keeping any already present (`tag task`, `add task`).
    Append,
    /// Make the task's tag set exactly the given tags (`update task`).
    Replace,
    /// Remove the tags, keeping the rest (`untag task`).
    Remove,
}

/// Resolve `refs` and rewrite `entity`'s body so its `#tag` markers match
/// `mode`.
///
/// Every ref is resolved through [`resolve_tag_ref`] **before** any body edit,
/// so a single unresolvable ref leaves the entity untouched rather than
/// applying a partial set.
///
/// Returns whether the body changed, letting callers skip a no-op write. The
/// caller owns the write and the follow-up
/// `shared::auto_create_body_tags` pass that mints any new `Tag` entities.
pub(crate) async fn apply_tag_refs(
    ectx: &EntityContext,
    entity: &mut Entity,
    refs: &[String],
    mode: TagApply,
) -> Result<bool> {
    // Append/Remove of nothing cannot change the body, so skip the tag scan on
    // the common `add task` path that carries no explicit tags. Replace of
    // nothing still clears, so it must go through.
    if refs.is_empty() && mode != TagApply::Replace {
        return Ok(false);
    }

    let tags = ectx.list("tag").await?;
    let mut slugs = refs
        .iter()
        .map(|raw| resolve_tag_ref(&tags, raw))
        .collect::<Result<Vec<String>>>()?;
    // Two refs can name one tag ("bug" and its ULID), and a caller may simply
    // repeat one. Applying a slug twice is a no-op, so collapse them and keep
    // the requested count honest for the Replace check below.
    let mut seen = HashSet::new();
    slugs.retain(|slug| seen.insert(slug.clone()));

    let body = entity.get_str("body").unwrap_or("").to_string();
    let new_body = rewrite_body(&body, &slugs, mode)?;
    if new_body == body {
        return Ok(false);
    }
    entity.set("body", json!(new_body));
    Ok(true)
}

/// Rewrite a body so its `#tag` markers match `slugs` under `mode`.
///
/// Pure, so the marker arithmetic is testable without a board.
///
/// The result is verified before it is returned: every applied slug must read
/// back out of the new body (and, for [`TagApply::Remove`], must not). A body
/// that defeats the writer — an unbalanced code fence swallows anything
/// appended after it — yields an error instead of a success that changed
/// nothing. Reporting `ok` on a write the parser cannot see is the silent input
/// loss this module exists to prevent.
fn rewrite_body(body: &str, slugs: &[String], mode: TagApply) -> Result<String> {
    let base = match mode {
        TagApply::Replace => strip_all_tags(body),
        TagApply::Append | TagApply::Remove => body.to_string(),
    };
    let new_body = slugs.iter().fold(base, |acc, slug| match mode {
        TagApply::Remove => tag_parser::remove_tag(&acc, slug),
        TagApply::Append | TagApply::Replace => tag_parser::append_tag(&acc, slug),
    });

    let applied = tag_parser::parse_tags(&new_body);
    let wanted_present = mode != TagApply::Remove;
    for slug in slugs {
        if applied.contains(slug) != wanted_present {
            let verb = if wanted_present { "apply" } else { "remove" };
            return Err(KanbanError::parse(format!(
                "could not {verb} tag {slug:?}: the description's code fences hide the \
                 `#{slug}` marker from the tag parser — close the fence or edit the \
                 description directly"
            )));
        }
    }
    if mode == TagApply::Replace && applied.len() != slugs.len() {
        return Err(KanbanError::parse(format!(
            "could not replace the tag set: {applied:?} survived instead of {slugs:?} — \
             edit the description directly"
        )));
    }
    Ok(new_body)
}

/// Remove every `#tag` marker the body currently carries.
///
/// The starting point for [`TagApply::Replace`]: strip what is there, then
/// append the requested set, so the result is exactly the requested tags.
/// Markers inside fenced blocks, inline code, and headings are left alone,
/// because [`tag_parser::parse_tags`] never counted them as tags in the first
/// place.
fn strip_all_tags(body: &str) -> String {
    tag_parser::parse_tags(body)
        .iter()
        .fold(body.to_string(), |acc, slug| {
            tag_parser::remove_tag(&acc, slug)
        })
}

/// Resolve one forgiving tag reference to its canonical slug.
///
/// Accepted forms, in priority order:
///
/// 1. **Explicit id ref** — a leading `^` sigil, or a well-formed 26-char
///    ULID. It must name an existing `Tag`, by entity id or (for the legacy
///    tags an earlier `tag task` created *named* after a ULID) by tag name.
///    Anything else is a [`KanbanError::TagNotFound`], never a silent no-op.
/// 2. **Existing tag name** — the ref normalizes (see
///    [`tag_parser::normalize_slug`]) to the name of a `Tag` that already
///    exists. A name always wins over a *short id* reading of the same
///    characters, so a real tag is never shadowed by a short id collision.
/// 3. **Bare short id** — the 7-char canonical short id of an existing `Tag`.
/// 4. **New tag name** — anything else is taken as a name and created on
///    demand, exactly as `tag task` does for an unknown name.
///
/// A ref with no slug characters at all (`""`, `"###"`) is a parse error: it
/// would otherwise write a bare `#` that reads back as no tag — the silent
/// input loss this resolver exists to prevent.
fn resolve_tag_ref(tags: &[Entity], raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    let (needle, explicit_id) = match trimmed.strip_prefix('^') {
        Some(rest) => (rest.trim(), true),
        None => (trimmed, false),
    };

    let slug = tag_parser::normalize_slug(needle);
    let named = |slug: &str| !slug.is_empty() && tags.iter().any(|tag| slug_of(tag) == slug);

    if explicit_id || is_full_ulid(needle) {
        if let Some(tag) = tag_by_id(tags, needle) {
            return Ok(slug_of(tag));
        }
        // A tag literally *named* after a ULID is the wreckage an earlier
        // `tag task` left behind when its id lookup failed. Matching it by name
        // is the only way to untag or replace those cards.
        if named(&slug) {
            return Ok(slug);
        }
        return Err(KanbanError::TagNotFound {
            id: raw.to_string(),
        });
    }

    if named(&slug) {
        return Ok(slug);
    }
    if let Some(tag) = tag_by_short_id(tags, needle) {
        return Ok(slug_of(tag));
    }
    if slug.is_empty() {
        return Err(KanbanError::parse(format!(
            "invalid tag ref: {raw:?} — a tag name needs at least one alphanumeric character"
        )));
    }
    Ok(slug)
}

/// Whether `needle` is a well-formed 26-char Crockford-base32 ULID.
///
/// Case-insensitive, matching how the board stores and compares ids.
fn is_full_ulid(needle: &str) -> bool {
    ulid::Ulid::from_string(needle).is_ok()
}

/// Find the tag whose entity id equals `needle`, or whose canonical short id
/// does when `needle` is short-id length. Both comparisons are
/// case-insensitive.
fn tag_by_id<'a>(tags: &'a [Entity], needle: &str) -> Option<&'a Entity> {
    let lowered = needle.to_lowercase();
    tags.iter()
        .find(|tag| tag.id.as_str().to_lowercase() == lowered)
        .or_else(|| tag_by_short_id(tags, needle))
}

/// Find the tag whose canonical short id equals `needle`.
///
/// Returns `None` unless `needle` is exactly [`SHORT_ID_LEN`] characters, so a
/// longer or shorter ref never matches by accident.
fn tag_by_short_id<'a>(tags: &'a [Entity], needle: &str) -> Option<&'a Entity> {
    if needle.len() != SHORT_ID_LEN {
        return None;
    }
    let lowered = needle.to_lowercase();
    tags.iter().find(|tag| short_id(tag.id.as_str()) == lowered)
}

/// The canonical slug for a tag entity: its stored `tag_name`, normalized so
/// the `#slug` written into a body reads back as the same slug.
fn slug_of(tag: &Entity) -> String {
    tag_parser::normalize_slug(tag.get_str("tag_name").unwrap_or(""))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a tag entity with the given id and stored name.
    fn tag(id: &str, name: &str) -> Entity {
        let mut entity = Entity::new("tag", id);
        entity.set("tag_name", json!(name));
        entity
    }

    const BUG_ID: &str = "01KJZEPKJ35S76KF7E9HS5742J";
    const INIT_ID: &str = "01KT7375T468PE35B87WY042DQ";

    fn board_tags() -> Vec<Entity> {
        vec![tag(BUG_ID, "bug"), tag(INIT_ID, "init")]
    }

    #[test]
    fn plain_name_resolves_to_its_slug() {
        let tags = board_tags();
        assert_eq!(resolve_tag_ref(&tags, "bug").unwrap(), "bug");
    }

    #[test]
    fn unknown_name_is_created_not_rejected() {
        // A name is a creation request — `tag task` has always worked this way.
        let tags = board_tags();
        assert_eq!(resolve_tag_ref(&tags, "brand-new").unwrap(), "brand-new");
    }

    #[test]
    fn name_is_normalized_to_a_slug_that_round_trips() {
        let tags = board_tags();
        assert_eq!(resolve_tag_ref(&tags, "Bug Fix").unwrap(), "Bug-Fix");
    }

    #[test]
    fn full_ulid_resolves_to_the_tag_name() {
        let tags = board_tags();
        assert_eq!(resolve_tag_ref(&tags, BUG_ID).unwrap(), "bug");
        assert_eq!(
            resolve_tag_ref(&tags, &INIT_ID.to_lowercase()).unwrap(),
            "init"
        );
    }

    #[test]
    fn short_id_and_caret_forms_resolve() {
        let tags = board_tags();
        let short = short_id(BUG_ID);
        assert_eq!(resolve_tag_ref(&tags, &short).unwrap(), "bug");
        assert_eq!(resolve_tag_ref(&tags, &format!("^{short}")).unwrap(), "bug");
        assert_eq!(
            resolve_tag_ref(&tags, &format!("^{BUG_ID}")).unwrap(),
            "bug"
        );
    }

    /// The silent-drop case from the bug report: a ULID that names no tag.
    /// It must error rather than become a tag literally called `01KJZ…`.
    #[test]
    fn unknown_full_ulid_is_an_error() {
        let tags = vec![tag(INIT_ID, "init")];
        let err = resolve_tag_ref(&tags, BUG_ID).unwrap_err();
        assert!(
            matches!(err, KanbanError::TagNotFound { .. }),
            "expected TagNotFound, got: {err}"
        );
    }

    #[test]
    fn caret_ref_to_unknown_tag_is_an_error() {
        let tags = board_tags();
        assert!(resolve_tag_ref(&tags, "^nosuch7").is_err());
    }

    /// An existing name wins over the short-id reading of the same characters,
    /// so no tag can be shadowed by an id collision.
    #[test]
    fn existing_name_wins_over_short_id() {
        let collide = "01KT7375T468PE35B87WYCLEANUP";
        let tags = vec![tag(collide, "not-me"), tag(BUG_ID, "cleanup")];
        assert_eq!(resolve_tag_ref(&tags, "cleanup").unwrap(), "cleanup");
    }

    #[test]
    fn ref_without_slug_characters_is_a_parse_error() {
        let tags = board_tags();
        for raw in ["", "   ", "###", "🎉"] {
            let err = resolve_tag_ref(&tags, raw).unwrap_err();
            assert!(
                matches!(err, KanbanError::Parse { .. }),
                "{raw:?} should be a parse error, got: {err}"
            );
        }
    }

    /// A tag ULID that names no tag entity but IS an existing tag's *name* —
    /// the wreckage the old resolver created — must still resolve, or those
    /// cards can never be untagged.
    #[test]
    fn ulid_shaped_tag_name_resolves_by_name() {
        let tags = vec![tag(INIT_ID, BUG_ID)];
        assert_eq!(resolve_tag_ref(&tags, BUG_ID).unwrap(), BUG_ID);
    }

    /// Replace really replaces, including markers sitting next to punctuation
    /// that `parse_tags` counts as tags.
    #[test]
    fn replace_clears_markers_next_to_punctuation() {
        let body = "Fix #bug, then ship #login.";
        let replaced = rewrite_body(body, &["feature".to_string()], TagApply::Replace).unwrap();
        assert_eq!(
            tag_parser::parse_tags(&replaced),
            vec!["feature".to_string()]
        );
    }

    /// An empty replace set clears every tag, whatever punctuation surrounds it.
    #[test]
    fn replace_with_no_slugs_clears_every_tag() {
        let body = "Fix #bug, then ship #login.";
        let cleared = rewrite_body(body, &[], TagApply::Replace).unwrap();
        assert_eq!(tag_parser::parse_tags(&cleared), Vec::<String>::new());
        assert_ne!(cleared, body, "clearing must actually rewrite the body");
    }

    /// A body ending in a code fence or a heading swallows an inline marker.
    /// The write must still round-trip — the tag has to be readable back.
    #[test]
    fn append_round_trips_on_bodies_that_swallow_inline_markers() {
        for body in [
            "Repro:\n```\ncargo test\n```",
            "Intro\n\n## Acceptance",
            "# Just a heading",
        ] {
            let applied = rewrite_body(body, &["bug".to_string()], TagApply::Append)
                .unwrap_or_else(|e| panic!("append failed for {body:?}: {e}"));
            assert!(
                tag_parser::parse_tags(&applied).contains(&"bug".to_string()),
                "tag did not round-trip for {body:?}, got: {applied:?}"
            );
        }
    }

    /// An unbalanced fence hides anything appended after it. That must be a
    /// loud error, never an `ok` that changed nothing readable.
    #[test]
    fn append_into_an_unbalanced_fence_errors() {
        let body = "text\n```\nnever closed";
        let err = rewrite_body(body, &["bug".to_string()], TagApply::Append).unwrap_err();
        assert!(
            matches!(err, KanbanError::Parse { .. }),
            "expected a parse error, got: {err}"
        );
    }

    /// Removing a marker next to punctuation works — it is a tag, so it must be
    /// removable.
    #[test]
    fn remove_clears_markers_next_to_punctuation() {
        let removed = rewrite_body(
            "Fix #bug, then ship",
            &["bug".to_string()],
            TagApply::Remove,
        )
        .unwrap();
        assert_eq!(tag_parser::parse_tags(&removed), Vec::<String>::new());
    }

    /// Replace must not eat prose, including heading text the parser never
    /// counted as a tag.
    #[test]
    fn replace_spares_prose_and_heading_text() {
        let body = "# Fix #login\n\nkeep this prose #stale";
        let replaced = rewrite_body(body, &["fresh".to_string()], TagApply::Replace).unwrap();
        assert!(
            replaced.contains("# Fix #login") && replaced.contains("keep this prose"),
            "replace ate prose: {replaced:?}"
        );
        assert_eq!(tag_parser::parse_tags(&replaced), vec!["fresh".to_string()]);
    }

    /// Two refs naming one tag, or a repeated ref, is not an error — the tag
    /// simply applies once.
    #[test]
    fn replace_tolerates_refs_that_resolve_to_one_slug() {
        let slugs = ["bug".to_string(), "bug".to_string()];
        // The caller dedupes before `rewrite_body`; prove the deduped call is
        // what `rewrite_body` accepts, and that a duplicate would be caught.
        assert!(rewrite_body("body", &slugs[..1], TagApply::Replace).is_ok());
        assert!(
            rewrite_body("body", &slugs, TagApply::Replace).is_err(),
            "rewrite_body counts slugs, so callers must dedupe first"
        );
    }

    #[test]
    fn strip_all_tags_removes_body_markers_but_spares_code() {
        let body = "fix #bug in #login\n```\n#fenced\n```\nand `#inline`";
        let stripped = strip_all_tags(body);
        assert_eq!(tag_parser::parse_tags(&stripped), Vec::<String>::new());
        assert!(
            stripped.contains("#fenced") && stripped.contains("#inline"),
            "code markers are not tags and must survive, got: {stripped}"
        );
    }
}
