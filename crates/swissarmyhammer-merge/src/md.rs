//! Markdown merge strategy: YAML frontmatter + line-level body merge.
//!
//! This module merges markdown files that may optionally contain a YAML frontmatter
//! block (delimited by `---` fences).  The merge operates in two phases:
//!
//! 1. **Frontmatter phase** — the frontmatter YAML (if present) is merged using the
//!    same three-way field merge from [`crate::yaml`].
//! 2. **Body phase** — the markdown body is merged line-by-line using `diffy`'s
//!    three-way text merge.  Non-overlapping changes from both sides are auto-merged;
//!    overlapping changes produce git-style conflict markers and cause the function to
//!    return `Err(MergeConflict)`.
//!
//! If a file has no frontmatter the entire content is treated as body and only the
//! line-level merge is performed.

use crate::frontmatter::{join_frontmatter, split_frontmatter};
use crate::yaml::{merge_yaml, MergeOpts};
use crate::{MergeConflict, MergeError};

/// Merge three markdown documents using YAML frontmatter field merge + line-level body
/// merge.
///
/// # Arguments
/// - `base` — common ancestor markdown content
/// - `ours` — our branch markdown content
/// - `theirs` — their branch markdown content
/// - `opts` — merge options forwarded to the frontmatter YAML merge (optional JSONL
///   changelog path, fallback precedence)
///
/// # Returns
/// - `Ok(merged_string)` when the merge succeeds with no unresolvable conflicts.
/// - `Err(MergeError::ParseFailure)` when any input cannot be parsed (propagated from
///   frontmatter YAML merge).
/// - `Err(MergeError::Conflict)` when the body has overlapping edits that cannot be
///   auto-merged. The inner `MergeConflict.conflicting_ids` contains a human-readable
///   description (the text of the conflict markers).
///
/// # Body merge behaviour
/// The body is passed directly to `diffy::merge` which performs a line-level three-way
/// merge.  `Ok` from `diffy` means the merge was clean; `Err` from `diffy` means
/// conflict markers were inserted into the output text.  We propagate that as a
/// `MergeError::Conflict`.
pub fn merge_md(
    base: &str,
    ours: &str,
    theirs: &str,
    opts: &MergeOpts,
) -> Result<String, MergeError> {
    let base_parts = split_frontmatter(base);
    let ours_parts = split_frontmatter(ours);
    let theirs_parts = split_frontmatter(theirs);

    // --- Merge the frontmatter ---
    // If any side has frontmatter, merge it.  If no side has frontmatter the result has
    // none either.
    let merged_frontmatter: Option<String> = match (
        &base_parts.frontmatter,
        &ours_parts.frontmatter,
        &theirs_parts.frontmatter,
    ) {
        (None, None, None) => None,
        _ => {
            // Treat missing frontmatter on any side as an empty YAML mapping.
            let base_fm = base_parts.frontmatter.as_deref().unwrap_or("");
            let ours_fm = ours_parts.frontmatter.as_deref().unwrap_or("");
            let theirs_fm = theirs_parts.frontmatter.as_deref().unwrap_or("");

            let merged = merge_yaml(base_fm, ours_fm, theirs_fm, opts)?;
            // An empty merged frontmatter (all fields removed) yields None, collapsing the
            // fences.
            if merged.trim().is_empty() {
                None
            } else {
                Some(merged.trim_end_matches('\n').to_owned())
            }
        }
    };

    // --- Merge the body ---
    let base_body = &base_parts.body;
    let ours_body = &ours_parts.body;
    let theirs_body = &theirs_parts.body;

    let merged_body = diffy::merge(base_body, ours_body, theirs_body).map_err(|conflict_text| {
        MergeError::Conflict(MergeConflict {
            conflicting_ids: vec![conflict_text],
        })
    })?;

    // --- Reassemble ---
    Ok(join_frontmatter(
        merged_frontmatter.as_deref(),
        &merged_body,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::yaml::{MergeOpts, Precedence};

    fn default_opts() -> MergeOpts {
        MergeOpts {
            jsonl_path: None,
            fallback_precedence: Precedence::Theirs,
        }
    }

    /// Both sides change only frontmatter fields (body is identical) → clean merge.
    #[test]
    fn frontmatter_only_changes_clean_merge() {
        let base = "---\ntitle: Original\ncolor: red\n---\n\nBody unchanged.\n";
        // Ours changed title; theirs changed color.
        let ours = "---\ntitle: Updated\ncolor: red\n---\n\nBody unchanged.\n";
        let theirs = "---\ntitle: Original\ncolor: blue\n---\n\nBody unchanged.\n";

        let merged = merge_md(base, ours, theirs, &default_opts()).expect("should merge cleanly");

        assert!(
            merged.contains("title: Updated"),
            "ours title change should appear"
        );
        assert!(
            merged.contains("color: blue"),
            "theirs color change should appear"
        );
        assert!(
            merged.contains("Body unchanged."),
            "body should be preserved"
        );
    }

    /// Both sides change only the body (frontmatter is identical) → clean merge.
    #[test]
    fn body_only_changes_clean_merge() {
        let base = "---\ntitle: Doc\n---\n\nOriginal paragraph.\n\nStable section.\n";
        // Ours added a sentence after the first paragraph.
        let ours =
            "---\ntitle: Doc\n---\n\nOriginal paragraph.\n\nAdded by ours.\n\nStable section.\n";
        // Theirs modified the stable section.
        let theirs =
            "---\ntitle: Doc\n---\n\nOriginal paragraph.\n\nStable section.\n\nAdded by theirs.\n";

        let merged = merge_md(base, ours, theirs, &default_opts()).expect("should merge cleanly");

        assert!(
            merged.contains("Added by ours."),
            "ours body addition should appear"
        );
        assert!(
            merged.contains("Added by theirs."),
            "theirs body addition should appear"
        );
        assert!(
            merged.contains("title: Doc"),
            "frontmatter should be preserved"
        );
    }

    /// Both frontmatter and body changed on different sides → clean merge.
    #[test]
    fn both_parts_changed_different_sides_clean_merge() {
        let base = "---\ntitle: Original\n---\n\nBody line.\n";
        // Ours changed the title in frontmatter.
        let ours = "---\ntitle: Updated\n---\n\nBody line.\n";
        // Theirs added a body line.
        let theirs = "---\ntitle: Original\n---\n\nBody line.\n\nTheirs extra.\n";

        let merged = merge_md(base, ours, theirs, &default_opts()).expect("should merge cleanly");

        assert!(
            merged.contains("title: Updated"),
            "ours frontmatter change should appear"
        );
        assert!(
            merged.contains("Theirs extra."),
            "theirs body addition should appear"
        );
    }

    /// Overlapping body edits → conflict markers and Err result.
    #[test]
    fn overlapping_body_edits_produce_conflict() {
        let base = "---\ntitle: Doc\n---\n\nConflict line.\n";
        // Both sides edit the same body line differently.
        let ours = "---\ntitle: Doc\n---\n\nOurs version of line.\n";
        let theirs = "---\ntitle: Doc\n---\n\nTheirs version of line.\n";

        let err = merge_md(base, ours, theirs, &default_opts()).expect_err("conflict expected");
        // The error must be a Conflict (not a ParseFailure).
        let conflict = match err {
            crate::MergeError::Conflict(c) => c,
            other => panic!("expected MergeError::Conflict, got: {other:?}"),
        };
        let conflict_text = &conflict.conflicting_ids[0];
        assert!(
            conflict_text.contains("<<<<<<<") || conflict_text.contains("======="),
            "conflict markers should be present in the error: {conflict_text}"
        );
    }

    /// File with no frontmatter merges as pure line-level text.
    #[test]
    fn no_frontmatter_pure_line_merge() {
        let base = "First line.\nSecond line.\n";
        // Ours added a line before second.
        let ours = "First line.\nOurs addition.\nSecond line.\n";
        // Theirs added a line after second.
        let theirs = "First line.\nSecond line.\nTheirs addition.\n";

        let merged = merge_md(base, ours, theirs, &default_opts()).expect("should merge cleanly");

        // No frontmatter fences in the output.
        assert!(
            !merged.starts_with("---"),
            "should have no frontmatter fences"
        );
        assert!(
            merged.contains("Ours addition."),
            "ours addition should appear"
        );
        assert!(
            merged.contains("Theirs addition."),
            "theirs addition should appear"
        );
        assert!(
            merged.contains("First line."),
            "original content should be preserved"
        );
    }

    /// When frontmatter is absent on all sides, the output has no frontmatter fences.
    #[test]
    fn no_frontmatter_preserved_in_output() {
        let base = "Hello world.\n";
        let merged =
            merge_md(base, base, base, &default_opts()).expect("identical inputs merge cleanly");
        assert!(
            !merged.starts_with("---"),
            "should have no frontmatter fences"
        );
        assert_eq!(merged, base);
    }

    /// Identical inputs on all three sides → output matches base.
    #[test]
    fn identical_inputs_return_base() {
        let content = "---\ntitle: Same\n---\n\nUnchanged body.\n";
        let merged = merge_md(content, content, content, &default_opts()).expect("no conflict");
        assert!(merged.contains("title: Same"));
        assert!(merged.contains("Unchanged body."));
    }

    /// Ours adds frontmatter to a file that had none; theirs leaves body unchanged.
    #[test]
    fn ours_adds_frontmatter_to_plain_file() {
        let base = "Plain body.\n";
        let ours = "---\ntitle: New\n---\nPlain body.\n";
        let theirs = "Plain body.\n";

        let merged = merge_md(base, ours, theirs, &default_opts()).expect("should merge cleanly");
        assert!(
            merged.contains("title: New"),
            "ours frontmatter addition should be present"
        );
        assert!(merged.contains("Plain body."), "body should be preserved");
    }

    /// Theirs adds frontmatter to a plain file; base and ours have none.
    /// This is the symmetric counterpart to `ours_adds_frontmatter_to_plain_file`.
    #[test]
    fn theirs_adds_frontmatter_to_plain_file() {
        let base = "Plain body.\n";
        let ours = "Plain body.\n";
        let theirs = "---\ntitle: TheirsTitle\n---\nPlain body.\n";

        let merged = merge_md(base, ours, theirs, &default_opts()).expect("should merge cleanly");
        assert!(
            merged.contains("title: TheirsTitle"),
            "theirs frontmatter addition should be present"
        );
        assert!(merged.contains("Plain body."), "body should be preserved");
    }

    /// Invalid YAML in the base frontmatter should produce MergeError::ParseFailure.
    #[test]
    fn invalid_yaml_frontmatter_produces_parse_failure() {
        // This frontmatter is syntactically invalid YAML (unquoted colon in value).
        let base = "---\ntitle: [broken: yaml: {{{\n---\n\nBody.\n";
        let ours = "---\ntitle: [broken: yaml: {{{\n---\n\nBody.\n";
        let theirs = "---\ntitle: [broken: yaml: {{{\n---\n\nBody.\n";

        let err = merge_md(base, ours, theirs, &default_opts())
            .expect_err("should fail to parse bad YAML");
        assert!(
            matches!(err, crate::MergeError::ParseFailure(_)),
            "expected ParseFailure, got: {err:?}"
        );
    }

    /// When all sides have empty frontmatter (all fields removed), the merged result
    /// should have no frontmatter fences — empty frontmatter collapses to None.
    #[test]
    fn empty_frontmatter_all_sides_yields_no_fences() {
        // All three sides have `---\n---\n` (empty frontmatter block).
        let base = "---\n---\nBody.\n";
        let ours = "---\n---\nBody.\n";
        let theirs = "---\n---\nBody.\n";

        let merged = merge_md(base, ours, theirs, &default_opts()).expect("should merge cleanly");
        assert!(
            !merged.starts_with("---"),
            "empty merged frontmatter should produce no fences: {merged:?}"
        );
        assert!(merged.contains("Body."), "body should be preserved");
    }

    /// JSONL changelog path is exercised through merge_md: a frontmatter conflict resolved
    /// by the changelog should pick the side whose value was most recently logged.
    #[test]
    fn jsonl_changelog_resolves_frontmatter_conflict() {
        use std::io::Write;

        // Create a JSONL changelog that records "title" was last set to "TheirsTitle".
        let dir = tempfile::tempdir().unwrap();
        let jsonl_path = dir.path().join("task.jsonl");
        let entry = r#"{"id":"01AAA000000000000000000001","timestamp":"2026-03-01T12:00:00Z","op":"update","entity_type":"task","entity_id":"abc","changes":[["title",{"kind":"changed","old_value":"Original","new_value":"TheirsTitle"}]]}"#;
        let mut f = std::fs::File::create(&jsonl_path).unwrap();
        writeln!(f, "{entry}").unwrap();

        let base = "---\ntitle: Original\n---\n\nBody.\n";
        let ours = "---\ntitle: OursTitle\n---\n\nBody.\n";
        let theirs = "---\ntitle: TheirsTitle\n---\n\nBody.\n";

        // fallback_precedence = Ours, but the JSONL should override and pick theirs.
        let opts = crate::yaml::MergeOpts {
            jsonl_path: Some(jsonl_path),
            fallback_precedence: crate::yaml::Precedence::Ours,
        };

        let merged = merge_md(base, ours, theirs, &opts).expect("should merge cleanly");
        assert!(
            merged.contains("title: TheirsTitle"),
            "JSONL changelog should cause theirs to win despite fallback_precedence=Ours: {merged}"
        );
        assert!(merged.contains("Body."), "body should be preserved");
    }
}
