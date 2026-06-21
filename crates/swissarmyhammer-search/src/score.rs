//! Scoring primitives: weighted-tf Okapi BM25, trigram-Dice, and RRF fusion.
//!
//! These are pure, allocation-light functions over the [`crate::tokenize`]
//! primitives and plain slices. They carry no DB, embedding, or async state and
//! are designed so that the top-level `search()` can make one corpus pass
//! (building a [`Bm25Corpus`]) followed by a per-document scoring pass.

use std::collections::{HashMap, HashSet};

/// BM25 term-frequency saturation parameter `k1`.
const K1: f32 = 1.2;
/// BM25 length-normalization parameter `b`.
const B: f32 = 0.75;
/// Default Reciprocal Rank Fusion constant `k`.
///
/// Larger values flatten the contribution difference between adjacent ranks.
/// `60.0` is the value from the original RRF paper.
pub const RRF_K: f32 = 60.0;

/// Precomputed corpus statistics for a single BM25 query.
///
/// Built once per query in a single corpus pass (see [`Bm25Corpus::build`]) and
/// then consumed by [`bm25_score`] for every document. It captures only the
/// statistics BM25 needs: the document frequency of each query term, the total
/// document count, and the mean *unweighted* token count.
#[derive(Debug, Clone)]
pub struct Bm25Corpus {
    /// `df(t)`: number of documents containing query term `t` in any field.
    df: HashMap<String, usize>,
    /// `N`: total number of documents in the corpus.
    n: usize,
    /// `avgdl`: mean unweighted token count across all documents.
    avgdl: f32,
}

impl Bm25Corpus {
    /// Build corpus statistics in a single pass over the documents.
    ///
    /// # Parameters
    /// - `query_tokens`: the tokenized query. Document frequency is tracked only
    ///   for these terms (duplicates are tolerated; each distinct term is
    ///   counted once).
    /// - `docs`: an iterator yielding one item per document: a tuple of
    ///   `(unweighted_token_count, query_terms_present)`, where
    ///   `query_terms_present` is the set of query terms that occur in *any*
    ///   field of that document. `unweighted_token_count` is the document's
    ///   total token count across all fields (no field weighting).
    ///
    /// # Returns
    /// A [`Bm25Corpus`] with `df` populated for every query term (defaulting to
    /// `0` for terms no document contains), `n` set to the number of documents
    /// iterated, and `avgdl` set to the mean unweighted token count (`0.0` for
    /// an empty corpus).
    pub fn build<'a>(
        query_tokens: &[String],
        docs: impl Iterator<Item = (usize, HashSet<&'a str>)>,
    ) -> Self {
        let mut df: HashMap<String, usize> = query_tokens.iter().map(|t| (t.clone(), 0)).collect();
        let mut n = 0usize;
        let mut total_len = 0usize;
        for (token_count, present) in docs {
            n += 1;
            total_len += token_count;
            for term in present {
                if let Some(count) = df.get_mut(term) {
                    *count += 1;
                }
            }
        }
        let avgdl = if n == 0 {
            0.0
        } else {
            total_len as f32 / n as f32
        };
        Self { df, n, avgdl }
    }
}

/// Score a single document with weighted-tf Okapi BM25 ("BM25F-lite").
///
/// Implements `Σ_t IDF(t)·tf·(k1+1) / (tf + k1·(1 − b + b·|D|/avgdl))` over the
/// distinct query terms, where:
/// - `IDF(t) = ln(1 + (N − df(t) + 0.5) / (df(t) + 0.5))`,
/// - `tf` is the *weighted* term frequency for `t` (sum of field weights over
///   each occurrence of `t` across all fields), taken from `weighted_tf` and
///   `0.0` when absent,
/// - `|D| = doc_len` is the document's *unweighted* total token count.
///
/// # Parameters
/// - `corpus`: precomputed statistics from [`Bm25Corpus::build`].
/// - `weighted_tf`: per-term weighted term frequency for this document.
/// - `doc_len`: the document's unweighted token count (`|D|`).
/// - `query_tokens`: the tokenized query; iterated as a *deduplicated* set so a
///   repeated query term is not double-counted.
///
/// # Returns
/// The BM25 score (always finite; `0.0` when no query term matches or the
/// corpus is empty).
pub fn bm25_score(
    corpus: &Bm25Corpus,
    weighted_tf: &HashMap<&str, f32>,
    doc_len: usize,
    query_tokens: &[String],
) -> f32 {
    // An empty corpus has no meaningful length normalization; score 0.0.
    if corpus.avgdl == 0.0 {
        return 0.0;
    }
    let n = corpus.n as f32;
    let len_norm = K1 * (1.0 - B + B * doc_len as f32 / corpus.avgdl);

    let distinct: HashSet<&str> = query_tokens.iter().map(String::as_str).collect();
    distinct
        .into_iter()
        .map(|term| {
            let tf = weighted_tf.get(term).copied().unwrap_or(0.0);
            if tf == 0.0 {
                return 0.0;
            }
            let df = corpus.df.get(term).copied().unwrap_or(0) as f32;
            let idf = (1.0 + (n - df + 0.5) / (df + 0.5)).ln();
            idf * tf * (K1 + 1.0) / (tf + len_norm)
        })
        .sum()
}

/// Sørensen-Dice coefficient over the *sets* of character trigrams of two strings.
///
/// Computes `2·|A∩B| / (|A|+|B|)` where `A` and `B` are the deduplicated sets of
/// length-3 character windows (via [`crate::tokenize::char_trigrams`]) of each
/// input.
///
/// Each input is first canonicalized through [`crate::tokenize::tokenize`] and
/// re-joined with single spaces before trigramming. This normalizes identifier
/// delimiters so that `camelCase`, `snake_case`, and `kebab-case` spellings of
/// the same words share trigrams — which is what makes the signal a *typo /
/// style* rescue rather than a literal substring match. Without it, `"getUsr"`
/// and `"get_user"` would share only the `get` trigram (Dice 0.2); after
/// canonicalization they become `"get usr"` vs `"get user"` and overlap
/// strongly (Dice > 0.7).
///
/// # Parameters
/// - `query`, `target`: the strings to compare. Order is irrelevant.
///
/// # Returns
/// A similarity in `[0.0, 1.0]`; `1.0` for equal canonical trigram sets, `0.0`
/// when either side yields no trigrams (too short after canonicalization) or the
/// sets are disjoint.
pub fn trigram_dice(query: &str, target: &str) -> f32 {
    let a = canonical_trigram_set(query);
    let b = canonical_trigram_set(target);
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let intersection = a.intersection(&b).count() as f32;
    2.0 * intersection / (a.len() + b.len()) as f32
}

/// Canonicalize `s` (tokenize, re-join with spaces) and return its trigram set.
///
/// This is the single authority for "does this string have trigrams?": the
/// trigram signal in [`crate::search`] both detects presence and scores
/// ([`trigram_dice`]) through this canonical form, so a string with an empty
/// canonical trigram set can never contribute a non-zero trigram score.
///
/// # Parameters
/// - `s`: the string to canonicalize and trigram.
///
/// # Returns
/// The deduplicated set of length-3 character windows of the canonical form.
pub(crate) fn canonical_trigram_set(s: &str) -> HashSet<[char; 3]> {
    let canonical = crate::tokenize::tokenize(s).join(" ");
    crate::tokenize::char_trigrams(&canonical)
        .into_iter()
        .collect()
}

/// Fuse several ranked lists of document indices via Reciprocal Rank Fusion.
///
/// Implements `RRF(d) = Σ_r w_r / (k + rank_r(d))` summed over the lists `r` in
/// which document `d` appears. **Ranks are 0-based: rank `0` is the best (first)
/// position in a list.** A document absent from a list contributes nothing for
/// that list (graceful degradation, no zero-fill).
///
/// # Parameters
/// - `ranked_lists`: one slice of document indices per signal, ordered best (rank
///   0) to worst.
/// - `weights`: the weight `w_r` for each list, positionally aligned with
///   `ranked_lists`. Must be the same length as `ranked_lists`.
/// - `k`: the RRF constant (see [`RRF_K`]).
///
/// # Returns
/// A map from document index to its fused score. Only documents appearing in at
/// least one list are present.
pub fn rrf_fuse(ranked_lists: &[&[usize]], weights: &[f32], k: f32) -> HashMap<usize, f32> {
    let mut fused: HashMap<usize, f32> = HashMap::new();
    for (list, &weight) in ranked_lists.iter().zip(weights) {
        for (rank, &doc) in list.iter().enumerate() {
            *fused.entry(doc).or_insert(0.0) += weight / (k + rank as f32);
        }
    }
    fused
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a corpus from explicit `(unweighted_len, present_terms)` rows.
    fn corpus_from(query: &[String], docs: Vec<(usize, Vec<&str>)>) -> Bm25Corpus {
        let iter = docs
            .into_iter()
            .map(|(len, terms)| (len, terms.into_iter().collect::<HashSet<&str>>()));
        Bm25Corpus::build(query, iter.collect::<Vec<_>>().into_iter())
    }

    /// Reference Okapi BM25 term contribution for hand-comparison.
    fn ref_term(n: f32, df: f32, tf: f32, doc_len: f32, avgdl: f32) -> f32 {
        let idf = (1.0 + (n - df + 0.5) / (df + 0.5)).ln();
        idf * tf * (K1 + 1.0) / (tf + K1 * (1.0 - B + B * doc_len / avgdl))
    }

    #[test]
    fn bm25_single_term_matches_hand_computed() {
        // 3-doc corpus, query "foo". doc lens 4, 2, 6 -> avgdl = 4.0.
        // "foo" appears in docs 0 and 1 -> df = 2, N = 3.
        let query = vec!["foo".to_string()];
        let corpus = corpus_from(
            &query,
            vec![(4, vec!["foo"]), (2, vec!["foo"]), (6, vec![])],
        );
        assert_eq!(corpus.avgdl, 4.0);

        // Score doc 0: unweighted tf 1.0, doc_len 4.
        let tf: HashMap<&str, f32> = [("foo", 1.0)].into_iter().collect();
        let got = bm25_score(&corpus, &tf, 4, &query);
        let want = ref_term(3.0, 2.0, 1.0, 4.0, 4.0);
        assert!((got - want).abs() < 1e-4, "got {got}, want {want}");
    }

    #[test]
    fn bm25_two_term_matches_hand_computed() {
        // Query "foo bar". df(foo)=2, df(bar)=1, N=3, avgdl=4.0.
        let query = vec!["foo".to_string(), "bar".to_string()];
        let corpus = corpus_from(
            &query,
            vec![(4, vec!["foo", "bar"]), (2, vec!["foo"]), (6, vec![])],
        );
        let tf: HashMap<&str, f32> = [("foo", 1.0), ("bar", 1.0)].into_iter().collect();
        let got = bm25_score(&corpus, &tf, 4, &query);
        let want = ref_term(3.0, 2.0, 1.0, 4.0, 4.0) + ref_term(3.0, 1.0, 1.0, 4.0, 4.0);
        assert!((got - want).abs() < 1e-4, "got {got}, want {want}");
    }

    #[test]
    fn bm25_rarer_term_scores_higher() {
        // Same tf/doc_len, but "rare" has df 1 vs "common" df 3.
        let query = vec!["rare".to_string(), "common".to_string()];
        let corpus = corpus_from(
            &query,
            vec![
                (4, vec!["rare", "common"]),
                (4, vec!["common"]),
                (4, vec!["common"]),
            ],
        );
        let tf_rare: HashMap<&str, f32> = [("rare", 1.0)].into_iter().collect();
        let tf_common: HashMap<&str, f32> = [("common", 1.0)].into_iter().collect();
        let rare = bm25_score(&corpus, &tf_rare, 4, &["rare".to_string()]);
        let common = bm25_score(&corpus, &tf_common, 4, &["common".to_string()]);
        assert!(rare > common, "rare {rare} should beat common {common}");
    }

    #[test]
    fn bm25_high_weight_field_scores_higher() {
        // Identical corpus and doc_len; only the weighted tf differs.
        let query = vec!["foo".to_string()];
        let corpus = corpus_from(&query, vec![(4, vec!["foo"]), (4, vec!["foo"])]);
        let high: HashMap<&str, f32> = [("foo", 3.0)].into_iter().collect();
        let low: HashMap<&str, f32> = [("foo", 1.0)].into_iter().collect();
        let high_score = bm25_score(&corpus, &high, 4, &query);
        let low_score = bm25_score(&corpus, &low, 4, &query);
        assert!(
            high_score > low_score,
            "high {high_score} vs low {low_score}"
        );
    }

    #[test]
    fn bm25_repeated_query_term_not_double_counted() {
        let query = vec!["foo".to_string(), "foo".to_string()];
        let corpus = corpus_from(&query, vec![(4, vec!["foo"]), (4, vec![])]);
        let tf: HashMap<&str, f32> = [("foo", 1.0)].into_iter().collect();
        let got = bm25_score(&corpus, &tf, 4, &query);
        let want = ref_term(2.0, 1.0, 1.0, 4.0, 4.0);
        assert!((got - want).abs() < 1e-4, "got {got}, want {want}");
    }

    #[test]
    fn bm25_empty_corpus_is_zero() {
        let query = vec!["foo".to_string()];
        let corpus = corpus_from(&query, vec![]);
        let tf: HashMap<&str, f32> = [("foo", 1.0)].into_iter().collect();
        assert_eq!(bm25_score(&corpus, &tf, 0, &query), 0.0);
    }

    #[test]
    fn trigram_dice_identical_is_one() {
        assert_eq!(trigram_dice("get_user", "get_user"), 1.0);
    }

    #[test]
    fn trigram_dice_typo_rescue_above_threshold() {
        assert!(trigram_dice("getUsr", "get_user") > 0.4);
    }

    #[test]
    fn trigram_dice_disjoint_is_zero() {
        assert_eq!(trigram_dice("abcdef", "uvwxyz"), 0.0);
    }

    #[test]
    fn trigram_dice_no_trigrams_is_zero() {
        assert_eq!(trigram_dice("ab", "get_user"), 0.0);
        assert_eq!(trigram_dice("get_user", ""), 0.0);
    }

    #[test]
    fn rrf_two_lists_beat_one() {
        // doc 0 is rank-0 in lists 0 and 1; doc 1 is rank-0 only in list 2.
        let l0: &[usize] = &[0, 1];
        let l1: &[usize] = &[0, 2];
        let l2: &[usize] = &[1, 0];
        let fused = rrf_fuse(&[l0, l1, l2], &[1.0, 1.0, 1.0], RRF_K);
        assert!(
            fused[&0] > fused[&1],
            "doc0 {} vs doc1 {}",
            fused[&0],
            fused[&1]
        );
    }

    #[test]
    fn rrf_matches_hand_computed() {
        let l0: &[usize] = &[0, 1];
        let l1: &[usize] = &[1, 0];
        let fused = rrf_fuse(&[l0, l1], &[1.0, 1.0], 60.0);
        // doc0: 1/60 + 1/61 ; doc1: 1/61 + 1/60 -> equal.
        let want0 = 1.0 / 60.0 + 1.0 / 61.0;
        assert!((fused[&0] - want0).abs() < 1e-6);
        assert!((fused[&1] - want0).abs() < 1e-6);
    }

    #[test]
    fn rrf_missing_doc_contributes_nothing() {
        let l0: &[usize] = &[0];
        let l1: &[usize] = &[1];
        let fused = rrf_fuse(&[l0, l1], &[1.0, 1.0], 60.0);
        assert!((fused[&0] - 1.0 / 60.0).abs() < 1e-6);
        assert!((fused[&1] - 1.0 / 60.0).abs() < 1e-6);
    }

    #[test]
    fn rrf_weight_effect() {
        let l0: &[usize] = &[0];
        let l1: &[usize] = &[1];
        let fused = rrf_fuse(&[l0, l1], &[2.0, 1.0], 60.0);
        assert!(fused[&0] > fused[&1]);
        assert!((fused[&0] - 2.0 / 60.0).abs() < 1e-6);
    }
}
