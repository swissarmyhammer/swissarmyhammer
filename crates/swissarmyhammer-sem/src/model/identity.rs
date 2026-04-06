use std::collections::{HashMap, HashSet};

use super::change::{ChangeType, SemanticChange};
use super::entity::SemanticEntity;

/// Function that computes similarity between two semantic entities, returning a
/// score in [0.0, 1.0].
pub type SimilarityFn<'a> = &'a dyn Fn(&SemanticEntity, &SemanticEntity) -> f64;

pub struct MatchResult {
    pub changes: Vec<SemanticChange>,
}

/// 3-phase entity matching algorithm:
/// 1. Exact ID match — same entity ID in before/after → modified or unchanged
/// 2. Content hash match — same hash, different ID → renamed or moved
/// 3. Fuzzy similarity — >80% content similarity → probable rename
pub fn match_entities(
    before: &[SemanticEntity],
    after: &[SemanticEntity],
    _file_path: &str,
    similarity_fn: Option<SimilarityFn<'_>>,
    commit_sha: Option<&str>,
    author: Option<&str>,
) -> MatchResult {
    let mut changes: Vec<SemanticChange> = Vec::new();
    let mut matched_before: HashSet<&str> = HashSet::new();
    let mut matched_after: HashSet<&str> = HashSet::new();

    let before_by_id: HashMap<&str, &SemanticEntity> =
        before.iter().map(|e| (e.id.as_str(), e)).collect();
    let after_by_id: HashMap<&str, &SemanticEntity> =
        after.iter().map(|e| (e.id.as_str(), e)).collect();

    // Phase 1: Exact ID match
    for (&id, after_entity) in &after_by_id {
        if let Some(before_entity) = before_by_id.get(id) {
            matched_before.insert(id);
            matched_after.insert(id);

            if before_entity.content_hash != after_entity.content_hash {
                let structural_change = match (
                    &before_entity.structural_hash,
                    &after_entity.structural_hash,
                ) {
                    (Some(before_sh), Some(after_sh)) => Some(before_sh != after_sh),
                    _ => None,
                };
                changes.push(SemanticChange {
                    id: format!("change::{id}"),
                    entity_id: id.to_string(),
                    change_type: ChangeType::Modified,
                    entity_type: after_entity.entity_type.clone(),
                    entity_name: after_entity.name.clone(),
                    file_path: after_entity.file_path.clone(),
                    old_file_path: None,
                    before_content: Some(before_entity.content.clone()),
                    after_content: Some(after_entity.content.clone()),
                    commit_sha: commit_sha.map(String::from),
                    author: author.map(String::from),
                    timestamp: None,
                    structural_change,
                });
            }
        }
    }

    // Collect unmatched
    let unmatched_before: Vec<&SemanticEntity> = before
        .iter()
        .filter(|e| !matched_before.contains(e.id.as_str()))
        .collect();
    let unmatched_after: Vec<&SemanticEntity> = after
        .iter()
        .filter(|e| !matched_after.contains(e.id.as_str()))
        .collect();

    // Phase 2: Content hash match (rename/move detection)
    let mut before_by_hash: HashMap<&str, Vec<&SemanticEntity>> = HashMap::new();
    let mut before_by_structural: HashMap<&str, Vec<&SemanticEntity>> = HashMap::new();
    for entity in &unmatched_before {
        before_by_hash
            .entry(entity.content_hash.as_str())
            .or_default()
            .push(entity);
        if let Some(ref sh) = entity.structural_hash {
            before_by_structural
                .entry(sh.as_str())
                .or_default()
                .push(entity);
        }
    }

    for after_entity in &unmatched_after {
        if matched_after.contains(after_entity.id.as_str()) {
            continue;
        }
        // Try exact content_hash first
        let found = before_by_hash
            .get_mut(after_entity.content_hash.as_str())
            .and_then(|c| c.pop());
        // Fall back to structural_hash (formatting/comment changes don't matter)
        let found = found.or_else(|| {
            after_entity.structural_hash.as_ref().and_then(|sh| {
                before_by_structural.get_mut(sh.as_str()).and_then(|c| {
                    c.iter()
                        .position(|e| !matched_before.contains(e.id.as_str()))
                        .map(|i| c.remove(i))
                })
            })
        });

        if let Some(before_entity) = found {
            matched_before.insert(&before_entity.id);
            matched_after.insert(&after_entity.id);

            let change_type = if before_entity.file_path != after_entity.file_path {
                ChangeType::Moved
            } else {
                ChangeType::Renamed
            };

            let old_file_path = if before_entity.file_path != after_entity.file_path {
                Some(before_entity.file_path.clone())
            } else {
                None
            };

            changes.push(SemanticChange {
                id: format!("change::{}", after_entity.id),
                entity_id: after_entity.id.clone(),
                change_type,
                entity_type: after_entity.entity_type.clone(),
                entity_name: after_entity.name.clone(),
                file_path: after_entity.file_path.clone(),
                old_file_path,
                before_content: Some(before_entity.content.clone()),
                after_content: Some(after_entity.content.clone()),
                commit_sha: commit_sha.map(String::from),
                author: author.map(String::from),
                timestamp: None,
                structural_change: None,
            });
        }
    }

    // Phase 3: Fuzzy similarity (>80% threshold)
    let still_unmatched_before: Vec<&SemanticEntity> = unmatched_before
        .iter()
        .filter(|e| !matched_before.contains(e.id.as_str()))
        .copied()
        .collect();
    let still_unmatched_after: Vec<&SemanticEntity> = unmatched_after
        .iter()
        .filter(|e| !matched_after.contains(e.id.as_str()))
        .copied()
        .collect();

    if let Some(sim_fn) = similarity_fn {
        if !still_unmatched_before.is_empty() && !still_unmatched_after.is_empty() {
            const THRESHOLD: f64 = 0.8;
            // Size ratio filter: pairs with very different content lengths can't reach 0.8 Jaccard
            const SIZE_RATIO_CUTOFF: f64 = 0.5;

            // Pre-compute content lengths for O(1) size filtering
            let before_lens: Vec<usize> = still_unmatched_before
                .iter()
                .map(|e| e.content.split_whitespace().count())
                .collect();
            let after_lens: Vec<usize> = still_unmatched_after
                .iter()
                .map(|e| e.content.split_whitespace().count())
                .collect();

            for (ai, after_entity) in still_unmatched_after.iter().enumerate() {
                let mut best_match: Option<&SemanticEntity> = None;
                let mut best_score: f64 = 0.0;
                let a_len = after_lens[ai];

                for (bi, before_entity) in still_unmatched_before.iter().enumerate() {
                    if matched_before.contains(before_entity.id.as_str()) {
                        continue;
                    }
                    if before_entity.entity_type != after_entity.entity_type {
                        continue;
                    }

                    // Early exit: skip pairs where token count ratio is too different
                    let b_len = before_lens[bi];
                    let (min_l, max_l) = if a_len < b_len {
                        (a_len, b_len)
                    } else {
                        (b_len, a_len)
                    };
                    if max_l > 0 && (min_l as f64 / max_l as f64) < SIZE_RATIO_CUTOFF {
                        continue;
                    }

                    let score = sim_fn(before_entity, after_entity);
                    if score > best_score && score >= THRESHOLD {
                        best_score = score;
                        best_match = Some(before_entity);
                    }
                }

                if let Some(matched) = best_match {
                    matched_before.insert(&matched.id);
                    matched_after.insert(&after_entity.id);

                    let change_type = if matched.file_path != after_entity.file_path {
                        ChangeType::Moved
                    } else {
                        ChangeType::Renamed
                    };

                    let old_file_path = if matched.file_path != after_entity.file_path {
                        Some(matched.file_path.clone())
                    } else {
                        None
                    };

                    changes.push(SemanticChange {
                        id: format!("change::{}", after_entity.id),
                        entity_id: after_entity.id.clone(),
                        change_type,
                        entity_type: after_entity.entity_type.clone(),
                        entity_name: after_entity.name.clone(),
                        file_path: after_entity.file_path.clone(),
                        old_file_path,
                        before_content: Some(matched.content.clone()),
                        after_content: Some(after_entity.content.clone()),
                        commit_sha: commit_sha.map(String::from),
                        author: author.map(String::from),
                        timestamp: None,
                        structural_change: None,
                    });
                }
            }
        }
    }

    // Remaining unmatched before = deleted
    for entity in before
        .iter()
        .filter(|e| !matched_before.contains(e.id.as_str()))
    {
        changes.push(SemanticChange {
            id: format!("change::deleted::{}", entity.id),
            entity_id: entity.id.clone(),
            change_type: ChangeType::Deleted,
            entity_type: entity.entity_type.clone(),
            entity_name: entity.name.clone(),
            file_path: entity.file_path.clone(),
            old_file_path: None,
            before_content: Some(entity.content.clone()),
            after_content: None,
            commit_sha: commit_sha.map(String::from),
            author: author.map(String::from),
            timestamp: None,
            structural_change: None,
        });
    }

    // Remaining unmatched after = added
    for entity in after
        .iter()
        .filter(|e| !matched_after.contains(e.id.as_str()))
    {
        changes.push(SemanticChange {
            id: format!("change::added::{}", entity.id),
            entity_id: entity.id.clone(),
            change_type: ChangeType::Added,
            entity_type: entity.entity_type.clone(),
            entity_name: entity.name.clone(),
            file_path: entity.file_path.clone(),
            old_file_path: None,
            before_content: None,
            after_content: Some(entity.content.clone()),
            commit_sha: commit_sha.map(String::from),
            author: author.map(String::from),
            timestamp: None,
            structural_change: None,
        });
    }

    MatchResult { changes }
}

/// Default content similarity using Jaccard index on whitespace-split tokens
pub fn default_similarity(a: &SemanticEntity, b: &SemanticEntity) -> f64 {
    let tokens_a: Vec<&str> = a.content.split_whitespace().collect();
    let tokens_b: Vec<&str> = b.content.split_whitespace().collect();

    // Early rejection: if token counts differ too much, Jaccard can't reach 0.8
    let (min_c, max_c) = if tokens_a.len() < tokens_b.len() {
        (tokens_a.len(), tokens_b.len())
    } else {
        (tokens_b.len(), tokens_a.len())
    };
    if max_c > 0 && (min_c as f64 / max_c as f64) < 0.6 {
        return 0.0;
    }

    let set_a: HashSet<&str> = tokens_a.into_iter().collect();
    let set_b: HashSet<&str> = tokens_b.into_iter().collect();

    let intersection_size = set_a.intersection(&set_b).count();
    let union_size = set_a.union(&set_b).count();

    if union_size == 0 {
        return 0.0;
    }

    intersection_size as f64 / union_size as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::hash::content_hash;

    fn make_entity(id: &str, name: &str, content: &str, file_path: &str) -> SemanticEntity {
        SemanticEntity {
            id: id.to_string(),
            file_path: file_path.to_string(),
            entity_type: "function".to_string(),
            name: name.to_string(),
            parent_id: None,
            content: content.to_string(),
            content_hash: content_hash(content),
            structural_hash: None,
            start_line: 1,
            end_line: 1,
            metadata: None,
        }
    }

    #[test]
    fn test_exact_match_modified() {
        let before = vec![make_entity("a::f::foo", "foo", "old content", "a.ts")];
        let after = vec![make_entity("a::f::foo", "foo", "new content", "a.ts")];
        let result = match_entities(&before, &after, "a.ts", None, None, None);
        assert_eq!(result.changes.len(), 1);
        assert_eq!(result.changes[0].change_type, ChangeType::Modified);
    }

    #[test]
    fn test_exact_match_unchanged() {
        let before = vec![make_entity("a::f::foo", "foo", "same", "a.ts")];
        let after = vec![make_entity("a::f::foo", "foo", "same", "a.ts")];
        let result = match_entities(&before, &after, "a.ts", None, None, None);
        assert_eq!(result.changes.len(), 0);
    }

    #[test]
    fn test_added_deleted() {
        let before = vec![make_entity("a::f::old", "old", "content", "a.ts")];
        let after = vec![make_entity("a::f::new", "new", "different", "a.ts")];
        let result = match_entities(&before, &after, "a.ts", None, None, None);
        assert_eq!(result.changes.len(), 2);
        let types: Vec<ChangeType> = result.changes.iter().map(|c| c.change_type).collect();
        assert!(types.contains(&ChangeType::Deleted));
        assert!(types.contains(&ChangeType::Added));
    }

    #[test]
    fn test_content_hash_rename() {
        let before = vec![make_entity("a::f::old", "old", "same content", "a.ts")];
        let after = vec![make_entity("a::f::new", "new", "same content", "a.ts")];
        let result = match_entities(&before, &after, "a.ts", None, None, None);
        assert_eq!(result.changes.len(), 1);
        assert_eq!(result.changes[0].change_type, ChangeType::Renamed);
    }

    #[test]
    fn test_default_similarity() {
        let a = make_entity("a", "a", "the quick brown fox", "a.ts");
        let b = make_entity("b", "b", "the quick brown dog", "a.ts");
        let score = default_similarity(&a, &b);
        assert!(score > 0.5);
        assert!(score < 1.0);
    }

    /// Helper that creates an entity with a structural_hash set explicitly.
    fn make_entity_with_structural(
        id: &str,
        name: &str,
        content: &str,
        file_path: &str,
        structural: &str,
    ) -> SemanticEntity {
        let mut e = make_entity(id, name, content, file_path);
        e.structural_hash = Some(structural.to_string());
        e
    }

    // ------------------------------------------------------------------
    // Move detection (Phase 2 – same content_hash, different file_path)
    // ------------------------------------------------------------------

    #[test]
    fn test_content_hash_move_different_files() {
        // Entity moved from a.ts to b.ts — same ID hash, different file path.
        let before = vec![make_entity("a::f::foo", "foo", "same content", "a.ts")];
        let after = vec![make_entity("b::f::foo", "foo", "same content", "b.ts")];
        let result = match_entities(&before, &after, "a.ts", None, None, None);
        assert_eq!(result.changes.len(), 1);
        assert_eq!(result.changes[0].change_type, ChangeType::Moved);
        // old_file_path must be populated for moves
        assert_eq!(result.changes[0].old_file_path, Some("a.ts".to_string()));
        assert_eq!(result.changes[0].file_path, "b.ts");
    }

    #[test]
    fn test_content_hash_move_preserves_content() {
        // Verify before_content / after_content are carried over correctly.
        let before = vec![make_entity(
            "old::f::fn1",
            "fn1",
            "body text here",
            "old.ts",
        )];
        let after = vec![make_entity(
            "new::f::fn1",
            "fn1",
            "body text here",
            "new.ts",
        )];
        let result = match_entities(&before, &after, "old.ts", None, Some("sha1"), Some("alice"));
        assert_eq!(result.changes.len(), 1);
        let ch = &result.changes[0];
        assert_eq!(ch.change_type, ChangeType::Moved);
        assert_eq!(ch.before_content.as_deref(), Some("body text here"));
        assert_eq!(ch.after_content.as_deref(), Some("body text here"));
        assert_eq!(ch.commit_sha.as_deref(), Some("sha1"));
        assert_eq!(ch.author.as_deref(), Some("alice"));
    }

    // ------------------------------------------------------------------
    // Rename detection via structural_hash (Phase 2 fallback)
    // ------------------------------------------------------------------

    #[test]
    fn test_structural_hash_rename_same_file() {
        // Same structural hash, different content hash (e.g. comment change),
        // same file path → should be Renamed, not Moved.
        let before = vec![make_entity_with_structural(
            "a::f::old",
            "old",
            "fn old() { /* comment */ }",
            "a.ts",
            "struct-abc",
        )];
        let after = vec![make_entity_with_structural(
            "a::f::new",
            "new",
            "fn new() {}",
            "a.ts",
            "struct-abc",
        )];
        let result = match_entities(&before, &after, "a.ts", None, None, None);
        assert_eq!(result.changes.len(), 1);
        assert_eq!(result.changes[0].change_type, ChangeType::Renamed);
        assert_eq!(result.changes[0].old_file_path, None);
    }

    #[test]
    fn test_structural_hash_move_different_files() {
        // Same structural hash, different file paths → Moved.
        let before = vec![make_entity_with_structural(
            "a::f::fn1",
            "fn1",
            "fn fn1() { let x = 1; }",
            "a.ts",
            "struct-xyz",
        )];
        let after = vec![make_entity_with_structural(
            "b::f::fn1",
            "fn1",
            "fn fn1() { let y = 1; }",
            "b.ts",
            "struct-xyz",
        )];
        let result = match_entities(&before, &after, "a.ts", None, None, None);
        assert_eq!(result.changes.len(), 1);
        assert_eq!(result.changes[0].change_type, ChangeType::Moved);
        assert_eq!(result.changes[0].old_file_path, Some("a.ts".to_string()));
    }

    #[test]
    fn test_structural_hash_not_matched_when_already_matched() {
        // If an entity was already matched in Phase 1, it should not appear in Phase 2.
        let before = vec![
            make_entity_with_structural("id1", "fn1", "content1", "a.ts", "hash-s"),
            make_entity_with_structural("id2", "fn2", "content2", "a.ts", "hash-s"),
        ];
        // id1 matches exactly in Phase 1 (same ID). id2 is unmatched.
        let after = vec![
            make_entity_with_structural("id1", "fn1", "content1-changed", "a.ts", "hash-s"),
            make_entity_with_structural("id3", "fn3", "content3", "a.ts", "hash-s"),
        ];
        let result = match_entities(&before, &after, "a.ts", None, None, None);
        // id1 → Modified (Phase 1), id2 → structural match with id3 (Phase 2 → Renamed)
        let change_types: Vec<ChangeType> = result.changes.iter().map(|c| c.change_type).collect();
        assert!(change_types.contains(&ChangeType::Modified));
        assert!(change_types.contains(&ChangeType::Renamed));
        // id2 must not appear as Deleted since it was matched via structural hash
        assert!(!change_types.contains(&ChangeType::Deleted));
    }

    // ------------------------------------------------------------------
    // Phase 3 fuzzy similarity – move vs rename distinction
    // ------------------------------------------------------------------

    #[test]
    fn test_fuzzy_match_rename_same_file() {
        // High similarity (≥0.8) but same file path → Renamed.
        // 9 shared tokens, 1 unique each → Jaccard = 9/11 ≈ 0.818 ≥ 0.8.
        let base = "alpha beta gamma delta epsilon zeta eta theta iota";
        let before = vec![make_entity(
            "a::old",
            "old",
            &format!("{base} old_unique"),
            "a.ts",
        )];
        let after = vec![make_entity(
            "a::new",
            "new",
            &format!("{base} new_unique"),
            "a.ts",
        )];
        let result = match_entities(
            &before,
            &after,
            "a.ts",
            Some(&default_similarity),
            None,
            None,
        );
        assert_eq!(result.changes.len(), 1);
        assert_eq!(result.changes[0].change_type, ChangeType::Renamed);
        assert_eq!(result.changes[0].old_file_path, None);
    }

    #[test]
    fn test_fuzzy_match_move_different_files() {
        // High similarity (≥0.8), different file paths → Moved.
        // 9 shared tokens, 1 unique each → Jaccard = 9/11 ≈ 0.818 ≥ 0.8.
        let base = "alpha beta gamma delta epsilon zeta eta theta iota";
        let before = vec![make_entity(
            "a::fn1",
            "fn1",
            &format!("{base} old_unique"),
            "a.ts",
        )];
        let after = vec![make_entity(
            "b::fn1",
            "fn1",
            &format!("{base} new_unique"),
            "b.ts",
        )];
        let result = match_entities(
            &before,
            &after,
            "a.ts",
            Some(&default_similarity),
            None,
            None,
        );
        assert_eq!(result.changes.len(), 1);
        assert_eq!(result.changes[0].change_type, ChangeType::Moved);
        assert_eq!(result.changes[0].old_file_path, Some("a.ts".to_string()));
    }

    // ------------------------------------------------------------------
    // default_similarity boundary cases
    // ------------------------------------------------------------------

    #[test]
    fn test_default_similarity_identical_content() {
        // Identical content → score should be 1.0.
        let a = make_entity("a", "a", "hello world foo bar", "a.ts");
        let b = make_entity("b", "b", "hello world foo bar", "b.ts");
        let score = default_similarity(&a, &b);
        assert!((score - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_default_similarity_empty_content() {
        // Both entities have empty content → should return 0.0 (no tokens to compare).
        let a = make_entity("a", "a", "", "a.ts");
        let b = make_entity("b", "b", "", "b.ts");
        let score = default_similarity(&a, &b);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_default_similarity_completely_different() {
        // No tokens in common → score 0.0.
        let a = make_entity("a", "a", "alpha beta gamma", "a.ts");
        let b = make_entity("b", "b", "delta epsilon zeta", "b.ts");
        let score = default_similarity(&a, &b);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_default_similarity_size_ratio_rejected() {
        // Token count ratio below 0.6 → early return 0.0.
        // "a" has 2 tokens, "b" has 10+ tokens, ratio << 0.6.
        let a = make_entity("a", "a", "fn foo", "a.ts");
        let b = make_entity(
            "b",
            "b",
            "one two three four five six seven eight nine ten eleven twelve",
            "b.ts",
        );
        let score = default_similarity(&a, &b);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_default_similarity_at_threshold() {
        // Build content that produces a score at exactly (or just above) the 0.8 threshold
        // so that the fuzzy matcher picks it up.
        // 9 shared tokens out of 10 unique → Jaccard = 9/10 = 0.9 ≥ 0.8.
        let common = "one two three four five six seven eight nine";
        let a = make_entity("a", "a", &format!("{common} alpha"), "a.ts");
        let b = make_entity("b", "b", &format!("{common} beta"), "b.ts");
        let score = default_similarity(&a, &b);
        // 9 shared / 11 total unique = 9/11 ≈ 0.818
        assert!(score >= 0.8, "expected score >= 0.8, got {score}");
    }

    #[test]
    fn test_default_similarity_below_threshold() {
        // Build content that produces a score below 0.8 so fuzzy matcher ignores it.
        // 1 shared token out of 5 → Jaccard = 1/5 = 0.2.
        let a = make_entity("a", "a", "shared alpha beta gamma", "a.ts");
        let b = make_entity("b", "b", "shared delta epsilon zeta", "b.ts");
        let score = default_similarity(&a, &b);
        assert!(score < 0.8, "expected score < 0.8, got {score}");
    }

    // ------------------------------------------------------------------
    // Phase 3 fuzzy: no match below threshold → deleted + added
    // ------------------------------------------------------------------

    #[test]
    fn test_fuzzy_no_match_below_threshold() {
        // Entities are too different to match → should produce Deleted + Added.
        let before = vec![make_entity(
            "a::fn1",
            "fn1",
            "completely different code here",
            "a.ts",
        )];
        let after = vec![make_entity(
            "b::fn2",
            "fn2",
            "totally unrelated stuff there",
            "b.ts",
        )];
        let result = match_entities(
            &before,
            &after,
            "a.ts",
            Some(&default_similarity),
            None,
            None,
        );
        let types: Vec<ChangeType> = result.changes.iter().map(|c| c.change_type).collect();
        assert!(types.contains(&ChangeType::Deleted));
        assert!(types.contains(&ChangeType::Added));
        assert!(!types.contains(&ChangeType::Moved));
        assert!(!types.contains(&ChangeType::Renamed));
    }

    // ------------------------------------------------------------------
    // Multiple entities: partial move scenario
    // ------------------------------------------------------------------

    #[test]
    fn test_multiple_entities_one_moved_one_unchanged() {
        // Two entities: one stays in place, one moves to a different file.
        let before = vec![
            make_entity("a::fn_stay", "fn_stay", "stay content", "a.ts"),
            make_entity("a::fn_move", "fn_move", "move content", "a.ts"),
        ];
        let after = vec![
            make_entity("a::fn_stay", "fn_stay", "stay content", "a.ts"),
            make_entity("b::fn_move", "fn_move", "move content", "b.ts"),
        ];
        let result = match_entities(&before, &after, "a.ts", None, None, None);
        // fn_stay is unchanged → no change recorded; fn_move → Moved
        assert_eq!(result.changes.len(), 1);
        assert_eq!(result.changes[0].change_type, ChangeType::Moved);
        assert_eq!(result.changes[0].entity_id, "b::fn_move");
    }
}
