//! Generic, in-memory document ranker.
//!
//! `swissarmyhammer-search` is a true leaf crate: it depends only on `serde`,
//! `convert_case`, and `unicode-segmentation`. It is consumed by both
//! `swissarmyhammer-code-context` (chunk search) and `swissarmyhammer-kanban`
//! (task search), so it deliberately avoids any workspace-sibling dependency,
//! tokio, or model crates.
//!
//! This module declares the public document/query/result types. Scoring (BM25,
//! trigram-Dice, cosine) is implemented separately on top of these types and the
//! [`tokenize`] primitives.

pub mod cosine;
pub mod score;
pub mod tokenize;

pub use cosine::{cosine_similarity, deserialize_embedding, serialize_embedding};
pub use score::{bm25_score, rrf_fuse, trigram_dice, Bm25Corpus, RRF_K};

use std::collections::{HashMap, HashSet};

/// The fully-tokenized form of a document, computed once and reused.
///
/// `search()` tokenizes every field exactly once and stashes the result here so
/// the corpus pass and the scoring pass never re-tokenize.
struct DocTokens {
    /// Per-field `(field_weight, tokens)`, parallel to the doc's `fields`.
    fields: Vec<(f32, Vec<String>)>,
    /// Total unweighted token count across all fields (`|D|`).
    len: usize,
}

impl DocTokens {
    /// Tokenize every field of `doc` once.
    fn from_doc(doc: &Doc) -> Self {
        let fields: Vec<(f32, Vec<String>)> = doc
            .fields()
            .iter()
            .map(|f| (f.weight(), tokenize::tokenize(f.text())))
            .collect();
        let len = fields.iter().map(|(_, t)| t.len()).sum();
        Self { fields, len }
    }

    /// The set of query terms (from `query_set`) that occur in any field.
    fn query_terms_present<'a>(&self, query_set: &HashSet<&'a str>) -> HashSet<&'a str> {
        let mut present = HashSet::new();
        for (_, tokens) in &self.fields {
            for token in tokens {
                if let Some(&term) = query_set.get(token.as_str()) {
                    present.insert(term);
                }
            }
        }
        present
    }

    /// Weighted term frequency for each query term: `Σ field_weight` over each
    /// occurrence of the term across all fields.
    fn weighted_tf<'a>(&self, query_set: &HashSet<&'a str>) -> HashMap<&'a str, f32> {
        let mut tf: HashMap<&str, f32> = HashMap::new();
        for (weight, tokens) in &self.fields {
            for token in tokens {
                if let Some(&term) = query_set.get(token.as_str()) {
                    *tf.entry(term).or_insert(0.0) += weight;
                }
            }
        }
        tf
    }
}

/// One scoring signal: a raw-value extractor plus its fusion weight.
///
/// `search()` builds a small table of these for the signals that are *present*
/// for a given query, then fuses them uniformly — avoiding three copy-pasted
/// branches for BM25 / trigram / cosine.
struct SignalSpec {
    /// Per-signal fusion weight from [`SignalWeights`].
    weight: f32,
    /// Extract this signal's raw value from a per-doc [`Signals`] record.
    extract: fn(&Signals) -> f32,
}

/// Rank documents over an in-memory corpus and return the top hits.
///
/// Runs two passes over `docs` (tokenizing each field exactly once):
/// 1. a corpus pass building a [`Bm25Corpus`] over the query terms, and
/// 2. a per-doc scoring pass producing raw [`Signals`] (`bm25`, `trigram`,
///    `cosine`).
///
/// The present signals are then fused with Reciprocal Rank Fusion (each signal
/// contributes a stable-sorted ranked list weighted by [`Query::weights`]) and
/// the fused score is normalized to `[0, 1]` by dividing by the maximum
/// achievable for the present signals. A signal with no data — empty query
/// text, no query embedding, or no document embeddings — is simply absent from
/// fusion rather than zero-filled (graceful degradation).
///
/// # Parameters
/// - `docs`: the in-memory corpus to search.
/// - `q`: the query (text, optional embedding, signal weights, `top_k`,
///   optional `min_score` floor on the normalized score).
///
/// # Returns
/// Up to `q.top_k` [`Hit`]s sorted by normalized score descending (stable), each
/// carrying its raw per-signal [`Signals`]; hits below `q.min_score` (when set)
/// are dropped. An empty corpus, or a query with no present signals, yields an
/// empty `Vec`.
pub fn search(docs: &[Doc], q: &Query) -> Vec<Hit> {
    let query_tokens = tokenize::tokenize(q.text());
    let query_set: HashSet<&str> = query_tokens.iter().map(String::as_str).collect();
    let doc_tokens: Vec<DocTokens> = docs.iter().map(DocTokens::from_doc).collect();

    // Determine which signals carry data for this query.
    let bm25_present = !query_tokens.is_empty();
    let trigram_present = !tokenize::char_trigrams(q.text()).is_empty();
    let cosine_present = q.embedding().is_some() && docs.iter().any(|d| d.embedding().is_some());

    // Pass 1: corpus statistics over the query terms.
    let corpus = Bm25Corpus::build(
        &query_tokens,
        doc_tokens
            .iter()
            .map(|dt| (dt.len, dt.query_terms_present(&query_set))),
    );

    // Pass 2: raw per-doc signals.
    let signals: Vec<Signals> = docs
        .iter()
        .zip(&doc_tokens)
        .map(|(doc, dt)| score_doc(doc, dt, q, &corpus, &query_tokens, &query_set))
        .collect();

    // Build the present-signal table.
    let mut specs: Vec<SignalSpec> = Vec::new();
    if bm25_present {
        specs.push(SignalSpec {
            weight: q.weights().bm25(),
            extract: |s| s.bm25,
        });
    }
    if trigram_present {
        specs.push(SignalSpec {
            weight: q.weights().trigram(),
            extract: |s| s.trigram,
        });
    }
    if cosine_present {
        specs.push(SignalSpec {
            weight: q.weights().cosine(),
            extract: |s| s.cosine,
        });
    }

    fuse_and_rank(docs, &signals, &specs, q)
}

/// Compute the raw [`Signals`] for one document.
///
/// The cosine value is `0.0` unless both the query and the document carry an
/// embedding. When the cosine signal is absent from fusion overall, this raw
/// value is simply never used as a ranked list.
fn score_doc(
    doc: &Doc,
    dt: &DocTokens,
    q: &Query,
    corpus: &Bm25Corpus,
    query_tokens: &[String],
    query_set: &HashSet<&str>,
) -> Signals {
    let weighted_tf = dt.weighted_tf(query_set);
    let bm25 = bm25_score(corpus, &weighted_tf, dt.len, query_tokens);
    let trigram = doc
        .fields()
        .iter()
        .map(|f| f.weight() * trigram_dice(q.text(), f.text()))
        .sum();
    let cosine = match (q.embedding(), doc.embedding()) {
        (Some(qe), Some(de)) => cosine_similarity(qe, de),
        _ => 0.0,
    };
    Signals {
        bm25,
        trigram,
        cosine,
    }
}

/// Fuse the present signals with RRF, normalize, sort, filter, and truncate.
fn fuse_and_rank(docs: &[Doc], signals: &[Signals], specs: &[SignalSpec], q: &Query) -> Vec<Hit> {
    // A stable-sorted ranked list of doc indices for each present signal.
    let ranked_lists: Vec<Vec<usize>> = specs
        .iter()
        .map(|spec| ranked_indices(signals, spec.extract))
        .collect();
    let list_refs: Vec<&[usize]> = ranked_lists.iter().map(Vec::as_slice).collect();
    let weights: Vec<f32> = specs.iter().map(|s| s.weight).collect();

    let fused = rrf_fuse(&list_refs, &weights, RRF_K);

    // Max achievable fused score: every present signal at rank 0.
    let max: f32 = weights.iter().map(|w| w / RRF_K).sum();

    let mut hits: Vec<Hit> = (0..docs.len())
        .map(|i| {
            let raw = fused.get(&i).copied().unwrap_or(0.0);
            let score = if max == 0.0 { 0.0 } else { raw / max };
            Hit {
                id: docs[i].id().to_string(),
                score,
                signals: signals[i],
            }
        })
        .collect();

    // Stable sort by score descending.
    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    if let Some(floor) = q.min_score() {
        hits.retain(|h| h.score >= floor);
    }
    hits.truncate(q.top_k());
    hits
}

/// Stable-sorted doc indices, descending by the given signal value.
///
/// Ties preserve doc-index order (the iterator starts in index order and the
/// sort is stable), so fused rankings are deterministic.
fn ranked_indices(signals: &[Signals], extract: fn(&Signals) -> f32) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..signals.len()).collect();
    idx.sort_by(|&a, &b| {
        extract(&signals[b])
            .partial_cmp(&extract(&signals[a]))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    idx
}

/// A single weighted text field of a [`Doc`].
///
/// A document is a bag of fields, each carrying its own `weight` so that, for
/// example, a title can be scored more heavily than a body.
///
/// Fields are private; construct with [`Field::new`] and read via
/// [`Field::weight`] / [`Field::text`]. Keeping them private leaves room to add
/// validation or change the internal representation without breaking callers.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    /// Relative importance of this field when scoring; higher counts for more.
    weight: f32,
    /// The raw text of the field. Tokenization happens at scoring time.
    text: String,
}

impl Field {
    /// Build a field from its `weight` and `text`.
    pub fn new(weight: f32, text: impl Into<String>) -> Self {
        Self {
            weight,
            text: text.into(),
        }
    }

    /// Relative importance of this field when scoring.
    pub fn weight(&self) -> f32 {
        self.weight
    }

    /// The raw text of the field.
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// A document in the in-memory corpus.
///
/// Fields are private; construct with [`Doc::new`] and read via [`Doc::id`],
/// [`Doc::fields`], and [`Doc::embedding`].
#[derive(Debug, Clone, PartialEq)]
pub struct Doc {
    /// Stable identifier returned in a [`Hit`].
    id: String,
    /// The weighted text fields that make up the document.
    fields: Vec<Field>,
    /// Optional dense embedding used for the cosine signal.
    embedding: Option<Vec<f32>>,
}

impl Doc {
    /// Build a document from its `id`, weighted `fields`, and optional
    /// `embedding`.
    pub fn new(id: impl Into<String>, fields: Vec<Field>, embedding: Option<Vec<f32>>) -> Self {
        Self {
            id: id.into(),
            fields,
            embedding,
        }
    }

    /// Stable identifier returned in a [`Hit`].
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The weighted text fields that make up the document.
    pub fn fields(&self) -> &[Field] {
        &self.fields
    }

    /// Optional dense embedding used for the cosine signal.
    pub fn embedding(&self) -> Option<&[f32]> {
        self.embedding.as_deref()
    }
}

/// Relative weights applied to each scoring signal when combining them.
///
/// The [`Default`] is all `1.0`, i.e. every signal contributes equally.
/// Fields are private; construct with [`SignalWeights::new`] or [`Default`] and
/// read via the [`SignalWeights::bm25`] / [`SignalWeights::trigram`] /
/// [`SignalWeights::cosine`] getters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SignalWeights {
    /// Weight applied to the BM25 lexical signal.
    w_bm25: f32,
    /// Weight applied to the character-trigram (fuzzy) signal.
    w_trigram: f32,
    /// Weight applied to the embedding cosine-similarity signal.
    w_cosine: f32,
}

impl SignalWeights {
    /// Build weights for the BM25, trigram, and cosine signals.
    pub fn new(w_bm25: f32, w_trigram: f32, w_cosine: f32) -> Self {
        Self {
            w_bm25,
            w_trigram,
            w_cosine,
        }
    }

    /// Weight applied to the BM25 lexical signal.
    pub fn bm25(&self) -> f32 {
        self.w_bm25
    }

    /// Weight applied to the character-trigram (fuzzy) signal.
    pub fn trigram(&self) -> f32 {
        self.w_trigram
    }

    /// Weight applied to the embedding cosine-similarity signal.
    pub fn cosine(&self) -> f32 {
        self.w_cosine
    }
}

impl Default for SignalWeights {
    fn default() -> Self {
        Self {
            w_bm25: 1.0,
            w_trigram: 1.0,
            w_cosine: 1.0,
        }
    }
}

/// The default number of [`Hit`]s a [`Query`] returns when `top_k` is not set.
const DEFAULT_TOP_K: usize = 10;

/// A search request against a corpus of [`Doc`]s.
///
/// Fields are private; construct with [`Query::new`] and refine with the
/// [`Query::with_embedding`] / [`Query::with_weights`] / [`Query::with_top_k`] /
/// [`Query::with_min_score`] builders. Read via the [`Query::text`],
/// [`Query::embedding`], [`Query::weights`], [`Query::top_k`], and
/// [`Query::min_score`] getters. Keeping the fields private leaves room to add
/// validation or change the internal representation without breaking callers.
#[derive(Debug, Clone)]
pub struct Query {
    /// The raw query text. Tokenized the same way as document fields.
    text: String,
    /// Optional dense embedding for the cosine signal.
    embedding: Option<Vec<f32>>,
    /// Per-signal weights used to combine signals into a final score.
    weights: SignalWeights,
    /// Maximum number of [`Hit`]s to return.
    top_k: usize,
    /// Optional minimum combined score; hits below it are dropped.
    min_score: Option<f32>,
}

impl Query {
    /// Build a query from its `text`, defaulting to no embedding, equal signal
    /// weights, a `top_k` of [`DEFAULT_TOP_K`], and no `min_score` floor.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            embedding: None,
            weights: SignalWeights::default(),
            top_k: DEFAULT_TOP_K,
            min_score: None,
        }
    }

    /// Attach a dense query embedding for the cosine signal.
    pub fn with_embedding(mut self, embedding: Vec<f32>) -> Self {
        self.embedding = Some(embedding);
        self
    }

    /// Override the per-signal fusion weights.
    pub fn with_weights(mut self, weights: SignalWeights) -> Self {
        self.weights = weights;
        self
    }

    /// Override the maximum number of [`Hit`]s to return.
    pub fn with_top_k(mut self, top_k: usize) -> Self {
        self.top_k = top_k;
        self
    }

    /// Set a minimum normalized-score floor; hits below it are dropped.
    pub fn with_min_score(mut self, min_score: f32) -> Self {
        self.min_score = Some(min_score);
        self
    }

    /// The raw query text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Optional dense embedding for the cosine signal.
    pub fn embedding(&self) -> Option<&[f32]> {
        self.embedding.as_deref()
    }

    /// Per-signal weights used to combine signals into a final score.
    pub fn weights(&self) -> SignalWeights {
        self.weights
    }

    /// Maximum number of [`Hit`]s to return.
    pub fn top_k(&self) -> usize {
        self.top_k
    }

    /// Optional minimum combined score; hits below it are dropped.
    pub fn min_score(&self) -> Option<f32> {
        self.min_score
    }
}

/// The per-signal raw scores that contributed to a [`Hit`].
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct Signals {
    /// BM25 lexical score.
    pub bm25: f32,
    /// Character-trigram (fuzzy) score.
    pub trigram: f32,
    /// Embedding cosine-similarity score.
    pub cosine: f32,
}

/// A scored search result for a single [`Doc`].
#[derive(Debug, Clone, serde::Serialize)]
pub struct Hit {
    /// The id of the matched document.
    pub id: String,
    /// The combined, weighted score.
    pub score: f32,
    /// The individual signal scores that produced [`Hit::score`].
    pub signals: Signals,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Convenience: a [`Field`] from a weight and text.
    fn field(weight: f32, text: &str) -> Field {
        Field::new(weight, text)
    }

    fn default_query(text: &str) -> Query {
        Query::new(text)
    }

    #[test]
    fn query_builders_and_getters_round_trip() {
        let q = Query::new("parse config")
            .with_embedding(vec![1.0, 0.0])
            .with_weights(SignalWeights::new(2.0, 0.5, 1.5))
            .with_top_k(5)
            .with_min_score(0.25);
        assert_eq!(q.text(), "parse config");
        assert_eq!(q.embedding(), Some(&[1.0, 0.0][..]));
        assert_eq!(q.weights().bm25(), 2.0);
        assert_eq!(q.top_k(), 5);
        assert_eq!(q.min_score(), Some(0.25));

        // Defaults: no embedding, equal weights, DEFAULT_TOP_K, no floor.
        let d = Query::new("x");
        assert_eq!(d.embedding(), None);
        assert_eq!(d.top_k(), DEFAULT_TOP_K);
        assert_eq!(d.min_score(), None);
        assert_eq!(d.weights().bm25(), 1.0);
    }

    #[test]
    fn constructors_and_getters_round_trip() {
        let f = Field::new(3.0, "parse config");
        assert_eq!(f.weight(), 3.0);
        assert_eq!(f.text(), "parse config");

        let w = SignalWeights::new(2.0, 0.5, 1.5);
        assert_eq!(w.bm25(), 2.0);
        assert_eq!(w.trigram(), 0.5);
        assert_eq!(w.cosine(), 1.5);

        let doc = Doc::new("d1", vec![f.clone()], Some(vec![1.0, 0.0]));
        assert_eq!(doc.id(), "d1");
        assert_eq!(doc.fields().len(), 1);
        assert_eq!(doc.embedding(), Some(&[1.0, 0.0][..]));
    }

    #[test]
    fn doc_partial_eq_allows_comparison() {
        let a = Doc::new("d", vec![field(1.0, "config")], None);
        let b = Doc::new("d", vec![field(1.0, "config")], None);
        let c = Doc::new("d", vec![field(1.0, "other")], None);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn strong_high_weight_lexical_beats_mediocre() {
        let docs = vec![
            // "parse config" appears in a weight-5 title field.
            Doc::new(
                "strong",
                vec![
                    field(5.0, "parse config"),
                    field(1.0, "loads settings from disk"),
                ],
                None,
            ),
            // "config" only in a low-weight body, amid noise.
            Doc::new(
                "mediocre",
                vec![field(1.0, "the config value is read lazily later on")],
                None,
            ),
        ];
        let hits = search(&docs, &default_query("parse config"));
        assert_eq!(hits.first().unwrap().id, "strong");
    }

    #[test]
    fn cosine_absent_when_no_doc_has_embedding() {
        let docs = vec![
            Doc::new("a", vec![field(1.0, "parse config")], None),
            Doc::new("b", vec![field(1.0, "unrelated text here")], None),
        ];
        // Query has an embedding, but no doc does -> cosine signal absent.
        let q = default_query("parse config").with_embedding(vec![1.0, 0.0, 0.0]);
        let hits = search(&docs, &q);
        // Results still come back, ranked by the lexical/trigram signals.
        assert!(!hits.is_empty());
        assert_eq!(hits[0].id, "a");
        // Cosine signal is raw 0.0 for every hit (no doc embedding to compare).
        assert!(hits.iter().all(|h| h.signals.cosine == 0.0));
    }

    #[test]
    fn empty_query_text_still_returns_via_other_signals() {
        // Empty query text -> no BM25, no trigrams; only cosine present.
        let docs = vec![
            Doc::new("near", vec![field(1.0, "anything")], Some(vec![1.0, 0.0])),
            Doc::new("far", vec![field(1.0, "anything")], Some(vec![0.0, 1.0])),
        ];
        let q = default_query("").with_embedding(vec![1.0, 0.0]);
        let hits = search(&docs, &q);
        assert_eq!(hits[0].id, "near");
    }

    #[test]
    fn rank_zero_everywhere_normalizes_to_one() {
        // One doc dominates every present signal -> normalized score == 1.0.
        let docs = vec![
            Doc::new(
                "best",
                vec![field(3.0, "parse config")],
                Some(vec![1.0, 0.0]),
            ),
            Doc::new(
                "rest",
                vec![field(1.0, "totally different words")],
                Some(vec![0.0, 1.0]),
            ),
        ];
        let q = default_query("parse config").with_embedding(vec![1.0, 0.0]);
        let hits = search(&docs, &q);
        assert_eq!(hits[0].id, "best");
        assert!(
            (hits[0].score - 1.0).abs() < 1e-6,
            "score was {}",
            hits[0].score
        );
    }

    #[test]
    fn min_score_filters_on_normalized_score() {
        let docs = vec![
            Doc::new(
                "best",
                vec![field(3.0, "parse config")],
                Some(vec![1.0, 0.0]),
            ),
            Doc::new(
                "rest",
                vec![field(1.0, "totally different words")],
                Some(vec![0.0, 1.0]),
            ),
        ];
        // "best" is rank-0 everywhere -> 1.0; "rest" is strictly below.
        let q = default_query("parse config")
            .with_embedding(vec![1.0, 0.0])
            .with_min_score(0.999);
        let hits = search(&docs, &q);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "best");
    }

    #[test]
    fn normalization_preserves_ordering() {
        let docs = vec![
            Doc::new("a", vec![field(3.0, "parse config file")], None),
            Doc::new("b", vec![field(1.0, "config")], None),
            Doc::new("c", vec![field(1.0, "nothing relevant at all")], None),
        ];
        let hits = search(&docs, &default_query("parse config"));
        let ids: Vec<&str> = hits.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids[0], "a");
        // Scores are monotone non-increasing.
        for pair in hits.windows(2) {
            assert!(pair[0].score >= pair[1].score);
        }
    }

    #[test]
    fn top_k_limits_results() {
        let docs = vec![
            Doc::new("a", vec![field(1.0, "config")], None),
            Doc::new("b", vec![field(1.0, "config")], None),
            Doc::new("c", vec![field(1.0, "config")], None),
        ];
        let q = default_query("config").with_top_k(2);
        assert_eq!(search(&docs, &q).len(), 2);
    }
}
