//! Short task identifiers derived from ULIDs, and a forgiving board resolver.
//!
//! A task's canonical *short id* is the last 7 Crockford-base32 characters of
//! its 26-char ULID, lowercased. It is never stored — always derived — and is
//! the canonical short handle shown to humans and accepted as forgiving input.
//!
//! # Why the suffix, not a prefix
//!
//! A ULID's first 10 characters encode a millisecond timestamp that advances
//! 32x slower per position, so tasks minted close in time share long leading
//! runs (a real 7-char prefix `01KT6SA` matched four sibling cards on this
//! board). The trailing 16 characters are uniform random; the last 7 give
//! ~35 bits — git-abbreviation length, collision-free to ~100k tasks even
//! within a same-second burst.

use crate::types::TaskId;
use std::collections::HashMap;
use std::collections::HashSet;

/// Number of trailing ULID characters that form the canonical short id.
pub const SHORT_ID_LEN: usize = 7;

/// Length of a full ULID, in characters.
const ULID_LEN: usize = 26;

/// Derive the canonical short id from a full ULID string.
///
/// The short id is the last [`SHORT_ID_LEN`] characters of `ulid`, lowercased.
/// ULIDs are pure ASCII (Crockford base32), so byte slicing on the last
/// `SHORT_ID_LEN` bytes is safe and equivalent to character slicing.
///
/// Inputs shorter than [`SHORT_ID_LEN`] are returned lowercased in full rather
/// than panicking — the function is a pure derivation with no error channel,
/// and malformed ids are a caller concern surfaced elsewhere.
///
/// # Examples
///
/// ```
/// use swissarmyhammer_kanban::types::short_id;
/// assert_eq!(short_id("01KT6R6HR3KJT6JVNDRAJV8V4T"), "ajv8v4t");
/// ```
pub fn short_id(ulid: &str) -> String {
    let start = ulid.len().saturating_sub(SHORT_ID_LEN);
    ulid[start..].to_lowercase()
}

impl TaskId {
    /// The canonical short id for this task: the last [`SHORT_ID_LEN`]
    /// characters of the ULID, lowercased. See [`short_id`].
    pub fn short_id(&self) -> String {
        short_id(self.as_str())
    }
}

/// Outcome of resolving a forgiving task reference against a board.
///
/// Not a bare `Option` because prefix matching must report ambiguity loudly:
/// a caller surfacing "ambiguous, N matches" needs the candidate list, and a
/// silent `None` would hide a real-but-non-unique prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveResult {
    /// Exactly one task matched.
    Found(TaskId),
    /// No task matched the reference.
    NotFound,
    /// More than one task's ULID started with the given prefix. The vector
    /// lists every matching task id so the caller can show their short ids.
    Ambiguous(Vec<TaskId>),
}

/// Resolve a forgiving task reference against the board's task ids.
///
/// `input` may be a full 26-char ULID, a 7-char short id (the canonical
/// suffix), either of those with a leading `^` sigil, or a git-style ULID
/// prefix. Matching is case-insensitive. Resolution proceeds in priority
/// order, so the canonical form always wins over a colliding prefix
/// interpretation:
///
/// 1. **Full ULID** — `input` equals a task's full 26-char ULID.
/// 2. **Exact short id** — `input` equals a task's 7-char short id (suffix).
/// 3. **ULID prefix** — `input` is a leading substring of one or more tasks'
///    ULIDs. Exactly one match → [`ResolveResult::Found`]; more than one →
///    [`ResolveResult::Ambiguous`]; none → [`ResolveResult::NotFound`].
///
/// An empty reference (after stripping `^`) never matches and returns
/// [`ResolveResult::NotFound`] rather than treating every task as a prefix
/// match.
///
/// The resolver is pure over the supplied id list so it stays trivially
/// unit-testable; callers load the board's task ids however they like.
pub fn resolve_short_ref(task_ids: &[TaskId], input: &str) -> ResolveResult {
    let needle = input.trim().strip_prefix('^').unwrap_or(input.trim());
    if needle.is_empty() {
        return ResolveResult::NotFound;
    }
    let needle = needle.to_lowercase();

    // 1. Full ULID match — canonical stored identity.
    if needle.len() == ULID_LEN {
        if let Some(id) = task_ids
            .iter()
            .find(|id| id.as_str().to_lowercase() == needle)
        {
            return ResolveResult::Found(id.clone());
        }
    }

    // 2. Exact short id match — the canonical short handle wins over any
    //    prefix interpretation of the same characters.
    if needle.len() == SHORT_ID_LEN {
        if let Some(id) = task_ids.iter().find(|id| id.short_id() == needle) {
            return ResolveResult::Found(id.clone());
        }
    }

    // 3. Git-style ULID prefix — forgiving input for hand-abbreviated ids.
    let prefix_matches: Vec<TaskId> = task_ids
        .iter()
        .filter(|id| id.as_str().to_lowercase().starts_with(&needle))
        .cloned()
        .collect();

    match prefix_matches.len() {
        0 => ResolveResult::NotFound,
        1 => ResolveResult::Found(prefix_matches.into_iter().next().unwrap()),
        _ => ResolveResult::Ambiguous(prefix_matches),
    }
}

/// Find every short id shared by two or more of `task_ids`.
///
/// Returns one entry per *colliding* short id: the short id itself paired with
/// the full list of tasks that derive it. Short ids that belong to exactly one
/// task are omitted, so an empty result means the board upholds the
/// board-unique short-id invariant.
///
/// This is the detection half of the invariant enforced at creation by
/// [`mint_unique_short_id`] — a safety net for tasks minted before the
/// invariant existed, surfaced by the kanban doctor.
pub fn find_short_id_collisions(task_ids: &[TaskId]) -> Vec<(String, Vec<TaskId>)> {
    let mut by_short: HashMap<String, Vec<TaskId>> = HashMap::new();
    for id in task_ids {
        by_short.entry(id.short_id()).or_default().push(id.clone());
    }
    by_short
        .into_iter()
        .filter(|(_, members)| members.len() > 1)
        .collect()
}

/// Mint a ULID whose canonical short id is unique against `existing`.
///
/// `mint` produces candidate full ULID strings (in production, a fresh random
/// `Ulid`). The candidate's [`short_id`] is checked against `existing` — the
/// set of short ids already on the board — and the first candidate whose short
/// id is absent is returned. On collision the loop re-mints.
///
/// `mint` is a parameter (not a hardwired `Ulid::new()`) so the retry path is
/// deterministically testable: a test can feed a candidate sequence whose first
/// entry collides and assert the loop advances to the unique one. Short-id
/// collisions are astronomically rare (~35 bits of entropy), so in production
/// this returns on the first candidate essentially always.
pub fn mint_unique_short_id(
    existing: &HashSet<String>,
    mut mint: impl FnMut() -> String,
) -> String {
    loop {
        let candidate = mint();
        if !existing.contains(&short_id(&candidate)) {
            return candidate;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // Real ULIDs from the short-ids epic board. The four `01KT6SA…` siblings
    // share the seven-char prefix `01KT6SA`, the exact same-prefix collision
    // case the resolver must report as ambiguous.
    const CORE: &str = "01KT6R6HR3KJT6JVNDRAJV8V4T";
    const SIBLING_A: &str = "01KT6SAMJAJ40XVQ9Y7JRAJ9VG";
    const SIBLING_B: &str = "01KT6SA4911JQPK09YQRC9RB4G";
    const SIBLING_C: &str = "01KT6SAAM6CR85YZD26JHSC87E";
    const SIBLING_D: &str = "01KT6SAXCBZFE6S0DEPZDJSQAA";

    fn board() -> Vec<TaskId> {
        [CORE, SIBLING_A, SIBLING_B, SIBLING_C, SIBLING_D]
            .iter()
            .map(|s| TaskId::from_string(*s))
            .collect()
    }

    #[test]
    fn short_id_is_last_seven_lowercased() {
        assert_eq!(short_id(CORE), "ajv8v4t");
        assert_eq!(short_id(CORE).len(), SHORT_ID_LEN);
    }

    #[test]
    fn short_id_method_matches_free_function() {
        let id = TaskId::from_string(CORE);
        assert_eq!(id.short_id(), short_id(CORE));
    }

    #[test]
    fn short_id_round_trips_via_suffix_match() {
        // The short id is exactly the lowercased suffix of the ULID, so the
        // full id ends with the short id once both are lowercased.
        let id = TaskId::from_string(SIBLING_A);
        assert!(id.as_str().to_lowercase().ends_with(&id.short_id()));
    }

    #[test]
    fn resolves_full_ulid() {
        let ids = board();
        assert_eq!(
            resolve_short_ref(&ids, CORE),
            ResolveResult::Found(TaskId::from_string(CORE))
        );
    }

    #[test]
    fn resolves_exact_short_id() {
        let ids = board();
        // `ajv8v4t` is CORE's short id; resolve by the bare short handle.
        assert_eq!(
            resolve_short_ref(&ids, "ajv8v4t"),
            ResolveResult::Found(TaskId::from_string(CORE))
        );
    }

    #[test]
    fn resolves_caret_prefixed_short_id() {
        let ids = board();
        assert_eq!(
            resolve_short_ref(&ids, "^ajv8v4t"),
            ResolveResult::Found(TaskId::from_string(CORE))
        );
    }

    #[test]
    fn resolution_is_case_insensitive() {
        let ids = board();
        assert_eq!(
            resolve_short_ref(&ids, "AJV8V4T"),
            ResolveResult::Found(TaskId::from_string(CORE))
        );
        assert_eq!(
            resolve_short_ref(&ids, CORE.to_lowercase().as_str()),
            ResolveResult::Found(TaskId::from_string(CORE))
        );
    }

    #[test]
    fn resolves_unique_ulid_prefix() {
        let ids = board();
        // `01KT6SAM` is unique to SIBLING_A among the siblings.
        assert_eq!(
            resolve_short_ref(&ids, "01KT6SAM"),
            ResolveResult::Found(TaskId::from_string(SIBLING_A))
        );
    }

    #[test]
    fn ambiguous_prefix_reports_all_matches() {
        let ids = board();
        // The real `01KT6SA` prefix matches all four siblings.
        match resolve_short_ref(&ids, "01KT6SA") {
            ResolveResult::Ambiguous(matches) => {
                assert_eq!(matches.len(), 4);
                for sibling in [SIBLING_A, SIBLING_B, SIBLING_C, SIBLING_D] {
                    assert!(
                        matches.contains(&TaskId::from_string(sibling)),
                        "ambiguous set should contain {sibling}"
                    );
                }
            }
            other => panic!("expected Ambiguous, got {other:?}"),
        }
    }

    #[test]
    fn exact_short_id_beats_colliding_prefix() {
        // A board where one task's 7-char short id equals a string that is
        // also a prefix of two other tasks' ULIDs. The exact short-id match
        // must win over the (otherwise ambiguous) prefix interpretation.
        let short_owner = TaskId::from_string("01KT6R6HR3KJT6JVNDR0123456");
        // `0123456` is `short_owner`'s short id.
        assert_eq!(short_owner.short_id(), "0123456");
        let prefix_a = TaskId::from_string("0123456AAAAAAAAAAAAAAAAAAAA");
        let prefix_b = TaskId::from_string("0123456BBBBBBBBBBBBBBBBBBBB");
        let ids = vec![short_owner.clone(), prefix_a, prefix_b];

        assert_eq!(
            resolve_short_ref(&ids, "0123456"),
            ResolveResult::Found(short_owner)
        );
    }

    #[test]
    fn unknown_reference_is_not_found() {
        let ids = board();
        assert_eq!(resolve_short_ref(&ids, "zzzzzzz"), ResolveResult::NotFound);
        assert_eq!(resolve_short_ref(&ids, "^zzzzzzz"), ResolveResult::NotFound);
    }

    #[test]
    fn empty_reference_is_not_found() {
        let ids = board();
        assert_eq!(resolve_short_ref(&ids, ""), ResolveResult::NotFound);
        assert_eq!(resolve_short_ref(&ids, "^"), ResolveResult::NotFound);
    }

    // ---- short-id uniqueness: collision detection (doctor) -----------------

    #[test]
    fn no_collisions_on_distinct_short_ids() {
        // The real board siblings share a 7-char *prefix* but have distinct
        // 7-char *suffixes* — so there are no short-id collisions.
        let ids = board();
        assert!(find_short_id_collisions(&ids).is_empty());
    }

    #[test]
    fn detects_a_shared_short_id() {
        // Two ULIDs whose last 7 chars are identical collide on short id.
        let a = TaskId::from_string("01KT6R6HR3KJT6JVNDR0123456");
        let b = TaskId::from_string("01KT6SAMJAJ40XVQ9YJ0123456");
        assert_eq!(a.short_id(), b.short_id());
        let unrelated = TaskId::from_string(CORE);
        let ids = vec![a.clone(), b.clone(), unrelated];

        let collisions = find_short_id_collisions(&ids);
        assert_eq!(collisions.len(), 1, "exactly one colliding short id");
        let (short, members) = &collisions[0];
        assert_eq!(short, "0123456");
        assert_eq!(members.len(), 2);
        assert!(members.contains(&a) && members.contains(&b));
    }

    #[test]
    fn collision_detection_is_case_insensitive() {
        // Short ids are lowercased on derivation, so two ULIDs differing only
        // by suffix case still collide.
        let upper = TaskId::from_string("01KT6R6HR3KJT6JVNDRABCDEFG");
        let lower = TaskId::from_string("01KT6SAMJAJ40XVQ9Yjabcdefg");
        assert_eq!(upper.short_id(), lower.short_id());
        let collisions = find_short_id_collisions(&[upper, lower]);
        assert_eq!(collisions.len(), 1);
    }

    // ---- short-id uniqueness: minting (create) -----------------------------

    #[test]
    fn mint_accepts_first_unique_candidate() {
        // First candidate's short id is unique → returned unchanged, mint
        // called exactly once.
        let existing: HashSet<String> = HashSet::new();
        let mut calls = 0;
        let minted = mint_unique_short_id(&existing, || {
            calls += 1;
            "01KT6R6HR3KJT6JVNDRAJV8V4T".to_string()
        });
        assert_eq!(minted, "01KT6R6HR3KJT6JVNDRAJV8V4T");
        assert_eq!(calls, 1, "a unique first candidate must not retry");
    }

    #[test]
    fn mint_retries_past_a_colliding_candidate() {
        // The forced-collision scenario: the board already holds short id
        // `0123456`, the first minted candidate collides with it, and the
        // second is unique. The retry loop must skip the colliding ULID and
        // return the unique one.
        let mut existing = HashSet::new();
        existing.insert("0123456".to_string());

        let candidates = ["01KT6R6HR3KJT6JVNDR0123456", "01KT6SAMJAJ40XVQ9YJABCDEFG"];
        let mut idx = 0;
        let minted = mint_unique_short_id(&existing, || {
            let c = candidates[idx].to_string();
            idx += 1;
            c
        });

        assert_eq!(
            minted, candidates[1],
            "must skip the colliding candidate and mint the unique one"
        );
        assert_eq!(short_id(&minted), "abcdefg");
        assert!(!existing.contains(&short_id(&minted)));
    }
}
