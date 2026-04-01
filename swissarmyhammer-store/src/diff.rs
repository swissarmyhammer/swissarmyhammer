//! Text diff and patch operations wrapping the `diffy` crate.
//!
//! Provides forward/reverse patch generation, patch application, and
//! three-way merge for concurrent edit resolution during undo.

use crate::error::{Result, StoreError};

/// Create forward and reverse patches between two text strings.
///
/// Returns a tuple of `(forward_patch, reverse_patch)` where:
/// - `forward_patch` transforms `old` into `new`
/// - `reverse_patch` transforms `new` back into `old`
///
/// Both patches are serialized as unified diff strings.
pub fn create_patches(old: &str, new: &str) -> (String, String) {
    let forward = diffy::create_patch(old, new);
    let reverse = diffy::create_patch(new, old);
    (forward.to_string(), reverse.to_string())
}

/// Apply a unified diff patch to a text string.
///
/// Returns the patched text, or an error if the patch cannot be applied
/// (e.g., the base text has diverged from what the patch expects).
pub fn apply_patch(text: &str, patch: &str) -> Result<String> {
    let parsed = diffy::Patch::from_str(patch)
        .map_err(|e| StoreError::PatchFailed(format!("parse error: {}", e)))?;
    diffy::apply(text, &parsed).map_err(|e| StoreError::PatchFailed(e.to_string()))
}

/// Perform a three-way merge between a base, current, and target text.
///
/// - `base`: the common ancestor (e.g., the text at the time of the original edit)
/// - `current`: the current on-disk text (may have been modified concurrently)
/// - `target`: the desired text (e.g., the undo target)
///
/// Returns the merged text, or an error if there are conflicts.
pub fn three_way_merge(base: &str, current: &str, target: &str) -> Result<String> {
    diffy::merge(base, current, target)
        .map_err(|conflict| StoreError::MergeConflict(conflict.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_patches_produces_valid_forward_and_reverse() {
        let old = "line1\nline2\nline3\n";
        let new = "line1\nmodified\nline3\n";

        let (forward, reverse) = create_patches(old, new);
        assert!(!forward.is_empty());
        assert!(!reverse.is_empty());
    }

    #[test]
    fn apply_forward_patch_transforms_old_to_new() {
        let old = "line1\nline2\nline3\n";
        let new = "line1\nmodified\nline3\n";

        let (forward, _) = create_patches(old, new);
        let result = apply_patch(old, &forward).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn apply_reverse_patch_transforms_new_to_old() {
        let old = "line1\nline2\nline3\n";
        let new = "line1\nmodified\nline3\n";

        let (_, reverse) = create_patches(old, new);
        let result = apply_patch(new, &reverse).unwrap();
        assert_eq!(result, old);
    }

    #[test]
    fn three_way_merge_no_conflict() {
        // Use enough context lines so diffy sees the edits as separate hunks
        let base = "line1\nline2\nline3\nline4\nline5\nline6\nline7\n";
        let current = "line1\ncurrent_change\nline3\nline4\nline5\nline6\nline7\n";
        let target = "line1\nline2\nline3\nline4\nline5\nline6\ntarget_change\n";

        // Non-overlapping changes should merge cleanly
        let result = three_way_merge(base, current, target).unwrap();
        assert!(result.contains("current_change"));
        assert!(result.contains("target_change"));
    }

    #[test]
    fn three_way_merge_conflict_returns_error() {
        let base = "line1\nline2\nline3\n";
        let current = "line1\ncurrent_change\nline3\n";
        let target = "line1\ntarget_change\nline3\n";

        // Both modify the same line -- should conflict
        let result = three_way_merge(base, current, target);
        assert!(result.is_err());
        if let Err(StoreError::MergeConflict(_)) = result {
            // expected
        } else {
            panic!("expected MergeConflict error");
        }
    }

    #[test]
    fn apply_patch_with_empty_texts() {
        let old = "";
        let new = "new content\n";

        let (forward, reverse) = create_patches(old, new);
        assert_eq!(apply_patch(old, &forward).unwrap(), new);
        assert_eq!(apply_patch(new, &reverse).unwrap(), old);
    }

    #[test]
    fn apply_patch_identical_texts() {
        let text = "same\n";
        let (forward, _) = create_patches(text, text);
        let result = apply_patch(text, &forward).unwrap();
        assert_eq!(result, text);
    }
}
