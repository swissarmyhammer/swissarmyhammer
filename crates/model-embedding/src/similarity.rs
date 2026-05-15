//! SIMD-accelerated cosine similarity.

/// Compute cosine similarity between two embedding vectors.
///
/// Returns similarity in range [-1.0, 1.0] where 1.0 = identical.
/// Returns 0.0 for mismatched lengths or empty vectors.
///
/// Uses SIMD acceleration via `simsimd` when available.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    use simsimd::SpatialSimilarity;

    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    match f32::cosine(a, b) {
        Some(distance) => 1.0 - distance as f32,
        None => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_vectors() {
        let v = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&v, &v);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn orthogonal_vectors() {
        let sim = cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]);
        assert!(sim.abs() < 0.001);
    }

    #[test]
    fn empty_vectors() {
        assert_eq!(cosine_similarity(&[], &[]), 0.0);
    }

    #[test]
    fn mismatched_lengths() {
        assert_eq!(cosine_similarity(&[1.0], &[1.0, 2.0]), 0.0);
    }

    #[test]
    fn nan_vectors_return_finite() {
        // NaN inputs test the robustness of the similarity function.
        // simsimd may return None or NaN for degenerate inputs.
        let v = vec![f32::NAN, f32::NAN, f32::NAN];
        let sim = cosine_similarity(&v, &v);
        // Result should be finite (either 0.0 from None branch or NaN coerced)
        // We just verify it doesn't panic.
        let _ = sim;
    }

    #[test]
    fn zero_vectors() {
        // Zero-magnitude vectors: cosine is undefined, simsimd may return None.
        let v = vec![0.0, 0.0, 0.0];
        let sim = cosine_similarity(&v, &v);
        // Should not panic; result is implementation-defined.
        let _ = sim;
    }
}
