//! `search code` answering while the index is still filling.

use model_embedding::TextEmbedder;

use crate::mcp::tools::code_context::execute::search_code_with_query_embedding;
use crate::mcp::tools::code_context::indexing::index_discovered_files_with_embedder;

use super::support::{extract_text, make_context_with_dir, make_tiny_indexable_project};

// -----------------------------------------------------------------------
// search code: readiness gate removal
//
// `execute_search_code` used to bail out with an "Index not ready"
// placeholder string when the tree-sitter pass wasn't done. The gate is
// gone: `search code` now always returns a `SearchCodeResult` and the
// caller learns about partial coverage via the `progress` field.
//
// The dispatch path runs a real embedder, so this unit test exercises
// the inner function `search_code_with_query_embedding` directly with a
// caller-supplied embedding vector. That keeps the test fast and
// deterministic while still proving the gate is gone — the test would
// fail with a "not ready" placeholder if it weren't.
// -----------------------------------------------------------------------

/// When files exist in `indexed_files` but none are embedded yet, the
/// inner search must return a `SearchCodeResult` (possibly with empty
/// matches) carrying a populated `progress` field — never the old
/// "Index not ready" placeholder string.
#[tokio::test]
async fn test_search_code_returns_result_with_progress_when_not_embedded() {
    let (_tmp, root, _shared_db) = make_tiny_indexable_project().await;
    let ctx = make_context_with_dir(root.clone());

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("search code"));
    args.insert("query".to_string(), serde_json::json!("anything"));

    // Use a tiny dummy embedding — the search returns no matches because
    // no chunk embeddings exist yet, but it must still succeed and
    // produce a `SearchCodeResult` JSON, not the readiness placeholder.
    let dummy_query_embedding = vec![1.0f32, 0.0, 0.0];
    let result = search_code_with_query_embedding(&args, &ctx, "anything", &dummy_query_embedding)
        .expect("search code should succeed without the readiness gate");

    let text = extract_text(&result);
    assert!(
        !text.contains("Index not ready"),
        "search code must not return the readiness placeholder, got: {text}"
    );

    // The body must parse as a SearchCodeResult JSON with the progress
    // field populated (3 files exist, 0 are embedded).
    let parsed: serde_json::Value =
        serde_json::from_str(text).expect("result must be JSON-encoded SearchCodeResult");
    assert!(
        parsed.get("matches").is_some(),
        "result must have a `matches` field"
    );
    let progress = parsed
        .get("progress")
        .expect("result must have a `progress` field");
    assert!(
        !progress.is_null(),
        "progress must be populated when embedded_files < total_files, got null"
    );
    assert!(
        progress.get("embedded_files").and_then(|v| v.as_u64()) == Some(0),
        "embedded_files should be 0 when no files have been embedded yet"
    );
    let total = progress
        .get("total_files")
        .and_then(|v| v.as_u64())
        .expect("total_files must be present and numeric");
    assert!(total > 0, "total_files should be > 0, got {total}");
}

/// A `search code` match exposes the fused-search response shape — a
/// normalized `score` and a `signals { bm25, trigram, cosine }` breakdown —
/// NOT the old single `similarity` field. This indexes a tiny project with
/// the mock embedder so real embedded chunks exist, then searches with a
/// query that lexically matches a chunk and asserts on the first match's
/// JSON shape.
#[tokio::test]
async fn test_search_code_match_exposes_score_and_signals_not_similarity() {
    use model_embedding::mock::MockEmbedder;

    let (_tmp, root, shared_db) = make_tiny_indexable_project().await;
    let dim = 8;
    let embedder: std::sync::Arc<dyn TextEmbedder> = std::sync::Arc::new(MockEmbedder::new(dim));
    index_discovered_files_with_embedder(
        &root,
        shared_db.clone(),
        Some(embedder),
        swissarmyhammer_code_context::noop_reporter(),
        swissarmyhammer_code_context::new_shutdown_flag(),
    )
    .await;

    let ctx = make_context_with_dir(root.clone());
    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("search code"));
    // "add" lexically matches the `pub fn add` chunk in src/lib.rs so the
    // BM25/trigram signals produce a non-empty fused result.
    args.insert("query".to_string(), serde_json::json!("add"));

    // The query embedding must match the mock embedder's dimension so the
    // cosine signal can be computed against the stored chunk embeddings.
    let query_embedding = vec![0.1f32; dim];
    let result = search_code_with_query_embedding(&args, &ctx, "add", &query_embedding)
        .expect("search code should succeed");

    let text = extract_text(&result);
    let parsed: serde_json::Value =
        serde_json::from_str(text).expect("result must be JSON-encoded SearchCodeResult");
    let matches = parsed
        .get("matches")
        .and_then(|v| v.as_array())
        .expect("result must have a `matches` array");
    assert!(
        !matches.is_empty(),
        "expected at least one match for a query that lexically hits an embedded chunk, got: {text}"
    );

    let first = &matches[0];
    assert!(
        first.get("score").and_then(|v| v.as_f64()).is_some(),
        "each match must expose a numeric `score`, got: {first}"
    );
    let signals = first
        .get("signals")
        .and_then(|v| v.as_object())
        .expect("each match must expose a `signals` object");
    for key in ["bm25", "trigram", "cosine"] {
        assert!(
            signals.get(key).and_then(|v| v.as_f64()).is_some(),
            "signals must expose a numeric `{key}`, got: {first}"
        );
    }
    assert!(
        first.get("similarity").is_none(),
        "the old `similarity` field must be gone from a search code match, got: {first}"
    );
}
