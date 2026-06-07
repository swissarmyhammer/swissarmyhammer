//! Bounded top-k ranking by cosine similarity.
//!
//! The single, reusable ranked-retrieval primitive for the workspace. Both this
//! crate's [`semantic_search`](crate::semantic) and `swissarmyhammer-code-context`'s
//! duplicate/similar probes rank `(id, embedding)` candidates against a query and
//! keep the highest-scoring few. They used to each open-code "score every
//! candidate → sort the whole list → truncate"; the duplicate-detection variant
//! additionally cloned every above-threshold candidate's full text *before*
//! truncating, so a hot query against a large corpus transiently materialized a
//! huge fraction of it for a result that keeps a handful.
//!
//! [`top_k_by_cosine`] replaces that with a bounded min-heap of size `limit`:
//! memory stays `O(limit)` regardless of corpus size, and — because it returns
//! only `(id, score)` — callers clone heavy payloads (chunk text, etc.) for the
//! kept candidates alone. The candidate pool is bounded, never the whole corpus.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use model_embedding::cosine_similarity;

/// One ranked candidate: its id and cosine score against the query.
#[derive(Debug, Clone, PartialEq)]
pub struct Ranked<Id> {
    /// The candidate's caller-supplied identity (an id, an index, a reference).
    pub id: Id,
    /// Cosine similarity to the query, in `[-1.0, 1.0]`.
    pub score: f32,
}

/// The outcome of a bounded top-k ranking.
#[derive(Debug, Clone, PartialEq)]
pub struct RankedTopK<Id> {
    /// The highest-scoring candidates at or above `min_similarity`, sorted by
    /// score descending, length `≤ limit`.
    pub ranked: Vec<Ranked<Id>>,
    /// How many candidates met `min_similarity` in total (`≥ ranked.len()`). Lets
    /// a caller report whether the result was truncated by `limit`
    /// (`considered > limit`) without retaining the dropped candidates.
    pub considered: usize,
}

/// Internal min-heap entry: ordered so the SMALLEST score compares *greatest*,
/// turning [`BinaryHeap`] (a max-heap) into a min-heap whose peek is the weakest
/// kept candidate — the one to evict when a stronger candidate arrives.
struct MinScore<Id> {
    score: f32,
    id: Id,
}

impl<Id> PartialEq for MinScore<Id> {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score
    }
}

impl<Id> Eq for MinScore<Id> {}

impl<Id> PartialOrd for MinScore<Id> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<Id> Ord for MinScore<Id> {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reversed: a smaller score is "greater", so it sits at the heap's top
        // and is the first popped once the heap is at capacity. `total_cmp`
        // gives a total order over f32 (NaN included) so the heap never panics.
        other.score.total_cmp(&self.score)
    }
}

/// Rank `candidates` against `query` by cosine similarity, keeping at most
/// `limit` highest-scoring candidates at or above `min_similarity`.
///
/// Uses a bounded min-heap, so peak memory is `O(limit)` no matter how large the
/// candidate set is. The result's `ranked` is sorted by score descending;
/// `considered` is the total number that met `min_similarity` (for truncation
/// reporting). Only `(id, score)` is retained — clone any heavy payload for the
/// returned ids alone.
///
/// Pass `f32::NEG_INFINITY` as `min_similarity` to keep every candidate (rank the
/// whole set down to `limit` with no threshold).
pub fn top_k_by_cosine<'e, I, Id>(
    query: &[f32],
    candidates: I,
    min_similarity: f32,
    limit: usize,
) -> RankedTopK<Id>
where
    I: IntoIterator<Item = (Id, &'e [f32])>,
{
    let mut considered = 0usize;
    let mut heap: BinaryHeap<MinScore<Id>> = BinaryHeap::new();

    for (id, embedding) in candidates {
        let score = cosine_similarity(query, embedding);
        if score < min_similarity {
            continue;
        }
        considered += 1;
        if limit == 0 {
            continue;
        }
        if heap.len() < limit {
            heap.push(MinScore { score, id });
        } else if let Some(weakest) = heap.peek() {
            // Heap is full: replace the weakest kept candidate only if this one
            // is strictly stronger.
            if score > weakest.score {
                heap.pop();
                heap.push(MinScore { score, id });
            }
        }
    }

    let mut ranked: Vec<Ranked<Id>> = heap
        .into_iter()
        .map(|m| Ranked {
            id: m.id,
            score: m.score,
        })
        .collect();
    ranked.sort_by(|a, b| b.score.total_cmp(&a.score));

    RankedTopK { ranked, considered }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A few orthogonal-ish embeddings keyed by name.
    fn corpus() -> Vec<(&'static str, Vec<f32>)> {
        vec![
            ("a", vec![1.0, 0.0, 0.0]),
            ("b", vec![0.9, 0.1, 0.0]),
            ("c", vec![0.0, 1.0, 0.0]),
            ("d", vec![0.0, 0.0, 1.0]),
        ]
    }

    fn rank(query: &[f32], min: f32, limit: usize) -> RankedTopK<&'static str> {
        let c = corpus();
        top_k_by_cosine(
            query,
            c.iter().map(|(id, e)| (*id, e.as_slice())),
            min,
            limit,
        )
    }

    #[test]
    fn keeps_only_the_top_limit_sorted_descending() {
        let result = rank(&[1.0, 0.0, 0.0], f32::NEG_INFINITY, 2);
        assert_eq!(result.ranked.len(), 2, "limit caps the kept set");
        assert_eq!(result.ranked[0].id, "a", "closest first");
        assert_eq!(result.ranked[1].id, "b");
        assert!(
            result.ranked[0].score >= result.ranked[1].score,
            "sorted descending"
        );
        assert_eq!(
            result.considered, 4,
            "every candidate met the -inf threshold"
        );
    }

    #[test]
    fn min_similarity_filters_and_considered_counts_only_passers() {
        // Only `a` (1.0) and `b` (~0.994) clear 0.5 for this query.
        let result = rank(&[1.0, 0.0, 0.0], 0.5, 10);
        assert_eq!(
            result.considered, 2,
            "only the two above 0.5 are considered"
        );
        let ids: Vec<&str> = result.ranked.iter().map(|r| r.id).collect();
        assert_eq!(ids, vec!["a", "b"]);
    }

    #[test]
    fn considered_exceeds_limit_when_truncated() {
        // 4 pass the -inf threshold but limit is 1 → considered=4 > limit=1.
        let result = rank(&[1.0, 0.0, 0.0], f32::NEG_INFINITY, 1);
        assert_eq!(result.ranked.len(), 1);
        assert_eq!(result.ranked[0].id, "a");
        assert_eq!(
            result.considered, 4,
            "considered counts all passers, not just kept"
        );
    }

    #[test]
    fn limit_zero_keeps_nothing_but_still_counts() {
        let result = rank(&[1.0, 0.0, 0.0], f32::NEG_INFINITY, 0);
        assert!(result.ranked.is_empty());
        assert_eq!(result.considered, 4);
    }

    #[test]
    fn empty_candidates_yields_empty() {
        let result: RankedTopK<&str> = top_k_by_cosine(&[1.0, 0.0], std::iter::empty(), 0.0, 5);
        assert!(result.ranked.is_empty());
        assert_eq!(result.considered, 0);
    }

    #[test]
    fn ties_keep_limit_many_without_panicking() {
        // All identical embeddings → identical scores; the bounded heap must keep
        // exactly `limit` of them and stay total-ordered (no NaN panic).
        let c: Vec<(usize, Vec<f32>)> = (0..10).map(|i| (i, vec![1.0, 0.0])).collect();
        let result = top_k_by_cosine(
            &[1.0, 0.0],
            c.iter().map(|(id, e)| (*id, e.as_slice())),
            f32::NEG_INFINITY,
            3,
        );
        assert_eq!(result.ranked.len(), 3);
        assert_eq!(result.considered, 10);
    }
}
