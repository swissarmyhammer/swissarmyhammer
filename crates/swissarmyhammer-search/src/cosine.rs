//! Embedding cosine similarity and little-endian f32 blob (de)serialization.
//!
//! This is a self-contained re-implementation of the cosine-similarity contract
//! that previously lived in `model_embedding`, plus the f32-blob helpers that
//! `code-context` uses to persist embeddings. It is pure `dot/norm` arithmetic
//! with no external math dependency, so `swissarmyhammer-search` stays a leaf
//! crate.

/// Cosine similarity between two embedding vectors.
///
/// Computes `dot(a, b) / (‖a‖·‖b‖)`. The contract mirrors the old
/// `model_embedding::cosine_similarity`:
/// - identical vectors -> `1.0`
/// - orthogonal vectors -> `0.0`
/// - opposite vectors -> `-1.0`
/// - empty input -> `0.0`
/// - mismatched lengths -> `0.0`
/// - either vector having zero magnitude -> `0.0` (avoids division by zero)
///
/// # Parameters
/// - `a`, `b`: the two embedding vectors to compare.
///
/// # Returns
/// The cosine similarity in `[-1.0, 1.0]`, or `0.0` for the degenerate cases
/// above.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|y| y * y).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

/// Serialize an f32 slice into a little-endian byte blob.
///
/// Each `f32` becomes 4 little-endian bytes, concatenated in order. The inverse
/// is [`deserialize_embedding`].
///
/// # Parameters
/// - `embedding`: the vector to serialize.
///
/// # Returns
/// A `Vec<u8>` of length `4 * embedding.len()`.
pub fn serialize_embedding(embedding: &[f32]) -> Vec<u8> {
    embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Deserialize a little-endian f32 byte blob back into a vector.
///
/// Mirrors the helper in `code-context/src/ops/search_code.rs`. Trailing bytes
/// that do not form a full 4-byte group are ignored (via `chunks_exact`).
///
/// # Parameters
/// - `blob`: the little-endian byte blob produced by [`serialize_embedding`].
///
/// # Returns
/// The reconstructed `Vec<f32>`.
pub fn deserialize_embedding(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_vectors_score_one() {
        let v = vec![1.0, 2.0, 3.0];
        assert!((cosine_similarity(&v, &v) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn orthogonal_vectors_score_zero() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert!(cosine_similarity(&a, &b).abs() < 1e-6);
    }

    #[test]
    fn opposite_vectors_score_negative_one() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![-1.0, -2.0, -3.0];
        assert!((cosine_similarity(&a, &b) + 1.0).abs() < 1e-6);
    }

    #[test]
    fn empty_input_scores_zero() {
        let empty: Vec<f32> = vec![];
        assert_eq!(cosine_similarity(&empty, &empty), 0.0);
    }

    #[test]
    fn mismatched_length_scores_zero() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn zero_magnitude_scores_zero() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 2.0, 3.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn serialize_deserialize_round_trip_exact() {
        let v = vec![1.0_f32, -2.5, 3.125, 0.0, f32::MIN, f32::MAX];
        let blob = serialize_embedding(&v);
        assert_eq!(blob.len(), v.len() * 4);
        assert_eq!(deserialize_embedding(&blob), v);
    }
}
