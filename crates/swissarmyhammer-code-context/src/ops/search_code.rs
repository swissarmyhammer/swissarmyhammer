//! Hybrid fusion search across stored chunk embeddings.
//!
//! Loads chunk rows from `ts_chunks` (text, `symbol_path`, embedding), builds a
//! [`swissarmyhammer_search::Doc`] per chunk — the short `symbol_path` as a
//! high-weight field, the chunk body as a low-weight field, plus the embedding —
//! and ranks them with [`swissarmyhammer_search::search`], which fuses BM25,
//! character-trigram, and cosine signals into a single normalized score. All
//! fusion logic lives in the leaf `swissarmyhammer-search` crate; this module
//! only adapts the DB rows into `Doc`s and maps the [`Hit`]s back out.

use rusqlite::Connection;
use swissarmyhammer_search::{search, Doc, Field, Hit, Query, SignalWeights};

use crate::error::CodeContextError;

// Re-export the per-signal score breakdown so `SearchCodeMatch::signals` has a
// public type and `swissarmyhammer_code_context::Signals` resolves.
pub use swissarmyhammer_search::Signals;

/// Fusion weight applied to the short, identifier-bearing `symbol_path` field.
///
/// High relative to [`TEXT_FIELD_WEIGHT`] so an exact identifier match in the
/// symbol path drives the BM25/trigram signals even though the much larger body
/// field dilutes them (a big denominator makes the body's Dice naturally tiny).
const SYMBOL_FIELD_WEIGHT: f32 = 5.0;

/// Fusion weight applied to the full chunk-body text field.
const TEXT_FIELD_WEIGHT: f32 = 1.0;

/// A chunk that matched the search query, with its fused score and per-signal
/// breakdown.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchCodeMatch {
    /// Path of the file containing this chunk.
    pub file_path: String,
    /// First line of the chunk (1-indexed).
    pub start_line: u32,
    /// Last line of the chunk (1-indexed).
    pub end_line: u32,
    /// Qualified symbol path for this chunk, if available.
    pub symbol_path: Option<String>,
    /// Full text of the matched chunk.
    pub text: String,
    /// Fused, normalized relevance score in `[0.0, 1.0]`.
    pub score: f32,
    /// The individual signal scores (`bm25`, `trigram`, `cosine`) that produced
    /// [`SearchCodeMatch::score`].
    pub signals: Signals,
}

/// Options for [`search_code`].
#[derive(Debug)]
pub struct SearchCodeOptions {
    /// Maximum number of results to return.
    pub top_k: usize,
    /// Fusion weight for the BM25 lexical signal.
    pub w_bm25: f32,
    /// Fusion weight for the character-trigram (fuzzy) signal.
    pub w_trigram: f32,
    /// Fusion weight for the embedding cosine-similarity signal.
    pub w_cosine: f32,
    /// Optional minimum on the NORMALIZED `[0, 1]` fused score; matches below it
    /// are dropped. `None` keeps every match the corpus yields.
    pub min_fused_score: Option<f32>,
    /// Only search chunks from files with these extensions.
    pub language: Option<Vec<String>>,
    /// Only search chunks matching this file path pattern.
    pub file_pattern: Option<String>,
}

impl Default for SearchCodeOptions {
    fn default() -> Self {
        Self {
            top_k: 10,
            w_bm25: 1.0,
            w_trigram: 1.0,
            w_cosine: 1.0,
            min_fused_score: None,
            language: None,
            file_pattern: None,
        }
    }
}

/// Index-build progress summary returned with a [`SearchCodeResult`].
///
/// Populated when `embedded_files < total_files`, i.e. the embedding pass is
/// still running. Callers use it to inform the user that results may be
/// incomplete and that retrying after a moment will yield more coverage.
///
/// Field semantics intentionally mirror `BlockingStatus::NotReady` so callers
/// see a consistent shape across the two paths.
#[derive(Debug, Clone, serde::Serialize)]
pub struct IndexingProgress {
    /// Number of files whose every chunk has an embedding (i.e. `embedded=1`).
    pub embedded_files: u64,
    /// Total number of tracked files in `indexed_files`.
    pub total_files: u64,
    /// `embedded_files / total_files * 100`, in the range 0.0..=100.0.
    pub embedded_percent: f64,
    /// Human-readable progress message suitable for surfacing in a CLI/MCP
    /// response, e.g. "Embedding still in progress: 3/5 files (60%). ...".
    pub message: String,
}

/// Result of [`search_code`].
#[derive(Debug, serde::Serialize)]
pub struct SearchCodeResult {
    /// Chunks ranked by cosine similarity (descending).
    pub matches: Vec<SearchCodeMatch>,
    /// Total number of chunks with embeddings that were searched.
    pub total_chunks_searched: usize,
    /// Whether the result set was truncated by `top_k`.
    pub truncated: bool,
    /// `Some(...)` when the embedding pass is still running, `None` when
    /// every tracked file is embedded (or there are no tracked files).
    pub progress: Option<IndexingProgress>,
}

/// A chunk loaded from `ts_chunks` together with its embedding.
///
/// Loading the embedding table is expensive (every row carries a multi-hundred-
/// float vector), so this type is the unit of a **reusable corpus**: a caller
/// that runs many comparisons against the same index — e.g. the review engine's
/// probe runner, which would otherwise call [`find_duplicates`] once per changed
/// file and [`search_code`] once per added function — loads the corpus once with
/// [`load_all_embedded_chunks`] and feeds it to [`search_loaded`] /
/// [`find_duplicates_in`](crate::find_duplicates_in) instead of re-materializing
/// the whole table per call.
#[derive(Debug, Clone)]
pub struct LoadedChunk {
    /// Path of the file containing this chunk.
    pub file_path: String,
    /// First line of the chunk (1-indexed).
    pub start_line: u32,
    /// Last line of the chunk (1-indexed).
    pub end_line: u32,
    /// Qualified symbol path for this chunk, if available.
    pub symbol_path: Option<String>,
    /// Full text of the chunk.
    pub text: String,
    /// The chunk's embedding vector.
    pub embedding: Vec<f32>,
}

// Re-export the canonical blob helper from the leaf search crate. Production
// consumers (the live indexer, the CLI doctor) import
// `swissarmyhammer_code_context::serialize_embedding`; keeping the name and
// signature here preserves that public path.
pub use swissarmyhammer_search::serialize_embedding;

use swissarmyhammer_search::deserialize_embedding;

/// Build the [`Doc`] for one chunk, identified by its position in the corpus.
///
/// The `id` is the chunk's index into the corpus slice (as a string) so a [`Hit`]
/// maps back to its [`LoadedChunk`] in O(1) without cloning chunk fields into a
/// side map. The `symbol_path` becomes a high-weight field and the chunk body a
/// low-weight one; the embedding is moved in to drive the cosine signal.
fn chunk_to_doc(index: usize, chunk: &LoadedChunk) -> Doc {
    let symbol = chunk.symbol_path.clone().unwrap_or_default();
    Doc::new(
        index.to_string(),
        vec![
            Field::new(SYMBOL_FIELD_WEIGHT, symbol),
            Field::new(TEXT_FIELD_WEIGHT, chunk.text.clone()),
        ],
        Some(chunk.embedding.clone()),
    )
}

/// Search chunk embeddings by hybrid fusion against a query.
///
/// `query_text` is tokenized for the BM25/trigram signals; `query_embedding`
/// (pre-computed by the caller with the same model that produced the chunk
/// embeddings) drives the cosine signal. Matches are ranked by the normalized
/// fused score; `options` controls the per-signal weights, the `top_k`, and the
/// optional `min_fused_score` floor.
///
/// # Errors
///
/// Returns [`CodeContextError::Database`] on SQLite failures.
pub fn search_code(
    conn: &Connection,
    query_text: &str,
    query_embedding: &[f32],
    options: &SearchCodeOptions,
) -> Result<SearchCodeResult, CodeContextError> {
    let rows = load_embedded_chunks(conn, options)?;
    let refs: Vec<&LoadedChunk> = rows.iter().collect();
    let (matches, total_chunks_searched, truncated) =
        rank_loaded(&refs, query_text, query_embedding, options);
    let progress = compute_indexing_progress(conn)?;

    Ok(SearchCodeResult {
        matches,
        total_chunks_searched,
        truncated,
        progress,
    })
}

/// Rank a pre-loaded corpus against the query with hybrid fusion.
///
/// The corpus-based counterpart of [`search_code`]: it runs the identical
/// ranking core ([`rank_loaded`]) but against chunks the caller already loaded
/// (via [`load_all_embedded_chunks`]) rather than re-reading the embedding table.
/// `options.language` / `options.file_pattern` are applied in memory here so the
/// shared corpus can be loaded unfiltered once and reused across many queries.
/// Returns only the ranked matches — index-build `progress` is a connection-level
/// snapshot the corpus path's callers (e.g. the review engine) do not consume.
pub fn search_loaded(
    corpus: &[LoadedChunk],
    query_text: &str,
    query_embedding: &[f32],
    options: &SearchCodeOptions,
) -> Vec<SearchCodeMatch> {
    let filtered: Vec<&LoadedChunk> = corpus
        .iter()
        .filter(|c| chunk_matches_filters(c, options))
        .collect();
    rank_loaded(&filtered, query_text, query_embedding, options).0
}

/// Whether a chunk passes a [`SearchCodeOptions`] language / file-pattern filter.
///
/// Approximates the SQL `LIKE` predicates [`load_embedded_chunks`] applies on the
/// connection path: `language` keeps chunks whose path ends in `.{ext}`, and
/// `file_pattern` keeps chunks whose path contains the pattern. Unlike SQL `LIKE`
/// this match is **case-sensitive** and treats the pattern **literally** (no `%`
/// / `_` wildcard interpretation), so the two paths are not byte-identical for
/// mixed-case extensions or wildcard patterns. The review engine — the only
/// corpus-path caller — never sets these filters, so the two paths coincide in
/// practice; a future corpus caller relying on full `LIKE` parity must account
/// for this.
fn chunk_matches_filters(chunk: &LoadedChunk, options: &SearchCodeOptions) -> bool {
    if let Some(langs) = &options.language {
        if !langs.is_empty()
            && !langs
                .iter()
                .any(|ext| chunk.file_path.ends_with(&format!(".{ext}")))
        {
            return false;
        }
    }
    if let Some(pattern) = &options.file_pattern {
        if !chunk.file_path.contains(pattern.as_str()) {
            return false;
        }
    }
    true
}

/// The shared ranking core: build a [`Doc`] per chunk, fuse the BM25 / trigram /
/// cosine signals via [`swissarmyhammer_search::search`], apply the
/// `min_fused_score` floor, and take the top `top_k`.
///
/// Returns the ranked matches, the number of chunks searched, and whether the
/// result was truncated by `top_k`. Both [`search_code`] (connection-loaded) and
/// [`search_loaded`] (corpus) route through here so the ranking logic lives in
/// exactly one place.
fn rank_loaded(
    chunks: &[&LoadedChunk],
    query_text: &str,
    query_embedding: &[f32],
    options: &SearchCodeOptions,
) -> (Vec<SearchCodeMatch>, usize, bool) {
    let total_chunks_searched = chunks.len();

    let docs: Vec<Doc> = chunks
        .iter()
        .enumerate()
        .map(|(i, chunk)| chunk_to_doc(i, chunk))
        .collect();

    // Ask the ranker for every passing hit (top_k = corpus size) so we can detect
    // truncation by comparing the passing count against the caller's `top_k`,
    // then truncate ourselves.
    let mut query = Query::new(query_text)
        .with_embedding(query_embedding.to_vec())
        .with_weights(SignalWeights::new(
            options.w_bm25,
            options.w_trigram,
            options.w_cosine,
        ))
        .with_top_k(docs.len());
    if let Some(floor) = options.min_fused_score {
        query = query.with_min_score(floor);
    }

    let hits = search(&docs, &query);
    let truncated = hits.len() > options.top_k;

    let matches: Vec<SearchCodeMatch> = hits
        .into_iter()
        .take(options.top_k)
        .map(|hit| hit_to_match(&hit, chunks))
        .collect();

    (matches, total_chunks_searched, truncated)
}

/// Map a [`Hit`] back to a [`SearchCodeMatch`] via the chunk's corpus index,
/// which was stashed in [`Hit::id`] by [`chunk_to_doc`].
fn hit_to_match(hit: &Hit, chunks: &[&LoadedChunk]) -> SearchCodeMatch {
    let index: usize = hit
        .id
        .parse()
        .expect("Doc id is the chunk's corpus index, set in chunk_to_doc");
    let chunk = chunks[index];
    SearchCodeMatch {
        file_path: chunk.file_path.clone(),
        start_line: chunk.start_line,
        end_line: chunk.end_line,
        symbol_path: chunk.symbol_path.clone(),
        text: chunk.text.clone(),
        score: hit.score,
        signals: hit.signals,
    }
}

/// Summarise embedding-pass progress from `indexed_files`.
///
/// Returns `Some(...)` when `embedded_files < total_files` (the embedding
/// pass is still running), `None` when every tracked file is embedded or
/// when there are no tracked files at all.
///
/// # Errors
///
/// Returns [`CodeContextError::Database`] on SQLite failures.
fn compute_indexing_progress(
    conn: &Connection,
) -> Result<Option<IndexingProgress>, CodeContextError> {
    // One round-trip: total file count and embedded file count.
    let (total_files, embedded_files): (i64, i64) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(embedded), 0) FROM indexed_files",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;

    if total_files == 0 || embedded_files >= total_files {
        return Ok(None);
    }

    let total = total_files as u64;
    let embedded = embedded_files as u64;
    let embedded_percent = (embedded as f64 / total as f64) * 100.0;
    let message = format!(
        "Embedding still in progress: {}/{} files ({:.0}%). Results may be incomplete — retry shortly for full coverage.",
        embedded, total, embedded_percent
    );

    Ok(Some(IndexingProgress {
        embedded_files: embedded,
        total_files: total,
        embedded_percent,
        message,
    }))
}

/// Load **every** embedded chunk from `ts_chunks`, unfiltered.
///
/// This is the reusable-corpus loader: it materializes the whole embedding table
/// once so a caller running many comparisons against the same index (the review
/// engine's probe runner — duplicates per changed file, similar per added
/// function) pays the load cost a single time instead of re-reading and
/// re-deserializing the table per call. Any size / language / path narrowing is
/// applied in memory by the corpus consumers ([`search_loaded`],
/// [`find_duplicates_in`](crate::find_duplicates_in)).
///
/// # Errors
///
/// Returns [`CodeContextError::Database`] on SQLite failures.
pub fn load_all_embedded_chunks(conn: &Connection) -> Result<Vec<LoadedChunk>, CodeContextError> {
    let mut stmt = conn.prepare(
        "SELECT file_path, start_line, end_line, symbol_path, text, embedding
         FROM ts_chunks
         WHERE embedding IS NOT NULL",
    )?;
    let rows = stmt.query_map([], row_to_loaded_chunk)?;
    let mut chunks = Vec::new();
    for row in rows {
        chunks.push(row?);
    }
    Ok(chunks)
}

/// Map a `ts_chunks` row (the standard 6-column projection) to a [`LoadedChunk`].
fn row_to_loaded_chunk(row: &rusqlite::Row) -> rusqlite::Result<LoadedChunk> {
    let blob: Vec<u8> = row.get(5)?;
    Ok(LoadedChunk {
        file_path: row.get(0)?,
        start_line: row.get(1)?,
        end_line: row.get(2)?,
        symbol_path: row.get(3)?,
        text: row.get(4)?,
        embedding: deserialize_embedding(&blob),
    })
}

/// Load chunk rows that have embeddings from `ts_chunks`.
fn load_embedded_chunks(
    conn: &Connection,
    options: &SearchCodeOptions,
) -> Result<Vec<LoadedChunk>, CodeContextError> {
    let mut sql = String::from(
        "SELECT file_path, start_line, end_line, symbol_path, text, embedding FROM ts_chunks WHERE embedding IS NOT NULL",
    );

    if let Some(ref langs) = options.language {
        if !langs.is_empty() {
            let like_clauses: Vec<String> = langs
                .iter()
                .map(|ext| format!("file_path LIKE '%.{ext}'"))
                .collect();
            sql.push_str(&format!(" AND ({})", like_clauses.join(" OR ")));
        }
    }

    if let Some(ref pattern) = options.file_pattern {
        sql.push_str(&format!(" AND file_path LIKE '%{pattern}%'"));
    }

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], row_to_loaded_chunk)?;

    let mut chunks = Vec::new();
    for row in rows {
        chunks.push(row?);
    }
    Ok(chunks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{insert_file_simple as insert_file, test_db};

    fn insert_chunk_with_embedding(
        conn: &Connection,
        file_path: &str,
        start_line: u32,
        end_line: u32,
        symbol_path: Option<&str>,
        text: &str,
        embedding: &[f32],
    ) {
        let blob = serialize_embedding(embedding);
        conn.execute(
            "INSERT INTO ts_chunks (file_path, start_byte, end_byte, start_line, end_line, symbol_path, text, embedding)
             VALUES (?1, 0, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![file_path, text.len() as i64, start_line, end_line, symbol_path, text, blob],
        )
        .unwrap();
    }

    fn insert_chunk_without_embedding(
        conn: &Connection,
        file_path: &str,
        start_line: u32,
        end_line: u32,
        text: &str,
    ) {
        conn.execute(
            "INSERT INTO ts_chunks (file_path, start_byte, end_byte, start_line, end_line, text)
             VALUES (?1, 0, ?2, ?3, ?4, ?5)",
            rusqlite::params![file_path, text.len() as i64, start_line, end_line, text],
        )
        .unwrap();
    }

    /// Cosine-only options (BM25/trigram off) — recovers the legacy embedding
    /// ranking so the migrated similarity tests still pin embedding order.
    fn cosine_only() -> SearchCodeOptions {
        SearchCodeOptions {
            w_bm25: 0.0,
            w_trigram: 0.0,
            w_cosine: 1.0,
            ..Default::default()
        }
    }

    #[test]
    fn test_search_code_ranking() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs");

        // Query vector points in the direction [1, 0, 0].
        let query = vec![1.0, 0.0, 0.0];

        // Chunk A: very similar to query.
        insert_chunk_with_embedding(
            &conn,
            "src/main.rs",
            1,
            5,
            Some("main"),
            "fn main() {}",
            &[0.9, 0.1, 0.0],
        );
        // Chunk B: somewhat similar.
        insert_chunk_with_embedding(
            &conn,
            "src/main.rs",
            6,
            10,
            Some("helper"),
            "fn helper() {}",
            &[0.5, 0.5, 0.0],
        );
        // Chunk C: not similar (orthogonal).
        insert_chunk_with_embedding(
            &conn,
            "src/main.rs",
            11,
            15,
            None,
            "const X: i32 = 1;",
            &[0.0, 0.0, 1.0],
        );

        // Cosine-only fusion reproduces the old embedding ranking: A then B then C.
        let result = search_code(&conn, "main", &query, &cosine_only()).unwrap();

        assert_eq!(result.matches.len(), 3);
        assert_eq!(result.matches[0].symbol_path.as_deref(), Some("main"));
        assert!(result.matches[0].score >= result.matches[1].score);
        assert!(result.matches[1].score >= result.matches[2].score);
        assert_eq!(result.total_chunks_searched, 3);
    }

    #[test]
    fn test_search_code_skips_null_embeddings() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs");

        let query = vec![1.0, 0.0];
        insert_chunk_with_embedding(
            &conn,
            "src/lib.rs",
            1,
            3,
            None,
            "fn has_embedding() {}",
            &[1.0, 0.0],
        );
        insert_chunk_without_embedding(&conn, "src/lib.rs", 4, 6, "fn no_embedding() {}");

        let result = search_code(&conn, "has_embedding", &query, &cosine_only()).unwrap();

        // Only the chunk with an embedding is loaded and searched.
        assert_eq!(result.total_chunks_searched, 1);
        assert_eq!(result.matches.len(), 1);
    }

    #[test]
    fn test_search_code_top_k() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs");

        let query = vec![1.0, 0.0];

        // Insert 5 chunks all similar to query.
        for i in 0..5 {
            let text = format!("fn func_{i}() {{}}");
            insert_chunk_with_embedding(
                &conn,
                "src/lib.rs",
                i * 3 + 1,
                i * 3 + 3,
                None,
                &text,
                &[1.0 - (i as f32 * 0.01), 0.1],
            );
        }

        let opts = SearchCodeOptions {
            top_k: 2,
            ..cosine_only()
        };
        let result = search_code(&conn, "func", &query, &opts).unwrap();

        assert_eq!(result.matches.len(), 2);
        assert!(result.truncated);
        assert_eq!(result.total_chunks_searched, 5);
    }

    /// Converted from the old `min_similarity` filter test: a `min_fused_score`
    /// floor on the NORMALIZED [0,1] fused score keeps only the dominating hit.
    #[test]
    fn test_search_code_min_fused_score_filter() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs");

        let query = vec![1.0, 0.0];

        // High cosine — wins every present signal -> normalized score 1.0.
        insert_chunk_with_embedding(
            &conn,
            "src/lib.rs",
            1,
            3,
            None,
            "fn close() {}",
            &[0.99, 0.1],
        );
        // Low cosine — strictly below the top.
        insert_chunk_with_embedding(&conn, "src/lib.rs", 4, 6, None, "fn far() {}", &[0.1, 0.99]);

        // Cosine-only so ranking is purely embedding-driven; floor just under 1.0.
        let opts = SearchCodeOptions {
            min_fused_score: Some(0.999),
            ..cosine_only()
        };
        let result = search_code(&conn, "close", &query, &opts).unwrap();

        assert_eq!(result.matches.len(), 1);
        assert!(result.matches[0].text.contains("close"));
        assert!((result.matches[0].score - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_search_code_empty_index() {
        let conn = test_db();
        let query = vec![1.0, 0.0, 0.0];

        let result = search_code(&conn, "anything", &query, &SearchCodeOptions::default()).unwrap();
        assert!(result.matches.is_empty());
        assert_eq!(result.total_chunks_searched, 0);
        assert!(!result.truncated);
    }

    /// A chunk whose `symbol_path` exactly matches the query identifier but whose
    /// cosine is weak must still rank first, carried by the BM25/trigram signals
    /// over the high-weight symbol field. This is the fusion contract for code.
    #[test]
    fn test_search_code_lexical_symbol_match_beats_weak_cosine() {
        let conn = test_db();
        insert_file(&conn, "src/a.rs");
        insert_file(&conn, "src/b.rs");

        // The query embedding points at [0,1] — so the lexical chunk's [1,0]
        // embedding is orthogonal (cosine 0), while the decoy is aligned.
        let query_embedding = vec![0.0, 1.0];

        // Lexical match: symbol_path == query identifier, weak (orthogonal) cosine.
        insert_chunk_with_embedding(
            &conn,
            "src/a.rs",
            1,
            5,
            Some("parse_config"),
            "fn parse_config() { /* body */ }",
            &[1.0, 0.0],
        );
        // Decoy: strong cosine, no lexical overlap with the query identifier.
        insert_chunk_with_embedding(
            &conn,
            "src/b.rs",
            1,
            5,
            Some("unrelated_helper"),
            "fn unrelated_helper() { /* body */ }",
            &[0.0, 1.0],
        );

        let result = search_code(
            &conn,
            "parse_config",
            &query_embedding,
            &SearchCodeOptions::default(),
        )
        .unwrap();

        assert_eq!(
            result.matches[0].symbol_path.as_deref(),
            Some("parse_config"),
            "the exact symbol_path match must rank first despite weak cosine"
        );
    }

    /// Every match carries its per-signal breakdown.
    #[test]
    fn test_search_code_exposes_signals() {
        let conn = test_db();
        insert_file(&conn, "src/a.rs");

        let query = vec![1.0, 0.0];
        insert_chunk_with_embedding(
            &conn,
            "src/a.rs",
            1,
            5,
            Some("parse_config"),
            "fn parse_config() {}",
            &[1.0, 0.0],
        );

        let result =
            search_code(&conn, "parse_config", &query, &SearchCodeOptions::default()).unwrap();

        let m = &result.matches[0];
        // All three signals are present and finite; cosine is high for the
        // aligned embedding, lexical signals fired for the matching identifier.
        assert!(m.signals.cosine > 0.9, "cosine={}", m.signals.cosine);
        assert!(m.signals.bm25 > 0.0, "bm25={}", m.signals.bm25);
        assert!(m.signals.trigram > 0.0, "trigram={}", m.signals.trigram);
    }

    /// Flipping the signal weights on the SAME corpus reorders results: boosting
    /// `w_cosine` (and zeroing the lexical signals) ranks the strong-cosine decoy
    /// over the lexical match, the inverse of the default-fusion ordering.
    #[test]
    fn test_search_code_weights_reorder_results() {
        let conn = test_db();
        insert_file(&conn, "src/a.rs");
        insert_file(&conn, "src/b.rs");

        let query_embedding = vec![0.0, 1.0];
        // Lexical match, weak cosine.
        insert_chunk_with_embedding(
            &conn,
            "src/a.rs",
            1,
            5,
            Some("parse_config"),
            "fn parse_config() {}",
            &[1.0, 0.0],
        );
        // Strong cosine, no lexical overlap.
        insert_chunk_with_embedding(
            &conn,
            "src/b.rs",
            1,
            5,
            Some("unrelated_helper"),
            "fn unrelated_helper() {}",
            &[0.0, 1.0],
        );

        // Default fusion: lexical match wins.
        let lexical = search_code(
            &conn,
            "parse_config",
            &query_embedding,
            &SearchCodeOptions::default(),
        )
        .unwrap();
        assert_eq!(
            lexical.matches[0].symbol_path.as_deref(),
            Some("parse_config")
        );

        // Cosine-only: the strong-cosine decoy wins instead.
        let cosine = search_code(&conn, "parse_config", &query_embedding, &cosine_only()).unwrap();
        assert_eq!(
            cosine.matches[0].symbol_path.as_deref(),
            Some("unrelated_helper"),
            "boosting cosine must reorder the strong-cosine chunk to the top"
        );
    }

    /// Set the `embedded` flag for a specific file row.
    fn set_embedded(conn: &Connection, file_path: &str, embedded: i32) {
        conn.execute(
            "UPDATE indexed_files SET embedded = ?1 WHERE file_path = ?2",
            rusqlite::params![embedded, file_path],
        )
        .unwrap();
    }

    /// When embedded_files < total_files, the result must include a populated
    /// `IndexingProgress` so the caller knows results may be incomplete.
    #[test]
    fn test_search_code_progress_some_when_partially_embedded() {
        let conn = test_db();

        // 5 tracked files, 3 of them embedded.
        insert_file(&conn, "src/a.rs");
        insert_file(&conn, "src/b.rs");
        insert_file(&conn, "src/c.rs");
        insert_file(&conn, "src/d.rs");
        insert_file(&conn, "src/e.rs");
        set_embedded(&conn, "src/a.rs", 1);
        set_embedded(&conn, "src/b.rs", 1);
        set_embedded(&conn, "src/c.rs", 1);

        let query = vec![1.0, 0.0];
        insert_chunk_with_embedding(
            &conn,
            "src/a.rs",
            1,
            3,
            Some("foo"),
            "fn foo() {}",
            &[1.0, 0.0],
        );

        let result = search_code(&conn, "foo", &query, &SearchCodeOptions::default()).unwrap();

        assert_eq!(result.matches.len(), 1);
        let progress = result
            .progress
            .expect("progress must be Some when embedded_files < total_files");
        assert_eq!(progress.embedded_files, 3);
        assert_eq!(progress.total_files, 5);
        assert!((progress.embedded_percent - 60.0).abs() < 0.01);
        assert!(
            !progress.message.is_empty(),
            "progress message must be present"
        );
    }

    /// When the index is fully embedded, `progress` must be `None`.
    #[test]
    fn test_search_code_progress_none_when_fully_embedded() {
        let conn = test_db();

        insert_file(&conn, "src/a.rs");
        insert_file(&conn, "src/b.rs");
        set_embedded(&conn, "src/a.rs", 1);
        set_embedded(&conn, "src/b.rs", 1);

        let query = vec![1.0, 0.0];
        insert_chunk_with_embedding(&conn, "src/a.rs", 1, 3, None, "fn foo() {}", &[1.0, 0.0]);

        let result = search_code(&conn, "foo", &query, &SearchCodeOptions::default()).unwrap();
        assert!(
            result.progress.is_none(),
            "progress must be None when embedded_files == total_files"
        );
    }

    /// When there are zero tracked files, `progress` is `None`: there is
    /// nothing to be "in progress" against.
    #[test]
    fn test_search_code_progress_none_when_no_files() {
        let conn = test_db();
        let query = vec![1.0, 0.0, 0.0];

        let result = search_code(&conn, "anything", &query, &SearchCodeOptions::default()).unwrap();
        assert!(result.matches.is_empty());
        assert!(
            result.progress.is_none(),
            "progress must be None when there are no tracked files"
        );
    }

    /// The corpus path ([`load_all_embedded_chunks`] + [`search_loaded`]) must
    /// return byte-identical matches to the connection-backed [`search_code`].
    /// This pins the load-once review path to the single-call ranking so the two
    /// can never silently diverge.
    #[test]
    fn search_loaded_matches_search_code() {
        let conn = test_db();
        insert_file(&conn, "src/a.rs");
        insert_file(&conn, "src/b.rs");
        insert_file(&conn, "src/c.rs");

        let query = vec![1.0, 0.0, 0.0];
        insert_chunk_with_embedding(
            &conn,
            "src/a.rs",
            1,
            5,
            Some("a"),
            "fn a() {}",
            &[0.9, 0.1, 0.0],
        );
        insert_chunk_with_embedding(
            &conn,
            "src/b.rs",
            1,
            5,
            Some("b"),
            "fn b() {}",
            &[0.5, 0.5, 0.0],
        );
        insert_chunk_with_embedding(
            &conn,
            "src/c.rs",
            1,
            5,
            None,
            "const X: i32 = 1;",
            &[0.0, 0.0, 1.0],
        );

        let options = SearchCodeOptions::default();
        let via_conn = search_code(&conn, "a", &query, &options).unwrap();
        let corpus = load_all_embedded_chunks(&conn).unwrap();
        let via_corpus = search_loaded(&corpus, "a", &query, &options);

        assert_eq!(
            serde_json::to_value(&via_conn.matches).unwrap(),
            serde_json::to_value(&via_corpus).unwrap(),
            "the corpus path must rank identically to the connection-backed search"
        );
    }

    /// `search_loaded`'s in-memory `language` / `file_pattern` filters must
    /// actually exclude non-matching chunks (the corpus path's filter, which the
    /// equivalence test does not cover because the review passes no filters).
    #[test]
    fn search_loaded_applies_language_and_file_pattern_filters() {
        let conn = test_db();
        insert_file(&conn, "src/keep.rs");
        insert_file(&conn, "src/skip.py");
        insert_file(&conn, "vendor/keep.rs");

        let query = vec![1.0, 0.0];
        insert_chunk_with_embedding(
            &conn,
            "src/keep.rs",
            1,
            3,
            Some("keep"),
            "fn keep() {}",
            &[1.0, 0.0],
        );
        insert_chunk_with_embedding(
            &conn,
            "src/skip.py",
            1,
            3,
            Some("skip"),
            "def skip(): pass",
            &[1.0, 0.0],
        );
        insert_chunk_with_embedding(
            &conn,
            "vendor/keep.rs",
            1,
            3,
            Some("vendored"),
            "fn vendored() {}",
            &[1.0, 0.0],
        );
        let corpus = load_all_embedded_chunks(&conn).unwrap();

        // language filter: only the `.rs` chunks survive.
        let rs_only = search_loaded(
            &corpus,
            "keep",
            &query,
            &SearchCodeOptions {
                language: Some(vec!["rs".to_string()]),
                ..Default::default()
            },
        );
        assert_eq!(rs_only.len(), 2, "two .rs chunks");
        assert!(rs_only.iter().all(|m| m.file_path.ends_with(".rs")));

        // file_pattern filter: only chunks whose path contains `src/`.
        let src_only = search_loaded(
            &corpus,
            "keep",
            &query,
            &SearchCodeOptions {
                file_pattern: Some("src/".to_string()),
                ..Default::default()
            },
        );
        assert_eq!(src_only.len(), 2, "two chunks under src/");
        assert!(src_only.iter().all(|m| m.file_path.contains("src/")));

        // no filters: the whole corpus is ranked.
        let unfiltered = search_loaded(&corpus, "keep", &query, &SearchCodeOptions::default());
        assert_eq!(unfiltered.len(), 3, "all three chunks when unfiltered");
    }

    /// When no files have been embedded yet (all `embedded=0`), `progress`
    /// is still populated with embedded_files=0/total_files=N.
    #[test]
    fn test_search_code_progress_some_when_none_embedded() {
        let conn = test_db();

        insert_file(&conn, "src/a.rs");
        insert_file(&conn, "src/b.rs");

        let query = vec![1.0, 0.0];
        let result = search_code(&conn, "anything", &query, &SearchCodeOptions::default()).unwrap();

        assert!(result.matches.is_empty());
        let progress = result
            .progress
            .expect("progress must be Some when no files are embedded");
        assert_eq!(progress.embedded_files, 0);
        assert_eq!(progress.total_files, 2);
        assert!(progress.embedded_percent.abs() < 0.01);
    }
}
